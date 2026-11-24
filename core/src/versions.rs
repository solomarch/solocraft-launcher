use crate::download::{client, download_file, DlTask};
use crate::mcjson::McVersion;
use crate::models::VersionEntry;
use crate::paths::Paths;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
    release_time: String,
    sha1: Option<String>,
}

async fn load_manifest(paths: &Paths) -> Result<Manifest> {
    let cache = paths.meta().join("version_manifest.json");
    let fetched = async {
        let text = client()
            .get(MANIFEST_URL)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        // Validate before caching.
        let m: Manifest = serde_json::from_str(&text)?;
        tokio::fs::create_dir_all(paths.meta()).await.ok();
        tokio::fs::write(&cache, text).await.ok();
        Ok::<Manifest, anyhow::Error>(m)
    }
    .await;
    match fetched {
        Ok(m) => Ok(m),
        Err(net_err) => {
            let bytes = tokio::fs::read(&cache)
                .await
                .map_err(|_| anyhow!("Could not reach Mojang and no cached version list exists: {net_err:#}"))?;
            Ok(serde_json::from_slice(&bytes)?)
        }
    }
}

pub async fn list_versions(paths: &Paths) -> Result<Vec<VersionEntry>> {
    let m = load_manifest(paths).await?;
    Ok(m.versions
        .into_iter()
        .map(|e| VersionEntry {
            id: e.id,
            kind: e.kind,
            release_time: e.release_time,
        })
        .collect())
}

/// Loads a version JSON, downloading it (SHA-1 verified) when it is not cached.
pub async fn load_version(paths: &Paths, id: &str) -> Result<McVersion> {
    let file = paths.version_json(id);
    if !file.exists() {
        let m = load_manifest(paths).await?;
        let entry = m
            .versions
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow!("Minecraft version {id} was not found in the Mojang version list"))?;
        let task = DlTask::new(entry.url.clone(), file.clone()).with_sha1(entry.sha1.clone());
        download_file(&task).await?;
    }
    let bytes = tokio::fs::read(&file).await?;
    serde_json::from_slice::<McVersion>(&bytes)
        .with_context(|| format!("parsing version JSON for {id}"))
}
