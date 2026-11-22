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
  width?: number | null;
  impedance?: string | null;
  problems?: number;
  problem_text?: string;
  edited?: boolean;
  order?: number;
  priority?: number;
  from?: string[];
  to?: string[];
}

export interface PlanPartBox {
  r: string;
  s: "F" | "B" | string;
  b: [number, number, number, number];
  o: boolean;
}

export interface PlanCopperLayer {
  layer: string;
  index: number;
  role: string;
  alias?: string;
  title?: string;
  subtitle?: string;
}

export interface PlanBoardGeom {
  bbox: [number, number, number, number];
  outline: Array<Array<[number, number]>>;
  parts: PlanPartBox[];
}

export interface PlanBand {
  id: string;
  layer: string;
  width?: number;
  path?: Array<[number, number]>;
  state?: string;
  color?: string;
  nets?: number;
  vias?: Array<[number, number]>;
  name?: string;
  group?: string;
  locked?: boolean;
  edited?: boolean;
  order?: number;
  from?: string[];
  to?: string[];
  impedance?: string | null;
  guides?: Array<[number, number, number, number]>;
  problem_text?: string;
}

export interface PlanRegion {
  id: string;
  layer: string;
  net?: string;
  outline?: Array<[number, number]>;
  holes?: Array<Array<[number, number]>>;
  state?: string;
  color?: string;
  locked?: boolean;
  edited?: boolean;
  reference?: string;
  priority?: number;
  problem_text?: string;
}

export interface PlanLayer {
  layer: string;
  index?: number;
  title?: string;
  subtitle?: string;
  role?: string;
  stackup?: string;
  status?: string;
  done?: number;
  approved?: number;
  total?: number;
  items?: PlanItem[];
}

export interface PlanInfo {
  badge?: { n?: number; tone?: string };
  on?: boolean;
  empty?: boolean;
  live?: boolean;
  routing?: boolean;
  revision?: number;
  total?: number;
  awaiting?: number;
  approved?: number;
  blocked?: number;
  done?: number;
  routed?: number;
  verified?: number;
  rejected?: number;
  status?: string;
  stackup_summary?: string;
  impedances?: string[];
  asks?: Array<{ id?: string; what?: string; note?: string }>;
  elsewhere?: Array<{ pcb?: string; rel?: string; name?: string }>;
  board?: PlanBoardGeom;
  board_trimmed?: boolean;
  copper_layers?: PlanCopperLayer[];
  notes?: Array<{ id?: string; text?: string; by?: string }>;
  constraints?: unknown;
  draw?: { bands?: PlanBand[]; regions?: PlanRegion[] };
  keys_note?: string;
  layers?: PlanLayer[];
}

export interface PlanCamera {
  cx: number;
  cy: number;
  scale: number;
  fitted: number;
  touched: boolean;
}

export type PlanHit =
  | { kind: "vertex"; id: string; isPlane: boolean; idx: number }
  | { kind: "midpoint"; id: string; isPlane: boolean; segIdx: number; pos: [number, number] }
  | { kind: "band"; id: string }
  | { kind: "region"; id: string };

