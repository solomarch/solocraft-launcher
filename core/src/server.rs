//! Per-instance dedicated Java server hosting (vanilla, Fabric, and Quilt).

use crate::download::{download_file, DlTask};
use crate::events::EventSink;
use crate::models::LogLine;
use crate::paths::Paths;
use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub online_mode: bool,
    pub port: u16,
    pub max_players: u8,
    pub eula_accepted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetInfo {
    pub lan_ip: Option<String>,
    pub public_ip: Option<String>,
}

/// Reports the LAN address that routes outbound IPv4 traffic and the public address seen by an
/// IP echo service. This does not change router or firewall settings.
pub async fn internet_info() -> Result<InternetInfo> {
    let lan_ip = UdpSocket::bind("0.0.0.0:0").ok().and_then(|socket| {
        socket.connect("1.1.1.1:80").ok()?;
        match socket.local_addr().ok()?.ip() {
            IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip.to_string()),
            _ => None,
        }
    });

    let response = crate::download::client()
        .get("https://api.ipify.org")
        .timeout(Duration::from_secs(6))
        .send().await;
    let public_ip = match response {
        Ok(r) if r.status().is_success() => r.text().await.ok()
            .map(|s| s.trim().to_string())
            .filter(|s| s.parse::<IpAddr>().is_ok()),
        _ => None,
    };
    Ok(InternetInfo { lan_ip, public_ip })
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { online_mode: true, port: 25565, max_players: 20, eula_accepted: false }
    }
}

struct RunningServer { child: Child, stdin: ChildStdin }

#[derive(Default)]
pub struct ServerHost {
    children: Mutex<HashMap<String, RunningServer>>,
    starting: Mutex<HashSet<String>>,
}

struct Starting<'a> { host: &'a ServerHost, id: String }
impl Drop for Starting<'_> {
    fn drop(&mut self) { self.host.starting.lock().unwrap().remove(&self.id); }
}

impl ServerHost {
    pub fn is_running(&self, id: &str) -> bool {
        let mut children = self.children.lock().unwrap();
        let live = children.get_mut(id).is_some_and(|s| matches!(s.child.try_wait(), Ok(None)));
        if !live { children.remove(id); }
        live
    }

    pub fn is_busy(&self, id: &str) -> bool {
        self.starting.lock().unwrap().contains(id) || self.is_running(id)
    }

    fn reserve_start(&self, id: &str) -> Result<Starting<'_>> {
        let mut starting = self.starting.lock().unwrap();
        if starting.contains(id) || self.is_running(id) {
            return Err(anyhow!("This instance's server is already starting or running"));
        }
        starting.insert(id.to_string());
        Ok(Starting { host: self, id: id.to_string() })
    }

    pub fn stop(&self, id: &str) -> Result<bool> {
        let Some(mut running) = self.children.lock().unwrap().remove(id) else { return Ok(false) };
        let _ = running.stdin.write_all(b"stop\n");
        let _ = running.stdin.flush();
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            if running.child.try_wait()?.is_some() { return Ok(true); }
            if Instant::now() >= until { break; }
            std::thread::sleep(Duration::from_millis(200));
        }
        running.child.kill().context("stopping the Minecraft server process")?;
        let _ = running.child.wait();
        Ok(true)
    }
}

impl Drop for ServerHost {
    fn drop(&mut self) {
        if let Ok(children) = self.children.get_mut() {
            for (_, mut server) in children.drain() {
                let _ = server.stdin.write_all(b"stop\n");
                let _ = server.stdin.flush();
                let until = Instant::now() + Duration::from_secs(15);
                while Instant::now() < until {
                    if matches!(server.child.try_wait(), Ok(Some(_))) { break; }
                    std::thread::sleep(Duration::from_millis(200));
                }
                if matches!(server.child.try_wait(), Ok(None)) { let _ = server.child.kill(); }
                let _ = server.child.wait();
            }
        }
    }
}

