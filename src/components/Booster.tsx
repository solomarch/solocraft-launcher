import { useCallback, useEffect, useState } from "react";
import { api, BoostPlan, errText } from "../lib/api";
import { useApp } from "../store/app";
import { Icon } from "./Icon";

const TIERS: { id: string; name: string; text: string }[] = [
  { id: "potato", name: "Potato", text: "Weak or old PC: short distances, minimal particles, lowest memory use." },
  { id: "balanced", name: "Balanced", text: "Mid-range PC: smooth with good looks." },
  { id: "performance", name: "Performance+", text: "Strong PC: maximum FPS and a long view distance." },
];

const MODS = ["Sodium", "Sodium Extra", "Lithium", "FerriteCore", "ModernFix", "ImmediatelyFast", "Entity Culling", "More Culling", "Dynamic FPS"];

export function Booster({ instId, running, onDone }: { instId: string; running: boolean; onDone: () => void }) {
  const toast = useApp((s) => s.toast);
  const [tier, setTier] = useState<string | null>(null);
  const [plan, setPlan] = useState<BoostPlan | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(
    async (t: string | null) => {
      try {
        const p = await api.boosterPlan(instId, t);
        setPlan(p);
        setTier((cur) => cur ?? p.recommended);
      } catch (e) {
        toast(errText(e), "error");
      }
    },
    [instId, toast],
  );
  useEffect(() => {
    void load(tier);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instId]);

  const pick = (t: string) => {
    setTier(t);
    void load(t);
  };

  const apply = async () => {
    setBusy(true);
    try {
      const r = await api.boosterApply(instId, tier);
      const n = r.mods?.installed.length ?? 0;
      toast(
        `FPS Booster applied (${TIERS.find((x) => x.id === r.plan.tier)?.name}).` +
          (n ? ` ${n} mod${n === 1 ? "" : "s"} installed.` : "") +
          (r.notes.length ? " " + r.notes.join(" ") : ""),
      );
      await load(tier);
      onDone();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const undo = async () => {
    setBusy(true);
    try {
      await api.boosterUndo(instId);
      toast("FPS Booster undone: your old settings are back and its mods were removed.");
      await load(tier);
      onDone();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  if (!plan) return <div className="card muted">Looking at your PC…</div>;
  const hw = plan.hardware;
  const opt = (k: string) => plan.options.find((o) => o[0] === k)?.[1] ?? "";
  return (
    <div className="card" style={{ marginBottom: 10 }}>
      <h3 style={{ marginTop: 0 }}>
        <Icon name="bolt" /> FPS Booster
      </h3>
      <div className="muted" style={{ marginBottom: 8 }}>
        One click: installs the FPS mods and writes the right settings for this PC. You never touch a config file, and Undo puts everything back.
      </div>
      <div className="muted" style={{ marginBottom: 8 }}>
        Detected: {hw.gpu || "graphics card not detected"} · {hw.ramMb ? Math.round(hw.ramMb / 1024) + " GB RAM" : "RAM not detected"} · {hw.threads} CPU threads
      </div>
      <div className="tiers">
        {TIERS.map((t) => (
          <div
            key={t.id}
            role="button"
            tabIndex={0}
            className={"tier" + (tier === t.id ? " on" : "")}
            onClick={() => pick(t.id)}
            onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && pick(t.id)}
          >
            <div className="tier-name">{t.name}</div>
            {plan.recommended === t.id && <div className="tier-rec">Recommended for this PC</div>}
            <div className="tier-text">{t.text}</div>
          </div>
        ))}
      </div>
      <div className="muted" style={{ lineHeight: 1.6 }}>
        Memory for the game: {Math.round(plan.maxRamMb / 102.4) / 10} GB (set automatically, never more than 60% of your RAM) · view distance {plan.renderDistance}, simulation {plan.simulationDistance} ·
        particles {opt("particles") === "2" ? "minimal" : "reduced"} · clouds off · VSync off · max FPS {opt("maxFps") === "260" ? "unlimited" : opt("maxFps")}
        <br />
        Mods: {MODS.join(", ")} (Fabric / Quilt instances). Sodium is also tuned for your CPU.
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <button className="btn primary" onClick={apply} disabled={busy || running}>
          <Icon name="bolt" /> {busy ? "Working…" : plan.alreadyBoosted ? "Apply again" : "Apply FPS Boost"}
        </button>
        {plan.alreadyBoosted && (
          <button className="btn" onClick={undo} disabled={busy || running}>
            Undo
          </button>
        )}
        {running && <span className="muted">Stop the game first.</span>}
      </div>
    </div>
  );
}
