//! Importing a modpack or instance from a file on disk: Modrinth `.mrpack`
//! (or a `.zip` with the same layout) and Prism / MultiMC / PolyMC instance zips.

use crate::events::{emit_progress, EventSink};
use crate::models::Instance;
use crate::modrinth::{install_mrpack_file, safe_rel};
use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

const MAX_ARCHIVE_OUTPUT: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 100_000;

#[derive(Debug, PartialEq)]
pub enum Kind {
    Modrinth,
    /// Prism/MultiMC export; `prefix` is the folder inside the zip that holds mmc-pack.json.
    Prism { prefix: String },
    CurseForge,
    Unknown,
}

pub fn detect(zip_path: &Path) -> Result<Kind> {
    let f = std::fs::File::open(zip_path).context("opening the file")?;
    let zip = zip::ZipArchive::new(f).map_err(|_| anyhow!("That file is not a zip / .mrpack archive"))?;
    let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    if names.iter().any(|n| n == "modrinth.index.json") {
        return Ok(Kind::Modrinth);
    }
    for n in &names {
        if n == "mmc-pack.json" || (n.ends_with("/mmc-pack.json") && n.matches('/').count() == 1) {
            let prefix = n.trim_end_matches("mmc-pack.json").to_string();
            return Ok(Kind::Prism { prefix });
        }
    }
    if names.iter().any(|n| n == "manifest.json") && names.iter().any(|n| n.starts_with("overrides/")) {
        return Ok(Kind::CurseForge);
    }
    Ok(Kind::Unknown)
}

#[derive(Deserialize)]
struct MmcPack {
    components: Vec<MmcComponent>,
}
#[derive(Deserialize)]
struct MmcComponent {
    uid: String,
    #[serde(default)]
    version: String,
}

/// (minecraft version, loader, loader version) from a Prism component list.
fn components_to_loader(c: &[MmcComponent]) -> Result<(String, &'static str, String)> {
    let get = |uid: &str| c.iter().find(|x| x.uid == uid).map(|x| x.version.clone());
    let mc = get("net.minecraft").ok_or_else(|| anyhow!("The instance does not say which Minecraft version it uses"))?;
    Ok(if let Some(v) = get("net.fabricmc.fabric-loader") {
        (mc, "fabric", v)
    } else if let Some(v) = get("org.quiltmc.quilt-loader") {
        (mc, "quilt", v)
    } else if let Some(v) = get("net.neoforged") {
        (mc, "neoforge", v)
    } else if let Some(v) = get("net.minecraftforge") {
        (mc, "forge", v)
    } else {
        (mc, "vanilla", String::new())
    })
}

fn cfg_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("name="))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn import_file(
    store: &Store,
    file: &Path,
    name: Option<&str>,
    group: &str,
    sink: &dyn EventSink,
    task: &str,
) -> Result<Instance> {
    let p = file.to_path_buf();
    let kind = tokio::task::spawn_blocking(move || detect(&p)).await??;
    match kind {
        Kind::Modrinth => install_mrpack_file(store, file, name, group, sink, task).await,
        Kind::Prism { prefix } => import_prism(store, file, &prefix, name, group, sink, task).await,
        Kind::CurseForge => Err(anyhow!(
            "This is a CurseForge modpack. SM Craft cannot install those yet (they need a CurseForge API key). Modrinth .mrpack files and Prism/MultiMC instance zips work."
        )),
        Kind::Unknown => Err(anyhow!(
            "SM Craft did not recognise this archive. Supported: Modrinth .mrpack (or .zip), and Prism/MultiMC/PolyMC instance exports."
        )),
    }
}

