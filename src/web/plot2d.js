// A board or schematic on the web page.
//
// The hub sends them as it does to the phones (core/plot.bend): a "plot"
// lists chunks in paint order, each either the index of one this page
// already holds (the last plot applied) or the chunk itself: a layer's
// colour and opacity, and its pieces as capsules (tracks, pads, lines,
// round ends) or polygons (fills), in micrometres. Bend decides which
// source is on screen and asks for it (client.bend's Solid.want, the
// canvas's data-key); this file only decodes and draws, panned and zoomed
// by the pointer. A chunk sent whole fades in over the canvas's data-fade
// ms (the phones do the same), so moving between versions of a file
// (data-fam, core/hist.bend's family: the view stays put) fades in only
// what changed; otherwise a frame is drawn only when something changed.

import { wheelKind } from "./solid.js";

// varints (7 bits a byte, low first) and zigzag, as the hub writes them
function reader(bytes) {
  let i = 0;
  const next = () => {
    let acc = 0, sh = 0;
    while (i < bytes.length) {
      const v = bytes[i++];
      acc += (v & 127) * 2 ** sh;
      if (v < 128) break;
      sh += 7;
    }
    return acc;
  };
  return { more: () => i < bytes.length, next, signed: () => { const z = next(); return z % 2 ? -(z + 1) / 2 : z / 2; } };
}

// one chunk as two paths in board coordinates: its fills and its strokes
// (capsules of the chunk's width; a dot is a capsule of no length)
function chunk(o) {
  const mode = o.m ?? 1;
  const r = (o.w ?? 0) / 2;
  const v = reader(o.d ?? new Uint8Array(0));
  const fill = new Path2D(), line = new Path2D(), dots = new Path2D();
  let px = 0, py = 0;
  while (v.more()) {
    v.next(); // the piece's info index (what a tap would ask about)
    if (mode === 2) {
      px += v.signed(); py += v.signed();
      dots.moveTo(px + r, py);
      dots.arc(px, py, r, 0, Math.PI * 2);
      continue;
    }
    const n = v.next();
    const pts = [];
    for (let k = 0; k < n && v.more(); k += 1) { px += v.signed(); py += v.signed(); pts.push(px, py); }
    if (pts.length < 2) continue;
    const p = mode === 0 ? fill : line;
    p.moveTo(pts[0], pts[1]);
    for (let k = 2; k < pts.length; k += 2) p.lineTo(pts[k], pts[k + 1]);
    if (mode === 0) p.closePath();
    else if (pts.length === 2) { dots.moveTo(pts[0] + r, pts[1]); dots.arc(pts[0], pts[1], r, 0, Math.PI * 2); }
  }
  const c = o.c ?? 0;
  const color = `rgba(${(c >> 16) & 255},${(c >> 8) & 255},${c & 255},${(o.a ?? 255) / 255})`;
  return { fill, line, dots, r, color, layer: o.l ?? 0, born: performance.now() };
}

// the newest plot of each source (or why there is none), by its key; a
// plot's kept chunks index the one its "base" names (an older hub names
// none: the last plot applied, whichever source)
const plots = new Map();
let lastKey = "";

// a new connection holds nothing
export function reset() {
  plots.clear();
  lastKey = "";
}

// how each plot came: its size on the wire and how long after it was asked for
const stats = new Map();
const mb = (n) => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

export function got(o, at = { bytes: 0, t: performance.now() }) {
  const key = typeof o.key === "string" ? o.key : "";
  for (const v of views.values()) if (v.key === key && v.asked && !stats.has(key)) stats.set(key, { bytes: at.bytes, waited: (at.t - v.asked) / 1000 });
  if (typeof o.none === "string") {
    plots.set(key, { none: o.none });
  } else {
    const held = plots.get(typeof o.base === "string" ? o.base : lastKey)?.chunks ?? [];
    const refs = Array.isArray(o.cs) ? o.cs : [];
    const all = [];
    for (const r of refs) {
      const c = typeof r === "number" ? held[r] : r && typeof r === "object" ? chunk(r) : null;
      if (c) all.push(c);
    }
    const box = Array.isArray(o.box) && o.box.length === 4 ? o.box : [0, 0, 1, 1];
    plots.set(key, { chunks: all, box, fam: typeof o.fam === "string" ? o.fam : key });
  }
  lastKey = key;
  for (const v of views.values()) if (v.key === key) { v.show(plots.get(key)); present(v); }
}

