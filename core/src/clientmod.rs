//! SoloCraft Client: the in-game menu/HUD mod built from `client-mod/`.
//! The launcher finds the built jar and drops it into an instance's mods folder.

use crate::events::EventSink;
use crate::models::Instance;
use crate::modrinth;
use crate::paths::Paths;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Minecraft 1.20.x versions that have their own jar.
const OLD_MC: [&str; 7] = ["1.20", "1.20.1", "1.20.2", "1.20.3", "1.20.4", "1.20.5", "1.20.6"];
/// Name tag of the one jar that works on every 1.21.x version.
const UNIVERSAL: &str = "1.21.x";

fn is_121(mc: &str) -> bool {
    mc == "1.21" || mc.starts_with("1.21.")
}

/// True when the mod can run on this Minecraft version (Fabric only).
pub fn supports(mc: &str) -> bool {
    OLD_MC.contains(&mc) || is_121(mc)
}
const FILE: &str = "solocraft-client.jar";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub jar_found: bool,
    pub supported: bool,
    pub installed: bool,
    /// Why it cannot be installed (empty when it can).
    pub reason: String,
}

/// Version number inside `solocraft-client-<tag>-<version>.jar`, as comparable numbers.
fn jar_version(name: &str, tag: &str) -> Vec<u64> {
    let rest = name.trim_start_matches(&format!("solocraft-client-{tag}-")[..]).trim_end_matches(".jar");
    rest.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect()
}

/// All jars in `dir` built for this Minecraft version (file name `solocraft-client-<mc>-<version>.jar`),
/// with their version and modified time.
fn jars_in(dir: &Path, mc: &str) -> Vec<(Vec<u64>, std::time::SystemTime, PathBuf)> {
    let mut tags = vec![mc.to_string()];
    if is_121(mc) {
        tags.push(UNIVERSAL.to_string());
    }
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !n.ends_with(".jar") || n.contains("sources") || n.contains("-dev") {
            continue;
        }
        for tag in &tags {
            if n.starts_with(&format!("solocraft-client-{tag}-")) {
                let t = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                out.push((jar_version(&n, tag), t, e.path()));
            }
        }
    }
    out
}

/// Looks for the jar of this Minecraft version in SOLOCRAFT_CLIENT_JAR, <data>/client/ (where the
/// install script puts downloads) and `client-mod/dist` or `build/libs` next to / above the app or the
/// working directory. When several are found the HIGHEST VERSION wins (then the newest file), so a
/// freshly built jar is never hidden by an older downloaded one.
pub fn find_jar(paths: &Paths, mc: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SOLOCRAFT_CLIENT_JAR") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut cands = vec![];
    for base in [paths.root.clone(), crate::storage::default_root()] {
        cands.extend(jars_in(&base.join("client"), mc));
    }
    let mut starts: Vec<PathBuf> = vec![];
    if let Ok(e) = std::env::current_exe() {
        starts.extend(e.parent().map(|p| p.to_path_buf()));
    }
    if let Ok(c) = std::env::current_dir() {
        starts.push(c);
    }
    for s in starts {
        let mut cur: Option<&Path> = Some(&s);
        for _ in 0..8 {
            let Some(d) = cur else { break };
            for sub in [
                d.join("client-mod").join("dist"),
                d.join("client-mod").join("build").join("libs"),
                d.join("client"),
            ] {
                cands.extend(jars_in(&sub, mc));
            }
            cur = d.parent();
        }
    }
    cands.into_iter().max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1))).map(|c| c.2)
}

fn target(paths: &Paths, id: &str) -> PathBuf {
    paths.game_dir(id).join("mods").join(FILE)
}

/// Before the game starts: when SoloCraft Client is installed and a different build of it is found
/// (for example after you rebuilt the mod), copy the newer one in. Returns true when it was updated.
pub fn refresh(paths: &Paths, inst: &Instance) -> bool {
    let dest = target(paths, &inst.id);
    if !dest.exists() {
        return false; // not installed (or switched off): leave it alone
    }
    let Some(jar) = find_jar(paths, &inst.mc_version) else { return false };
    let same = match (std::fs::read(&jar), std::fs::read(&dest)) {
        (Ok(a), Ok(b)) => a == b,
        _ => true,
    };
    if same {
        return false;
    }
    std::fs::copy(&jar, &dest).is_ok()
}

