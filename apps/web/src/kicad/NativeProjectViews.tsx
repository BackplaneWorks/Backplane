import { useEffect, useRef, useState } from "react";
import { ArrowLeftRight, Search } from "lucide-react";

type Source = { filename: string; content: string };
export type NativeView = "pcb" | "schematic";
type Selection = {
  sourceContext?: "PCB" | "SCH";
  reference?: string;
  designator?: string;
  net?: string;
  itemType?: string;
  value?: string;
};
type Probe = { id: number; kind: "net" | "component"; value: string; targetContext: "PCB" | "SCH" };
const origin = location.origin === "null" ? "*" : location.origin;

function NativeFrame({
  view,
  sources,
  revision,
  active,
  probe,
  onSelection,
  onProbe,
  onResult,
}: {
  view: NativeView;
  sources: Source[];
  revision: string;
  active: boolean;
  probe?: Probe | undefined;
  onSelection: (selection: Selection) => void;
  onProbe: (selection: Selection) => void;
  onResult: (found: boolean, value: string) => void;
}) {
  const ref = useRef<HTMLIFrameElement>(null);
  const ready = useRef(false);
  const sentRevision = useRef<string | undefined>(undefined);
  const snapshot = { type: "k3eda-snapshot", kind: "native", sources, revision, active, probe };
  const latest = useRef({ snapshot, onSelection, onProbe, onResult });
  latest.current = { snapshot, onSelection, onProbe, onResult };
  const send = () => {
    if (!ready.current) return;
    const { sources, ...state } = latest.current.snapshot;
    ref.current?.contentWindow?.postMessage(
      sentRevision.current === state.revision ? state : { ...state, sources },
      origin,
    );
    sentRevision.current = state.revision;
  };
  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.source !== ref.current?.contentWindow || event.origin !== location.origin) return;
      if (event.data?.type === "k3eda-runtime-ready") {
        ready.current = true;
        sentRevision.current = undefined;
        send();
      }
      if (event.data?.type === "k3eda-selection") latest.current.onSelection(event.data.selection);
      if (event.data?.type === "k3eda-crossprobe") latest.current.onProbe(event.data.selection);
      if (event.data?.type === "k3eda-probe-result")
        latest.current.onResult(Boolean(event.data.found), String(event.data.value));
    };
    window.addEventListener("message", receive);
    return () => window.removeEventListener("message", receive);
  }, []);
  useEffect(() => {
    send();
  }, [sources, revision, active, probe]);
  return (
    <iframe
      ref={ref}
      title={`Prism ${view === "pcb" ? "PCB" : "schematic"} viewer`}
      src="/kicad-viewer/runtime.html"
      className="h-full w-full border-0"
    />
  );
}