class View {
  constructor(canvas, bg) {
    this.canvas = canvas;
    this.bg = bg;
    this.key = "";
    this.plot = null;
    this.off = 0;
    this.dim = 0;
    this.lift = 255;
    this.scale = 1; this.ox = 0; this.oy = 0;
    this.fitted = false;
    this.fade = 220;
    this.fam = "";
    this.drag = null;
    this.tap = null;
    this.sheets = [];
    this.sheetText = "";
    this.back = null;
    // one finger (or the mouse) pans; two pinch to zoom about their middle
    // and move the view with it
    this.pts = new Map();
    const mid = () => {
      const [a, b] = [...this.pts.values()];
      const r = canvas.getBoundingClientRect();
      return { x: (a.x + b.x) / 2 - r.left, y: (a.y + b.y) / 2 - r.top, d: Math.hypot(a.x - b.x, a.y - b.y) };
    };
    canvas.addEventListener("pointerdown", (e) => {
      try { canvas.setPointerCapture(e.pointerId); } catch {}
      this.pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
      // a press that stays put is a tap (a sheet symbol opens), unless a
      // second finger joins it
      this.tap = this.pts.size === 1 && e.button === 0 ? { x: e.clientX, y: e.clientY } : null;
      this.drag = this.pts.size === 1 ? { x: e.clientX, y: e.clientY } : null;
      this.pinch = this.pts.size === 2 ? mid() : null;
      canvas.classList.add("dragging");
    });
    canvas.addEventListener("pointermove", (e) => {
      if (!this.pts.has(e.pointerId)) { this.hover(e);return; }
      this.pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (this.tap && Math.hypot(e.clientX - this.tap.x, e.clientY - this.tap.y) > 5) this.tap = null;
      if (this.pinch && this.pts.size === 2) {
        const m = mid(), f = m.d / Math.max(this.pinch.d, 1);
        this.ox = m.x - (this.pinch.x - this.ox) * f; this.oy = m.y - (this.pinch.y - this.oy) * f;
        this.scale *= f;
        this.pinch = m;
      } else if (this.drag) {
        this.ox += e.clientX - this.drag.x; this.oy += e.clientY - this.drag.y;
        this.drag.x = e.clientX; this.drag.y = e.clientY;
      } else return;
      this.touched = true;
      this.later();
    });
    const up = (e) => {
      const tap = e.type === "pointerup" && this.tap && this.pts.size === 1 ? this.tap : null;
      this.tap = null;
      this.pts.delete(e.pointerId);
      if (tap) { const s = this.sheetAt(e.clientX, e.clientY);if (s && sheeted) sheeted(s.value); }
      this.pinch = null;
      // the finger left behind carries on panning from where it is
      const [rest] = [...this.pts.values()];
      this.drag = rest ? { x: rest.x, y: rest.y } : null;
      if (!rest) canvas.classList.remove("dragging");
    };
    canvas.addEventListener("pointerup", up);
    canvas.addEventListener("pointercancel", up);
    canvas.addEventListener("wheel", (e) => {
      e.preventDefault();
      const b = canvas.getBoundingClientRect();
      const mx = e.clientX - b.left, my = e.clientY - b.top;
      // a touchpad's two fingers move the plot; a pinch or a wheel zooms at the pointer
      const kind = wheelKind(e, this.wheelLast);
      this.wheelLast = { kind, t: e.timeStamp };
      const px = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? canvas.clientHeight : 1;
      if (kind === "pad") { this.ox -= e.deltaX * px; this.oy -= e.deltaY * px; }
      else {
        const f = Math.exp(-e.deltaY * px * (kind === "pinch" ? 0.01 : 0.0015));
        this.ox = mx - (mx - this.ox) * f; this.oy = my - (my - this.oy) * f;
        this.scale *= f;
      }
      this.touched = true;
      this.later();
    }, { passive: false });
    canvas.addEventListener("dblclick", () => this.fit());
    // a right click asks to comment there: the place in mm (the plot's own
    // coordinates); the page decides what to do (host.js)
    canvas.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      const b = canvas.getBoundingClientRect();
      // the plot's units are thousandths of a mm
      const x = (e.clientX - b.left - this.ox) / this.scale / 1000, y = (e.clientY - b.top - this.oy) / this.scale / 1000;
      if (placed && Number.isFinite(x) && Number.isFinite(y)) placed(`${x.toFixed(4)}\t${y.toFixed(4)}\t\t`);
    });
    // a new size refits the view, unless the user has moved it
    new ResizeObserver(() => { if (!this.fitted || !this.touched) this.fit(); else this.later(); }).observe(canvas);
  }

  // the sheet symbol (client.bend's Sch.boxes, the canvas's data-sheets)
  // under a point on the page; the smallest when boxes nest
  sheetAt(cx, cy) {
    if (!this.sheets.length || !this.plot || !this.plot.chunks) return null;
    const b = this.canvas.getBoundingClientRect();
    const x = (cx - b.left - this.ox) / this.scale / 1000, y = (cy - b.top - this.oy) / this.scale / 1000;
    let best = null;
    for (const s of this.sheets) {
      if (x < s.x0 || x > s.x1 || y < s.y0 || y > s.y1) continue;
      if (!best || (s.x1 - s.x0) * (s.y1 - s.y0) < (best.x1 - best.x0) * (best.y1 - best.y0)) best = s;
    }
    return best;
  }

  hover(e) {
    const on = !!this.sheetAt(e.clientX, e.clientY);
    if (on !== this.canvas.classList.contains("on-sheet")) this.canvas.classList.toggle("on-sheet", on);
  }

  // the whole plot in view, centred, with a margin
  fit() {
    const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
    if (!this.plot || !this.plot.box || w === 0 || h === 0) { this.later(); return; }
    const [x0, y0, x1, y1] = this.plot.box;
    this.scale = 0.9 * Math.min(w / Math.max(x1 - x0, 1), h / Math.max(y1 - y0, 1));
    this.ox = w / 2 - (x0 + x1) / 2 * this.scale;
    this.oy = h / 2 - (y0 + y1) / 2 * this.scale;
    this.fitted = true;
    this.touched = false;
    this.later();
  }

  show(p) {
    const first = !this.plot || !this.plot.box;
    this.plot = p;
    // a sheet seen before comes back as it was left
    if (first && this.back && p && p.box) { this.scale = this.back.scale;this.ox = this.back.ox;this.oy = this.back.oy;this.fitted = true;this.touched = true;this.back = null;this.later(); }
    else if (first) this.fit(); else this.later();
  }

  later() {
    if (this.queued) return;
    this.queued = true;
    requestAnimationFrame(() => { this.queued = false; this.draw(); });
  }

  // review comments' pins and the note form sit on the stage at their place
  // in mm (data-x, data-y), moved with the view
  pins() {
    const stage = this.canvas.parentElement;
    if (!stage) return;
    const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
    for (const el of stage.querySelectorAll("[data-x][data-y]")) {
      const sx = this.ox + Number(el.dataset.x) * 1000 * this.scale, sy = this.oy + Number(el.dataset.y) * 1000 * this.scale;
      // a card sits beside its place, kept inside the stage
      const card = el.classList.contains("rc-card");
      el.style.left = `${card ? Math.max(8, Math.min(sx + 16, w - el.offsetWidth - 8)) : sx}px`;
      el.style.top = `${card ? Math.max(8, Math.min(sy + 16, h - el.offsetHeight - 8)) : sy}px`;
      el.style.visibility = card || (sx >= 0 && sy >= 0 && sx <= w && sy <= h) ? "visible" : "hidden";
    }
  }

  draw() {
    this.pins();
    const c = this.canvas, dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(c.clientWidth * dpr)), h = Math.max(1, Math.round(c.clientHeight * dpr));
    if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
    const g = c.getContext("2d");
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.fillStyle = this.bg;
    g.fillRect(0, 0, w, h);
    const p = this.plot;
    if (!p || !p.chunks) {
      g.fillStyle = "#8a8f98";
      g.font = `${13 * dpr}px system-ui, sans-serif`;
      g.textAlign = "center";
      // while the hub reads the file, the seconds count (a large board is a
      // few seconds the first time)
      const s = this.asked ? Math.floor((performance.now() - this.asked) / 1000) : 0;
      g.fillText(p && p.none ? p.none : `Reading the file on the hub · ${s} s`, w / 2, h / 2);
      if (!(p && p.none) && !this.tick) this.tick = setInterval(() => { if (this.plot && (this.plot.chunks || this.plot.none)) { clearInterval(this.tick); this.tick = null; } this.later(); }, 1000);
      return;
    }
    g.setTransform(this.scale * dpr, 0, 0, this.scale * dpr, this.ox * dpr, this.oy * dpr);
    g.lineCap = "round";
    g.lineJoin = "round";
    const now = performance.now();
    let fading = false;
    // the active layer of a focus (data-lift) after the rest, on top
    const lift = this.lift;
    const order = lift >= 0 && lift < 32 ? [...p.chunks.filter((k) => k.layer !== lift), ...p.chunks.filter((k) => k.layer === lift)] : p.chunks;
    for (const k of order) {
      if (Math.floor(this.off / 2 ** k.layer) % 2) continue; // a layer turned off
      const a = this.fade > 0 ? Math.min(1, (now - (k.born ?? 0)) / this.fade) : 1;
      if (a < 1) fading = true;
      // a layer the focus dims (core/layers.bend's Lyr.masks) at a quarter
      g.globalAlpha = Math.floor(this.dim / 2 ** k.layer) % 2 ? a / 4 : a;
      g.fillStyle = k.color;
      g.strokeStyle = k.color;
      g.fill(k.fill);
      g.fill(k.dots);
      // a line never thinner than a pixel, however far out the view is
      g.lineWidth = Math.max(k.r * 2, 1 / this.scale);
      g.stroke(k.line);
    }
    g.globalAlpha = 1;
    // only while something fades in does it ask for the next frame
    if (fading) this.later();
    // how it came, small in a corner
    const st = stats.get(this.key);
    if (st) {
      g.setTransform(1, 0, 0, 1, 0, 0);
      g.fillStyle = "rgba(138,143,152,0.9)";
      g.font = `${11 * dpr}px system-ui, sans-serif`;
      g.textAlign = "left";
      g.fillText(`${mb(st.bytes)}` + (st.waited > 0.5 ? ` · came after ${st.waited.toFixed(1)} s` : ""), 8 * dpr, h - 8 * dpr);
    }
  }
}

