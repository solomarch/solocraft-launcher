//! Maps / worlds: put a world into an instance's `saves` folder from a .zip, a folder or a link,
//! and read an optional online list (`maps.json`) so maps never have to be bundled with the launcher.
use crate::download::{client, download_file_with, ByteCb, DlTask};
use crate::events::{emit_progress, EventSink};
use crate::paths::Paths;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub const DEFAULT_MANIFEST: &str = "https://raw.githubusercontent.com/solomarch/sm-craft/main/maps.json";
const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub mc_version: String,
    pub size_mb: u64,
    pub sha256: String,
}

/// Reads the online list. A missing or broken list is not an error: it just means "no maps listed".
pub async fn list_online() -> Vec<MapEntry> {
    let url = std::env::var("SOLOCRAFT_MAPS_URL").unwrap_or_else(|_| DEFAULT_MANIFEST.to_string());
    let Ok(resp) = client().get(&url).timeout(std::time::Duration::from_secs(15)).send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    match resp.text().await {
        Ok(t) => parse_manifest(&t),
        Err(_) => Vec::new(),
    }
}

pub fn parse_manifest(text: &str) -> Vec<MapEntry> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    let arr = if v.is_array() { v.clone() } else { v.get("maps").cloned().unwrap_or_default() };
    let Some(items) = arr.as_array() else { return Vec::new() };
    items
        .iter()
        .filter_map(|x| serde_json::from_value::<MapEntry>(x.clone()).ok())
        .filter(|m| !m.name.is_empty() && m.url.starts_with("https://"))
        .map(|mut m| {
            if m.id.is_empty() {
                m.id = m.name.to_lowercase().replace(' ', "-");
            }
            m
        })
        .collect()
}

fn saves_dir(paths: &Paths, instance: &str) -> PathBuf {
    paths.game_dir(instance).join("saves")
}

/// Worlds already in the instance (folders that contain level.dat).
pub fn installed(paths: &Paths, instance: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(saves_dir(paths, instance)) {
        for e in rd.flatten() {
            if e.path().join("level.dat").is_file() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

fn clean_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let n = n.trim().trim_matches('.').to_string();
    if n.is_empty() { "World".to_string() } else { n }
}

fn unique_dir(saves: &Path, name: &str) -> PathBuf {
    let base = clean_name(name);
    let mut p = saves.join(&base);
    let mut n = 2;
    while p.exists() {
        p = saves.join(format!("{base} ({n})"));
        n += 1;
    }
    p
}

/// Removes one world by folder name (only if it really is a world).
pub fn remove(paths: &Paths, instance: &str, name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name == ".." || name.is_empty() {
        bail!("bad map name");
    }
    let dir = saves_dir(paths, instance).join(name);
    if !dir.join("level.dat").is_file() {
        bail!("that is not a world folder");
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

fn safe_rel(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return None, // absolute path, drive letter or ".." : refuse
        }
    }
    Some(out)
}

/// Extracts a world from a zip. The world root is where level.dat is, so wrapper folders do not matter.
pub fn install_zip(saves: &Path, zip_path: &Path, fallback_name: &str) -> Result<String> {
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| anyhow!("that file is not a valid zip"))?;
    let mut root: Option<PathBuf> = None;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let f = zip.by_index(i)?;
        total += f.size();
        if let Some(rel) = safe_rel(f.name()) {
            if rel.file_name().map(|n| n == "level.dat").unwrap_or(false) {
                let parent = rel.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                if root.as_ref().map(|r| parent.components().count() < r.components().count()).unwrap_or(true) {
                    root = Some(parent);
                }
            }
        }
    }
    if total > MAX_UNPACKED {
        bail!("this map is too large to unpack");
    }
    let root = root.ok_or_else(|| anyhow!("no world found in this zip (it has no level.dat)"))?;
    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| fallback_name.to_string());
    std::fs::create_dir_all(saves)?;
    let dest = unique_dir(saves, &name);
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let Some(rel) = safe_rel(f.name()) else { continue };
        let Ok(inner) = rel.strip_prefix(&root) else { continue };
        if inner.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(inner);
        if f.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut f, &mut w)?;
        }
    }
    Ok(dest.file_name().unwrap().to_string_lossy().to_string())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let ty = e.file_type()?;
        if ty.is_symlink() {
            continue;
        }
        let to = dst.join(e.file_name());
        if ty.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

