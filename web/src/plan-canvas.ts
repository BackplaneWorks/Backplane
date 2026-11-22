// Interactive per-layer PCB routing floorplanner Canvas2D renderer & controller for Web UI.
// Matches native/gpui/src/screens/mgmt_plan_canvas.rs in visual styling, 10-step back-to-front
// paint order, hit testing, pan/zoom synchronization, and direct vertex/midpoint/keyboard steering.
import { t } from "./i18n";
import {
  planAllCanvasLayers,
  planBandStrokePx,
  planBusColor,
  planDefaultLayerPair,
  planFitCamera,
  planHitTest,
  planInsertVertex,
  planLayoutLabels,
  planPanCamera,
  planPolylineMidpoint,
  planPxToWorld,
  planRailColor,
  planRemoveVertex,
  planWorldToPx,
  planZoomCamera,
  type PlanBand,
  type PlanBoardGeom,
  type PlanCamera,
  type PlanCopperLayer,
  type PlanHit,
  type PlanLayer,
  type PlanRegion,
} from "./mgmt-model";

export type PlanViewMode = "canvas" | "split" | "list";
export type PlanGridMode = "pair" | "all" | "single";

export interface PlanCanvasStateSnapshot {
  gridMode: PlanGridMode;
  syncZoom: boolean;
  shownLayers: string[];
  selectedId: string;
  board: PlanBoardGeom;
  bands: PlanBand[];
  regions: PlanRegion[];
  layers: PlanLayer[];
  copperLayers?: PlanCopperLayer[];
}

export interface PlanCanvasCallbacks {
  onSelect: (id: string) => void;
  onFocusItem: (id: string) => void;
  onOptimisticPatch: (id: string, fields: Record<string, unknown>) => void;
  onRevertOptimistic?: (id: string) => void;
  onDragEnd?: () => void;
  onCommitFields: (id: string, fields: Record<string, unknown>) => void;
  onApproveLayer: (layer: string, approve: boolean) => void;
  onQuickApprove: (ids: string[], approve: boolean) => void;
}

const SUBSTRATE_BG = "#111917";
const BOARD_FILL = "#16201d";
const EDGE_STROKE = "rgba(212, 224, 219, 0.92)";
const PART_STROKE = "rgba(61, 80, 73, 0.72)";
const PART_OVERHANG = "rgba(82, 107, 97, 0.85)";
const REFDES_COLOR = "rgba(143, 163, 155, 0.88)";
const HANDLE_ACTIVE = "#f59e0b";
const HANDLE_BG = "#eef5f2";
const BAD_RED = "#ef4444";

