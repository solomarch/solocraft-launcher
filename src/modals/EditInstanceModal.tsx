import { useEffect, useState } from "react";
import { Instance } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";
import { LoaderPicker } from "../components/LoaderPicker";
import { api, errText } from "../lib/api";

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return <button className={"toggle" + (on ? " on" : "")} onClick={() => onChange(!on)} aria-pressed={on} />;
}

export function EditInstanceModal() {
  const id = useApp((s) => s.selectedId);
  const original = useApp((s) => s.instances.find((i) => i.id === id));
  const settings = useApp((s) => s.settings);
  const accounts = useApp((s) => s.accounts);
  const java = useApp((s) => s.java);
  const refreshJava = useApp((s) => s.refreshJava);
  const saveInstance = useApp((s) => s.saveInstance);
  const closeModal = useApp((s) => s.closeModal);

  const [inst, setInst] = useState<Instance | null>(original ?? null);
  const [supported, setSupported] = useState(true);
  const toast = useApp((s) => s.toast);

  useEffect(() => {
    if (java.length === 0) void refreshJava();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!inst) return null;
  const set = <K extends keyof Instance>(k: K, v: Instance[K]) => setInst({ ...inst, [k]: v });

  const save = async () => {
    let fixed = { ...inst, name: inst.name.trim() || inst.mcVersion };
    if (original && (fixed.loader !== original.loader || fixed.loaderVersion !== original.loaderVersion)) {
      try {
        const changed = await api.changeLoader(original.id, fixed.loader, fixed.loaderVersion);
        fixed = { ...fixed, loader: changed.loader, loaderVersion: changed.loaderVersion };
      } catch (e) {
        toast(errText(e), "error");
        return;
      }
    }
    if (await saveInstance(fixed)) closeModal();
  };

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && closeModal()}>
      <div className="modal">
        <div className="mhead">
          Edit {original?.name}
          <button className="btn ghost" onClick={closeModal} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>
        <div className="mbody">
          <div className="section-title">General</div>
          <div className="field">
            <label>Name</label>
            <input type="text" value={inst.name} onChange={(e) => set("name", e.target.value)} />
          </div>
          <div className="field">
            <label>Group</label>
            <select value={inst.group} onChange={(e) => set("group", e.target.value)}>
              <option value="">No group</option>
              {settings.groups.map((g) => (
                <option key={g} value={g}>
                  {g}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label>Account</label>
            <select value={inst.accountId ?? ""} onChange={(e) => set("accountId", e.target.value || null)}>
              <option value="">Use the active account</option>
              {accounts.accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.username}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label>Notes</label>
            <textarea rows={2} value={inst.notes} onChange={(e) => set("notes", e.target.value)} />
          </div>

          <LoaderPicker
            mcVersion={inst.mcVersion}
            loader={inst.loader}
            loaderVersion={inst.loaderVersion}
            onChange={(l, v) => setInst({ ...inst, loader: l, loaderVersion: v })}
            onSupport={setSupported}
          />
          {original && inst.loader !== original.loader && original.loader !== "vanilla" && (
            <div className="hint" style={{ marginLeft: 128 }}>
              Mods built for {original.loader} will not load under {inst.loader}. Check the Mods window afterwards.
            </div>
          )}

          <div className="section-title" style={{ marginTop: 14 }}>
            Java
          </div>
          <div className="field">
            <label>Java</label>
            <select value={inst.javaPath ?? ""} onChange={(e) => set("javaPath", e.target.value || null)}>
              <option value="">Auto (recommended)</option>
              {java.map((j) => (
                <option key={j.path} value={j.path}>
                  Java {j.major} · {j.vendor || "unknown"} · {j.version}
                </option>
              ))}
              {inst.javaPath && !java.some((j) => j.path === inst.javaPath) && (
                <option value={inst.javaPath}>{inst.javaPath}</option>
              )}
            </select>
          </div>
          <div className="field">
            <label>Custom path</label>
            <input
              type="text"
              placeholder="Leave empty for Auto"
              value={inst.javaPath ?? ""}
              onChange={(e) => set("javaPath", e.target.value.trim() === "" ? null : e.target.value)}
            />
          </div>
          <div className="field">
            <label>Minimum RAM</label>
            <div className="row">
              <input
                type="range"
                min={256}
                max={16384}
                step={256}
                value={inst.minRamMb}
                onChange={(e) => set("minRamMb", Number(e.target.value))}
                style={{ flex: 1 }}
              />
              <span style={{ width: 60, textAlign: "right" }}>{inst.minRamMb} MB</span>
            </div>
          </div>
          <div className="field">
            <label>Maximum RAM</label>
            <div className="row">
              <input
                type="range"
                min={512}
                max={32768}
                step={256}
                value={inst.maxRamMb}
                onChange={(e) => set("maxRamMb", Number(e.target.value))}
                style={{ flex: 1 }}
              />
              <span style={{ width: 60, textAlign: "right" }}>{inst.maxRamMb} MB</span>
            </div>
          </div>
          <div className="field">
            <label>JVM arguments</label>
            <input
              type="text"
              placeholder="-XX:+UseG1GC"
              value={inst.jvmArgs}
              onChange={(e) => set("jvmArgs", e.target.value)}
            />
          </div>

          <div className="section-title" style={{ marginTop: 14 }}>
            Window and graphics
          </div>
          <div className="field">
            <label>Window size</label>
            <div className="row">
              <input
                type="number"
                min={0}
                value={inst.windowWidth || ""}
                placeholder="Width"
                onChange={(e) => set("windowWidth", Number(e.target.value) || 0)}
                style={{ width: 80 }}
              />
              ×
              <input
                type="number"
                min={0}
                value={inst.windowHeight || ""}
                placeholder="Height"
                onChange={(e) => set("windowHeight", Number(e.target.value) || 0)}
                style={{ width: 80 }}
              />
              <span style={{ color: "var(--muted)" }}>Empty uses the game default</span>
            </div>
          </div>
          <div className="field">
            <label>NVIDIA GPU</label>
            <div className="row">
              <Toggle on={inst.useNvidiaGpu} onChange={(v) => set("useNvidiaGpu", v)} />
              <span style={{ color: "var(--muted)" }}>
                Run on the dedicated GPU (Windows high-performance preference, Linux PRIME offload)
              </span>
            </div>
          </div>
        </div>
        <div className="mfoot">
          <button className="btn" onClick={closeModal}>
            Cancel
          </button>
          <button className="btn primary" onClick={save} disabled={!supported}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