export interface PlanPlacedLabel {
  text: string;
  cx: number;
  cy: number;
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
    case "rejected":
    case "failed": return "✗";
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

export function planStatusBadge(d: PlanInfo): {
  key: string;
  label: string;
  tone: "ok" | "warn" | "bad" | "accent" | "faint";
} {
  switch (d.status) {
    case "blocked": return { key: "gpui.mgmt.plan.st.blocked", label: "Blocked", tone: "bad" };
    case "routing": return { key: "gpui.mgmt.plan.st.routing", label: "Routing", tone: "accent" };
    case "awaiting": return { key: "gpui.mgmt.plan.st.awaiting", label: "Awaiting review", tone: "warn" };
    case "verified": return { key: "gpui.mgmt.plan.st.verified", label: "Verified", tone: "ok" };
    case "routed": return { key: "gpui.mgmt.plan.st.routed", label: "Routed", tone: "ok" };
    case "ready": return { key: "gpui.mgmt.plan.st.approved", label: "Approved", tone: "accent" };
    case "partial": return { key: "gpui.mgmt.plan.st.partial", label: "Partial", tone: "warn" };
    case "rejected": return { key: "gpui.mgmt.plan.st.rejected", label: "Rejected", tone: "bad" };
    default:
      if ((d.blocked ?? 0) > 0) return { key: "gpui.mgmt.plan.st.blocked", label: "Blocked", tone: "bad" };
      if (d.routing) return { key: "gpui.mgmt.plan.st.routing", label: "Routing", tone: "accent" };
      if ((d.awaiting ?? 0) > 0) return { key: "gpui.mgmt.plan.st.awaiting", label: "Awaiting review", tone: "warn" };
      return { key: "gpui.mgmt.plan.st.empty", label: "Empty", tone: "faint" };
  }
}

export function planProgressSegments(d: PlanInfo): Array<{
  count: number;
  tone: "ok" | "accent" | "input" | "warn" | "bad";
}> {
  let doneN = d.done ?? 0;
  let approvedN = d.approved ?? 0;
  const awaitingN = d.awaiting ?? 0;
  const blockedN = d.blocked ?? 0;
  let routingN = 0;

  if (typeof d.total !== "number") {
    for (const l of d.layers ?? []) {
      for (const it of l.items ?? []) {
        if (it.state === "routed" || it.state === "verified") doneN += 1;
        else if (it.state === "routing") routingN += 1;
        else if (it.state === "approved") approvedN += 1;
      }
    }
  } else if (d.routing) {
    routingN = Math.max(0, d.total - (doneN + approvedN + awaitingN + blockedN));
  }

  return [
    { count: doneN, tone: "ok" },
    { count: routingN, tone: "accent" },
    { count: approvedN, tone: "input" },
    { count: awaitingN, tone: "warn" },
    { count: blockedN, tone: "bad" },
  ];
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

export interface BomUndoRecord {
  project: string;
  editId: string;
  refs: string[];
  field: string;
  was: string;
}

export interface BomWriteSnapshot {
  watchGen: number;
  editGen: number;
  project: string;
  editId: string;
}

export interface ChecksFixSnapshot {
  watchGen: number;
  project: string;
  threadId: string;
  threadGen: number;
}

export function isBomWriteCurrent(start: BomWriteSnapshot, now: BomWriteSnapshot): boolean {
  return (
    start.watchGen === now.watchGen &&
    start.editGen === now.editGen &&
    start.project !== "" &&
    start.project === now.project &&
    start.editId !== "" &&
    start.editId === now.editId
  );
}

export function buildBomUndoParams(undo: BomUndoRecord): {
  project: string;
  refs: string[];
  field: string;
  value: string;
} {
  return {
    project: undo.project,
    refs: undo.refs.slice(),
    field: undo.field,
    value: undo.was,
  };
}

export function isChecksFixCurrent(start: ChecksFixSnapshot, now: ChecksFixSnapshot): boolean {
  return (
    start.watchGen === now.watchGen &&
    start.project !== "" &&
    start.project === now.project &&
    start.threadId !== "" &&
    start.threadId === now.threadId &&
    start.threadGen === now.threadGen
  );
}

// ---- Interactive Floorplanner Canvas pure helpers ----

const PLAN_RAIL_PALETTE = [
  "#f59e0b", "#38bdf8", "#c084fc", "#2dd4bf", "#ef4444", "#a3e635", "#f97316", "#ec4899",
];
const PLAN_BUS_PALETTE = [
  "#14b8a6", "#f59e0b", "#c084fc", "#22c55e", "#38bdf8", "#ef4444", "#e879f9", "#84cc16",
];

function fnv1aHash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

export function planRailColor(net: string, fallback = ""): string {
  const low = fallback.trim().toLowerCase();
  if (low && low !== "#5a7a9a" && low !== "#8a8a8a") return fallback;
  const up = net.toUpperCase();
  if (up.includes("3V3A") || up.includes("AVDD") || up.includes("VDDA")) return "#38bdf8";
  if (up.includes("1V8") || up.includes("1.8V")) return "#c084fc";
  if (up.includes("2V5") || up.includes("2.5V")) return "#a3e635";
  if (up.includes("3V3") || up.includes("3.3V") || up.includes("VCC")) return "#f59e0b";
  if (up.includes("5V") || up.startsWith("+5")) return "#ef4444";
  if (up.includes("12V") || up.includes("24V")) return "#f97316";
  if (up.includes("VBAT") || up.includes("VIN")) return "#ec4899";
  if (up.includes("GND")) return "#2dd4bf";
  return PLAN_RAIL_PALETTE[fnv1aHash(up) % PLAN_RAIL_PALETTE.length];
}

export function planBusColor(name: string, fallback = ""): string {
  const low = fallback.trim().toLowerCase();
  if (low && low !== "#8a8a8a") return fallback;
  return PLAN_BUS_PALETTE[fnv1aHash(name) % PLAN_BUS_PALETTE.length];
}

export function planFitCamera(bbox: [number, number, number, number], w: number, h: number, pad = 14): PlanCamera {
  const bw = Math.max(1, Math.abs(bbox[2] - bbox[0]));
  const bh = Math.max(1, Math.abs(bbox[3] - bbox[1]));
  const uw = Math.max(20, w - pad * 2);
  const uh = Math.max(20, h - pad * 2);
  const scale = Math.max(0.05, Math.min(200, Math.min(uw / bw, uh / bh)));
  return {
    cx: (bbox[0] + bbox[2]) * 0.5,
    cy: (bbox[1] + bbox[3]) * 0.5,
    scale,
    fitted: scale,
    touched: false,
  };
}

export function planWorldToPx(cam: PlanCamera, x: number, y: number, w: number, h: number): [number, number] {
  return [w * 0.5 + (x - cam.cx) * cam.scale, h * 0.5 + (y - cam.cy) * cam.scale];
}

export function planPxToWorld(cam: PlanCamera, pxX: number, pxY: number, w: number, h: number): [number, number] {
  const s = Math.max(1e-4, cam.scale);
  return [cam.cx + (pxX - w * 0.5) / s, cam.cy + (pxY - h * 0.5) / s];
}

export function planPanCamera(cam: PlanCamera, dxPx: number, dyPx: number): PlanCamera {
  const s = Math.max(1e-4, cam.scale);
  return { ...cam, cx: cam.cx - dxPx / s, cy: cam.cy - dyPx / s, touched: true };
}

export function planZoomCamera(cam: PlanCamera, factor: number, mx: number, my: number, w: number, h: number): PlanCamera {
  const minS = Math.max(0.02, cam.fitted * 0.35);
  const maxS = Math.max(40, cam.fitted * 24);
  const ns = Math.max(minS, Math.min(maxS, cam.scale * factor));
  const [wx, wy] = planPxToWorld(cam, mx, my, w, h);
  return {
    ...cam,
    cx: wx - (mx - w * 0.5) / ns,
    cy: wy - (my - h * 0.5) / ns,
    scale: ns,
    touched: true,
  };
}

export function planPolylineMidpoint(pts: Array<[number, number]>): [number, number] | null {
  if (pts.length === 0) return null;
  if (pts.length === 1) return pts[0];
  let total = 0;
  for (let i = 0; i + 1 < pts.length; i++) {
    total += Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]);
  }
  if (total < 1e-6) return pts[0];
  const target = total * 0.5;
  let acc = 0;
  for (let i = 0; i + 1 < pts.length; i++) {
    const segLen = Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]);
    if (acc + segLen >= target && segLen > 1e-9) {
      const t = (target - acc) / segLen;
      return [
        pts[i][0] + (pts[i + 1][0] - pts[i][0]) * t,
        pts[i][1] + (pts[i + 1][1] - pts[i][1]) * t,
      ];
    }
    acc += segLen;
  }
  return pts[pts.length - 1];
}

