//! Mod loader support. Fabric and Quilt publish ready-made launch profiles on
//! their meta servers; we download the profile, merge it with the vanilla
//! version it inherits from, and launch it like any other version.

use crate::download::{client, DlTask, download_file};
use crate::mcjson::{merge_loader, McVersion};
use crate::models::{Instance, LoaderVersion};
use crate::paths::Paths;
use crate::versions;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

pub const LOADERS: [&str; 5] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];

fn meta_base(loader: &str) -> Result<&'static str> {
    match loader {
        "fabric" => Ok("https://meta.fabricmc.net/v2/versions/loader"),
        "quilt" => Ok("https://meta.quiltmc.org/v3/versions/loader"),
        other => Err(anyhow!("{other} is not supported yet")),
    }
}

#[derive(Deserialize)]
struct LoaderRow {
    loader: LoaderInner,
}

#[derive(Deserialize)]
struct LoaderInner {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

/// Loader versions that support a given Minecraft version, newest first.
pub async fn list_loader_versions(loader: &str, mc: &str) -> Result<Vec<LoaderVersion>> {
    if matches!(loader, "forge" | "neoforge") {
        return crate::forge::list(loader, mc).await;
    }
    let url = format!("{}/{}", meta_base(loader)?, mc);
    let rows: Vec<LoaderRow> = client()
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .with_context(|| format!("{loader} does not list Minecraft {mc}"))?
        .json()
        .await?;
    if rows.is_empty() {
        return Err(anyhow!("{loader} does not support Minecraft {mc} yet"));
    }
    Ok(rows
        .into_iter()
        .map(|r| LoaderVersion {
            // Quilt does not flag stability; treat non-beta names as stable.
            stable: r.loader.stable.unwrap_or_else(|| {
                let v = r.loader.version.to_lowercase();
                !(v.contains("beta") || v.contains("alpha") || v.contains("pre"))
            }),
            version: r.loader.version,
        })
        .collect())
}

pub async fn latest_loader_version(loader: &str, mc: &str) -> Result<String> {
    let list = list_loader_versions(loader, mc).await?;
    Ok(list
        .iter()
        .find(|v| v.stable)
        .or_else(|| list.first())
        .map(|v| v.version.clone())
        .unwrap())
}

pub fn profile_id(loader: &str, loader_version: &str, mc: &str) -> String {
    match loader {
        "forge" | "neoforge" => crate::forge::installed_id(loader, mc, loader_version),
        "quilt" => format!("quilt-loader-{loader_version}-{mc}"),
        _ => format!("fabric-loader-{loader_version}-{mc}"),
    }
}

/// Loads the version to launch: plain vanilla, or the loader profile merged
/// with vanilla. Returns the merged version and the loader version used.
pub async fn load_effective_version(
    paths: &Paths,
    inst: &Instance,
) -> Result<(McVersion, String)> {
    let vanilla = versions::load_version(paths, &inst.mc_version).await?;
    if inst.loader == "vanilla" || inst.loader.is_empty() {
        return Ok((vanilla, String::new()));
    }
    let lv = if inst.loader_version.trim().is_empty() {
        latest_loader_version(&inst.loader, &inst.mc_version).await?
    } else {
        inst.loader_version.clone()
    };
    let id = profile_id(&inst.loader, &lv, &inst.mc_version);
    let file = paths.version_json(&id);
    if matches!(inst.loader.as_str(), "forge" | "neoforge") && !file.exists() {
        return Err(anyhow!("{} {lv} is not installed yet", inst.loader));
    }
    if !file.exists() {
        let url = format!(
            "{}/{}/{}/profile/json",
            meta_base(&inst.loader)?,
            inst.mc_version,
            lv
        );
        download_file(&DlTask::new(url, file.clone()))
            .await
            .with_context(|| format!("downloading the {} profile", inst.loader))?;
    }
    let bytes = tokio::fs::read(&file).await?;
    let child: McVersion = match serde_json::from_slice(&bytes) {
        Ok(c) => c,
        Err(e) => {
            let _ = tokio::fs::remove_file(&file).await;
            return Err(anyhow!("The {} profile was unreadable ({e}). Try again.", inst.loader));
        }
    };
    Ok((merge_loader(vanilla, child), lv))
}
