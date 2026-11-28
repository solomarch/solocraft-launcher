use crate::download::{download_file_with, get_json, ByteCb, DlTask};
use crate::events::{emit_progress, EventSink};
use crate::mcjson::McVersion;
use crate::models::{Instance, JavaInstall};
use crate::paths::Paths;
use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// Java major version the game needs. Mojang's version JSON states it for
/// every version since 1.17; older ones fall back to a table.
pub fn required_major(v: &McVersion) -> u32 {
    if let Some(j) = &v.java_version {
        return j.major_version;
    }
    fallback_major(&v.id)
}

pub fn fallback_major(id: &str) -> u32 {
    let nums: Vec<u32> = id
        .split(|c| c == '.' || c == '-' || c == ' ')
        .map_while(|p| p.parse().ok())
        .collect();
    match nums.as_slice() {
        [maj, ..] if *maj >= 26 => 25,
        [1, minor, rest @ ..] => {
            let patch = rest.first().copied().unwrap_or(0);
            if *minor >= 21 || (*minor == 20 && patch >= 5) {
                21
            } else if *minor >= 17 {
                17
            } else {
                8
            }
        }
        _ => 8,
    }
}

pub fn parse_major(version: &str) -> u32 {
    let v = version.trim();
    let digits = |s: &str| -> u32 {
        s.split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0)
    };
    if let Some(rest) = v.strip_prefix("1.") {
        return digits(rest);
    }
    digits(v)
}

static RE_VERSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*java\.version = (.+?)\s*$").unwrap());
static RE_VENDOR: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*java\.vendor = (.+?)\s*$").unwrap());
static RE_ARCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*os\.arch = (.+?)\s*$").unwrap());

/// Runs the executable to read its real version, vendor and architecture.
pub async fn probe(path: &Path, managed: bool) -> Option<JavaInstall> {
    let mut cmd = tokio::process::Command::new(path);
    cmd.args(["-XshowSettings:properties", "-version"])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = tokio::time::timeout(Duration::from_secs(15), cmd.output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&out.stderr).to_string()
        + &String::from_utf8_lossy(&out.stdout);
    let version = RE_VERSION.captures(&text)?.get(1)?.as_str().to_string();
    let vendor = RE_VENDOR
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let arch = RE_ARCH
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let major = parse_major(&version);
    if major == 0 {
        return None;
    }
    Some(JavaInstall {
        path: path.to_string_lossy().to_string(),
        version,
        major,
        vendor,
        arch,
        managed,
    })
}

fn find_java_exes(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for e in walkdir::WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(true)
        .into_iter()
        .flatten()
    {
        if e.file_type().is_file()
            && e.file_name() == java_exe_name()
            && e.path().parent().and_then(|p| p.file_name()).map(|n| n == "bin").unwrap_or(false)
        {
            out.push(e.path().to_path_buf());
        }
    }
    out
}

fn candidate_paths(paths: &Paths) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();

    // Launcher-managed Java
    found.extend(find_java_exes(&paths.java_dir(), 5));

    #[cfg(windows)]
    {
        let vendor_dirs = [
            "Java",
            "Eclipse Adoptium",
            "Eclipse Foundation",
            "Amazon Corretto",
            "Microsoft",
            "Zulu",
            "BellSoft",
            "Semeru",
            "AdoptOpenJDK",
            "OpenJDK",
            "Temurin",
        ];
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(pf) = std::env::var(var) {
                for v in vendor_dirs {
                    found.extend(find_java_exes(&Path::new(&pf).join(v), 4));
                }
            }
        }
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            found.extend(find_java_exes(&Path::new(&la).join("Programs").join("Eclipse Adoptium"), 4));
            found.extend(find_java_exes(
                &Path::new(&la)
                    .join("Packages")
                    .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                    .join("LocalCache")
                    .join("Local")
                    .join("runtime"),
                6,
            ));
        }
        if let Ok(ap) = std::env::var("APPDATA") {
            found.extend(find_java_exes(&Path::new(&ap).join(".minecraft").join("runtime"), 6));
        }
        if let Ok(up) = std::env::var("USERPROFILE") {
            found.extend(find_java_exes(&Path::new(&up).join(".jdks"), 4));
        }
    }

    #[cfg(not(windows))]
    {
        for root in [
            "/usr/lib/jvm",
            "/usr/lib64/jvm",
            "/usr/java",
            "/opt/java",
            "/opt/jdk",
            "/usr/local/lib/jvm",
        ] {
            found.extend(find_java_exes(Path::new(root), 4));
        }
        if let Some(home) = dirs::home_dir() {
            found.extend(find_java_exes(&home.join(".sdkman").join("candidates").join("java"), 4));
            found.extend(find_java_exes(&home.join(".jdks"), 4));
            found.extend(find_java_exes(&home.join(".minecraft").join("runtime"), 6));
        }
    }

    if let Ok(jh) = std::env::var("JAVA_HOME") {
        let p = Path::new(&jh).join("bin").join(java_exe_name());
        if p.is_file() {
            found.push(p);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(java_exe_name());
            if p.is_file() {
                found.push(p);
            }
        }
    }

    let mut seen = HashSet::new();
    found
        .into_iter()
        .filter(|p| {
            let key = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            seen.insert(key)
        })
        .collect()
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CacheEntry {
    mtime: u64,
    len: u64,
    install: JavaInstall,
}

