use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: String,
    pub account_id: Option<String>,
    pub java_path: Option<String>,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub jvm_args: String,
    pub window_width: u32,
    pub window_height: u32,
    pub use_nvidia_gpu: bool,
    pub notes: String,
    pub created_at: i64,
    pub last_played: Option<i64>,
    pub play_time_secs: u64,
}

impl Default for Instance {
    fn default() -> Self {
        Instance {
            id: String::new(),
            name: String::new(),
            icon: "#2f6b45".into(),
            group: String::new(),
            mc_version: String::new(),
            loader: "vanilla".into(),
            loader_version: String::new(),
            account_id: None,
            java_path: None,
            min_ram_mb: 512,
            max_ram_mb: 4096,
            jvm_args: String::new(),
            window_width: 0,
            window_height: 0,
            use_nvidia_gpu: false,
            notes: String::new(),
            created_at: 0,
            last_played: None,
            play_time_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    pub accent: String,
    pub background_image: Option<String>,
    pub background_blur: u32,
    pub grid_density: String,
    pub lite_mode: bool,
    pub groups: Vec<String>,
    pub default_min_ram_mb: u32,
    pub default_max_ram_mb: u32,
    pub close_on_launch: bool,
    /// Azure application (client) ID used for Microsoft sign-in
    pub ms_client_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "frosted-glass".into(),
            accent: "#4ade80".into(),
            background_image: None,
            background_blur: 12,
            grid_density: "comfortable".into(),
            lite_mode: false,
            groups: vec![
                "Survival".into(),
                "Modpacks".into(),
                "PvP / Testing".into(),
            ],
            default_min_ram_mb: 512,
            default_max_ram_mb: 4096,
            close_on_launch: false,
            ms_client_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub vendor: String,
    pub arch: String,
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub task: String,
    pub stage: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub instance_id: String,
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub instance_id: String,
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}
