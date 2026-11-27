//! Modrinth: search, one-click mod install with dependencies, FPS stack,
//! mod management (enable / disable / remove / update) and .mrpack modpacks.
//!
//! Every file that is downloaded is verified against the SHA-512 (or SHA-1)
//! that Modrinth publishes, written to a `.part` file first and renamed only
//! when it matches. Modpack paths are validated so a pack can never write
//! outside its own instance folder.

use crate::download::{client, download_file, download_many, file_sha1, DlTask};
use crate::events::{emit_progress, EventSink};
use crate::models::Instance;
use crate::paths::Paths;
use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

fn api() -> String {
    std::env::var("MODRINTH_API").unwrap_or_else(|_| "https://api.modrinth.com/v2".into())
}

/// Mods for these loaders are accepted by the given instance loader.
pub fn loader_filter(loader: &str) -> Vec<&'static str> {
    match loader {
        "quilt" => vec!["quilt", "fabric"],
        "fabric" => vec!["fabric"],
        "forge" => vec!["forge"],
        "neoforge" => vec!["neoforge"],
        _ => vec![],
    }
}

/// Mods that raise FPS without changing gameplay. Installed best effort:
/// anything without a build for the instance's Minecraft version is skipped.
// Cull Leaves was dropped: it patches renderer internals and breaks on new versions.
pub const FPS_STACK: [&str; 9] = [
    "sodium",
    "sodium-extra",
    "lithium",
    "ferrite-core",
    "modernfix",
    "immediatelyfast",
    "entityculling",
    "moreculling",
    "dynamic-fps",
];

// ---------- API types (serialize camelCase for the UI, accept Modrinth's snake_case) ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchHit {
    #[serde(alias = "project_id")]
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(alias = "icon_url")]
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub author: String,
    pub categories: Vec<String>,
    #[serde(alias = "project_type")]
    pub project_type: String,
    #[serde(alias = "date_modified")]
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    #[serde(alias = "total_hits")]
    pub total_hits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    #[serde(alias = "icon_url")]
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub followers: u64,
    pub categories: Vec<String>,
    #[serde(alias = "game_versions")]
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    #[serde(alias = "source_url")]
    pub source_url: Option<String>,
    #[serde(alias = "issues_url")]
    pub issues_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Hashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Dependency {
    #[serde(alias = "version_id")]
    pub version_id: Option<String>,
    #[serde(alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(alias = "dependency_type")]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Version {
    pub id: String,
    #[serde(alias = "project_id")]
    pub project_id: String,
    pub name: String,
    #[serde(alias = "version_number")]
    pub version_number: String,
    #[serde(alias = "version_type")]
    pub version_type: String,
    #[serde(alias = "game_versions")]
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    #[serde(alias = "date_published")]
    pub date_published: String,
    pub downloads: u64,
    pub files: Vec<VFile>,
    pub dependencies: Vec<Dependency>,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMod {
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    pub is_dependency: bool,
}

// ---------- HTTP ----------

async fn get<T: serde::de::DeserializeOwned>(path: &str, query: &[(&str, String)]) -> Result<T> {
    let url = format!("{}{}", api(), path);
    let r = client().get(&url).query(query).send().await.with_context(|| format!("reaching Modrinth ({path})"))?;
    if !r.status().is_success() {
        return Err(anyhow!("Modrinth returned {} for {}", r.status(), path));
    }
    Ok(r.json::<T>().await?)
}

async fn post<T: serde::de::DeserializeOwned>(path: &str, body: &serde_json::Value) -> Result<T> {
    let url = format!("{}{}", api(), path);
    let r = client().post(&url).json(body).send().await.with_context(|| format!("reaching Modrinth ({path})"))?;
    if !r.status().is_success() {
        return Err(anyhow!("Modrinth returned {} for {}", r.status(), path));
    }
    Ok(r.json::<T>().await?)
}

pub struct SearchQuery {
    pub query: String,
    /// "mod" or "modpack"
    pub project_type: String,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub categories: Vec<String>,
    /// relevance | downloads | follows | newest | updated
    pub sort: String,
    pub offset: u32,
    pub limit: u32,
}

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", q.project_type)]];
    if let Some(v) = q.mc_version.as_ref().filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{v}")]);
    }
    if let Some(l) = q.loader.as_ref().filter(|l| !l.is_empty() && *l != "vanilla") {
        let ors: Vec<String> = loader_filter(l).iter().map(|x| format!("categories:{x}")).collect();
        if !ors.is_empty() {
            facets.push(ors);
        }
    }
    for c in &q.categories {
        facets.push(vec![format!("categories:{c}")]);
    }
    let index = match q.sort.as_str() {
        "downloads" | "follows" | "newest" | "updated" => q.sort.clone(),
        _ => "relevance".into(),
    };
    get(
        "/search",
        &[
            ("query", q.query.clone()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index),
            ("offset", q.offset.to_string()),
            ("limit", q.limit.clamp(1, 50).to_string()),
        ],
    )
    .await
}

