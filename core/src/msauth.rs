//! Microsoft sign-in for Minecraft: Java Edition (official accounts).
//!
//! Flow: Microsoft device code -> Microsoft token -> Xbox Live token -> XSTS token
//! -> Minecraft access token -> profile (ownership check). The refresh token is
//! kept in `ms_secrets.json` next to the other launcher data, and the
//! Minecraft token is refreshed automatically before launch when it is about
//! to expire.
//!
//! The Azure application (client) ID is supplied by the user in Settings:
//! Microsoft only allows approved app IDs to use the Minecraft API.

use crate::download::client;
use crate::models::Account;
use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Endpoints {
    device: String,
    token: String,
    xbl: String,
    xsts: String,
    mc_login: String,
    profile: String,
}

fn endpoints() -> Endpoints {
    // SOLOCRAFT_AUTH_TEST_BASE lets the automated tests talk to a local fake server.
    if let Ok(b) = std::env::var("SOLOCRAFT_AUTH_TEST_BASE") {
        return Endpoints {
            device: format!("{b}/devicecode"),
            token: format!("{b}/token"),
            xbl: format!("{b}/xbl"),
            xsts: format!("{b}/xsts"),
            mc_login: format!("{b}/mc/login"),
            profile: format!("{b}/mc/profile"),
        };
    }
    Endpoints {
        device: "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".into(),
        token: "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".into(),
        xbl: "https://user.auth.xboxlive.com/user/authenticate".into(),
        xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
        mc_login: "https://api.minecraftservices.com/authentication/login_with_xbox".into(),
        profile: "https://api.minecraftservices.com/minecraft/profile".into(),
    }
}

const SCOPE: &str = "XboxLive.signin offline_access";

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
struct DeviceRaw {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default)]
    interval: u64,
}

#[derive(Deserialize)]
struct TokenRaw {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub struct MsTokens {
    pub access: String,
    pub refresh: String,
}

// Tokens are secrets: never print them, even by accident in a log.
impl std::fmt::Debug for MsTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MsTokens(<redacted>)")
    }
}

#[derive(Clone)]
pub struct McSession {
    pub token: String,
    pub expires_at: u64,
    pub username: String,
    /// undashed UUID
    pub uuid: String,
}

impl std::fmt::Debug for McSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "McSession({})", self.username)
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Secret {
    refresh_token: String,
    mc_token: String,
    expires_at: u64,
}

fn require_client_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() {
        return Err(anyhow!(
            "Microsoft sign-in has no client ID in this build (no app ID was compiled in). Please update SM Craft to the latest version."
        ));
    }
    Ok(id)
}

pub async fn begin_device_login(client_id: &str) -> Result<DeviceCode> {
    let cid = require_client_id(client_id)?;
    let ep = endpoints();
    let r = client()
        .post(&ep.device)
        .form(&[("client_id", cid), ("scope", SCOPE)])
        .send()
        .await
        .context("reaching Microsoft")?;
    let status = r.status();
    let text = r.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Microsoft refused the sign-in request ({status}): {}", short(&text)));
    }
    let d: DeviceRaw = serde_json::from_str(&text).context("unexpected reply from Microsoft")?;
    Ok(DeviceCode {
        device_code: d.device_code,
        user_code: d.user_code,
        verification_uri: d.verification_uri,
        expires_in: d.expires_in,
        interval: d.interval.max(1),
    })
}

fn short(s: &str) -> String {
    s.chars().take(300).collect()
}

/// Waits until the person has entered the code, the code expires, or `cancel` is set.
pub async fn poll_device_login(client_id: &str, dc: &DeviceCode, cancel: &AtomicBool) -> Result<MsTokens> {
    let cid = require_client_id(client_id)?;
    let ep = endpoints();
    let deadline = now() + dc.expires_in;
    let mut interval = dc.interval.max(1);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(anyhow!("Sign-in cancelled"));
        }
        if now() >= deadline {
            return Err(anyhow!("The sign-in code expired. Start again."));
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let r = client()
            .post(&ep.token)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", cid),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .await
            .context("reaching Microsoft")?;
        let t: TokenRaw = r.json().await.context("unexpected reply from Microsoft")?;
        if let (Some(a), Some(rf)) = (t.access_token, t.refresh_token) {
            return Ok(MsTokens { access: a, refresh: rf });
        }
        match t.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some("authorization_declined") => return Err(anyhow!("Sign-in was declined")),
            Some("expired_token") => return Err(anyhow!("The sign-in code expired. Start again.")),
            Some(e) => {
                return Err(anyhow!(
                    "Microsoft sign-in failed: {e} {}",
                    t.error_description.map(|d| short(&d)).unwrap_or_default()
                ))
            }
            None => return Err(anyhow!("Microsoft sign-in failed")),
        }
    }
}

