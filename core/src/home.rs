//! Data for the Home page: the player's skin, Minecraft news, playtime per day and screenshots.
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

// ---------- tiny base64 (no extra crate) ----------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut buf = 0u32;
    let mut bits = 0;
    let mut out = Vec::new();
    for ch in s.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' || ch == b' ' {
            continue;
        }
        let v = B64.iter().position(|&b| b == ch)? as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------- skin ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkinInfo {
    /// `data:image/png;base64,...` (empty when the account has no downloadable skin)
    pub data_url: String,
    pub slim: bool,
}

/// Reads the skin address out of the Mojang profile answer.
pub fn parse_skin_profile(profile: &Value) -> Option<(String, bool)> {
    let prop = profile.get("properties")?.as_array()?.iter().find(|p| p.get("name").and_then(|n| n.as_str()) == Some("textures"))?;
    let raw = b64_decode(prop.get("value")?.as_str()?)?;
    let tex: Value = serde_json::from_slice(&raw).ok()?;
    let skin = tex.get("textures")?.get("SKIN")?;
    let url = skin.get("url")?.as_str()?.replacen("http://", "https://", 1);
    let slim = skin.get("metadata").and_then(|m| m.get("model")).and_then(|m| m.as_str()) == Some("slim");
    Some((url, slim))
}

/// Downloads the skin of an online account. Offline accounts have none: the page draws a default.
pub async fn skin(uuid: &str, kind: &str) -> Result<SkinInfo> {
    if kind != "microsoft" {
        return Ok(SkinInfo::default());
    }
    let id: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if id.len() != 32 {
        return Ok(SkinInfo::default());
    }
    let c = crate::download::client();
    let profile: Value = c
        .get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some((url, slim)) = parse_skin_profile(&profile) else { return Ok(SkinInfo::default()) };
    if !url.starts_with("https://textures.minecraft.net/") {
        return Err(anyhow!("unexpected skin address"));
    }
    let bytes = c.get(&url).timeout(std::time::Duration::from_secs(10)).send().await?.error_for_status()?.bytes().await?;
    if bytes.len() > 400_000 {
        return Err(anyhow!("skin file is too large"));
    }
    Ok(SkinInfo { data_url: format!("data:image/png;base64,{}", b64_encode(&bytes)), slim })
}

// ---------- news ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    pub version: String,
    pub kind: String,
    pub date: String,
}

pub fn parse_news(v: &Value) -> Vec<NewsItem> {
    let s = |e: &Value, k: &str| e.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut items: Vec<NewsItem> = v
        .get("entries")
        .and_then(|e| e.as_array())
        .map(|a| a.iter().map(|e| NewsItem { title: s(e, "title"), version: s(e, "version"), kind: s(e, "type"), date: s(e, "date") }).filter(|n| !n.title.is_empty()).collect())
        .unwrap_or_default();
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(3);
    items
}

/// Latest Java patch notes from Mojang (the format can change, so everything is optional).
pub async fn news() -> Result<Vec<NewsItem>> {
    let c = crate::download::client();
    for url in ["https://launchercontent.mojang.com/v2/javaPatchNotes.json", "https://launchercontent.mojang.com/javaPatchNotes.json"] {
        if let Ok(r) = c.get(url).timeout(std::time::Duration::from_secs(10)).send().await {
            if let Ok(r) = r.error_for_status() {
                if let Ok(v) = r.json::<Value>().await {
                    let n = parse_news(&v);
                    if !n.is_empty() {
                        return Ok(n);
                    }
                }
            }
        }
    }
    Ok(vec![])
}

// ---------- playtime ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayPlay {
    pub date: String,
    pub secs: u64,
}

fn playtime_file(paths: &Paths) -> PathBuf {
    paths.root.join("playtime.json")
}