pub async fn get_project(id: &str) -> Result<Project> {
    get(&format!("/project/{id}"), &[]).await
}

pub async fn list_versions(
    project: &str,
    loaders: &[&str],
    mc: Option<&str>,
) -> Result<Vec<Version>> {
    let mut q: Vec<(&str, String)> = vec![];
    if !loaders.is_empty() {
        q.push(("loaders", serde_json::to_string(loaders)?));
    }
    if let Some(m) = mc.filter(|m| !m.is_empty()) {
        q.push(("game_versions", serde_json::to_string(&[m])?));
    }
    get(&format!("/project/{project}/version"), &q).await
}

async fn get_version(id: &str) -> Result<Version> {
    get(&format!("/version/{id}"), &[]).await
}

async fn get_projects(ids: &[String]) -> HashMap<String, Project> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let r: Result<Vec<Project>> = get("/projects", &[("ids", serde_json::to_string(ids).unwrap_or_default())]).await;
    r.map(|v| v.into_iter().map(|p| (p.id.clone(), p)).collect()).unwrap_or_default()
}

/// Newest release, else beta, else alpha. Modrinth lists newest first.
fn pick_best(versions: &[Version]) -> Option<&Version> {
    for t in ["release", "beta", "alpha"] {
        if let Some(v) = versions.iter().find(|v| v.version_type == t) {
            return Some(v);
        }
    }
    versions.first()
}

// ---------- installed-mod records ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ModRecord {
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub title: String,
    pub icon_url: Option<String>,
    /// false when the mod was only installed because another mod needs it
    pub explicit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ModManifest {
    /// keyed by SHA-1 of the jar
    mods: HashMap<String, ModRecord>,
}

fn manifest_path(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_dir(id).join("mods.json")
}

fn load_manifest(paths: &Paths, id: &str) -> ModManifest {
    std::fs::read(manifest_path(paths, id))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_manifest(paths: &Paths, id: &str, m: &ModManifest) {
    if let Ok(b) = serde_json::to_vec_pretty(m) {
        let _ = std::fs::write(manifest_path(paths, id), b);
    }
}

fn mods_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.game_dir(id).join("mods")
}