export function planInsertVertex(
  pts: Array<[number, number]>,
  segIdx: number,
  pos: [number, number],
  closed: boolean,
): Array<[number, number]> {
  const out = pts.map((p) => [p[0], p[1]] as [number, number]);
  if (out.length === 0) return [pos];
  const at = closed && segIdx + 1 >= out.length ? out.length : Math.min(segIdx + 1, out.length);
  out.splice(at, 0, [pos[0], pos[1]]);
  return out;
}

export function planRemoveVertex(
  pts: Array<[number, number]>,
  idx: number,
  closed: boolean,
): Array<[number, number]> | null {
  const minPts = closed ? 3 : 2;
  if (pts.length <= minPts || idx < 0 || idx >= pts.length) return null;
  const out = pts.map((p) => [p[0], p[1]] as [number, number]);
  out.splice(idx, 1);
  return out;
}

function distToSegSq(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1e-6) return (px - ax) * (px - ax) + (py - ay) * (py - ay);
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lenSq));
  const cx = ax + t * dx;
  const cy = ay + t * dy;
  return (px - cx) * (px - cx) + (py - cy) * (py - cy);
}

function pointInRing(x: number, y: number, ring: Array<[number, number]>): boolean {
  if (ring.length < 3) return false;
  let inside = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    const intersect = (yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi + 1e-12) + xi;
    if (intersect) inside = !inside;
  }
  return inside;
}