/// Copies a world folder (or a folder that contains one) into saves.
pub fn install_folder(saves: &Path, folder: &Path) -> Result<String> {
    let mut best: Option<PathBuf> = None;
    for e in walkdir::WalkDir::new(folder).max_depth(3).into_iter().flatten() {
        if e.file_name() == "level.dat" && e.file_type().is_file() {
            let d = e.path().parent().unwrap().to_path_buf();
            if best.as_ref().map(|b| d.components().count() < b.components().count()).unwrap_or(true) {
                best = Some(d);
            }
        }
    }
    let src = best.ok_or_else(|| anyhow!("no world found in that folder (it has no level.dat)"))?;
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "World".into());
    std::fs::create_dir_all(saves)?;
    let dest = unique_dir(saves, &name);
    copy_dir(&src, &dest)?;
    Ok(dest.file_name().unwrap().to_string_lossy().to_string())
}

/// Installs from a path chosen by the user: a .zip file or a folder.
pub fn install_path(paths: &Paths, instance: &str, path: &str) -> Result<String> {
    let p = Path::new(path);
    let saves = saves_dir(paths, instance);
    if p.is_dir() {
        install_folder(&saves, p)
    } else {
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "World".into());
        install_zip(&saves, p, &stem)
    }
}

/// Downloads a map zip (https only) and installs it, with a progress bar.
pub async fn install_url(
    paths: &Paths,
    instance: &str,
    url: &str,
    name: &str,
    sha256: &str,
    sink: Arc<dyn EventSink>,
    task: &str,
) -> Result<String> {
    if !url.starts_with("https://") {
        bail!("map links must start with https://");
    }
    let tmp_dir = paths.meta().join("maps-tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(format!("{}.zip", uuid::Uuid::new_v4()));
    let mut dl = DlTask::new(url, &tmp);
    if !sha256.is_empty() {
        dl.sha256 = Some(sha256.to_string());
    }
    let (s2, t2) = (sink.clone(), task.to_string());
    let cb: ByteCb = Arc::new(move |cur, total| emit_progress(&*s2, &t2, "Downloading map", cur, total));
    let res = download_file_with(&dl, Some(cb)).await;
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    emit_progress(&*sink, task, "Unpacking map", 0, 0);
    let saves = saves_dir(paths, instance);
    let fallback = if name.is_empty() { "World".to_string() } else { name.to_string() };
    let tmp2 = tmp.clone();
    let out = tokio::task::spawn_blocking(move || install_zip(&saves, &tmp2, &fallback)).await?;
    let _ = std::fs::remove_file(&tmp);
    emit_progress(&*sink, task, "Done", 1, 1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        for (n, d) in files {
            z.start_file(*n, o).unwrap();
            z.write_all(d).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn zip_with_wrapper_folder_installs_at_world_root() {
        let t = std::env::temp_dir().join(format!("sc-maps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&t).unwrap();
        let z = t.join("m.zip");
        make_zip(&z, &[("pack/Arena/level.dat", b"x"), ("pack/Arena/region/r.0.0.mca", b"y"), ("readme.txt", b"hi")]);
        let saves = t.join("saves");
        let name = install_zip(&saves, &z, "fallback").unwrap();
        assert_eq!(name, "Arena");
        assert!(saves.join("Arena/level.dat").is_file());
        assert!(saves.join("Arena/region/r.0.0.mca").is_file());
        // a second install never overwrites
        assert_eq!(install_zip(&saves, &z, "x").unwrap(), "Arena (2)");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn zip_without_world_and_path_tricks_are_refused() {
        let t = std::env::temp_dir().join(format!("sc-maps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&t).unwrap();
        let z = t.join("bad.zip");
        make_zip(&z, &[("a.txt", b"x")]);
        assert!(install_zip(&t.join("saves"), &z, "x").is_err());
        let z2 = t.join("evil.zip");
        make_zip(&z2, &[("../../evil/level.dat", b"x")]);
        assert!(install_zip(&t.join("saves"), &z2, "x").is_err());
        assert!(!t.join("evil").exists());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn folder_import_and_manifest() {
        let t = std::env::temp_dir().join(format!("sc-maps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(t.join("src/Parkour")).unwrap();
        std::fs::write(t.join("src/Parkour/level.dat"), b"x").unwrap();
        assert_eq!(install_folder(&t.join("saves"), &t.join("src")).unwrap(), "Parkour");
        let m = parse_manifest(r#"{"maps":[{"name":"PvP One","url":"https://x.y/a.zip"},{"name":"bad","url":"http://x"}]}"#);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "pvp-one");
        let _ = std::fs::remove_dir_all(&t);
    }
}