/// A file name that is safe to create inside the mods folder.
fn safe_filename(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.contains('/') || n.contains('\\') || n == "." || n == ".." || n.contains(':') {
        return Err(anyhow!("Unsafe file name in download: {name}"));
    }
    Ok(n.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub filename: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    pub explicit: bool,
}

/// Lists the mods folder. Jars that Modrinth recognises by hash (including
/// ones the user dropped in by hand or that came with a modpack) get their
/// real name, icon and version; the result is remembered for next time.
pub async fn list_mods(paths: &Paths, inst_id: &str) -> Result<Vec<ModEntry>> {
    let dir = mods_dir(paths, inst_id);
    let scan_dir = dir.clone();
    let files: Vec<(String, bool, u64, String)> = tokio::task::spawn_blocking(move || {
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(&scan_dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let (enabled, is_mod) = if name.ends_with(".jar") {
                    (true, true)
                } else if name.ends_with(".jar.disabled") {
                    (false, true)
                } else {
                    (false, false)
                };
                if !is_mod || !e.path().is_file() {
                    continue;
                }
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                if let Ok(h) = file_sha1(&e.path()) {
                    out.push((name, enabled, size, h));
                }
            }
        }
        out
    })
    .await?;

    let mut manifest = load_manifest(paths, inst_id);
    let unknown: Vec<String> = files
        .iter()
        .filter(|f| !manifest.mods.contains_key(&f.3))
        .map(|f| f.3.clone())
        .collect();
    if !unknown.is_empty() {
        let found: Result<HashMap<String, Version>> = post(
            "/version_files",
            &serde_json::json!({"hashes": unknown, "algorithm": "sha1"}),
        )
        .await;
        if let Ok(found) = found {
            let pids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
            let projects = get_projects(&pids).await;
            for (hash, v) in found {
                let p = projects.get(&v.project_id);
                manifest.mods.insert(
                    hash,
                    ModRecord {
                        project_id: v.project_id.clone(),
                        version_id: v.id.clone(),
                        version_number: v.version_number.clone(),
                        title: p.map(|p| p.title.clone()).unwrap_or_default(),
                        icon_url: p.and_then(|p| p.icon_url.clone()),
                        explicit: true,
                    },
                );
            }
            save_manifest(paths, inst_id, &manifest);
        }
    }

    let mut out: Vec<ModEntry> = files
        .into_iter()
        .map(|(name, enabled, size, sha1)| {
            let rec = manifest.mods.get(&sha1);
            ModEntry {
                title: rec
                    .map(|r| r.title.clone())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| name.trim_end_matches(".disabled").trim_end_matches(".jar").to_string()),
                project_id: rec.map(|r| r.project_id.clone()),
                version_id: rec.map(|r| r.version_id.clone()),
                version_number: rec.map(|r| r.version_number.clone()),
                icon_url: rec.and_then(|r| r.icon_url.clone()),
                explicit: rec.map(|r| r.explicit).unwrap_or(true),
                filename: name,
                enabled,
                size,
                sha1,
            }
        })
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

pub fn set_mod_enabled(paths: &Paths, inst_id: &str, filename: &str, enabled: bool) -> Result<()> {
    let name = safe_filename(filename)?;
    let dir = mods_dir(paths, inst_id);
    let (from, to) = if enabled {
        (dir.join(&name), dir.join(name.trim_end_matches(".disabled")))
    } else {
        (dir.join(&name), dir.join(format!("{name}.disabled")))
    };
    if from == to {
        return Ok(());
    }
    std::fs::rename(&from, &to).with_context(|| format!("renaming {}", from.display()))?;
    Ok(())
}

pub fn remove_mod(paths: &Paths, inst_id: &str, filename: &str) -> Result<()> {
    let name = safe_filename(filename)?;
    let p = mods_dir(paths, inst_id).join(&name);
    if p.is_file() {
        std::fs::remove_file(&p)?;
    }
    Ok(())
}

// ---------- install with dependencies ----------

struct Resolved {
    planned: Vec<(PlannedMod, VFile)>,
}

async fn resolve(
    project_id: &str,
    mc: &str,
    loader: &str,
    already: &HashSet<String>,
) -> Result<Resolved> {
    let filter = loader_filter(loader);
    if filter.is_empty() {
        return Err(anyhow!("Mods need a mod loader. Create the instance with Fabric or Quilt first."));
    }
    let mut planned: Vec<(PlannedMod, VFile)> = vec![];
    let mut seen: HashSet<String> = already.clone();
    // (project id, optional pinned version id, is dependency)
    let mut queue: Vec<(String, Option<String>, bool)> = vec![(project_id.to_string(), None, false)];
    let mut wanted_ids: Vec<String> = vec![];
    while let Some((pid, pinned, is_dep)) = queue.pop() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        let version = if let Some(vid) = pinned {
            get_version(&vid).await?
        } else {
            let vs = list_versions(&pid, &filter, Some(mc)).await?;
            match pick_best(&vs) {
                Some(v) => v.clone(),
                None => {
                    if is_dep {
                        return Err(anyhow!(
                            "A required dependency ({pid}) has no build for Minecraft {mc} with {loader}"
                        ));
                    }
                    return Err(anyhow!("This mod has no version for Minecraft {mc} with {loader}"));
                }
            }
        };
        let file = version
            .primary_file()
            .ok_or_else(|| anyhow!("Version {} has no downloadable file", version.version_number))?
            .clone();
        safe_filename(&file.filename)?;
        for d in &version.dependencies {
            if d.dependency_type == "required" {
                if let Some(dp) = &d.project_id {
                    queue.push((dp.clone(), d.version_id.clone(), true));
                } else if let Some(dv) = &d.version_id {
                    let v = get_version(dv).await?;
                    queue.push((v.project_id.clone(), Some(dv.clone()), true));
                }
            }
        }
        wanted_ids.push(version.project_id.clone());
        planned.push((
            PlannedMod {
                project_id: version.project_id.clone(),
                title: String::new(),
                icon_url: None,
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                filename: file.filename.clone(),
                is_dependency: is_dep,
            },
            file,
        ));
    }
    let projects = get_projects(&wanted_ids).await;
    for (p, _) in planned.iter_mut() {
        if let Some(pr) = projects.get(&p.project_id) {
            p.title = pr.title.clone();
            p.icon_url = pr.icon_url.clone();
        }
        if p.title.is_empty() {
            p.title = p.filename.clone();
        }
    }
    Ok(Resolved { planned })
}

