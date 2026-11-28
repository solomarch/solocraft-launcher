use crate::download::{download_file, download_many, DlTask};
use crate::events::{emit_progress, EventSink};
use crate::mcjson::{rules_allow, AssetIndex, Library, McVersion};
use crate::paths::Paths;
use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct InstallPlan {
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<(PathBuf, Vec<String>)>,
    pub asset_index_id: String,
    pub game_assets: PathBuf,
    pub logging_arg: Option<String>,
}

/// group:artifact:version[:classifier] -> relative maven path
pub fn maven_path(name: &str, classifier: Option<&str>) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.to_string();
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let cls = classifier
        .map(|c| format!("-{c}"))
        .or_else(|| parts.get(3).map(|c| format!("-{c}")))
        .unwrap_or_default();
    format!("{group}/{artifact}/{version}/{artifact}-{version}{cls}.jar")
}

fn native_key(lib: &Library) -> Option<String> {
    let map = lib.natives.as_ref()?;
    let raw = map.get(crate::mcjson::current_os())?;
    let arch = if std::env::consts::ARCH == "x86" { "32" } else { "64" };
    Some(raw.replace("${arch}", arch))
}

pub async fn install_version(
    paths: &Paths,
    v: &McVersion,
    game_dir: &Path,
    sink: &dyn EventSink,
    task: &str,
) -> Result<InstallPlan> {
    let features: HashMap<String, bool> = HashMap::new();

    // 1. Client jar and libraries
    let client = v
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or_else(|| anyhow!("Version {} has no client download", v.id))?;
    let client_jar = paths.client_jar(&v.id);
    let mut tasks = vec![DlTask::new(client.url.clone(), client_jar.clone())
        .with_sha1(client.sha1.clone())
        .with_size(client.size)];

    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut natives: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for lib in &v.libraries {
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules, &features) {
                continue;
            }
        }
        let dl = match &lib.downloads {
            Some(d) => d,
            None => {
                // Loader libraries: plain maven coordinates plus a repository URL.
                if let Some(base) = &lib.url {
                    let rel = maven_path(&lib.name, None);
                    let dest = paths.libraries().join(&rel);
                    if seen.insert(dest.clone()) {
                        let url = format!("{}/{}", base.trim_end_matches('/'), rel);
                        tasks.push(
                            DlTask::new(url, dest.clone())
                                .with_sha1(lib.sha1.clone())
                                .with_size(lib.size),
                        );
                        classpath.push(dest);
                    }
                } else {
                    // Written by the Forge/NeoForge installer: use it if present.
                    let dest = paths.libraries().join(maven_path(&lib.name, None));
                    if dest.exists() && seen.insert(dest.clone()) {
                        classpath.push(dest);
                    }
                }
                continue;
            }
        };
        if let Some(a) = &dl.artifact {
            if a.url.is_empty() {
                // Patched jars produced by the Forge installer have no URL.
                let rel = a.path.clone().unwrap_or_else(|| maven_path(&lib.name, None));
                let dest = paths.libraries().join(&rel);
                if dest.exists() && seen.insert(dest.clone()) {
                    classpath.push(dest);
                }
            } else {
                let rel = a.path.clone().unwrap_or_else(|| maven_path(&lib.name, None));
                let dest = paths.libraries().join(&rel);
                if seen.insert(dest.clone()) {
                    tasks.push(
                        DlTask::new(a.url.clone(), dest.clone())
                            .with_sha1(a.sha1.clone())
                            .with_size(a.size),
                    );
                    classpath.push(dest);
                }
            }
        }
        if let Some(key) = native_key(lib) {
            if let Some(a) = dl.classifiers.as_ref().and_then(|c| c.get(&key)) {
                let rel = a
                    .path
                    .clone()
                    .unwrap_or_else(|| maven_path(&lib.name, Some(&key)));
                let dest = paths.libraries().join(&rel);
                tasks.push(
                    DlTask::new(a.url.clone(), dest.clone())
                        .with_sha1(a.sha1.clone())
                        .with_size(a.size),
                );
                let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
                natives.push((dest, exclude));
            }
        }
    }
    classpath.push(client_jar);

    emit_progress(sink, task, "Downloading libraries", 0, tasks.len() as u64);
    download_many(tasks, 16, |d, t| emit_progress(sink, task, "Downloading libraries", d, t))
        .await
        .context("libraries")?;

    // 2. Assets
    let idx_ref = v
        .asset_index
        .as_ref()
        .ok_or_else(|| anyhow!("Version {} has no asset index", v.id))?;
    let idx_path = paths.assets().join("indexes").join(format!("{}.json", idx_ref.id));
    download_file(
        &DlTask::new(idx_ref.url.clone(), idx_path.clone())
            .with_sha1(idx_ref.sha1.clone())
            .with_size(idx_ref.size),
    )
    .await
    .context("asset index")?;
    let index: AssetIndex = serde_json::from_slice(&tokio::fs::read(&idx_path).await?)?;

    let mut hashes: HashSet<&str> = HashSet::new();
    let mut asset_tasks: Vec<DlTask> = Vec::new();
    for obj in index.objects.values() {
        if obj.hash.len() < 2 || !hashes.insert(obj.hash.as_str()) {
            continue;
        }
        let h2 = &obj.hash[..2];
        asset_tasks.push(
            DlTask::new(
                format!("https://resources.download.minecraft.net/{h2}/{}", obj.hash),
                paths.assets().join("objects").join(h2).join(&obj.hash),
            )
            .with_sha1(Some(obj.hash.clone()))
            .with_size(Some(obj.size)),
        );
    }
    let total_assets = asset_tasks.len() as u64;
    emit_progress(sink, task, "Downloading assets", 0, total_assets);
    download_many(asset_tasks, 32, |d, t| {
        if d % 25 == 0 || d == t {
            emit_progress(sink, task, "Downloading assets", d, t);
        }
    })
    .await
    .context("assets")?;

    // Legacy asset layouts
    let game_assets = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        paths.assets().join("virtual").join(&idx_ref.id)
    } else {
        paths.assets()
    };
    if index.map_to_resources || index.is_virtual {
        let objects_root = paths.assets().join("objects");
        let target = game_assets.clone();
        let entries: Vec<(String, String)> = index
            .objects
            .iter()
            .map(|(k, o)| (k.clone(), o.hash.clone()))
            .collect();
        tokio::task::spawn_blocking(move || -> Result<()> {
            for (name, hash) in entries {
                let src = objects_root.join(&hash[..2]).join(&hash);
                let dst = target.join(&name);
                if dst.exists() {
                    continue;
                }
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(&src, &dst)?;
            }
            Ok(())
        })
        .await??;
    }

    // 3. Logging configuration
    let mut logging_arg = None;
    if let Some(lc) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let dest = paths.assets().join("log_configs").join(&lc.file.id);
        download_file(
            &DlTask::new(lc.file.url.clone(), dest.clone())
                .with_sha1(lc.file.sha1.clone())
                .with_size(lc.file.size),
        )
        .await
        .context("logging config")?;
        logging_arg = Some(lc.argument.replace("${path}", &dest.to_string_lossy()));
    }

    Ok(InstallPlan {
        classpath,
        natives,
        asset_index_id: idx_ref.id.clone(),
        game_assets,
        logging_arg,
    })
}

