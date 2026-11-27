import { fmtEta, progressPct } from "../lib/format";
import { ACTIVE_STATES, useApp } from "../store/app";

export function StatusBar() {
  const progress = useApp((s) => s.progress);
  const etas = useApp((s) => s.eta);
  const states = useApp((s) => s.states);
  const instances = useApp((s) => s.instances);

  const entry = Object.values(progress)[0];
  const runningCount = Object.values(states).filter((s) => s.state === "running").length;
  const busyCount = Object.values(states).filter((s) => ACTIVE_STATES.includes(s.state)).length;
  const pct = progressPct(entry);
  const eta = entry ? fmtEta(etas[entry.task]) : "";
  const name = entry ? instances.find((i) => i.id === entry.task)?.name ?? (entry.task === "storage" ? "Storage" : "Java") : "";

  return (
    <div className="statusbar">
      {entry ? (
        <>
          <span>
            {name}: {entry.stage}
          </span>
          {pct !== null && (
            <div className="bar">
              <i style={{ width: pct + "%" }} />
            </div>
          )}
          {pct !== null && <span>{pct}%</span>}
          {eta && <span className="muted">· {eta} (this step)</span>}
        </>
      ) : busyCount > 0 ? (
        <span>{runningCount > 0 ? `${runningCount} game running` : "Starting…"}</span>
      ) : (
        <span>Ready</span>
      )}
      <div className="spacer" />
      <span>{instances.length} instance{instances.length === 1 ? "" : "s"}</span>
    </div>
  );
}