fn installed_project_ids(paths: &Paths, inst_id: &str) -> HashSet<String> {
    let dir = mods_dir(paths, inst_id);
    let m = load_manifest(paths, inst_id);
    let mut present: HashSet<String> = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let Ok(h) = file_sha1(&e.path()) {
                if let Some(r) = m.mods.get(&h) {
                    present.insert(r.project_id.clone());
                }
            }
        }
    }
    present
}

/// What would be installed for a mod (the mod plus every required dependency
/// that is not already present). Used for the confirmation list in the UI.
pub async fn plan_mod_install(paths: &Paths, inst: &Instance, project_id: &str) -> Result<Vec<PlannedMod>> {
    let have = installed_project_ids(paths, &inst.id);
    Ok(resolve(project_id, &inst.mc_version, &inst.loader, &have)
        .await?
        .planned
        .into_iter()
        .map(|(p, _)| p)
        .collect())
}

async fn install_planned(
    paths: &Paths,
    inst_id: &str,
    planned: Vec<(PlannedMod, VFile)>,
    sink: &dyn EventSink,
    task: &str,
) -> Result<Vec<PlannedMod>> {
    let dir = mods_dir(paths, inst_id);
    tokio::fs::create_dir_all(&dir).await?;
    let tasks: Vec<DlTask> = planned
        .iter()
        .map(|(_, f)| {
            DlTask::new(f.url.clone(), dir.join(&f.filename))
                .with_sha1(f.hashes.sha1.clone())
                .with_sha512(f.hashes.sha512.clone())
                .with_size(Some(f.size).filter(|s| *s > 0))
        })
        .collect();
    emit_progress(sink, task, "Downloading mods", 0, tasks.len() as u64);
    download_many(tasks, 6, |d, t| emit_progress(sink, task, "Downloading mods", d, t)).await?;
    let mut manifest = load_manifest(paths, inst_id);
    for (p, f) in &planned {
        let hash = match &f.hashes.sha1 {
            Some(h) => h.clone(),
            None => file_sha1(&dir.join(&f.filename))?,
        };
        manifest.mods.insert(
            hash,
            ModRecord {
                project_id: p.project_id.clone(),
                version_id: p.version_id.clone(),
                version_number: p.version_number.clone(),
                title: p.title.clone(),
                icon_url: p.icon_url.clone(),
                explicit: !p.is_dependency,
            },
        );
    }
    save_manifest(paths, inst_id, &manifest);
    Ok(planned.into_iter().map(|(p, _)| p).collect())
}