async fn import_prism(
    store: &Store,
    file: &Path,
    prefix: &str,
    name: Option<&str>,
    group: &str,
    sink: &dyn EventSink,
    task: &str,
) -> Result<Instance> {
    let (pack, cfg) = {
        let (f, pre) = (file.to_path_buf(), prefix.to_string());
        tokio::task::spawn_blocking(move || -> Result<(MmcPack, String)> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(f)?)?;
            if zip.len() > MAX_ARCHIVE_ENTRIES { return Err(anyhow!("archive contains too many entries")); }
            let mut s = String::new();
            zip.by_name(&format!("{pre}mmc-pack.json"))?.read_to_string(&mut s)?;
            let pack: MmcPack = serde_json::from_str(&s).context("mmc-pack.json is damaged")?;
            let mut cfg = String::new();
            if let Ok(mut e) = zip.by_name(&format!("{pre}instance.cfg")) {
                let _ = e.read_to_string(&mut cfg);
            }
            Ok((pack, cfg))
        })
        .await??
    };
    let (mc, loader, lv) = components_to_loader(&pack.components)?;
    let display = name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| cfg_name(&cfg))
        .unwrap_or_else(|| "Imported instance".into());
    let mut inst = store.create_instance(&display, &mc, group)?;
    inst.loader = loader.to_string();
    inst.loader_version = lv;
    store.save_instance(&inst)?;

    emit_progress(sink, task, "Copying instance files", 0, 0);
    let game_dir = store.paths.game_dir(&inst.id);
    let (f, pre, g) = (file.to_path_buf(), prefix.to_string(), game_dir);
    let res = tokio::task::spawn_blocking(move || -> Result<usize> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(f)?)?;
        if zip.len() > MAX_ARCHIVE_ENTRIES { return Err(anyhow!("archive contains too many entries")); }
        let roots = [format!("{pre}.minecraft/"), format!("{pre}minecraft/")];
        let mut n = 0;
        let mut expanded = 0u64;
        for i in 0..zip.len() {
            let mut e = zip.by_index(i)?;
            let name = e.name().to_string();
            let Some(rest) = roots.iter().find_map(|r| name.strip_prefix(r.as_str())) else { continue };
            if e.is_dir() || rest.is_empty() {
                continue;
            }
            let Some(rel) = safe_rel(rest) else { continue };
            expanded = expanded.saturating_add(e.size());
            if expanded > MAX_ARCHIVE_OUTPUT { return Err(anyhow!("archive expands beyond the 8 GiB safety limit")); }
            let out = g.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::io::copy(&mut e, &mut std::fs::File::create(&out)?)?;
            n += 1;
        }
        Ok(n)
    })
    .await
    .map_err(|e| anyhow!("{e}"))
    .and_then(|r| r);
    if let Err(e) = res {
        let _ = store.delete_instance(&inst.id);
        return Err(e);
    }
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sc-import-{}-{name}", std::process::id()));
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for (n, c) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        p
    }

    #[test]
    fn detects_kinds() {
        let a = make_zip("a.mrpack", &[("modrinth.index.json", "{}")]);
        assert_eq!(detect(&a).unwrap(), Kind::Modrinth);
        let b = make_zip("b.zip", &[("My Pack/mmc-pack.json", "{}"), ("My Pack/.minecraft/options.txt", "x")]);
        assert_eq!(detect(&b).unwrap(), Kind::Prism { prefix: "My Pack/".into() });
        let c = make_zip("c.zip", &[("manifest.json", "{}"), ("overrides/a", "x")]);
        assert_eq!(detect(&c).unwrap(), Kind::CurseForge);
        let d = make_zip("d.zip", &[("readme.txt", "x")]);
        assert_eq!(detect(&d).unwrap(), Kind::Unknown);
    }

    #[test]
    fn maps_components() {
        let c = vec![
            MmcComponent { uid: "net.minecraft".into(), version: "1.20.1".into() },
            MmcComponent { uid: "net.minecraftforge".into(), version: "47.2.0".into() },
        ];
        assert_eq!(components_to_loader(&c).unwrap(), ("1.20.1".into(), "forge", "47.2.0".into()));
        assert_eq!(cfg_name("a=b\nname=Cool Pack\n").as_deref(), Some("Cool Pack"));
    }
}
