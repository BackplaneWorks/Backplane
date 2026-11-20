// Pure Management Strip model (BOM | Checks | Lifecycle | Plan | Fab):
// Matches mobile/android DataMgmt.kt and native/gpui/src/screens/mgmt*.rs.
// All tab order, badges, counts, and rows come from the hub's `mgmt.*` and `kicad.checks.*`
// info frames; this module only formats and filters what the hub sends.

export interface MgmtPanelTab {
  id: string;
  label: string;
}

export interface MgmtBadge {
  text: string;
  tone: "warn" | "bad";
}

export interface MgmtStateInfo {
  project?: string;
  open?: string;
  side?: string;
  size?: number;
  panels?: MgmtPanelTab[];
}

export interface BomStock {
  status?: string;
  stock?: number;
  need?: number;
  price?: number;
  class?: string;
  fab?: string;
}

export interface BomLine {
  id: string;
  refs?: string[];
  qty?: number;
  value?: string;
  footprint?: string;
  mpn?: string;
  mfr?: string;
  fab?: string;
  dnp?: boolean;
  key?: string;
  stock?: BomStock;
}

export interface BomInfo {
  badge?: { n?: number; tone?: string };
  source?: string;
  parts?: number;
  missing_mpn?: number;
  dnp?: number;
  more?: number;
  priced?: number;
  lookups?: number;
  checking?: number;
  cost?: number;
  paused?: boolean;
  stock_note?: string;
  lines?: BomLine[];
}

export interface CheckRow {
  id: string;
  source?: string;
  severity?: string;
  rule?: string;
  title?: string;
  names?: string;
  doc?: string;
}

export interface CheckGroup {
  source: string;
  severity: string;
  rule: string;
  n: number;
  rows?: CheckRow[];
}

export interface ChecksInfo {
  badge?: { n?: number; tone?: string };
  total?: number;
  sources?: Record<string, { label?: string; state?: string; judged?: boolean }>;
  groups?: CheckGroup[];
}

export interface LifePart {
  key: string;
  id?: string;
  refs?: string[];
  mpn?: string;
  lifecycle?: string;
  stock?: number;
  price?: number;
  n?: number;
}

export interface LifeInfo {
  badge?: { n?: number; tone?: string };
  counts?: { active?: number; nrnd?: number; eol?: number; unknown?: number };
  unchecked?: number;
  samples?: number;
  since?: number;
  bom?: { cost?: Array<[number, number]>; instock?: Array<[number, number]>; lines?: number };
  parts?: LifePart[];
}

export interface PlanItem {
  id: string;
  name?: string;
  kind?: string;
  state?: string;
  color?: string;
  locked?: boolean;
  nets?: number;
  reference?: string;
  routed?: number;
  total?: number;
  problems?: number;
}

export interface PlanLayer {
  layer: string;
  role?: string;
  stackup?: string;
  status?: string;
  done?: number;
  items?: PlanItem[];
}

export interface PlanInfo {
  badge?: { n?: number; tone?: string };
  on?: boolean;
  empty?: boolean;
  live?: boolean;
  routing?: boolean;
  awaiting?: number;
  keys_note?: string;
  layers?: PlanLayer[];
}

export interface FabLimitSide {
  text?: string;
  source?: string;
  fetched?: string;
  confidence?: string;
}

export interface FabLimit {
  label?: string;
  changed?: boolean;
  inherited?: boolean;
  free?: FabLimitSide;
  extra?: FabLimitSide;
}

export interface FabInfo {
  badge?: { n?: number; tone?: string };
  error?: string;
  rev?: number;
  updated?: string;
  board?: string;
  tier?: string;
  review?: {
    running?: boolean;
    at?: number | null;
    scope?: string;
    judge?: string;
    counts?: { error?: number; warning?: number; info?: number };
  };
  drc?: { state?: string };
  conflicts?: string[];
  suggestions?: Array<{ key: string; label?: string; project?: number; fab?: number }>;
  stackup?: { chosen?: string; by?: string; outer_50_mm?: number };
  changed?: string[];
  limits?: FabLimit[];
}

const KNOWN_PANELS = new Set(["bom", "checks", "life", "plan", "fab"]);

export function mgmtPanelsOf(state: MgmtStateInfo | null | undefined): MgmtPanelTab[] {
  if (!state || !Array.isArray(state.panels)) return [];
  return state.panels.filter((p) => p && typeof p.id === "string" && KNOWN_PANELS.has(p.id));
}

export function badgeOf(data: { badge?: { n?: number; tone?: string } } | null | undefined): MgmtBadge | null {
  const n = data?.badge?.n ?? 0;
  if (!Number.isFinite(n) || n <= 0) return null;
  return {
    text: n > 99 ? "99+" : String(Math.floor(n)),
    tone: data?.badge?.tone === "bad" ? "bad" : "warn",
  };
}

/** "C2, C4, C37 +1": references fitting in `max` chars, then +N remainder. */
export function refsFit(refs: string[], max = 22): string {
  const all = refs.join(", ");
  if (all.length <= max) return all;
  let out = "";
  for (let i = 0; i < refs.length; i++) {
    const r = refs[i];
    const next = out ? `${out}, ${r}` : r;
    const rem = refs.length - i - 1;
    if (next.length + (rem > 0 ? ` +${rem}`.length : 0) > max) {
      return out ? `${out} +${refs.length - i}` : `${refs[0]} +${refs.length - 1}`;
    }
    out = next;
  }
  return out;
}

