use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub main_class: String,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub downloads: Option<Downloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersionReq>,
    pub logging: Option<Logging>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionReq {
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    /// Maven repository base URL (loader profiles such as Fabric use this
    /// instead of a `downloads` block).
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub downloads: Option<LibDownloads>,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    Plain(String),
    Ruled {
        #[serde(default)]
        rules: Vec<Rule>,
        value: StringOrVec,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LogFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFile {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn rule_matches(rule: &Rule, features: &HashMap<String, bool>) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != current_os() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            // Only the 32-bit x86 rule is distinguishing for our targets.
            if arch == "x86" && std::env::consts::ARCH != "x86" {
                return false;
            }
        }
    }
    if let Some(wanted) = &rule.features {
        for (k, v) in wanted {
            let have = features.get(k).copied().unwrap_or(false);
            if have != *v {
                return false;
            }
        }
    }
    true
}

/// Mojang rule semantics: no rules means allowed; otherwise the last
/// matching rule decides.
pub fn rules_allow(rules: &[Rule], features: &HashMap<String, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if rule_matches(r, features) {
            allowed = r.action == "allow";
        }
    }
    allowed
}

pub fn flatten_args(args: &[ArgValue], features: &HashMap<String, bool>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            ArgValue::Plain(s) => out.push(s.clone()),
            ArgValue::Ruled { rules, value } => {
                if rules_allow(rules, features) {
                    match value {
                        StringOrVec::One(s) => out.push(s.clone()),
                        StringOrVec::Many(v) => out.extend(v.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// "group:artifact[:classifier]" without the version, used to let loader
/// libraries replace same-named vanilla ones.
fn lib_key(name: &str) -> String {
    let p: Vec<&str> = name.split(':').collect();
    match p.len() {
        0..=2 => name.to_string(),
        3 => format!("{}:{}", p[0], p[1]),
        _ => format!("{}:{}:{}", p[0], p[1], p[3]),
    }
}

/// Combines a loader profile (child) with the vanilla version it inherits
/// from (parent), the same way the official launcher does.
pub fn merge_loader(parent: McVersion, child: McVersion) -> McVersion {
    let child_keys: std::collections::HashSet<String> =
        child.libraries.iter().map(|l| lib_key(&l.name)).collect();
    let mut libraries = child.libraries.clone();
    libraries.extend(
        parent
            .libraries
            .iter()
            .filter(|l| !child_keys.contains(&lib_key(&l.name)))
            .cloned(),
    );
    let arguments = match (parent.arguments.clone(), child.arguments.clone()) {
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.into_iter().chain(c.game).collect(),
            jvm: p.jvm.into_iter().chain(c.jvm).collect(),
        }),
        (p, c) => c.or(p),
    };
    McVersion {
        id: parent.id.clone(),
        kind: child.kind.or(parent.kind),
        inherits_from: None,
        main_class: if child.main_class.is_empty() { parent.main_class } else { child.main_class },
        assets: parent.assets,
        asset_index: parent.asset_index,
        downloads: parent.downloads,
        libraries,
        arguments,
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        java_version: child.java_version.or(parent.java_version),
        logging: parent.logging,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_loader_profile() {
        let parent: McVersion = serde_json::from_str(r#"{"id":"1.21.4","mainClass":"net.minecraft.client.main.Main",
          "libraries":[{"name":"org.ow2.asm:asm:9.0"},{"name":"a:b:1"}],
          "arguments":{"game":["--username","x"],"jvm":["-cp","${classpath}"]},
          "javaVersion":{"majorVersion":21}}"#).unwrap();
        let child: McVersion = serde_json::from_str(r#"{"id":"fabric-loader-0.16.9-1.21.4","inheritsFrom":"1.21.4",
          "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
          "libraries":[{"name":"org.ow2.asm:asm:9.7","url":"https://maven.fabricmc.net/"},{"name":"net.fabricmc:fabric-loader:0.16.9","url":"https://maven.fabricmc.net/"}],
          "arguments":{"game":[],"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "]}}"#).unwrap();
        let m = merge_loader(parent, child);
        assert_eq!(m.id, "1.21.4");
        assert!(m.main_class.contains("Knot"));
        assert_eq!(m.libraries.len(), 3);
        assert!(m.libraries.iter().any(|l| l.name == "org.ow2.asm:asm:9.7"));
        assert!(!m.libraries.iter().any(|l| l.name == "org.ow2.asm:asm:9.0"));
        assert_eq!(m.java_version.unwrap().major_version, 21);
        assert_eq!(m.arguments.unwrap().jvm.len(), 3);
    }

    #[test]
    fn parses_modern_arguments() {
        let j = r#"{
          "id":"1.21.4","type":"release","mainClass":"net.minecraft.client.main.Main",
          "assets":"19","assetIndex":{"id":"19","sha1":"x","size":1,"url":"http://a"},
          "downloads":{"client":{"sha1":"a","size":2,"url":"http://c"}},
          "javaVersion":{"component":"java-runtime-delta","majorVersion":21},
          "libraries":[{"name":"a:b:1","downloads":{"artifact":{"path":"a/b/1/b-1.jar","sha1":"s","size":3,"url":"http://l"}},
            "rules":[{"action":"allow","os":{"name":"windows"}}]}],
          "arguments":{"game":["--username","${auth_player_name}",
            {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]}],
            "jvm":["-cp","${classpath}"]}
        }"#;
        let v: McVersion = serde_json::from_str(j).unwrap();
        assert_eq!(v.java_version.unwrap().major_version, 21);
        let args = v.arguments.unwrap();
        let mut f = HashMap::new();
        let none = flatten_args(&args.game, &f);
        assert_eq!(none.len(), 2);
        f.insert("has_custom_resolution".to_string(), true);
        let some = flatten_args(&args.game, &f);
        assert_eq!(some.len(), 4);
    }

    #[test]
    fn os_rules_follow_last_match() {
        let allow_all = Rule { action: "allow".into(), os: None, features: None };
        let deny_other = Rule {
            action: "disallow".into(),
            os: Some(OsRule { name: Some(current_os().into()), arch: None, version: None }),
            features: None,
        };
        let f = HashMap::new();
        assert!(rules_allow(&[allow_all.clone()], &f));
        assert!(!rules_allow(&[allow_all, deny_other], &f));
    }
}
