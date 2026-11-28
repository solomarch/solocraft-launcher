import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../store/app";
import { Icon } from "./Icon";

const EMPTY: never[] = [];

export function Console({ instanceId }: { instanceId: string }) {
  const lines = useApp((s) => s.logs[instanceId]) ?? EMPTY;
  const gstate = useApp((s) => s.states[instanceId]);
  const clearLogs = useApp((s) => s.clearLogs);
  const toggleConsole = useApp((s) => s.toggleConsole);
  const toast = useApp((s) => s.toast);

  const [show, setShow] = useState({ info: true, warn: true, error: true });
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    const out = lines.filter((l) => show[l.level] && (q === "" || l.text.toLowerCase().includes(q)));
    return out.length > 1500 ? out.slice(out.length - 1500) : out;
  }, [lines, show, query]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [visible, follow]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(lines.map((l) => l.text).join("\n"));
      toast("Log copied");
    } catch {
      toast("Could not copy the log", "error");
    }
  };

  const bad = gstate && (gstate.state === "crashed" || gstate.state === "error");

  return (
    <div className="console">
      <div className="head">
        <Icon name="terminal" />
        <b>Console</b>
        {(["info", "warn", "error"] as const).map((lv) => (
          <button
            key={lv}
            className={"pill " + lv + (show[lv] ? " on" : "")}
            onClick={() => setShow({ ...show, [lv]: !show[lv] })}
          >
            {lv[0].toUpperCase() + lv.slice(1)}
          </button>
        ))}
        <input
          type="text"
          placeholder="Search log"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{ height: 20, width: 150 }}
        />
        <button className={"pill" + (follow ? " on" : "")} onClick={() => setFollow(!follow)}>
          Follow
        </button>
        <div className="spacer" />
        <button className="btn ghost" style={{ height: 20 }} onClick={copy}>
          <Icon name="copy" /> Copy
        </button>
        <button className="btn ghost" style={{ height: 20 }} onClick={() => clearLogs(instanceId)}>
          Clear
        </button>
        <button className="btn ghost" style={{ height: 20 }} onClick={() => toggleConsole(false)} aria-label="Close console">
          <Icon name="x" />
        </button>
      </div>
      {bad && <div className="diag bad">{gstate.detail}</div>}
      <div className="lines" ref={box}>
        {visible.length === 0 ? (
          <span className="l-info" style={{ opacity: 0.6 }}>
            The game log appears here while Minecraft runs.
          </span>
        ) : (
          visible.map((l, i) => (
            <div key={i} className={"l-" + l.level}>
              {l.text}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
