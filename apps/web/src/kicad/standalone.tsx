import { createRoot } from "react-dom/client";
import { useEffect, useRef, useState } from "react";
import {
  Box,
  CircuitBoard,
  FileText,
  Layers3,
  RefreshCw,
  Radio,
  Shapes,
  Cpu,
  X,
  List,
} from "lucide-react";
import "../index.css";
import "./viewer.css";
import type { KiCadProjectManifest } from "@t3tools/contracts";
type KiCadViewerSource = { filename: string; content: string };
import { GerberBrowser } from "./GerberBrowser";
import { NativeProjectViews } from "./NativeProjectViews";
import { LibraryView } from "./LibraryView";
import { AnalysisView } from "./AnalysisView";
import { BomView } from "./BomView";

type View = "gerbers" | "pcb" | "schematic" | "3d" | "footprint" | "symbol" | "analysis" | "bom";
type Manifest = KiCadProjectManifest & {
  config?: { pcb?: string; schematic?: string; gerbers?: string[]; analysisUrl?: string };
  warnings?: string[];
};
const tabs = [
  { id: "gerbers", label: "GERBERs", icon: Layers3 },
  { id: "pcb", label: "PCB", icon: CircuitBoard },
  { id: "schematic", label: "Schematic", icon: FileText },
  { id: "3d", label: "3D model", icon: Box },
] as const;
const optionalTabs = [
  { id: "bom", label: "BOM", icon: List },
  { id: "footprint", label: "Footprints", icon: Shapes },
  { id: "symbol", label: "Symbols", icon: Cpu },
  { id: "analysis", label: "EMerge / Analysis", icon: Radio },
] as const;
const allTabs = [...tabs, ...optionalTabs];
const params = new URLSearchParams(location.hash.slice(1));
const apiBase = params.get("api") || location.origin;
const token = params.get("token") || "";
const messageOrigin = location.origin === "null" ? "*" : location.origin;
function apiUrl(route: string, path?: string, revision?: string) {
  const url = new URL(`${apiBase.replace(/\/$/, "")}/api/kicad/${route}`);
  url.searchParams.set("token", token);
  if (path) url.searchParams.set("path", path);
  if (revision) url.searchParams.set("revision", revision);
  return url.toString();
}
async function readResponse(url: string, signal: AbortSignal) {
  const response = await fetch(url, { signal, cache: "no-store" });
  if (!response.ok) {
    if (response.status === 401 || response.status === 403)
      throw new Error("Viewer access expired. Reopen the KiCad panel to reconnect.");
    throw new Error(
      (await response.text()).slice(0, 700) || `Unable to load project (${response.status})`,
    );
  }
  return response;
}

// The embedding panel sends its resolved theme, including custom palettes.
window.addEventListener("message", (event) => {
  if (
    event.source !== parent ||
    event.origin !== location.origin ||
    event.data?.type !== "k3eda-theme"
  )
    return;
  for (const [key, value] of Object.entries(event.data.variables ?? {})) {
    if (/^--[a-z-]+$/.test(key) && typeof value === "string")
      document.documentElement.style.setProperty(key, value);
  }
  document.documentElement.classList.toggle("dark", Boolean(event.data.dark));
});
if (matchMedia("(prefers-color-scheme: dark)").matches)
  document.documentElement.classList.add("dark");

function RuntimeView({
  snapshot,
}: {
  snapshot:
    | { kind: "native"; sources: KiCadViewerSource[]; revision: string }
    | { kind: "model"; url: string };
}) {
  const ref = useRef<HTMLIFrameElement>(null);
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;
  useEffect(() => {
    const send = (event: MessageEvent) => {
      if (
        event.source === ref.current?.contentWindow &&
        event.origin === location.origin &&
        event.data?.type === "k3eda-runtime-ready"
      ) {
        ref.current?.contentWindow?.postMessage(
          { type: "k3eda-snapshot", ...snapshotRef.current },
          messageOrigin,
        );
      }
    };
    window.addEventListener("message", send);
    return () => window.removeEventListener("message", send);
  }, []);
  useEffect(() => {
    ref.current?.contentWindow?.postMessage({ type: "k3eda-snapshot", ...snapshot }, messageOrigin);
  }, [snapshot]);
  return (
    <iframe
      ref={ref}
      title={snapshot.kind === "model" ? "Prism 3D viewer" : "Prism ECAD viewer"}
      src="/kicad-viewer/runtime.html"
      className="block h-full w-full border-0"
    />
  );
}
function Notice({ text }: { text: string }) {
  return (
    <div
      className="flex h-full items-center justify-center p-6 text-center text-xs text-muted-foreground"
      role="status"
    >
      {text}
    </div>
  );
}

