use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Paths { root }
    }

    pub fn default_root() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SoloCraft")
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn instance_file(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join(".minecraft")
    }
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("natives")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }
    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn ensure_base(&self) -> std::io::Result<()> {
        for d in [
            self.instances(),
            self.libraries(),
            self.assets(),
            self.versions(),
            self.java_dir(),
            self.meta(),
        ] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }
}

pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}