/** Each Prism context retains its camera and parsed project when switching tabs. */
export function NativeProjectViews({
  view,
  pcb,
  schematic,
  sheets,
  revision,
  read,
  onView,
}: {
  view: NativeView | null;
  pcb: string | undefined;
  schematic: string | undefined;
  sheets: string[];
  revision: string;
  read: (path: string, signal: AbortSignal) => Promise<string>;
  onView: (view: NativeView) => void;
}) {
  const [loaded, setLoaded] = useState<
    Partial<Record<NativeView, { key: string; sources: Source[] }>>
  >({});
  const [errors, setErrors] = useState<Partial<Record<NativeView, string>>>({});
  const [selection, setSelection] = useState<Selection | null>(null);
  const [query, setQuery] = useState("");
  const [net, setNet] = useState(false);
  const [probe, setProbe] = useState<Probe>();
  const [status, setStatus] = useState("");
  const requestId = useRef(0);
  const readRef = useRef(read);
  readRef.current = read;
  const sheetsKey = JSON.stringify(sheets);
  useEffect(() => {
    const controller = new AbortController();
    setErrors({});
    for (const [context, path] of [
      ["pcb", pcb],
      ["schematic", schematic],
    ] as const) {
      if (!path) continue;
      const paths = context === "pcb" ? [path] : [path, ...sheets.filter((item) => item !== path)];
      void Promise.all(
        paths.map(async (filename) => ({
          filename,
          content: await readRef.current(filename, controller.signal),
        })),
      )
        .then((sources) => {
          if (!controller.signal.aborted)
            setLoaded((old) => ({ ...old, [context]: { key: `${revision}:${path}`, sources } }));
        })
        .catch((error: unknown) => {
          if (!controller.signal.aborted)
            setErrors((old) => ({ ...old, [context]: String(error) }));
        });
    }
    return () => controller.abort();
  }, [pcb, schematic, revision, sheetsKey]);
  const runProbe = (value: string, kind: "net" | "component", target: NativeView) => {
    if (!value.trim()) return;
    setStatus(`Finding ${value}…`);
    setProbe({
      id: ++requestId.current,
      value: value.trim(),
      kind,
      targetContext: target === "pcb" ? "PCB" : "SCH",
    });
    onView(target);
  };
  const crossProbe = (item: Selection) => {
    const value =
      item.itemType === "net" ? item.net : (item.reference ?? item.designator ?? item.net);
    if (!value) {
      setStatus("Select a component or net to cross-probe.");
      return;
    }
    const kind =
      item.itemType === "net" || !(item.reference ?? item.designator) ? "net" : "component";
    runProbe(value, kind, item.sourceContext === "PCB" ? "schematic" : "pcb");
  };
  return (
    <div className="flex h-full min-h-0 flex-col">
      <form
        className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-2 py-1 text-xs"
        onSubmit={(event) => {
          event.preventDefault();
          if (view) runProbe(query, net ? "net" : "component", view);
        }}
      >
        <Search size={14} className="text-muted-foreground" />
        <input
          aria-label="Find component or net"
          placeholder={net ? "Net name" : "Reference, e.g. U1"}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="min-w-0 flex-1 bg-transparent px-1 py-1 outline-none"
        />
        <label className="flex items-center gap-1">
          <input type="checkbox" checked={net} onChange={(event) => setNet(event.target.checked)} />
          Net
        </label>
        <button type="submit" className="rounded px-2 py-1 hover:bg-accent">
          Find
        </button>
        <button
          type="button"
          disabled={!selection || !pcb || !schematic}
          onClick={() => selection && crossProbe(selection)}
          className="flex items-center gap-1 rounded px-2 py-1 hover:bg-accent disabled:opacity-40"
        >
          <ArrowLeftRight size={14} />
          Show in {view === "pcb" ? "schematic" : "PCB"}
        </button>
      </form>
      {status && (
        <div role="status" className="shrink-0 px-3 py-1 text-xs text-muted-foreground">
          {status}
        </div>
      )}
      {(["pcb", "schematic"] as const).map((context) => {
        const path = context === "pcb" ? pcb : schematic;
        const data = loaded[context];
        const current = data?.key === `${revision}:${path}`;
        return (
          <div key={context} hidden={view !== context} className="relative min-h-0 flex-1">
            {data && (
              <NativeFrame
                key={path}
                view={context}
                sources={data.sources}
                revision={data.key}
                active={view === context && current}
                probe={
                  probe?.targetContext === (context === "pcb" ? "PCB" : "SCH") && current
                    ? probe
                    : undefined
                }
                onSelection={(item) => {
                  if (view === context) {
                    setSelection(item);
                    setStatus(item.reference ?? item.designator ?? item.net ?? "");
                  }
                }}
                onProbe={crossProbe}
                onResult={(found, value) =>
                  setStatus(
                    found ? `Located ${value}` : `No match for ${value} in this ${context}.`,
                  )
                }
              />
            )}
            {(!current || errors[context]) && (
              <div
                role="status"
                className="absolute inset-0 flex items-center justify-center bg-background p-4 text-center text-xs text-muted-foreground"
              >
                {errors[context] ?? (path ? "Loading saved project…" : `No ${context} file found.`)}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