/// Extracts legacy native libraries into a clean directory.
pub fn extract_natives(natives: &[(PathBuf, Vec<String>)], dest: &Path) -> Result<()> {
    // A game that is still running (or stuck) keeps its native libraries locked on Windows.
    // Deleting the folder then fails, so try, and fall back to reusing the files that are there.
    if dest.exists() {
        let _ = std::fs::remove_dir_all(dest);
    }
    std::fs::create_dir_all(dest)?;
    for (jar, exclude) in natives {
        let f = std::fs::File::open(jar).with_context(|| format!("opening {}", jar.display()))?;
        let mut zip = zip::ZipArchive::new(f)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if exclude.iter().any(|e| name.starts_with(e.as_str())) {
                continue;
            }
            let rel = match entry.enclosed_name() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            let out = dest.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            match std::fs::File::create(&out) {
                Ok(mut w) => {
                    std::io::copy(&mut entry, &mut w)?;
                }
                Err(e) => {
                    // locked by a running game: fine when the same file is already there
                    let same = std::fs::metadata(&out).map(|m| m.len() == entry.size()).unwrap_or(false);
                    if !same {
                        return Err(anyhow::anyhow!(
                            "Could not write {} ({e}). A previous Minecraft is probably still running - close it in Task Manager (javaw.exe) and try again.",
                            out.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3", None),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3", Some("natives-windows")),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-linux", None),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
    }
}