export function planBandStrokePx(widthMm: number, scale: number): number {
  return Math.max(5, Math.min(32, widthMm * scale));
}

export function planHitTest(
  bands: PlanBand[],
  regions: PlanRegion[],
  layer: string,
  selectedId: string,
  cam: PlanCamera,
  w: number,
  h: number,
  mx: number,
  my: number,
): PlanHit | null {
  if (selectedId) {
    for (const b of bands) {
      if (b.layer !== layer || b.id !== selectedId || b.locked) continue;
      const pts = b.path ?? [];
      for (let i = 0; i < pts.length; i++) {
        const [vx, vy] = planWorldToPx(cam, pts[i][0], pts[i][1], w, h);
        if (Math.hypot(mx - vx, my - vy) <= 9.5) return { kind: "vertex", id: selectedId, isPlane: false, idx: i };
      }
      for (let i = 0; i + 1 < pts.length; i++) {
        const mid: [number, number] = [(pts[i][0] + pts[i + 1][0]) * 0.5, (pts[i][1] + pts[i + 1][1]) * 0.5];
        const [hx, hy] = planWorldToPx(cam, mid[0], mid[1], w, h);
        if (Math.hypot(mx - hx, my - hy) <= 8.5) return { kind: "midpoint", id: selectedId, isPlane: false, segIdx: i, pos: mid };
      }
    }
    for (const r of regions) {
      if (r.layer !== layer || r.id !== selectedId || r.locked) continue;
      const pts = r.outline ?? [];
      for (let i = 0; i < pts.length; i++) {
        const [vx, vy] = planWorldToPx(cam, pts[i][0], pts[i][1], w, h);
        if (Math.hypot(mx - vx, my - vy) <= 9.5) return { kind: "vertex", id: selectedId, isPlane: true, idx: i };
      }
      if (pts.length >= 3) {
        for (let i = 0; i < pts.length; i++) {
          const a = pts[i];
          const b = pts[(i + 1) % pts.length];
          const mid: [number, number] = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
          const [hx, hy] = planWorldToPx(cam, mid[0], mid[1], w, h);
          if (Math.hypot(mx - hx, my - hy) <= 8.5) return { kind: "midpoint", id: selectedId, isPlane: true, segIdx: i, pos: mid };
        }
      }
    }
  }

  let bestBand: { dist: number; id: string } | null = null;
  for (const b of bands) {
    if (b.layer !== layer) continue;
    const pts = b.path ?? [];
    if (pts.length < 2) continue;
    const thresh = Math.max(7, planBandStrokePx(b.width ?? 1.5, cam.scale) * 0.5 + 3);
    let minD2 = Number.POSITIVE_INFINITY;
    for (let i = 0; i + 1 < pts.length; i++) {
      const [ax, ay] = planWorldToPx(cam, pts[i][0], pts[i][1], w, h);
      const [bx, by] = planWorldToPx(cam, pts[i + 1][0], pts[i + 1][1], w, h);
      minD2 = Math.min(minD2, distToSegSq(mx, my, ax, ay, bx, by));
    }
    const dist = Math.sqrt(minD2);
    if (dist <= thresh && (!bestBand || dist <= bestBand.dist)) {
      bestBand = { dist, id: b.id };
    }
  }
  if (bestBand) return { kind: "band", id: bestBand.id };

  const [wx, wy] = planPxToWorld(cam, mx, my, w, h);
  for (let i = regions.length - 1; i >= 0; i--) {
    const r = regions[i];
    if (r.layer !== layer) continue;
    const outer = r.outline ?? [];
    if (!pointInRing(wx, wy, outer)) continue;
    const inHole = (r.holes ?? []).some((hRing) => pointInRing(wx, wy, hRing));
    if (!inHole) return { kind: "region", id: r.id };
  }
  return null;
}

