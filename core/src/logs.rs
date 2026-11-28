//! Per-instance log files: the launcher's own log (kept across restarts) and
//! the game's `latest.log`, older logs and crash reports.

use crate::events::EventSink;
use crate::models::{GameState, LogLine, Progress};
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Forwards everything to `inner` and also writes log lines to `launcher.log`.
pub struct TeeSink {
    inner: Arc<dyn EventSink>,
    file: Mutex<Option<std::fs::File>>,
}

impl TeeSink {
    pub fn new(inner: Arc<dyn EventSink>, paths: &Paths, id: &str) -> Self {
        let file = std::fs::create_dir_all(paths.instance_dir(id))
            .ok()
            .and_then(|_| std::fs::File::create(launcher_log(paths, id)).ok());
        TeeSink { inner, file: Mutex::new(file) }
    }
}

impl EventSink for TeeSink {
    fn progress(&self, p: Progress) {
        self.inner.progress(p)
    }
    fn log(&self, l: LogLine) {
        if let Ok(mut g) = self.file.lock() {
            if let Some(f) = g.as_mut() {
                let _ = writeln!(f, "[{}] {}", l.level.to_uppercase(), l.text);
            }
        }
        self.inner.log(l)
    }
    fn state(&self, s: GameState) {
        self.inner.state(s)
    }
}

fn launcher_log(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_dir(id).join("launcher.log")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    pub name: String,
    /// "launcher" | "game" | "crash"
    pub kind: String,
    pub size: u64,
    pub modified: u64,
}

fn secs(m: &std::fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn list_logs(paths: &Paths, id: &str) -> Vec<LogFile> {
    let mut out = vec![];
    if let Ok(m) = std::fs::metadata(launcher_log(paths, id)) {
        out.push(LogFile { name: "launcher.log".into(), kind: "launcher".into(), size: m.len(), modified: secs(&m) });
    }
    let game = paths.game_dir(id);
    for (sub, kind) in [("logs", "game"), ("crash-reports", "crash")] {
        let mut part = vec![];
        if let Ok(rd) = std::fs::read_dir(game.join(sub)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let ok = name.ends_with(".log") || name.ends_with(".log.gz") || name.ends_with(".txt");
                if let (true, Ok(m)) = (ok, e.metadata()) {
                    if m.is_file() {
                        part.push(LogFile { name, kind: kind.into(), size: m.len(), modified: secs(&m) });
                    }
                }
            }
        }
        // latest.log first, then newest
        part.sort_by(|a, b| (b.name == "latest.log").cmp(&(a.name == "latest.log")).then(b.modified.cmp(&a.modified)));
        out.extend(part);
    }
    out
}

const MAX_BYTES: usize = 400 * 1024;

/// Text of a log file (the last 400 KB if it is larger). `.gz` logs are unpacked.
pub fn read_log(paths: &Paths, id: &str, kind: &str, name: &str) -> Result<String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(anyhow!("Invalid log name"));
    }
    let path = match kind {
        "launcher" if name == "launcher.log" => launcher_log(paths, id),
        "game" => paths.game_dir(id).join("logs").join(name),
        "crash" => paths.game_dir(id).join("crash-reports").join(name),
        _ => return Err(anyhow!("Unknown log")),
    };
    let mut raw = Vec::new();
    if name.ends_with(".gz") {
        let f = std::fs::File::open(&path)?;
        // cap what a corrupt or huge archive can expand to
        flate2::read::GzDecoder::new(f).take(20 * 1024 * 1024).read_to_end(&mut raw)?;
    } else {
        std::fs::File::open(&path)?.take(20 * 1024 * 1024).read_to_end(&mut raw)?;
    }
    let cut = raw.len().saturating_sub(MAX_BYTES);
    let text = String::from_utf8_lossy(&raw[cut..]).to_string();
    Ok(if cut > 0 { format!("… (showing the last {} KB)\n{text}", MAX_BYTES / 1024) } else { text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_reads_logs_safely() {
        let dir = std::env::temp_dir().join(format!("sc-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::new(dir.clone());
        let g = paths.game_dir("a");
        std::fs::create_dir_all(g.join("logs")).unwrap();
        std::fs::create_dir_all(g.join("crash-reports")).unwrap();
        std::fs::write(g.join("logs/latest.log"), "hello game").unwrap();
        std::fs::write(g.join("logs/2026-01-01-1.log"), "old").unwrap();
        std::fs::write(g.join("crash-reports/crash-1.txt"), "boom").unwrap();
        std::fs::write(paths.instance_dir("a").join("launcher.log"), "[INFO] started").unwrap();
        let l = list_logs(&paths, "a");
        assert_eq!(l[0].kind, "launcher");
        assert_eq!(l[1].name, "latest.log");
        assert!(l.iter().any(|f| f.kind == "crash"));
        assert_eq!(read_log(&paths, "a", "game", "latest.log").unwrap(), "hello game");
        assert!(read_log(&paths, "a", "game", "../../settings.json").is_err());
        assert!(read_log(&paths, "a", "launcher", "x.log").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
