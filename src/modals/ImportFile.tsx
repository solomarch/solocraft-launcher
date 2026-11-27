import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText } from "../lib/api";
import { progressPct } from "../lib/format";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";

export function ImportFile({ group }: { group: string }) {
  const toast = useApp((s) => s.toast);
  const adoptInstance = useApp((s) => s.adoptInstance);
  const progress = useApp((s) => s.progress);
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const pr = progress["import"];
  const pct = progressPct(pr);

  const browse = async () => {
    try {
      const picked = await open({
        multiple: false,
        title: "Choose a modpack file",
        filters: [{ name: "Modpack (.mrpack, .zip)", extensions: ["mrpack", "zip"] }],
      });
      if (typeof picked === "string") setPath(picked);
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  const run = async () => {
    setBusy(true);
    try {
      const inst = await api.importModpackFile(path, name.trim() || null, group);
      toast(`${inst.name} was imported`);
      await adoptInstance(inst);
    } catch (e) {
      toast(errText(e), "error");
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="field">
        <label>Modpack file</label>
        <div className="row">
          <input
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="C:\Users\you\Downloads\pack.mrpack"
            style={{ flex: 1 }}
          />
          <button className="btn" onClick={browse} disabled={busy}>
            <Icon name="folder" /> Browse…
          </button>
        </div>
      </div>
      <div className="field">
        <label>Name (optional)</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Leave empty to use the pack's own name"
        />
      </div>
      <div className="row">
        <button className="btn primary" disabled={busy || path.trim() === ""} onClick={run}>
          <Icon name="download" /> {busy ? "Importing…" : "Import"}
        </button>
        {busy && (
          <div style={{ flex: 1 }}>
            <div className="muted">{pr?.stage ?? "Starting…"}</div>
            {pct !== null && (
              <div className="bar">
                <i style={{ width: pct + "%" }} />
              </div>
            )}
          </div>
        )}
      </div>
      <div className="muted" style={{ marginTop: 12, lineHeight: 1.6 }}>
        Works with:
        <ul style={{ margin: "4px 0 0 18px" }}>
          <li>Modrinth modpacks (.mrpack, or a .zip with the same layout) – Fabric, Quilt, Forge, NeoForge</li>
          <li>Prism / MultiMC / PolyMC instance exports (.zip)</li>
        </ul>
        CurseForge packs are not supported yet. Every download is checked against its checksum, and a failed import
        removes the half-made instance.
      </div>
    </div>
  );
}
