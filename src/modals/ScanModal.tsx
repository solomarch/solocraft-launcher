import { useEffect, useMemo, useState } from "react";
import { api, errText, FoundInstance, ImportOpts } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";

const LOADER_LABEL: Record<string, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
  vanilla: "Vanilla",
};

const OPTION_LIST: { key: keyof ImportOpts; label: string }[] = [
  { key: "mods", label: "Mods & their configs" },
  { key: "settings", label: "Settings & keybinds" },
  { key: "servers", label: "Servers list" },
  { key: "packs", label: "Resource packs & shaders" },
  { key: "worlds", label: "Worlds (saves, can be big)" },
  { key: "screenshots", label: "Screenshots" },
];

export function ScanModal() {
  const closeModal = useApp((s) => s.closeModal);
  const toast = useApp((s) => s.toast);
  const progress = useApp((s) => s.progress);
  const [found, setFound] = useState<FoundInstance[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [opts, setOpts] = useState<ImportOpts>({
    mods: true,
    settings: true,
    servers: true,
    packs: true,
    worlds: true,
    screenshots: false,
  });
  const [busy, setBusy] = useState(false);
  const [doneCount, setDoneCount] = useState(0);
  const pr = progress["scan"];

  const scan = async () => {
    setFound(null);
    try {
      const list = await api.scanInstances();
      setFound(list);
    } catch (e) {
      toast(errText(e), "error");
      setFound([]);
    }
  };

  useEffect(() => {
    void scan();
  }, []);

  const groups = useMemo(() => {
    const m = new Map<string, FoundInstance[]>();
    for (const f of found ?? []) m.set(f.launcher, [...(m.get(f.launcher) ?? []), f]);
    return [...m.entries()];
  }, [found]);

  const toggle = (f: FoundInstance) => {
    if (f.already || !f.mcVersion || busy) return;
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(f.key)) n.delete(f.key);
      else n.add(f.key);
      return n;
    });
  };

  const run = async () => {
    const list = (found ?? []).filter((f) => picked.has(f.key));
    if (list.length === 0) return;
    setBusy(true);
    setDoneCount(0);
    let last = null;
    let ok = 0;
    for (const f of list) {
      try {
        last = await api.importFoundInstance(f, opts);
        ok++;
        setDoneCount(ok);
      } catch (e) {
        toast(`${f.name}: ${errText(e)}`, "error");
      }
    }
    await useApp.getState().refreshInstances();
    try {
      useApp.setState({ settings: await api.getSettings() });
    } catch {
      /* ignore */
    }
    if (ok > 0 && last) {
      toast(`Imported ${ok} instance${ok === 1 ? "" : "s"}`);
      useApp.setState({ selectedId: last.id, modal: null, page: "instances" });
    } else {
      setBusy(false);
    }
  };

  const total = found?.length ?? 0;
  const launchers = groups.length;

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && !busy && closeModal()}>
      <div className="modal wide tall">
        <div className="mhead">
          <span>Found on this PC</span>
          <button className="btn ghost" onClick={closeModal} disabled={busy} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>
        <div className="mbody">
          <div className="muted" style={{ marginBottom: 8 }}>
            Pick what to bring into SM Craft. Your original launchers are never changed.
          </div>
          {found === null ? (
            <div className="scanstate">Scanning your PC…</div>
          ) : total === 0 ? (
            <div className="scanstate">
              No instances from other launchers were found in the usual places.
              <div className="muted" style={{ marginTop: 4 }}>
                Checked: Minecraft Launcher, Prism / PolyMC / MultiMC, CurseForge app and Modrinth App.
              </div>
            </div>
          ) : (
            <div className="scanstate ok">
              ✓ {total} instance{total === 1 ? "" : "s"} in {launchers} launcher{launchers === 1 ? "" : "s"}
            </div>
          )}

          {groups.map(([launcher, list]) => (
            <div key={launcher}>
              <div className="scanhead">
                {launcher} <span>{list.length}</span>
              </div>
              {list.map((f) => {
                const disabled = f.already || !f.mcVersion;
                const on = picked.has(f.key);
                return (
                  <div
                    key={f.key}
                    className={"scanrow" + (on ? " sel" : "") + (disabled ? " off" : "")}
                    onClick={() => toggle(f)}
                  >
                    <span className="ck">{on ? "✓" : ""}</span>
                    <span className="nm">{f.name}</span>
                    {f.loader !== "vanilla" && f.mcVersion && <span className="chip lc">{LOADER_LABEL[f.loader] ?? f.loader}</span>}
                    {f.loader === "vanilla" && f.mcVersion && <span className="chip">Vanilla</span>}
                    {f.mcVersion && <span className="chip">{f.mcVersion}</span>}
                    <span className="chip">{f.mods} mods</span>
                    <span className="chip">
                      {f.worlds} world{f.worlds === 1 ? "" : "s"}
                    </span>
                    <span className="grow" />
                    <span className="muted small">
                      {f.already
                        ? "Already imported"
                        : f.note
                        ? f.note
                        : [f.hasOptions && "keybinds", f.hasServers && "servers", f.hasPacks && "packs"].filter(Boolean).join(" · ")}
                    </span>
                  </div>
                );
              })}
            </div>
          ))}

          {total > 0 && (
            <div className="scanopts">
              <span className="muted">Bring over:</span>
              {OPTION_LIST.map((o) => (
                <button
                  key={o.key}
                  className={"pill" + (opts[o.key] ? " on" : "")}
                  disabled={busy}
                  onClick={() => setOpts({ ...opts, [o.key]: !opts[o.key] })}
                >
                  {opts[o.key] ? "✓ " : ""}
                  {o.label}
                </button>
              ))}
            </div>
          )}
          {busy && (
            <div className="muted" style={{ marginTop: 8 }}>
              Importing… {doneCount}/{picked.size} {pr ? `· ${pr.stage}` : ""}
            </div>
          )}
        </div>
        <div className="mfoot">
          <span className="muted small" style={{ flex: 1, alignSelf: "center" }}>
            Files are copied. Accounts are not: sign in once in SM Craft.
          </span>
          <button className="btn" onClick={scan} disabled={busy}>
            <Icon name="refresh" /> Scan again
          </button>
          <button className="btn" onClick={closeModal} disabled={busy}>
            Cancel
          </button>
          <button className="btn primary" onClick={run} disabled={busy || picked.size === 0}>
            Import {picked.size || ""} instance{picked.size === 1 ? "" : "s"}
          </button>
        </div>
      </div>
    </div>
  );
}