fn load_playtime(paths: &Paths) -> std::collections::BTreeMap<String, u64> {
    std::fs::read(playtime_file(paths)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Adds a finished play session to today's total.
pub fn record_playtime(paths: &Paths, secs: u64) {
    if secs == 0 {
        return;
    }
    let mut m = load_playtime(paths);
    *m.entry(chrono::Local::now().format("%Y-%m-%d").to_string()).or_insert(0) += secs;
    // keep a year of history
    while m.len() > 366 {
        let k = m.keys().next().cloned().unwrap();
        m.remove(&k);
    }
    if let Ok(b) = serde_json::to_vec(&m) {
        let _ = std::fs::write(playtime_file(paths), b);
    }
}

/// The last 7 days, oldest first (days without play are 0).
pub fn playtime_week(paths: &Paths) -> Vec<DayPlay> {
    let m = load_playtime(paths);
    let today = chrono::Local::now().date_naive();
    (0..7)
        .rev()
        .map(|i| {
            let d = (today - chrono::Duration::days(i)).format("%Y-%m-%d").to_string();
            DayPlay { secs: m.get(&d).copied().unwrap_or(0), date: d }
        })
        .collect()
}

// ---------- screenshots ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub path: String,
    pub name: String,
    pub instance: String,
    pub modified: i64,
}

/// Newest screenshots across all instances.
pub fn list_screenshots(paths: &Paths, limit: usize) -> Vec<Shot> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(paths.instances()) else { return out };
    for inst in rd.flatten() {
        let dir = inst.path().join(".minecraft").join("screenshots");
        let Ok(files) = std::fs::read_dir(&dir) else { continue };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().ends_with(".png") {
                continue;
            }
            let modified = f
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push(Shot { path: f.path().to_string_lossy().to_string(), name, instance: inst.file_name().to_string_lossy().to_string(), modified });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out.truncate(limit);
    out
}

/// Returns one screenshot as a data URL. Only files inside the instances folder are allowed.
pub fn read_screenshot(paths: &Paths, path: &str) -> Result<String> {
    let p = Path::new(path).canonicalize()?;
    let root = paths.instances().canonicalize()?;
    if !p.starts_with(&root) || !p.to_string_lossy().to_lowercase().ends_with(".png") {
        return Err(anyhow!("not a screenshot of an instance"));
    }
    let meta = std::fs::metadata(&p)?;
    if meta.len() > 12_000_000 {
        return Err(anyhow!("screenshot is too large to preview"));
    }
    Ok(format!("data:image/png;base64,{}", b64_encode(&std::fs::read(&p)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            let e = b64_encode(s.as_bytes());
            assert_eq!(b64_decode(&e).unwrap(), s.as_bytes());
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn skin_profile_is_parsed() {
        let tex = r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#;
        let profile = serde_json::json!({"properties":[{"name":"textures","value": b64_encode(tex.as_bytes())}]});
        assert_eq!(parse_skin_profile(&profile), Some(("https://textures.minecraft.net/texture/abc".into(), true)));
        assert_eq!(parse_skin_profile(&serde_json::json!({})), None);
    }

    #[test]
    fn news_is_sorted_and_trimmed() {
        let v = serde_json::json!({"entries":[
            {"title":"Old","date":"2024-01-01"},{"title":"New","version":"1.21.11","type":"release","date":"2026-05-01"},
            {"title":"Mid","date":"2025-01-01"},{"title":"Older","date":"2023-01-01"},{"title":""}]});
        let n = parse_news(&v);
        assert_eq!(n.len(), 3);
        assert_eq!(n[0].title, "New");
        assert_eq!(n[0].version, "1.21.11");
    }

    #[test]
    fn playtime_adds_up() {
        let d = std::env::temp_dir().join(format!("sc-play-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = Paths::new(d);
        record_playtime(&p, 600);
        record_playtime(&p, 300);
        let w = playtime_week(&p);
        assert_eq!(w.len(), 7);
        assert_eq!(w[6].secs, 900);
        assert_eq!(w[0].secs, 0);
    }

    #[test]
    fn screenshot_paths_are_confined() {
        let d = std::env::temp_dir().join(format!("sc-shot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let sdir = d.join("instances/a/.minecraft/screenshots");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(sdir.join("x.png"), [1, 2, 3]).unwrap();
        std::fs::write(d.join("secret.png"), [9]).unwrap();
        let p = Paths::new(d.clone());
        assert_eq!(list_screenshots(&p, 6).len(), 1);
        assert!(read_screenshot(&p, sdir.join("x.png").to_str().unwrap()).unwrap().starts_with("data:image/png;base64,"));
        assert!(read_screenshot(&p, d.join("secret.png").to_str().unwrap()).is_err());
    }
}
