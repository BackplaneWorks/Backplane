// Canvas2D renderer for Backplane 2D plots: Board, Schematic, Gerbers, and Gerber Compare.
// Mirrors native/gpui/src/screens/plotview.rs — hub decides chunk order, colors, and layer masks;
// this renderer is a pure camera + hit-tester with zero idle repaint loops.

import type { Plot2DFrame, PlotChunk } from "./cbor.ts";

export interface LayerMasks {
  off: number;
  dim: number;
  /** Layer index to lift on top (255 = NO_LIFT), matching GPUI layers.rs `Masks::lift`. */
  lift: number;
}

const NO_LIFT = 255;
const LAYER_DIM = 0.25;

export interface PickHit {
  at: number;
  idx: number;
}

export interface Plot2DCallbacks {
  onPick?: (hit: PickHit | null) => void;
  onUserMove?: () => void;
}

function rgbCss(color: number, alpha: number): string {
  const r = (color >>> 16) & 0xff;
  const g = (color >>> 8) & 0xff;
  const b = color & 0xff;
  return alpha >= 0.999 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${alpha.toFixed(3)})`;
}

export class Plot2DViewer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private frame: Plot2DFrame | null = null;
  private masks: LayerMasks = { off: 0, dim: 0, lift: NO_LIFT };
  private highlights = new Set<string>();
  private lightTheme = false;

  // Camera: world (µm) -> screen CSS px: sx = (wx - cx) * scale + w/2
  private cx = 0;
  private cy = 0;
  private scale = 1e-4;
  private fittedKey = "";
  private fitW = 0;
  private fitH = 0;
  private userMoved = false;
  private dirty = false;
  private rafId = 0;

  // Pointer & touch state
  private pointers = new Map<number, { x: number; y: number }>();
  private dragStart: { x: number; y: number; cx: number; cy: number; moved: boolean } | null = null;
  private pinchStart: { dist: number; midX: number; midY: number; scale: number; cx: number; cy: number } | null = null;
  private resizeObs: ResizeObserver;
  private cbs: Plot2DCallbacks;

  constructor(container: HTMLElement, cbs: Plot2DCallbacks = {}) {
    this.cbs = cbs;
    this.canvas = document.createElement("canvas");
    this.canvas.className = "bp-viewer-canvas";
    this.canvas.style.width = "100%";
    this.canvas.style.height = "100%";
    this.canvas.style.display = "block";
    this.canvas.style.touchAction = "none";
    container.appendChild(this.canvas);

    const ctx = this.canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas2D unavailable");
    this.ctx = ctx;

    this.resizeObs = new ResizeObserver(() => this.scheduleDraw());
    this.resizeObs.observe(this.canvas);

    this.canvas.addEventListener("pointerdown", this.onPointerDown);
    this.canvas.addEventListener("pointermove", this.onPointerMove);
    this.canvas.addEventListener("pointerup", this.onPointerUp);
    this.canvas.addEventListener("pointercancel", this.onPointerUp);
    this.canvas.addEventListener("wheel", this.onWheel, { passive: false });
    this.canvas.addEventListener("dblclick", this.onDblClick);
  }

  setTheme(light: boolean): void {
    if (this.lightTheme !== light) {
      this.lightTheme = light;
      this.scheduleDraw();
    }
  }

  setFrame(frame: Plot2DFrame | null): void {
    this.frame = frame;
    if (frame && frame.key !== this.fittedKey) {
      this.fittedKey = frame.key;
      this.fit();
    } else {
      this.scheduleDraw();
    }
  }

  setLayers(masks: LayerMasks): void {
    this.masks = masks;
    this.scheduleDraw();
  }

  setHighlights(matches: [number, number][]): void {
    this.highlights.clear();
    for (const [at, idx] of matches) this.highlights.add(`${at}:${idx}`);
    this.scheduleDraw();
  }

  /** Present layer bitmask for `viewer.layers`. */
  presentMask(): number {
    if (!this.frame) return 0;
    let mask = 0;
    for (const ch of this.frame.chunks) {
      if (ch.layer < 32) mask = (mask | (1 << ch.layer)) >>> 0;
    }
    return mask >>> 0;
  }

  /** First chunk colour per present layer (0xRRGGBB), for the Layers popover swatches. */
  presentColors(): Map<number, number> {
    const out = new Map<number, number>();
    if (!this.frame) return out;
    for (const ch of this.frame.chunks) {
      if (!out.has(ch.layer)) out.set(ch.layer, ch.color);
    }
    return out;
  }

  private applyFit(w: number, h: number): void {
    if (!this.frame || w < 10 || h < 10) return;
    const b = this.frame.box;
    const bw = Math.max(b[2] - b[0], 1000);
    const bh = Math.max(b[3] - b[1], 1000);
    this.cx = (b[0] + b[2]) * 0.5;
    this.cy = (b[1] + b[3]) * 0.5;
    this.scale = Math.min(w / bw, h / bh) * 0.92;
    this.fitW = w;
    this.fitH = h;
    this.userMoved = false;
  }

  fit(): void {
    if (!this.frame) return;
    this.userMoved = false;
    this.fitW = 0;
    this.fitH = 0;
    const rect = this.canvas.getBoundingClientRect();
    if (rect.width >= 10 && rect.height >= 10) {
      this.applyFit(rect.width, rect.height);
    }
    this.scheduleDraw();
  }

  /** Zoom to a box in millimetres `[x0, y0, x1, y1]` (Follow agent / finding focus). */
  zoomToBoxMm(boxMm: [number, number, number, number]): void {
    const x0 = boxMm[0] * 1000;
    const y0 = boxMm[1] * 1000;
    const x1 = boxMm[2] * 1000;
    const y1 = boxMm[3] * 1000;
    const bw = Math.max(x1 - x0, 1000);
    const bh = Math.max(y1 - y0, 1000);
    const rect = this.canvas.getBoundingClientRect();
    const w = Math.max(rect.width, 200);
    const h = Math.max(rect.height, 200);
    this.cx = (x0 + x1) * 0.5;
    this.cy = (y0 + y1) * 0.5;
    this.scale = Math.min(w / bw, h / bh) * 0.85;
    this.userMoved = true;
    this.scheduleDraw();
  }

  private scheduleDraw(): void {
    if (this.dirty) return;
    this.dirty = true;
    this.rafId = requestAnimationFrame(() => {
      this.dirty = false;
      this.rafId = 0;
      this.draw();
    });
  }

  private draw(): void {
    const rect = this.canvas.getBoundingClientRect();
    const w = Math.max(1, Math.round(rect.width));
    const h = Math.max(1, Math.round(rect.height));
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const pw = Math.round(w * dpr);
    const ph = Math.round(h * dpr);
    if (this.canvas.width !== pw || this.canvas.height !== ph) {
      this.canvas.width = pw;
      this.canvas.height = ph;
    }

    const ctx = this.ctx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    // Match GPUI plotview.rs `.bg(c("view"))` (`--view` in assets/web/style.css).
    ctx.fillStyle = this.lightTheme ? "#f5f4ef" : "#08090b";
    ctx.fillRect(0, 0, w, h);

    const frame = this.frame;
    if (!frame) return;

    // Auto-refit untouched camera when first painted or when the canvas resizes (matches GPUI `refit_needed`).
    if (!this.userMoved && w >= 10 && h >= 10 && (Math.abs(this.fitW - w) > 2 || Math.abs(this.fitH - h) > 2)) {
      this.applyFit(w, h);
    }

    const halfW = w * 0.5;
    const halfH = h * 0.5;
    const scale = this.scale;
    const cx = this.cx;
    const cy = this.cy;

    // Visible world bounds in µm for viewport culling
    const vx0 = cx - halfW / scale;
    const vx1 = cx + halfW / scale;
    const vy0 = cy - halfH / scale;
    const vy1 = cy + halfH / scale;

    // Order chunks exactly like GPUI `viewers::layers::plan`:
    // hidden layers (`off` bitmask) omitted; layers outside 0..31 always shown;
    // `lift` is a layer index (255 = NO_LIFT) painted after all other layers.
    const { off, dim, lift } = this.masks;
    const normalOrder: number[] = [];
    const liftedOrder: number[] = [];
    for (let i = 0; i < frame.chunks.length; i++) {
      const layer = frame.chunks[i].layer;
      if (layer < 32 && ((off >>> layer) & 1) === 1) continue;
      if (layer === lift) liftedOrder.push(i);
      else normalOrder.push(i);
    }

    const drawOrder = normalOrder.concat(liftedOrder);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";

    for (let oi = 0; oi < drawOrder.length; oi++) {
      const ci = drawOrder[oi];
      const ch = frame.chunks[ci];
      const isDim = ch.layer < 32 && ((dim >>> ch.layer) & 1) === 1;
      const alpha = ch.alpha * (isDim ? LAYER_DIM : 1);
      if (alpha <= 0.01) continue;

      this.drawChunk(ctx, ch, ci, alpha, vx0, vy0, vx1, vy1, cx, cy, scale, halfW, halfH);
    }
  }

  private drawChunk(
    ctx: CanvasRenderingContext2D,
    ch: PlotChunk,
    chunkIdx: number,
    alpha: number,
    vx0: number,
    vy0: number,
    vx1: number,
    vy1: number,
    cx: number,
    cy: number,
    scale: number,
    halfW: number,
    halfH: number
  ): void {
    const pts = ch.pts;
    const hasHl = this.highlights.size > 0;
    const baseColor = rgbCss(ch.color, alpha);
    const hlColor = "rgba(255, 224, 64, 0.95)";

    if (ch.mode === 0) {
      // Fill polygons
      ctx.fillStyle = baseColor;
      ctx.beginPath();
      let count = 0;
      for (let pi = 0; pi < ch.pieces.length; pi++) {
        const p = ch.pieces[pi];
        if (p.x1 < vx0 || p.x0 > vx1 || p.y1 < vy0 || p.y0 > vy1) continue;
        if (p.n < 3) continue;
        const x0 = (pts[p.at] - cx) * scale + halfW;
        const y0 = (pts[p.at + 1] - cy) * scale + halfH;
        ctx.moveTo(x0, y0);
        for (let k = 1; k < p.n; k++) {
          ctx.lineTo(
            (pts[p.at + k * 2] - cx) * scale + halfW,
            (pts[p.at + k * 2 + 1] - cy) * scale + halfH
          );
        }
        ctx.closePath();
        count++;
      }
      if (count > 0) ctx.fill();
    } else if (ch.mode === 2) {
      // Dots
      const rad = Math.max(0.75, ch.r * scale);
      ctx.fillStyle = baseColor;
      ctx.beginPath();
      let count = 0;
      for (let pi = 0; pi < ch.pieces.length; pi++) {
        const p = ch.pieces[pi];
        if (p.x1 < vx0 || p.x0 > vx1 || p.y1 < vy0 || p.y0 > vy1) continue;
        const sx = (pts[p.at] - cx) * scale + halfW;
        const sy = (pts[p.at + 1] - cy) * scale + halfH;
        ctx.moveTo(sx + rad, sy);
        ctx.arc(sx, sy, rad, 0, Math.PI * 2);
        count++;
      }
      if (count > 0) ctx.fill();
    } else {
      // Strokes
      ctx.strokeStyle = baseColor;
      ctx.lineWidth = Math.max(0.75, ch.r * 2 * scale);
      ctx.beginPath();
      let count = 0;
      for (let pi = 0; pi < ch.pieces.length; pi++) {
        const p = ch.pieces[pi];
        if (p.x1 < vx0 || p.x0 > vx1 || p.y1 < vy0 || p.y0 > vy1) continue;
        const x0 = (pts[p.at] - cx) * scale + halfW;
        const y0 = (pts[p.at + 1] - cy) * scale + halfH;
        if (p.n === 1) {
          ctx.moveTo(x0, y0);
          ctx.lineTo(x0 + 0.01, y0);
        } else {
          ctx.moveTo(x0, y0);
          for (let k = 1; k < p.n; k++) {
            ctx.lineTo(
              (pts[p.at + k * 2] - cx) * scale + halfW,
              (pts[p.at + k * 2 + 1] - cy) * scale + halfH
            );
          }
        }
        count++;
      }
      if (count > 0) ctx.stroke();
    }

    // Highlighted pieces overlay
    if (hasHl) {
      for (let pi = 0; pi < ch.pieces.length; pi++) {
        const p = ch.pieces[pi];
        if (!this.highlights.has(`${chunkIdx}:${p.info}`)) continue;
        ctx.strokeStyle = hlColor;
        ctx.fillStyle = hlColor;
        ctx.lineWidth = Math.max(2, ch.r * 2 * scale + 2);
        ctx.beginPath();
        const x0 = (pts[p.at] - cx) * scale + halfW;
        const y0 = (pts[p.at + 1] - cy) * scale + halfH;
        if (p.kind === 2) {
          ctx.arc(x0, y0, Math.max(3, ch.r * scale + 2), 0, Math.PI * 2);
          ctx.fill();
        } else {
          ctx.moveTo(x0, y0);
          for (let k = 1; k < p.n; k++) {
            ctx.lineTo(
              (pts[p.at + k * 2] - cx) * scale + halfW,
              (pts[p.at + k * 2 + 1] - cy) * scale + halfH
            );
          }
          if (p.kind === 0) {
            ctx.closePath();
            ctx.stroke();
          } else {
            ctx.stroke();
          }
        }
      }
    }
  }

  private hitTest(sx: number, sy: number): PickHit | null {
    const frame = this.frame;
    if (!frame) return null;
    const rect = this.canvas.getBoundingClientRect();
    const wx = (sx - rect.width * 0.5) / this.scale + this.cx;
    const wy = (sy - rect.height * 0.5) / this.scale + this.cy;
    const tol = 6 / this.scale;

    let best: PickHit | null = null;
    let bestArea = Infinity;
    const { off } = this.masks;

    for (let ci = frame.chunks.length - 1; ci >= 0; ci--) {
      const ch = frame.chunks[ci];
      if (ch.layer < 32 && ((off >>> ch.layer) & 1) === 1) continue;
      for (let pi = 0; pi < ch.pieces.length; pi++) {
        const p = ch.pieces[pi];
        if (wx < p.x0 - tol || wx > p.x1 + tol || wy < p.y0 - tol || wy > p.y1 + tol) continue;
        const area = (p.x1 - p.x0 + 1) * (p.y1 - p.y0 + 1);
        if (area < bestArea) {
          bestArea = area;
          best = { at: ci, idx: p.info };
        }
      }
    }
    return best;
  }

  private onPointerDown = (ev: PointerEvent): void => {
    this.canvas.setPointerCapture(ev.pointerId);
    this.pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY });

    if (this.pointers.size === 1) {
      this.dragStart = { x: ev.clientX, y: ev.clientY, cx: this.cx, cy: this.cy, moved: false };
      this.pinchStart = null;
    } else if (this.pointers.size === 2) {
      const [a, b] = Array.from(this.pointers.values());
      this.pinchStart = {
        dist: Math.hypot(b.x - a.x, b.y - a.y) || 1,
        midX: (a.x + b.x) * 0.5,
        midY: (a.y + b.y) * 0.5,
        scale: this.scale,
        cx: this.cx,
        cy: this.cy,
      };
      this.dragStart = null;
    }
  };

  private onPointerMove = (ev: PointerEvent): void => {
    if (!this.pointers.has(ev.pointerId)) return;
    this.pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY });

    if (this.pointers.size === 2 && this.pinchStart) {
      const [a, b] = Array.from(this.pointers.values());
      const dist = Math.hypot(b.x - a.x, b.y - a.y) || 1;
      const midX = (a.x + b.x) * 0.5;
      const midY = (a.y + b.y) * 0.5;
      const factor = dist / this.pinchStart.dist;
      const nextScale = Math.max(1e-7, Math.min(2, this.pinchStart.scale * factor));
      const rect = this.canvas.getBoundingClientRect();
      const ox = this.pinchStart.midX - rect.left - rect.width * 0.5;
      const oy = this.pinchStart.midY - rect.top - rect.height * 0.5;
      const nx = midX - rect.left - rect.width * 0.5;
      const ny = midY - rect.top - rect.height * 0.5;
      this.scale = nextScale;
      this.cx = this.pinchStart.cx + ox / this.pinchStart.scale - nx / nextScale;
      this.cy = this.pinchStart.cy + oy / this.pinchStart.scale - ny / nextScale;
      this.userMoved = true;
      this.cbs.onUserMove?.();
      this.scheduleDraw();
      return;
    }

    if (this.dragStart && this.pointers.size === 1) {
      const dx = ev.clientX - this.dragStart.x;
      const dy = ev.clientY - this.dragStart.y;
      if (Math.hypot(dx, dy) > 3) this.dragStart.moved = true;
      if (this.dragStart.moved) {
        this.cx = this.dragStart.cx - dx / this.scale;
        this.cy = this.dragStart.cy - dy / this.scale;
        this.userMoved = true;
        this.cbs.onUserMove?.();
        this.scheduleDraw();
      }
    }
  };

  private onPointerUp = (ev: PointerEvent): void => {
    const wasClick = this.dragStart && !this.dragStart.moved && this.pointers.size === 1;
    this.pointers.delete(ev.pointerId);
    if (this.pointers.size < 2) this.pinchStart = null;
    if (this.pointers.size === 0) this.dragStart = null;

    if (wasClick && this.cbs.onPick) {
      const rect = this.canvas.getBoundingClientRect();
      const hit = this.hitTest(ev.clientX - rect.left, ev.clientY - rect.top);
      this.cbs.onPick(hit);
    }
  };

  private onWheel = (ev: WheelEvent): void => {
    ev.preventDefault();
    const rect = this.canvas.getBoundingClientRect();
    const sx = ev.clientX - rect.left - rect.width * 0.5;
    const sy = ev.clientY - rect.top - rect.height * 0.5;
    const factor = ev.deltaY < 0 ? 1.16 : 1 / 1.16;
    const nextScale = Math.max(1e-7, Math.min(2, this.scale * factor));
    this.cx += sx / this.scale - sx / nextScale;
    this.cy += sy / this.scale - sy / nextScale;
    this.scale = nextScale;
    this.userMoved = true;
    this.cbs.onUserMove?.();
    this.scheduleDraw();
  };

  private onDblClick = (): void => {
    this.fit();
  };

  destroy(): void {
    if (this.rafId) cancelAnimationFrame(this.rafId);
    this.resizeObs.disconnect();
    this.canvas.removeEventListener("pointerdown", this.onPointerDown);
    this.canvas.removeEventListener("pointermove", this.onPointerMove);
    this.canvas.removeEventListener("pointerup", this.onPointerUp);
    this.canvas.removeEventListener("pointercancel", this.onPointerUp);
    this.canvas.removeEventListener("wheel", this.onWheel);
    this.canvas.removeEventListener("dblclick", this.onDblClick);
    this.canvas.remove();
  }
}