pub fn server_dir(paths: &Paths, id: &str) -> PathBuf { paths.instance_dir(id).join("server") }

fn eula_path(paths: &Paths, id: &str) -> PathBuf { server_dir(paths, id).join("eula.txt") }
fn properties_path(paths: &Paths, id: &str) -> PathBuf { server_dir(paths, id).join("server.properties") }

pub fn config(paths: &Paths, id: &str) -> ServerConfig {
    let text = std::fs::read_to_string(properties_path(paths, id)).unwrap_or_default();
    let get = |key: &str| text.lines().find_map(|line| line.split_once('=')
        .filter(|(k, _)| *k == key).map(|(_, value)| value.trim()));
    let eula_accepted = std::fs::read_to_string(eula_path(paths, id)).ok()
        .is_some_and(|s| s.lines().any(|l| l.trim() == "eula=true"));
    ServerConfig {
        online_mode: get("online-mode").and_then(|v| v.parse().ok()).unwrap_or(true),
        port: get("server-port").and_then(|v| v.parse().ok()).filter(|p| *p >= 1024).unwrap_or(25565),
        max_players: get("max-players").and_then(|v| v.parse().ok()).filter(|n| (1..=100).contains(n)).unwrap_or(20),
        eula_accepted,
    }
}

pub fn save_config(paths: &Paths, id: &str, cfg: &ServerConfig) -> Result<()> {
    if !(1024..=65535).contains(&cfg.port) { return Err(anyhow!("Choose a port between 1024 and 65535")); }
    if !(1..=100).contains(&cfg.max_players) { return Err(anyhow!("Maximum players must be between 1 and 100")); }
    let dir = server_dir(paths, id);
    std::fs::create_dir_all(&dir)?;
    if cfg.eula_accepted {
        std::fs::write(eula_path(paths, id), "# Accepted in SM Craft. See https://www.minecraft.net/eula\neula=true\n")?;
    }
    let file = properties_path(paths, id);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<String> = old.lines().filter(|line| {
        !["online-mode", "server-port", "max-players", "server-ip"].iter().any(|key| line.starts_with(&format!("{key}=")))
    }).map(str::to_owned).collect();
    lines.extend([
        format!("online-mode={}", cfg.online_mode),
        format!("server-port={}", cfg.port),
        format!("max-players={}", cfg.max_players),
        "server-ip=".to_string(),
    ]);
    let tmp = file.with_extension("properties.tmp");
    std::fs::write(&tmp, lines.join("\n") + "\n")?;
    #[cfg(windows)]
    if file.exists() { std::fs::remove_file(&file)?; }
    std::fs::rename(tmp, file)?;
    Ok(())
}

fn log(sink: &dyn EventSink, id: &str, text: String) {
    sink.log(LogLine { instance_id: id.to_string(), level: "info".into(), text: format!("[Server] {text}") });
}

fn pump_log<R: std::io::Read + Send + 'static>(reader: R, sink: Arc<dyn EventSink>, id: String, is_err: bool) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().flatten() {
            log(&*sink, &id, if is_err { format!("[stderr] {line}") } else { line });
        }
    });
}

