// Hub-driven Web Tile Renderer (`viewer.layout`, `viewer.layout.act`, `viewer.watch { kind: "layout" }`).
// Geometry, fitting (`stack` | `twoUp` | `full`), clamping, and watch key sets come strictly from the hub.
// One viewer instance per visible tile; incoming frames routed by `{ tileId: [key...] }`.

import { t } from "../i18n";
import type { ViewerHostApi } from "../app";
import { decodeCbor, decodePlot2DFrame, isPlotFrame, type CborValue, type PlotChunk } from "./cbor";
import { mountDiffPane, mountFilesPane, mountReviewPane, type PaneHandle } from "./panes";
import { Plot2DViewer } from "./plot2d";
import { Solid3DViewer } from "./solid3d";
import {
  canOverlayGerbers,
  formatLayoutError,
  isRowSplit,
  isTiledOrOverlayLayout,
  normalizeBounds,
  proposedSplitRatio,
  routeFrameToTiles,
  TILE_KINDS,
  type LayoutReply,
  type LayoutSplit,
  type LayoutTile,
  type LayoutWatchReply,
  type OverlayMode,
} from "./tiles-state";

interface TileInstance {
  id: string;
  kind: string;
  source: string;
  unavailable: boolean;
  el: HTMLElement;
  headEl: HTMLElement;
  bodyEl: HTMLElement;
  overlayEl: HTMLElement;
  plot2d: Plot2DViewer | null;
  solid3d: Solid3DViewer | null;
  pane: PaneHandle | null;
}

export interface TileGridCallbacks {
  pageLabel: (kind: string) => string;
  isLight: () => boolean;
  hasGerbers: () => boolean;
  onRefusal: (msg: string) => void;
  onExitToSingle: (kind: string) => void;
  onOpenFileViewer: (kind: string, relPath: string) => void;
}

function basename(path: string): string {
  if (!path) return "";
  const clean = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const idx = clean.lastIndexOf("/");
  return idx >= 0 ? clean.slice(idx + 1) : clean;
}

function isPlot2DKind(kind: string): boolean {
  return kind === "board" || kind === "schematic" || kind === "gerbers";
}

function isSolidKind(kind: string): boolean {
  return kind === "3d" || kind === "mech";
}

export class TileGridController {
  private readonly host: HTMLElement;
  private readonly api: ViewerHostApi;
  private readonly cb: TileGridCallbacks;

  private readonly rootEl: HTMLElement;
  private readonly tabsEl: HTMLElement;
  private readonly canvasEl: HTMLElement;
  private readonly addMenuEl: HTMLElement;

  private threadId = "";
  private layout: LayoutReply | null = null;
  private tileKeys: Record<string, string[]> = {};
  private heldChunks = new Map<string, PlotChunk[]>();
  private instances = new Map<string, TileInstance>();
  private splitEls: HTMLElement[] = [];
  private gen = 0;
  private resizeTimer = 0;
  private lastBounds: [number, number] = [0, 0];
  private addMenuTargetId = "";
  private ro: ResizeObserver | null = null;
  private alive = true;

  private readonly onDocClick: (e: MouseEvent) => void;
  private readonly onDocKeyDown: (e: KeyboardEvent) => void;

