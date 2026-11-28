import { useEffect, useState } from "react";
import { api, errText, ServerConfig, ServerInternetInfo } from "../lib/api";
import { formatPlayTime, initials, loaderLabel, progressPct, stateText } from "../lib/format";
import { isActive, useApp } from "../store/app";
import { Icon } from "./Icon";

export function DetailsPanel() {
  const id = useApp((s) => s.selectedId);
  const inst = useApp((s) => s.instances.find((i) => i.id === id));
  const states = useApp((s) => s.states);
  const progress = useApp((s) => s.progress);
  const accounts = useApp((s) => s.accounts);
  const launch = useApp((s) => s.launch);
  const kill = useApp((s) => s.kill);
  const openModal = useApp((s) => s.openModal);
  const toggleConsole = useApp((s) => s.toggleConsole);
  const deleteInstance = useApp((s) => s.deleteInstance);
  const toast = useApp((s) => s.toast);
  const [confirm, setConfirm] = useState(false);

  if (!inst) {
    return (
      <div className="details">
        <div className="empty" style={{ padding: 10 }}>
          Select an instance to see its details.
        </div>
      </div>
    );
  }

  const running = isActive(states, inst.id);
  const st = states[inst.id];
  const pr = progress[inst.id];
  const pct = progressPct(pr);
  const acc =
    accounts.accounts.find((a) => a.id === inst.accountId) ??
    accounts.accounts.find((a) => a.id === accounts.active) ??
    accounts.accounts[0];

  const openFolder = async () => {
    try {
      await api.openInstanceFolder(inst.id);
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  return (
    <div className="details">
      <div className="ico" style={{ background: inst.icon }}>
        {initials(inst.name)}
      </div>
      <div className="title">{inst.name}</div>
      <div className="meta">
        {loaderLabel(inst)}
        <br />
        {formatPlayTime(inst.playTimeSecs)}
        {acc ? ` · as ${acc.username}` : ""}
      </div>

      {running ? (
        <button className="btn danger block big" onClick={() => kill(inst.id)}>
          <Icon name="stop" /> Stop
        </button>
      ) : (
        <button className="btn primary block big" onClick={() => launch(inst.id)}>
          <Icon name="play" /> Launch
        </button>
      )}

      {(running || (st && st.state !== "stopped")) && st && (
        <div className="status">
          {stateText(st)}
          {pr && running && st.state !== "running" ? ` · ${pr.stage}` : ""}
          {pct !== null && running && st.state !== "running" && (
            <div className="bar">
              <i style={{ width: pct + "%" }} />
            </div>
          )}
        </div>
      )}

      <ServerHosting id={inst.id} loader={inst.loader} />

      <div className="stack" style={{ marginTop: 8 }}>
        <button className="btn block" onClick={() => openModal("edit")}>
          <Icon name="edit" /> Edit instance
        </button>
        <button className="btn block" onClick={() => openModal("mods")}>
          <Icon name="puzzle" /> Mods
        </button>
        <button className="btn block" onClick={() => openModal("maps")}>
          <Icon name="folder" /> Maps &amp; worlds
        </button>
        <button className="btn block" onClick={() => toggleConsole()}>
          <Icon name="terminal" /> Live console
        </button>
        <button className="btn block" onClick={() => openModal("logs")}>
          <Icon name="file" /> Log files
        </button>
        <button className="btn block" onClick={openFolder}>
          <Icon name="folder" /> Open .minecraft folder
        </button>
        {confirm ? (
          <div className="row">
            <button
              className="btn danger"
              style={{ flex: 1 }}
              onClick={() => {
                setConfirm(false);
                void deleteInstance(inst.id);
              }}
            >
              Delete
            </button>
            <button className="btn" style={{ flex: 1 }} onClick={() => setConfirm(false)}>
              Cancel
            </button>
          </div>
        ) : (
          <button className="btn danger block" onClick={() => setConfirm(true)} disabled={running}>
            <Icon name="trash" /> Delete
          </button>
        )}
      </div>
    </div>
  );
}

function ServerHosting({ id, loader }: { id: string; loader: string }) {
  const toast = useApp((s) => s.toast);
  const [config, setConfig] = useState<ServerConfig | null>(null);
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [networkBusy, setNetworkBusy] = useState(false);
  const [internetInfo, setInternetInfo] = useState<ServerInternetInfo | null>(null);
  const supported = ["", "vanilla", "fabric"].includes(loader.toLowerCase());

  useEffect(() => {
    let active = true;
    setConfig(null);
    Promise.all([api.serverConfig(id), api.serverRunning(id)]).then(([cfg, status]) => {
      if (active) { setConfig(cfg); setRunning(status); }
    }).catch((e) => { if (active) toast(errText(e), "error"); });
    const timer = window.setInterval(() => {
      api.serverRunning(id).then((status) => { if (active) setRunning(status); }).catch(() => undefined);
    }, 3000);
    return () => { active = false; window.clearInterval(timer); };
  }, [id, toast]);

  const save = async (cfg = config) => {
    if (!cfg) return;
    await api.saveServerConfig(id, cfg);
    setConfig(cfg);
  };
  const start = async () => {
    if (!config) return;
    setBusy(true);
    try {
      await save(config);
      await api.startServer(id);
      setRunning(true);
      toast("Server is starting. Watch Live console for startup logs.");
    } catch (e) { toast(errText(e), "error"); }
    finally { setBusy(false); }
  };
  const stop = async () => {
    setBusy(true);
    try { await api.stopServer(id); setRunning(false); toast("Server stopped"); }
    catch (e) { toast(errText(e), "error"); }
    finally { setBusy(false); }
  };
  const refreshInternetInfo = async () => {
    setNetworkBusy(true);
    try { setInternetInfo(await api.serverInternetInfo()); }
    catch (e) { toast(errText(e), "error"); }
    finally { setNetworkBusy(false); }
  };
  const copyAddress = async (address: string) => {
    try { await navigator.clipboard.writeText(address); toast("Join address copied"); }
    catch { toast("Could not access clipboard. Copy the address manually.", "error"); }
  };

  return <details style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 8 }}>
    <summary style={{ cursor: "pointer", fontWeight: 600 }}>Server hosting</summary>
    <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
      <div className="muted">Runs a separate server and world in this instance’s <code>server</code> folder. Supported: Vanilla and Fabric.</div>
      {!supported && <div className="muted">This instance uses {loader}; server hosting for this loader is not supported yet.</div>}
      {config && <>
        <label className="row" style={{ gap: 8 }}>
          <input type="checkbox" checked={config.onlineMode} disabled={running || busy} onChange={(e) => setConfig({ ...config, onlineMode: e.target.checked })} />
          <span>Online mode (verify Microsoft/Minecraft accounts)</span>
        </label>
        {!config.onlineMode && <div style={{ color: "var(--warning, #fbbf24)" }}>
          Offline mode accepts unverified names that can impersonate other players. Keep it private; do not expose it to the internet.
        </div>}
        <div className="row wrap">
          <label>Port <input type="number" min={1024} max={65535} value={config.port} disabled={running || busy} onChange={(e) => setConfig({ ...config, port: Number(e.target.value) })} style={{ width: 90 }} /></label>
          <label>Players <input type="number" min={1} max={100} value={config.maxPlayers} disabled={running || busy} onChange={(e) => setConfig({ ...config, maxPlayers: Number(e.target.value) })} style={{ width: 70 }} /></label>
        </div>
        <label className="row" style={{ gap: 8, alignItems: "flex-start" }}>
          <input type="checkbox" checked={config.eulaAccepted} disabled={running || busy || config.eulaAccepted} onChange={(e) => setConfig({ ...config, eulaAccepted: e.target.checked })} />
          <span>I have read and agree to the <a href="https://www.minecraft.net/eula" target="_blank" rel="noreferrer">Minecraft EULA</a>.</span>
        </label>
        <div className="muted">Players on this PC can connect to <code>localhost:{config.port}</code>. LAN/internet access may require firewall and router port-forwarding. Fabric server mods go in this server folder; client-only mods should not be copied.</div>
        <div style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 10, display: "grid", gap: 7 }}>
          <strong>Internet access</strong>
          <div className="muted">Find your addresses, then forward TCP port {config.port} on your router to this PC’s LAN address and port {config.port}. Allow Java through your firewall when prompted. This launcher does not change router or firewall settings.</div>
          <div className="muted">Address lookup contacts api.ipify.org to learn the public IP address visible to the internet.</div>
          <div className="row wrap">
            <button className="btn" disabled={networkBusy} onClick={refreshInternetInfo}>{networkBusy ? "Checking…" : "Show connection addresses"}</button>
            {internetInfo?.publicIp && <button className="btn" onClick={() => copyAddress(`${internetInfo.publicIp}:${config.port}`)}>Copy internet address</button>}
            {internetInfo?.lanIp && <button className="btn" onClick={() => copyAddress(`${internetInfo.lanIp}:${config.port}`)}>Copy LAN address</button>}
          </div>
          {internetInfo && <div className="muted" style={{ display: "grid", gap: 3 }}>
            <span>LAN: <code>{internetInfo.lanIp ? `${internetInfo.lanIp}:${config.port}` : "unavailable"}</code></span>
            <span>Internet: <code>{internetInfo.publicIp ? `${internetInfo.publicIp}:${config.port}` : "could not detect public IP"}</code></span>
            <span>After forwarding, share the Internet address with friends. If it still cannot be reached, your ISP may use CGNAT or your router may block port forwarding.</span>
          </div>}
          {!config.onlineMode && <div style={{ color: "var(--warning, #fbbf24)" }}>Do not expose an Offline mode server to the internet: player names are not authenticated.</div>}
        </div>
        <div className="row wrap">
          {!running ? <button className="btn primary" disabled={busy || !supported || !config.eulaAccepted} onClick={start}>{busy ? "Starting…" : "Start server"}</button>
            : <button className="btn danger" disabled={busy} onClick={stop}>{busy ? "Stopping…" : "Stop server"}</button>}
          <button className="btn" disabled={busy} onClick={() => save().then(() => toast("Server settings saved")).catch((e) => toast(errText(e), "error"))}>Save settings</button>
          <button className="btn" onClick={() => api.openServerFolder(id).catch((e) => toast(errText(e), "error"))}>Open server folder</button>
        </div>
      </>}
      {!config && <div className="muted">Loading server settings…</div>}
    </div>
  </details>;
}
