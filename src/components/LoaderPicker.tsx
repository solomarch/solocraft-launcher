import { useEffect, useState } from "react";
import { api, errText, LoaderVersion } from "../lib/api";

const LOADERS: { id: string; label: string; ready: boolean }[] = [
  { id: "vanilla", label: "Vanilla", ready: true },
  { id: "fabric", label: "Fabric", ready: true },
  { id: "quilt", label: "Quilt", ready: true },
  { id: "forge", label: "Forge", ready: true },
  { id: "neoforge", label: "NeoForge", ready: true },
];

interface Props {
  mcVersion: string;
  loader: string;
  loaderVersion: string;
  onChange: (loader: string, loaderVersion: string) => void;
  /** reports whether the chosen loader supports the Minecraft version */
  onSupport?: (ok: boolean) => void;
}

export function LoaderPicker({ mcVersion, loader, loaderVersion, onChange, onSupport }: Props) {
  const [versions, setVersions] = useState<LoaderVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (loader === "vanilla" || !mcVersion) {
      setVersions([]);
      setError(null);
      onSupport?.(true);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    onSupport?.(false);
    api
      .listLoaderVersions(loader, mcVersion)
      .then((v) => {
        if (cancelled) return;
        setVersions(v);
        setLoading(false);
        onSupport?.(true);
      })
      .catch((e) => {
        if (cancelled) return;
        setVersions([]);
        setLoading(false);
        setError(errText(e));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loader, mcVersion]);

  return (
    <>
      <div className="field">
        <label>Mod loader</label>
        <div className="loaders">
          {LOADERS.map((l) => (
            <button
              key={l.id}
              className={"lchip" + (loader === l.id ? " on" : "")}
              disabled={!l.ready}
              title={l.ready ? "" : "Coming in the next update"}
              onClick={() => onChange(l.id, "")}
            >
              {l.label}
            </button>
          ))}
        </div>
      </div>
      {loader !== "vanilla" && (
        <div className="field">
          <label>Loader version</label>
          <div>
            <select
              value={loaderVersion}
              disabled={loading || versions.length === 0}
              onChange={(e) => onChange(loader, e.target.value)}
              style={{ width: "100%" }}
            >
              <option value="">{loading ? "Loading…" : "Latest stable (recommended)"}</option>
              {versions.map((v) => (
                <option key={v.version} value={v.version}>
                  {v.version}
                  {v.stable ? "" : " (beta)"}
                </option>
              ))}
            </select>
            {error && <div className="hint err">{error}</div>}
          </div>
        </div>
      )}
    </>
  );
}
