import { useEffect } from "react";
import { progressPct } from "../lib/format";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";

const CHOICES = [8, 17, 21, 25];

export function JavaPage() {
  const java = useApp((s) => s.java);
  const loading = useApp((s) => s.javaLoading);
  const refresh = useApp((s) => s.refreshJava);
  const install = useApp((s) => s.installJava);
  const progress = useApp((s) => s.progress["java"]);

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pct = progressPct(progress);

  return (
    <div className="page">
      <div className="card">
        <h3>How Java works here</h3>
        <div style={{ color: "var(--muted)" }}>
          Each instance picks the Java its Minecraft version needs. If none is installed, SoloCraft downloads a portable
          Eclipse Temurin into its own folder, so nothing changes on your system. You can also pre-install one below.
        </div>
      </div>

      <div className="card">
        <h3>Download Java</h3>
        <div className="row">
          {CHOICES.map((m) => (
            <button
              key={m}
              className="btn"
              onClick={() => install(m)}
              disabled={!!progress || java.some((j) => j.major === m && j.managed)}
            >
              <Icon name="download" /> Java {m}
              {java.some((j) => j.major === m && j.managed) ? " (installed)" : ""}
            </button>
          ))}
        </div>
        {progress && (
          <div className="status">
            {progress.stage}
            {pct !== null && (
              <div className="bar">
                <i style={{ width: pct + "%" }} />
              </div>
            )}
          </div>
        )}
      </div>

      <div className="card">
        <div className="row" style={{ marginBottom: 6 }}>
          <h3 style={{ margin: 0 }}>Detected Java installs</h3>
          <div className="spacer" />
          <button className="btn" onClick={refresh} disabled={loading}>
            <Icon name="refresh" /> {loading ? "Scanning…" : "Rescan"}
          </button>
        </div>
        {java.length === 0 && !loading && (
          <div className="empty" style={{ padding: 14 }}>
            No Java found. It will be downloaded automatically when you launch.
          </div>
        )}
        {java.map((j) => (
          <div className="li" key={j.path}>
            <div className="grow">
              <div>
                Java {j.major} <span style={{ color: "var(--muted)" }}>· {j.version}</span>{" "}
                {j.managed && <span className="badge acc">SoloCraft</span>}{" "}
                {!j.arch.includes("64") && <span className="badge">32-bit</span>}
              </div>
              <div className="sub" title={j.path}>
                {j.vendor || "Unknown vendor"} · {j.path}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
