import { Clipboard, Download, ExternalLink, FlaskConical, Gauge, Link2 } from "lucide-react";
import { useMemo, useState } from "react";
import {
  antennaProblemJson,
  defaultAntennaSpecForm,
  validateAntennaSpec,
  type AntennaSpecForm,
} from "./antennaSpec";

export type AnalysisViewProps = {
  workspace: string;
  dashboardUrl?: string | undefined;
};

type NumberField = Exclude<keyof AntennaSpecForm, "backend" | "secondBandEnabled">;

const fields: { key: NumberField; label: string; unit: string; step?: string }[] = [
  { key: "boardWidthMm", label: "Board width", unit: "mm" },
  { key: "boardHeightMm", label: "Board height", unit: "mm" },
  { key: "radiatorWidthMm", label: "Radiator width", unit: "mm" },
  { key: "radiatorLengthMm", label: "Radiator length", unit: "mm" },
  { key: "feedXmm", label: "Feed X", unit: "mm" },
  { key: "feedYmm", label: "Feed Y", unit: "mm" },
  { key: "groundXmm", label: "Ground X", unit: "mm" },
  { key: "groundYmm", label: "Ground Y", unit: "mm" },
  { key: "lowerMhz", label: "Band start", unit: "MHz" },
  { key: "upperMhz", label: "Band end", unit: "MHz" },
  { key: "samples", label: "Frequency samples", unit: "count", step: "1" },
  { key: "targetDb", label: "S11 target", unit: "dB" },
  { key: "maxSteps", label: "Max steps", unit: "count", step: "1" },
  { key: "lowerMhz2", label: "Band 2 start", unit: "MHz" },
  { key: "upperMhz2", label: "Band 2 end", unit: "MHz" },
];

function validDashboardUrl(value: string | undefined): value is string {
  if (!value) return false;
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function downloadSpec(json: string) {
  const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "t3cad-antenna-problem.json";
  anchor.click();
  URL.revokeObjectURL(url);
}

export function AnalysisView({ workspace, dashboardUrl }: AnalysisViewProps) {
  const [form, setForm] = useState<AntennaSpecForm>(defaultAntennaSpecForm);
  const [copied, setCopied] = useState(false);
  const errors = useMemo(() => validateAntennaSpec(form), [form]);
  const json = useMemo(() => (errors.length ? "" : antennaProblemJson(form)), [errors, form]);
  const dashboard = validDashboardUrl(dashboardUrl) ? dashboardUrl : undefined;
  const workspaceName = workspace.split(/[\\/]/).filter(Boolean).at(-1) ?? "Current workspace";
  const update = (key: NumberField, value: string) => {
    const number = Number(value);
    if (Number.isFinite(number)) setForm((old) => ({ ...old, [key]: number }));
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(json);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className="flex h-full min-h-0 flex-col overflow-auto bg-background text-foreground">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-3 py-2">
        <FlaskConical className="size-4 text-muted-foreground" />
        <span className="font-medium">Antenna analysis</span>
        <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
          {workspaceName}
        </span>
        {dashboard && (
          <a
            href={dashboard}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-accent"
          >
            <ExternalLink className="size-3.5" /> Open dashboard
          </a>
        )}
      </div>
      <div className="grid min-h-0 gap-3 p-3 lg:grid-cols-[minmax(0,360px)_minmax(0,1fr)]">
        <section className="rounded-md border border-border p-3">
          <div className="mb-3 flex items-center gap-2">
            <Gauge className="size-4 text-muted-foreground" />
            <h2 className="text-xs font-medium uppercase tracking-wide">Problem specification</h2>
          </div>
          <p className="mb-3 text-xs text-muted-foreground">
            Create a planar starter mesh for the antenna-rl EMerge workflow. Dimensions are in mm;
            exported frequencies are in Hz.
          </p>
          <div className="grid grid-cols-2 gap-2">
            {fields.map(({ key, label, unit, step }) => (
              <label key={key} className="min-w-0 text-xs">
                <span className="mb-1 block text-muted-foreground">{label}</span>
                <span className="flex items-center gap-1 rounded border border-border px-2">
                  <input
                    aria-label={label}
                    type="number"
                    step={step ?? "any"}
                    value={form[key]}
                    onChange={(event) => update(key, event.target.value)}
                    className="min-w-0 flex-1 bg-transparent py-1.5 outline-none"
                  />
                  <span className="text-[10px] text-muted-foreground">{unit}</span>
                </span>
              </label>
            ))}
          </div>
          <label className="mt-2 block text-xs">
            <span className="mb-1 block text-muted-foreground">Analysis backend</span>
            <select
              aria-label="Analysis backend"
              value={form.backend}
              onChange={(event) =>
                setForm((old) => ({
                  ...old,
                  backend: event.target.value as AntennaSpecForm["backend"],
                }))
              }
              className="w-full rounded border border-border bg-background px-2 py-1.5"
            >
              <option value="emerge">EMerge dashboard</option>
              <option value="openems">openEMS spec only (no adapter)</option>
            </select>
          </label>
          <label className="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
            <input
              type="checkbox"
              checked={form.secondBandEnabled}
              onChange={(event) =>
                setForm((old) => ({ ...old, secondBandEnabled: event.target.checked }))
              }
            />
            Include second frequency band
          </label>
          {errors.length > 0 && (
            <div
              role="alert"
              className="mt-3 rounded border border-destructive/40 bg-destructive/10 px-2 py-1.5 text-xs text-destructive"
            >
              {errors.join(". ")}
            </div>
          )}
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              disabled={!json}
              onClick={() => downloadSpec(json)}
              className="inline-flex items-center gap-1 rounded bg-accent px-2 py-1.5 text-xs hover:bg-accent/80 disabled:opacity-40"
            >
              <Download className="size-3.5" /> Download spec
            </button>
            <button
              type="button"
              disabled={!json}
              onClick={() => void copy()}
              className="inline-flex items-center gap-1 rounded border border-border px-2 py-1.5 text-xs hover:bg-accent disabled:opacity-40"
            >
              <Clipboard className="size-3.5" /> {copied ? "Copied" : "Copy JSON"}
            </button>
          </div>
          <p className="mt-3 text-[11px] text-muted-foreground">
            {form.backend === "openems"
              ? "openEMS integration is spec-only; no openEMS adapter or solve is connected."
              : "Export creates a problem document only. It does not start a solver or training run."}
          </p>
        </section>
        <section className="flex min-h-[360px] min-w-0 flex-col rounded-md border border-border">
          {dashboard ? (
            <>
              <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2 text-xs">
                <Link2 className="size-3.5 text-muted-foreground" />
                <span className="truncate text-muted-foreground">Configured EMerge dashboard</span>
              </div>
              <iframe
                title="EMerge antenna dashboard"
                src={dashboard}
                className="min-h-[360px] w-full flex-1 border-0"
              />
            </>
          ) : (
            <div className="flex h-full min-h-[360px] items-center justify-center p-6 text-center text-xs text-muted-foreground">
              Configure an EMerge dashboard URL in the project settings to embed its live results
              here.
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
