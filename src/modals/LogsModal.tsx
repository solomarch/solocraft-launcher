import { useCallback, useEffect, useState } from "react";
import { api, errText, LogFile } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";
import { fmtBytes } from "../components/Storage";

const KIND_LABEL: Record<string, string> = {
  launcher: "Launcher log",
  game: "Game logs",
  crash: "Crash reports",
};

function when(secs: number): string {
  return secs ? new Date(secs * 1000).toLocaleString() : "";
}

export function LogsModal() {
  const id = useApp((s) => s.selectedId);
  const inst = useApp((s) => s.instances.find((i) => i.id === id));
  const closeModal = useApp((s) => s.closeModal);
  const toast = useApp((s) => s.toast);

  const [files, setFiles] = useState<LogFile[]>([]);
  const [sel, setSel] = useState<LogFile | null>(null);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);

  const load = useCallback(
    async (f: LogFile) => {
      if (!id) return;
      setSel(f);
      setLoading(true);
      try {
        setText(await api.readLogFile(id, f.kind, f.name));
      } catch (e) {
        setText("");
        toast(errText(e), "error");
      } finally {
        setLoading(false);
      }
    },
    [id, toast],
  );

  const refresh = useCallback(async () => {
    if (!id) return;
    const list = await api.listLogFiles(id);
    setFiles(list);
    // open the newest crash report if there is one, otherwise latest.log
    const pick = list.find((f) => f.kind === "crash") ?? list.find((f) => f.name === "latest.log") ?? list[0];
    if (pick) void load(pick);
  }, [id, load]);

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  if (!inst) return null;
  const kinds = ["launcher", "game", "crash"] as const;

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && closeModal()}>
      <div className="modal wide tall">
        <div className="mhead">
          Log files · {inst.name}
          <button className="btn ghost" onClick={closeModal} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>
        <div className="logsplit">
          <div className="logfiles">
            {files.length === 0 && <div className="empty">No logs yet. Launch the instance once.</div>}
            {kinds.map((k) => {
              const group = files.filter((f) => f.kind === k);
              if (group.length === 0) return null;
              return (
                <div key={k}>
                  <div className="section-title">{KIND_LABEL[k]}</div>
                  {group.map((f) => (
                    <button
                      key={k + f.name}
                      className={"logfile" + (sel?.name === f.name && sel.kind === f.kind ? " on" : "")}
                      onClick={() => load(f)}
                    >
                      <div>{f.name}</div>
                      <div className="muted">
                        {fmtBytes(f.size)} · {when(f.modified)}
                      </div>
                    </button>
                  ))}
                </div>
              );
            })}
          </div>
          <pre className="logview">{loading ? "Loading…" : text || (sel ? "(empty)" : "")}</pre>
        </div>
        <div className="mfoot">
          <button className="btn" onClick={() => id && api.openInstanceFolder(id).catch((e) => toast(errText(e), "error"))}>
            <Icon name="folder" /> Open instance folder
          </button>
          <button className="btn" onClick={() => void refresh()}>
            <Icon name="refresh" /> Refresh
          </button>
          <div className="spacer" />
          <button
            className="btn"
            disabled={!text}
            onClick={() => navigator.clipboard?.writeText(text).then(() => toast("Log copied"))}
          >
            <Icon name="copy" /> Copy log
          </button>
          <button className="btn" onClick={closeModal}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
