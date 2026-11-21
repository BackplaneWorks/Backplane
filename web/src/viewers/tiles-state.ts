// Pure helpers and types for the hub-driven web tile renderer (`viewer.layout` / `viewer.layout.act`).
// The hub owns all layout tree state, fitting (`stack` / `twoUp` / `full`), clamping, and watch planning.
// This module performs zero layout arithmetic — only key routing, frame buffering/replay, bounds formatting,
// transition classification, and overlay/action helpers.

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

export type LayoutTransition =
  | { action: "empty" }
  | { action: "single"; kind: string; source: string }
  | { action: "grid" };

export function isTileKind(kind: string): kind is TileKind {
  return (TILE_KINDS as readonly string[]).includes(kind);
}

/** True when a hub layout needs the multi-tile/overlay renderer (2+ tiles or an active overlay). */
export function isTiledOrOverlayLayout(layout: LayoutReply | null | undefined): boolean {
  if (!layout || !Array.isArray(layout.tiles)) return false;
  if (layout.tiles.length >= 2) return true;
  return layout.tiles.some((t) => t.overlay != null);
}

/**
 * Unified transition classifier shared by passive reads, action replies, and layout pushes.
 * Preserves the surviving tile's `source` on collapse to single-pane, and reports `empty` on 0 tiles.
 */
