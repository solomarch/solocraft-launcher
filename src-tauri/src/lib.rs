use solocraft_core::events::EventSink;
use solocraft_core::models::{
    AccountStore, GameState, Instance, JavaInstall, LogLine, Progress, Settings, VersionEntry,
};
use solocraft_core::models::LoaderVersion;
use solocraft_core::booster::{self, BoostReport, Plan};
use solocraft_core::modrinth::{self, FpsReport, ModEntry, ModUpdate, PlannedMod, Project, SearchQuery, SearchResult, Version};
use solocraft_core::msauth::{self, DeviceCode};
use solocraft_core::{java, loaders, versions, Engine, Paths};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

struct AppState {
    engine: Arc<Engine>,
    first_run: bool,
    storage_warning: Option<String>,
    ms_cancel: Arc<std::sync::atomic::AtomicBool>,
}

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn progress(&self, p: Progress) {
        let _ = self.0.emit("progress", p);
    }
    fn log(&self, l: LogLine) {
        let _ = self.0.emit("log", l);
    }
    fn state(&self, s: GameState) {
        let _ = self.0.emit("game-state", s);
    }
}

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

#[tauri::command]
fn list_instances(state: State<'_, AppState>) -> Vec<Instance> {
    state.engine.store.list_instances()
}

#[tauri::command(rename_all = "camelCase")]
fn create_instance(
    state: State<'_, AppState>,
    name: String,
    mc_version: String,
    group: String,
) -> Res<Instance> {
    state
        .engine
        .store
        .create_instance(&name, &mc_version, &group)
        .map_err(err)
}

#[tauri::command]
fn update_instance(state: State<'_, AppState>, instance: Instance) -> Res<()> {
    state.engine.store.save_instance(&instance).map_err(err)
}

#[tauri::command]
fn delete_instance(state: State<'_, AppState>, id: String) -> Res<()> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before deleting this instance".into());
    }
    state.engine.store.delete_instance(&id).map_err(err)
}

#[tauri::command]
async fn list_versions(state: State<'_, AppState>) -> Res<Vec<VersionEntry>> {
    let paths: Paths = state.engine.paths().clone();
    versions::list_versions(&paths).await.map_err(err)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.engine.store.load_settings()
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Res<()> {
    state.engine.store.save_settings(&settings).map_err(err)
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> AccountStore {
    state.engine.store.load_accounts()
}

#[tauri::command]
fn add_offline_account(state: State<'_, AppState>, username: String) -> Res<AccountStore> {
    state.engine.store.add_offline_account(&username).map_err(err)?;
    Ok(state.engine.store.load_accounts())
}

#[tauri::command]
fn set_active_account(state: State<'_, AppState>, id: String) -> Res<AccountStore> {
    state.engine.store.set_active_account(&id).map_err(err)?;
    Ok(state.engine.store.load_accounts())
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, id: String) -> Res<AccountStore> {
    state.engine.store.remove_account(&id).map_err(err)?;
    Ok(state.engine.store.load_accounts())
}

#[tauri::command]
async fn detect_java(state: State<'_, AppState>) -> Res<Vec<JavaInstall>> {
    let paths: Paths = state.engine.paths().clone();
    Ok(java::detect_installs(&paths).await)
}

#[tauri::command]
async fn install_java(app: AppHandle, state: State<'_, AppState>, major: u32) -> Res<JavaInstall> {
    let paths: Paths = state.engine.paths().clone();
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink(app));
    java::install_temurin(&paths, major, sink, "java")
        .await
        .map_err(err)
}

#[tauri::command]
fn launch_instance(app: AppHandle, state: State<'_, AppState>, id: String) {
    let engine = state.engine.clone();
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink(app));
    tauri::async_runtime::spawn(async move {
        engine.launch(&id, sink).await;
    });
}

#[tauri::command]
fn kill_instance(state: State<'_, AppState>, id: String) -> bool {
    state.engine.kill(&id)
}

#[tauri::command]
fn running_instances(state: State<'_, AppState>) -> Vec<String> {
    state.engine.running_ids()
}