type ProbeCache = std::collections::HashMap<String, CacheEntry>;

fn cache_file(paths: &Paths) -> PathBuf {
    paths.meta().join("java_cache.json")
}

fn load_cache(paths: &Paths) -> ProbeCache {
    std::fs::read(cache_file(paths))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn stamp(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((t, m.len()))
}

/// Finds every Java on the system. Each executable is only started once:
/// results are cached by path, modification time and size, so later launches
/// skip the (slow) probing entirely.
pub async fn detect_installs(paths: &Paths) -> Vec<JavaInstall> {
    let p = paths.clone();
    let cands = tokio::task::spawn_blocking(move || candidate_paths(&p))
        .await
        .unwrap_or_default();
    let managed_root = paths.java_dir();
    let cache = load_cache(paths);

    let futs = cands.into_iter().map(|c| {
        let managed = c.starts_with(&managed_root);
        let key = c.to_string_lossy().to_string();
        let st = stamp(&c);
        let hit = match (&st, cache.get(&key)) {
            (Some((t, l)), Some(e)) if e.mtime == *t && e.len == *l => Some(e.install.clone()),
            _ => None,
        };
        async move {
            if let Some(mut h) = hit {
                h.managed = managed;
                return Some((key, st, h));
            }
            probe(&c, managed).await.map(|j| (key, st, j))
        }
    });
    let results: Vec<(String, Option<(u64, u64)>, JavaInstall)> = futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .collect();

    let mut fresh: ProbeCache = ProbeCache::new();
    for (key, st, inst) in &results {
        if let Some((t, l)) = st {
            fresh.insert(
                key.clone(),
                CacheEntry { mtime: *t, len: *l, install: inst.clone() },
            );
        }
    }
    if let Ok(bytes) = serde_json::to_vec(&fresh) {
        let _ = std::fs::write(cache_file(paths), bytes);
    }

    let mut list: Vec<JavaInstall> = results.into_iter().map(|(_, _, j)| j).collect();
    list.sort_by(|a, b| b.major.cmp(&a.major).then(a.path.cmp(&b.path)));
    list
}

/// Exact major match first. Newer Javas are accepted for 16+ requirements
/// only when the exact one is absent. Java 8 games never accept newer Java.
pub fn choose(installs: &[JavaInstall], required: u32) -> Option<JavaInstall> {
    let is64 = |j: &&JavaInstall| j.arch.contains("64");
    if let Some(j) = installs.iter().filter(is64).find(|j| j.major == required) {
        return Some(j.clone());
    }
    if required >= 16 {
        return installs
            .iter()
            .filter(is64)
            .filter(|j| j.major > required)
            .min_by_key(|j| j.major)
            .cloned();
    }
    None
}

pub async fn resolve_java(
    paths: &Paths,
    inst: &Instance,
    major: u32,
    sink: Arc<dyn EventSink>,
    task: &str,
) -> Result<JavaInstall> {
    if let Some(p) = inst.java_path.as_ref().filter(|p| !p.trim().is_empty()) {
        return probe(Path::new(p), false).await.ok_or_else(|| {
            anyhow!("The Java at {p} did not run. Pick another Java or set it back to Auto.")
        });
    }
    let installs = detect_installs(paths).await;
    if let Some(j) = choose(&installs, major) {
        return Ok(j);
    }
    install_temurin(paths, major, sink, task).await
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(f)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let rel = match entry.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut w)?;
    }
    Ok(())
}

fn extract_targz(archive: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(dest)?;
    Ok(())
}

