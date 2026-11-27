import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  errText,
  AccountStore,
  GameState,
  Instance,
  JavaInstall,
  LogLine,
  Progress,
  Settings,
  VersionEntry,
} from "../lib/api";

export type Page = "home" | "instances" | "java" | "settings";
export const ACTIVE_STATES = ["preparing", "downloading", "launching", "running"];
const MAX_LINES = 4000;

export interface Toast {
  id: number;
  text: string;
  kind: "error" | "info";
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "sm-night",
  accent: "#ff7a45",
  backgroundImage: null,
  backgroundBlur: 12,
  gridDensity: "comfortable",
  liteMode: false,
  groups: ["Survival", "Modpacks", "PvP / Testing"],
  defaultMinRamMb: 512,
  defaultMaxRamMb: 4096,
  closeOnLaunch: false,
  msClientId: "",
};

interface AppState {
  ready: boolean;
  page: Page;
  instances: Instance[];
  selectedId: string | null;
  group: string;
  search: string;
  settings: Settings;
  accounts: AccountStore;
  versions: VersionEntry[];
  versionsError: string | null;
  versionsLoading: boolean;
  java: JavaInstall[];
  javaLoading: boolean;
  logs: Record<string, LogLine[]>;
  states: Record<string, GameState>;
  progress: Record<string, Progress>;
  /** estimated seconds left for the current step, per task */
  eta: Record<string, number | null>;
  modal: null | "new" | "edit" | "mods" | "logs" | "scan" | "maps";
  consoleOpen: boolean;
  toasts: Toast[];

  init: () => Promise<void>;
  setPage: (p: Page) => void;
  select: (id: string | null) => void;
  setGroup: (g: string) => void;
  setSearch: (s: string) => void;
  openModal: (m: "new" | "edit" | "mods" | "logs" | "scan" | "maps") => void;
  closeModal: () => void;
  toggleConsole: (open?: boolean) => void;
  toast: (text: string, kind?: "error" | "info") => void;
  dismissToast: (id: number) => void;
  clearLogs: (id: string) => void;

  refreshInstances: () => Promise<void>;
  loadVersions: (force?: boolean) => Promise<void>;
  refreshJava: () => Promise<void>;
  createInstance: (
    name: string,
    mcVersion: string,
    group: string,
    loader?: string,
    loaderVersion?: string,
    fps?: boolean,
  ) => Promise<boolean>;
  adoptInstance: (inst: Instance) => Promise<void>;
  saveInstance: (i: Instance) => Promise<boolean>;
  deleteInstance: (id: string) => Promise<void>;
  launch: (id: string) => Promise<void>;
  kill: (id: string) => Promise<void>;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  addAccount: (username: string) => Promise<boolean>;
  setActiveAccount: (id: string) => Promise<void>;
  removeAccount: (id: string) => Promise<void>;
  installJava: (major: number) => Promise<void>;
}