pub fn status(paths: &Paths, inst: &Instance) -> Status {
    let jar_found = find_jar(paths, &inst.mc_version).is_some();
    let supported = inst.loader == "fabric" && supports(&inst.mc_version);
    let installed = target(paths, &inst.id).exists() || target(paths, &inst.id).with_extension("jar.disabled").exists();
    let reason = if !supported {
        format!("Needs a Fabric instance on Minecraft 1.20 - 1.21.x (this one is {} {}).", inst.loader, inst.mc_version)
    } else if !jar_found && !installed {
        format!("No SoloCraft Client file for Minecraft {v} yet. Run build-all.ps1 in the client-mod folder.", v = inst.mc_version)
    } else {
        String::new()
    };
    Status { jar_found, supported, installed, reason }
}

pub async fn install(paths: &Paths, inst: &Instance, sink: &dyn EventSink) -> Result<()> {
    let st = status(paths, inst);
    if !st.supported || (!st.jar_found && !st.installed) {
        return Err(anyhow!("{}", st.reason));
    }
    let jar = find_jar(paths, &inst.mc_version).ok_or_else(|| anyhow!("{}", st.reason))?;
    let dest = target(paths, &inst.id);
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::copy(&jar, &dest).with_context(|| format!("copying {}", jar.display()))?;
    // The mod needs Fabric API; the installer skips it when already present.
    modrinth::install_mod(paths, inst, "fabric-api", sink)
        .await
        .context("installing Fabric API (needed by SoloCraft Client)")?;
    Ok(())
}

pub fn remove(paths: &Paths, id: &str) -> Result<()> {
    let t = target(paths, id);
    for f in [t.clone(), t.with_extension("jar.disabled")] {
        if f.exists() {
            std::fs::remove_file(f)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick_jar(dir: &Path, mc: &str) -> Option<PathBuf> {
        jars_in(dir, mc).into_iter().max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1))).map(|c| c.2)
    }

    #[test]
    fn higher_version_beats_older_download() {
        let d = std::env::temp_dir().join(format!("sc-cm-ver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // the older one is written last, so it is the newest FILE: the version must still win
        std::fs::write(d.join("solocraft-client-1.21.x-1.2.0.jar"), "x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(d.join("solocraft-client-1.21.x-1.1.0.jar"), "x").unwrap();
        assert!(pick_jar(&d, "1.21.11").unwrap().ends_with("solocraft-client-1.21.x-1.2.0.jar"));
        assert!(jar_version("solocraft-client-1.21.x-1.10.0.jar", "1.21.x") > jar_version("solocraft-client-1.21.x-1.9.0.jar", "1.21.x"));
    }

    #[test]
    fn picks_jar_for_the_right_version() {
        let d = std::env::temp_dir().join(format!("sc-cm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["solocraft-client-1.21.4-1.0.0-sources.jar", "solocraft-client-1.21.4-1.0.0.jar", "solocraft-client-1.20.1-1.0.0.jar"] {
            std::fs::write(d.join(n), "x").unwrap();
        }
        assert!(pick_jar(&d, "1.21.4").unwrap().ends_with("solocraft-client-1.21.4-1.0.0.jar"));
        assert!(pick_jar(&d, "1.20.1").unwrap().ends_with("solocraft-client-1.20.1-1.0.0.jar"));
        assert!(pick_jar(&d, "1.20.4").is_none());
        // the universal jar serves every 1.21.x, an exact jar wins when present
        std::fs::write(d.join("solocraft-client-1.21.x-1.0.0.jar"), "x").unwrap();
        assert!(pick_jar(&d, "1.21.11").unwrap().ends_with("solocraft-client-1.21.x-1.0.0.jar"));
        assert!(pick_jar(&d, "1.21.4").is_some());
        assert!(pick_jar(&d, "1.20.6").is_none());
        assert!(supports("1.21.10") && supports("1.20.2") && !supports("1.19.4") && !supports("26.1"));
    }

    #[test]
    fn only_fabric_on_supported_versions() {
        let paths = Paths::new(std::env::temp_dir().join("sc-cm-none"));
        let mut i = Instance { loader: "fabric".into(), mc_version: "1.21.4".into(), ..Default::default() };
        assert!(status(&paths, &i).supported);
        i.mc_version = "1.20.1".into();
        assert!(status(&paths, &i).supported);
        i.mc_version = "1.19.2".into();
        assert!(!status(&paths, &i).supported);
        i.loader = "forge".into();
        i.mc_version = "1.21.4".into();
        assert!(!status(&paths, &i).supported);
    }
}