pub async fn install_mod(
    paths: &Paths,
    inst: &Instance,
    project_id: &str,
    sink: &dyn EventSink,
) -> Result<Vec<PlannedMod>> {
    let have = installed_project_ids(paths, &inst.id);
    let r = resolve(project_id, &inst.mc_version, &inst.loader, &have).await?;
    install_planned(paths, &inst.id, r.planned, sink, &inst.id).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsReport {
    pub installed: Vec<PlannedMod>,
    pub skipped: Vec<String>,
}

/// Installs the FPS stack, skipping members with no build for this version.
pub async fn install_fps_stack(
    paths: &Paths,
    inst: &Instance,
    sink: &dyn EventSink,
) -> Result<FpsReport> {
    if loader_filter(&inst.loader).is_empty() {
        return Err(anyhow!("The FPS pack needs Fabric or Quilt"));
    }
    let mut have = installed_project_ids(paths, &inst.id);
    let mut all: Vec<(PlannedMod, VFile)> = vec![];
    let mut skipped = vec![];
    for slug in FPS_STACK {
        match resolve(slug, &inst.mc_version, &inst.loader, &have).await {
            Ok(r) => {
                for (p, f) in r.planned {
                    have.insert(p.project_id.clone());
                    all.push((p, f));
                }
            }
            Err(_) => skipped.push(slug.to_string()),
        }
    }
    if all.is_empty() {
        return Ok(FpsReport { installed: vec![], skipped });
    }
    let installed = install_planned(paths, &inst.id, all, sink, &inst.id).await?;
    Ok(FpsReport { installed, skipped })
}

// ---------- updates ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdate {
    pub filename: String,
    pub title: String,
    pub project_id: String,
    pub from_version: String,
    pub to_version: String,
    pub to_version_id: String,
}

