use crate::models::{Account, AccountStore, Instance, Settings};
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use md5::{Digest, Md5};
use std::path::Path;

/// Minecraft's offline UUID: MD5 of "OfflinePlayer:<name>" with version 3 bits set.
pub fn offline_uuid(name: &str) -> String {
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut b: [u8; 16] = h.finalize().into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    hex::encode(b)
}

pub fn dashed_uuid(u: &str) -> String {
    if u.len() != 32 {
        return u.to_string();
    }
    format!("{}-{}-{}-{}-{}", &u[0..8], &u[8..12], &u[12..16], &u[16..20], &u[20..32])
}

fn slug(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "instance".to_string()
    } else {
        s
    }
}

#[derive(Clone)]
pub struct Store {
    pub paths: Paths,
}

impl Store {
    pub fn new(paths: Paths) -> Self {
        Store { paths }
    }

    pub fn load_settings(&self) -> Settings {
        std::fs::read(self.paths.settings_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        write_json(&self.paths.settings_file(), s)
    }

    pub fn list_instances(&self) -> Vec<Instance> {
        let mut out: Vec<Instance> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.paths.instances()) {
            for e in rd.flatten() {
                let f = e.path().join("instance.json");
                if let Ok(b) = std::fs::read(&f) {
                    if let Ok(i) = serde_json::from_slice::<Instance>(&b) {
                        out.push(i);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    pub fn get_instance(&self, id: &str) -> Result<Instance> {
        let b = std::fs::read(self.paths.instance_file(id))
            .map_err(|_| anyhow!("Instance {id} was not found"))?;
        Ok(serde_json::from_slice(&b)?)
    }

    pub fn save_instance(&self, inst: &Instance) -> Result<()> {
        std::fs::create_dir_all(self.paths.game_dir(&inst.id))?;
        write_json(&self.paths.instance_file(&inst.id), inst)
    }

    pub fn create_instance(&self, name: &str, mc_version: &str, group: &str) -> Result<Instance> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("Enter a name for the instance"));
        }
        if mc_version.is_empty() {
            return Err(anyhow!("Choose a Minecraft version"));
        }
        let settings = self.load_settings();
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.paths.instance_dir(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let colors = ["#2f6b45", "#6b4a2f", "#2f4a6b", "#5a2f6b", "#6b2f3f", "#2f6b66"];
        let color = colors[(self.list_instances().len()) % colors.len()];
        let inst = Instance {
            id,
            name: name.to_string(),
            icon: color.to_string(),
            group: group.to_string(),
            mc_version: mc_version.to_string(),
            min_ram_mb: settings.default_min_ram_mb,
            max_ram_mb: settings.default_max_ram_mb,
            created_at: chrono::Utc::now().timestamp(),
            ..Default::default()
        };
        self.save_instance(&inst)?;
        Ok(inst)
    }

    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let dir = self.paths.instance_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn load_accounts(&self) -> AccountStore {
        std::fs::read(self.paths.accounts_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_accounts(&self, a: &AccountStore) -> Result<()> {
        write_json(&self.paths.accounts_file(), a)
    }

    pub fn add_offline_account(&self, username: &str) -> Result<Account> {
        let username = username.trim();
        let ok = (3..=16).contains(&username.len())
            && username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !ok {
            return Err(anyhow!(
                "Usernames are 3 to 16 characters: letters, numbers and underscores"
            ));
        }
        let mut store = self.load_accounts();
        let uuid = offline_uuid(username);
        if store.accounts.iter().any(|a| a.id == uuid) {
            return Err(anyhow!("An account named {username} already exists"));
        }
        let acc = Account {
            id: uuid.clone(),
            username: username.to_string(),
            uuid,
            kind: "offline".into(),
        };
        store.accounts.push(acc.clone());
        if store.active.is_none() {
            store.active = Some(acc.id.clone());
        }
        self.save_accounts(&store)?;
        Ok(acc)
    }

    pub fn set_active_account(&self, id: &str) -> Result<()> {
        let mut store = self.load_accounts();
        if !store.accounts.iter().any(|a| a.id == id) {
            return Err(anyhow!("Account not found"));
        }
        store.active = Some(id.to_string());
        self.save_accounts(&store)
    }

    pub fn remove_account(&self, id: &str) -> Result<()> {
        let mut store = self.load_accounts();
        store.accounts.retain(|a| a.id != id);
        crate::msauth::forget(self, id);
        if store.active.as_deref() == Some(id) {
            store.active = store.accounts.first().map(|a| a.id.clone());
        }
        self.save_accounts(&store)
    }

    /// The account bound to the instance, else the active one, else the first.
    pub fn resolve_account(&self, inst: &Instance) -> Option<Account> {
        let store = self.load_accounts();
        if let Some(id) = &inst.account_id {
            if let Some(a) = store.accounts.iter().find(|a| &a.id == id) {
                return Some(a.clone());
            }
        }
        if let Some(id) = &store.active {
            if let Some(a) = store.accounts.iter().find(|a| &a.id == id) {
                return Some(a.clone());
            }
        }
        store.accounts.first().cloned()
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_v3_and_stable() {
        let a = offline_uuid("Steve_Pi");
        assert_eq!(a.len(), 32);
        assert_eq!(&a[12..13], "3");
        assert!(matches!(&a[16..17], "8" | "9" | "a" | "b"));
        assert_eq!(a, offline_uuid("Steve_Pi"));
        assert_ne!(a, offline_uuid("Steve_Pj"));
    }

    #[test]
    fn instance_crud_and_accounts() {
        let dir = std::env::temp_dir().join(format!("sc-test-{}", uuid::Uuid::new_v4()));
        let store = Store::new(Paths::new(dir.clone()));
        store.paths.ensure_base().unwrap();
        let i = store.create_instance("My World!", "1.21.4", "Survival").unwrap();
        assert_eq!(i.id, "my-world");
        let j = store.create_instance("My World!", "1.21.4", "").unwrap();
        assert_eq!(j.id, "my-world-2");
        assert_eq!(store.list_instances().len(), 2);
        assert!(store.create_instance("  ", "1.21.4", "").is_err());
        store.delete_instance(&i.id).unwrap();
        assert_eq!(store.list_instances().len(), 1);

        assert!(store.add_offline_account("ab").is_err());
        let a = store.add_offline_account("Steve_Pi").unwrap();
        assert!(store.add_offline_account("Steve_Pi").is_err());
        assert_eq!(store.resolve_account(&j).unwrap().id, a.id);
        std::fs::remove_dir_all(dir).ok();
    }
}