export function planDefaultLayerPair(layers: PlanLayer[]): string[] {
  let bestSig: { count: number; layer: string } | null = null;
  let bestPwr: { count: number; layer: string } | null = null;
  const allNames: string[] = [];
  for (const l of layers) {
    if (!l.layer) continue;
    allNames.push(l.layer);
    const items = l.items ?? [];
    const corN = items.filter((it) => it.kind === "corridor").length;
    const plnN = items.filter((it) => it.kind === "plane").length;
    if ((corN > 0 || l.role === "signal") && (!bestSig || corN > bestSig.count)) {
      bestSig = { count: corN, layer: l.layer };
    }
    if ((plnN > 0 || l.role === "power" || l.role === "plane") && (!bestPwr || plnN > bestPwr.count)) {
      bestPwr = { count: plnN, layer: l.layer };
    }
  }
  const out: string[] = [];
  if (bestSig) out.push(bestSig.layer);
  if (bestPwr && !out.includes(bestPwr.layer)) out.push(bestPwr.layer);
  for (const name of allNames) {
    if (out.length >= 2) break;
    if (!out.includes(name)) out.push(name);
  }
  return out;
}

export function planLayoutLabels(
  parts: PlanPartBox[],
  cam: PlanCamera,
  w: number,
  h: number,
  pinned: string[],
): PlanPlacedLabel[] {
  const pinSet = new Set(pinned);
  const cands: Array<{ text: string; cx: number; cy: number; bw: number; bh: number; score: number }> = [];
  for (const p of parts) {
    if (!p.r || !p.b || p.b.length !== 4) continue;
    const [x0, y0] = planWorldToPx(cam, p.b[0], p.b[1], w, h);
    const [x1, y1] = planWorldToPx(cam, p.b[2], p.b[3], w, h);
    const pw = Math.abs(x1 - x0);
    const ph = Math.abs(y1 - y0);
    if (pw < 13 || ph < 9) continue;
    const cx = (x0 + x1) * 0.5;
    const cy = (y0 + y1) * 0.5;
    if (cx < 8 || cx > w - 8 || cy < 8 || cy > h - 8) continue;
    cands.push({
      text: p.r,
      cx,
      cy,
      bw: p.r.length * 5.8 + 4,
      bh: 11,
      score: pw * ph + (pinSet.has(p.r) ? 10000 : 0),
    });
  }
  cands.sort((a, b) => b.score - a.score);
  const placedBoxes: Array<[number, number, number, number]> = [];
  const out: PlanPlacedLabel[] = [];
  for (const c of cands) {
    const rect: [number, number, number, number] = [c.cx - c.bw * 0.5, c.cy - c.bh * 0.5, c.cx + c.bw * 0.5, c.cy + c.bh * 0.5];
    const overlaps = placedBoxes.some((o) => !(rect[2] <= o[0] || rect[0] >= o[2] || rect[3] <= o[1] || rect[1] >= o[3]));
    if (!overlaps) {
      placedBoxes.push(rect);
      out.push({ text: c.text, cx: c.cx, cy: c.cy });
    }
  }
  return out;
}

export function unwrapPlanGetPanel(res: unknown): PlanInfo | null {
  if (!res || typeof res !== "object") return null;
  const r = res as Record<string, unknown>;
  if (r.panel && typeof r.panel === "object") return r.panel as PlanInfo;
  if (r.result && typeof r.result === "object") {
    const nested = (r.result as Record<string, unknown>).panel;
    if (nested && typeof nested === "object") return nested as PlanInfo;
  }
  return null;
}

export function planAllCanvasLayers(layers: PlanLayer[], copperLayers?: PlanCopperLayer[]): PlanLayer[] {
  if (!copperLayers || copperLayers.length === 0) return layers;
  const byName = new Map<string, PlanLayer>();
  for (const l of layers) {
    if (l.layer) byName.set(l.layer, l);
  }
  const out: PlanLayer[] = [];
  const seen = new Set<string>();
  for (const cl of copperLayers) {
    if (!cl.layer) continue;
    seen.add(cl.layer);
    const existing = byName.get(cl.layer);
    if (existing) {
      out.push(existing);
    } else {
      out.push({
        layer: cl.layer,
        index: cl.index,
        title: cl.title || `L${cl.index} · ${cl.layer}`,
        subtitle: cl.subtitle || "",
        role: cl.role,
        status: "proposed",
        done: 0,
        approved: 0,
        total: 0,
        items: [],
      });
    }
  }
  for (const l of layers) {
    if (l.layer && !seen.has(l.layer)) out.push(l);
  }
  return out;
}
