import { useEffect, useRef, useState } from "react";

const S = 10; // screen pixels per skin pixel

type Rect = [number, number, number, number];

/** Draws the front view of a Minecraft skin (64x64 or the old 64x32 layout), layers included. */
function drawSkin(c: HTMLCanvasElement, img: HTMLImageElement, slim: boolean) {
  const x = c.getContext("2d");
  if (!x) return;
  x.clearRect(0, 0, c.width, c.height);
  x.imageSmoothingEnabled = false;
  const modern = img.height >= 64;
  const aw = slim ? 3 : 4;
  const put = (src: Rect, dx: number, dy: number, flip = false) => {
    const [sx, sy, sw, sh] = src;
    if (flip) {
      x.save();
      x.translate((dx + sw) * S, dy * S);
      x.scale(-1, 1);
      x.drawImage(img, sx, sy, sw, sh, 0, 0, sw * S, sh * S);
      x.restore();
    } else {
      x.drawImage(img, sx, sy, sw, sh, dx * S, dy * S, sw * S, sh * S);
    }
  };
  // base layer
  put([8, 8, 8, 8], 4, 0); // head
  put([20, 20, 8, 12], 4, 8); // body
  put([44, 20, aw, 12], slim ? 1 : 0, 8); // right arm (left on screen)
  put([4, 20, 4, 12], 4, 20); // right leg
  if (modern) {
    put([36, 52, aw, 12], 12, 8); // left arm
    put([20, 52, 4, 12], 8, 20); // left leg
  } else {
    put([44, 20, aw, 12], 12, 8, true);
    put([4, 20, 4, 12], 8, 20, true);
  }
  // second layer (hat, jacket, sleeves, trousers)
  put([40, 8, 8, 8], 4, 0);
  if (modern) {
    put([20, 36, 8, 12], 4, 8);
    put([44, 36, aw, 12], slim ? 1 : 0, 8);
    put([52, 52, aw, 12], 12, 8);
    put([4, 36, 4, 12], 4, 20);
    put([4, 52, 4, 12], 8, 20);
  }
}

/** Default figure for accounts without a downloadable skin (offline accounts). */
function drawDefault(c: HTMLCanvasElement) {
  const x = c.getContext("2d");
  if (!x) return;
  x.clearRect(0, 0, c.width, c.height);
  const R = (col: string, a: number, b: number, w: number, h: number) => {
    x.fillStyle = col;
    x.fillRect(a * S, b * S, w * S, h * S);
  };
  const o = -1; // figure is 16 wide, this layout was drawn on 20
  R("#3b3f4a", 4 + o + 1, 20, 4, 12);
  R("#454a57", 8 + o + 1, 20, 4, 12);
  R("#ff7a45", 4 + o + 1, 30, 4, 2);
  R("#ff7a45", 8 + o + 1, 30, 4, 2);
  R("#1f6f4a", 4, 8, 8, 12);
  R("#2a8a5c", 4, 8, 8, 2);
  R("#e8f0ea", 4, 13, 8, 1);
  R("#1f6f4a", 0, 8, 4, 10);
  R("#1f6f4a", 12, 8, 4, 10);
  R("#f0c9a0", 0, 18, 4, 3);
  R("#f0c9a0", 12, 18, 4, 3);
  R("#f0c9a0", 4, 0, 8, 8);
  R("#15151c", 4, 0, 8, 2);
  R("#15151c", 4, 0, 1, 5);
  R("#15151c", 11, 0, 1, 5);
  R("#ffd23f", 5, 3, 6, 2);
  R("#c47f5a", 7, 6, 2, 1);
}

export function SkinFigure({ dataUrl, slim }: { dataUrl: string; slim: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [bad, setBad] = useState(false);

  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    setBad(false);
    if (!dataUrl) {
      drawDefault(c);
      return;
    }
    const img = new Image();
    img.onload = () => {
      if (img.width !== 64 || (img.height !== 64 && img.height !== 32)) {
        setBad(true);
        drawDefault(c);
        return;
      }
      drawSkin(c, img, slim);
    };
    img.onerror = () => {
      setBad(true);
      drawDefault(c);
    };
    img.src = dataUrl;
  }, [dataUrl, slim]);

  return <canvas ref={ref} width={16 * S} height={32 * S} className={"skinfig" + (bad ? " bad" : "")} aria-label="Player model" />;
}
