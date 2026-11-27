import { useMemo } from "react";
import { initials, loaderLabel } from "../lib/format";
import { isActive, useApp } from "../store/app";
import { Icon } from "./Icon";
import { Console } from "./Console";
import { DetailsPanel } from "./DetailsPanel";

export function InstancesPage() {
  const instances = useApp((s) => s.instances);
  const settings = useApp((s) => s.settings);
  const group = useApp((s) => s.group);
  const setGroup = useApp((s) => s.setGroup);
  const search = useApp((s) => s.search);
  const setSearch = useApp((s) => s.setSearch);
  const selectedId = useApp((s) => s.selectedId);
  const select = useApp((s) => s.select);
  const states = useApp((s) => s.states);
  const launch = useApp((s) => s.launch);
  const openModal = useApp((s) => s.openModal);
  const consoleOpen = useApp((s) => s.consoleOpen);

  const groups = useMemo(() => {
    const names = ["All", ...settings.groups];
    for (const i of instances) if (i.group && !names.includes(i.group)) names.push(i.group);
    return names;
  }, [settings.groups, instances]);

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    return instances.filter(
      (i) =>
        (group === "All" || i.group === group) &&
        (q === "" || i.name.toLowerCase().includes(q) || i.mcVersion.toLowerCase().includes(q))
    );
  }, [instances, group, search]);

  const count = (g: string) => (g === "All" ? instances.length : instances.filter((i) => i.group === g).length);

  return (
    <>
      <div className="sidebar">
        <h4>Groups</h4>
        {groups.map((g) => (
          <button key={g} className={"gitem" + (group === g ? " on" : "")} onClick={() => setGroup(g)}>
            <span>{g}</span>
            <span>{count(g)}</span>
          </button>
        ))}
      </div>
      <div className="center">
        <div className="toolbar">
          <input
            className="search"
            type="text"
            placeholder="Search instances"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <div className="spacer" />
          <button className="btn primary" onClick={() => openModal("scan")}>
            <Icon name="refresh" /> Scan this PC
          </button>
          <button className="btn" onClick={() => openModal("new")}>
            <Icon name="plus" /> New instance
          </button>
        </div>
        {shown.length === 0 ? (
          <div className="empty">
            <h3>{instances.length === 0 ? "Start your first instance" : "No instances match"}</h3>
            {instances.length === 0
              ? "Pick a Minecraft version and SM Craft sets up the rest."
              : "Try another group or search."}
            {instances.length === 0 && (
              <div style={{ marginTop: 10 }}>
                <button className="btn primary" onClick={() => openModal("new")}>
                  <Icon name="plus" /> Add instance
                </button>
              </div>
            )}
          </div>
        ) : (
          <div className="grid">
            {shown.map((i) => (
              <button
                key={i.id}
                className={"tile" + (selectedId === i.id ? " sel" : "")}
                onClick={() => select(i.id)}
                onDoubleClick={() => launch(i.id)}
                title="Double-click to launch"
              >
                {isActive(states, i.id) && <span className="dot" />}
                <div className="ico" style={{ background: i.icon }}>
                  {initials(i.name)}
                </div>
                <div className="nm">{i.name}</div>
                <div className="sub">{loaderLabel(i)}</div>
              </button>
            ))}
          </div>
        )}
        {consoleOpen && selectedId && <Console instanceId={selectedId} />}
      </div>
      <DetailsPanel />
    </>
  );
}