// the views on the page, by canvas (the combined view shows two plots)
const views = new Map();
let placed = null;
let sheeted = null;
// where the view was left on each source moved by hand (a few dozen kept)
const left = new Map();

// the page's answer to a tap on a sheet symbol (host.js)
export function onSheet(fn) {
  sheeted = fn;
}

// "<row>\t<place>\t<uuid path>\t<x0 y0 x1 y1>" lines, mm on the sheet on
// show; the value sent back is the instance (all but the box)
function sheetBoxes(text) {
  const out = [];
  for (const line of String(text || "").split("\n")) {
    const f = line.split("\t");
    if (f.length < 4) continue;
    const n = f[3].trim().split(/\s+/).map(Number);
    if (n.length !== 4 || !n.every(Number.isFinite)) continue;
    out.push({ value: `${f[0]}\t${f[1]}\t${f[2]}`, x0: Math.min(n[0], n[2]), y0: Math.min(n[1], n[3]), x1: Math.max(n[0], n[2]), y1: Math.max(n[1], n[3]) });
  }
  return out;
}
let noteSeen = false;

// the page's answer to a right click (host.js)
export function onPlace(fn) {
  placed = fn;
}

// the Layers menu lists only the layers the plot has something on
function present(view) {
  const p = view.plot;
  const has = new Set(p && p.chunks ? p.chunks.map((c) => c.layer) : []);
  for (const b of document.querySelectorAll(".layer-list [data-layer]")) b.hidden = has.size > 0 && !has.has(Number(b.dataset.layer));
}

