import { useEffect, useState } from "react";
import { api, UpdateInfo } from "../lib/api";
import { Icon } from "./Icon";

const DISMISS_KEY = "solocraft.update.dismissed";

/** Checks GitHub for a newer launcher now and every 6 hours; shows a slim banner when one exists. */
export function useUpdate() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = async () => {
    setChecking(true);
    setError(null);
    try {
      setInfo(await api.checkUpdate());
    } catch (e) {
      setError(typeof e === "string" ? e : "Could not reach GitHub");
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    void check();
    const t = setInterval(check, 6 * 60 * 60 * 1000);
    return () => clearInterval(t);
  }, []);

  return { info, checking, error, check };
}

export function UpdateBanner({ info }: { info: UpdateInfo | null }) {
  const [hidden, setHidden] = useState<string | null>(null);
  useEffect(() => {
    try {
      setHidden(localStorage.getItem(DISMISS_KEY));
    } catch {
      /* ignore */
    }
  }, []);
  if (!info || !info.available || hidden === info.latest) return null;
  const dismiss = () => {
    setHidden(info.latest);
    try {
      localStorage.setItem(DISMISS_KEY, info.latest);
    } catch {
      /* ignore */
    }
  };
  return (
    <div className="updatebar">
      <Icon name="download" />
      <span>
        <strong>SM Craft {info.latest}</strong> is available (you have {info.current}).
      </span>
      <span className="grow" />
      <button className="btn primary" onClick={() => api.openUpdatePage(info.downloadUrl).catch(() => undefined)}>
        Download update
      </button>
      <button className="btn" onClick={() => api.openUpdatePage(info.pageUrl).catch(() => undefined)}>
        What's new
      </button>
      <button className="btn ghost" aria-label="Dismiss" onClick={dismiss}>
        <Icon name="x" />
      </button>
    </div>
  );
}