function hexToRgba(hex: string, alpha: number): string {
  const h = hex.trim().replace(/^#/, "");
  if (h.length === 6) {
    const r = parseInt(h.slice(0, 2), 16);
    const g = parseInt(h.slice(2, 4), 16);
    const b = parseInt(h.slice(4, 6), 16);
    if (!Number.isNaN(r) && !Number.isNaN(g) && !Number.isNaN(b)) {
      return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }
  }
  return `rgba(20, 184, 166, ${alpha})`;
}

function bandAlphas(state: string, selected: boolean, hovered: boolean): { glow: number; body: number; core: number; bad: boolean } {
  let glow = 0.14;
  let body = 0.45;
  let core = 0.85;
  let bad = false;
  switch (state) {
    case "approved":
    case "routing":
      glow = 0.16; body = 0.51; core = 0.91; break;
    case "edited":
      glow = 0.20; body = 0.56; core = 0.95; break;
    case "routed":
    case "verified":
      glow = 0.08; body = 0.26; core = 0.59; break;
    case "rejected":
    case "failed":
      glow = 0.05; body = 0.13; core = 0.27; break;
    case "blocked":
      glow = 0.18; body = 0.48; core = 0.88; bad = true; break;
  }
  if (selected) {
    glow = Math.min(0.35, glow + 0.10);
    body = Math.min(0.71, body + 0.12);
    core = 1.0;
  } else if (hovered) {
    glow = Math.min(0.28, glow + 0.06);
    body = Math.min(0.63, body + 0.07);
  }
  return { glow, body, core, bad };
}

function regionAlphas(state: string, selected: boolean, hovered: boolean): { fill: number; edge: number; bad: boolean } {
  let fill = 0.27;
  let edge = 0.83;
  let bad = false;
  switch (state) {
    case "approved":
    case "routing":
      fill = 0.36; edge = 0.91; break;
    case "edited":
      fill = 0.38; edge = 0.95; break;
    case "routed":
    case "verified":
      fill = 0.16; edge = 0.54; break;
    case "rejected":
    case "failed":
      fill = 0.08; edge = 0.27; break;
    case "blocked":
      fill = 0.31; edge = 0.94; bad = true; break;
  }
  if (selected) {
    fill = Math.min(0.53, fill + 0.12);
    edge = 1.0;
  } else if (hovered) {
    fill = Math.min(0.45, fill + 0.06);
    edge = Math.min(1.0, edge + 0.10);
  }
  return { fill, edge, bad };
}

function layerStatusBadge(l: PlanLayer): { key: string; label: string; tone: string } {
  const items = l.items ?? [];
  if (items.some((i) => i.state === "blocked" || i.state === "failed")) return { key: "gpui.mgmt.plan.st.blocked", label: "Blocked", tone: "bad" };
  if (items.some((i) => i.state === "routing")) return { key: "gpui.mgmt.plan.st.routing", label: "Routing", tone: "accent" };
  if (items.some((i) => i.state === "proposed" || i.state === "edited")) return { key: "gpui.mgmt.plan.st.awaiting", label: "Awaiting review", tone: "warn" };
  if (items.length > 0 && items.every((i) => i.state === "routed" || i.state === "verified")) return { key: "gpui.mgmt.plan.st.routed", label: "Routed", tone: "ok" };
  if (items.length > 0 && items.every((i) => i.state === "approved" || i.state === "routed" || i.state === "verified")) return { key: "gpui.mgmt.plan.st.approved", label: "Approved", tone: "accent" };
  return { key: "gpui.mgmt.plan.st.empty", label: "Empty", tone: "faint" };
}

interface CardSlot {
  layer: string;
  el: HTMLElement;
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  legendEl: HTMLElement;
  okBtn: HTMLButtonElement | null;
  noBtn: HTMLButtonElement | null;
  statusEl: HTMLElement;
  titleEl: HTMLElement;
  subtitleEl: HTMLElement;
  hoverHudEl: HTMLElement;
  ro: ResizeObserver;
}

export class PlanFloorplannerCanvas {
  public readonly root: HTMLElement;
  private cb: PlanCanvasCallbacks;
  private state: PlanCanvasStateSnapshot | null = null;
  private cards = new Map<string, CardSlot>();
  private renderedLayerKey = "";

  private sharedCam: PlanCamera | null = null;
  private layerCams = new Map<string, PlanCamera>();
  private hover: { layer: string; hit: PlanHit } | null = null;
  private selectedVertex: { id: string; idx: number } | null = null;
  private drag:
    | { layer: string; kind: "pan"; lastX: number; lastY: number }
    | { layer: string; kind: "vertex"; id: string; isPlane: boolean; idx: number; pts: Array<[number, number]>; origPts: Array<[number, number]>; moved: boolean }
    | null = null;
  private downPos: { x: number; y: number } | null = null;
  private rafPending = 0;

  constructor(cb: PlanCanvasCallbacks) {
    this.cb = cb;
    this.root = document.createElement("div");
    this.root.className = "bp-plan-canvas-grid";
    this.root.tabIndex = 0;
    this.root.setAttribute("data-testid", "mgmt-plan-canvas-grid");
    this.root.addEventListener("keydown", (e) => this.onKeyDown(e));
  }

  public isDragging(): boolean {
    return this.drag !== null;
  }

  public fitAll(): void {
    this.sharedCam = null;
    this.layerCams.clear();
    this.scheduleDraw();
  }

  public destroy(): void {
    if (this.rafPending) {
      cancelAnimationFrame(this.rafPending);
      this.rafPending = 0;
    }
    for (const card of this.cards.values()) {
      card.ro.disconnect();
    }
    this.cards.clear();
    this.root.replaceChildren();
  }

  private visibleLayers(s: PlanCanvasStateSnapshot): PlanLayer[] {
    const all = planAllCanvasLayers(s.layers, s.copperLayers);
    if (all.length === 0) return [];
    if (s.gridMode === "all") return all;
    if (s.gridMode === "single") {
      const target =
        s.shownLayers.find((n) => all.some((l) => l.layer === n)) ??
        planDefaultLayerPair(all)[0] ??
        all[0]?.layer;
      const found = all.find((l) => l.layer === target);
      return found ? [found] : [];
    }
    const pairNames =
      s.shownLayers.length > 0 && s.shownLayers.every((n) => all.some((l) => l.layer === n))
        ? s.shownLayers.slice(0, 2)
        : planDefaultLayerPair(all);
    const out: PlanLayer[] = [];
    for (const name of pairNames) {
      const found = all.find((l) => l.layer === name);
      if (found) out.push(found);
    }
    return out.length > 0 ? out : all.slice(0, 2);
  }

  public update(snapshot: PlanCanvasStateSnapshot, twoCols: boolean): void {
    this.state = snapshot;
    const vis = this.visibleLayers(snapshot);
    this.root.classList.toggle("two-cols", twoCols && vis.length > 1);
    const key = vis.map((l) => l.layer).join("|");
    if (key !== this.renderedLayerKey) {
      this.renderedLayerKey = key;
      for (const card of this.cards.values()) card.ro.disconnect();
      this.cards.clear();
      this.root.replaceChildren();
      for (const l of vis) {
        const slot = this.buildCard(l);
        this.cards.set(l.layer, slot);
        this.root.appendChild(slot.el);
      }
    } else {
      for (const l of vis) {
        const slot = this.cards.get(l.layer);
        if (slot) this.syncCardHeaderAndLegend(slot, l);
      }
    }
    this.scheduleDraw();
  }

  private effectiveBbox(board: PlanBoardGeom): [number, number, number, number] {
    const bb = board.bbox;
    if (!bb || bb.length !== 4 || Math.abs(bb[2] - bb[0]) < 0.5 || Math.abs(bb[3] - bb[1]) < 0.5) {
      return [0, 0, 100, 70];
    }
    return bb;
  }

  private getCam(layer: string, w: number, h: number): PlanCamera {
    const s = this.state;
    const raw = s?.syncZoom ? this.sharedCam : this.layerCams.get(layer) ?? null;
    if (raw && raw.touched) return raw;
    return planFitCamera(s ? this.effectiveBbox(s.board) : [0, 0, 100, 70], w, h, 14);
  }

  private setCam(layer: string, cam: PlanCamera): void {
    if (this.state?.syncZoom) {
      this.sharedCam = cam;
    } else {
      this.layerCams.set(layer, cam);
    }
  }

  private buildCard(l: PlanLayer): CardSlot {
    const el = document.createElement("section");
    el.className = "bp-plan-card";
    el.setAttribute("data-testid", `mgmt-plan-card-${l.layer}`);

    const head = document.createElement("div");
    head.className = "bp-plan-card-head";
    const headTop = document.createElement("div");
    headTop.className = "bp-plan-card-head-top";

    const bar = document.createElement("span");
    const isPower = l.role === "power" || ((l.items?.length ?? 0) > 0 && (l.items ?? []).every((i) => i.kind === "plane"));
    bar.className = "bp-plan-card-bar " + (isPower ? "pwr" : "sig");

    const titleEl = document.createElement("span");
    titleEl.className = "bp-plan-card-title";
    const statusEl = document.createElement("span");
    statusEl.className = "bp-plan-status-badge";

    const okBtn = document.createElement("button");
    okBtn.type = "button";
    okBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
    okBtn.textContent = "✓";
    okBtn.title = t("gpui.mgmt.plan.layer", "Approve layer");
    okBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.cb.onApproveLayer(l.layer, true);
    });

    const noBtn = document.createElement("button");
    noBtn.type = "button";
    noBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
    noBtn.textContent = "✗";
    noBtn.title = t("gpui.mgmt.plan.rejectlayer", "Reject layer");
    noBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.cb.onApproveLayer(l.layer, false);
    });

    headTop.append(bar, titleEl, statusEl, okBtn, noBtn);
    const subtitleEl = document.createElement("div");
    subtitleEl.className = "bp-plan-card-sub";
    head.append(headTop, subtitleEl);

    const wrap = document.createElement("div");
    wrap.className = "bp-plan-card-canvas-wrap";
    const canvas = document.createElement("canvas");
    canvas.className = "bp-plan-card-canvas";
    canvas.setAttribute("data-testid", `mgmt-plan-canvas-${l.layer}`);
    const hoverHudEl = document.createElement("div");
    hoverHudEl.className = "bp-plan-hover-hud bp-hidden";
    wrap.append(canvas, hoverHudEl);

    const legendEl = document.createElement("div");
    legendEl.className = "bp-plan-legend bp-hidden";

    el.append(head, wrap, legendEl);

    const ctx = canvas.getContext("2d")!;
    const ro = new ResizeObserver(() => this.scheduleDraw());
    ro.observe(wrap);

    this.attachPointerListeners(canvas, l.layer);

    const slot: CardSlot = {
      layer: l.layer,
      el,
      canvas,
      ctx,
      legendEl,
      okBtn,
      noBtn,
      statusEl,
      titleEl,
      subtitleEl,
      hoverHudEl,
      ro,
    };
    this.syncCardHeaderAndLegend(slot, l);
    return slot;
  }

  private syncCardHeaderAndLegend(slot: CardSlot, l: PlanLayer): void {
    slot.titleEl.textContent = l.title || l.layer;
    const sub = l.subtitle || l.stackup || "";
    slot.subtitleEl.textContent = sub;
    slot.subtitleEl.classList.toggle("bp-hidden", !sub);

    const st = layerStatusBadge(l);
    slot.statusEl.className = `bp-plan-status-badge tone-${st.tone}`;
    slot.statusEl.textContent = t(st.key, st.label);

    const hasAwaiting = (l.items ?? []).some((i) => i.state === "proposed" || i.state === "edited");
    if (slot.okBtn) slot.okBtn.classList.toggle("bp-hidden", !hasAwaiting);
    if (slot.noBtn) slot.noBtn.classList.toggle("bp-hidden", !hasAwaiting);

    const regions = (this.state?.regions ?? []).filter((r) => r.layer === l.layer);
    if (regions.length === 0) {
      slot.legendEl.classList.add("bp-hidden");
      slot.legendEl.replaceChildren();
    } else {
      slot.legendEl.classList.remove("bp-hidden");
      const frag = document.createDocumentFragment();
      for (const r of regions) {
        const chip = document.createElement("button");
        chip.type = "button";
        const isSel = this.state?.selectedId === r.id;
        chip.className = "bp-plan-legend-chip" + (isSel ? " active" : "");
        const sw = document.createElement("span");
        sw.className = "bp-plan-legend-sw";
        sw.style.background = planRailColor(r.net || r.id, r.color || "");
        const tx = document.createElement("span");
        tx.textContent = r.net || r.id;
        chip.append(sw, tx);
        chip.addEventListener("click", () => {
          this.cb.onSelect(r.id);
          this.cb.onFocusItem(r.id);
        });
        frag.appendChild(chip);
      }
      slot.legendEl.replaceChildren(frag);
    }
  }

  private attachPointerListeners(canvas: HTMLCanvasElement, layer: string): void {
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    canvas.addEventListener("dblclick", () => this.fitAll());

    canvas.addEventListener("pointerdown", (e) => {
      const s = this.state;
      if (!s) return;
      this.root.focus({ preventScroll: true });
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const w = Math.max(1, rect.width);
      const h = Math.max(1, rect.height);
      const cam = this.getCam(layer, w, h);
      const hit = planHitTest(s.bands, s.regions, layer, s.selectedId, cam, w, h, mx, my);

      if (e.button === 2) {
        if (hit?.kind === "vertex") {
          const pts = this.getItemPts(hit.id, hit.isPlane);
          const nextPts = planRemoveVertex(pts, hit.idx, hit.isPlane);
          if (nextPts) {
            this.selectedVertex = null;
            this.cb.onCommitFields(hit.id, { [hit.isPlane ? "outline" : "path"]: nextPts });
          }
        }
        return;
      }

      if (e.button !== 0) return;
      canvas.setPointerCapture(e.pointerId);
      this.downPos = { x: e.clientX, y: e.clientY };

      if (hit?.kind === "vertex") {
        const pts = this.getItemPts(hit.id, hit.isPlane);
        const origPts = pts.map((p) => [p[0], p[1]] as [number, number]);
        this.selectedVertex = { id: hit.id, idx: hit.idx };
        this.drag = { layer, kind: "vertex", id: hit.id, isPlane: hit.isPlane, idx: hit.idx, pts: pts.map((p) => [p[0], p[1]]), origPts, moved: false };
        this.scheduleDraw();
      } else if (hit?.kind === "midpoint") {
        const curPts = this.getItemPts(hit.id, hit.isPlane);
        const origPts = curPts.map((p) => [p[0], p[1]] as [number, number]);
        const newPts = planInsertVertex(curPts, hit.segIdx, hit.pos, hit.isPlane);
        const insertedIdx = hit.isPlane && hit.segIdx + 1 >= curPts.length ? newPts.length - 1 : Math.min(hit.segIdx + 1, newPts.length - 1);
        this.selectedVertex = { id: hit.id, idx: insertedIdx };
        this.patchLocalItem(hit.id, hit.isPlane, newPts);
        this.cb.onOptimisticPatch(hit.id, { [hit.isPlane ? "outline" : "path"]: newPts });
        this.drag = { layer, kind: "vertex", id: hit.id, isPlane: hit.isPlane, idx: insertedIdx, pts: newPts, origPts, moved: true };
        this.scheduleDraw();
      } else {
        this.selectedVertex = null;
        this.drag = { layer, kind: "pan", lastX: mx, lastY: my };
      }
    });

    canvas.addEventListener("pointermove", (e) => {
      const s = this.state;
      if (!s) return;
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const w = Math.max(1, rect.width);
      const h = Math.max(1, rect.height);

      if (this.drag && this.drag.layer === layer) {
        if (this.drag.kind === "pan") {
          const cam = this.getCam(layer, w, h);
          this.setCam(layer, planPanCamera(cam, mx - this.drag.lastX, my - this.drag.lastY));
          this.drag.lastX = mx;
          this.drag.lastY = my;
          this.scheduleDraw();
          return;
        }
        if (this.downPos && Math.hypot(e.clientX - this.downPos.x, e.clientY - this.downPos.y) < 1.5 && !this.drag.moved) {
          return;
        }
        const cam = this.getCam(layer, w, h);
        const [wx, wy] = planPxToWorld(cam, mx, my, w, h);
        const rounded: [number, number] = [Math.round(wx * 1000) / 1000, Math.round(wy * 1000) / 1000];
        this.drag.moved = true;
        this.drag.pts[this.drag.idx] = rounded;
        const copy = this.drag.pts.map((p) => [p[0], p[1]] as [number, number]);
        this.patchLocalItem(this.drag.id, this.drag.isPlane, copy);
        this.cb.onOptimisticPatch(this.drag.id, { [this.drag.isPlane ? "outline" : "path"]: copy });
        this.scheduleDraw();
        return;
      }

      const cam = this.getCam(layer, w, h);
      const hit = planHitTest(s.bands, s.regions, layer, s.selectedId, cam, w, h, mx, my);
      const prevId = this.hover?.hit ? ("id" in this.hover.hit ? this.hover.hit.id : "") : "";
      const prevKind = this.hover?.hit.kind ?? "";
      const nextId = hit ? hit.id : "";
      this.hover = hit ? { layer, hit } : null;
      canvas.style.cursor = hit ? "pointer" : "crosshair";
      if (prevId !== nextId || (hit?.kind ?? "") !== prevKind) {
        this.syncHoverHud(layer);
        this.scheduleDraw();
      }
    });

    canvas.addEventListener("pointerleave", () => {
      if (this.hover?.layer === layer) {
        this.hover = null;
        this.syncHoverHud(layer);
        this.scheduleDraw();
      }
    });

    const cancelActiveDrag = (): void => {
      const d = this.drag;
      if (!d || d.layer !== layer) return;
      this.drag = null;
      this.downPos = null;
      if (d.kind === "vertex") {
        const copy = d.origPts.map((p) => [p[0], p[1]] as [number, number]);
        this.patchLocalItem(d.id, d.isPlane, copy);
        this.cb.onRevertOptimistic?.(d.id);
      } else {
        this.cb.onDragEnd?.();
      }
      this.scheduleDraw();
    };
    canvas.addEventListener("pointercancel", cancelActiveDrag);
    canvas.addEventListener("lostpointercapture", cancelActiveDrag);

    canvas.addEventListener("pointerup", (e) => {
      const s = this.state;
      const d = this.drag;
      const down = this.downPos;
      this.drag = null;
      this.downPos = null;
      if (canvas.hasPointerCapture(e.pointerId)) {
        canvas.releasePointerCapture(e.pointerId);
      }
      if (d && d.kind === "vertex" && d.layer === layer) {
        if (d.moved) {
          this.cb.onCommitFields(d.id, { [d.isPlane ? "outline" : "path"]: d.pts });
        } else {
          this.cb.onDragEnd?.();
          this.scheduleDraw();
        }
        return;
      }
      if (s && down && Math.hypot(e.clientX - down.x, e.clientY - down.y) < 4.5) {
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const w = Math.max(1, rect.width);
        const h = Math.max(1, rect.height);
        const cam = this.getCam(layer, w, h);
        const hit = planHitTest(s.bands, s.regions, layer, s.selectedId, cam, w, h, mx, my);
        if (hit && (hit.kind === "band" || hit.kind === "region")) {
          this.cb.onSelect(hit.id);
          this.cb.onFocusItem(hit.id);
          return;
        }
        if (!hit && s.selectedId) {
          this.cb.onSelect("");
          return;
        }
      }
      this.cb.onDragEnd?.();
    });

    canvas.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const w = Math.max(1, rect.width);
        const h = Math.max(1, rect.height);
        const cam = this.getCam(layer, w, h);
        const factor = Math.max(0.65, Math.min(1.55, 1 - e.deltaY * 0.002));
        this.setCam(layer, planZoomCamera(cam, factor, mx, my, w, h));
        this.scheduleDraw();
      },
      { passive: false },
    );
  }

  private patchLocalItem(id: string, isPlane: boolean, pts: Array<[number, number]>): void {
    const s = this.state;
    if (!s) return;
    if (isPlane) {
      const r = s.regions.find((x) => x.id === id);
      if (r) { r.outline = pts; r.edited = true; if (r.state === "proposed") r.state = "edited"; }
    } else {
      const b = s.bands.find((x) => x.id === id);
      if (b) { b.path = pts; b.edited = true; if (b.state === "proposed") b.state = "edited"; }
    }
  }

  private getItemPts(id: string, isPlane: boolean): Array<[number, number]> {
    const s = this.state;
    if (!s) return [];
    if (isPlane) {
      return (s.regions.find((r) => r.id === id)?.outline ?? []).map((p) => [p[0], p[1]]);
    }
    return (s.bands.find((b) => b.id === id)?.path ?? []).map((p) => [p[0], p[1]]);
  }

  private syncHoverHud(layer: string): void {
    const slot = this.cards.get(layer);
    const s = this.state;
    if (!slot || !s) return;
    if (!this.hover || this.hover.layer !== layer) {
      slot.hoverHudEl.classList.add("bp-hidden");
      slot.hoverHudEl.textContent = "";
      return;
    }
    const hid = this.hover.hit.id;
    const band = s.bands.find((b) => b.id === hid);
    const region = s.regions.find((r) => r.id === hid);
    let txt = "";
    if (band) {
      txt = `${band.name || hid} · ${band.nets ?? 0}n · ${(band.width ?? 1.5).toFixed(2)}mm`;
    } else if (region) {
      txt = region.reference ? `${region.net || hid} · ${region.reference}` : region.net || hid;
    }
    if (!txt) {
      slot.hoverHudEl.classList.add("bp-hidden");
    } else {
      slot.hoverHudEl.classList.remove("bp-hidden");
      slot.hoverHudEl.textContent = txt;
    }
  }

  private onKeyDown(e: KeyboardEvent): void {
    const s = this.state;
    if (!s) return;
    if (e.key === "0" || e.key === "Home") {
      e.preventDefault();
      this.fitAll();
      return;
    }
    if (e.key === "Escape") {
      this.selectedVertex = null;
      this.cb.onSelect("");
      return;
    }
    if (!s.selectedId) return;
    const band = s.bands.find((b) => b.id === s.selectedId);
    const region = s.regions.find((r) => r.id === s.selectedId);
    const locked = Boolean(band?.locked ?? region?.locked);
    const k = e.key.toLowerCase();
    if (k === "a") {
      e.preventDefault();
      this.cb.onQuickApprove([s.selectedId], true);
    } else if (k === "r") {
      e.preventDefault();
      this.cb.onQuickApprove([s.selectedId], false);
    } else if (k === "l") {
      e.preventDefault();
      this.cb.onCommitFields(s.selectedId, { locked: !locked });
    } else if (k === "w" && band && !locked) {
      e.preventDefault();
      const delta = e.shiftKey ? -0.25 : 0.25;
      const nw = Math.round(Math.max(0.15, Math.min(25, (band.width ?? 1.5) + delta)) * 100) / 100;
      this.cb.onCommitFields(s.selectedId, { width: nw });
    } else if ((e.key === "Delete" || e.key === "Backspace") && !locked && this.selectedVertex?.id === s.selectedId) {
      e.preventDefault();
      const isPlane = Boolean(region);
      const pts = this.getItemPts(s.selectedId, isPlane);
      const next = planRemoveVertex(pts, this.selectedVertex.idx, isPlane);
      if (next) {
        this.selectedVertex = null;
        this.cb.onCommitFields(s.selectedId, { [isPlane ? "outline" : "path"]: next });
      }
    }
  }

  private scheduleDraw(): void {
    if (this.rafPending) return;
    this.rafPending = requestAnimationFrame(() => {
      this.rafPending = 0;
      this.drawAll();
    });
  }

  private drawAll(): void {
    const s = this.state;
    if (!s) return;
    const aspect = Math.max(
      0.45,
      Math.min(1.25, Math.abs(this.effectiveBbox(s.board)[3] - this.effectiveBbox(s.board)[1]) / Math.max(1, Math.abs(this.effectiveBbox(s.board)[2] - this.effectiveBbox(s.board)[0]))),
    );
    for (const slot of this.cards.values()) {
      const wrap = slot.canvas.parentElement;
      if (!wrap) continue;
      const cssW = Math.max(120, Math.round(wrap.clientWidth));
      const cssH = Math.max(165, Math.min(250, Math.round(cssW * aspect)));
      slot.canvas.style.height = `${cssH}px`;
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      const pw = Math.round(cssW * dpr);
      const ph = Math.round(cssH * dpr);
      if (slot.canvas.width !== pw || slot.canvas.height !== ph) {
        slot.canvas.width = pw;
        slot.canvas.height = ph;
      }
      slot.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      this.drawLayer(slot.ctx, slot.layer, cssW, cssH, s);
    }
  }

  private drawLayer(ctx: CanvasRenderingContext2D, layer: string, w: number, h: number, s: PlanCanvasStateSnapshot): void {
    const cam = this.getCam(layer, w, h);
    const toPx = (x: number, y: number): [number, number] => planWorldToPx(cam, x, y, w, h);

    // 1. Dark PCB substrate
    ctx.fillStyle = SUBSTRATE_BG;
    ctx.fillRect(0, 0, w, h);

    // 2. Board interior (#16201d)
    const outline = (s.board.outline ?? []).filter((p) => p.length >= 3);
    ctx.fillStyle = BOARD_FILL;
    if (outline.length > 0) {
      ctx.beginPath();
      for (const poly of outline) {
        for (let i = 0; i < poly.length; i++) {
          const [px, py] = toPx(poly[i][0], poly[i][1]);
          if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.closePath();
      }
      ctx.fill("evenodd");
    } else {
      const bb = this.effectiveBbox(s.board);
      const [x0, y0] = toPx(bb[0], bb[1]);
      const [x1, y1] = toPx(bb[2], bb[3]);
      ctx.fillRect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    }

    // 3. Power plane pours (on this layer, beneath components)
    const layerRegions = s.regions.filter((r) => r.layer === layer);
    const hovHit = this.hover?.layer === layer ? this.hover.hit : null;
    for (const r of layerRegions) {
      const outer = r.outline ?? [];
      if (outer.length < 3) continue;
      const sel = s.selectedId === r.id;
      const hov = hovHit?.id === r.id;
      const al = regionAlphas(r.state || "proposed", sel, hov);
      const col = planRailColor(r.net || r.id, r.color || "");

      ctx.beginPath();
      for (let i = 0; i < outer.length; i++) {
        const [px, py] = toPx(outer[i][0], outer[i][1]);
        if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
      }
      ctx.closePath();
      for (const hole of r.holes ?? []) {
        if (hole.length < 3) continue;
        for (let i = 0; i < hole.length; i++) {
          const [px, py] = toPx(hole[i][0], hole[i][1]);
          if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.closePath();
      }
      ctx.fillStyle = hexToRgba(col, al.fill);
      ctx.fill("evenodd");

      ctx.beginPath();
      for (let i = 0; i < outer.length; i++) {
        const [px, py] = toPx(outer[i][0], outer[i][1]);
        if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.lineWidth = sel || al.bad ? 2 : 1.25;
      ctx.lineJoin = "round";
      ctx.strokeStyle = al.bad ? BAD_RED : sel ? "#ffffff" : hexToRgba(col, al.edge);
      ctx.stroke();
    }

    // 4. Crisp Edge.Cuts outline
    ctx.strokeStyle = EDGE_STROKE;
    ctx.lineWidth = 1.25;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    if (outline.length > 0) {
      ctx.beginPath();
      for (const poly of outline) {
        for (let i = 0; i < poly.length; i++) {
          const [px, py] = toPx(poly[i][0], poly[i][1]);
          if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.closePath();
      }
      ctx.stroke();
    } else {
      const bb = this.effectiveBbox(s.board);
      const [x0, y0] = toPx(bb[0], bb[1]);
      const [x1, y1] = toPx(bb[2], bb[3]);
      ctx.strokeRect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    }

    // 5. Component courtyard/body outlines (solid top, dashed bottom/overhang)
    const parts = s.board.parts ?? [];
    ctx.lineWidth = 0.85;
    ctx.strokeStyle = PART_STROKE;
    ctx.setLineDash([]);
    ctx.beginPath();
    for (const p of parts) {
      if (p.o) continue;
      const [x0, y0] = toPx(p.b[0], p.b[1]);
      const [x1, y1] = toPx(p.b[2], p.b[3]);
      ctx.rect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    }
    ctx.stroke();

    ctx.lineWidth = 0.95;
    ctx.strokeStyle = PART_OVERHANG;
    ctx.setLineDash([4, 3]);
    ctx.beginPath();
    for (const p of parts) {
      if (!p.o) continue;
      const [x0, y0] = toPx(p.b[0], p.b[1]);
      const [x1, y1] = toPx(p.b[2], p.b[3]);
      ctx.rect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    }
    ctx.stroke();
    ctx.setLineDash([]);

    // 6. Collision-free refdes labels
    const layerBands = s.bands.filter((b) => b.layer === layer);
    const pinnedRefs: string[] = [];
    for (const b of layerBands) {
      for (const r of b.from ?? []) pinnedRefs.push(r);
      for (const r of b.to ?? []) pinnedRefs.push(r);
    }
    ctx.fillStyle = REFDES_COLOR;
    ctx.font = "9.5px ui-monospace, 'SFMono-Regular', Menlo, monospace";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    for (const lbl of planLayoutLabels(parts, cam, w, h, pinnedRefs)) {
      ctx.fillText(lbl.text, lbl.cx, lbl.cy);
    }

    // 7. Faint signal ratsnest guides
    for (const b of layerBands) {
      const guides = b.guides ?? [];
      if (guides.length === 0) continue;
      const active = s.selectedId === b.id || hovHit?.id === b.id;
      const col = planBusColor(b.name || b.id, b.color || "");
      ctx.strokeStyle = hexToRgba(col, active ? 0.38 : 0.17);
      ctx.lineWidth = active ? 1 : 0.7;
      ctx.beginPath();
      for (const seg of guides) {
        const [x1, y1] = toPx(seg[0], seg[1]);
        const [x2, y2] = toPx(seg[2], seg[3]);
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
      }
      ctx.stroke();
    }

    // 8, 9, 10. Multi-pass rounded-cap signal bus corridors + diamond vias + net-count badges
    for (const b of layerBands) {
      const pts = b.path ?? [];
      if (pts.length < 2) continue;
      const sel = s.selectedId === b.id;
      const hov = hovHit?.id === b.id;
      const al = bandAlphas(b.state || "proposed", sel, hov);
      const col = planBusColor(b.name || b.id, b.color || "");
      const wpx = planBandStrokePx(b.width ?? 1.5, cam.scale);

      const tracePath = () => {
        ctx.beginPath();
        for (let i = 0; i < pts.length; i++) {
          const [px, py] = toPx(pts[i][0], pts[i][1]);
          if (i === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
      };

      ctx.lineCap = "round";
      ctx.lineJoin = "round";

      // Pass 1: outer halo glow
      tracePath();
      ctx.strokeStyle = hexToRgba(col, al.glow);
      ctx.lineWidth = wpx + 6;
      ctx.stroke();

      if (al.bad || sel) {
        tracePath();
        ctx.strokeStyle = al.bad ? BAD_RED : "rgba(255,255,255,0.75)";
        ctx.lineWidth = wpx + 2.5;
        ctx.stroke();
      }

      // Pass 2: corridor envelope
      tracePath();
      ctx.strokeStyle = hexToRgba(col, al.body);
      ctx.lineWidth = wpx;
      ctx.stroke();

      // Pass 3: crisp inner spine
      tracePath();
      ctx.strokeStyle = hexToRgba(col, al.core);
      ctx.lineWidth = Math.max(1.8, Math.min(5, wpx * 0.34));
      ctx.stroke();

      // 9. Diamond vias
      for (const v of b.vias ?? []) {
        const [vx, vy] = toPx(v[0], v[1]);
        const r = 4.2;
        ctx.beginPath();
        ctx.moveTo(vx, vy - r);
        ctx.lineTo(vx + r, vy);
        ctx.lineTo(vx, vy + r);
        ctx.lineTo(vx - r, vy);
        ctx.closePath();
        ctx.fillStyle = "rgba(17, 25, 23, 0.95)";
        ctx.fill();
        ctx.lineWidth = 1.5;
        ctx.strokeStyle = sel ? "#ffffff" : col;
        ctx.stroke();
      }

      // 10. Centered net-count pill
      const nets = b.nets ?? 0;
      if (nets > 0) {
        const mid = planPolylineMidpoint(pts);
        if (mid) {
          const [mx, my] = toPx(mid[0], mid[1]);
          const txt = String(nets);
          const pw = Math.max(16, txt.length * 6.5 + 8);
          const ph = 15;
          ctx.fillStyle = "rgba(17, 25, 23, 0.94)";
          ctx.fillRect(mx - pw * 0.5, my - ph * 0.5, pw, ph);
          ctx.lineWidth = 1.2;
          ctx.strokeStyle = sel ? "#ffffff" : col;
          ctx.strokeRect(mx - pw * 0.5, my - ph * 0.5, pw, ph);
          ctx.fillStyle = "#eef5f2";
          ctx.font = "bold 10px ui-monospace, 'SFMono-Regular', Menlo, monospace";
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(txt, mx, my + 0.5);
        }
      }
    }

    // 11. Selected item vertex & midpoint handles
    if (s.selectedId) {
      const paintHandles = (pts: Array<[number, number]>, closed: boolean) => {
        const segs = closed ? pts.length : Math.max(0, pts.length - 1);
        for (let i = 0; i < segs; i++) {
          const a = pts[i];
          const b = pts[(i + 1) % pts.length];
          const [hx, hy] = toPx((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5);
          const isHov = hovHit?.kind === "midpoint" && hovHit.id === s.selectedId && hovHit.segIdx === i;
          const sz = isHov ? 9 : 7.5;
          ctx.fillStyle = "rgba(22, 32, 29, 0.92)";
          ctx.fillRect(hx - sz * 0.5, hy - sz * 0.5, sz, sz);
          ctx.lineWidth = 1;
          ctx.strokeStyle = isHov ? HANDLE_ACTIVE : "#94a3b8";
          ctx.strokeRect(hx - sz * 0.5, hy - sz * 0.5, sz, sz);
        }
        for (let i = 0; i < pts.length; i++) {
          const [vx, vy] = toPx(pts[i][0], pts[i][1]);
          const isSelV = this.selectedVertex?.id === s.selectedId && this.selectedVertex.idx === i;
          const isHovV = hovHit?.kind === "vertex" && hovHit.id === s.selectedId && hovHit.idx === i;
          const sz = isSelV || isHovV ? 9.5 : 7.5;
          ctx.fillStyle = isSelV || isHovV ? HANDLE_ACTIVE : HANDLE_BG;
          ctx.fillRect(vx - sz * 0.5, vy - sz * 0.5, sz, sz);
          ctx.lineWidth = 1.2;
          ctx.strokeStyle = SUBSTRATE_BG;
          ctx.strokeRect(vx - sz * 0.5, vy - sz * 0.5, sz, sz);
        }
      };
      for (const b of layerBands) {
        if (b.id === s.selectedId && !b.locked) paintHandles(b.path ?? [], false);
      }
      for (const r of layerRegions) {
        if (r.id === s.selectedId && !r.locked) paintHandles(r.outline ?? [], true);
      }
    }
  }
}