pub async fn check_updates(paths: &Paths, inst: &Instance) -> Result<Vec<ModUpdate>> {
    let mods = list_mods(paths, &inst.id).await?;
    let hashes: Vec<String> = mods.iter().filter(|m| m.project_id.is_some() && m.enabled).map(|m| m.sha1.clone()).collect();
    if hashes.is_empty() {
        return Ok(vec![]);
    }
    let latest: HashMap<String, Version> = post(
        "/version_files/update",
        &serde_json::json!({
            "hashes": hashes, "algorithm": "sha1",
            "loaders": loader_filter(&inst.loader), "game_versions": [inst.mc_version]
        }),
    )
    .await?;
    let mut out = vec![];
    for m in mods {
        if let Some(v) = latest.get(&m.sha1) {
            if Some(&v.id) != m.version_id.as_ref() && v.primary_file().is_some() {
                out.push(ModUpdate {
                    filename: m.filename.clone(),
                    title: m.title.clone(),
                    project_id: v.project_id.clone(),
                    from_version: m.version_number.clone().unwrap_or_default(),
                    to_version: v.version_number.clone(),
                    to_version_id: v.id.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// Replaces one mod with a newer version. The new file is downloaded and
/// verified first; the old jar is moved to `mods-backup` rather than deleted,
/// so an update can always be rolled back by hand.
pub async fn update_mod(
    paths: &Paths,
    inst: &Instance,
    filename: &str,
    to_version_id: &str,
    sink: &dyn EventSink,
) -> Result<()> {
    let old = safe_filename(filename)?;
    let v = get_version(to_version_id).await?;
    let f = v.primary_file().ok_or_else(|| anyhow!("That version has no file"))?.clone();
    let new_name = safe_filename(&f.filename)?;
    let dir = mods_dir(paths, &inst.id);
    emit_progress(sink, &inst.id, "Updating mod", 0, 1);
    download_file(
        &DlTask::new(f.url.clone(), dir.join(&new_name))
            .with_sha1(f.hashes.sha1.clone())
            .with_sha512(f.hashes.sha512.clone())
            .with_size(Some(f.size).filter(|s| *s > 0)),
    )
    .await?;
    if old != new_name {
        let backup = paths.instance_dir(&inst.id).join("mods-backup");
        std::fs::create_dir_all(&backup)?;
        let src = dir.join(&old);
        if src.is_file() {
            std::fs::rename(&src, backup.join(&old))?;
        }
    }
    let titles = get_projects(&[v.project_id.clone()]).await;
    let mut manifest = load_manifest(paths, &inst.id);
    let was_explicit = manifest
        .mods
        .values()
        .find(|r| r.project_id == v.project_id)
        .map(|r| r.explicit)
        .unwrap_or(true);
    manifest.mods.retain(|_, r| r.project_id != v.project_id);
    let hash = match &f.hashes.sha1 {
        Some(h) => h.clone(),
        None => file_sha1(&dir.join(&new_name))?,
    };
    manifest.mods.insert(
        hash,
        ModRecord {
            project_id: v.project_id.clone(),
            version_id: v.id.clone(),
            version_number: v.version_number.clone(),
            title: titles.get(&v.project_id).map(|p| p.title.clone()).unwrap_or_default(),
            icon_url: titles.get(&v.project_id).and_then(|p| p.icon_url.clone()),
            explicit: was_explicit,
        },
    );
    save_manifest(paths, &inst.id, &manifest);
    emit_progress(sink, &inst.id, "Updating mod", 1, 1);
    Ok(())
}

// ---------- modpacks (.mrpack) ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndex {
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    game: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<PackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    path: String,
    #[serde(default)]
    hashes: Hashes,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
    env: Option<PackEnv>,
}

#[derive(Deserialize)]
struct PackEnv {
    client: Option<String>,
}

/// Relative path that stays inside the game folder, or None.
pub fn safe_rel(p: &str) -> Option<PathBuf> {
    let norm = p.replace('\\', "/");
    if norm.is_empty() || norm.starts_with('/') || norm.contains(':') {
        return None;
    }
    let path = PathBuf::from(&norm);
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn host_allowed(url: &str) -> bool {
    // The mrpack spec limits download hosts; local test servers are allowed
    // only when MODRINTH_API points at one.
    const OK: [&str; 5] = [
        "https://cdn.modrinth.com/",
        "https://github.com/",
        "https://raw.githubusercontent.com/",
        "https://gitlab.com/",
        "https://edge.forgecdn.net/",
    ];
    OK.iter().any(|h| url.starts_with(h))
        || (std::env::var("MODRINTH_API").is_ok() && url.starts_with("http://127.0.0.1"))
}

fn extract_overrides(zip_path: &Path, game_dir: &Path) -> Result<usize> {
    let f = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut count = 0;
    // overrides first, then client-overrides so they win
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut e = zip.by_index(i)?;
            let name = e.name().to_string();
            let Some(rest) = name.strip_prefix(prefix) else { continue };
            if e.is_dir() || rest.is_empty() {
                continue;
            }
            let Some(rel) = safe_rel(rest) else { continue };
            let out = game_dir.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut e, &mut w)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Installs a local .mrpack as a new instance. On any failure the half-built
/// instance is removed again.
pub async fn install_mrpack_file(
    store: &Store,
    pack: &Path,
    name: Option<&str>,
    group: &str,
    sink: &dyn EventSink,
    task: &str,
) -> Result<Instance> {
    let pack_path = pack.to_path_buf();
    let index: PackIndex = tokio::task::spawn_blocking(move || -> Result<PackIndex> {
        let f = std::fs::File::open(&pack_path).context("opening the modpack file")?;
        let mut zip = zip::ZipArchive::new(f).context("the file is not a valid .mrpack (zip)")?;
        let mut entry = zip
            .by_name("modrinth.index.json")
            .map_err(|_| anyhow!("This is not a Modrinth modpack: modrinth.index.json is missing"))?;
        let mut s = String::new();
        entry.read_to_string(&mut s)?;
        Ok(serde_json::from_str(&s).context("modrinth.index.json is damaged")?)
    })
    .await??;

    if index.format_version != 1 || index.game != "minecraft" {
        return Err(anyhow!("Unsupported modpack format (format {} / game {})", index.format_version, index.game));
    }
    let mc = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| anyhow!("The modpack does not say which Minecraft version it needs"))?;
    let (loader, loader_version) = if let Some(v) = index.dependencies.get("fabric-loader") {
        ("fabric", v.clone())
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        ("quilt", v.clone())
    } else if let Some(v) = index.dependencies.get("neoforge") {
        ("neoforge", v.clone())
    } else if let Some(v) = index.dependencies.get("forge") {
        ("forge", v.clone())
    } else {
        ("vanilla", String::new())
    };

    let display = name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| if index.name.is_empty() { "Modpack".into() } else { index.name.clone() });
    let mut inst = store.create_instance(&display, &mc, group)?;
    inst.loader = loader.to_string();
    inst.loader_version = loader_version;
    store.save_instance(&inst)?;

    let res = fill_pack(store, &inst, &index, pack, sink, task).await;
    if let Err(e) = res {
        let _ = store.delete_instance(&inst.id);
        return Err(e);
    }
    Ok(inst)
}

async fn fill_pack(
    store: &Store,
    inst: &Instance,
    index: &PackIndex,
    pack: &Path,
    sink: &dyn EventSink,
    task_id: &str,
) -> Result<()> {
    let game_dir = store.paths.game_dir(&inst.id);
    let mut tasks = vec![];
    for f in &index.files {
        if f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            continue;
        }
        let rel = safe_rel(&f.path).ok_or_else(|| anyhow!("The modpack tries to write to an unsafe path: {}", f.path))?;
        let url = f
            .downloads
            .iter()
            .find(|u| host_allowed(u))
            .ok_or_else(|| anyhow!("No allowed download source for {}", f.path))?;
        if f.hashes.sha512.is_none() && f.hashes.sha1.is_none() {
            return Err(anyhow!("{} has no checksum, refusing to download it", f.path));
        }
        tasks.push(
            DlTask::new(url.clone(), game_dir.join(rel))
                .with_sha1(f.hashes.sha1.clone())
                .with_sha512(f.hashes.sha512.clone())
                .with_size(Some(f.file_size).filter(|s| *s > 0)),
        );
    }
    let task_id = task_id.to_string();
    emit_progress(sink, &task_id, "Downloading modpack files", 0, tasks.len() as u64);
    download_many(tasks, 12, |d, t| emit_progress(sink, &task_id, "Downloading modpack files", d, t)).await?;

    emit_progress(sink, &task_id, "Applying modpack overrides", 0, 0);
    let p = pack.to_path_buf();
    let g = game_dir.clone();
    tokio::task::spawn_blocking(move || extract_overrides(&p, &g)).await??;
    Ok(())
}

/// Downloads a modpack version from Modrinth and installs it.
pub async fn install_modpack(
    store: &Store,
    project_id: &str,
    version_id: Option<&str>,
    name: Option<&str>,
    group: &str,
    sink: &dyn EventSink,
) -> Result<Instance> {
    let version = match version_id.filter(|v| !v.is_empty()) {
        Some(v) => get_version(v).await?,
        None => {
            let vs = list_versions(project_id, &[], None).await?;
            pick_best(&vs).cloned().ok_or_else(|| anyhow!("This modpack has no versions"))?
        }
    };
    let file = version
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .or_else(|| version.primary_file())
        .ok_or_else(|| anyhow!("This modpack version has no file"))?
        .clone();
    let proj = get_project(project_id).await.ok();
    let tmp_dir = store.paths.root.join("downloads");
    tokio::fs::create_dir_all(&tmp_dir).await?;
    let dest = tmp_dir.join(safe_filename(&file.filename)?);
    emit_progress(sink, project_id, "Downloading modpack", 0, 0);
    download_file(
        &DlTask::new(file.url.clone(), dest.clone())
            .with_sha1(file.hashes.sha1.clone())
            .with_sha512(file.hashes.sha512.clone())
            .with_size(Some(file.size).filter(|s| *s > 0)),
    )
    .await?;
    let display = name
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| proj.as_ref().map(|p| p.title.clone()));
    let res = install_mrpack_file(store, &dest, display.as_deref(), group, sink, project_id).await;
    let _ = tokio::fs::remove_file(&dest).await;
    let mut inst = res?;
    if let Some(p) = proj {
        inst.notes = format!("Modpack: {} {}", p.title, version.version_number);
        store.save_instance(&inst)?;
    }
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths() {
        assert!(safe_rel("mods/sodium.jar").is_some());
        assert!(safe_rel("config\\a\\b.toml").is_some());
        assert!(safe_rel("../evil.jar").is_none());
        assert!(safe_rel("mods/../../evil.jar").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert!(safe_rel("C:/Windows/x.dll").is_none());
        assert!(safe_rel("").is_none());
    }

    #[test]
    fn filenames_are_checked() {
        assert!(safe_filename("sodium-0.6.jar").is_ok());
        assert!(safe_filename("../a.jar").is_err());
        assert!(safe_filename("a/b.jar").is_err());
    }

    #[test]
    fn picks_release_over_beta() {
        let v = |id: &str, t: &str| Version { id: id.into(), version_type: t.into(), ..Default::default() };
        let list = vec![v("b", "beta"), v("r", "release"), v("a", "alpha")];
        assert_eq!(pick_best(&list).unwrap().id, "r");
    }

    #[test]
    fn loaders_map() {
        assert_eq!(loader_filter("quilt"), vec!["quilt", "fabric"]);
        assert!(loader_filter("vanilla").is_empty());
    }
}