pub async fn start(host: &ServerHost, store: &Store, id: &str, sink: Arc<dyn EventSink>) -> Result<()> {
    let instance = store.get_instance(id)?;
    let _starting = host.reserve_start(id)?;
    let cfg = config(&store.paths, id);
    if !cfg.eula_accepted { return Err(anyhow!("Read and accept the Minecraft EULA in Server Hosting before starting")); }
    save_config(&store.paths, id, &cfg)?;
    log(&*sink, id, "Preparing dedicated server files".into());
    let paths = store.paths.clone();
    let dir = server_dir(&paths, id);
    std::fs::create_dir_all(&dir)?;
    let vanilla = crate::versions::load_version(&paths, &instance.mc_version).await?;
    let major = crate::java::required_major(&vanilla);
    let jar = dir.join("server.jar");
    match instance.loader.as_str() {
        "" | "vanilla" => {
            let artifact = vanilla.downloads.as_ref().and_then(|d| d.server.as_ref())
                .ok_or_else(|| anyhow!("Mojang does not provide a dedicated server for Minecraft {}", instance.mc_version))?;
            if artifact.url.is_empty() { return Err(anyhow!("Mojang's server download is unavailable for Minecraft {}", instance.mc_version)); }
            download_file(&DlTask::new(artifact.url.clone(), &jar).with_sha1(artifact.sha1.clone()).with_size(artifact.size)).await?;
        }
        "fabric" | "quilt" => {
            if instance.loader == "quilt" { return Err(anyhow!("Quilt's server launcher is not yet wired into this host. Vanilla and Fabric are supported.")); }
            let loader = if instance.loader_version.trim().is_empty() {
                crate::loaders::latest_loader_version(&instance.loader, &instance.mc_version).await?
            } else { instance.loader_version.clone() };
            #[derive(serde::Deserialize)]
            struct Installer { version: String, #[serde(default)] stable: bool }
            let installers: Vec<Installer> = crate::download::client()
                .get("https://meta.fabricmc.net/v2/versions/installer")
                .send().await?.error_for_status()?.json().await?;
            let installer = installers.iter().find(|v| v.stable).or_else(|| installers.first())
                .ok_or_else(|| anyhow!("Fabric did not provide an installer version"))?;
            let safe = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
            if !safe(&loader) || !safe(&installer.version) || !safe(&instance.mc_version) {
                return Err(anyhow!("The saved Minecraft or Fabric version contains invalid characters"));
            }
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar", instance.mc_version, loader, installer.version);
            download_file(&DlTask::new(url, &jar)).await?;
        }
        other => return Err(anyhow!("Server hosting for {other} instances is not supported yet. Vanilla, Fabric, and Quilt are supported.")),
    }
    log(&*sink, id, "Resolving server Java runtime".into());
    let java = crate::java::resolve_java(&paths, &instance, major, sink.clone(), id).await?;
    if java.major < major { return Err(anyhow!("This server requires Java {major}, but the selected runtime is Java {}. Set Java to Auto or choose a newer runtime.", java.major)); }
    log(&*sink, id, format!("Starting Minecraft {} server ({})", instance.mc_version, instance.loader));
    let mut command = Command::new(&java.path);
    command.args(["-Xms1G", "-Xmx2G", "-jar"]).arg(&jar).arg("nogui")
        .current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().with_context(|| format!("starting server with Java at {}", java.path))?;
    let stdin = child.stdin.take().ok_or_else(|| anyhow!("server console input could not be opened"))?;
    if let Some(stdout) = child.stdout.take() { pump_log(stdout, sink.clone(), id.to_string(), false); }
    if let Some(stderr) = child.stderr.take() { pump_log(stderr, sink.clone(), id.to_string(), true); }
    host.children.lock().unwrap().insert(id.to_string(), RunningServer { child, stdin });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_defaults_securely_and_round_trips() {
        let root = std::env::temp_dir().join(format!("sc-server-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        let initial = config(&paths, "world");
        assert!(initial.online_mode);
        assert!(!initial.eula_accepted);
        let cfg = ServerConfig { online_mode: false, port: 25570, max_players: 8, eula_accepted: true };
        save_config(&paths, "world", &cfg).unwrap();
        let loaded = config(&paths, "world");
        assert!(!loaded.online_mode);
        assert_eq!(loaded.port, 25570);
        assert_eq!(loaded.max_players, 8);
        assert!(loaded.eula_accepted);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn rejects_privileged_or_invalid_server_ports() {
        let root = std::env::temp_dir().join(format!("sc-server-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        let cfg = ServerConfig { port: 80, ..Default::default() };
        assert!(save_config(&paths, "world", &cfg).is_err());
        std::fs::remove_dir_all(root).ok();
    }
}