function App() {
  const [view, setView] = useState<View>(
    allTabs.some((tab) => tab.id === params.get("view")) ? (params.get("view") as View) : "pcb",
  );
  const [openTabs, setOpenTabs] = useState<View[]>(
    optionalTabs.some((tab) => tab.id === params.get("view")) ? [params.get("view") as View] : [],
  );
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [error, setError] = useState<string | null>(
    token ? null : "Open this viewer from the project's KiCad panel.",
  );
  const [selected, setSelected] = useState<Partial<Record<View, string>>>({});
  const [nativeVisited, setNativeVisited] = useState(
    !params.get("view") || params.get("view") === "pcb" || params.get("view") === "schematic",
  );
  const [refresh, setRefresh] = useState(0);
  const [visible, setVisible] = useState(!document.hidden);
  useEffect(() => {
    const change = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", change);
    return () => document.removeEventListener("visibilitychange", change);
  }, []);
  useEffect(() => {
    if (!token || !visible) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const next = (await (
          await readResponse(apiUrl("manifest"), controller.signal)
        ).json()) as Manifest;
        if (!controller.signal.aborted) {
          setManifest((old) => (old?.revision === next.revision ? old : next));
        }
      } catch (cause) {
        if (!controller.signal.aborted) setError(String(cause));
      }
      if (!controller.signal.aborted)
        timer = setTimeout(() => {
          void poll();
        }, 2500);
    };
    void poll();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [refresh, visible]);
  const libraryView = view === "footprint" || view === "symbol" ? view : null;
  const kind =
    libraryView ??
    (view === "gerbers" ? "gerber" : view === "schematic" || view === "bom" ? "schematic" : "pcb");
  const files =
    manifest?.files.filter((file) => file.kind === kind && !file.path.endsWith(".gbrjob")) ?? [];
  const configured =
    view === "schematic" || view === "bom" ? manifest?.config?.schematic : manifest?.config?.pcb;
  const configuredGerbers = files.filter((file) =>
    manifest?.config?.gerbers?.some(
      (directory) => directory === "." || file.path.startsWith(`${directory.replace(/\/$/, "")}/`),
    ),
  );
  const gerberCandidates = configuredGerbers.length ? configuredGerbers : files;
  const selectableFiles = view === "gerbers" ? gerberCandidates : files;
  const preferred =
    view === "gerbers"
      ? (
          gerberCandidates.find((item) => /(?:F[_ .-]?Cu|\.gtl$)/i.test(item.path)) ??
          gerberCandidates[0]
        )?.path
      : configured;
  const file =
    selectableFiles.find((item) => item.path === selected[view]) ??
    selectableFiles.find((item) => item.path === preferred) ??
    (view === "gerbers"
      ? files.find((item) => /(?:F[_ .-]?Cu|\.gtl$)/i.test(item.path))
      : undefined) ??
    files[0];
  const revision = manifest?.revision ?? "";
  const boards = manifest?.files.filter((item) => item.kind === "pcb") ?? [];
  const schematics = manifest?.files.filter((item) => item.kind === "schematic") ?? [];
  const pcb =
    boards.find((item) => item.path === selected.pcb)?.path ??
    boards.find((item) => item.path === manifest?.config?.pcb)?.path ??
    boards[0]?.path;
  const schematic =
    schematics.find((item) => item.path === selected.schematic)?.path ??
    schematics.find((item) => item.path === manifest?.config?.schematic)?.path ??
    schematics[0]?.path;
  const nativeView = view === "pcb" || view === "schematic" ? view : null;
  const chooseView = (next: View) => {
    setError(null);
    if (next === "pcb" || next === "schematic") setNativeVisited(true);
    setView(next);
  };
  return (
    <main
      className="flex h-dvh min-h-0 flex-col overflow-hidden bg-background text-foreground"
      data-kicad-viewer
      data-revision={manifest?.revision}
    >
      <div
        className="flex shrink-0 items-center gap-1 overflow-x-auto border-b border-border px-2 py-1"
        role="tablist"
        aria-label="KiCad views"
      >
        {allTabs
          .filter((tab) => tabs.some((base) => base.id === tab.id) || openTabs.includes(tab.id))
          .map(({ id, label, icon: Icon }) => (
            <div key={id} className="flex shrink-0 items-center">
              <button
                type="button"
                role="tab"
                aria-selected={id === view}
                className={`flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs ${id === view ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-accent"}`}
                onClick={() => {
                  chooseView(id);
                }}
              >
                <Icon size={14} />
                {label}
              </button>
              {openTabs.includes(id) && (
                <button
                  type="button"
                  className="kicad-icon-button"
                  aria-label={`Close ${label}`}
                  onClick={() => {
                    setOpenTabs((old) => old.filter((tab) => tab !== id));
                    if (view === id) chooseView("pcb");
                  }}
                >
                  <X size={12} />
                </button>
              )}
            </div>
          ))}
        <select
          aria-label="Open optional viewer tab"
          value=""
          className="max-w-40 shrink-0 rounded border border-border bg-background px-1 py-1 text-xs"
          onChange={(event) => {
            const next = optionalTabs.find((tab) => tab.id === event.target.value)?.id;
            if (!next) return;
            setOpenTabs((old) => (old.includes(next) ? old : [...old, next]));
            chooseView(next);
          }}
        >
          <option value="">+ Open view</option>
          {optionalTabs.map((tab) => (
            <option key={tab.id} value={tab.id}>
              {tab.label}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="kicad-icon-button ml-auto shrink-0"
          aria-label="Refresh saved files"
          onClick={() => {
            setError(null);
            setRefresh((n) => n + 1);
          }}
        >
          <RefreshCw size={14} />
        </button>
      </div>
      {view !== "analysis" && (
        <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5 text-xs">
          <select
            aria-label={view === "gerbers" ? "Gerber layer" : "KiCad file"}
            value={file?.path ?? ""}
            onChange={(event) => setSelected((old) => ({ ...old, [view]: event.target.value }))}
            className="min-w-0 flex-1 rounded border border-border bg-background px-1.5 py-1 text-xs"
          >
            {!selectableFiles.length && <option value="">No files found</option>}
            {selectableFiles.map((item) => (
              <option key={item.path} value={item.path}>
                {item.path}
              </option>
            ))}
          </select>
          <span className="shrink-0 text-[10px] text-muted-foreground">Saved files</span>
        </div>
      )}
      {!!manifest?.warnings?.length && (
        <div className="shrink-0 px-3 py-2 text-xs text-muted-foreground" role="status">
          {manifest.warnings.join(" ")}
        </div>
      )}
      <div className="relative min-h-0 flex-1" role="tabpanel" aria-label={view}>
        {error ? (
          <Notice text={error} />
        ) : !manifest ? (
          <Notice text="Loading saved project…" />
        ) : (
          <>
            {nativeVisited && (
              <div hidden={!nativeView} className="h-full">
                <NativeProjectViews
                  view={nativeView}
                  pcb={pcb}
                  schematic={schematic}
                  sheets={schematics.map((item) => item.path)}
                  revision={`${revision}:${refresh}`}
                  onView={chooseView}
                  read={async (path, signal) =>
                    (await readResponse(apiUrl("assets", path, revision), signal)).text()
                  }
                />
              </div>
            )}
            {libraryView &&
              (file ? (
                <LibraryView
                  kind={libraryView}
                  path={file.path}
                  revision={`${revision}:${refresh}`}
                  read={async (path, signal) =>
                    (await readResponse(apiUrl("library", path, revision), signal)).text()
                  }
                />
              ) : (
                <Notice
                  text={`No ${libraryView === "footprint" ? ".kicad_mod footprints" : ".kicad_sym symbol libraries"} found in this workspace.`}
                />
              ))}
            {openTabs.includes("analysis") && (
              <div hidden={view !== "analysis"} className="h-full">
                <AnalysisView
                  workspace={manifest.root}
                  dashboardUrl={manifest.config?.analysisUrl}
                />
              </div>
            )}
            {view === "bom" &&
              (file ? (
                <BomView
                  path={file.path}
                  revision={`${revision}:${refresh}`}
                  read={async (path, signal) =>
                    (await readResponse(apiUrl("bom", path, revision), signal)).text()
                  }
                />
              ) : (
                <Notice text="No schematic found for the BOM. Select the project's root schematic in .k3eda.json." />
              ))}
            {view === "gerbers" &&
              (file ? (
                <GerberBrowser
                  paths={gerberCandidates.map((item) => item.path)}
                  selected={file.path}
                  revision={`${revision}:${refresh}`}
                  onSelect={(path) => setSelected((old) => ({ ...old, gerbers: path }))}
                  read={readResponse}
                  url={(paths) => {
                    const url = new URL(apiUrl("gerber", undefined, revision));
                    for (const path of paths) url.searchParams.append("path", path);
                    return url.toString();
                  }}
                />
              ) : (
                <Notice text="No Gerber layers found. Point gerbers in .k3eda.json at your generated output directory." />
              ))}
            {view === "3d" &&
              (file ? (
                <RuntimeView
                  key={`model:${file.path}:${revision}`}
                  snapshot={{ kind: "model", url: apiUrl("model", file.path, revision) }}
                />
              ) : (
                <Notice text="No PCB found. Set pcb in .k3eda.json." />
              ))}
          </>
        )}
      </div>
    </main>
  );
}

const root = createRoot(document.getElementById("root")!);
root.render(<App />);
import.meta.hot?.dispose(() => root.unmount());
