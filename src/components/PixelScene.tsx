import { useEffect, useRef } from "react";

interface Layer {
  base: number;
  amp: number;
  f: number;
  c: string;
  top?: string;
  trees?: boolean;
  trunk?: string;
  leaf?: string;
}
interface Palette {
  sky: string[];
  stars?: boolean;
  sun?: [number, number];
  sunc?: string;
  sunc2?: string;
  cloud: string;
  layers: Layer[];
}

export const PALETTES: Record<string, Palette> = {
  sunset: {
    sky: ["#2a1b4d", "#8a2f6b", "#e0566a", "#ff9a56", "#ffd27a"],
    stars: true,
    sun: [0.72, 0.42],
    sunc: "#fff1b8",
    sunc2: "#ffe27a",
    cloud: "rgba(255,170,150,.55)",
    layers: [
      { base: 0.62, amp: 9, f: 14, c: "#5a2a62" },
      { base: 0.74, amp: 7, f: 10, c: "#3a1d52", trees: true, trunk: "#2a1540", leaf: "#2a1540" },
      { base: 0.88, amp: 3, f: 8, c: "#1d1038", trees: true, trunk: "#150a2a", leaf: "#150a2a" },
    ],
  },
  day: {
    sky: ["#1f6fb5", "#7cc4f0"],
    sun: [0.8, 0.15],
    sunc: "#fff",
    sunc2: "#fff8c4",
    cloud: "#fff",
    layers: [
      { base: 0.6, amp: 3, f: 9, c: "#5aa84b", top: "#7fd05e", trees: true, trunk: "#6b4a2a", leaf: "#2f7a3a" },
      { base: 0.85, amp: 2, f: 7, c: "#3f7f35", top: "#5ab048" },
    ],
  },
  desert: {
    sky: ["#d77a2f", "#f6c56a"],
    sun: [0.2, 0.2],
    sunc: "#fff",
    sunc2: "#ffe9a0",
    cloud: "rgba(255,255,255,.6)",
    layers: [
      { base: 0.62, amp: 4, f: 12, c: "#c9a35a", top: "#e3c277" },
      { base: 0.82, amp: 3, f: 8, c: "#a7803f", top: "#c9a35a" },
    ],
  },
  night: {
    sky: ["#0d1b3e", "#2c4d8f"],
    stars: true,
    sun: [0.15, 0.2],
    sunc: "#e8eefc",
    sunc2: "#ffffff",
    cloud: "rgba(150,170,220,.4)",
    layers: [
      { base: 0.65, amp: 5, f: 11, c: "#2c6b6a", top: "#43a39b", trees: true, trunk: "#2a2a3a", leaf: "#1f5a52" },
      { base: 0.86, amp: 2, f: 8, c: "#1f4a52", top: "#2c6b6a" },
    ],
  },
};
const ORDER = ["day", "desert", "night", "sunset"];

export function paletteFor(key: string): string {
  let h = 0;
  for (const ch of key) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return ORDER[h % ORDER.length];
}

function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

function draw(c: HTMLCanvasElement, P: Palette, seed: number, clouds: number) {
  const x = c.getContext("2d");
  if (!x) return;
  const W = c.width;
  const H = c.height;
  const r = rng(seed);
  const g = x.createLinearGradient(0, 0, 0, H);
  P.sky.forEach((s, i) => g.addColorStop(P.sky.length === 1 ? 0 : i / (P.sky.length - 1), s));
  x.fillStyle = g;
  x.fillRect(0, 0, W, H);
  if (P.stars) {
    for (let i = 0; i < (W * H) / 70; i++) {
      x.fillStyle = `rgba(255,255,255,${0.3 + r() * 0.6})`;
      x.fillRect(Math.floor(r() * W), Math.floor(r() * H * 0.5), 1, 1);
    }
  }
  if (P.sun) {
    const sx = Math.floor(W * P.sun[0]);
    const sy = Math.floor(H * P.sun[1]);
    const s = Math.max(5, Math.floor(H / 9));
    x.fillStyle = P.sunc ?? "#fff";
    x.fillRect(sx, sy, s, s);
    x.fillStyle = P.sunc2 ?? "#ffe";
    x.fillRect(sx + 1, sy + 1, s - 2, s - 2);
  }
  for (let i = 0; i < clouds; i++) {
    const cx = Math.floor(r() * W);
    const cy = Math.floor(H * 0.1 + r() * H * 0.25);
    const w = 8 + Math.floor(r() * 14);
    x.fillStyle = P.cloud;
    x.fillRect(cx, cy, w, 2);
    x.fillRect(cx + 2, cy - 1, w - 5, 1);
  }
  for (const L of P.layers) {
    const ph = r() * 10;
    const yAt = (i: number) =>
      Math.floor(H * L.base + Math.sin(i / L.f + ph) * L.amp + Math.sin(i / (L.f * 2.3) + ph * 2) * L.amp * 0.6);
    for (let i = 0; i < W; i++) {
      const y = yAt(i);
      x.fillStyle = L.c;
      x.fillRect(i, y, 1, H - y);
      if (L.top) {
        x.fillStyle = L.top;
        x.fillRect(i, y, 1, Math.max(1, Math.floor(H / 40)));
      }
    }
    if (L.trees) {
      for (let i = 0; i < W; i += 6 + Math.floor(r() * 9)) {
        const y = yAt(i);
        const t = 2 + Math.floor(r() * 3);
        x.fillStyle = L.trunk ?? "#432";
        x.fillRect(i, y - t, 1, t);
        x.fillStyle = L.leaf ?? "#2a6";
        x.fillRect(i - 1, y - t - 2, 3, 2);
        x.fillRect(i - 2, y - t - 1, 5, 1);
      }
    }
  }
}

/** A small pixel-art landscape drawn with the canvas (no image files needed). */
export function PixelScene({
  palette,
  seed = 7,
  w = 120,
  h = 30,
  clouds = 3,
  className,
}: {
  palette: string;
  seed?: number;
  w?: number;
  h?: number;
  clouds?: number;
  className?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) draw(ref.current, PALETTES[palette] ?? PALETTES.sunset, seed, clouds);
  }, [palette, seed, clouds, w, h]);
  return <canvas ref={ref} width={w} height={h} className={"pixscene " + (className ?? "")} />;
}
