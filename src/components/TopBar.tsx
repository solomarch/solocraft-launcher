import { useApp, Page } from "../store/app";
import { Icon } from "./Icon";

const NAV: { id: Page; label: string; icon: string }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "instances", label: "Instances", icon: "grid" },
  { id: "java", label: "Java", icon: "coffee" },
  { id: "settings", label: "Settings", icon: "sliders" },
];

export function TopBar() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const openModal = useApp((s) => s.openModal);
  const accounts = useApp((s) => s.accounts);
  const active = accounts.accounts.find((a) => a.id === accounts.active) ?? accounts.accounts[0];

  return (
    <div className="topbar">
      <div className="brand">
        <Icon name="cube" /> SM Craft
      </div>
      {NAV.map((n) => (
        <button key={n.id} className={"nav" + (page === n.id ? " on" : "")} onClick={() => setPage(n.id)}>
          <Icon name={n.icon} /> {n.label}
        </button>
      ))}
      <div className="spacer" />
      <button className="btn primary" onClick={() => openModal("new")}>
        <Icon name="plus" /> Add instance
      </button>
      <button className="chip-acc" onClick={() => setPage("settings")} title="Manage accounts">
        <Icon name="user" /> {active ? active.username : "Add account"}
      </button>
    </div>
  );
}