export function classifyLayoutTransition(layout: LayoutReply | null | undefined): LayoutTransition {
  if (!layout || !Array.isArray(layout.tiles) || layout.tiles.length === 0) {
    return { action: "empty" };
  }
  if (!isTiledOrOverlayLayout(layout) && layout.tiles.length === 1) {
    return {
      action: "single",
      kind: layout.tiles[0].kind,
      source: layout.tiles[0].source || "",
    };
  }
  return { action: "grid" };
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

/**
 * True when a tile can display the Gerbers-over-Board overlay controls (Old | Diff | New).
 * Requires an affirmatively known Gerber source (or an already-active overlay).
 */
export function canOverlayGerbers(
  tile: Pick<LayoutTile, "kind" | "overlay">,
  gerberSource: string
): boolean {
  return tile.kind === "board" && (!!gerberSource || tile.overlay != null);
}

/**
 * Build the `overlay` payload for `viewer.layout.act { op: "overlay" }` on a Board tile.
 * Prefers merging an existing resolved Gerbers tile in `layout`; otherwise uses the hub-provided
 * root-relative `defaultGerbers` set. Returns `null` when no Gerber source is known (never sends empty source).
 */
export function resolveGerberOverlayArg(
  layout: LayoutReply | null | undefined,
  boardTileId: string,
  defaultGerbers: string,
  mode: OverlayMode
): { overlay: string | { kind: "gerbers"; source: string; mode: OverlayMode }; mode: OverlayMode } | null {
  const existingGerbers = layout?.tiles?.find(
    (x) => x.id !== boardTileId && x.kind === "gerbers" && !!x.source && !x.unavailable
  );
  if (existingGerbers) {
    return { overlay: existingGerbers.id, mode };
  }
  if (defaultGerbers) {
    return { overlay: { kind: "gerbers", source: defaultGerbers, mode }, mode };
  }
  return null;
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

export type QueuedTileFrame =
  | { kind: "bin"; buf: ArrayBuffer }
  | { kind: "none"; key: string; none: string; retryable: boolean };

export interface CachedTileNone {
  none: string;
  retryable: boolean;
}

export const MAX_TILE_PENDING_FRAMES = 64;
export const MAX_TILE_PENDING_BYTES = 16 << 20;

/**
 * Pure frame routing, bounded early-frame queue, and latest-frame replay cache for `TileGridController`.
 * Isolated per thread/watch lifecycle via `reset()`.
 */
export class TileFrameRouter<TPlot = unknown> {
  watchPending = false;
  tileKeys: Record<string, string[]> = {};
  private pending: QueuedTileFrame[] = [];
  private pendingBytes = 0;
  private latestPlot = new Map<string, TPlot>();
  private latestNone = new Map<string, CachedTileNone>();

  reset(): void {
    this.watchPending = false;
    this.tileKeys = {};
    this.pending = [];
    this.pendingBytes = 0;
    this.latestPlot.clear();
    this.latestNone.clear();
  }

  beginWatch(): void {
    this.watchPending = true;
  }

  enqueue(item: QueuedTileFrame): boolean {
    if (!this.watchPending) return false;
    if (this.pending.length >= MAX_TILE_PENDING_FRAMES) return false;
    if (item.kind === "bin") {
      if (this.pendingBytes + item.buf.byteLength > MAX_TILE_PENDING_BYTES) return false;
      this.pendingBytes += item.buf.byteLength;
    }
    this.pending.push(item);
    return true;
  }

  /**
   * Commit a new hub watch set (`tiles`, `keys`).
   * Prunes cached frames for dropped keys, drains bounded early frames, and returns:
   * - `drained`: queued early frames to process now
   * - `emptyTileIds`: tile IDs whose watch key list is now empty (must clear stale artwork!)
   * - `changedTileIds`: tile IDs whose key list changed to a new non-empty set
   */
  commitWatch(
    visibleTileIds: readonly string[],
    w: LayoutWatchReply | null | undefined
  ): {
    drained: QueuedTileFrame[];
    emptyTileIds: string[];
    changedTileIds: string[];
  } {
    const prevTiles = this.tileKeys;
    const nextTiles: Record<string, string[]> =
      w && typeof w.tiles === "object" && w.tiles ? w.tiles : {};
    this.tileKeys = nextTiles;
    this.watchPending = false;

    const liveKeys = new Set<string>(Array.isArray(w?.keys) ? w.keys : []);
    for (const ks of Object.values(nextTiles)) {
      if (Array.isArray(ks)) {
        for (const k of ks) liveKeys.add(k);
      }
    }
    for (const k of Array.from(this.latestPlot.keys())) {
      if (!liveKeys.has(k)) this.latestPlot.delete(k);
    }
    for (const k of Array.from(this.latestNone.keys())) {
      if (!liveKeys.has(k)) this.latestNone.delete(k);
    }

    const emptyTileIds: string[] = [];
    const changedTileIds: string[] = [];
    for (const id of visibleTileIds) {
      const nextKs = Array.isArray(nextTiles[id]) ? nextTiles[id] : [];
      const prevKs = Array.isArray(prevTiles[id]) ? prevTiles[id] : [];
      const same =
        nextKs.length === prevKs.length && nextKs.every((k, i) => k === prevKs[i]);
      if (nextKs.length === 0) {
        emptyTileIds.push(id);
      } else if (!same) {
        changedTileIds.push(id);
      }
    }

    const drained = this.pending;
    this.pending = [];
    this.pendingBytes = 0;
    return { drained, emptyTileIds, changedTileIds };
  }

  recordPlot(key: string, frame: TPlot): string[] {
    this.latestNone.delete(key);
    this.latestPlot.set(key, frame);
    return routeFrameToTiles(key, this.tileKeys);
  }

  recordNone(key: string, none: string, retryable: boolean): string[] {
    this.latestPlot.delete(key);
    this.latestNone.set(key, { none, retryable });
    return routeFrameToTiles(key, this.tileKeys);
  }

  /** Replay the latest cached plot or none status for a newly mounted tile sharing an unchanged subscription. */
  replayForTile(tileId: string): { plots: TPlot[]; nones: CachedTileNone[] } {
    const keys = this.tileKeys[tileId];
    if (!Array.isArray(keys) || keys.length === 0) return { plots: [], nones: [] };
    const plots: TPlot[] = [];
    const nones: CachedTileNone[] = [];
    for (const k of keys) {
      const p = this.latestPlot.get(k);
      if (p !== undefined) plots.push(p);
      const n = this.latestNone.get(k);
      if (n !== undefined) nones.push(n);
    }
    return { plots, nones };
  }
}
