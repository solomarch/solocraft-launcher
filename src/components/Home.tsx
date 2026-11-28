import { useEffect, useMemo, useRef, useState } from "react";
import { api, DayPlay, NewsItem, SearchHit, Shot, SkinInfo } from "../lib/api";
import { errText } from "../lib/api";
import { loaderLabel } from "../lib/format";
import { isActive, useApp } from "../store/app";
import { Icon } from "./Icon";
import { PixelScene, paletteFor } from "./PixelScene";
import { SkinFigure } from "./SkinFigure";

function dur(secs: number): string {
  const m = Math.floor(secs / 60);
  if (m < 1) return secs > 0 ? "<1 min" : "0 min";
  const h = Math.floor(m / 60);
  return h > 0 ? `${h} h ${String(m % 60).padStart(2, "0")} m` : `${m} min`;
}

function ago(ts: number | null | undefined): string {
  if (!ts) return "never played";
  const d = Math.floor((Date.now() / 1000 - ts) / 60);
  if (d < 2) return "just now";
  if (d < 60) return `${d} min ago`;
  if (d < 60 * 24) return `${Math.floor(d / 60)} h ago`;
  return `${Math.floor(d / 1440)} d ago`;
}

const DAYS = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

function skinKey(accountId: string) {
  return "smcraft.skin." + accountId;
}

function readCustomSkin(accountId: string): SkinInfo | null {
  try {
    const raw = localStorage.getItem(skinKey(accountId));
    return raw ? (JSON.parse(raw) as SkinInfo) : null;
  } catch {
    return null;
  }
}