let toastId = 1;
let listening = false;
let pending: LogLine[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;

function flushLogs() {
  flushTimer = null;
  const batch = pending;
  pending = [];
  if (batch.length === 0) return;
  useApp.setState((s) => {
    const logs = { ...s.logs };
    const byId: Record<string, LogLine[]> = {};
    for (const l of batch) (byId[l.instanceId] ||= []).push(l);
    for (const id of Object.keys(byId)) {
      const merged = (logs[id] || []).concat(byId[id]);
      logs[id] = merged.length > MAX_LINES ? merged.slice(merged.length - MAX_LINES) : merged;
    }
    return { logs };
  });
}

// Remaining time for the current step, from the speed over the last few seconds.
const samples: Record<string, { stage: string; pts: { t: number; c: number }[] }> = {};
function estimate(p: Progress): number | null {
  if (p.total === 0) {
    delete samples[p.task];
    return null;
  }
  const now = Date.now();
  let h = samples[p.task];
  const last = h?.pts[h.pts.length - 1];
  if (!h || h.stage !== p.stage || (last && p.current < last.c)) {
    h = samples[p.task] = { stage: p.stage, pts: [] };
  }
  h.pts.push({ t: now, c: p.current });
  h.pts = h.pts.filter((x) => now - x.t <= 8000);
  const first = h.pts[0];
  const span = (now - first.t) / 1000;
  if (span < 2 || p.current <= first.c) return null;
  const rate = (p.current - first.c) / span;
  return Math.max(0, Math.round((p.total - p.current) / rate));
}

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  page: "home",
  instances: [],
  selectedId: null,
  group: "All",
  search: "",
  settings: DEFAULT_SETTINGS,
  accounts: { accounts: [], active: null },
  versions: [],
  versionsError: null,
  versionsLoading: false,
  java: [],
  javaLoading: false,
  logs: {},
  states: {},
  progress: {},
  eta: {},
  modal: null,
  consoleOpen: false,
  toasts: [],

  init: async () => {
    if (!listening) {
      listening = true;
      await listen<LogLine>("log", (e) => {
        pending.push(e.payload);
        if (!flushTimer) flushTimer = setTimeout(flushLogs, 80);
      });
      await listen<Progress>("progress", (e) => {
        const eta = estimate(e.payload);
        set((s) => ({ progress: { ...s.progress, [e.payload.task]: e.payload }, eta: { ...s.eta, [e.payload.task]: eta } }));
      });
      await listen<GameState>("game-state", (e) => {
        const g = e.payload;
        const prev = get().states[g.instanceId];
        const wasActive = prev ? ACTIVE_STATES.includes(prev.state) : false;
        set((s) => {
          const next: Partial<AppState> = { states: { ...s.states, [g.instanceId]: g } };
          if (g.state === "preparing" && !wasActive) {
            next.logs = { ...s.logs, [g.instanceId]: [] };
          }
          if (g.state === "running" || !ACTIVE_STATES.includes(g.state)) {
            const p = { ...s.progress };
            delete p[g.instanceId];
            next.progress = p;
          }
          return next as AppState;
        });
        if (g.state === "error") get().toast(g.detail, "error");
        if (g.state === "crashed") get().toast(g.detail, "error");
        if (!ACTIVE_STATES.includes(g.state)) void get().refreshInstances();
      });
    }
    try {
      const [instances, settings, accounts, running] = await Promise.all([
        api.listInstances(),
        api.getSettings(),
        api.listAccounts(),
        api.runningInstances(),
      ]);
      const states: Record<string, GameState> = {};
      for (const id of running) states[id] = { instanceId: id, state: "running", detail: "" };
      set({
        instances,
        settings,
        accounts,
        states,
        selectedId: instances[0]?.id ?? null,
        ready: true,
      });
    } catch (e) {
      set({ ready: true });
      get().toast(errText(e), "error");
    }
  },

  setPage: (page) => set({ page }),
  select: (id) => set({ selectedId: id }),
  setGroup: (group) => set({ group }),
  setSearch: (search) => set({ search }),
  openModal: (modal) => set({ modal }),
  closeModal: () => set({ modal: null }),
  toggleConsole: (open) => set((s) => ({ consoleOpen: open ?? !s.consoleOpen })),
  toast: (text, kind = "info") => {
    const id = toastId++;
    set((s) => ({ toasts: [...s.toasts, { id, text, kind }] }));
    setTimeout(() => get().dismissToast(id), kind === "error" ? 9000 : 4000);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clearLogs: (id) => set((s) => ({ logs: { ...s.logs, [id]: [] } })),

  refreshInstances: async () => {
    try {
      const instances = await api.listInstances();
      set((s) => ({
        instances,
        selectedId:
          s.selectedId && instances.some((i) => i.id === s.selectedId)
            ? s.selectedId
            : instances[0]?.id ?? null,
      }));
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  loadVersions: async (force) => {
    if (get().versionsLoading) return;
    if (!force && get().versions.length > 0) return;
    set({ versionsLoading: true, versionsError: null });
    try {
      const versions = await api.listVersions();
      set({ versions, versionsLoading: false });
    } catch (e) {
      set({ versionsLoading: false, versionsError: errText(e) });
    }
  },

  refreshJava: async () => {
    set({ javaLoading: true });
    try {
      set({ java: await api.detectJava(), javaLoading: false });
    } catch (e) {
      set({ javaLoading: false });
      get().toast(errText(e), "error");
    }
  },

  createInstance: async (name, mcVersion, group, loader = "vanilla", loaderVersion = "", fps = false) => {
    try {
      const inst = await api.createInstanceWithLoader(name, mcVersion, group, loader, loaderVersion);
      await get().refreshInstances();
      set({ selectedId: inst.id, modal: null, page: "instances" });
      if (fps && loader !== "vanilla") {
        get().toast("Installing the FPS boost pack…");
        try {
          const rep = await api.installFpsPack(inst.id);
          const n = rep.installed.length;
          get().toast(
            n > 0
              ? `FPS pack installed (${n} mod${n === 1 ? "" : "s"})` +
                  (rep.skipped.length ? `. Not available for ${inst.mcVersion}: ${rep.skipped.join(", ")}` : "")
              : `No FPS mods are available for ${inst.mcVersion} yet`,
          );
        } catch (e) {
          get().toast("The instance was created, but the FPS pack failed: " + errText(e), "error");
        }
        set((s) => {
          const p = { ...s.progress };
          delete p[inst.id];
          return { progress: p };
        });
      }
      return true;
    } catch (e) {
      get().toast(errText(e), "error");
      return false;
    }
  },

  adoptInstance: async (inst) => {
    await get().refreshInstances();
    set({ selectedId: inst.id, modal: null, page: "instances" });
  },

  saveInstance: async (i) => {
    try {
      await api.updateInstance(i);
      await get().refreshInstances();
      return true;
    } catch (e) {
      get().toast(errText(e), "error");
      return false;
    }
  },

  deleteInstance: async (id) => {
    try {
      await api.deleteInstance(id);
      await get().refreshInstances();
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  launch: async (id) => {
    if (get().accounts.accounts.length === 0) {
      get().toast("Add an account in Settings before launching", "error");
      set({ page: "settings" });
      return;
    }
    set({ selectedId: id, consoleOpen: true });
    try {
      await api.launchInstance(id);
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  kill: async (id) => {
    try {
      await api.killInstance(id);
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  saveSettings: async (patch) => {
    const next = { ...get().settings, ...patch };
    set({ settings: next });
    try {
      await api.saveSettings(next);
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  addAccount: async (username) => {
    try {
      set({ accounts: await api.addOfflineAccount(username) });
      return true;
    } catch (e) {
      get().toast(errText(e), "error");
      return false;
    }
  },

  setActiveAccount: async (id) => {
    try {
      set({ accounts: await api.setActiveAccount(id) });
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  removeAccount: async (id) => {
    try {
      set({ accounts: await api.removeAccount(id) });
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },

  installJava: async (major) => {
    try {
      await api.installJava(major);
      set((s) => {
        const p = { ...s.progress };
        delete p["java"];
        return { progress: p };
      });
      await get().refreshJava();
      get().toast(`Java ${major} is ready`);
    } catch (e) {
      get().toast(errText(e), "error");
    }
  },
}));

export function isActive(states: Record<string, GameState>, id: string): boolean {
  const s = states[id];
  return !!s && ACTIVE_STATES.includes(s.state);
}