/** 1060770 -> "1.1M", 12400 -> "12k", 450 -> "450". */
export function abbrev(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "?";
  if (n < 1000) return String(Math.round(n));
  if (n < 1_000_000) {
    const k = n / 1000;
    return k < 10 ? `${k.toFixed(1).replace(/\.0$/, "")}k` : `${Math.round(k)}k`;
  }
  const m = n / 1_000_000;
  return m < 10 ? `${m.toFixed(1).replace(/\.0$/, "")}M` : `${Math.round(m)}M`;
}

/** $1.20, $0.50, $0.0029 */
export function money(v: number): string {
  if (!Number.isFinite(v)) return "";
  if (v >= 1) return `$${v.toFixed(2)}`;
  const trimmed = v.toFixed(4).replace(/0+$/, "");
  const frac = trimmed.split(".")[1] ?? "";
  return frac.length < 2 ? `$${v.toFixed(2)}` : `$${trimmed}`;
}

const STD_PKGS = new Set(["0201", "0402", "0603", "0805", "1206", "1210", "1812", "2010", "2512"]);
const IC_FAMS = new Set([
  "QFN", "DFN", "SOT", "SOIC", "SOP", "SSOP", "TSSOP", "MSOP",
  "QFP", "LQFP", "TQFP", "BGA", "WLCSP", "SOD", "TO", "SON", "LGA",
]);

/** Footprint package summary matching DataMgmt.kt::pkg. */
export function pkgName(fp: string): string {
  if (!fp) return "";
  const idx = fp.indexOf(":");
  const name = idx >= 0 ? fp.slice(idx + 1) : fp;
  const parts = name.split("_");
  for (const p of parts) {
    if (STD_PKGS.has(p)) return p;
  }
  const head = parts[0] ?? "";
  const fam = head.replace(/[^a-zA-Z].*$/, "").toUpperCase();
  if (IC_FAMS.has(fam)) {
    const dash = head.split("-");
    if (dash.length >= 2 && /^\d+$/.test(dash[1])) return `${dash[0]}-${dash[1]}`;
    return dash[0];
  }
  return parts.slice(0, 2).join("_");
}

export function stockSummary(st: BomStock | undefined): { text: string; tone: "ok" | "warn" | "bad" | "faint" } | null {
  const s = st?.status ?? "";
  if (!s) return null;
  const n = typeof st?.stock === "number" ? st.stock : -1;
  if (s === "checking") return { text: "…", tone: "faint" };
  if (s === "unknown") return { text: "?", tone: "faint" };
  if (s === "out") return { text: `out ${abbrev(Math.max(n, 0))}`, tone: "bad" };
  if (s === "low") return { text: `low ${abbrev(n)}`, tone: "warn" };
  return { text: abbrev(n), tone: "ok" };
}

export function filterBomLines(lines: BomLine[], q: string): BomLine[] {
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return lines;
  return lines.filter((l) => {
    const hay = [
      (l.refs ?? []).join(" "),
      l.value ?? "",
      l.footprint ?? "",
      l.mpn ?? "",
      l.fab ?? "",
    ].join(" ").toLowerCase();
    return words.every((w) => (w === "dnp" && Boolean(l.dnp)) || hay.includes(w));
  });
}

export function findingRefs(names: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const part of names.split("·")) {
    const ref = part.trim().split(".")[0] ?? "";
    if (/^[A-Z#]{1,6}[0-9]{1,6}[A-Z]?$/.test(ref) && !seen.has(ref)) {
      seen.add(ref);
      out.push(ref);
    }
  }
  return out;
}

export function filterCheckGroups(groups: CheckGroup[], source: string): CheckGroup[] {
  if (!source) return groups;
  return groups.filter((g) => g.source === source);
}

export function lifeWord(l: string): { label: string; tone: "ok" | "warn" | "bad" | "faint" } {
  if (l === "active") return { label: "Active", tone: "ok" };
  if (l === "nrnd") return { label: "NRND", tone: "warn" };
  if (l === "eol") return { label: "EOL", tone: "bad" };
  return { label: "Unknown", tone: "faint" };
}

export function formatUtcDay(unixSec: number): string {
  if (!Number.isFinite(unixSec) || unixSec <= 0) return "";
  const d = new Date(unixSec * 1000);
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function planDone(item: PlanItem): boolean {
  return item.state === "routed" || item.state === "verified";
}

export function planMark(item: PlanItem): string {
  switch (item.state) {
    case "approved": return "✓";
    case "edited": return "✎";
    case "verified": return "✓✓";
    case "routed": return "100%";
    case "rejected": return "✗";
    case "routing":
      return typeof item.routed === "number" && typeof item.total === "number" && item.total > 0
        ? `${item.routed}/${item.total}`
        : "…";
    case "blocked":
      return `⚠ ${Math.max(1, item.problems ?? 1)}`;
    default:
      return "";
  }
}

export function drcWords(state: string): { text: string; button: string | null } {
  switch (state) {
    case "current": return { text: "The fab's limits are in the DRC rules", button: null };
    case "stale": return { text: "The DRC rules have older fab limits", button: "Update" };
    case "none": return { text: "The fab's limits are not in the DRC rules", button: "Write" };
    default: return { text: "No KiCad project for DRC rules", button: null };
  }
}

export function tierWords(tier: string): string {
  return tier === "extra" ? "costs-more limits" : "base-price limits";
}
