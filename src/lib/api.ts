import { invoke } from "@tauri-apps/api/core";

export interface Instance {
  id: string;
  name: string;
  icon: string;
  group: string;
  mcVersion: string;
  loader: string;
  loaderVersion: string;
  accountId: string | null;
  javaPath: string | null;
  minRamMb: number;
  maxRamMb: number;
  jvmArgs: string;
  windowWidth: number;
  windowHeight: number;
  useNvidiaGpu: boolean;
  notes: string;
  createdAt: number;
  lastPlayed: number | null;
  playTimeSecs: number;
}

export interface Settings {
  theme: "pure-black" | "frosted-glass" | "classic-slate";
  accent: string;
  backgroundImage: string | null;
  backgroundBlur: number;
  gridDensity: "compact" | "comfortable";
  liteMode: boolean;
  groups: string[];
  defaultMinRamMb: number;
  defaultMaxRamMb: number;
  closeOnLaunch: boolean;
  msClientId: string;
}

export interface Account {
  id: string;
  username: string;
  uuid: string;
  kind: string;
}

export interface AccountStore {
  accounts: Account[];
  active: string | null;
}

export interface VersionEntry {
  id: string;
  kind: string;
  releaseTime: string;
}

export interface JavaInstall {
  path: string;
  version: string;
  major: number;
  vendor: string;
  arch: string;
  managed: boolean;
}

export interface Progress {
  task: string;
  stage: string;
  current: number;
  total: number;
}

export interface LogLine {
  instanceId: string;
  level: "info" | "warn" | "error";
  text: string;
}

export interface GameState {
  instanceId: string;
  state: string;
  detail: string;
}

export interface LoaderVersion {
  version: string;
  stable: boolean;
}

export interface SearchHit {
  projectId: string;
  slug: string;
  title: string;
  description: string;
  iconUrl: string | null;
  downloads: number;
  follows: number;
  author: string;
  categories: string[];
  projectType: string;
  dateModified: string;
}

export interface SearchResult {
  hits: SearchHit[];
  offset: number;
  limit: number;
  totalHits: number;
}

export interface MrProject {
  id: string;
  slug: string;
  title: string;
  description: string;
  body: string;
  iconUrl: string | null;
  downloads: number;
  followers: number;
  categories: string[];
  gameVersions: string[];
  loaders: string[];
  sourceUrl: string | null;
  issuesUrl: string | null;
}

export interface MrVersion {
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  versionType: string;
  gameVersions: string[];
  loaders: string[];
  datePublished: string;
  downloads: number;
}

export interface PlannedMod {
  projectId: string;
  title: string;
  iconUrl: string | null;
  versionId: string;
  versionNumber: string;
  filename: string;
  isDependency: boolean;
}

export interface ClientModStatus {
  jarFound: boolean;
  supported: boolean;
  installed: boolean;
  reason: string;
}

export interface ModEntry {
  filename: string;
  enabled: boolean;
  size: number;
  sha1: string;
  projectId: string | null;
  versionId: string | null;
  versionNumber: string | null;
  title: string;
  iconUrl: string | null;
  explicit: boolean;
}

export interface ModUpdate {
  filename: string;
  title: string;
  projectId: string;
  fromVersion: string;
  toVersion: string;
  toVersionId: string;
}

export interface FpsReport {
  installed: PlannedMod[];
  skipped: string[];
}

export interface SearchArgs {
  query: string;
  projectType: "mod" | "modpack";
  mcVersion?: string | null;
  loader?: string | null;
  categories: string[];
  sort: string;
  offset: number;
  limit: number;
}

export interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export interface DriveInfo {
  path: string;
  free: number;
  total: number;
  system: boolean;
}
export interface StorageInfo {
  current: string;
  defaultPath: string;
  firstRun: boolean;
  warning: string | null;
  drives: DriveInfo[];
}
export interface UsageItem {
  name: string;
  bytes: number;
}
export interface TargetPreview {
  target: string;
  freeBytes: number;
  error: string | null;
}

export interface LogFile {
  name: string;
  kind: "launcher" | "game" | "crash";
  size: number;
  modified: number;
}

