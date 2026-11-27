import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, DriveInfo, errText, StorageInfo, TargetPreview, UsageItem } from "../lib/api";
import { progressPct } from "../lib/format";
import { useApp } from "../store/app";
import { Icon } from "./Icon";

export function fmtBytes(n: number): string {
  if (n >= 1024 ** 3) return (n / 1024 ** 3).toFixed(1) + " GB";
  if (n >= 1024 ** 2) return Math.round(n / 1024 ** 2) + " MB";
  return Math.max(0, Math.round(n / 1024)) + " KB";
}

function DriveRow({ d, onPick }: { d: DriveInfo; onPick: (path: string) => void }) {
  const used = d.total > 0 ? Math.round(((d.total - d.free) / d.total) * 100) : 0;
  return (
    <button className="driverow" onClick={() => onPick(d.path)}>
      <Icon name="folder" />
      <div style={{ flex: 1, minWidth: 0, textAlign: "left" }}>
        <div>
          {d.path} {d.system && <span className="tag">system drive</span>}
        </div>
        <div className="bar thin">
          <i style={{ width: used + "%", background: used > 90 ? "#f87171" : undefined }} />
        </div>
      </div>
      <div className="muted" style={{ whiteSpace: "nowrap" }}>
        {fmtBytes(d.free)} free of {fmtBytes(d.total)}
      </div>
    </button>
  );
}

/** Pick a folder, preview where data would go, confirm, move, then ask for a restart. */
function useMover(info: StorageInfo | null) {
  const toast = useApp((s) => s.toast);
  const progress = useApp((s) => s.progress["storage"]);
  const [path, setPath] = useState("");
  const [preview, setPreview] = useState<TargetPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);

  useEffect(() => {
    if (!path.trim()) {
      setPreview(null);
      return;
    }
    const t = setTimeout(() => {
      api.storagePreview(path).then(setPreview).catch(() => setPreview(null));
    }, 200);
    return () => clearTimeout(t);
  }, [path]);

  const browse = async () => {
    try {
      const picked = await open({ directory: true, multiple: false, title: "Choose where SM Craft stores its data" });
      if (typeof picked === "string") setPath(picked);
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  const move = async () => {
    setBusy(true);
    try {
      setDone(await api.storageMove(path));
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return { path, setPath, preview, busy, done, browse, move, pct: progressPct(progress), info };
}

function Chooser({ info, m, label }: { info: StorageInfo; m: ReturnType<typeof useMover>; label: string }) {
  if (m.done) {
    return (
      <div className="devicebox">
        <strong>Done. Data is now in {m.done}</strong>
        <div className="muted" style={{ margin: "4px 0 8px" }}>
          Close SM Craft and open it again so it uses the new location.
        </div>
        <button className="btn primary" onClick={() => api.quitApp()}>
          Quit SM Craft
        </button>
      </div>
    );
  }
  const ok = !!m.preview && !m.preview.error;
  return (
    <>
      <div className="drives">
        {info.drives.map((d) => (
          <DriveRow key={d.path} d={d} onPick={m.setPath} />
        ))}
      </div>
      <div className="row wrap" style={{ marginTop: 8 }}>
        <input
          type="text"
          placeholder="Folder, for example E:\Games"
          value={m.path}
          onChange={(e) => m.setPath(e.target.value)}
          style={{ flex: 1, minWidth: 220 }}
          disabled={m.busy}
        />
        <button className="btn" onClick={m.browse} disabled={m.busy}>
          <Icon name="folder" /> Browse…
        </button>
      </div>
      {m.preview && (
        <div className={"hint" + (m.preview.error ? " err" : "")} style={{ marginTop: 6 }}>
          {m.preview.error ?? `Data will be stored in ${m.preview.target} (${fmtBytes(m.preview.freeBytes)} free there)`}
        </div>
      )}
      <div className="row" style={{ marginTop: 8 }}>
        <button className="btn primary" onClick={m.move} disabled={!ok || m.busy}>
          {m.busy ? "Moving…" : label}
        </button>
        {m.busy && m.pct !== null && (
          <div className="bar" style={{ flex: 1 }}>
            <i style={{ width: m.pct + "%" }} />
          </div>
        )}
      </div>
    </>
  );
}

export function StorageCard() {
  const [info, setInfo] = useState<StorageInfo | null>(null);
  const [usage, setUsage] = useState<UsageItem[] | null>(null);
  const [changing, setChanging] = useState(false);
  const m = useMover(info);

  useEffect(() => {
    api.storageInfo().then(setInfo).catch(() => undefined);
    api.storageUsage().then(setUsage).catch(() => undefined);
  }, []);

  const total = (usage ?? []).reduce((a, b) => a + b.bytes, 0);
  return (
    <div className="card wide">
      <h3>Storage</h3>
      {info?.warning && <div className="hint err" style={{ marginBottom: 6 }}>{info.warning}</div>}
      <div className="row wrap">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="muted">Games, mods, Java and assets are stored in</div>
          <div style={{ fontWeight: 600, wordBreak: "break-all" }}>{info?.current ?? "…"}</div>
        </div>
        <div className="muted">Total used: {usage ? fmtBytes(total) : "…"}</div>
        <button className="btn" onClick={() => setChanging(!changing)}>
          {changing ? "Cancel" : "Change location…"}
        </button>
      </div>
      {usage && usage.length > 0 && (
        <div className="usage">
          {usage
            .filter((u) => u.bytes > 0)
            .slice(0, 6)
            .map((u) => (
              <div key={u.name} className="usageitem">
                <span>{u.name}</span>
                <b>{fmtBytes(u.bytes)}</b>
              </div>
            ))}
        </div>
      )}
      {changing && info && (
        <div className="msbox">
          <div className="muted" style={{ marginBottom: 6 }}>
            Pick a drive or folder. Everything is copied, checked, and only then removed from the old place. Works for
            external and secondary drives.
          </div>
          <Chooser info={info} m={m} label="Move data here" />
        </div>
      )}
    </div>
  );
}

/** Shown once, on the very first start, before anything big is downloaded. */
export function StoragePrompt({ info, onClose }: { info: StorageInfo; onClose: () => void }) {
  const m = useMover(info);
  return (
    <div className="overlay">
      <div className="modal" style={{ width: 560 }}>
        <div className="mhead">Where should SM Craft store its data?</div>
        <div className="mbody">
          <div className="muted" style={{ marginBottom: 8 }}>
            Games, mods and Java take several GB. Pick a drive with room, such as an external or secondary drive. You can
            change this later in Settings → Storage.
          </div>
          <Chooser info={info} m={m} label="Use this location" />
        </div>
        <div className="mfoot">
          <span className="muted" style={{ marginRight: "auto" }}>
            Default: {info.defaultPath}
          </span>
          {!m.done && (
            <button className="btn" onClick={onClose} disabled={m.busy}>
              Keep the default
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