export function Home() {
  const instances = useApp((s) => s.instances);
  const accounts = useApp((s) => s.accounts);
  const states = useApp((s) => s.states);
  const launch = useApp((s) => s.launch);
  const kill = useApp((s) => s.kill);
  const select = useApp((s) => s.select);
  const setPage = useApp((s) => s.setPage);
  const openModal = useApp((s) => s.openModal);
  const toast = useApp((s) => s.toast);

  const sorted = useMemo(() => [...instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)), [instances]);
  const [selId, setSelId] = useState<string | null>(null);
  const cur = sorted.find((i) => i.id === selId) ?? sorted[0];
  const running = cur ? isActive(states, cur.id) : false;

  // ---- player model ----
  const account = accounts.accounts.find((a) => a.id === accounts.active) ?? accounts.accounts[0];
  const [skin, setSkin] = useState<SkinInfo>({ dataUrl: "", slim: false });
  const fileRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!account) return;
    const custom = readCustomSkin(account.id);
    if (custom) {
      setSkin(custom);
      return;
    }
    setSkin({ dataUrl: "", slim: false });
    let live = true;
    api
      .accountSkin(account.uuid, account.kind)
      .then((s) => live && setSkin(s))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [account?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const pickSkin = (f: File | undefined) => {
    if (!f || !account) return;
    if (f.size > 400_000) {
      toast("That skin file is too large", "error");
      return;
    }
    const r = new FileReader();
    r.onload = () => {
      const info: SkinInfo = { dataUrl: String(r.result), slim: false };
      const img = new Image();
      img.onload = () => {
        if (img.width !== 64 || (img.height !== 64 && img.height !== 32)) {
          toast("A skin image must be 64x64 (or 64x32) pixels", "error");
          return;
        }
        try {
          localStorage.setItem(skinKey(account.id), JSON.stringify(info));
        } catch {
          /* ignore */
        }
        setSkin(info);
      };
      img.src = info.dataUrl;
    };
    r.readAsDataURL(f);
  };
  const resetSkin = () => {
    if (!account) return;
    try {
      localStorage.removeItem(skinKey(account.id));
    } catch {
      /* ignore */
    }
    setSkin({ dataUrl: "", slim: false });
    api.accountSkin(account.uuid, account.kind).then(setSkin).catch(() => undefined);
  };

  // ---- data panels ----
  const [week, setWeek] = useState<DayPlay[]>([]);
  const [news, setNews] = useState<NewsItem[] | null>(null);
  const [packs, setPacks] = useState<SearchHit[] | null>(null);
  const [shots, setShots] = useState<Shot[]>([]);
  const [shotUrls, setShotUrls] = useState<Record<string, string>>({});
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    api.playtimeWeek().then(setWeek).catch(() => undefined);
    api.minecraftNews().then(setNews).catch(() => setNews([]));
    api
      .modrinthSearch({ query: "", projectType: "modpack", mcVersion: null, loader: null, categories: [], sort: "downloads", offset: 0, limit: 4 })
      .then((r) => setPacks(r.hits))
      .catch(() => setPacks([]));
    api
      .listScreenshots()
      .then(async (list) => {
        setShots(list);
        for (const s of list) {
          try {
            const url = await api.readScreenshot(s.path);
            setShotUrls((m) => ({ ...m, [s.path]: url }));
          } catch {
            /* skip unreadable screenshot */
          }
        }
      })
      .catch(() => undefined);
  }, []);

  const weekTotal = week.reduce((a, d) => a + d.secs, 0);
  const maxDay = Math.max(1, ...week.map((d) => d.secs));

  const installPack = async (h: SearchHit) => {
    setInstalling(h.projectId);
    toast(`Installing ${h.title}… this can take a minute`);
    try {
      const inst = await api.installModpack(h.projectId, null, null, "Modpacks");
      await useApp.getState().adoptInstance(inst);
      toast(`${h.title} installed`);
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setInstalling(null);
    }
  };

  const play = () => {
    if (!cur) return;
    void launch(cur.id);
    setPage("instances");
  };

  return (
    <div className="page home">
      <input ref={fileRef} type="file" accept="image/png" hidden onChange={(e) => pickSkin(e.target.files?.[0])} />

      <div className="homehero">
        <PixelScene palette="sunset" seed={7} w={370} h={140} clouds={5} className="herobg" />
        <div className="hh-left">
          {cur ? (
            <>
              <span className="hh-kicker">{cur.lastPlayed ? "Continue playing" : "Ready to play"}</span>
              <h1>{cur.name}</h1>
              <div className="hh-meta">
                {loaderLabel(cur)} · {cur.lastPlayed ? `played ${ago(cur.lastPlayed)}` : "not played yet"}
              </div>
            </>
          ) : (
            <>
              <span className="hh-kicker">Welcome</span>
              <h1>Create your first instance</h1>
              <div className="hh-meta">Pick a Minecraft version and SM Craft sets up Java and the game files for you.</div>
            </>
          )}
          <div className="hh-actions">
            {cur ? (
              running ? (
                <button className="playbtn stop" onClick={() => kill(cur.id)}>
                  <Icon name="stop" /> STOP
                </button>
              ) : (
                <button className="playbtn" onClick={play}>
                  <Icon name="play" /> PLAY
                </button>
              )
            ) : (
              <button className="playbtn" onClick={() => openModal("new")}>
                <Icon name="plus" /> NEW INSTANCE
              </button>
            )}
            {sorted.length > 1 && (
              <select className="hh-select" value={cur?.id} onChange={(e) => setSelId(e.target.value)} aria-label="Instance">
                {sorted.map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name} · {i.mcVersion}
                  </option>
                ))}
              </select>
            )}
          </div>
        </div>

        <div className="hh-model">
          {account && <div className="nametag">{account.username}</div>}
          <div className="figwrap">
            <SkinFigure dataUrl={skin.dataUrl} slim={skin.slim} />
          </div>
          <div className="hh-ctl">
            <button onClick={() => (account ? fileRef.current?.click() : setPage("settings"))}>
              {account ? "Change skin" : "Add account"}
            </button>
            {account && readCustomSkin(account.id) && <button onClick={resetSkin}>Reset</button>}
          </div>
        </div>

        <div className="hh-stats">
          <div>
            <b>{dur(weekTotal)}</b>
            <span>this week</span>
          </div>
          <div>
            <b>{cur ? dur(cur.playTimeSecs) : "0 min"}</b>
            <span>in this instance</span>
          </div>
        </div>
      </div>

      <div className="hsec">Quick actions</div>
      <div className="qgrid">
        <button
          className="qcard"
          onClick={() => {
            setPage("instances");
            openModal("scan");
          }}
        >
          <i style={{ background: "var(--accent)" }} />
          <span>
            <b>Scan this PC</b>
            <small>Import from other launchers</small>
          </span>
        </button>
        <button
          className="qcard"
          disabled={!cur}
          onClick={() => {
            if (!cur) return;
            select(cur.id);
            openModal("mods");
          }}
        >
          <i style={{ background: "#ffd23f" }} />
          <span>
            <b>FPS Booster</b>
            <small>Mods & settings tuned to this PC</small>
          </span>
        </button>
        <button className="qcard" onClick={() => toast("In game: press Right Shift, then open the Keybinds tab")}>
          <i style={{ background: "#4de1ff" }} />
          <span>
            <b>Keybinds</b>
            <small>Visual key manager (in game)</small>
          </span>
        </button>
        <button className="qcard" disabled={!cur} onClick={() => cur && api.openInstanceFolder(cur.id).catch((e) => toast(errText(e), "error"))}>
          <i style={{ background: "#ff6ad5" }} />
          <span>
            <b>Game folder</b>
            <small>{cur ? cur.name : "No instance yet"}</small>
          </span>
        </button>
      </div>

      <div className="hsec">
        Your instances <button onClick={() => setPage("instances")}>View all</button>
      </div>
      <div className="igrid">
        {sorted.slice(0, 3).map((i) => (
          <button
            key={i.id}
            className="icard"
            onClick={() => {
              select(i.id);
              setPage("instances");
            }}
          >
            <PixelScene palette={paletteFor(i.id)} seed={i.id.length * 7 + 3} w={120} h={30} />
            <span className="ib">
              <b>{i.name}</b>
              <small>
                {loaderLabel(i)} · {ago(i.lastPlayed)}
              </small>
            </span>
            {isActive(states, i.id) && <span className="live">RUNNING</span>}
          </button>
        ))}
        <button className="icard add" onClick={() => openModal("new")}>
          <Icon name="plus" /> New instance
        </button>
      </div>

      <div className="hsec">Playtime & news</div>
      <div className="two">
        <div className="hpanel">
          <div className="phead">
            PLAYTIME <span>7 days · {dur(weekTotal)}</span>
          </div>
          <div className="chart">
            {week.map((d, i) => (
              <div key={d.date} className={i === week.length - 1 ? "t" : ""} style={{ height: `${Math.max(4, (d.secs / maxDay) * 100)}%` }} title={dur(d.secs)} />
            ))}
          </div>
          <div className="cl">
            {week.map((d) => (
              <span key={d.date}>{DAYS[new Date(d.date + "T12:00:00").getDay()]}</span>
            ))}
          </div>
        </div>
        <div className="hpanel">
          <div className="phead">MINECRAFT NEWS</div>
          {news === null ? (
            <div className="hmuted">Loading…</div>
          ) : news.length === 0 ? (
            <div className="hmuted">Can't load the news right now.</div>
          ) : (
            news.map((n) => (
              <div className="nrow" key={n.title + n.date}>
                <span className="tag">{n.kind ? n.kind : "Patch notes"}</span>
                <b>{n.title}</b>
                <small>{[n.version, n.date && new Date(n.date).toLocaleDateString()].filter(Boolean).join(" · ")}</small>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="hsec">Popular modpacks</div>
      <div className="hpanel">
        {packs === null ? (
          <div className="hmuted">Loading…</div>
        ) : packs.length === 0 ? (
          <div className="hmuted">Can't reach Modrinth right now.</div>
        ) : (
          packs.map((h) => (
            <div className="prow" key={h.projectId}>
              {h.iconUrl ? <img src={h.iconUrl} alt="" /> : <i />}
              <span>
                <b>{h.title}</b>
                <small>
                  Modrinth · {Intl.NumberFormat("en", { notation: "compact" }).format(h.downloads)} downloads
                </small>
              </span>
              <button className="smallbtn" disabled={installing !== null} onClick={() => installPack(h)}>
                {installing === h.projectId ? "INSTALLING…" : "INSTALL"}
              </button>
            </div>
          ))
        )}
      </div>

      <div className="hsec">Screenshots</div>
      {shots.length === 0 ? (
        <div className="hmuted" style={{ marginBottom: 20 }}>
          Screenshots you take in game (F2) show up here.
        </div>
      ) : (
        <div className="shotgrid">
          {shots.map((s) =>
            shotUrls[s.path] ? (
              <img key={s.path} src={shotUrls[s.path]} alt={s.name} />
            ) : (
              <div key={s.path} className="shotph" />
            ),
          )}
        </div>
      )}
    </div>
  );
}
