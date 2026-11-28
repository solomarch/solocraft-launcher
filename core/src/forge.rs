//! Forge and NeoForge. Unlike Fabric they have no ready-made launch profile:
//! the official installer jar patches the game. We run that installer
//! headless in a scratch folder, then copy the finished version profile and
//! libraries into SM Craft's own folders (so nothing touches .minecraft).

use crate::download::{client, download_file, DlTask};
use crate::events::{emit_progress, EventSink};
use crate::models::{LoaderVersion, LogLine};
use crate::paths::Paths;
use crate::versions;
use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

const FORGE_REPO: &str = "https://maven.minecraftforge.net";
const NEO_REPO: &str = "https://maven.neoforged.net/releases";

fn repo(loader: &str) -> Result<&'static str> {
    match loader {
        "forge" => Ok(FORGE_REPO),
        "neoforge" => Ok(NEO_REPO),
        o => Err(anyhow!("{o} is not a Forge-style loader")),
    }
}

fn metadata_url(loader: &str) -> Result<String> {
    Ok(match loader {
        "forge" => format!("{FORGE_REPO}/net/minecraftforge/forge/maven-metadata.xml"),
        "neoforge" => format!("{NEO_REPO}/net/neoforged/neoforge/maven-metadata.xml"),
        o => return Err(anyhow!("{o} is not a Forge-style loader")),
    })
}

/// Every `<version>` entry of a maven-metadata.xml, oldest first.
pub fn parse_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(s) = rest.find("<version>") {
        let after = &rest[s + 9..];
        let Some(e) = after.find("</version>") else { break };
        let v = after[..e].trim();
        if !v.is_empty() {
            out.push(v.to_string());
        }
        rest = &after[e + 10..];
    }
    out
}

/// NeoForge numbers versions after Minecraft: 21.4.x -> 1.21.4, 21.0.x -> 1.21.
pub fn neoforge_mc(v: &str) -> Option<String> {
    let mut it = v.split('.');
    let a: u32 = it.next()?.parse().ok()?;
    let b: u32 = it.next()?.parse().ok()?;
    match a {
        20..=25 => Some(if b == 0 { format!("1.{a}") } else { format!("1.{a}.{b}") }),
        // Minecraft's year-style names (26.1 ...): best guess, same pattern.
        26..=40 => Some(format!("{a}.{b}")),
        _ => None,
    }
}

/// (loader version, full maven version) pairs for a Minecraft version, newest first.
pub fn versions_for(loader: &str, mc: &str, all: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for e in all {
        match loader {
            "forge" => {
                let Some((m, rest)) = e.split_once('-') else { continue };
                if m != mc {
                    continue;
                }
                // very old builds look like 10.13.4.1614-1.7.10
                let lv = rest.strip_suffix(&format!("-{mc}")).unwrap_or(rest);
                out.push((lv.to_string(), e.clone()));
            }
            _ => {
                if neoforge_mc(e).as_deref() == Some(mc) {
                    out.push((e.clone(), e.clone()));
                }
            }
        }
    }
    out.reverse();
    out
}

async fn fetch_all(loader: &str) -> Result<Vec<String>> {
    let url = metadata_url(loader)?;
    let text = client()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("contacting the {loader} download server"))?
        .error_for_status()?
        .text()
        .await?;
    Ok(parse_versions(&text))
}

pub async fn list(loader: &str, mc: &str) -> Result<Vec<LoaderVersion>> {
    let all = fetch_all(loader).await?;
    let found = versions_for(loader, mc, &all);
    if found.is_empty() {
        return Err(anyhow!("{loader} does not support Minecraft {mc} yet"));
    }
    Ok(found
        .into_iter()
        .map(|(lv, _)| {
            let l = lv.to_lowercase();
            LoaderVersion { stable: !(l.contains("beta") || l.contains("alpha")), version: lv }
        })
        .collect())
}

/// Name of the profile we store under versions/ once installed.
pub fn installed_id(loader: &str, mc: &str, lv: &str) -> String {
    match loader {
        "neoforge" => format!("neoforge-{lv}"),
        _ => format!("forge-{mc}-{lv}"),
    }
}

pub fn installer_url(loader: &str, full: &str) -> Result<String> {
    let base = repo(loader)?;
    Ok(match loader {
        "forge" => format!("{base}/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"),
        _ => format!("{base}/net/neoforged/neoforge/{full}/neoforge-{full}-installer.jar"),
    })
}