// where a view was left on its source, kept for when it comes back
function park(view) {
  if (view.key && view.plot && view.plot.chunks && view.touched) {
    left.delete(view.key);left.set(view.key, { scale: view.scale, ox: view.ox, oy: view.oy });
    if (left.size > 48) left.delete(left.keys().next().value);
  }
}

function mountOne(canvas) {
  let view = views.get(canvas);
  if (!view) views.set(canvas, (view = new View(canvas, canvas.dataset.bg || "#0d0f12")));
  const off = Number(canvas.dataset.off || 0), dim = Number(canvas.dataset.dim || 0), lift = Number(canvas.dataset.lift ?? 255);
  if (off !== view.off || dim !== view.dim || lift !== view.lift) { view.off = off; view.dim = dim; view.lift = lift; view.later(); }
  present(view);
  view.pins();
  view.fade = Number(canvas.dataset.fade || 0);
  if (view.sheetText !== (canvas.dataset.sheets || "")) { view.sheetText = canvas.dataset.sheets || "";view.sheets = sheetBoxes(view.sheetText); }
  const key = canvas.dataset.key || "";
  if (key !== view.key) {
    park(view);
    // another version of the same file keeps its view, and the last one
    // stays up until the next comes
    const same = view.plot && view.plot.chunks && canvas.dataset.fam && canvas.dataset.fam === view.fam;
    view.key = key;
    view.fam = canvas.dataset.fam || "";
    view.asked = performance.now();
    if (!same) { view.plot = null; view.fitted = false; view.back = left.get(key) || null; }
    if (plots.has(key)) view.show(plots.get(key)); else view.later();
  }
}

// after each render: each canvas on the page (the board's or schematic's
// tab, or the combined view's two) shows the source its data-key names,
// once the hub has sent it
export function mount(canvases) {
  const list = canvases.filter(Boolean);
  for (const [c, v] of [...views]) if (!list.includes(c)) { park(v); views.delete(c); }
  for (const c of list) mountOne(c);
  // a note form that has just come up takes the keyboard
  const note = document.querySelector(".rc-note input");
  if (note && !noteSeen) note.focus();
  noteSeen = !!note;
}

export function fit() {
  for (const v of views.values()) v.fit();
}
