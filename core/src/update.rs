//! Looks for a newer SoloCraft Launcher release on GitHub.
use serde::{Deserialize, Serialize};

/// GitHub repository that publishes the launcher releases (override with SOLOCRAFT_REPO).
pub const DEFAULT_REPO: &str = "solomarch/solocraft-launcher";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current: String,
    pub latest: String,
    pub page_url: String,
    pub download_url: String,
    pub notes: String,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

pub fn repo() -> String {
    std::env::var("SOLOCRAFT_REPO")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

/// "v1.2.3-beta" -> [1,2,3]
pub fn parse_version(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse_version(latest), parse_version(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

fn pick_asset(assets: &[Asset]) -> Option<&Asset> {
    let want: &[&str] = if cfg!(windows) {
        &[".msi", "setup.exe", ".exe"]
    } else if cfg!(target_os = "macos") {
        &[".dmg"]
    } else {
        &[".appimage", ".deb", ".rpm"]
    };
    for ext in want {
        if let Some(a) = assets.iter().find(|a| a.name.to_lowercase().ends_with(ext)) {
            return Some(a);
        }
    }
    None
}

/// Asks GitHub for the newest published release. A missing release is not an error.
pub async fn check(current: &str) -> anyhow::Result<UpdateInfo> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
    let resp = crate::download::client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;
    let mut info = UpdateInfo { current: current.to_string(), latest: current.to_string(), ..Default::default() };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(info); // no release published yet
    }
    let rel: Release = resp.error_for_status()?.json().await?;
    if rel.draft || rel.prerelease {
        return Ok(info);
    }
    info.latest = rel.tag_name.trim_start_matches(['v', 'V']).to_string();
    info.available = is_newer(&rel.tag_name, current);
    info.page_url = rel.html_url.clone();
    info.download_url = pick_asset(&rel.assets).map(|a| a.browser_download_url.clone()).unwrap_or(rel.html_url);
    info.notes = rel.body.unwrap_or_default().chars().take(600).collect();
    Ok(info)
}

/// Only GitHub addresses of the configured repository may be opened from the UI.
pub fn url_allowed(url: &str) -> bool {
    url.starts_with(&format!("https://github.com/{}/", repo()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_versions() {
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("1.1", "1.0.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("v0.9.0", "1.0.0"));
        assert!(is_newer("1.2.0-beta", "1.1.0"));
    }
    #[test]
    fn only_own_repo_urls() {
        assert!(url_allowed(&format!("https://github.com/{}/releases/tag/v1", repo())));
        assert!(!url_allowed("https://evil.example/x"));
        assert!(!url_allowed("https://github.com/someone/else/releases"));
    }
}
