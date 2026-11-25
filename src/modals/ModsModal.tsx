import { useCallback, useEffect, useRef, useState } from "react";
import { api, ClientModStatus, errText, ModEntry, ModUpdate, PlannedMod, SearchHit } from "../lib/api";
import { progressPct } from "../lib/format";
import { isActive, useApp } from "../store/app";
import { Icon } from "../components/Icon";
import { Booster } from "../components/Booster";
import { compact, MrIcon } from "../components/MrIcon";

function fmtSize(n: number): string {
  if (n >= 1_048_576) return (n / 1_048_576).toFixed(1) + " MB";
  return Math.max(1, Math.round(n / 1024)) + " KB";
}

export function ModsModal() {
  const id = useApp((s) => s.selectedId);
  const inst = useApp((s) => s.instances.find((i) => i.id === id));
  const closeModal = useApp((s) => s.closeModal);
  const toast = useApp((s) => s.toast);
  const states = useApp((s) => s.states);
  const progress = useApp((s) => s.progress);
  const openModal = useApp((s) => s.openModal);

  const [tab, setTab] = useState<"installed" | "browse">("installed");
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [updates, setUpdates] = useState<ModUpdate[] | null>(null);
  const [filter, setFilter] = useState("");
  const [cm, setCm] = useState<ClientModStatus | null>(null);
  const [cmBusy, setCmBusy] = useState(false);
  const [showBoost, setShowBoost] = useState(false);

  const loadCm = useCallback(async () => {
    if (!id) return;
    try {
      setCm(await api.clientModStatus(id));
    } catch {
      setCm(null);
    }
  }, [id]);
  useEffect(() => {
    void loadCm();
  }, [loadCm]);

  const toggleCm = async () => {
    if (!id || !cm) return;
    setCmBusy(true);
    try {
      if (cm.installed) await api.clientModRemove(id);
      else await api.clientModInstall(id);
      toast(cm.installed ? "SoloCraft Client removed" : "SoloCraft Client installed. Press Right Shift in game.");
      await loadCm();
      await refresh();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setCmBusy(false);
    }
  };

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("downloads");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [plan, setPlan] = useState<{ hit: SearchHit; items: PlannedMod[] } | null>(null);
  const reqId = useRef(0);

  const running = !!inst && isActive(states, inst.id);
  const pr = inst ? progress[inst.id] : undefined;
  const pct = progressPct(pr);

  const refresh = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      setMods(await api.listMods(id));
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setLoading(false);
    }
  }, [id, toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const search = useCallback(
    async (offset: number) => {
      if (!inst) return;
      const my = ++reqId.current;
      setSearching(true);
      setSearchError(null);
      try {
        const r = await api.modrinthSearch({
          query,
          projectType: "mod",
          mcVersion: inst.mcVersion,
          loader: inst.loader,
          categories: [],
          sort: query.trim() === "" && sort === "relevance" ? "downloads" : sort,
          offset,
          limit: 20,
        });
        if (my !== reqId.current) return;
        setHits((h) => (offset === 0 ? r.hits : h.concat(r.hits)));
        setTotal(r.totalHits);
      } catch (e) {
        if (my === reqId.current) setSearchError(errText(e));
      } finally {
        if (my === reqId.current) setSearching(false);
      }
    },
    [inst, query, sort],
  );

  useEffect(() => {
    if (tab !== "browse") return;
    const t = setTimeout(() => void search(0), 250);
    return () => clearTimeout(t);
  }, [tab, search]);

  if (!inst) return null;

  const modded = inst.loader !== "vanilla";
  const installedIds = new Set(mods.map((m) => m.projectId).filter(Boolean) as string[]);

  const guard = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(null);
      await refresh();
    }
  };

  const toggle = (m: ModEntry) =>
    guard("toggle", async () => {
      await api.setModEnabled(inst.id, m.filename, !m.enabled);
    });

  const remove = (m: ModEntry) =>
    guard("remove", async () => {
      await api.removeMod(inst.id, m.filename);
      setUpdates(null);
    });

  const checkUpdates = () =>
    guard("check", async () => {
      const u = await api.checkModUpdates(inst.id);
      setUpdates(u);
      if (u.length === 0) toast("All mods are up to date");
    });

  const applyUpdate = (u: ModUpdate) =>
    guard("update", async () => {
      await api.updateMod(inst.id, u.filename, u.toVersionId);
      setUpdates((x) => (x ? x.filter((y) => y.filename !== u.filename) : x));
      toast(`${u.title} updated to ${u.toVersion}. The old file is in mods-backup.`);
    });

  const updateAll = () =>
    guard("update", async () => {
      for (const u of updates ?? []) await api.updateMod(inst.id, u.filename, u.toVersionId);
      toast(`Updated ${updates?.length ?? 0} mods. Old files are in mods-backup.`);
      setUpdates([]);
    });

  const askInstall = (hit: SearchHit) =>
    guard("plan:" + hit.projectId, async () => {
      const items = await api.planModInstall(inst.id, hit.projectId);
      if (items.length === 0) {
        toast(`${hit.title} is already installed`);
        return;
      }
      if (items.length === 1) {
        await api.installMod(inst.id, hit.projectId);
        toast(`${hit.title} installed`);
        return;
      }
      setPlan({ hit, items });
    });

  const confirmInstall = () => {
    if (!plan) return;
    const hit = plan.hit;
    setPlan(null);
    void guard("install", async () => {
      const done = await api.installMod(inst.id, hit.projectId);
      toast(`Installed ${done.map((d) => d.title).join(", ")}`);
    });
  };

  const shown = mods.filter((m) => filter.trim() === "" || m.title.toLowerCase().includes(filter.trim().toLowerCase()));
  const working = busy !== null;

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && closeModal()}>
      <div className="modal wide tall">
        <div className="mhead">
          Mods · {inst.name}
          <button className="btn ghost" onClick={closeModal} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>

        {!modded ? (
          <div className="mbody">
            <div className="empty">
              <h3>This instance is Vanilla</h3>
              Mods need a mod loader. Choose Fabric or Quilt in Edit instance, then come back here.
              <div style={{ marginTop: 10 }}>
                <button className="btn primary" onClick={() => openModal("edit")}>
                  Open Edit instance
                </button>
              </div>
            </div>
          </div>
        ) : (
          <>
            {cm && (
              <div className="cmbanner">
                <div>
                  <strong>SoloCraft Client</strong> – in-game menu (Right Shift): FPS, CPS, keystrokes, coordinates,
                  armor, potion effects and more, movable on screen.
                  {cm.reason && !cm.installed && <div className="muted">{cm.reason}</div>}
                </div>
                <button
                  className={"btn " + (cm.installed ? "" : "primary")}
                  disabled={cmBusy || running || (!cm.installed && !!cm.reason)}
                  onClick={toggleCm}
                >
                  {cmBusy ? "Working…" : cm.installed ? "Remove" : "Install"}
                </button>
              </div>
            )}
            <div className="tabs">
              <button className={tab === "installed" ? "on" : ""} onClick={() => setTab("installed")}>
                Installed ({mods.length})
              </button>
              <button className={tab === "browse" ? "on" : ""} onClick={() => setTab("browse")}>
                Browse Modrinth
              </button>
              <div className="spacer" />
              <span className="muted" style={{ padding: "0 10px" }}>
                {inst.mcVersion} · {inst.loader}
              </span>
            </div>
            {running && (
              <div className="banner warn">The game is running. Changes to mods apply the next time you launch.</div>
            )}
            {busy && pr && (
              <div className="banner">
                {pr.stage}
                {pct !== null && (
                  <div className="bar">
                    <i style={{ width: pct + "%" }} />
                  </div>
                )}
              </div>
            )}

            {tab === "installed" && (
              <div className="mbody">
                <div className="row wrap" style={{ marginBottom: 8 }}>
                  <div className="searchbox" style={{ flex: 1, minWidth: 140 }}>
                    <Icon name="search" />
                    <input
                      type="text"
                      placeholder="Filter installed mods"
                      value={filter}
                      onChange={(e) => setFilter(e.target.value)}
                    />
                  </div>
                  <button className="btn" onClick={checkUpdates} disabled={working || running}>
                    <Icon name="refresh" /> Check for updates
                  </button>
                  <button className="btn primary" onClick={() => setShowBoost((v) => !v)} title="Installs the FPS mods and tunes settings for this PC">
                    <Icon name="bolt" /> FPS Booster
                  </button>
                </div>

                {showBoost && inst && <Booster instId={inst.id} running={running} onDone={() => void refresh()} />}

                {updates && updates.length > 0 && (
                  <div className="card" style={{ marginBottom: 8 }}>
                    <div className="row" style={{ marginBottom: 6 }}>
                      <strong>{updates.length} update{updates.length === 1 ? "" : "s"} available</strong>
                      <div className="spacer" />
                      <button className="btn primary" onClick={updateAll} disabled={working || running}>
                        Update all
                      </button>
                    </div>
                    {updates.map((u) => (
                      <div key={u.filename} className="li">
                        <div className="grow">
                          {u.title} <span className="muted">{u.fromVersion || "?"} → {u.toVersion}</span>
                        </div>
                        <button className="btn" onClick={() => applyUpdate(u)} disabled={working || running}>
                          Update
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <div className="modlist">
                  {loading && <div className="empty">Reading mods folder…</div>}
                  {!loading && shown.length === 0 && (
                    <div className="empty">
                      {mods.length === 0 ? (
                        <>
                          <h3>No mods yet</h3>
                          Add the FPS boost pack above, or open Browse Modrinth to install mods in one click. Required
                          dependencies such as Fabric API come along automatically.
                        </>
                      ) : (
                        "No installed mod matches the filter."
                      )}
                    </div>
                  )}
                  {shown.map((m) => (
                    <div key={m.filename} className={"modrow" + (m.enabled ? "" : " off")}>
                      <MrIcon url={m.iconUrl} title={m.title} size={30} />
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <div className="mptitle">
                          {m.title}
                          {!m.explicit && <span className="tag dep">dependency</span>}
                        </div>
                        <div className="mpsum">
                          {m.versionNumber ? `${m.versionNumber} · ` : ""}
                          {m.filename} · {fmtSize(m.size)}
                        </div>
                      </div>
                      <button
                        className={"toggle" + (m.enabled ? " on" : "")}
                        onClick={() => toggle(m)}
                        disabled={working || running}
                        aria-pressed={m.enabled}
                        title={m.enabled ? "Enabled. Click to disable" : "Disabled. Click to enable"}
                      />
                      <button className="btn ghost" onClick={() => remove(m)} disabled={working || running} title="Remove">
                        <Icon name="trash" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {tab === "browse" && (
              <div className="mbody">
                <div className="row" style={{ marginBottom: 8 }}>
                  <div className="searchbox" style={{ flex: 1 }}>
                    <Icon name="search" />
                    <input
                      type="text"
                      placeholder={`Search mods for ${inst.mcVersion} (${inst.loader})`}
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      autoFocus
                    />
                  </div>
                  <select value={sort} onChange={(e) => setSort(e.target.value)}>
                    <option value="relevance">Relevance</option>
                    <option value="downloads">Downloads</option>
                    <option value="follows">Followers</option>
                    <option value="updated">Recently updated</option>
                    <option value="newest">Newest</option>
                  </select>
                </div>
                <div className="modlist">
                  {searchError && (
                    <div className="empty">
                      <h3>Could not reach Modrinth</h3>
                      {searchError}
                    </div>
                  )}
                  {!searchError &&
                    hits.map((h) => {
                      const have = installedIds.has(h.projectId);
                      return (
                        <div key={h.projectId} className="modrow">
                          <MrIcon url={h.iconUrl} title={h.title} size={34} />
                          <div style={{ minWidth: 0, flex: 1 }}>
                            <div className="mptitle">
                              {h.title} <span className="muted">by {h.author}</span>
                            </div>
                            <div className="mpsum">{h.description}</div>
                          </div>
                          <div className="mpstat">
                            <Icon name="download" /> {compact(h.downloads)}
                          </div>
                          <button
                            className={"btn" + (have ? "" : " primary")}
                            disabled={have || working || running}
                            onClick={() => askInstall(h)}
                          >
                            {have ? "Installed" : busy === "plan:" + h.projectId ? "…" : "Install"}
                          </button>
                        </div>
                      );
                    })}
                  {!searchError && !searching && hits.length === 0 && <div className="empty">No mods found.</div>}
                  {searching && <div className="empty">Searching…</div>}
                  {!searchError && !searching && hits.length < total && (
                    <div style={{ padding: 8, textAlign: "center" }}>
                      <button className="btn" onClick={() => search(hits.length)}>
                        Load more ({hits.length} of {total})
                      </button>
                    </div>
                  )}
                </div>
              </div>
            )}
          </>
        )}

        {plan && (
          <div className="confirm">
            <div className="confirmbox">
              <strong>Install {plan.hit.title}?</strong>
              <div className="muted" style={{ margin: "4px 0 6px" }}>
                It needs these required mods, which will be installed too:
              </div>
              <ul>
                {plan.items.map((p) => (
                  <li key={p.projectId}>
                    {p.title} <span className="muted">{p.versionNumber}</span>
                    {p.isDependency ? <span className="tag dep">dependency</span> : null}
                  </li>
                ))}
              </ul>
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <button className="btn" onClick={() => setPlan(null)}>
                  Cancel
                </button>
                <button className="btn primary" onClick={confirmInstall}>
                  Install {plan.items.length} mods
                </button>
              </div>
            </div>
          </div>
        )}

        <div className="mfoot">
          <button className="btn" onClick={() => api.openInstanceSubfolder(inst.id, "mods").catch((e) => toast(errText(e), "error"))}>
            <Icon name="folder" /> Open mods folder
          </button>
          <div className="spacer" />
          <button className="btn" onClick={closeModal}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
