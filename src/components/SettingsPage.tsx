import { useState } from "react";
import { api, DeviceCode, errText, Settings, SystemReport } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "./Icon";
import { StorageCard } from "./Storage";
import { useUpdate } from "./UpdateBanner";

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return <button className={"toggle" + (on ? " on" : "")} onClick={() => onChange(!on)} aria-pressed={on} />;
}

const THEMES: { id: Settings["theme"]; label: string; swatch: string }[] = [
  { id: "sm-night", label: "SM Craft Night", swatch: "#15121c" },
  { id: "sm-day", label: "SM Craft Day", swatch: "#efe6d8" },
];
const ACCENTS = ["#4ade80", "#60a5fa", "#f472b6", "#fbbf24", "#a78bfa", "#fb923c"];

export function SettingsPage() {
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const accounts = useApp((s) => s.accounts);
  const addAccount = useApp((s) => s.addAccount);
  const setActive = useApp((s) => s.setActiveAccount);
  const removeAccount = useApp((s) => s.removeAccount);
  const toast = useApp((s) => s.toast);
  const upd = useUpdate();
  const [username, setUsername] = useState("");
  const [newGroup, setNewGroup] = useState("");
  const [device, setDevice] = useState<DeviceCode | null>(null);
  const [verifying, setVerifying] = useState<string | null>(null);
  const [systemReport, setSystemReport] = useState<SystemReport | null>(null);
  const [scanning, setScanning] = useState(false);

  const scanSystem = async () => {
    setScanning(true);
    try { setSystemReport(await api.systemScan()); }
    catch (e) { toast(errText(e), "error"); }
    finally { setScanning(false); }
  };

  const startMs = async () => {
    try {
      const d = await api.msBeginLogin();
      setDevice(d);
      const accounts = await api.msFinishLogin(d);
      useApp.setState({ accounts });
      toast("Microsoft account added");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setDevice(null);
    }
  };
  const cancelMs = () => {
    void api.msCancelLogin();
    setDevice(null);
  };
  const verify = async (id: string) => {
    setVerifying(id);
    try {
      toast(await api.msVerify(id));
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setVerifying(null);
    }
  };

  const add = async () => {
    if (await addAccount(username)) setUsername("");
  };
  const addGroup = () => {
    const g = newGroup.trim();
    if (!g || settings.groups.includes(g)) return;
    void save({ groups: [...settings.groups, g] });
    setNewGroup("");
  };

  return (
    <div className="page settingsgrid">
      <div className="card wide">
        <h3>Accounts</h3>
        {accounts.accounts.length === 0 && (
          <div style={{ color: "var(--muted)", marginBottom: 8 }}>Add an offline account to start playing.</div>
        )}
        {accounts.accounts.map((a) => (
          <div className="li" key={a.id}>
            <Icon name="user" />
            <div className="grow">
              {a.username} {a.id === accounts.active && <span className="badge acc">Active</span>}
              <div className="sub">{a.kind === "microsoft" ? "Microsoft account (official)" : "Offline account"}</div>
            </div>
            {a.kind === "microsoft" && (
              <button className="btn" onClick={() => verify(a.id)} disabled={verifying === a.id}>
                {verifying === a.id ? "Checking…" : "Verify"}
              </button>
            )}
            {a.id !== accounts.active && (
              <button className="btn" onClick={() => setActive(a.id)}>
                Make active
              </button>
            )}
            <button className="btn danger" onClick={() => removeAccount(a.id)}>
              <Icon name="trash" />
            </button>
          </div>
        ))}
        <div className="row" style={{ marginTop: 8 }}>
          <input
            type="text"
            placeholder="Username"
            value={username}
            maxLength={16}
            onChange={(e) => setUsername(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
            style={{ width: 200 }}
          />
          <button className="btn primary" onClick={add}>
            <Icon name="plus" /> Add offline account
          </button>
        </div>
        <div className="msbox">
          <strong>Microsoft account (official)</strong>
          <div className="muted" style={{ margin: "3px 0 6px" }}>
            Needed for online servers. Sign-in is just a code: SM Craft shows you a code, you type it on the page shown, in any browser (a private window keeps your other logins untouched).
          </div>
          <div className="row wrap">
            <button className="btn primary" onClick={startMs} disabled={!!device}>
              Sign in with Microsoft
            </button>
          </div>
          {device && (
            <div className="devicebox">
              <div className="muted">Open {device.verificationUri} in any browser (a private window works) and enter this code:</div>
              <div className="usercode">{device.userCode}</div>
              <div className="row">
                <button className="btn" onClick={() => navigator.clipboard?.writeText(device.userCode)}>
                  Copy code
                </button>
                <button className="btn" onClick={() => api.openLoginPage(device.verificationUri)}>
                  Open page
                </button>
                <button className="btn" onClick={() => navigator.clipboard?.writeText(device.verificationUri)}>
                  Copy link
                </button>
                <button className="btn danger" onClick={cancelMs}>
                  Cancel
                </button>
                <span className="muted">Waiting for you to enter the code…</span>
              </div>
            </div>
          )}
        </div>
        <div className="muted" style={{ marginTop: 6 }}>
          Offline accounts cannot join online-mode servers.
        </div>
      </div>

      <StorageCard />

      <div className="card wide">
        <h3>System scan &amp; diagnosis</h3>
        <p className="muted">Checks hardware, Windows/Linux graphics-driver status, free space, and Java compatibility. This is a read-only system check; it does not scan personal files.</p>
        <button className="btn primary" onClick={scanSystem} disabled={scanning}>{scanning ? "Scanning this device…" : "Scan this device"}</button>
        {systemReport && <div style={{ marginTop: 12 }}>
          <div><strong>{systemReport.os}</strong> · {systemReport.architecture}</div>
          <div className="muted">Processor: {systemReport.processor || "Not identified"} · {systemReport.physicalCores || "?"} cores / {systemReport.hardware.threads} logical threads</div>
          <div className="muted">RAM: {Math.round(systemReport.hardware.ramMb / 1024 * 10) / 10} GB total, {Math.round(systemReport.availableRamMb / 1024 * 10) / 10} GB available</div>
          <div className="muted">Graphics: {systemReport.hardware.gpu || "Not identified"}{systemReport.hardware.discreteGpu ? " · dedicated GPU detected" : " · integrated/unknown GPU"}</div>
          {systemReport.graphicsDrivers && <div className="muted">Graphics driver: {systemReport.graphicsDrivers}</div>}
          <div className="muted">Launcher drive: {systemReport.freeDiskMb != null ? `${Math.round(systemReport.freeDiskMb / 1024 * 10) / 10} GB free` : "Free space unavailable"}</div>
          <div className="muted">Java: {systemReport.java.length ? systemReport.java.map(j => `Java ${j.major} ${j.arch || ""} (${j.vendor || "unknown vendor"})`).join(", ") : "No Java installation detected"}</div>
          <ul>{systemReport.diagnoses.map((d, i) => <li key={i}>{d}</li>)}</ul>
        </div>}
      </div>

      <div className="card">
        <h3>Appearance</h3>
        <div className="themes">
          {THEMES.map((t) => (
            <button
              key={t.id}
              className={"themecard" + (settings.theme === t.id ? " on" : "")}
              onClick={() => save({ theme: t.id })}
            >
              <i style={{ background: t.swatch }} />
              {t.label}
            </button>
          ))}
        </div>
        <div className="field" style={{ marginTop: 12 }}>
          <label>Accent color</label>
          <div className="row">
            <input type="color" value={settings.accent} onChange={(e) => save({ accent: e.target.value })} />
            {ACCENTS.map((c) => (
              <button
                key={c}
                aria-label={"Accent " + c}
                onClick={() => save({ accent: c })}
                style={{
                  width: 18,
                  height: 18,
                  borderRadius: "50%",
                  background: c,
                  border: settings.accent === c ? "2px solid var(--text)" : "1px solid var(--border)",
                  cursor: "pointer",
                }}
              />
            ))}
          </div>
        </div>
        <div className="field">
          <label>Grid density</label>
          <div className="row">
            {(["comfortable", "compact"] as const).map((d) => (
              <button
                key={d}
                className={"pill" + (settings.gridDensity === d ? " on" : "")}
                onClick={() => save({ gridDensity: d })}
              >
                {d[0].toUpperCase() + d.slice(1)}
              </button>
            ))}
          </div>
        </div>
        <div className="field">
          <label>Lite mode</label>
          <div className="row">
            <Toggle on={settings.liteMode} onChange={(v) => save({ liteMode: v })} />
            <span style={{ color: "var(--muted)" }}>Turns off blur and animations for weaker PCs</span>
          </div>
        </div>
      </div>

      <div className="card">
        <h3>Defaults for new instances</h3>
        <div className="field">
          <label>Minimum RAM</label>
          <div className="row">
            <input
              type="range"
              min={256}
              max={16384}
              step={256}
              value={settings.defaultMinRamMb}
              onChange={(e) => save({ defaultMinRamMb: Number(e.target.value) })}
              style={{ flex: 1 }}
            />
            <span style={{ width: 60, textAlign: "right" }}>{settings.defaultMinRamMb} MB</span>
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
              value={settings.defaultMaxRamMb}
              onChange={(e) => save({ defaultMaxRamMb: Number(e.target.value) })}
              style={{ flex: 1 }}
            />
            <span style={{ width: 60, textAlign: "right" }}>{settings.defaultMaxRamMb} MB</span>
          </div>
        </div>
      </div>

      <div className="card">
        <h3>Groups</h3>
        <div className="row" style={{ flexWrap: "wrap", marginBottom: 8 }}>
          {settings.groups.map((g) => (
            <span key={g} className="badge" style={{ display: "inline-flex", gap: 4, alignItems: "center" }}>
              {g}
              <button
                className="btn ghost"
                style={{ height: 14, padding: 0, border: "none" }}
                aria-label={"Remove " + g}
                onClick={() => save({ groups: settings.groups.filter((x) => x !== g) })}
              >
                <Icon name="x" />
              </button>
            </span>
          ))}
        </div>
        <div className="row">
          <input
            type="text"
            placeholder="New group"
            value={newGroup}
            onChange={(e) => setNewGroup(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addGroup()}
            style={{ width: 200 }}
          />
          <button className="btn" onClick={addGroup}>
            <Icon name="plus" /> Add group
          </button>
        </div>
      </div>

      <div className="card">
        <h3>Updates</h3>
        <div style={{ marginBottom: 8 }}>
          {upd.error
            ? "Could not check for updates right now."
            : !upd.info
            ? "Checking…"
            : upd.info.available
            ? `Version ${upd.info.latest} is available (you have ${upd.info.current}).`
            : `You are up to date (version ${upd.info.current}).`}
        </div>
        <div className="row">
          <button className="btn" onClick={upd.check} disabled={upd.checking}>
            {upd.checking ? "Checking…" : "Check for updates"}
          </button>
          {upd.info?.available && (
            <button className="btn primary" onClick={() => api.openUpdatePage(upd.info!.downloadUrl).catch(() => undefined)}>
              Download update
            </button>
          )}
        </div>
      </div>

      <div className="card">
        <h3>Data</h3>
        <button
          className="btn"
          onClick={async () => {
            try {
              await api.openDataFolder();
            } catch (e) {
              toast(errText(e), "error");
            }
          }}
        >
          <Icon name="folder" /> Open SM Craft data folder
        </button>
      </div>
    </div>
  );
}
