import { useState } from "react";
import { api, errText } from "../lib/api";
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
