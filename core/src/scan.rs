//! "Scan this PC": finds instances that other launchers already made and imports them as copies.
//! Everything here only READS the other launcher's folders; the import copies files.

use crate::events::{emit_progress, EventSink};
use crate::models::Instance;
use crate::store::Store;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Found {
    /// Stable id: launcher + folder (used to spot "already imported").
    pub key: String,
    pub launcher: String,
    pub name: String,
    pub game_dir: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: String,
    pub mods: u32,
    pub worlds: u32,
    pub has_options: bool,
    pub has_servers: bool,
    pub has_packs: bool,
    pub already: bool,
    /// Shown when the instance cannot be imported (for example the version is unknown).
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportOpts {
    pub mods: bool,
    pub settings: bool,
    pub servers: bool,
    pub packs: bool,
    pub worlds: bool,
    pub screenshots: bool,
}

impl Default for ImportOpts {
    fn default() -> Self {
        ImportOpts { mods: true, settings: true, servers: true, packs: true, worlds: true, screenshots: false }
    }
}

// ---------- small helpers ----------

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

fn count_mods(game: &Path) -> u32 {
    std::fs::read_dir(game.join("mods"))
        .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar")).count() as u32)
        .unwrap_or(0)
}

fn count_worlds(game: &Path) -> u32 {
    std::fs::read_dir(game.join("saves"))
        .map(|rd| rd.flatten().filter(|e| e.path().join("level.dat").is_file()).count() as u32)
        .unwrap_or(0)
}

fn fill(f: &mut Found, game: &Path) {
    f.game_dir = game.to_string_lossy().to_string();
    f.mods = count_mods(game);
    f.worlds = count_worlds(game);
    f.has_options = game.join("options.txt").is_file();
    f.has_servers = game.join("servers.dat").is_file();
    f.has_packs = game.join("resourcepacks").is_dir() || game.join("shaderpacks").is_dir();
    f.key = format!("{}|{}|{}", f.launcher, f.game_dir, f.name);
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

fn subdirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn dir_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

fn is_mc_version(s: &str) -> bool {
    !s.is_empty() && s.split('.').count() >= 2 && s.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// "21.1.5" (NeoForge) -> "1.21.1"; "20.4.80" -> "1.20.4"; "21.0.3" -> "1.21".
fn neoforge_mc(v: &str) -> Option<String> {
    let mut it = v.split('.');
    let major: u32 = it.next()?.parse().ok()?;
    let minor: u32 = it.next()?.parse().ok()?;
    Some(if minor == 0 { format!("1.{major}") } else { format!("1.{major}.{minor}") })
}

/// Splits a version folder name used by the official launcher into (minecraft, loader, loader version).
pub fn parse_version_id(id: &str) -> Option<(String, String, String)> {
    if is_mc_version(id) {
        return Some((id.to_string(), "vanilla".into(), String::new()));
    }
    for (prefix, loader) in [("fabric-loader-", "fabric"), ("quilt-loader-", "quilt")] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let (lv, mc) = rest.rsplit_once('-')?;
            if is_mc_version(mc) {
                return Some((mc.to_string(), loader.into(), lv.to_string()));
            }
        }
    }
    if let Some(rest) = id.strip_prefix("neoforge-") {
        return Some((neoforge_mc(rest)?, "neoforge".into(), rest.to_string()));
    }
    if let Some((mc, rest)) = id.split_once("-forge") {
        if is_mc_version(mc) {
            return Some((mc.to_string(), "forge".into(), rest.trim_start_matches('-').to_string()));
        }
    }
    None
}

// ---------- detectors ----------

fn official(out: &mut Vec<Found>) {
    let mut roots = vec![];
    if let Some(a) = env_path("APPDATA") {
        roots.push(a.join(".minecraft"));
    }
    if let Some(h) = dirs::home_dir() {
        roots.push(h.join(".minecraft"));
    }
    roots.dedup();
    for root in roots.into_iter().filter(|r| r.is_dir()) {
        let versions = root.join("versions");
        let mut ids: Vec<String> = subdirs(&versions).iter().map(|p| dir_name(p)).collect();
        ids.sort();
        let latest_release = ids.iter().filter(|i| is_mc_version(i)).max_by_key(|i| i.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>()).cloned();
        let mut seen = std::collections::HashSet::new();
        if let Some(profiles) = read_json(&root.join("launcher_profiles.json")).and_then(|j| j.get("profiles").cloned()) {
            if let Some(map) = profiles.as_object() {
                for (_, p) in map {
                    let ty = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if ty == "latest-snapshot" {
                        continue;
                    }
                    let mut vid = p.get("lastVersionId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if vid.is_empty() || vid == "latest-release" {
                        vid = latest_release.clone().unwrap_or_default();
                    }
                    let Some((mc, loader, lv)) = parse_version_id(&vid) else { continue };
                    let pname = p.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                    let name = if pname.is_empty() { if ty == "latest-release" { "Latest release".to_string() } else { vid.clone() } } else { pname };
                    let game = p.get("gameDir").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| root.clone());
                    if !seen.insert((vid.clone(), game.clone())) {
                        continue;
                    }
                    let mut f = Found { launcher: "Official Minecraft Launcher".into(), name, mc_version: mc, loader, loader_version: lv, ..Default::default() };
                    fill(&mut f, &game);
                    out.push(f);
                }
            }
        }
    }
}