#[tauri::command]
fn open_instance_folder(state: State<'_, AppState>, id: String) -> Res<()> {
    let dir = state.engine.paths().game_dir(&id);
    std::fs::create_dir_all(&dir).map_err(err)?;
    open::that(dir).map_err(err)
}

/// Opens a well-known folder inside the instance. Only names from this list are
/// accepted, so the UI can never point the command at an arbitrary path.
#[tauri::command(rename_all = "camelCase")]
fn open_instance_subfolder(state: State<'_, AppState>, id: String, kind: String) -> Res<()> {
    let sub = match kind.as_str() {
        "worlds" => "saves",
        "mods" => "mods",
        "resourcepacks" => "resourcepacks",
        "shaders" => "shaderpacks",
        "screenshots" => "screenshots",
        "config" => "config",
        "logs" => "logs",
        _ => return Err(format!("Unknown folder: {kind}")),
    };
    state.engine.store.get_instance(&id).map_err(err)?;
    let dir = state.engine.paths().game_dir(&id).join(sub);
    std::fs::create_dir_all(&dir).map_err(err)?;
    open::that(dir).map_err(err)
}

#[tauri::command]
fn open_data_folder(state: State<'_, AppState>) -> Res<()> {
    open::that(state.engine.paths().root.clone()).map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn list_loader_versions(loader: String, mc_version: String) -> Res<Vec<LoaderVersion>> {
    loaders::list_loader_versions(&loader, &mc_version).await.map_err(err)
}

/// Creates an instance with a mod loader. The loader version is resolved
/// (and checked against the Minecraft version) before anything is saved.
#[tauri::command(rename_all = "camelCase")]
async fn create_instance_with_loader(
    state: State<'_, AppState>,
    name: String,
    mc_version: String,
    group: String,
    loader: String,
    loader_version: String,
) -> Res<Instance> {
    let mut lv = loader_version;
    if loader != "vanilla" {
        if lv.trim().is_empty() {
            lv = loaders::latest_loader_version(&loader, &mc_version).await.map_err(err)?;
        } else {
            let list = loaders::list_loader_versions(&loader, &mc_version).await.map_err(err)?;
            if !list.iter().any(|v| v.version == lv) {
                return Err(format!("{loader} {lv} does not support Minecraft {mc_version}"));
            }
        }
    }
    let mut inst = state.engine.store.create_instance(&name, &mc_version, &group).map_err(err)?;
    if loader != "vanilla" {
        inst.loader = loader;
        inst.loader_version = lv;
        state.engine.store.save_instance(&inst).map_err(err)?;
    }
    Ok(inst)
}

#[tauri::command(rename_all = "camelCase")]
async fn change_loader(
    state: State<'_, AppState>,
    id: String,
    loader: String,
    loader_version: String,
) -> Res<Instance> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before changing its loader".into());
    }
    let mut inst = state.engine.store.get_instance(&id).map_err(err)?;
    let mut lv = loader_version;
    if loader != "vanilla" {
        let list = loaders::list_loader_versions(&loader, &inst.mc_version).await.map_err(err)?;
        if lv.trim().is_empty() || !list.iter().any(|v| v.version == lv) {
            lv = loaders::latest_loader_version(&loader, &inst.mc_version).await.map_err(err)?;
        }
    } else {
        lv = String::new();
    }
    inst.loader = loader;
    inst.loader_version = lv;
    state.engine.store.save_instance(&inst).map_err(err)?;
    Ok(inst)
}

