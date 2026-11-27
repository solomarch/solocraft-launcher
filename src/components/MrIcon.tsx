import { useState } from "react";
import { initials } from "../lib/format";

export function MrIcon({ url, title, size = 36 }: { url: string | null | undefined; title: string; size?: number }) {
  const [bad, setBad] = useState(false);
  const style = { width: size, height: size };
  if (!url || bad) {
    return (
      <div className="mricon fallback" style={style}>
        {initials(title)}
      </div>
    );
  }
  return <img className="mricon" style={style} src={url} alt="" loading="lazy" onError={() => setBad(true)} />;
}

export function compact(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(n >= 10_000 ? 0 : 1) + "k";
  return String(n);
}
