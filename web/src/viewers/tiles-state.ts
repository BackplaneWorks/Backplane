// Pure helpers and types for the hub-driven web tile renderer (`viewer.layout` / `viewer.layout.act`).
// The hub owns all layout tree state, fitting (`stack` / `twoUp` / `full`), clamping, and watch planning.
// This module performs zero layout arithmetic — only key routing, bounds formatting, and overlay/action helpers.

export type TileKind =
  | "board"
  | "schematic"
  | "3d"
  | "gerbers"
  | "mech"
  | "files"
  | "diff"
  | "review";

export const TILE_KINDS: ReadonlyArray<TileKind> = [
  "board",
  "schematic",
  "3d",
  "gerbers",
  "mech",
  "files",
  "diff",
  "review",
];

export type OverlayMode = "old" | "diff" | "new";

export interface LayoutRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TileOverlay {
  kind: string;
  source?: string;
  mode: OverlayMode;
}

export interface LayoutTile {
  id: string;
  kind: string;
  source: string;
  explicit?: boolean;
  focused: boolean;
  visible: boolean;
  rect: LayoutRect | null;
  overlay?: TileOverlay | null;
  unavailable?: boolean;
}

export interface LayoutSplit {
  id: string;
  axis: "row" | "col" | "h" | "v" | string;
  ratio: number;
  rect: LayoutRect;
}

export interface LayoutWatchReply {
  thread: string;
  tiles: Record<string, string[]>;
  keys: string[];
}

export interface LayoutReply {
  thread: string;
  rev: number;
  mode: "stack" | "twoUp" | "full" | string;
  axis?: string;
  bounds: [number, number];
  focus?: string | null;
  maximized?: string | null;
  order: string[];
  tiles: LayoutTile[];
  splits: LayoutSplit[];
  limits?: { maxTiles: number; minTilePx: number };
  changed?: boolean;
  watch?: LayoutWatchReply;
  watchError?: { code?: string; key?: string; message?: string };
}

export function isTileKind(kind: string): kind is TileKind {
  return (TILE_KINDS as readonly string[]).includes(kind);
}

/** True when a hub layout needs the multi-tile/overlay renderer (2+ tiles or an active overlay). */
export function isTiledOrOverlayLayout(layout: LayoutReply | null | undefined): boolean {
  if (!layout || !Array.isArray(layout.tiles)) return false;
  if (layout.tiles.length >= 2) return true;
  return layout.tiles.some((t) => t.overlay != null);
}

/** Format container pixel bounds for `viewer.layout` / `viewer.watch { kind: "layout" }` (clamped to hub range 100..8192). */
export function normalizeBounds(width: number, height: number): [number, number] {
  const clamp = (n: number): number => {
    if (!Number.isFinite(n) || n < 100) return 100;
    if (n > 8192) return 8192;
    return Math.round(n);
  };
  return [clamp(width), clamp(height)];
}

/** Route an incoming plot key to every visible tile whose hub-assigned key list includes `key`. */
export function routeFrameToTiles(
  key: string,
  tileKeys: Readonly<Record<string, readonly string[]>>
): string[] {
  if (!key) return [];
  const hits: string[] = [];
  for (const [tileId, keys] of Object.entries(tileKeys)) {
    if (Array.isArray(keys) && keys.includes(key)) {
      hits.push(tileId);
    }
  }
  return hits;
}

/** True when a tile can display the Gerbers-over-Board overlay controls (Old | Diff | New). */
export function canOverlayGerbers(tile: Pick<LayoutTile, "kind" | "overlay">, hasGerbers: boolean): boolean {
  return tile.kind === "board" && (hasGerbers || tile.overlay != null);
}

/** Whether a hub split axis is horizontal (side-by-side columns, vertical divider bar). */
export function isRowSplit(axis: string): boolean {
  return axis === "row" || axis === "h" || axis === "horizontal";
}

/** Compute a proposed splitter ratio from a pointer coordinate inside a split bounding box (hub still clamps authoritatively). */
export function proposedSplitRatio(
  pointerPx: number,
  containerStartPx: number,
  containerSizePx: number
): number {
  if (!Number.isFinite(containerSizePx) || containerSizePx <= 0) return 0.5;
  const raw = (pointerPx - containerStartPx) / containerSizePx;
  if (!Number.isFinite(raw)) return 0.5;
  return Math.max(0.1, Math.min(0.9, raw));
}

/** Extract a human-readable refusal message from a `viewer.layout.act` RPC rejection. */
export function formatLayoutError(err: unknown): string {
  if (err && typeof err === "object") {
    const rec = err as Record<string, unknown>;
    if (typeof rec.message === "string" && rec.message) return rec.message;
    if (rec.error && typeof rec.error === "object") {
      const inner = rec.error as Record<string, unknown>;
      if (typeof inner.message === "string" && inner.message) return inner.message;
    }
  }
  return err instanceof Error ? err.message : String(err || "Layout action refused");
}
