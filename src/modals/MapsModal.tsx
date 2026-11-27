import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText, MapEntry } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";

export function MapsModal() {
  const closeModal = useApp((s) => s.closeModal);
  const toast = useApp((s) => s.toast);
  const progress = useApp((s) => s.progress);
  const id = useApp((s) => s.selectedId);
  const inst = useApp((s) => s.instances.find((i) => i.id === s.selectedId));
  const [online, setOnline] = useState<MapEntry[] | null>(null);
  const [have, setHave] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [link, setLink] = useState("");
  const pr = progress["maps"];

  const reload = () => {
    if (id) api.mapsInstalled(id).then(setHave).catch(() => setHave([]));
  };
  useEffect(() => {
    reload();
    api.mapsOnline().then(setOnline).catch(() => setOnline([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  if (!id || !inst) return null;

  const run = async (key: string, fn: () => Promise<string>, ok: string) => {
    setBusy(key);
    try {
      const name = await fn();
      toast(`${ok}: ${name}`);
      reload();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(null);
    }
  };

  const pickZip = async () => {
    const p = await open({ multiple: false, filters: [{ name: "Map zip", extensions: ["zip"] }] });
    if (typeof p === "string") void run("file", () => api.mapInstallPath(id, p), "Map added");
  };
  const pickFolder = async () => {
    const p = await open({ directory: true, multiple: false, title: "Choose the map (world) folder" });
    if (typeof p === "string") void run("file", () => api.mapInstallPath(id, p), "Map added");
  };
  const fromLink = () => {
    const u = link.trim();
    if (!u.startsWith("https://")) {
      toast("The link must start with https://", "error");
      return;
    }
    void run("link", () => api.mapInstallUrl(id, u, "", ""), "Map added");
    setLink("");
  };
  const remove = async (name: string) => {
    if (!window.confirm(`Delete the world "${name}" from this instance? This cannot be undone.`)) return;
    try {
      await api.mapRemove(id, name);
      reload();
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  const installAll = async () => {
    for (const m of online ?? []) {
      if (have.includes(m.name)) continue;
      setBusy(m.id);
      try {
        await api.mapInstallUrl(id, m.url, m.name, m.sha256);
      } catch (e) {
        toast(`${m.name}: ${errText(e)}`, "error");
      }
    }
    setBusy(null);
    reload();
  };

  const missing = (online ?? []).filter((m) => !have.includes(m.name)).length;

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && !busy && closeModal()}>
      <div className="modal wide tall">
        <div className="mhead">
          <span>Maps &amp; worlds · {inst.name}</span>
          <button className="btn ghost" onClick={closeModal} disabled={!!busy} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>
        <div className="mbody">
          <div className="hsec">Add a map</div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
            <button className="btn" onClick={pickZip} disabled={!!busy}>
              <Icon name="file" /> From a .zip file
            </button>
            <button className="btn" onClick={pickFolder} disabled={!!busy}>
              <Icon name="folder" /> From a folder
            </button>
          </div>
          <div className="row" style={{ gap: 8, marginBottom: 14 }}>
            <input
              style={{ flex: 1 }}
              placeholder="Or paste a direct download link (https://…/map.zip)"
              value={link}
              onChange={(e) => setLink(e.target.value)}
              disabled={!!busy}
            />
            <button className="btn primary" onClick={fromLink} disabled={!!busy || !link.trim()}>
              <Icon name="download" /> Download
            </button>
          </div>
          {busy && (
            <div className="muted" style={{ marginBottom: 10 }}>
              Working… {pr ? `${pr.stage}${pr.total > 1 ? ` · ${Math.round((pr.current / pr.total) * 100)}%` : ""}` : ""}
            </div>
          )}

          <div className="hsec">
            Available maps {missing > 0 && <button onClick={installAll} disabled={!!busy}>Install all ({missing})</button>}
          </div>
          {online === null ? (
            <div className="hmuted">Loading…</div>
          ) : online.length === 0 ? (
            <div className="hmuted" style={{ marginBottom: 12 }}>
              No online map list yet. You can still add maps with the buttons above.
            </div>
          ) : (
            online.map((m) => (
              <div className="prow" key={m.id}>
                <i />
                <span>
                  <b>{m.name}</b>
                  <small>
                    {[m.mcVersion && `Minecraft ${m.mcVersion}`, m.sizeMb > 0 && `${m.sizeMb} MB`, m.description].filter(Boolean).join(" · ")}
                  </small>
                </span>
                {have.includes(m.name) ? (
                  <span className="chip">Installed</span>
                ) : (
                  <button className="smallbtn" disabled={!!busy} onClick={() => run(m.id, () => api.mapInstallUrl(id, m.url, m.name, m.sha256), "Map added")}>
                    {busy === m.id ? "INSTALLING…" : "INSTALL"}
                  </button>
                )}
              </div>
            ))
          )}

          <div className="hsec" style={{ marginTop: 14 }}>In this instance ({have.length})</div>
          {have.length === 0 ? (
            <div className="hmuted">No worlds yet.</div>
          ) : (
            have.map((n) => (
              <div className="prow" key={n}>
                <i />
                <span>
                  <b>{n}</b>
                </span>
                <button className="smallbtn" onClick={() => remove(n)} disabled={!!busy}>
                  REMOVE
                </button>
              </div>
            ))
          )}
        </div>
        <div className="mfoot">
          <span className="muted small" style={{ flex: 1, alignSelf: "center" }}>
            Worlds go into this instance&apos;s saves folder. Existing worlds are never overwritten.
          </span>
          <button className="btn" onClick={closeModal} disabled={!!busy}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