export const api = {
  msBeginLogin: () => invoke<DeviceCode>("ms_begin_login"),
  msFinishLogin: (device: DeviceCode) => invoke<AccountStore>("ms_finish_login", { device }),
  msCancelLogin: () => invoke<void>("ms_cancel_login"),
  msVerify: (id: string) => invoke<string>("ms_verify", { id }),
  openLoginPage: (url: string) => invoke<void>("open_login_page", { url }),
  storageInfo: () => invoke<StorageInfo>("storage_info"),
  storageUsage: () => invoke<UsageItem[]>("storage_usage"),
  storagePreview: (path: string) => invoke<TargetPreview>("storage_preview", { path }),
  storageMove: (path: string) => invoke<string>("storage_move", { path }),
  quitApp: () => invoke<void>("quit_app"),
  listLogFiles: (id: string) => invoke<LogFile[]>("list_log_files", { id }),
  readLogFile: (id: string, kind: string, name: string) => invoke<string>("read_log_file", { id, kind, name }),
  listLoaderVersions: (loader: string, mcVersion: string) =>
    invoke<LoaderVersion[]>("list_loader_versions", { loader, mcVersion }),
  createInstanceWithLoader: (
    name: string,
    mcVersion: string,
    group: string,
    loader: string,
    loaderVersion: string,
  ) => invoke<Instance>("create_instance_with_loader", { name, mcVersion, group, loader, loaderVersion }),
  changeLoader: (id: string, loader: string, loaderVersion: string) =>
    invoke<Instance>("change_loader", { id, loader, loaderVersion }),
  modrinthSearch: (a: SearchArgs) =>
    invoke<SearchResult>("modrinth_search", {
      query: a.query,
      projectType: a.projectType,
      mcVersion: a.mcVersion ?? null,
      loader: a.loader ?? null,
      categories: a.categories,
      sort: a.sort,
      offset: a.offset,
      limit: a.limit,
    }),
  modrinthProject: (projectId: string) => invoke<MrProject>("modrinth_project", { projectId }),
  modrinthVersions: (projectId: string, loaders: string[], mcVersion?: string | null) =>
    invoke<MrVersion[]>("modrinth_versions", { projectId, loaders, mcVersion: mcVersion ?? null }),
  importModpackFile: (path: string, name: string | null, group: string) =>
    invoke<Instance>("import_modpack_file", { path, name, group }),
  installModpack: (projectId: string, versionId: string | null, name: string | null, group: string) =>
    invoke<Instance>("install_modpack", { projectId, versionId, name, group }),
  msHasBuiltin: () => invoke<boolean>("ms_has_builtin"),
  clientModStatus: (id: string) => invoke<ClientModStatus>("client_mod_status", { id }),
  clientModInstall: (id: string) => invoke<void>("client_mod_install", { id }),
  clientModRemove: (id: string) => invoke<void>("client_mod_remove", { id }),
  listMods: (id: string) => invoke<ModEntry[]>("list_mods", { id }),
  planModInstall: (id: string, projectId: string) => invoke<PlannedMod[]>("plan_mod_install", { id, projectId }),
  installMod: (id: string, projectId: string) => invoke<PlannedMod[]>("install_mod", { id, projectId }),
  installFpsPack: (id: string) => invoke<FpsReport>("install_fps_pack", { id }),
  setModEnabled: (id: string, filename: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { id, filename, enabled }),
  removeMod: (id: string, filename: string) => invoke<void>("remove_mod", { id, filename }),
  checkModUpdates: (id: string) => invoke<ModUpdate[]>("check_mod_updates", { id }),
  updateMod: (id: string, filename: string, toVersionId: string) =>
    invoke<void>("update_mod", { id, filename, toVersionId }),

  listInstances: () => invoke<Instance[]>("list_instances"),
  createInstance: (name: string, mcVersion: string, group: string) =>
    invoke<Instance>("create_instance", { name, mcVersion, group }),
  updateInstance: (instance: Instance) => invoke<void>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  listVersions: () => invoke<VersionEntry[]>("list_versions"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  listAccounts: () => invoke<AccountStore>("list_accounts"),
  addOfflineAccount: (username: string) => invoke<AccountStore>("add_offline_account", { username }),
  setActiveAccount: (id: string) => invoke<AccountStore>("set_active_account", { id }),
  removeAccount: (id: string) => invoke<AccountStore>("remove_account", { id }),
  detectJava: () => invoke<JavaInstall[]>("detect_java"),
  installJava: (major: number) => invoke<JavaInstall>("install_java", { major }),
  launchInstance: (id: string) => invoke<void>("launch_instance", { id }),
  killInstance: (id: string) => invoke<boolean>("kill_instance", { id }),
  runningInstances: () => invoke<string[]>("running_instances"),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),
  openInstanceSubfolder: (id: string, kind: string) =>
    invoke<void>("open_instance_subfolder", { id, kind }),
  openDataFolder: () => invoke<void>("open_data_folder"),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
