import { useEffect, useState } from "react";
import { useApp } from "./store/app";
import { TopBar } from "./components/TopBar";
import { Home } from "./components/Home";
import { InstancesPage } from "./components/InstancesPage";
import { JavaPage } from "./components/JavaPage";
import { SettingsPage } from "./components/SettingsPage";
import { StatusBar } from "./components/StatusBar";
import { Toasts } from "./components/Toasts";
import { NewInstanceModal } from "./modals/NewInstanceModal";
import { EditInstanceModal } from "./modals/EditInstanceModal";
import { ModsModal } from "./modals/ModsModal";
import { LogsModal } from "./modals/LogsModal";
import { StoragePrompt } from "./components/Storage";
import { api, StorageInfo } from "./lib/api";

function readableOn(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return "#06240f";
  const n = parseInt(m[1], 16);
  const lum = (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
  return lum > 0.55 ? "#0b0b0b" : "#ffffff";
}

export default function App() {
  const ready = useApp((s) => s.ready);
  const page = useApp((s) => s.page);
  const modal = useApp((s) => s.modal);
  const settings = useApp((s) => s.settings);
  const init = useApp((s) => s.init);

  const [firstRun, setFirstRun] = useState<StorageInfo | null>(null);
  const toast = useApp((s) => s.toast);

  useEffect(() => {
    void init();
    api
      .storageInfo()
      .then((i) => {
        if (i.firstRun) setFirstRun(i);
        else if (i.warning) toast(i.warning, "error");
      })
      .catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [init]);

  useEffect(() => {
    const el = document.documentElement;
    el.dataset.theme = settings.theme;
    el.dataset.density = settings.gridDensity;
    el.dataset.lite = String(settings.liteMode);
    el.style.setProperty("--accent", settings.accent);
    el.style.setProperty("--accent-ink", readableOn(settings.accent));
  }, [settings]);

  if (!ready) return <div className="app" />;

  return (
    <div className="app">
      <TopBar />
      <div className="main">
        {page === "home" && <Home />}
        {page === "instances" && <InstancesPage />}
        {page === "java" && <JavaPage />}
        {page === "settings" && <SettingsPage />}
      </div>
      <StatusBar />
      {modal === "new" && <NewInstanceModal />}
      {modal === "edit" && <EditInstanceModal />}
      {modal === "mods" && <ModsModal />}
      {modal === "logs" && <LogsModal />}
      {firstRun && <StoragePrompt info={firstRun} onClose={() => setFirstRun(null)} />}
      <Toasts />
    </div>
  );
}