/// Downloads a portable Eclipse Temurin JRE (SHA-256 verified) into the
/// launcher's own folder. Nothing is installed system-wide.
pub async fn install_temurin(
    paths: &Paths,
    major: u32,
    sink: Arc<dyn EventSink>,
    task: &str,
) -> Result<JavaInstall> {
    let dest = paths.java_dir().join(format!("temurin-{major}"));
    if let Some(exe) = find_java_exes(&dest, 5).into_iter().next() {
        if let Some(j) = probe(&exe, true).await {
            return Ok(j);
        }
    }

    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if std::env::consts::ARCH == "aarch64" { "aarch64" } else { "x64" };
    let stage = format!("Downloading Java {major}");
    emit_progress(&*sink, task, &stage, 0, 0);

    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse"
    );
    let v: serde_json::Value = get_json(&api)
        .await
        .with_context(|| format!("looking up Java {major}"))?;
    let pkg = v
        .get(0)
        .and_then(|x| x.pointer("/binary/package"))
        .ok_or_else(|| anyhow!("Adoptium has no Java {major} build for this system"))?;
    let link = pkg
        .get("link")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("Adoptium response had no download link"))?;
    let name = pkg
        .get("name")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("Adoptium response had no file name"))?;

    let archive = paths.java_dir().join("downloads").join(name);
    let mut t = DlTask::new(link, archive.clone());
    t.sha256 = pkg.get("checksum").and_then(|x| x.as_str()).map(|s| s.to_string());
    t.size = pkg.get("size").and_then(|x| x.as_u64());

    let last_pct = Arc::new(AtomicU64::new(u64::MAX));
    let cb_sink = sink.clone();
    let cb_task = task.to_string();
    let cb_stage = stage.clone();
    let cb: ByteCb = Arc::new(move |done, total| {
        if total == 0 {
            return;
        }
        let pct = done * 100 / total;
        if last_pct.swap(pct, Ordering::SeqCst) != pct {
            emit_progress(&*cb_sink, &cb_task, &cb_stage, done, total);
        }
    });
    download_file_with(&t, Some(cb)).await?;

    emit_progress(&*sink, task, &format!("Unpacking Java {major}"), 0, 0);
    let a = archive.clone();
    let d = dest.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        if d.exists() {
            std::fs::remove_dir_all(&d)?;
        }
        std::fs::create_dir_all(&d)?;
        let name = a.to_string_lossy().to_lowercase();
        if name.ends_with(".zip") {
            extract_zip(&a, &d)
        } else {
            extract_targz(&a, &d)
        }
    })
    .await??;
    let _ = tokio::fs::remove_file(&archive).await;

    let exe = find_java_exes(&dest, 5)
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Java was unpacked but no java executable was found"))?;
    probe(&exe, true)
        .await
        .ok_or_else(|| anyhow!("The downloaded Java did not run"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_majors() {
        assert_eq!(parse_major("1.8.0_392"), 8);
        assert_eq!(parse_major("21.0.2"), 21);
        assert_eq!(parse_major("25"), 25);
        assert_eq!(parse_major("17.0.9-ea"), 17);
    }

    #[test]
    fn fallback_table() {
        assert_eq!(fallback_major("1.8.9"), 8);
        assert_eq!(fallback_major("1.16.5"), 8);
        assert_eq!(fallback_major("1.17.1"), 17);
        assert_eq!(fallback_major("1.20.4"), 17);
        assert_eq!(fallback_major("1.20.5"), 21);
        assert_eq!(fallback_major("1.21.11"), 21);
        assert_eq!(fallback_major("26.1"), 25);
        assert_eq!(fallback_major("b1.7.3"), 8);
    }

    fn j(major: u32, arch: &str) -> JavaInstall {
        JavaInstall {
            path: format!("/j/{major}"),
            version: major.to_string(),
            major,
            vendor: String::new(),
            arch: arch.into(),
            managed: false,
        }
    }

    #[test]
    fn chooses_exact_then_newer() {
        let list = vec![j(21, "amd64"), j(17, "amd64"), j(8, "amd64")];
        assert_eq!(choose(&list, 17).unwrap().major, 17);
        assert_eq!(choose(&list, 8).unwrap().major, 8);
        let only21 = vec![j(21, "amd64")];
        assert_eq!(choose(&only21, 17).unwrap().major, 21);
        assert!(choose(&only21, 8).is_none());
        let only32 = vec![j(17, "x86")];
        assert!(choose(&only32, 17).is_none());
    }
}