#[tauri::command(rename_all = "camelCase")]
async fn modrinth_search(
    query: String,
    project_type: String,
    mc_version: Option<String>,
    loader: Option<String>,
    categories: Vec<String>,
    sort: String,
    offset: u32,
    limit: u32,
) -> Res<SearchResult> {
    modrinth::search(&SearchQuery { query, project_type, mc_version, loader, categories, sort, offset, limit })
        .await
        .map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn modrinth_project(project_id: String) -> Res<Project> {
    modrinth::get_project(&project_id).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn modrinth_versions(
    project_id: String,
    loaders: Vec<String>,
    mc_version: Option<String>,
) -> Res<Vec<Version>> {
    let l: Vec<&str> = loaders.iter().map(|s| s.as_str()).collect();
    modrinth::list_versions(&project_id, &l, mc_version.as_deref()).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    version_id: Option<String>,
    name: Option<String>,
    group: String,
) -> Res<Instance> {
    let sink = TauriSink(app);
    modrinth::install_modpack(&state.engine.store, &project_id, version_id.as_deref(), name.as_deref(), &group, &sink)
        .await
        .map_err(err)
}

/// Imports a .mrpack / .zip (Modrinth pack or Prism/MultiMC instance) from disk.
#[tauri::command(rename_all = "camelCase")]
async fn import_modpack_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
    group: String,
) -> Res<Instance> {
    let sink = TauriSink(app);
    solocraft_core::import::import_file(
        &state.engine.store,
        std::path::Path::new(&path),
        name.as_deref(),
        &group,
        &sink,
        "import",
    )
    .await
    .map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
fn client_mod_status(state: State<'_, AppState>, id: String) -> Res<solocraft_core::clientmod::Status> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    Ok(solocraft_core::clientmod::status(state.engine.paths(), &inst))
}

#[tauri::command(rename_all = "camelCase")]
async fn client_mod_install(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let sink = TauriSink(app);
    solocraft_core::clientmod::install(state.engine.paths(), &inst, &sink).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
fn client_mod_remove(state: State<'_, AppState>, id: String) -> Res<()> {
    solocraft_core::clientmod::remove(state.engine.paths(), &id).map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn list_mods(state: State<'_, AppState>, id: String) -> Res<Vec<ModEntry>> {
    modrinth::list_mods(state.engine.paths(), &id).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn plan_mod_install(state: State<'_, AppState>, id: String, project_id: String) -> Res<Vec<PlannedMod>> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    modrinth::plan_mod_install(state.engine.paths(), &inst, &project_id).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    project_id: String,
) -> Res<Vec<PlannedMod>> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let sink = TauriSink(app);
    modrinth::install_mod(state.engine.paths(), &inst, &project_id, &sink).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_fps_pack(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<FpsReport> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let sink = TauriSink(app);
    modrinth::install_fps_stack(state.engine.paths(), &inst, &sink).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn booster_plan(state: State<'_, AppState>, id: String, tier: Option<String>) -> Res<Plan> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let paths = state.engine.paths().clone();
    tokio::task::spawn_blocking(move || booster::plan_for(&paths, &inst, tier.as_deref())).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn booster_apply(app: AppHandle, state: State<'_, AppState>, id: String, tier: Option<String>) -> Res<BoostReport> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before using the FPS Booster".into());
    }
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let sink = TauriSink(app);
    booster::apply(state.engine.paths(), &state.engine.store, &inst, tier.as_deref(), &sink).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
fn booster_undo(state: State<'_, AppState>, id: String) -> Res<()> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before undoing the FPS Booster".into());
    }
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    booster::undo(state.engine.paths(), &state.engine.store, &inst).map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
fn set_mod_enabled(state: State<'_, AppState>, id: String, filename: String, enabled: bool) -> Res<()> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before changing mods".into());
    }
    modrinth::set_mod_enabled(state.engine.paths(), &id, &filename, enabled).map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
fn remove_mod(state: State<'_, AppState>, id: String, filename: String) -> Res<()> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before changing mods".into());
    }
    modrinth::remove_mod(state.engine.paths(), &id, &filename).map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn check_mod_updates(state: State<'_, AppState>, id: String) -> Res<Vec<ModUpdate>> {
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    modrinth::check_updates(state.engine.paths(), &inst).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn update_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    filename: String,
    to_version_id: String,
) -> Res<()> {
    if state.engine.is_running(&id) {
        return Err("Stop the game before changing mods".into());
    }
    let inst = state.engine.store.get_instance(&id).map_err(err)?;
    let sink = TauriSink(app);
    modrinth::update_mod(state.engine.paths(), &inst, &filename, &to_version_id, &sink).await.map_err(err)
}

#[tauri::command(rename_all = "camelCase")]
async fn ms_begin_login(state: State<'_, AppState>) -> Res<DeviceCode> {
    state.ms_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    let cid = msauth::effective_client_id(&state.engine.store);
    msauth::begin_device_login(&cid).await.map_err(err)
}

/// Waits for the code to be entered, then finishes the Xbox / Minecraft steps.
#[tauri::command(rename_all = "camelCase")]
async fn ms_finish_login(state: State<'_, AppState>, device: DeviceCode) -> Res<AccountStore> {
    let cid = msauth::effective_client_id(&state.engine.store);
    let tokens = msauth::poll_device_login(&cid, &device, &state.ms_cancel).await.map_err(err)?;
    let session = msauth::minecraft_session(&tokens.access).await.map_err(err)?;
    msauth::save_login(&state.engine.store, &tokens, &session).map_err(err)?;
    Ok(state.engine.store.load_accounts())
}

/// True when this build ships its own Microsoft app ID (users then see no setup).
#[tauri::command]
fn ms_has_builtin() -> bool {
    !msauth::builtin_client_id().is_empty()
}

#[tauri::command]
fn ms_cancel_login(state: State<'_, AppState>) {
    state.ms_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// "Check that this account really works": full refresh, Xbox, Minecraft, ownership.
#[tauri::command(rename_all = "camelCase")]
async fn ms_verify(state: State<'_, AppState>, id: String) -> Res<String> {
    let acc = state
        .engine
        .store
        .load_accounts()
        .accounts
        .into_iter()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;
    if acc.kind != "microsoft" {
        return Err("Only Microsoft accounts can be verified".into());
    }
    let s = msauth::verify(&state.engine.store, &acc).await.map_err(err)?;
    Ok(format!("Signed in as {}. Minecraft: Java Edition is owned and the token works.", s.username))
}

/// Opens a Microsoft sign-in page. Only Microsoft addresses are accepted.
#[tauri::command]
fn open_login_page(url: String) -> Res<()> {
    let ok = ["https://www.microsoft.com/", "https://microsoft.com/", "https://login.live.com/", "https://login.microsoftonline.com/", "https://aka.ms/"]
        .iter()
        .any(|p| url.starts_with(p));
    if !ok {
        return Err("Refusing to open a non-Microsoft address".into());
    }
    open::that(url).map_err(err)
}

#[tauri::command]
async fn check_update() -> Res<solocraft_core::update::UpdateInfo> {
    solocraft_core::update::check(env!("CARGO_PKG_VERSION")).await.map_err(err)
}

#[tauri::command]
fn open_update_page(url: String) -> Res<()> {
    if !solocraft_core::update::url_allowed(&url) {
        return Err("Refusing to open an address outside the SoloCraft repository".into());
    }
    open::that(url).map_err(err)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    current: String,
    default_path: String,
    first_run: bool,
    warning: Option<String>,
    drives: Vec<solocraft_core::storage::DriveInfo>,
}

#[tauri::command(rename_all = "camelCase")]
async fn storage_info(state: State<'_, AppState>) -> Res<StorageInfo> {
    let drives = tokio::task::spawn_blocking(solocraft_core::storage::list_drives).await.map_err(err)?;
    Ok(StorageInfo {
        current: state.engine.paths().root.to_string_lossy().to_string(),
        default_path: solocraft_core::storage::default_root().to_string_lossy().to_string(),
        first_run: state.first_run,
        warning: state.storage_warning.clone(),
        drives,
    })
}

#[tauri::command(rename_all = "camelCase")]
async fn storage_usage(state: State<'_, AppState>) -> Res<Vec<solocraft_core::storage::UsageItem>> {
    let root = state.engine.paths().root.clone();
    tokio::task::spawn_blocking(move || solocraft_core::storage::usage(&root)).await.map_err(err)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetPreview {
    target: String,
    free_bytes: u64,
    error: Option<String>,
}

/// Shows what folder would be used for a picked folder, and how much room it has.
#[tauri::command(rename_all = "camelCase")]
fn storage_preview(state: State<'_, AppState>, path: String) -> TargetPreview {
    use solocraft_core::storage as st;
    let picked = std::path::PathBuf::from(path.trim());
    let target = st::normalize_target(&picked);
    let old = state.engine.paths().root.clone();
    let error = if !target.is_absolute() {
        Some("Choose a full folder path such as D:\\Games\\SoloCraft".to_string())
    } else if target == old {
        Some("That is already the current location".to_string())
    } else if target.starts_with(&old) || old.starts_with(&target) {
        Some("The new folder cannot be inside the current one (or the other way round)".to_string())
    } else if target.exists() && std::fs::read_dir(&target).map(|mut r| r.next().is_some()).unwrap_or(false) {
        Some("That folder is not empty. Pick an empty folder or a new one.".to_string())
    } else {
        None
    };
    TargetPreview { free_bytes: st::free_space(&target), target: target.to_string_lossy().to_string(), error }
}

/// Moves all launcher data to another folder or drive. A restart is needed afterwards.
#[tauri::command(rename_all = "camelCase")]
async fn storage_move(app: AppHandle, state: State<'_, AppState>, path: String) -> Res<String> {
    if !state.engine.running_ids().is_empty() {
        return Err("Close the running game first".into());
    }
    let old = state.engine.paths().root.clone();
    let target = solocraft_core::storage::normalize_target(&std::path::PathBuf::from(path.trim()));
    let t2 = target.clone();
    let sink = TauriSink(app);
    tokio::task::spawn_blocking(move || {
        let last = std::sync::atomic::AtomicU64::new(0);
        solocraft_core::storage::move_data(&old, &t2, &|done, total| {
            // report about every 2% so the UI is not flooded
            let step = (total / 50).max(1);
            if done / step != last.load(std::sync::atomic::Ordering::Relaxed) {
                last.store(done / step, std::sync::atomic::Ordering::Relaxed);
                solocraft_core::events::emit_progress(&sink, "storage", "Moving data", done, total);
            }
        })
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command(rename_all = "camelCase")]
fn list_log_files(state: State<'_, AppState>, id: String) -> Vec<solocraft_core::logs::LogFile> {
    solocraft_core::logs::list_logs(state.engine.paths(), &id)
}

#[tauri::command(rename_all = "camelCase")]
async fn read_log_file(state: State<'_, AppState>, id: String, kind: String, name: String) -> Res<String> {
    let paths = state.engine.paths().clone();
    tokio::task::spawn_blocking(move || solocraft_core::logs::read_log(&paths, &id, &kind, &name))
        .await
        .map_err(err)?
        .map_err(err)
}

pub fn run() {
    let loc = solocraft_core::storage::resolve();
    let engine = Engine::new(loc.root.clone()).expect("could not create the SoloCraft data folder");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { engine, first_run: loc.first_run, storage_warning: loc.warning.clone(), ms_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)) })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            create_instance,
            update_instance,
            delete_instance,
            list_versions,
            get_settings,
            save_settings,
            list_accounts,
            add_offline_account,
            set_active_account,
            remove_account,
            detect_java,
            install_java,
            launch_instance,
            kill_instance,
            running_instances,
            open_instance_folder,
            open_data_folder,
            open_instance_subfolder,
            list_loader_versions,
            create_instance_with_loader,
            change_loader,
            modrinth_search,
            modrinth_project,
            modrinth_versions,
            install_modpack,
            import_modpack_file,
            ms_has_builtin,
            client_mod_status,
            client_mod_install,
            client_mod_remove,
            list_mods,
            plan_mod_install,
            install_mod,
            install_fps_pack,
            booster_plan,
            booster_apply,
            booster_undo,
            set_mod_enabled,
            remove_mod,
            check_mod_updates,
            update_mod,
            ms_begin_login,
            ms_finish_login,
            ms_cancel_login,
            ms_verify,
            open_login_page,
            check_update,
            open_update_page,
            storage_info,
            storage_usage,
            storage_preview,
            storage_move,
            quit_app,
            list_log_files,
            read_log_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running SoloCraft Launcher");
}