pub async fn refresh_ms(client_id: &str, refresh_token: &str) -> Result<MsTokens> {
    let cid = require_client_id(client_id)?;
    let ep = endpoints();
    let r = client()
        .post(&ep.token)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", cid),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await
        .context("reaching Microsoft")?;
    let t: TokenRaw = r.json().await.context("unexpected reply from Microsoft")?;
    match (t.access_token, t.refresh_token) {
        (Some(a), Some(rf)) => Ok(MsTokens { access: a, refresh: rf }),
        _ => Err(anyhow!("Your Microsoft sign-in expired. Sign in again in Settings → Accounts.")),
    }
}

fn xsts_error(code: u64) -> &'static str {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile. Create one at minecraft.net or xbox.com, then try again.",
        2148916235 => "Xbox Live is not available in this account's country or region.",
        2148916236 | 2148916237 => "This account needs adult verification on the Xbox website.",
        2148916238 => "This is a child account. An adult must add it to a Microsoft Family first.",
        _ => "Xbox Live refused the sign-in.",
    }
}

/// Xbox Live -> XSTS -> Minecraft token -> profile.
pub async fn minecraft_session(ms_access: &str) -> Result<McSession> {
    let ep = endpoints();
    let c = client();

    let r = c
        .post(&ep.xbl)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
            "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"
        }))
        .send()
        .await
        .context("reaching Xbox Live")?;
    if !r.status().is_success() {
        return Err(anyhow!("Xbox Live sign-in failed ({})", r.status()));
    }
    let v: serde_json::Value = r.json().await?;
    let xbl = v["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live sent no token"))?.to_string();

    let r = c
        .post(&ep.xsts)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl]},
            "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"
        }))
        .send()
        .await
        .context("reaching Xbox Live")?;
    let status = r.status();
    let v: serde_json::Value = r.json().await.unwrap_or_default();
    if !status.is_success() {
        let code = v["XErr"].as_u64().unwrap_or(0);
        return Err(anyhow!("{}", xsts_error(code)));
    }
    let xsts = v["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live sent no XSTS token"))?.to_string();
    let uhs = v["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| anyhow!("Xbox Live sent no user hash"))?
        .to_string();

    let r = c
        .post(&ep.mc_login)
        .json(&serde_json::json!({"identityToken": format!("XBL3.0 x={uhs};{xsts}")}))
        .send()
        .await
        .context("reaching Minecraft services")?;
    let status = r.status();
    let text = r.text().await.unwrap_or_default();
    if status.as_u16() == 403 {
        return Err(anyhow!(
            "Minecraft services refused this app ID (403). Microsoft must approve your Azure app for the Minecraft API first. See https://aka.ms/AppRegInfo"
        ));
    }
    if !status.is_success() {
        return Err(anyhow!("Minecraft sign-in failed ({status}): {}", short(&text)));
    }
    let v: serde_json::Value = serde_json::from_str(&text).context("unexpected reply from Minecraft services")?;
    let token = v["access_token"].as_str().ok_or_else(|| anyhow!("Minecraft sent no token"))?.to_string();
    let expires_in = v["expires_in"].as_u64().unwrap_or(86400);

    let r = c
        .get(&ep.profile)
        .bearer_auth(&token)
        .send()
        .await
        .context("reaching Minecraft services")?;
    if r.status().as_u16() == 404 {
        return Err(anyhow!(
            "This Microsoft account does not own Minecraft: Java Edition (or has no profile yet). Buy it or log in once with the official launcher."
        ));
    }
    if !r.status().is_success() {
        return Err(anyhow!("Could not read the Minecraft profile ({})", r.status()));
    }
    let p: serde_json::Value = r.json().await?;
    Ok(McSession {
        token,
        expires_at: now() + expires_in,
        username: p["name"].as_str().unwrap_or_default().to_string(),
        uuid: p["id"].as_str().unwrap_or_default().to_string(),
    })
}

// ---------- storage ----------

fn secrets_path(store: &Store) -> std::path::PathBuf {
    store.paths.root.join("ms_secrets.json")
}