fn cfg_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn prism_family(out: &mut Vec<Found>) {
    // (launcher name, data folders)
    let mut homes: Vec<(&str, PathBuf)> = vec![];
    if let Some(a) = env_path("APPDATA") {
        homes.push(("Prism Launcher", a.join("PrismLauncher")));
        homes.push(("PolyMC", a.join("PolyMC")));
        homes.push(("MultiMC", a.join("MultiMC")));
    }
    if let Some(d) = dirs::data_dir() {
        homes.push(("Prism Launcher", d.join("PrismLauncher")));
        homes.push(("PolyMC", d.join("polymc")));
        homes.push(("MultiMC", d.join("multimc")));
    }
    if let Some(h) = dirs::home_dir() {
        homes.push(("Prism Launcher", h.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher")));
    }
    let mut seen = std::collections::HashSet::new();
    for (launcher, home) in homes {
        if !home.is_dir() || !seen.insert(home.clone()) {
            continue;
        }
        let mut inst_dir = home.join("instances");
        for cfg in ["prismlauncher.cfg", "polymc.cfg", "multimc.cfg"] {
            if let Ok(t) = std::fs::read_to_string(home.join(cfg)) {
                if let Some(d) = cfg_value(&t, "InstanceDir") {
                    let p = PathBuf::from(&d);
                    inst_dir = if p.is_absolute() { p } else { home.join(p) };
                }
            }
        }
        for dir in subdirs(&inst_dir) {
            let Some(pack) = read_json(&dir.join("mmc-pack.json")) else { continue };
            let comps = pack.get("components").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            let ver = |uid: &str| comps.iter().find(|c| c.get("uid").and_then(|u| u.as_str()) == Some(uid)).and_then(|c| c.get("version").and_then(|v| v.as_str())).map(|s| s.to_string());
            let Some(mc) = ver("net.minecraft") else { continue };
            let (loader, lv) = if let Some(v) = ver("net.fabricmc.fabric-loader") {
                ("fabric", v)
            } else if let Some(v) = ver("org.quiltmc.quilt-loader") {
                ("quilt", v)
            } else if let Some(v) = ver("net.neoforged") {
                ("neoforge", v)
            } else if let Some(v) = ver("net.minecraftforge") {
                ("forge", v)
            } else {
                ("vanilla", String::new())
            };
            let name = std::fs::read_to_string(dir.join("instance.cfg")).ok().and_then(|t| cfg_value(&t, "name")).unwrap_or_else(|| dir_name(&dir));
            let game = [dir.join("minecraft"), dir.join(".minecraft")].into_iter().find(|p| p.is_dir()).unwrap_or_else(|| dir.join("minecraft"));
            let mut f = Found { launcher: launcher.into(), name, mc_version: mc, loader: loader.into(), loader_version: lv, ..Default::default() };
            fill(&mut f, &game);
            out.push(f);
        }
    }
}

fn curseforge(out: &mut Vec<Found>) {
    let mut roots = vec![];
    if let Some(h) = dirs::home_dir() {
        roots.push(h.join("curseforge/minecraft/Instances"));
    }
    if let Some(d) = dirs::document_dir() {
        roots.push(d.join("curseforge/minecraft/Instances"));
    }
    roots.dedup();
    for root in roots {
        for dir in subdirs(&root) {
            let Some(j) = read_json(&dir.join("minecraftinstance.json")) else { continue };
            let mc = j.get("gameVersion").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if mc.is_empty() {
                continue;
            }
            let base = j.get("baseModLoader").and_then(|b| b.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_lowercase();
            let mut parts = base.split('-');
            let (loader, lv) = match parts.next().unwrap_or("") {
                "fabric" => ("fabric", parts.next().unwrap_or("").to_string()),
                "quilt" => ("quilt", parts.next().unwrap_or("").to_string()),
                "neoforge" => ("neoforge", parts.next().unwrap_or("").to_string()),
                "forge" => ("forge", parts.next().unwrap_or("").to_string()),
                _ => ("vanilla", String::new()),
            };
            let name = j.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| dir_name(&dir));
            let mut f = Found { launcher: "CurseForge app".into(), name, mc_version: mc, loader: loader.into(), loader_version: lv, ..Default::default() };
            fill(&mut f, &dir);
            out.push(f);
        }
    }
}

/// Modrinth App keeps its profile list in a database, so the version is read from the profile's own files.
fn modrinth_app(out: &mut Vec<Found>) {
    let mut roots = vec![];
    if let Some(a) = env_path("APPDATA") {
        roots.push(a.join("ModrinthApp/profiles"));
    }
    if let Some(d) = dirs::data_dir() {
        roots.push(d.join("ModrinthApp/profiles"));
    }
    roots.dedup();
    for root in roots {
        for dir in subdirs(&root) {
            let name = dir_name(&dir);
            let mut f = Found { launcher: "Modrinth App".into(), name, loader: "vanilla".into(), ..Default::default() };
            let mut done = false;
            if let Some(j) = read_json(&dir.join("profile.json")) {
                let m = j.get("metadata").unwrap_or(&j);
                if let Some(v) = m.get("game_version").and_then(|v| v.as_str()) {
                    f.mc_version = v.to_string();
                    f.loader = m.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla").to_lowercase();
                    f.loader_version = m.get("loader_version").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    done = true;
                }
            }
            if !done {
                if let Ok(log) = std::fs::read_to_string(dir.join("logs/latest.log")) {
                    for l in log.lines().take(400) {
                        if let Some(i) = l.find("Loading Minecraft ") {
                            let rest = &l[i + "Loading Minecraft ".len()..];
                            let mut w = rest.split_whitespace();
                            let mc = w.next().unwrap_or("").to_string();
                            if is_mc_version(&mc) {
                                f.mc_version = mc;
                                if rest.contains("Fabric Loader") {
                                    f.loader = "fabric".into();
                                    f.loader_version = rest.split("Fabric Loader").nth(1).and_then(|s| s.split_whitespace().next()).unwrap_or("").to_string();
                                } else if rest.contains("Quilt Loader") {
                                    f.loader = "quilt".into();
                                    f.loader_version = rest.split("Quilt Loader").nth(1).and_then(|s| s.split_whitespace().next()).unwrap_or("").to_string();
                                }
                                done = true;
                                break;
                            }
                        }
                    }
                }
            }
            fill(&mut f, &dir);
            if !done {
                f.mc_version.clear();
                f.note = "Version unknown: start it once in Modrinth App, then scan again.".into();
            }
            out.push(f);
        }
    }
}

/// Looks in the known folders of other launchers (a few seconds at most; no full disk search).
pub fn scan(store: &Store) -> Vec<Found> {
    let mut out = vec![];
    official(&mut out);
    prism_family(&mut out);
    curseforge(&mut out);
    modrinth_app(&mut out);
    let mine = store.paths.instances();
    // do not offer SoloCraft's own folders, and mark what was imported before
    out.retain(|f| !Path::new(&f.game_dir).starts_with(&mine));
    let existing = store.list_instances();
    for f in &mut out {
        f.already = existing.iter().any(|i| i.notes.contains(&f.key));
    }
    out
}

// ---------- import ----------

fn copy_dir(src: &Path, dst: &Path, skipped: &mut u32) {
    if std::fs::create_dir_all(dst).is_err() {
        *skipped += 1;
        return;
    }
    let Ok(rd) = std::fs::read_dir(src) else { return };
    for e in rd.flatten() {
        let from = e.path();
        let to = dst.join(e.file_name());
        let ty = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ty.is_symlink() {
            continue;
        }
        if ty.is_dir() {
            copy_dir(&from, &to, skipped);
        } else if std::fs::copy(&from, &to).is_err() {
            *skipped += 1; // locked or unreadable: leave it out rather than stop
        }
    }
}

fn copy_item(src_game: &Path, dst_game: &Path, rel: &str, skipped: &mut u32) {
    let from = src_game.join(rel);
    let to = dst_game.join(rel);
    if from.is_dir() {
        copy_dir(&from, &to, skipped);
    } else if from.is_file() {
        if let Some(p) = to.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if std::fs::copy(&from, &to).is_err() {
            *skipped += 1;
        }
    }
}

/// Creates a SoloCraft instance from what another launcher has and copies the chosen parts.
pub fn import_found(store: &Store, f: &Found, o: &ImportOpts, sink: &dyn EventSink, task: &str) -> Result<Instance> {
    if f.mc_version.is_empty() {
        return Err(anyhow!("{}", if f.note.is_empty() { "This instance has no Minecraft version" } else { &f.note }));
    }
    let src = PathBuf::from(&f.game_dir);
    if !src.is_dir() {
        return Err(anyhow!("The folder {} no longer exists", src.display()));
    }
    let mut settings = store.load_settings();
    if !settings.groups.iter().any(|g| g == "Imported") {
        settings.groups.push("Imported".into());
        store.save_settings(&settings)?;
    }
    let mut inst = store.create_instance(&f.name, &f.mc_version, "Imported")?;
    inst.loader = f.loader.clone();
    inst.loader_version = f.loader_version.clone();
    inst.notes = format!("Imported from {}: {}", f.launcher, f.key);
    store.save_instance(&inst)?;

    let dst = store.paths.game_dir(&inst.id);
    std::fs::create_dir_all(&dst)?;
    if dst.starts_with(&src) {
        return Err(anyhow!("The SoloCraft folder is inside the source folder"));
    }
    let mut items: Vec<&str> = vec![];
    if o.mods {
        items.extend(["mods", "config"]);
    }
    if o.settings {
        items.extend(["options.txt", "optionsof.txt", "optionsshaders.txt"]);
    }
    if o.servers {
        items.push("servers.dat");
    }
    if o.packs {
        items.extend(["resourcepacks", "shaderpacks"]);
    }
    if o.worlds {
        items.push("saves");
    }
    if o.screenshots {
        items.push("screenshots");
    }
    let mut skipped = 0u32;
    let total = items.len() as u64;
    for (i, rel) in items.iter().enumerate() {
        emit_progress(sink, task, &format!("Copying {rel}"), i as u64, total);
        copy_item(&src, &dst, rel, &mut skipped);
    }
    emit_progress(sink, task, "Done", total, total);
    if skipped > 0 {
        inst.notes.push_str(&format!(" ({skipped} files could not be copied, they may have been in use)"));
        store.save_instance(&inst)?;
    }
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ids() {
        assert_eq!(parse_version_id("1.21.11"), Some(("1.21.11".into(), "vanilla".into(), "".into())));
        assert_eq!(parse_version_id("fabric-loader-0.20.1-1.21.11"), Some(("1.21.11".into(), "fabric".into(), "0.20.1".into())));
        assert_eq!(parse_version_id("quilt-loader-0.26.0-1.21.1"), Some(("1.21.1".into(), "quilt".into(), "0.26.0".into())));
        assert_eq!(parse_version_id("1.20.1-forge-47.2.0"), Some(("1.20.1".into(), "forge".into(), "47.2.0".into())));
        assert_eq!(parse_version_id("neoforge-21.1.5"), Some(("1.21.1".into(), "neoforge".into(), "21.1.5".into())));
        assert_eq!(parse_version_id("neoforge-21.0.3"), Some(("1.21".into(), "neoforge".into(), "21.0.3".into())));
        assert_eq!(parse_version_id("OptiFine_1.8"), None);
        assert_eq!(parse_version_id("24w10a"), None);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sc-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn import_copies_only_what_was_chosen() {
        let src = tmp("src");
        std::fs::create_dir_all(src.join("mods")).unwrap();
        std::fs::create_dir_all(src.join("saves/World1")).unwrap();
        std::fs::write(src.join("mods/a.jar"), "x").unwrap();
        std::fs::write(src.join("options.txt"), "key_key.attack:key.mouse.left\n").unwrap();
        std::fs::write(src.join("saves/World1/level.dat"), "x").unwrap();
        std::fs::write(src.join("servers.dat"), "x").unwrap();
        let store = Store::new(crate::paths::Paths::new(tmp("root")));
        store.paths.ensure_base().unwrap();
        let mut f = Found { launcher: "Test".into(), name: "My Pack".into(), mc_version: "1.21.4".into(), loader: "fabric".into(), loader_version: "0.16.0".into(), ..Default::default() };
        fill(&mut f, &src);
        assert_eq!((f.mods, f.worlds, f.has_options, f.has_servers), (1, 1, true, true));
        struct Null;
        impl EventSink for Null {
            fn log(&self, _: crate::models::LogLine) {}
            fn progress(&self, _: crate::models::Progress) {}
            fn state(&self, _: crate::models::GameState) {}
        }
        let inst = import_found(&store, &f, &ImportOpts::default(), &Null, "scan").unwrap();
        let g = store.paths.game_dir(&inst.id);
        assert!(g.join("mods/a.jar").is_file());
        assert!(g.join("options.txt").is_file());
        assert!(g.join("servers.dat").is_file());
        assert!(g.join("saves/World1/level.dat").is_file(), "worlds are copied by default");
        assert_eq!(inst.loader, "fabric");
        assert_eq!(inst.group, "Imported");
        assert!(inst.notes.contains(&f.key));
        assert!(src.join("options.txt").is_file(), "the original is untouched");
    }

    #[test]
    fn prism_instance_is_found() {
        let home = tmp("prism");
        let inst = home.join("instances/Cool");
        std::fs::create_dir_all(inst.join(".minecraft/mods")).unwrap();
        std::fs::write(inst.join("instance.cfg"), "name=Cool Pack\n").unwrap();
        std::fs::write(inst.join("mmc-pack.json"), r#"{"components":[{"uid":"net.minecraft","version":"1.21.4"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.9"}]}"#).unwrap();
        std::env::set_var("APPDATA", &home);
        std::fs::rename(home.join("instances"), {
            std::fs::create_dir_all(home.join("PrismLauncher")).unwrap();
            home.join("PrismLauncher/instances")
        })
        .unwrap();
        let mut out = vec![];
        prism_family(&mut out);
        let f = out.iter().find(|f| f.name == "Cool Pack").expect("found");
        assert_eq!((f.mc_version.as_str(), f.loader.as_str(), f.loader_version.as_str()), ("1.21.4", "fabric", "0.16.9"));
    }
}
