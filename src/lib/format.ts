import { GameState, Instance, Progress } from "../lib/api";

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}

export function loaderLabel(i: Instance): string {
  const l = i.loader === "vanilla" ? "Vanilla" : i.loader[0].toUpperCase() + i.loader.slice(1);
  return `${i.mcVersion} · ${l}`;
}

export function formatPlayTime(secs: number): string {
  if (secs < 60) return secs > 0 ? "<1 min" : "never played";
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min`;
}

export function stateText(s: GameState | undefined): string {
  if (!s) return "";
  switch (s.state) {
    case "preparing":
      return s.detail || "Preparing";
    case "downloading":
      return "Downloading game files";
    case "launching":
      return "Starting Minecraft";
    case "running":
      return "Running";
    case "crashed":
      return "Closed unexpectedly";
    case "error":
      return "Could not start";
    default:
      return "";
  }
}

export function progressPct(p: Progress | undefined): number | null {
  if (!p || p.total === 0) return null;
  return Math.min(100, Math.round((p.current / p.total) * 100));
}

export function fmtEta(sec: number | null | undefined): string {
  if (sec === null || sec === undefined) return "";
  if (sec < 5) return "a few seconds left";
  if (sec < 90) return `about ${Math.round(sec / 5) * 5} s left`;
  if (sec < 3600) return `about ${Math.round(sec / 60)} min left`;
  return `about ${(sec / 3600).toFixed(1)} h left`;
}
