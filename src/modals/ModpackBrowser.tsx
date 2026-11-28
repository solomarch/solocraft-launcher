import { useCallback, useEffect, useRef, useState } from "react";
import { api, errText, MrProject, MrVersion, SearchHit } from "../lib/api";
import { progressPct } from "../lib/format";
import { useApp } from "../store/app";
import { compact, MrIcon } from "../components/MrIcon";
import { Icon } from "../components/Icon";

const SORTS = [
  { id: "relevance", label: "Relevance" },
  { id: "downloads", label: "Downloads" },
  { id: "follows", label: "Followers" },
  { id: "updated", label: "Recently updated" },
  { id: "newest", label: "Newest" },
];
const CATEGORIES = [
  "adventure",
  "challenging",
  "combat",
  "kitchen-sink",
  "lightweight",
  "magic",
  "multiplayer",
  "optimization",
  "quests",
  "technology",
];
const PAGE = 20;

export function ModpackBrowser({ group }: { group: string }) {
  const toast = useApp((s) => s.toast);
  const adoptInstance = useApp((s) => s.adoptInstance);
  const progress = useApp((s) => s.progress);

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("downloads");
  const [loader, setLoader] = useState("fabric");
  const [mc, setMc] = useState("");
  const [cats, setCats] = useState<string[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [open, setOpen] = useState<SearchHit | null>(null);
  const [project, setProject] = useState<MrProject | null>(null);
  const [versions, setVersions] = useState<MrVersion[]>([]);
  const [versionId, setVersionId] = useState("");
  const [detailError, setDetailError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [installing, setInstalling] = useState(false);

  const reqId = useRef(0);

  const run = useCallback(
    async (offset: number) => {
      const my = ++reqId.current;
      setLoading(true);
      setError(null);
      try {
        const r = await api.modrinthSearch({
          query,
          projectType: "modpack",
          mcVersion: mc.trim() || null,
          loader: loader === "any" ? null : loader,
          categories: cats,
          sort: query.trim() === "" && sort === "relevance" ? "downloads" : sort,
          offset,
          limit: PAGE,
        });
        if (my !== reqId.current) return;
        setHits((h) => (offset === 0 ? r.hits : h.concat(r.hits)));
        setTotal(r.totalHits);
      } catch (e) {
        if (my === reqId.current) setError(errText(e));
      } finally {
        if (my === reqId.current) setLoading(false);
      }
    },
    [query, sort, loader, mc, cats],
  );

  // Search as the person types or changes a filter.
  useEffect(() => {
    const t = setTimeout(() => void run(0), 250);
    return () => clearTimeout(t);
  }, [run]);

  const openHit = async (h: SearchHit) => {
    setOpen(h);
    setProject(null);
    setVersions([]);
    setVersionId("");
    setDetailError(null);
    setName(h.title);
    try {
      const [p, v] = await Promise.all([api.modrinthProject(h.projectId), api.modrinthVersions(h.projectId, [], null)]);
      setProject(p);
      setVersions(v);
      const firstOk = v.find((x) => supported(x));
      setVersionId((firstOk ?? v[0])?.id ?? "");
    } catch (e) {
      setDetailError(errText(e));
    }
  };

  const supported = (v: MrVersion) =>
    v.loaders.length === 0 || v.loaders.some((l) => ["fabric", "quilt", "forge", "neoforge"].includes(l));

  const chosen = versions.find((v) => v.id === versionId);
  const pr = open ? progress[open.projectId] : undefined;
  const pct = progressPct(pr);

  const install = async () => {
    if (!open) return;
    setInstalling(true);
    try {
      const inst = await api.installModpack(open.projectId, versionId || null, name.trim() || null, group);
      toast(`${inst.name} is ready`);
      await adoptInstance(inst);
    } catch (e) {
      toast(errText(e), "error");
      setInstalling(false);
    }
  };

  if (open) {
    return (
      <div className="mpdetail">
        <button className="btn ghost" onClick={() => !installing && setOpen(null)} disabled={installing}>
          ← Back to results
        </button>
        <div className="mphead">
          <MrIcon url={open.iconUrl} title={open.title} size={56} />
          <div style={{ minWidth: 0 }}>
            <div className="mptitle">{open.title}</div>
            <div className="muted">
              by {open.author} · {compact(open.downloads)} downloads · {compact(open.follows)} followers
            </div>
            <div className="tags">
              {open.categories.slice(0, 8).map((c) => (
                <span key={c} className="tag">
                  {c}
                </span>
              ))}
            </div>
          </div>
        </div>
        <p className="mpdesc">{open.description}</p>
        {detailError && <div className="hint err">{detailError}</div>}
        {!project && !detailError && <div className="muted">Loading versions…</div>}
        {project && (
          <>
            <div className="field">
              <label>Version</label>
              <select value={versionId} onChange={(e) => setVersionId(e.target.value)} disabled={installing}>
                {versions.map((v) => (
                  <option key={v.id} value={v.id} disabled={!supported(v)}>
                    {v.name || v.versionNumber} · MC {v.gameVersions.slice(-1)[0]} · {v.loaders.join("/") || "?"}
                    {v.versionType !== "release" ? ` (${v.versionType})` : ""}
                    {!supported(v) ? " — loader not supported yet" : ""}
                  </option>
                ))}
              </select>
            </div>
            {chosen && (
              <div className="muted" style={{ margin: "-4px 0 8px 128px" }}>
                Minecraft {chosen.gameVersions.join(", ")} · published {chosen.datePublished.slice(0, 10)}
              </div>
            )}
            <div className="field">
              <label>Instance name</label>
              <input type="text" value={name} onChange={(e) => setName(e.target.value)} disabled={installing} />
            </div>
            {project.sourceUrl && (
              <div className="muted" style={{ marginLeft: 128 }}>
                Source: {project.sourceUrl}
              </div>
            )}
            <div className="row" style={{ marginTop: 12 }}>
              <button
                className="btn primary"
                disabled={installing || !chosen || !supported(chosen) || name.trim() === ""}
                onClick={install}
              >
                <Icon name="download" /> {installing ? "Installing…" : "Install modpack"}
              </button>
              {installing && (
                <div style={{ flex: 1 }}>
                  <div className="muted">{pr?.stage ?? "Starting…"}</div>
                  {pct !== null && (
                    <div className="bar">
                      <i style={{ width: pct + "%" }} />
                    </div>
                  )}
                </div>
              )}
            </div>
            <div className="muted" style={{ marginTop: 10 }}>
              Every file is checked against Modrinth's SHA-512 before it is kept. If anything fails, the half-made
              instance is removed.
            </div>
          </>
        )}
      </div>
    );
  }

  return (
    <div className="mpbrowse">
      <div className="row" style={{ marginBottom: 6 }}>
        <div className="searchbox" style={{ flex: 1 }}>
          <Icon name="search" />
          <input
            type="text"
            placeholder="Search Modrinth modpacks"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
        </div>
        <select value={sort} onChange={(e) => setSort(e.target.value)} title="Sort by">
          {SORTS.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
        <input
          type="text"
          placeholder="MC version"
          value={mc}
          onChange={(e) => setMc(e.target.value)}
          style={{ width: 84 }}
        />
      </div>
      <div className="row wrap" style={{ marginBottom: 6 }}>
        {["fabric", "quilt", "any"].map((l) => (
          <button key={l} className={"pill" + (loader === l ? " on" : "")} onClick={() => setLoader(l)}>
            {l === "any" ? "Any loader" : l[0].toUpperCase() + l.slice(1)}
          </button>
        ))}
        <span className="sep" />
        {CATEGORIES.map((c) => (
          <button
            key={c}
            className={"pill" + (cats.includes(c) ? " on" : "")}
            onClick={() => setCats((x) => (x.includes(c) ? x.filter((y) => y !== c) : [...x, c]))}
          >
            {c}
          </button>
        ))}
      </div>
      <div className="mplist">
        {error && (
          <div className="empty">
            <h3>Could not reach Modrinth</h3>
            {error}
            <div style={{ marginTop: 8 }}>
              <button className="btn" onClick={() => run(0)}>
                <Icon name="refresh" /> Try again
              </button>
            </div>
          </div>
        )}
        {!error &&
          hits.map((h) => (
            <div key={h.projectId} className="mprow" onClick={() => openHit(h)}>
              <MrIcon url={h.iconUrl} title={h.title} />
              <div style={{ minWidth: 0, flex: 1 }}>
                <div className="mptitle">
                  {h.title} <span className="muted">by {h.author}</span>
                </div>
                <div className="mpsum">{h.description}</div>
                <div className="tags">
                  {h.categories.slice(0, 5).map((c) => (
                    <span key={c} className="tag">
                      {c}
                    </span>
                  ))}
                </div>
              </div>
              <div className="mpstat">
                <Icon name="download" /> {compact(h.downloads)}
              </div>
            </div>
          ))}
        {!error && !loading && hits.length === 0 && <div className="empty">No modpacks found.</div>}
        {loading && <div className="empty">Searching…</div>}
        {!error && !loading && hits.length < total && (
          <div style={{ padding: 8, textAlign: "center" }}>
            <button className="btn" onClick={() => run(hits.length)}>
              Load more ({hits.length} of {total})
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