  constructor(host: HTMLElement, api: ViewerHostApi, cb: TileGridCallbacks) {
    this.host = host;
    this.api = api;
    this.cb = cb;

    this.rootEl = document.createElement("div");
    this.rootEl.className = "bp-tiles-root";
    this.rootEl.setAttribute("data-testid", "tile-grid");

    this.tabsEl = document.createElement("div");
    this.tabsEl.className = "bp-tile-tabs";
    this.tabsEl.setAttribute("data-testid", "tile-stack-tabs");
    this.tabsEl.hidden = true;

    this.canvasEl = document.createElement("div");
    this.canvasEl.className = "bp-tiles-canvas";

    this.addMenuEl = document.createElement("div");
    this.addMenuEl.className = "bp-tile-add-menu";
    this.addMenuEl.setAttribute("data-testid", "tile-add-menu");
    this.addMenuEl.hidden = true;

    this.rootEl.append(this.tabsEl, this.canvasEl, this.addMenuEl);
    this.host.replaceChildren(this.rootEl);

    this.onDocClick = (e: MouseEvent) => {
      if (this.addMenuEl.hidden) return;
      if (e.target instanceof Node && (this.addMenuEl.contains(e.target) || (e.target as HTMLElement).closest?.("[data-tile-add-btn]"))) {
        return;
      }
      this.closeAddMenu();
    };
    this.onDocKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !this.addMenuEl.hidden) {
        this.closeAddMenu();
      }
    };
    document.addEventListener("click", this.onDocClick);
    document.addEventListener("keydown", this.onDocKeyDown);

    if (typeof ResizeObserver !== "undefined") {
      this.ro = new ResizeObserver(() => {
        if (!this.alive || !this.threadId || !this.layout) return;
        const next = this.measureBounds();
        if (Math.abs(next[0] - this.lastBounds[0]) < 8 && Math.abs(next[1] - this.lastBounds[1]) < 8) return;
        if (this.resizeTimer) window.clearTimeout(this.resizeTimer);
        this.resizeTimer = window.setTimeout(() => {
          this.resizeTimer = 0;
          void this.refreshLayout(false);
        }, 120);
      });
      this.ro.observe(this.canvasEl);
    }
  }

  measureBounds(): [number, number] {
    const rect = this.canvasEl.getBoundingClientRect();
    const w = rect.width || this.host.clientWidth || 960;
    const h = rect.height || this.host.clientHeight || 640;
    return normalizeBounds(w, h);
  }

  currentLayout(): LayoutReply | null {
    return this.layout;
  }

  async openSplit(threadId: string): Promise<void> {
    this.threadId = threadId;
    await this.runAct("show", { kind: "split" }, true);
  }

  async openAddBeside(threadId: string, kind: string, seedPage?: string): Promise<void> {
    this.threadId = threadId;
    await this.runAct(
      "add",
      {
        kind,
        side: "right",
        ...(seedPage ? { page: seedPage } : {}),
      },
      true
    );
  }

  async openOverlayGerbers(threadId: string, seedPage?: string, mode: OverlayMode = "diff"): Promise<void> {
    this.threadId = threadId;
    const bounds = this.measureBounds();
    try {
      const cur = await this.api.hub.rpc<LayoutReply>("viewer.layout", {
        thread: threadId,
        bounds,
        ...(seedPage ? { page: seedPage } : {}),
      });
      if (!this.alive || this.threadId !== threadId) return;
      const boardTile = cur.tiles.find((x) => x.kind === "board") || cur.tiles[0];
      if (!boardTile) return;
      await this.runAct(
        "overlay",
        {
          tile: boardTile.id,
          overlay: { kind: "gerbers", mode },
          mode,
          ...(seedPage ? { page: seedPage } : {}),
        },
        true
      );
    } catch (err) {
      this.cb.onRefusal(formatLayoutError(err));
    }
  }

  async syncExisting(threadId: string, layout: LayoutReply): Promise<void> {
    this.threadId = threadId;
    this.applyLayout(layout);
    await this.installWatch(false);
  }

  async handleLayoutPush(pushThread: string, rev: number): Promise<void> {
    if (!this.alive || !this.threadId || pushThread !== this.threadId) return;
    if (this.layout && this.layout.rev === rev) return;
    await this.refreshLayout(false);
  }

  private async refreshLayout(forceWatch: boolean): Promise<void> {
    if (!this.alive || !this.threadId) return;
    const gen = ++this.gen;
    const bounds = this.measureBounds();
    this.lastBounds = bounds;
    try {
      const next = await this.api.hub.rpc<LayoutReply>("viewer.layout", {
        thread: this.threadId,
        bounds,
      });
      if (!this.alive || gen !== this.gen) return;
      this.applyLayout(next);
      await this.installWatch(forceWatch);
    } catch (err) {
      if (!this.alive || gen !== this.gen) return;
      this.cb.onRefusal(formatLayoutError(err));
    }
  }

  private async runAct(
    op: string,
    params: Record<string, unknown>,
    forceInstallWatch: boolean
  ): Promise<void> {
    if (!this.alive || !this.threadId) return;
    const gen = ++this.gen;
    const bounds = this.measureBounds();
    this.lastBounds = bounds;
    this.cb.onRefusal("");
    try {
      const res = await this.api.hub.rpc<LayoutReply>("viewer.layout.act", {
        thread: this.threadId,
        op,
        bounds,
        ...params,
      });
      if (!this.alive || gen !== this.gen) return;
      if (res.watchError?.message) {
        this.cb.onRefusal(res.watchError.message);
      }
      if (!isTiledOrOverlayLayout(res) && res.tiles.length === 1 && op === "close") {
        this.cb.onExitToSingle(res.tiles[0].kind);
        return;
      }
      this.applyLayout(res);
      if (res.watch && res.watch.tiles) {
        this.adoptWatch(res.watch);
      } else if (forceInstallWatch || Object.keys(this.tileKeys).length === 0) {
        await this.installWatch(false);
      }
    } catch (err) {
      if (!this.alive || gen !== this.gen) return;
      this.cb.onRefusal(formatLayoutError(err));
    }
  }

  private async installWatch(force: boolean): Promise<void> {
    if (!this.alive || !this.threadId) return;
    const gen = this.gen;
    const bounds = this.measureBounds();
    this.lastBounds = bounds;
    try {
      const w = await this.api.hub.rpc<LayoutWatchReply>("viewer.watch", {
        thread: this.threadId,
        kind: "layout",
        bounds,
        ...(force ? { force: true } : {}),
      });
      if (!this.alive || gen !== this.gen) return;
      this.adoptWatch(w);
    } catch (err) {
      if (!this.alive || gen !== this.gen) return;
      this.cb.onRefusal(formatLayoutError(err));
    }
  }

  private adoptWatch(w: LayoutWatchReply): void {
    this.tileKeys = w && typeof w.tiles === "object" && w.tiles ? w.tiles : {};
    // Drop cached chunk state for keys no longer watched
    const liveKeys = new Set<string>(Array.isArray(w?.keys) ? w.keys : []);
    for (const k of Array.from(this.heldChunks.keys())) {
      if (!liveKeys.has(k)) this.heldChunks.delete(k);
    }
  }

  private closeAddMenu(): void {
    this.addMenuEl.hidden = true;
    this.addMenuTargetId = "";
  }

  private openAddMenu(targetTileId: string, anchor: HTMLElement): void {
    if (!this.addMenuEl.hidden && this.addMenuTargetId === targetTileId) {
      this.closeAddMenu();
      return;
    }
    this.addMenuTargetId = targetTileId;
    this.addMenuEl.replaceChildren();
    for (const kind of TILE_KINDS) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-tile-add-item";
      btn.textContent = this.cb.pageLabel(kind);
      btn.setAttribute("data-testid", `tile-add-kind-${kind}`);
      btn.addEventListener("click", () => {
        this.closeAddMenu();
        void this.runAct("add", { kind, target: targetTileId, side: "right" }, false);
      });
      this.addMenuEl.append(btn);
    }
    const hostRect = this.rootEl.getBoundingClientRect();
    const btnRect = anchor.getBoundingClientRect();
    this.addMenuEl.style.top = `${Math.max(4, Math.round(btnRect.bottom - hostRect.top + 2))}px`;
    this.addMenuEl.style.right = `${Math.max(4, Math.round(hostRect.right - btnRect.right))}px`;
    this.addMenuEl.hidden = false;
  }

  private applyLayout(next: LayoutReply): void {
    this.layout = next;
    this.rootEl.setAttribute("data-layout-mode", next.mode || "full");
    this.rootEl.setAttribute("data-layout-rev", String(next.rev || 0));

    // Stack mode tabs (short side < 600px)
    const isStack = next.mode === "stack" && next.tiles.length > 1;
    this.tabsEl.hidden = !isStack;
    if (isStack) {
      this.tabsEl.replaceChildren();
      const byId = new Map(next.tiles.map((tile) => [tile.id, tile]));
      const ordered = (Array.isArray(next.order) && next.order.length > 0 ? next.order : next.tiles.map((x) => x.id))
        .map((id) => byId.get(id))
        .filter((x): x is LayoutTile => !!x);
      for (const tile of ordered) {
        const tab = document.createElement("button");
        tab.type = "button";
        tab.className = "bp-tile-tab" + (tile.focused || tile.visible ? " is-on" : "");
        tab.textContent = tile.overlay
          ? `${this.cb.pageLabel(tile.kind)} + ${this.cb.pageLabel(tile.overlay.kind)}`
          : this.cb.pageLabel(tile.kind);
        tab.setAttribute("data-testid", `tile-tab-${tile.id}`);
        tab.addEventListener("click", () => {
          void this.runAct("focus", { tile: tile.id }, false);
        });
        this.tabsEl.append(tab);
      }
    }

    const visibleTiles = next.tiles.filter((tile) => tile.visible && tile.rect);
    const visibleIds = new Set(visibleTiles.map((tile) => tile.id));

    // Destroy tiles no longer visible or whose backing kind/source/availability changed
    for (const [id, inst] of Array.from(this.instances.entries())) {
      const tile = visibleTiles.find((x) => x.id === id);
      if (
        !tile ||
        !visibleIds.has(id) ||
        inst.kind !== tile.kind ||
        inst.source !== (tile.source || "") ||
        inst.unavailable !== !!tile.unavailable
      ) {
        this.destroyInstance(inst);
        this.instances.delete(id);
      }
    }

    // Create or update visible tiles strictly from hub rect
    for (const tile of visibleTiles) {
      let inst = this.instances.get(tile.id);
      if (!inst) {
        inst = this.createInstance(tile);
        this.instances.set(tile.id, inst);
        this.canvasEl.append(inst.el);
      }
      const r = tile.rect!;
      inst.el.style.left = `${(r.x * 100).toFixed(4)}%`;
      inst.el.style.top = `${(r.y * 100).toFixed(4)}%`;
      inst.el.style.width = `${(r.w * 100).toFixed(4)}%`;
      inst.el.style.height = `${(r.h * 100).toFixed(4)}%`;
      inst.el.classList.toggle("is-focused", !!tile.focused);
      this.renderTileHeader(inst, tile, next);
    }

    // Render hub splitters (none in stack mode)
    for (const s of this.splitEls) s.remove();
    this.splitEls = [];
    if (!isStack && Array.isArray(next.splits)) {
      for (const sp of next.splits) {
        const bar = this.createSplitter(sp);
        this.splitEls.push(bar);
        this.canvasEl.append(bar);
      }
    }
  }

  private createInstance(tile: LayoutTile): TileInstance {
    const el = document.createElement("div");
    el.className = "bp-tile";
    el.setAttribute("data-testid", `tile-${tile.id}`);
    el.setAttribute("data-tile-kind", tile.kind);

    const headEl = document.createElement("div");
    headEl.className = "bp-tile-head";

    const bodyEl = document.createElement("div");
    bodyEl.className = "bp-tile-body";

    const overlayEl = document.createElement("div");
    overlayEl.className = "bp-v-overlay";
    overlayEl.hidden = true;

    el.append(headEl, bodyEl, overlayEl);

    el.addEventListener("pointerdown", () => {
      if (this.layout && this.layout.focus !== tile.id) {
        void this.runAct("focus", { tile: tile.id }, false);
      }
    });

    let plot2d: Plot2DViewer | null = null;
    let solid3d: Solid3DViewer | null = null;
    let pane: PaneHandle | null = null;

    if (tile.unavailable) {
      this.setTileOverlay(overlayEl, "empty", t("viewer.unavailable", "Source unavailable"));
    } else if (isPlot2DKind(tile.kind)) {
      this.setTileOverlay(overlayEl, "loading", "Loading…");
      plot2d = new Plot2DViewer(bodyEl, {});
      plot2d.setTheme(this.cb.isLight());
    } else if (isSolidKind(tile.kind)) {
      this.setTileOverlay(overlayEl, "loading", "Loading…");
      solid3d = new Solid3DViewer(bodyEl, {
        onStatus: (msg, streaming) => {
          if (streaming || !msg) overlayEl.hidden = true;
          else this.setTileOverlay(overlayEl, "loading", msg);
        },
      });
      solid3d.setTheme(this.cb.isLight());
    } else if (tile.kind === "files") {
      pane = mountFilesPane(bodyEl, this.api, this.threadId, (kind, relPath) => {
        this.cb.onOpenFileViewer(kind, relPath);
      });
    } else if (tile.kind === "diff") {
      pane = mountDiffPane(bodyEl, this.api, this.threadId);
    } else if (tile.kind === "review") {
      pane = mountReviewPane(bodyEl, this.api, this.threadId);
    }

    return {
      id: tile.id,
      kind: tile.kind,
      source: tile.source || "",
      unavailable: !!tile.unavailable,
      el,
      headEl,
      bodyEl,
      overlayEl,
      plot2d,
      solid3d,
      pane,
    };
  }

  private renderTileHeader(inst: TileInstance, tile: LayoutTile, layout: LayoutReply): void {
    const titleWrap = document.createElement("div");
    titleWrap.className = "bp-tile-title";
    const kindSpan = document.createElement("span");
    kindSpan.className = "bp-tile-kind";
    kindSpan.textContent = this.cb.pageLabel(tile.kind);
    titleWrap.append(kindSpan);

    const srcName = basename(tile.source);
    if (srcName) {
      const sep = document.createElement("span");
      sep.className = "bp-vh-sep";
      sep.textContent = "·";
      const fileSpan = document.createElement("span");
      fileSpan.className = "bp-tile-file";
      fileSpan.textContent = srcName;
      titleWrap.append(sep, fileSpan);
    }

    const actions = document.createElement("div");
    actions.className = "bp-tile-actions";

    // Gerbers-over-Board overlay controls (Old | Diff | New)
    if (canOverlayGerbers(tile, this.cb.hasGerbers())) {
      if (tile.overlay) {
        const seg = document.createElement("div");
        seg.className = "bp-tile-overlay-seg";
        seg.setAttribute("data-testid", `tile-overlay-seg-${tile.id}`);
        const modes: Array<{ id: OverlayMode; label: string }> = [
          { id: "old", label: "Old" },
          { id: "diff", label: "Diff" },
          { id: "new", label: "New" },
        ];
        for (const m of modes) {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "bp-v-btn" + (tile.overlay.mode === m.id ? " is-on" : "");
          btn.textContent = m.label;
          btn.setAttribute("data-testid", `tile-overlay-mode-${m.id}`);
          btn.addEventListener("click", (e) => {
            e.stopPropagation();
            void this.runAct(
              "overlay",
              {
                tile: tile.id,
                overlay: {
                  kind: tile.overlay!.kind,
                  ...(tile.overlay!.source ? { source: tile.overlay!.source } : {}),
                  mode: m.id,
                },
                mode: m.id,
              },
              false
            );
          });
          seg.append(btn);
        }
        actions.append(seg);

        const unsplitBtn = document.createElement("button");
        unsplitBtn.type = "button";
        unsplitBtn.className = "bp-v-btn";
        unsplitBtn.textContent = "Unsplit";
        unsplitBtn.setAttribute("data-testid", `tile-overlay-unsplit-${tile.id}`);
        unsplitBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          void this.runAct("unsplit", { tile: tile.id, side: "right" }, false);
        });
        actions.append(unsplitBtn);

        const clearBtn = document.createElement("button");
        clearBtn.type = "button";
        clearBtn.className = "bp-v-btn";
        clearBtn.textContent = "Clear overlay";
        clearBtn.setAttribute("data-testid", `tile-overlay-clear-${tile.id}`);
        clearBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          void this.runAct("overlay", { tile: tile.id, overlay: null }, false);
        });
        actions.append(clearBtn);
      } else {
        const ovBtn = document.createElement("button");
        ovBtn.type = "button";
        ovBtn.className = "bp-v-btn";
        ovBtn.textContent = "Overlay Gerbers";
        ovBtn.setAttribute("data-testid", `tile-overlay-gerbers-${tile.id}`);
        ovBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          void this.runAct(
            "overlay",
            {
              tile: tile.id,
              overlay: { kind: "gerbers", mode: "diff" },
              mode: "diff",
            },
            false
          );
        });
        actions.append(ovBtn);
      }
    }

    if (inst.plot2d || inst.solid3d) {
      const fitBtn = document.createElement("button");
      fitBtn.type = "button";
      fitBtn.className = "bp-v-btn";
      fitBtn.textContent = "Fit";
      fitBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        inst.plot2d?.fit();
        inst.solid3d?.fit();
      });
      actions.append(fitBtn);
    }

    const addBtn = document.createElement("button");
    addBtn.type = "button";
    addBtn.className = "bp-v-btn";
    addBtn.textContent = "+";
    addBtn.title = "Add beside";
    addBtn.setAttribute("data-tile-add-btn", "1");
    addBtn.setAttribute("data-testid", `tile-add-${tile.id}`);
    addBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.openAddMenu(tile.id, addBtn);
    });
    actions.append(addBtn);

    const isMax = layout.maximized === tile.id;
    const maxBtn = document.createElement("button");
    maxBtn.type = "button";
    maxBtn.className = "bp-v-btn" + (isMax ? " is-on" : "");
    maxBtn.textContent = isMax ? "⤡" : "⤢";
    maxBtn.title = isMax ? "Restore" : "Maximize";
    maxBtn.setAttribute("data-testid", `tile-max-${tile.id}`);
    maxBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      if (isMax) void this.runAct("restore", {}, false);
      else void this.runAct("maximize", { tile: tile.id }, false);
    });
    actions.append(maxBtn);

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "bp-v-btn bp-v-close";
    closeBtn.textContent = "✕";
    closeBtn.title = t("common.close", "Close");
    closeBtn.setAttribute("data-testid", `tile-close-${tile.id}`);
    closeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      void this.runAct("close", { tile: tile.id }, false);
    });
    actions.append(closeBtn);

    inst.headEl.replaceChildren(titleWrap, actions);
  }

  private createSplitter(sp: LayoutSplit): HTMLElement {
    const row = isRowSplit(sp.axis);
    const bar = document.createElement("div");
    bar.className = "bp-tile-split " + (row ? "is-row" : "is-col");
    bar.setAttribute("data-testid", `tile-split-${sp.id}`);
    const r = sp.rect;
    if (row) {
      bar.style.left = `${((r.x + r.w * sp.ratio) * 100).toFixed(4)}%`;
      bar.style.top = `${(r.y * 100).toFixed(4)}%`;
      bar.style.height = `${(r.h * 100).toFixed(4)}%`;
    } else {
      bar.style.left = `${(r.x * 100).toFixed(4)}%`;
      bar.style.top = `${((r.y + r.h * sp.ratio) * 100).toFixed(4)}%`;
      bar.style.width = `${(r.w * 100).toFixed(4)}%`;
    }

    let dragging = false;
    let latestRatio = sp.ratio;

    bar.addEventListener("pointerdown", (ev) => {
      ev.preventDefault();
      dragging = true;
      bar.classList.add("is-drag");
      bar.setPointerCapture(ev.pointerId);
    });

    bar.addEventListener("pointermove", (ev) => {
      if (!dragging) return;
      const box = this.canvasEl.getBoundingClientRect();
      if (row) {
        const start = box.left + r.x * box.width;
        const size = r.w * box.width;
        latestRatio = proposedSplitRatio(ev.clientX, start, size);
        bar.style.left = `${((r.x + r.w * latestRatio) * 100).toFixed(4)}%`;
      } else {
        const start = box.top + r.y * box.height;
        const size = r.h * box.height;
        latestRatio = proposedSplitRatio(ev.clientY, start, size);
        bar.style.top = `${((r.y + r.h * latestRatio) * 100).toFixed(4)}%`;
      }
    });

    const finishDrag = (ev: PointerEvent) => {
      if (!dragging) return;
      dragging = false;
      bar.classList.remove("is-drag");
      try { bar.releasePointerCapture(ev.pointerId); } catch {}
      void this.runAct("resize", { split: sp.id, ratio: latestRatio }, false);
    };
    bar.addEventListener("pointerup", finishDrag);
    bar.addEventListener("pointercancel", finishDrag);

    return bar;
  }

  private setTileOverlay(el: HTMLElement, mode: "loading" | "empty" | "error", text: string): void {
    el.hidden = false;
    el.className = `bp-v-overlay is-${mode}`;
    const card = document.createElement("div");
    card.className = "bp-v-state-card";
    const msg = document.createElement("div");
    msg.className = "bp-v-state-msg";
    msg.textContent = text;
    card.append(msg);
    el.replaceChildren(card);
  }

  handlePlotNone(key: string, none: string, retryable: boolean): void {
    const targets = routeFrameToTiles(key, this.tileKeys);
    for (const tileId of targets) {
      const inst = this.instances.get(tileId);
      if (!inst) continue;
      this.setTileOverlay(inst.overlayEl, retryable ? "error" : "empty", none || "Nothing to display");
    }
  }

  handleBinary(buf: ArrayBuffer): void {
    const bytes = new Uint8Array(buf);
    if (!isPlotFrame(bytes)) return;
    const decoded = decodeCbor(bytes);
    if (!decoded || typeof decoded !== "object" || Array.isArray(decoded) || decoded instanceof Uint8Array) return;
    const map = decoded as Record<string, CborValue>;
    if (map.t !== "plot") return;
    const key = typeof map.key === "string" ? map.key : "";
    if (!key) return;

    const targets = routeFrameToTiles(key, this.tileKeys);
    if (targets.length === 0) return;

    if (typeof map.none === "string") {
      this.handlePlotNone(key, map.none, map.retryable === 1 || (map.retryable as unknown) === true);
      return;
    }

    if (map.mesh instanceof Uint8Array || map.parts) {
      for (const tileId of targets) {
        const inst = this.instances.get(tileId);
        if (inst?.solid3d) {
          inst.overlayEl.hidden = true;
          inst.solid3d.handleFrame(map);
        }
      }
      return;
    }

    const prev = this.heldChunks.get(key) || [];
    const frame = decodePlot2DFrame(map, prev);
    this.heldChunks.set(key, frame.chunks);
    for (const tileId of targets) {
      const inst = this.instances.get(tileId);
      if (inst?.plot2d) {
        inst.overlayEl.hidden = true;
        inst.plot2d.setFrame(frame);
      }
    }
  }

  setTheme(light: boolean): void {
    for (const inst of this.instances.values()) {
      inst.plot2d?.setTheme(light);
      inst.solid3d?.setTheme(light);
    }
  }

  private destroyInstance(inst: TileInstance): void {
    inst.pane?.destroy();
    inst.plot2d?.destroy();
    inst.solid3d?.destroy();
    inst.el.remove();
  }

  destroy(): void {
    this.alive = false;
    if (this.resizeTimer) {
      window.clearTimeout(this.resizeTimer);
      this.resizeTimer = 0;
    }
    this.ro?.disconnect();
    this.ro = null;
    document.removeEventListener("click", this.onDocClick);
    document.removeEventListener("keydown", this.onDocKeyDown);
    for (const inst of this.instances.values()) {
      this.destroyInstance(inst);
    }
    this.instances.clear();
    for (const s of this.splitEls) s.remove();
    this.splitEls = [];
    this.heldChunks.clear();
    this.tileKeys = {};
    this.layout = null;
    this.rootEl.remove();
  }
}