fn load_secrets(store: &Store) -> HashMap<String, Secret> {
    std::fs::read(secrets_path(store))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_secrets(store: &Store, m: &HashMap<String, Secret>) -> Result<()> {
    std::fs::create_dir_all(&store.paths.root)?;
    let path = secrets_path(store);
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(m)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(&tmp)?;
        std::io::Write::write_all(&mut f, &bytes)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, bytes)?;
    #[cfg(windows)]
    if path.exists() { std::fs::remove_file(&path)?; }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn forget(store: &Store, account_id: &str) {
    let mut m = load_secrets(store);
    if m.remove(account_id).is_some() {
        let _ = save_secrets(store, &m);
    }
}

/// Stores the result of a successful sign-in and returns the account.
pub fn save_login(store: &Store, ms: &MsTokens, s: &McSession) -> Result<Account> {
    if s.uuid.is_empty() || s.username.is_empty() {
        return Err(anyhow!("Minecraft returned no profile"));
    }
    let mut accounts = store.load_accounts();
    let acc = Account {
        id: s.uuid.clone(),
        username: s.username.clone(),
        uuid: s.uuid.clone(),
        kind: "microsoft".into(),
    };
    match accounts.accounts.iter_mut().find(|a| a.id == acc.id) {
        Some(existing) => *existing = acc.clone(),
        None => accounts.accounts.push(acc.clone()),
    }
    if accounts.active.is_none() {
        accounts.active = Some(acc.id.clone());
    }
    store.save_accounts(&accounts)?;
    let mut m = load_secrets(store);
    m.insert(
        acc.id.clone(),
        Secret { refresh_token: ms.refresh.clone(), mc_token: s.token.clone(), expires_at: s.expires_at },
    );
    save_secrets(store, &m)?;
    Ok(acc)
}

/// A valid Minecraft access token for the account, refreshing it if it expires
/// within five minutes. Also returns the current profile name.
pub async fn ensure_token(store: &Store, acc: &Account) -> Result<(String, McSession)> {
    let secrets = load_secrets(store);
    let sec = secrets
        .get(&acc.id)
        .cloned()
        .ok_or_else(|| anyhow!("This Microsoft account needs to sign in again (Settings → Accounts)."))?;
    if !sec.mc_token.is_empty() && sec.expires_at > now() + 300 {
        return Ok((
            sec.mc_token.clone(),
            McSession { token: sec.mc_token, expires_at: sec.expires_at, username: acc.username.clone(), uuid: acc.uuid.clone() },
        ));
    }
    let client_id = effective_client_id(store);
    let ms = refresh_ms(&client_id, &sec.refresh_token).await?;
    let session = minecraft_session(&ms.access).await?;
    save_login(store, &ms, &session)?;
    Ok((session.token.clone(), session))
}

/// Client ID baked in at build time (`SOLOCRAFT_MS_CLIENT_ID=... cargo build`).
pub fn builtin_client_id() -> &'static str {
    match option_env!("SOLOCRAFT_MS_CLIENT_ID") {
        Some(v) if !v.trim().is_empty() => v,
        _ => DEFAULT_CLIENT_ID,
    }
}

/// SM Craft's own Azure app registration (public client; the ID is not a secret).
/// Paste the "Application (client) ID" between the quotes.
const DEFAULT_CLIENT_ID: &str = "47fb9e67-429b-455c-94bd-d587bb1cf91c";

/// The ID in use. The built-in ID always wins (a stale ID saved in old settings can no longer override it);
/// a saved ID is only used when this build has none.
pub fn effective_client_id(store: &Store) -> String {
    let built = builtin_client_id().trim();
    if !built.is_empty() {
        return built.to_string();
    }
    store.load_settings().ms_client_id.trim().to_string()
}

/// Forces a full refresh and ownership check, for the "Verify" button.
pub async fn verify(store: &Store, acc: &Account) -> Result<McSession> {
    let secrets = load_secrets(store);
    let sec = secrets
        .get(&acc.id)
        .ok_or_else(|| anyhow!("This Microsoft account needs to sign in again."))?;
    let client_id = effective_client_id(store);
    let ms = refresh_ms(&client_id, &sec.refresh_token).await?;
    let session = minecraft_session(&ms.access).await?;
    save_login(store, &ms, &session)?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xsts_errors_are_explained() {
        assert!(xsts_error(2148916233).contains("no Xbox profile"));
        assert!(xsts_error(2148916238).contains("child"));
        assert!(xsts_error(1).contains("refused"));
    }

    #[test]
    fn needs_client_id() {
        assert!(require_client_id("  ").is_err());
        assert_eq!(require_client_id(" abc ").unwrap(), "abc");
    }
}
