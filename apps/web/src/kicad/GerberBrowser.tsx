import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { PrismGerberView } from "./PrismGerberView";
import { gerberPresetPaths, type GerberPreset } from "./gerberPresets";

const presets = [
  ["layer", "Single layer"],
  ["copper", "Copper layers"],
  ["front", "Front placement"],
  ["back", "Back placement"],
  ["all", "All layers"],
] as const;

export function GerberBrowser({
  paths,
  selected,
  revision,
  onSelect,
  url,
  read,
}: {
  paths: string[];
  selected: string;
  revision: string;
  onSelect: (path: string) => void;
  url: (paths: string[]) => string;
  read: (url: string, signal: AbortSignal) => Promise<Response>;
}) {
  const [preset, setPreset] = useState<GerberPreset>("layer");
  const [svg, setSvg] = useState<{ key: string; value: string }>();
  const [error, setError] = useState("");
  const cache = useRef(new Map<string, Promise<string>>());
  const chosen = gerberPresetPaths(paths, selected, preset);
  const key = `${revision}:${JSON.stringify(chosen)}`;
  const index = paths.indexOf(selected);
  const step = (direction: number) => {
    const path = paths[(index + direction + paths.length) % paths.length];
    if (path) onSelect(path);
  };
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (
        event.target instanceof HTMLElement &&
        event.target.closest("input,select,textarea,button")
      )
        return;
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        step(event.key === "ArrowLeft" ? -1 : 1);
      }
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [selected, paths]);
  useEffect(() => {
    let alive = true;
    setError("");
    const load = (layers: string[]) => {
      const cacheKey = `${revision}:${JSON.stringify(layers)}`;
      let pending = cache.current.get(cacheKey);
      if (!pending) {
        pending = read(url(layers), AbortSignal.timeout(60_000)).then((response) =>
          response.text(),
        );
        cache.current.set(cacheKey, pending);
        void pending.catch(() => {
          cache.current.delete(cacheKey);
        });
        while (cache.current.size > 24) cache.current.delete(cache.current.keys().next().value!);
      }
      return pending;
    };
    if (!chosen.length) return;
    void load(chosen)
      .then(async (value) => {
        if (!alive) return;
        setSvg({ key, value });
        // Warm adjacent single layers only, keeping background render work bounded.
        if (preset === "layer") {
          for (const offset of [1, -1]) {
            if (!alive) break;
            const next = paths[(index + offset + paths.length) % paths.length];
            if (next) await load(gerberPresetPaths(paths, next, "layer")).catch(() => {});
          }
        }
      })
      .catch((cause: unknown) => {
        if (alive) setError(String(cause));
      });
    return () => {
      alive = false;
    };
  }, [key]);
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1 text-xs">
        <button
          type="button"
          className="kicad-icon-button"
          aria-label="Previous Gerber layer"
          onClick={() => step(-1)}
        >
          <ChevronLeft size={14} />
        </button>
        <span className="shrink-0 text-muted-foreground">
          {index + 1} / {paths.length}
        </span>
        <button
          type="button"
          className="kicad-icon-button"
          aria-label="Next Gerber layer"
          onClick={() => step(1)}
        >
          <ChevronRight size={14} />
        </button>
        <select
          aria-label="Gerber inspection preset"
          value={preset}
          onChange={(event) => setPreset(event.target.value as GerberPreset)}
          className="min-w-0 flex-1 rounded border border-border bg-background px-2 py-1"
        >
          {presets.map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </select>
      </div>
      <div className="relative min-h-0 flex-1">
        {svg && <PrismGerberView svg={svg.value} label={chosen.join(", ")} />}
        {(error || !chosen.length || svg?.key !== key) && (
          <div
            role="status"
            className="absolute inset-0 flex items-center justify-center bg-background/90 p-4 text-center text-xs text-muted-foreground"
          >
            {error ||
              (!chosen.length ? "No matching layers in this fabrication set." : "Loading layers…")}
          </div>
        )}
      </div>
    </div>
  );
}
