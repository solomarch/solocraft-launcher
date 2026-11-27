import { useApp, Page } from "../store/app";
import { Icon } from "./Icon";

const NAV: { id: Page; label: string; icon: string }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "instances", label: "Instances", icon: "grid" },
  { id: "java", label: "Java", icon: "coffee" },
  { id: "settings", label: "Settings", icon: "sliders" },
];

export function Rail() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const openModal = useApp((s) => s.openModal);
  const accounts = useApp((s) => s.accounts);
  const active = accounts.accounts.find((a) => a.id === accounts.active) ?? accounts.accounts[0];

  return (
    <nav className="rail">
      <img className="rail-logo" src="/logo.png" alt="SM Craft" title="SM Craft" draggable={false} />
      {NAV.map((n) => (
        <button key={n.id} className={"rail-btn" + (page === n.id ? " on" : "")} onClick={() => setPage(n.id)} title={n.label} aria-label={n.label}>
          <Icon name={n.icon} />
          <span>{n.label}</span>
        </button>
      ))}
      <div className="rail-sp" />
      <button className="rail-btn" onClick={() => openModal("new")} title="Add instance" aria-label="Add instance">
        <Icon name="plus" />
        <span>New</span>
      </button>
      <button className="rail-avatar" onClick={() => setPage("settings")} title={active ? active.username : "Add account"}>
        {active ? active.username.slice(0, 1).toUpperCase() : "+"}
      </button>
    </nav>
  );
}
