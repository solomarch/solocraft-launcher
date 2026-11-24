//! Where SoloCraft keeps its data. The location can be any drive (an external
//! or secondary disk, for example). A tiny pointer file in the normal
//! per-user config folder remembers the choice; everything big lives at the
//! chosen location.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Location {
    pub root: PathBuf,
    /// set when the chosen folder could not be used and the default was used instead
    pub warning: Option<String>,
    /// true when no data folder existed yet (the very first start)
    pub first_run: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pointer {
    data_dir: String,
}

fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("SOLOCRAFT_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("SoloCraft")
}

fn pointer_file() -> PathBuf {
    config_dir().join("location.json")
}

pub fn default_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("SoloCraft")
}

fn read_pointer() -> Option<PathBuf> {
    let b = std::fs::read(pointer_file()).ok()?;
    let p: Pointer = serde_json::from_slice(&b).ok()?;
    if p.data_dir.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(p.data_dir))
    }
}

pub fn write_pointer(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(config_dir())?;
    let p = Pointer { data_dir: dir.to_string_lossy().to_string() };
    std::fs::write(pointer_file(), serde_json::to_vec_pretty(&p)?)?;
    Ok(())
}

/// Decides which folder to use at startup. If the chosen drive is missing
/// (an unplugged external disk), the default folder is used and a warning is
/// returned; the choice itself is kept so it works again once the drive is back.
pub fn resolve() -> Location {
    let default = default_root();
    if let Some(chosen) = read_pointer() {
        match std::fs::create_dir_all(&chosen) {
            Ok(()) => return Location { root: chosen, warning: None, first_run: false },
            Err(e) => {
                return Location {
                    root: default,
                    warning: Some(format!(
                        "Your data folder {} is not available ({e}). Is the drive connected? SoloCraft is using the default folder until it is.",
                        chosen.display()
                    )),
                    first_run: false,
                }
            }
        }
    }
    let first_run = !default.exists();
    Location { root: default, warning: None, first_run }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub path: String,
    pub free: u64,
    pub total: u64,
    pub system: bool,
}

pub fn list_drives() -> Vec<DriveInfo> {
    let mut cands: Vec<PathBuf> = vec![];
    if cfg!(windows) {
        for l in b'A'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", l as char));
            if p.exists() {
                cands.push(p);
            }
        }
    } else {
        cands.push(PathBuf::from("/"));
        if let Some(h) = dirs::home_dir() {
            cands.push(h);
        }
        for base in ["/mnt", "/media", "/run/media"] {
            if let Ok(rd) = std::fs::read_dir(base) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        // /run/media/<user>/<disk> and /media/<user>/<disk>
                        let nested: Vec<PathBuf> = std::fs::read_dir(&p)
                            .map(|r| r.flatten().map(|x| x.path()).filter(|x| x.is_dir()).collect())
                            .unwrap_or_default();
                        if base != "/mnt" && !nested.is_empty() {
                            cands.extend(nested);
                        } else {
                            cands.push(p);
                        }
                    }
                }
            }
        }
    }
    let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let mut out: Vec<DriveInfo> = vec![];
    for p in cands {
        let total = fs4::total_space(&p).unwrap_or(0);
        if total == 0 {
            continue;
        }
        let s = p.to_string_lossy().to_string();
        let system = if cfg!(windows) { s.to_uppercase().starts_with(&sys_drive) } else { s == "/" };
        if out.iter().any(|d| d.total == total && d.free == fs4::available_space(&p).unwrap_or(0) && !cfg!(windows)) {
            continue; // same filesystem listed twice on Linux (e.g. / and home)
        }
        out.push(DriveInfo { path: s, free: fs4::available_space(&p).unwrap_or(0), total, system });
    }
    out
}

pub fn free_space(path: &Path) -> u64 {
    // walk up to an existing folder so a not-yet-created target can be measured
    let mut p = path;
    loop {
        if p.exists() {
            return fs4::available_space(p).unwrap_or(0);
        }
        match p.parent() {
            Some(x) => p = x,
            None => return 0,
        }
    }
}

/// The folder SoloCraft would really use for a picked folder: the folder itself
/// when it is empty or already called SoloCraft, otherwise a SoloCraft folder inside it.
pub fn normalize_target(picked: &Path) -> PathBuf {
    let is_named = picked.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("SoloCraft")).unwrap_or(false);
    let empty = picked.is_dir() && std::fs::read_dir(picked).map(|mut r| r.next().is_none()).unwrap_or(false);
    if is_named || empty {
        picked.to_path_buf()
    } else {
        picked.join("SoloCraft")
    }
}

