import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../store/app";
import { Icon } from "../components/Icon";
import { LoaderPicker } from "../components/LoaderPicker";
import { ModpackBrowser } from "./ModpackBrowser";
import { ImportFile } from "./ImportFile";

const KINDS: { id: string; label: string }[] = [
  { id: "release", label: "Releases" },
  { id: "snapshot", label: "Snapshots" },
  { id: "old_beta", label: "Betas" },
  { id: "old_alpha", label: "Alphas" },
];
const KIND_LABEL: Record<string, string> = {
  release: "Release",
  snapshot: "Snapshot",
  old_beta: "Beta",
  old_alpha: "Alpha",
};

export function NewInstanceModal() {
  const versions = useApp((s) => s.versions);
  const versionsLoading = useApp((s) => s.versionsLoading);
  const versionsError = useApp((s) => s.versionsError);
  const loadVersions = useApp((s) => s.loadVersions);
  const closeModal = useApp((s) => s.closeModal);
  const createInstance = useApp((s) => s.createInstance);
  const settings = useApp((s) => s.settings);
  const currentGroup = useApp((s) => s.group);

  const [kind, setKind] = useState("release");
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<string>("");
  const [name, setName] = useState("");
  const [group, setGroup] = useState(currentGroup === "All" ? "" : currentGroup);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<"custom" | "modpack" | "import">("custom");
  const [loader, setLoader] = useState("vanilla");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [supported, setSupported] = useState(true);
  const [fps, setFps] = useState(true);
  const autoName = useRef("");

  useEffect(() => {
    void loadVersions();
  }, [loadVersions]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return versions.filter((v) => v.kind === kind && (q === "" || v.id.toLowerCase().includes(q)));
  }, [versions, kind, query]);

  useEffect(() => {
    if (!picked && versions.length > 0) {
      const first = versions.find((v) => v.kind === "release");
      if (first) choose(first.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [versions]);

  function choose(id: string) {
    setPicked(id);
    setName((n) => (n === "" || n === autoName.current ? id : n));
    autoName.current = id;
  }

  const submit = async () => {
    setBusy(true);
    const ok = await createInstance(name, picked, group, loader, loaderVersion, fps && (loader === "fabric" || loader === "quilt"));
    if (!ok) setBusy(false);
  };

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && closeModal()}>
      <div className="modal wide">
        <div className="mhead">
          Add instance
          <button className="btn ghost" onClick={closeModal} aria-label="Close">
            <Icon name="x" />
          </button>
        </div>
        <div className="tabs">
          <button className={tab === "custom" ? "on" : ""} onClick={() => setTab("custom")}>
            Custom
          </button>
          <button className={tab === "modpack" ? "on" : ""} onClick={() => setTab("modpack")}>
            Modrinth modpacks
          </button>
          <button className={tab === "import" ? "on" : ""} onClick={() => setTab("import")}>
            Import file
          </button>
        </div>
        {tab === "import" ? (
          <div className="mbody">
            <ImportFile group={group} />
          </div>
        ) : tab === "modpack" ? (
          <div className="mbody">
            <ModpackBrowser group={group} />
          </div>
        ) : (
        <div className="mbody">
          <div className="field">
            <label>Name</label>
            <input type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="My survival world" />
          </div>
          <div className="field">
            <label>Group</label>
            <select value={group} onChange={(e) => setGroup(e.target.value)}>
              <option value="">No group</option>
              {settings.groups.map((g) => (
                <option key={g} value={g}>
                  {g}
                </option>
              ))}
            </select>
          </div>
          <LoaderPicker
            mcVersion={picked}
            loader={loader}
            loaderVersion={loaderVersion}
            onChange={(l, v) => {
              setLoader(l);
              setLoaderVersion(v);
            }}
            onSupport={setSupported}
          />
          {(loader === "fabric" || loader === "quilt") && (
            <div className="field">
              <label>FPS boost pack</label>
              <div className="row">
                <button className={"toggle" + (fps ? " on" : "")} onClick={() => setFps(!fps)} aria-pressed={fps} />
                <span className="muted">
                  Installs Sodium, Sodium Extra, Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling, More
                  Culling (the ones that exist for this version)
                </span>
              </div>
            </div>
          )}

          {(loader === "forge" || loader === "neoforge") && (
            <div className="hint">
              The first launch downloads and runs the official {loader === "forge" ? "Forge" : "NeoForge"} installer
              (1–3 minutes, one time only). Sodium is Fabric-only; for Forge use Embeddium from the Mods window.
            </div>
          )}
          <div className="row" style={{ margin: "10px 0 6px" }}>
            {KINDS.map((k) => (
              <button key={k.id} className={"pill" + (kind === k.id ? " on" : "")} onClick={() => setKind(k.id)}>
                {k.label}
              </button>
            ))}
            <div className="spacer" />
            <input
              type="text"
              placeholder="Filter versions"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              style={{ width: 170 }}
            />
          </div>
          <div className="vtable">
            {versionsLoading && <div className="empty">Loading versions…</div>}
            {versionsError && (
              <div className="empty">
                <h3>Could not load versions</h3>
                {versionsError}
                <div style={{ marginTop: 8 }}>
                  <button className="btn" onClick={() => loadVersions(true)}>
                    <Icon name="refresh" /> Try again
                  </button>
                </div>
              </div>
            )}
            {!versionsLoading &&
              !versionsError &&
              filtered.map((v) => (
                <div
                  key={v.id}
                  className={"vrow" + (picked === v.id ? " sel" : "")}
                  onClick={() => choose(v.id)}
                  onDoubleClick={() => choose(v.id)}
                >
                  <span>{v.id}</span>
                  <span>{KIND_LABEL[v.kind] ?? v.kind}</span>
                  <span>{v.releaseTime.slice(0, 10)}</span>
                </div>
              ))}
            {!versionsLoading && !versionsError && filtered.length === 0 && (
              <div className="empty">No versions match.</div>
            )}
          </div>
        </div>
        )}
        <div className="mfoot">
          {tab !== "custom" && (
            <>
              <span className="muted" style={{ marginRight: "auto" }}>
                Install into group:
                <select value={group} onChange={(e) => setGroup(e.target.value)} style={{ marginLeft: 6 }}>
                  <option value="">No group</option>
                  {settings.groups.map((g) => (
                    <option key={g} value={g}>
                      {g}
                    </option>
                  ))}
                </select>
              </span>
            </>
          )}
          <button className="btn" onClick={closeModal}>
            {tab !== "custom" ? "Close" : "Cancel"}
          </button>
          {tab === "custom" && (
            <button
              className="btn primary"
              disabled={busy || !picked || name.trim() === "" || !supported}
              onClick={submit}
            >
              {busy ? "Creating…" : "Create instance"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