fn copy_tree(src: &Path, dst: &Path) -> Result<usize> {
    let mut n = 0;
    if !src.exists() {
        return Ok(0);
    }
    let mut stack: Vec<PathBuf> = vec![src.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p.strip_prefix(src)?;
            let out = dst.join(rel);
            let len = e.metadata()?.len();
            if out.metadata().map(|m| m.len() == len).unwrap_or(false) {
                continue;
            }
            if let Some(par) = out.parent() {
                std::fs::create_dir_all(par)?;
            }
            std::fs::copy(&p, &out)?;
            n += 1;
        }
    }
    Ok(n)
}

fn read_lines<R: AsyncRead + Unpin + Send + 'static>(r: R, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    tokio::spawn(async move {
        let mut r = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let _ = tx.send(String::from_utf8_lossy(&buf).trim_end().to_string());
                }
            }
        }
    });
}

/// Runs an installer jar for the client side in a scratch folder and moves its
/// output into `paths`. `mc` must already be installed (json + jar) so the
/// installer does not have to download it again.
pub async fn run_installer(
    paths: &Paths,
    id: &str,
    mc: &str,
    installer: &Path,
    java: &str,
    sink: &dyn EventSink,
    task: &str,
    label: &str,
) -> Result<()> {
    let scratch = paths.root.join("tmp").join(format!("loader-{id}"));
    let _ = tokio::fs::remove_dir_all(&scratch).await;
    let vdir = scratch.join("versions").join(mc);
    tokio::fs::create_dir_all(&vdir).await?;
    // The installer refuses to run without a launcher profile file.
    tokio::fs::write(scratch.join("launcher_profiles.json"), r#"{"profiles":{},"version":3}"#).await?;
    let _ = tokio::fs::copy(paths.version_json(mc), vdir.join(format!("{mc}.json"))).await;
    let _ = tokio::fs::copy(paths.client_jar(mc), vdir.join(format!("{mc}.jar"))).await;

    emit_progress(sink, task, &format!("Installing {label} (1–3 minutes, one time)"), 0, 0);
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(installer)
        .arg("--installClient")
        .arg(&scratch)
        .current_dir(&scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start Java at {java}: {e}"))?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Some(o) = child.stdout.take() {
        read_lines(o, tx.clone());
    }
    if let Some(e) = child.stderr.take() {
        read_lines(e, tx.clone());
    }
    drop(tx);

    let mut tail: VecDeque<String> = VecDeque::new();
    let pump = async {
        while let Some(line) = rx.recv().await {
            if line.trim().is_empty() {
                continue;
            }
            sink.log(LogLine {
                instance_id: task.to_string(),
                level: "info".into(),
                text: format!("[{label} installer] {line}"),
            });
            if tail.len() >= 14 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
    };
    if tokio::time::timeout(std::time::Duration::from_secs(900), pump).await.is_err() {
        let _ = child.kill().await;
        return Err(anyhow!("The {label} installer took longer than 15 minutes and was stopped"));
    }
    let status = child.wait().await?;
    if !status.success() {
        let _ = tokio::fs::remove_dir_all(&scratch).await;
        let why = tail.iter().cloned().collect::<Vec<_>>().join("\n");
        return Err(anyhow!("The {label} installer failed (exit {}).\n{why}", status.code().unwrap_or(-1)));
    }

    // Find the profile the installer wrote (anything in versions/ except vanilla).
    let mut found: Option<(String, PathBuf)> = None;
    let mut rd = tokio::fs::read_dir(scratch.join("versions")).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().to_string();
        let json = e.path().join(format!("{name}.json"));
        if name != mc && json.exists() {
            if found.is_none() || name.to_lowercase().contains("forge") {
                found = Some((name, json));
            }
        }
    }
    let (_, json) = found.ok_or_else(|| {
        anyhow!("The {label} installer finished but did not create a version profile. Check the log lines above.")
    })?;

    let libs_src = scratch.join("libraries");
    let libs_dst = paths.libraries();
    let json_dst = paths.version_json(id);
    tokio::task::spawn_blocking(move || -> Result<()> {
        copy_tree(&libs_src, &libs_dst)?;
        if let Some(p) = json_dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(&json, &json_dst)?;
        Ok(())
    })
    .await??;
    let _ = tokio::fs::remove_dir_all(&scratch).await;
    Ok(())
}

/// Makes sure the Forge/NeoForge profile for this instance exists locally,
/// installing it when needed. `java` must be able to run the installer.
pub async fn ensure_installed(
    paths: &Paths,
    loader: &str,
    mc: &str,
    lv: &str,
    java: &str,
    sink: &dyn EventSink,
    task: &str,
) -> Result<()> {
    let id = installed_id(loader, mc, lv);
    if paths.version_json(&id).exists() {
        return Ok(());
    }
    let label = if loader == "neoforge" { "NeoForge" } else { "Forge" };

    // vanilla json + jar, so the installer can patch it
    let vanilla = versions::load_version(paths, mc).await?;
    if let Some(c) = vanilla.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        emit_progress(sink, task, "Downloading Minecraft", 0, 0);
        download_file(&DlTask::new(c.url.clone(), paths.client_jar(mc)).with_sha1(c.sha1.clone()).with_size(c.size))
            .await
            .context("Minecraft client jar")?;
    }

    let all = fetch_all(loader).await?;
    let full = versions_for(loader, mc, &all)
        .into_iter()
        .find(|(v, _)| v == lv)
        .map(|(_, f)| f)
        .ok_or_else(|| anyhow!("{label} {lv} is not available for Minecraft {mc}"))?;
    let url = installer_url(loader, &full)?;
    let dir = paths.root.join("downloads");
    tokio::fs::create_dir_all(&dir).await?;
    let jar = dir.join(format!("{loader}-{full}-installer.jar"));
    emit_progress(sink, task, &format!("Downloading the {label} installer"), 0, 0);
    // maven publishes a .sha1 next to every file; use it when present
    let sha1 = match client().get(format!("{url}.sha1")).send().await {
        Ok(r) if r.status().is_success() => r.text().await.ok().map(|t| t.trim().to_lowercase()),
        _ => None,
    }
    .filter(|s| s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()));
    download_file(&DlTask::new(url, jar.clone()).with_sha1(sha1))
        .await
        .with_context(|| format!("downloading the {label} installer"))?;

    let res = run_installer(paths, &id, mc, &jar, java, sink, task, label).await;
    let _ = tokio::fs::remove_file(&jar).await;
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<metadata><versioning><versions>\
        <version>1.20.1-47.2.0</version><version>1.20.1-47.3.0</version>\
        <version>1.21.4-54.0.1</version><version>1.7.10-10.13.4.1614-1.7.10</version>\
        </versions></versioning></metadata>";

    #[test]
    fn parses_and_filters_forge() {
        let all = parse_versions(XML);
        assert_eq!(all.len(), 4);
        let v = versions_for("forge", "1.20.1", &all);
        assert_eq!(v[0], ("47.3.0".to_string(), "1.20.1-47.3.0".to_string()));
        assert_eq!(v.len(), 2);
        let old = versions_for("forge", "1.7.10", &all);
        assert_eq!(old[0], ("10.13.4.1614".to_string(), "1.7.10-10.13.4.1614-1.7.10".to_string()));
    }

    #[test]
    fn neoforge_numbering() {
        assert_eq!(neoforge_mc("21.4.100").as_deref(), Some("1.21.4"));
        assert_eq!(neoforge_mc("21.0.5-beta").as_deref(), Some("1.21"));
        assert_eq!(neoforge_mc("20.2.86").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_mc("47.1.82"), None);
        let all = vec!["21.4.1".to_string(), "21.4.100".to_string(), "21.1.9".to_string()];
        let v = versions_for("neoforge", "1.21.4", &all);
        assert_eq!(v[0].0, "21.4.100");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn urls_and_ids() {
        assert_eq!(
            installer_url("forge", "1.20.1-47.2.0").unwrap(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar"
        );
        assert_eq!(
            installer_url("neoforge", "21.4.100").unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.4.100/neoforge-21.4.100-installer.jar"
        );
        assert_eq!(installed_id("forge", "1.20.1", "47.2.0"), "forge-1.20.1-47.2.0");
        assert_eq!(installed_id("neoforge", "1.21.4", "21.4.100"), "neoforge-21.4.100");
    }
}