pub fn validate_target(old: &Path, target: &Path) -> Result<()> {
    if !target.is_absolute() {
        return Err(anyhow!("Choose a full folder path such as D:\\Games\\SoloCraft"));
    }
    if target == old {
        return Err(anyhow!("That is already the current location"));
    }
    if target.starts_with(old) || old.starts_with(target) {
        return Err(anyhow!("The new folder cannot be inside the current one (or the other way round)"));
    }
    if target.exists() && std::fs::read_dir(target).map(|mut r| r.next().is_some()).unwrap_or(false) {
        return Err(anyhow!("{} is not empty. Pick an empty folder or a new one.", target.display()));
    }
    std::fs::create_dir_all(target).with_context(|| format!("cannot create {}", target.display()))?;
    let probe = target.join(".solocraft-write-test");
    std::fs::write(&probe, b"ok").with_context(|| format!("{} is not writable", target.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

pub fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageItem {
    pub name: String,
    pub bytes: u64,
}

/// Size of each top-level folder in the data directory, biggest first.
pub fn usage(root: &Path) -> Vec<UsageItem> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            let bytes = if p.is_dir() { dir_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) };
            out.push(UsageItem { name: e.file_name().to_string_lossy().to_string(), bytes });
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

/// Copies the whole data folder to `target`, checks the result, switches the
/// pointer and only then removes the old folder. On any failure the old data is
/// left untouched and the partial copy is removed.
pub fn move_data(old: &Path, target: &Path, progress: &dyn Fn(u64, u64)) -> Result<()> {
    validate_target(old, target)?;
    let total = dir_size(old);
    if free_space(target) < total + 64 * 1024 * 1024 {
        return Err(anyhow!(
            "Not enough free space at the new location (needs about {} MB)",
            total / 1_048_576 + 64
        ));
    }
    let result = (|| -> Result<(u64, u64)> {
        let mut done = 0u64;
        let mut files = 0u64;
        for e in WalkDir::new(old).into_iter().flatten() {
            let rel = e.path().strip_prefix(old)?;
            let dest = target.join(rel);
            if e.file_type().is_dir() {
                std::fs::create_dir_all(&dest)?;
            } else if e.file_type().is_file() {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                done += std::fs::copy(e.path(), &dest).with_context(|| format!("copying {}", e.path().display()))?;
                files += 1;
                progress(done, total);
            }
        }
        Ok((done, files))
    })();
    let (copied, files) = match result {
        Ok(x) => x,
        Err(e) => {
            let _ = std::fs::remove_dir_all(target);
            return Err(e);
        }
    };
    let new_files = WalkDir::new(target).into_iter().flatten().filter(|e| e.file_type().is_file()).count() as u64;
    if new_files != files || dir_size(target) != copied {
        let _ = std::fs::remove_dir_all(target);
        return Err(anyhow!("The copy could not be verified, so nothing was changed"));
    }
    write_pointer(target)?;
    let _ = std::fs::remove_dir_all(old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // these tests change a process-wide environment variable
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sc-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn moves_data_and_switches_pointer() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let base = tmp("move");
        std::env::set_var("SOLOCRAFT_CONFIG_DIR", base.join("cfg"));
        let old = base.join("old");
        std::fs::create_dir_all(old.join("instances/a/.minecraft")).unwrap();
        std::fs::write(old.join("instances/a/.minecraft/options.txt"), b"fps=1").unwrap();
        std::fs::write(old.join("settings.json"), b"{}").unwrap();
        let target = base.join("drive2").join("SoloCraft");
        let seen = std::sync::atomic::AtomicU64::new(0);
        move_data(&old, &target, &|d, _| seen.store(d, std::sync::atomic::Ordering::SeqCst)).unwrap();
        assert_eq!(std::fs::read(target.join("instances/a/.minecraft/options.txt")).unwrap(), b"fps=1");
        assert!(!old.exists(), "old folder removed after a verified copy");
        assert!(seen.load(std::sync::atomic::Ordering::SeqCst) > 0);
        let loc = resolve();
        assert_eq!(loc.root, target);
        assert!(loc.warning.is_none());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rejects_bad_targets() {
        let base = tmp("bad");
        let old = base.join("old");
        std::fs::create_dir_all(&old).unwrap();
        assert!(validate_target(&old, &old).is_err());
        assert!(validate_target(&old, &old.join("inside")).is_err());
        assert!(validate_target(&old, Path::new("relative/path")).is_err());
        let full = base.join("full");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::write(full.join("x.txt"), b"1").unwrap();
        assert!(validate_target(&old, &full).is_err());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn missing_drive_falls_back_with_warning() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let base = tmp("missing");
        std::env::set_var("SOLOCRAFT_CONFIG_DIR", base.join("cfg2"));
        // a path whose parent is a file cannot be created
        let blocker = base.join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        write_pointer(&blocker.join("SoloCraft")).unwrap();
        let loc = resolve();
        assert!(loc.warning.is_some());
        assert_eq!(loc.root, default_root());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn target_naming() {
        let base = tmp("name");
        assert_eq!(normalize_target(&base.join("Games")), base.join("Games").join("SoloCraft"));
        assert_eq!(normalize_target(&base.join("SoloCraft")), base.join("SoloCraft"));
        assert_eq!(normalize_target(&base), base);
        std::fs::remove_dir_all(&base).ok();
    }
}
