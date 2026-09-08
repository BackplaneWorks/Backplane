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
  FolderOpen,
  ChevronDown,
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
import { resolveProjectDesign } from "./projectDesign";

type View =
  | "gerbers"
  | "pcb"
  | "schematic"
  | "3d"
  | "step"
  | "footprint"
  | "symbol"
  | "analysis"
  | "bom";
type Manifest = KiCadProjectManifest & {
  config?: {
    pcb?: string;
    schematic?: string;
    gerbers?: string[];
    analysisUrl?: string;
    symbol?: string;
    symbolMember?: string;
    footprint?: string;
  };
  warnings?: string[];
};
const tabs = [
  { id: "schematic", label: "Schematic", icon: FileText },
  { id: "pcb", label: "PCB", icon: CircuitBoard },
  { id: "3d", label: "3D", icon: Box },
  { id: "gerbers", label: "Gerbers", icon: Layers3 },
  { id: "step", label: "STEP", icon: Box },
] as const;
const optionalTabs = [
  { id: "bom", label: "BOM", icon: List },
  { id: "footprint", label: "Footprints", icon: Shapes },
  { id: "symbol", label: "Symbols", icon: Cpu },
  { id: "analysis", label: "Analysis", icon: Radio },
] as const;
const allTabs = [...tabs, ...optionalTabs];
const params = new URLSearchParams(location.hash.slice(1));
const apiBase = params.get("api") || location.origin;
const token = params.get("token") || "";
const messageOrigin = location.origin === "null" ? "*" : location.origin;
const normalizeProjectPath = (path: string) => path.replaceAll("\\", "/").replace(/^\.\//, "");
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
    | { kind: "model" | "step"; url: string };
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
      title={
        snapshot.kind === "step"
          ? "STEP viewer"
          : snapshot.kind === "model"
            ? "Prism 3D viewer"
            : "Prism ECAD viewer"
      }
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
  const [browse, setBrowse] = useState(false);
  const [fileQuery, setFileQuery] = useState("");
  const [selected, setSelected] = useState<Partial<Record<View, string>>>({});
  const [nativeVisited, setNativeVisited] = useState(
    !params.get("view") || params.get("view") === "pcb" || params.get("view") === "schematic",
  );
  const [refresh, setRefresh] = useState(0);
  const [localStep, setLocalStep] = useState<{ name: string; url: string } | null>(null);
  useEffect(
    () => () => {
      if (localStep) URL.revokeObjectURL(localStep.url);
    },
    [localStep],
  );
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
    (view === "step"
      ? "model"
      : view === "gerbers"
        ? "gerber"
        : view === "schematic" || view === "bom"
          ? "schematic"
          : "pcb");
  const files =
    manifest?.files.filter(
      (file) =>
        file.kind === kind &&
        !file.path.endsWith(".gbrjob") &&
        (view !== "step" || /\.(step|stp)$/i.test(file.path)),
    ) ?? [];
  const design = resolveProjectDesign(manifest?.files ?? [], manifest?.config);
  const boards = manifest?.files.filter((item) => item.kind === "pcb") ?? [];
  const schematics = manifest?.files.filter((item) => item.kind === "schematic") ?? [];
  const pcb = boards.find((item) => item.path === selected.pcb)?.path ?? design.pcb?.path;
  const schematic =
    schematics.find((item) => item.path === selected.schematic)?.path ?? design.schematic?.path;
  const selectionKey = view === "3d" ? "pcb" : view === "bom" ? "schematic" : view;
  const designView = ["pcb", "schematic", "3d", "bom"].includes(view);
  const configured = view === "schematic" || view === "bom" ? schematic : pcb;
  const configuredLibrary =
    libraryView === "symbol" ? manifest?.config?.symbol : manifest?.config?.footprint;
  const normalizedConfiguredLibrary = configuredLibrary
    ? normalizeProjectPath(configuredLibrary)
    : undefined;
  const assignedLibrary = Boolean(configuredLibrary);
  const gerberCandidates = manifest?.config?.gerbers?.length
    ? files.filter((file) =>
        manifest.config!.gerbers!.some(
          (directory) =>
            directory === "." || file.path.startsWith(`${directory.replace(/\/$/, "")}/`),
        ),
      )
    : files;
  const selectableFiles =
    view === "gerbers"
      ? gerberCandidates
      : view === "step"
        ? [...files].sort((a, b) => b.mtimeMs - a.mtimeMs || a.path.localeCompare(b.path))
        : designView
          ? files.filter((item) => item.path === configured)
          : libraryView && assignedLibrary && !selected[selectionKey]
            ? files.filter((item) => item.path === normalizedConfiguredLibrary)
          : files;
  const file =
    selectableFiles.find((item) => item.path === selected[selectionKey]) ??
    selectableFiles.find((item) => item.path === configured) ??
    selectableFiles.find((item) => item.path === normalizedConfiguredLibrary) ??
    (view === "gerbers"
      ? selectableFiles.find((item) => /(?:F[_ .-]?Cu|\.gtl$)/i.test(item.path))
      : undefined) ??
    selectableFiles[0];
  const revision = manifest?.revision ?? "";
  const workspaceName = manifest?.root.split(/[\\/]/).findLast(Boolean) ?? "Design workspace";
  const designName =
    (pcb ?? schematic)
      ?.split("/")
      .at(-1)
      ?.replace(/\.kicad_(pcb|sch)$/i, "") ?? workspaceName;
  const browseFiles = files.filter((item) =>
    item.path.toLowerCase().includes(fileQuery.toLowerCase()),
  );
  const nativeView = view === "pcb" || view === "schematic" ? view : null;
  const chooseView = (next: View) => {
    setError(null);
    if (next === "pcb" || next === "schematic") setNativeVisited(true);
    setBrowse(false);
    setFileQuery("");
    setView(next);
  };
  return (
    <main
      className="design-workspace flex h-dvh min-h-0 flex-col overflow-hidden bg-background text-foreground"
      data-kicad-viewer
      data-revision={manifest?.revision}
    >
      <header className="design-header">
        <div className="design-mark" aria-hidden="true">
          <CircuitBoard size={21} strokeWidth={1.5} />
        </div>
        <div className="design-identity">
          <span className="design-eyebrow">DESIGN WORKSPACE</span>
          <h1>{designName}</h1>
        </div>
        <span className="design-live">
          <span />
          Saved files
        </span>
        <button
          type="button"
          className="kicad-icon-button"
          aria-label="Refresh saved files"
          onClick={() => {
            setError(null);
            setRefresh((n) => n + 1);
          }}
        >
          <RefreshCw size={15} />
        </button>
      </header>
      <nav className="design-navigation" aria-label="Design navigation">
        <div
          className="design-tabs"
          role="tablist"
          aria-label="KiCad views"
          onKeyDown={(event) => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
            const buttons = Array.from(
              event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'),
            );
            const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
            if (index < 0) return;
            event.preventDefault();
            const next =
              event.key === "Home"
                ? 0
                : event.key === "End"
                  ? buttons.length - 1
                  : (index + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) %
                    buttons.length;
            buttons[next]?.focus();
            buttons[next]?.click();
          }}
        >
          {allTabs
            .filter((tab) => tabs.some((base) => base.id === tab.id) || openTabs.includes(tab.id))
            .map(({ id, label, icon: Icon }) => (
              <div key={id} className="design-tab-wrap">
                <button
                  type="button"
                  role="tab"
                  aria-selected={id === view}
                  aria-controls="design-canvas"
                  tabIndex={id === view ? 0 : -1}
                  className="design-tab"
                  onClick={() => chooseView(id)}
                >
                  <Icon size={15} strokeWidth={1.7} />
                  {label}
                </button>
                {openTabs.includes(id) && (
                  <button
                    type="button"
                    className="design-tab-close"
                    aria-label={`Close ${label}`}
                    onClick={() => {
                      setOpenTabs((old) => old.filter((tab) => tab !== id));
                      if (view === id) chooseView("pcb");
                    }}
                  >
                    <X size={11} />
                  </button>
                )}
              </div>
            ))}
        </div>
        <div className="design-tools">
          <select
            aria-label="Open optional viewer tab"
            value=""
            onChange={(event) => {
              const next = optionalTabs.find((tab) => tab.id === event.target.value)?.id;
              if (!next) return;
              setOpenTabs((old) => (old.includes(next) ? old : [...old, next]));
              chooseView(next);
            }}
          >
            <option value="">Tools</option>
            {optionalTabs.map((tab) => (
              <option key={tab.id} value={tab.id}>
                {tab.label}
              </option>
            ))}
          </select>
          <ChevronDown size={12} aria-hidden="true" />
        </div>
      </nav>
      {view !== "analysis" && (
        <div className="design-filebar">
          <div className="design-file-identity">
            <FileText size={14} aria-hidden="true" />
            {designView ? (
              <span className="design-filename">
                {file?.path.split("/").at(-1) ?? "Choose a design file"}
              </span>
            ) : (
              <select
                aria-label={
                  view === "step" ? "STEP file" : view === "gerbers" ? "Gerber layer" : "KiCad file"
                }
                value={view === "step" && localStep ? "__local_step__" : (file?.path ?? "")}
                onChange={(event) => {
                  if (view === "step") setLocalStep(null);
                  setSelected((old) => ({ ...old, [selectionKey]: event.target.value }));
                }}
              >
                {!selectableFiles.length && <option value="">No files found</option>}
                {view === "step" && localStep && (
                  <option value="__local_step__">{localStep.name} (local)</option>
                )}
                {selectableFiles.map((item) => (
                  <option key={item.path} value={item.path}>
                    {item.path}
                  </option>
                ))}
              </select>
            )}
          </div>
          <span className="design-source">
            {view === "step"
              ? "Recent first"
                : designView
                  ? selected[selectionKey]
                    ? "Preview override"
                    : design.assigned
                      ? "Assigned design"
                      : "Project design"
                  : libraryView
                    ? selected[selectionKey]
                      ? "Preview override"
                      : assignedLibrary
                        ? "Assigned library"
                        : "Saved library"
                : "Saved output"}
          </span>
          {(designView || libraryView) && (
            <button
              type="button"
              className="design-text-button"
              aria-expanded={browse}
              onClick={() => {
                setBrowse(!browse);
                setFileQuery("");
              }}
            >
              <FolderOpen size={14} /> <span>{browse ? "Close files" : "Browse"}</span>
            </button>
          )}
          {(designView || libraryView) && selected[selectionKey] && (
            <button
              type="button"
              className="kicad-icon-button"
              aria-label={libraryView ? "Return to assigned library" : "Return to assigned design"}
              onClick={() => setSelected((old) => ({ ...old, [selectionKey]: undefined }))}
            >
              <X size={14} />
            </button>
          )}
          {view === "step" && (
            <label className="design-text-button cursor-pointer">
              <FolderOpen size={14} />
              Open file
              <input
                type="file"
                accept=".step,.stp"
                className="sr-only"
                aria-label="Open local STEP file"
                onChange={(event) => {
                  const picked = event.target.files?.[0];
                  event.target.value = "";
                  if (!picked) return;
                  if (!/\.(step|stp)$/i.test(picked.name) || picked.size > 100 * 1024 * 1024) {
                    setError("Choose a .step or .stp file smaller than 100 MB.");
                    return;
                  }
                  setError(null);
                  setLocalStep({ name: picked.name, url: URL.createObjectURL(picked) });
                }}
              />
            </label>
          )}
        </div>
      )}
      {browse && (
        <section className="design-file-browser" aria-label="Workspace design files">
          <div className="design-browser-heading">
            <span>WORKSPACE FILES</span>
            <span>Preview another file</span>
          </div>
          <input
            autoFocus
            aria-label="Search workspace files"
            placeholder="Search by filename or folder…"
            value={fileQuery}
            onChange={(event) => setFileQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setBrowse(false);
            }}
          />
          <div className="design-file-results">
            {browseFiles.map((item) => (
              <button
                type="button"
                key={item.path}
                onClick={() => {
                  setSelected((old) => ({ ...old, [selectionKey]: item.path }));
                  setBrowse(false);
                }}
              >
                <FileText size={15} />
                <span>{item.path}</span>
                {(item.path === configured || item.path === normalizedConfiguredLibrary) && (
                  <span className="design-file-current">Assigned</span>
                )}
              </button>
            ))}
            {!browseFiles.length && <p>No matching design files.</p>}
          </div>
        </section>
      )}
      {!!manifest?.warnings?.length && (
        <div className="shrink-0 px-3 py-2 text-xs text-muted-foreground" role="status">
          {manifest.warnings.join(" ")}
        </div>
      )}
      <div
        id="design-canvas"
        className="design-canvas relative min-h-0 flex-1"
        role="tabpanel"
        aria-label={view}
      >
        <div key={view} className="design-view-transition" aria-hidden="true" />
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
                  {...(libraryView === "symbol" && manifest.config?.symbolMember
                    ? { member: manifest.config.symbolMember }
                    : {})}
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
            {view === "step" &&
              (localStep || file ? (
                <RuntimeView
                  key={`step:${localStep?.url ?? file?.path}:${file?.mtimeMs}:${refresh}`}
                  snapshot={{
                    kind: "step",
                    url: localStep?.url ?? apiUrl("assets", file?.path, revision),
                  }}
                />
              ) : (
                <Notice text="Choose a project STEP file or use Open file to preview a .step or .stp file from your device." />
              ))}
            {view === "3d" &&
              (file ? (
                <RuntimeView
                  key={`model:${file.path}:${revision}`}
                  snapshot={{ kind: "model", url: apiUrl("model", file.path, revision) }}
                />
              ) : (
                <Notice text="Choose a PCB with Browse to preview it in 3D." />
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
