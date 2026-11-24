use crate::install::InstallPlan;
use crate::mcjson::{flatten_args, McVersion};
use crate::models::{Account, Instance};
use crate::paths::Paths;
use std::collections::HashMap;
use std::path::Path;

pub struct Launch {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Splits a user-typed JVM argument string, honouring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    for c in s.chars() {
        match c {
            '"' => in_q = !in_q,
            c if c.is_whitespace() && !in_q => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

pub fn build(
    paths: &Paths,
    inst: &Instance,
    v: &McVersion,
    plan: &InstallPlan,
    acc: &Account,
    access_token: Option<&str>,
    natives_dir: &Path,
    game_dir: &Path,
) -> Launch {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = plan
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);
    let custom_res = inst.window_width > 0 && inst.window_height > 0;

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", acc.username.clone());
    vars.insert("version_name", v.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", paths.assets().to_string_lossy().to_string());
    vars.insert("assets_index_name", plan.asset_index_id.clone());
    vars.insert("auth_uuid", acc.uuid.clone());
    vars.insert("auth_access_token", access_token.unwrap_or("0").to_string());
    vars.insert("auth_session", access_token.unwrap_or("0").to_string());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("user_type", if access_token.is_some() { "msa" } else { "legacy" }.into());
    vars.insert("user_properties", "{}".into());
    vars.insert("profile_properties", "{}".into());
    vars.insert("version_type", v.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("launcher_name", "SoloCraft".into());
    vars.insert("launcher_version", "1.0".into());
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("classpath", classpath);
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", paths.libraries().to_string_lossy().to_string());
    vars.insert("game_assets", plan.game_assets.to_string_lossy().to_string());
    vars.insert("resolution_width", inst.window_width.to_string());
    vars.insert("resolution_height", inst.window_height.to_string());

    let mut features: HashMap<String, bool> = HashMap::new();
    features.insert("has_custom_resolution".into(), custom_res);

    let (min, max) = if inst.min_ram_mb > inst.max_ram_mb {
        (inst.max_ram_mb, inst.max_ram_mb)
    } else {
        (inst.min_ram_mb, inst.max_ram_mb)
    };

    let mut args: Vec<String> = vec![format!("-Xms{min}M"), format!("-Xmx{max}M")];
    args.extend(split_args(&inst.jvm_args));
    if let Some(l) = &plan.logging_arg {
        args.push(l.clone());
    }

    let jvm_raw: Vec<String> = match v.arguments.as_ref() {
        Some(a) if !a.jvm.is_empty() => flatten_args(&a.jvm, &features),
        _ => vec![
            "-Djava.library.path=${natives_directory}".into(),
            "-cp".into(),
            "${classpath}".into(),
        ],
    };
    let jvm: Vec<String> = jvm_raw.iter().map(|s| substitute(s, &vars)).collect();
    let has_cp = jvm.iter().any(|a| a == "-cp" || a == "-classpath");
    args.extend(jvm);
    if !has_cp {
        args.push("-cp".into());
        args.push(vars["classpath"].clone());
    }

    args.push(v.main_class.clone());

    let game_raw: Vec<String> = if let Some(a) = v.arguments.as_ref() {
        flatten_args(&a.game, &features)
    } else if let Some(m) = &v.minecraft_arguments {
        let mut g: Vec<String> = m.split_whitespace().map(|s| s.to_string()).collect();
        if custom_res {
            g.extend(["--width".into(), "${resolution_width}".into()]);
            g.extend(["--height".into(), "${resolution_height}".into()]);
        }
        g
    } else {
        Vec::new()
    };
    args.extend(game_raw.iter().map(|s| substitute(s, &vars)));

    let mut env: Vec<(String, String)> = Vec::new();
    if inst.use_nvidia_gpu && cfg!(target_os = "linux") {
        env.push(("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into()));
        env.push(("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into()));
        env.push(("__VK_LAYER_NV_optimus".into(), "NVIDIA_only".into()));
    }
    Launch { args, env }
}

/// Windows: ask the OS to run this Java on the high-performance GPU.
#[cfg(windows)]
pub fn set_gpu_preference(exe: &str) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey("Software\\Microsoft\\DirectX\\UserGpuPreferences") {
        let _ = key.set_value(exe, &"GpuPreference=2;");
    }
}

#[cfg(not(windows))]
pub fn set_gpu_preference(_exe: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcjson::McVersion;
    use std::path::PathBuf;

    #[test]
    fn splits_quoted_args() {
        assert_eq!(
            split_args(r#"-Dfoo="a b" -XX:+UseG1GC  -Dx=1"#),
            vec!["-Dfoo=a b", "-XX:+UseG1GC", "-Dx=1"]
        );
    }

    #[test]
    fn builds_modern_command() {
        let j = r#"{"id":"1.21.4","type":"release","mainClass":"net.minecraft.client.main.Main",
          "arguments":{"game":["--username","${auth_player_name}","--uuid","${auth_uuid}"],
          "jvm":["-Djava.library.path=${natives_directory}","-cp","${classpath}"]}}"#;
        let v: McVersion = serde_json::from_str(j).unwrap();
        let plan = InstallPlan {
            classpath: vec![PathBuf::from("/a.jar"), PathBuf::from("/b.jar")],
            natives: vec![],
            asset_index_id: "19".into(),
            game_assets: PathBuf::from("/assets"),
            logging_arg: None,
        };
        let acc = Account {
            id: "x".into(),
            username: "Steve_Pi".into(),
            uuid: "abc".into(),
            kind: "offline".into(),
        };
        let inst = Instance { min_ram_mb: 512, max_ram_mb: 2048, ..Default::default() };
        let paths = Paths::new(PathBuf::from("/root"));
        let l = build(&paths, &inst, &v, &plan, &acc, None, Path::new("/n"), Path::new("/g"));
        assert_eq!(l.args[0], "-Xms512M");
        assert_eq!(l.args[1], "-Xmx2048M");
        assert!(l.args.contains(&"-Djava.library.path=/n".to_string()));
        let main_pos = l.args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        assert_eq!(l.args[main_pos + 1], "--username");
        assert_eq!(l.args[main_pos + 2], "Steve_Pi");
        assert_eq!(l.args[main_pos + 4], "abc");
    }
}
