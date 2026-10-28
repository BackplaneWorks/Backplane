// A board or schematic on the web page.
//
// The hub sends them as it does to the phones (core/plot.bend): a "plot"
// lists chunks in paint order, each either the index of one the page holds
// (of the plot its "base" names: the last of that source, or another's a
// new view started from) or the chunk itself: a layer's colour and opacity,
// and its pieces as capsules (tracks, pads, lines, round ends) or polygons
// (fills), in micrometres. Bend decides which source is on screen and asks
// for it (client.bend's Solid.want, the canvas's data-key); this file only
// decodes and draws, panned and zoomed by the pointer. A chunk sent whole
// fades in over the canvas's data-fade ms (the phones do the same), so
// moving between versions of a file (data-fam, core/hist.bend's family:
// the view stays put) fades in only what changed; otherwise a frame is
// drawn only when something changed. Several canvases may show plots at
// once (the combined view: the schematic beside the board).
//
// Cross-probing (probe.js decides): each piece names its item's info, which
// the hub keeps in a file beside the plot (GET /plot/info, by the chunks'
// sigs). A click picks the piece under it and the page tells the hub what
// it probes (the canvas's data-probe comes back); the probed part or net is
// drawn over the rest, which dims.

import App from "./app.bend";
import { wheelKind } from "./solid.js";
import * as P from "./probe.js";

// the page's own font (style.css's), for what is written on the canvas
let family = "";
const font = (el, px) => `${px}px ${family ||= getComputedStyle(el).fontFamily || "monospace"}`;

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

// each piece's info index and points, in board coordinates
function decode(d, mode) {
  const v = reader(d ?? new Uint8Array(0));
  const out = [];
  let px = 0, py = 0;
  while (v.more()) {
    const i = v.next();
    if (mode === 2) { px += v.signed(); py += v.signed(); out.push({ i, pts: [px, py] }); continue; }
    const n = v.next();
    const pts = [];
    for (let k = 0; k < n && v.more(); k += 1) { px += v.signed(); py += v.signed(); pts.push(px, py); }
    out.push({ i, pts });
  }
  return out;
}

// some pieces as paths: fills, strokes, and dots (a stroke of one point)
function paths(ps, mode, r) {
  const fill = new Path2D(), line = new Path2D(), dots = new Path2D();
  for (const { pts } of ps) {
    if (mode === 2) { dots.moveTo(pts[0] + r, pts[1]); dots.arc(pts[0], pts[1], r, 0, Math.PI * 2); continue; }
    if (pts.length < 2) continue;
    const p = mode === 0 ? fill : line;
    p.moveTo(pts[0], pts[1]);
    for (let k = 2; k < pts.length; k += 2) p.lineTo(pts[k], pts[k + 1]);
    if (mode === 0) p.closePath();
    else if (pts.length === 2) { dots.moveTo(pts[0] + r, pts[1]); dots.arc(pts[0], pts[1], r, 0, Math.PI * 2); }
  }
  return { fill, line, dots };
}

// one chunk: its paths, and what picking and lighting need (its bytes,
// decoded on first use, and its sig, which names its infos)
function chunk(o) {
  const mode = o.m ?? 1;
  const r = (o.w ?? 0) / 2;
  const d = o.d ?? new Uint8Array(0);
  const c = o.c ?? 0;
  const color = `rgba(${(c >> 16) & 255},${(c >> 8) & 255},${c & 255},${(o.a ?? 255) / 255})`;
  return { ...paths(decode(d, mode), mode, r), r, mode, d, sig: typeof o.s === "number" ? o.s >>> 0 : null, color, layer: o.l ?? 0, born: performance.now() };
}

function pieces(k) {
  if (k.ps) return k.ps;
  const ps = decode(k.d, k.mode);
  for (const p of ps) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (let j = 0; j < p.pts.length; j += 2) { x0 = Math.min(x0, p.pts[j]); x1 = Math.max(x1, p.pts[j]); y0 = Math.min(y0, p.pts[j + 1]); y1 = Math.max(y1, p.pts[j + 1]); }
    p.box = [x0 - k.r, y0 - k.r, x1 + k.r, y1 + k.r];
  }
  k.ps = ps;
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const p of ps) { x0 = Math.min(x0, p.box[0]); y0 = Math.min(y0, p.box[1]); x1 = Math.max(x1, p.box[2]); y1 = Math.max(y1, p.box[3]); }
  k.box = [x0, y0, x1, y1];
  return ps;
}

// the newest plot of each source (or why there is none), by its key
const plots = new Map();
// the source a plot without a "base" (an older hub's) is a delta against
let lastKey = "";

// a new connection holds nothing
export function reset() {
  plots.clear();
  lastKey = "";
  infos.clear();
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
    const base = typeof o.base === "string" ? o.base : lastKey;
    const held = plots.get(base)?.chunks ?? [];
    const refs = Array.isArray(o.cs) ? o.cs : [];
    const all = [];
    for (const r of refs) {
      const c = typeof r === "number" ? held[r] : r && typeof r === "object" ? chunk(r) : null;
      if (c) all.push(c);
    }
    const box = Array.isArray(o.box) && o.box.length === 4 ? o.box : [0, 0, 1, 1];
    plots.set(key, { chunks: all, box, fam: typeof o.fam === "string" ? o.fam : key, n: (plots.get(key)?.n ?? 0) + 1, thick: typeof o.thick === "number" ? o.thick : 1600 });
  }
  lastKey = key;
  for (const v of views.values()) if (v.key === key) { v.show(plots.get(key)); present(v); prefetch(v); }
  changed(key);
}

// a plot or its infos came: every view lights again, and the page hears it
// (a 3D view finds its parts on the board's plot)
function changed(key) {
  for (const v of views.values()) v.relight();
  for (const f of onPlot) f(key);
  drawnCheck();
}

// Waiting for what is on show to be drawn (the history playing: a step is
// seen for its beat once drawn, host.js's Later "after" "drawn"). A wait
// starts at the next mount, the render of the view it came with, and ends
// once every canvas has its plot (or the hub's word that there is none),
// or after DRAWN_MS whatever comes
const DRAWN_MS = 30000;
const drawnWaits = [];
export function whenDrawn(f) {
  drawnWaits.push({ f, mounted: false, at: performance.now() });
  setTimeout(drawnCheck, DRAWN_MS + 10);
}
function drawnCheck() {
  if (!drawnWaits.length) return;
  const ready = [...views.values()].every((v) => !v.key || plots.has(v.key)), now = performance.now();
  for (const w of drawnWaits.splice(0)) {
    if ((w.mounted && ready) || now - w.at >= DRAWN_MS) w.f();
    else drawnWaits.push(w);
  }
}

// Infos
// -----
// Each source's info file, fetched when a probe or a pick needs it, kept
// by sig (a chunk's sig covers its infos, so a table once read stays true
// for every chunk of that sig).

const tables = new Map();       // sig -> infos
const infos = new Map();        // key -> { at: plot count fetched for, wait: Promise }
const onPlot = new Set();

// a source's name on the hub's disk: the FNV-1a of its key, as
// core/plot.bend's Key.hash makes it (a char at a time)
export function keyHash(key) {
  let h = 2166136261;
  for (const ch of key) h = Math.imul(h ^ ch.codePointAt(0), 16777619) >>> 0;
  return h;
}

async function fetchInfos(key) {
  const r = await fetch(`/plot/info?h=${keyHash(key)}`, { credentials: "same-origin", cache: "no-store" });
  if (!r.ok) return new Map();
  return P.parseInfos(await r.text());
}

// a source's infos: once its plot's chunks all have theirs (one fetch per
// new version of the plot); a source never plotted on this page (the
// board behind a schematic) is read once
export function need(key) {
  const p = plots.get(key);
  const missing = p?.chunks ? p.chunks.some((c) => c.sig !== null && !tables.has(c.sig)) : !infos.has(key);
  const n = p?.n ?? 0;
  const had = infos.get(key);
  if (had && (!missing || had.at === n)) return had.wait;
  const wait = fetchInfos(key).then((m) => { for (const [s, t] of m) tables.set(s, t); return m; }).catch(() => new Map());
  infos.set(key, { at: n, wait, all: null });
  // (none found while some were missing, the hub not done writing them:
  // the next need asks again)
  wait.then((m) => { const e = infos.get(key); if (e && e.wait === wait) { if (!m.size && missing) infos.delete(key); else e.all = m; } changed(key); });
  return wait;
}

// A live plot on show has its infos read once it is drawn, while the page
// is idle (and a schematic's board's), so the first probe is as quick as
// the next; a version of the design history waits for a probe
function prefetch(v) {
  const key = v.key;
  if (!key || /\|(git\t|diff\n)/.test(key)) return;
  const idle = window.requestIdleCallback || ((f) => setTimeout(f, 200));
  idle(() => {
    if (v.key !== key || !plots.get(key)?.chunks) return;
    need(key);
    if (v.side === "schematic" && v.board) need(v.board);
  }, { timeout: 2000 });
}

// the infos of a chunk
const tableOf = (k) => (k.sig === null ? null : tables.get(k.sig) || null);

// the board's nets behind a source: its own plot's tables, or the info
// file the hub keeps for the board (a schematic names its board)
function boardNets(key) {
  const p = plots.get(key);
  if (p?.chunks) {
    const m = new Map();
    for (const k of p.chunks) { const t = tableOf(k); if (t) m.set(k.sig, t); }
    if (m.size) return P.boardNets(m);
  }
  const e = infos.get(key);
  return e?.all ? P.boardNets(e.all) : null;
}

// a plot's sheet groups (schematic.bend's Conn)
function groupsOf(p) {
  const m = new Map();
  for (const k of p.chunks) { const t = tableOf(k); if (t) m.set(k.sig, t); }
  return P.sheetGroups(m);
}

// Parts' boxes on a board: each ref's pads and drawing (texts left out:
// a reference may sit far from its part), and its side (more on the back's
// layers, 0..5, than the front's, 12..18)
export function parts(key) {
  const p = plots.get(key);
  if (!p?.chunks) return null;
  if (p.parts && p.partsAt === tables.size) return p.parts;
  const out = new Map();
  for (const k of p.chunks) {
    const t = tableOf(k);
    if (!t) continue;
    const refs = t.map((s) => { const w = P.what(s); return w.kind === "text" ? "" : w.ref; });
    if (!refs.some(Boolean)) continue;
    for (const pc of pieces(k)) {
      const ref = refs[pc.i];
      if (!ref) continue;
      let e = out.get(ref);
      if (!e) out.set(ref, (e = { box: [Infinity, Infinity, -Infinity, -Infinity], back: 0, front: 0 }));
      e.box[0] = Math.min(e.box[0], pc.box[0]); e.box[1] = Math.min(e.box[1], pc.box[1]);
      e.box[2] = Math.max(e.box[2], pc.box[2]); e.box[3] = Math.max(e.box[3], pc.box[3]);
      if (k.layer <= 5) e.back += 1; else if (k.layer >= 12 && k.layer <= 18 && k.layer !== 13) e.front += 1;
    }
  }
  p.parts = out;
  p.partsAt = tables.size;
  return out;
}

export function plotOf(key) {
  return plots.get(key) || null;
}

// a board's thickness in micrometres (the 3D view's sides)
export function thick(key) {
  return plots.get(key)?.thick ?? 1600;
}

export function onPlotted(f) {
  onPlot.add(f);
}

// Picking
// -------

function segDist2(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay, l2 = dx * dx + dy * dy;
  let t = l2 > 0 ? ((px - ax) * dx + (py - ay) * dy) / l2 : 0;
  t = Math.max(0, Math.min(1, t));
  const qx = ax + t * dx - px, qy = ay + t * dy - py;
  return qx * qx + qy * qy;
}

function inside(pts, x, y) {
  let c = false;
  for (let i = 0, j = pts.length - 2; i < pts.length; j = i, i += 2) {
    const yi = pts[i + 1], yj = pts[j + 1];
    if ((yi > y) !== (yj > y) && x < ((pts[j] - pts[i]) * (y - yi)) / (yj - yi) + pts[i]) c = !c;
  }
  return c;
}

function area(pts, mode, r) {
  if (mode === 2 || pts.length < 4) return Math.PI * r * r + 1;
  if (mode === 0) {
    let a = 0;
    for (let i = 0, j = pts.length - 2; i < pts.length; j = i, i += 2) a += (pts[j] + pts[i]) * (pts[j + 1] - pts[i + 1]);
    return Math.abs(a / 2) + 1;
  }
  let l = 0;
  for (let i = 2; i < pts.length; i += 2) l += Math.hypot(pts[i] - pts[i - 2], pts[i + 1] - pts[i - 1]);
  return l * Math.max(2 * r, 1) + Math.PI * r * r + 1;
}

function hits(p, k, x, y, tol) {
  const pts = p.pts, r = k.r;
  if (k.mode === 2) return Math.hypot(pts[0] - x, pts[1] - y) <= r + tol;
  if (k.mode === 0 && pts.length >= 6 && inside(pts, x, y)) return true;
  const w = (k.mode === 0 ? 0 : r) + tol, w2 = w * w;
  if (pts.length === 2) return (pts[0] - x) ** 2 + (pts[1] - y) ** 2 <= w2;
  for (let i = 2; i < pts.length; i += 2) if (segDist2(x, y, pts[i - 2], pts[i - 1], pts[i], pts[i + 1]) <= w2) return true;
  if (k.mode === 0 && pts.length >= 6 && segDist2(x, y, pts[pts.length - 2], pts[pts.length - 1], pts[0], pts[1]) <= w2) return true;
  return false;
}

// what lies under a point (board coordinates), on the layers shown
function under(plot, x, y, tol, off) {
  const out = [];
  for (const k of plot.chunks) {
    if (Math.floor(off / 2 ** k.layer) % 2) continue;
    const t = tableOf(k);
    if (!t) continue;
    pieces(k);
    const b = k.box;
    if (x < b[0] - tol || x > b[2] + tol || y < b[1] - tol || y > b[3] + tol) continue;
    for (const p of k.ps) {
      const pb = p.box;
      if (x < pb[0] - tol || x > pb[2] + tol || y < pb[1] - tol || y > pb[3] + tol) continue;
      if (hits(p, k, x, y, tol)) out.push({ text: t[p.i] || "", area: area(p.pts, k.mode, k.r), layer: k.layer });
    }
  }
  return out;
}

// the smallest part's box holding a point
function partAt(key, x, y) {
  const ps = parts(key);
  if (!ps) return "";
  let best = "", a = Infinity;
  for (const [ref, e] of ps) {
    const b = e.box;
    if (x < b[0] || x > b[2] || y < b[1] || y > b[3]) continue;
    const s = (b[2] - b[0]) * (b[3] - b[1]);
    if (s < a) { a = s; best = ref; }
  }
  return best;
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
    this.probe = "";
    this.side = "board";
    this.place = "/";
    this.board = "";
    this.hl = null;
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
      // a press that stays put is a tap (a sheet symbol opens, anything
      // else is probed), unless a second finger joins it
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
      this.follow = false;
      this.later();
    });
    const up = (e) => {
      const tap = e.type === "pointerup" && this.tap && this.pts.size === 1 ? this.tap : null;
      this.tap = null;
      this.pts.delete(e.pointerId);
      if (tap) {
        const s = this.sheetAt(e.clientX, e.clientY);
        if (s && sheeted) sheeted(s.value);
        else this.pick(e.clientX, e.clientY);
      }
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
      this.follow = false;
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

  // a tap probes what lies under it (probe.js), once the plot's infos are
  // in; the page tells the hub, and lights it here at once
  async pick(cx, cy) {
    if (!probed || !this.plot || !this.plot.chunks) return;
    const key = this.key, b = this.canvas.getBoundingClientRect();
    const x = (cx - b.left - this.ox) / this.scale, y = (cy - b.top - this.oy) / this.scale;
    await need(key);
    if (this.side === "schematic" && this.board) await need(this.board);
    if (this.key !== key || !this.plot || !this.plot.chunks) return;
    const exact = under(this.plot, x, y, 0.75 / this.scale, this.off), near = under(this.plot, x, y, 4 / this.scale, this.off);
    const pick = this.side === "board" ? P.pickBoard(exact, near, partAt(key, x, y)) : P.pickSheet(exact, near);
    const ctx = this.side === "board" ? {} : { groups: groupsOf(this.plot), nets: this.board ? boardNets(this.board) : null, place: this.place };
    const v = P.clickProbe(pick, this.side, ctx);
    this.probe = v;
    for (const w of views.values()) if (w !== this) { w.probe = v; w.follow = true; w.relight(); }
    this.relight();
    probed(v);
  }

  // what the probe lights on this plot: the pieces of each chunk it holds,
  // as paths, and their box (for Fit to it)
  relight() {
    this.hl = null;
    this.later();
  }

  lit() {
    const p = this.plot;
    if (!this.probe || !p || !p.chunks) return null;
    const stamp = `${this.probe}|${tables.size}|${p.n}|${this.place}`;
    if (this.hl && this.hl.stamp === stamp) return this.hl;
    const pr = P.parseProbe(this.probe);
    if (!pr.kind) return null;
    // infos not yet here: ask, and light once they come
    need(this.key);
    if (this.side === "schematic" && this.board && pr.kind === "n") need(this.board);
    const ctx = this.side === "board" ? { nets: boardNets(this.key) } : { groups: groupsOf(p), nets: this.board ? boardNets(this.board) : null, place: this.place };
    const test = P.lit(pr, this.side, ctx);
    const out = [];
    const box = [Infinity, Infinity, -Infinity, -Infinity];
    for (const k of p.chunks) {
      const t = tableOf(k);
      if (!t) continue;
      const on = t.map(test);
      if (!on.some(Boolean)) continue;
      const ps = pieces(k).filter((pc) => on[pc.i]);
      if (!ps.length) continue;
      for (const pc of ps) { box[0] = Math.min(box[0], pc.box[0]); box[1] = Math.min(box[1], pc.box[1]); box[2] = Math.max(box[2], pc.box[2]); box[3] = Math.max(box[3], pc.box[3]); }
      // (the light edge goes round small fills, pads, not a net's pours)
      const small = k.mode === 0 ? ps.filter((pc) => (pc.box[2] - pc.box[0]) * (pc.box[3] - pc.box[1]) < EDGED) : [];
      out.push({ ...paths(ps, k.mode, k.r), edge: paths(small, 0, 0).fill, r: k.r, color: k.color, layer: k.layer });
    }
    this.hl = { stamp, parts: out, box: out.length ? box : null, kind: pr.kind };
    return this.hl;
  }

  // the probed part or net in view (the chip's name): its box fitted, at
  // least a few millimetres of what is round it
  show_probe() {
    const h = this.lit();
    if (!h || !h.box) return;
    const w = this.canvas.clientWidth, ht = this.canvas.clientHeight;
    const [x0, y0, x1, y1] = h.box;
    this.scale = 0.8 * Math.min(w / Math.max(x1 - x0, 6000), ht / Math.max(y1 - y0, 6000));
    this.ox = w / 2 - (x0 + x1) / 2 * this.scale;
    this.oy = ht / 2 - (y0 + y1) / 2 * this.scale;
    this.touched = true;
    this.follow = false;
    this.later();
  }

  // a probe made elsewhere is followed: what it lights, if out of view,
  // comes to the middle (fitted when it is bigger than the view)
  follow_probe(h) {
    if (!h.box) return;
    const w = this.canvas.clientWidth, ht = this.canvas.clientHeight;
    const [x0, y0, x1, y1] = h.box;
    const sx0 = this.ox + x0 * this.scale, sy0 = this.oy + y0 * this.scale, sx1 = this.ox + x1 * this.scale, sy1 = this.oy + y1 * this.scale;
    this.follow = false;
    if (sx0 >= 0 && sy0 >= 0 && sx1 <= w && sy1 <= ht) return;
    if (sx1 - sx0 > w || sy1 - sy0 > ht) { this.show_probe(); return; }
    this.ox = w / 2 - (x0 + x1) / 2 * this.scale;
    this.oy = ht / 2 - (y0 + y1) / 2 * this.scale;
    this.touched = true;
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
    this.hl = null;
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
      g.font = font(c, 13 * dpr);
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
    // a probe dims the rest, and draws what it lights over it
    let hl = this.lit();
    if (hl && this.follow && hl.box) { this.follow_probe(hl); g.setTransform(this.scale * dpr, 0, 0, this.scale * dpr, this.ox * dpr, this.oy * dpr); }
    const dimAll = hl && hl.parts.length ? DIM : 1;
    // the active layer of a focus (data-lift) after the rest, on top
    const lift = this.lift;
    const order = lift >= 0 && lift < 32 ? [...p.chunks.filter((k) => k.layer !== lift), ...p.chunks.filter((k) => k.layer === lift)] : p.chunks;
    const thin = 1 / this.scale;
    for (const k of order) {
      if (Math.floor(this.off / 2 ** k.layer) % 2) continue; // a layer turned off
      const a = this.fade > 0 ? Math.min(1, (now - (k.born ?? 0)) / this.fade) : 1;
      if (a < 1) fading = true;
      // a layer the focus dims (core/layers.bend's Lyr.masks) at a quarter
      g.globalAlpha = (Math.floor(this.dim / 2 ** k.layer) % 2 ? a / 4 : a) * dimAll;
      g.fillStyle = k.color;
      g.strokeStyle = k.color;
      g.fill(k.fill);
      g.fill(k.dots);
      // a line never thinner than a pixel, however far out the view is
      g.lineWidth = Math.max(k.r * 2, thin);
      g.stroke(k.line);
    }
    if (dimAll !== 1) {
      // what the probe lights: in its own colours, full strength (a line
      // at least a few pixels wide), with a light edge round its fills and
      // dots, so a part or net reads at any zoom
      g.globalAlpha = 1;
      for (const k of hl.parts) {
        if (Math.floor(this.off / 2 ** k.layer) % 2) continue;
        g.fillStyle = k.color;
        g.strokeStyle = k.color;
        g.fill(k.fill);
        g.fill(k.dots);
        g.lineWidth = Math.max(k.r * 2, 2.5 * thin);
        g.stroke(k.line);
      }
      g.globalAlpha = 0.9;
      g.strokeStyle = EDGE;
      g.lineWidth = 1.5 * thin;
      for (const k of hl.parts) {
        if (Math.floor(this.off / 2 ** k.layer) % 2) continue;
        g.stroke(k.edge);
        g.stroke(k.dots);
      }
    }
    g.globalAlpha = 1;
    // only while something fades in does it ask for the next frame
    if (fading) this.later();
    // how it came, small in a corner, on the page's background so a board
    // panned under it never shows through
    const st = stats.get(this.key);
    if (st) {
      g.setTransform(1, 0, 0, 1, 0, 0);
      g.font = font(this.canvas, 11 * dpr);
      g.textAlign = "left";
      const text = App.plot_stats(document.documentElement.lang, mb(st.bytes), st.waited.toFixed(1), st.waited > 0.5);
      g.fillStyle = this.bg;
      g.fillRect(4 * dpr, h - 20 * dpr, g.measureText(text).width + 8 * dpr, 16 * dpr);
      g.fillStyle = "rgba(138,143,152,0.9)";
      g.fillText(text, 8 * dpr, h - 8 * dpr);
    }
  }
}

// how far the rest dims under a probe, and the edge drawn round what it lights
const DIM = 0.28;
// fills under this many square micrometres (25 mm²) get the edge
const EDGED = 25e6;
const EDGE = "rgba(255,255,255,0.85)";

// the views on the page, by canvas
const views = new Map();
let placed = null;
let sheeted = null;
let probed = null;
// where the view was left on each source moved by hand (a few dozen kept)
const left = new Map();

// the page's answer to a tap on a sheet symbol (host.js)
export function onSheet(fn) {
  sheeted = fn;
}

// the page's answer to a tap that probes (host.js tells the hub)
export function onProbe(fn) {
  probed = fn;
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

// the Layers menu lists only the layers the plot has something on (the
// menu beside the canvas: its pane's, else the viewer's)
function present(v) {
  const p = v.plot;
  const has = new Set(p && p.chunks ? p.chunks.map((c) => c.layer) : []);
  const scope = v.canvas.closest(".combo-pane") || document;
  for (const b of scope.querySelectorAll(".layer-list [data-layer]")) b.hidden = has.size > 0 && !has.has(Number(b.dataset.layer));
}

// after each render: each canvas on the page shows the source its data-key
// names, once the hub has sent it
export function mount(canvases) {
  const list = (Array.isArray(canvases) ? canvases : [canvases]).filter(Boolean);
  for (const c of [...views.keys()]) if (!list.includes(c)) { const v = views.get(c); park(v); views.delete(c); }
  for (const canvas of list) mountOne(canvas);
  // a note form that has just come up takes the keyboard
  const note = document.querySelector(".rc-note input");
  if (note && !noteSeen) note.focus();
  noteSeen = !!note;
  for (const w of drawnWaits) w.mounted = true;
  drawnCheck();
}

function park(v) {
  if (v.key && v.plot && v.plot.chunks && v.touched) {
    left.delete(v.key);left.set(v.key, { scale: v.scale, ox: v.ox, oy: v.oy });
    if (left.size > 48) left.delete(left.keys().next().value);
  }
}

function mountOne(canvas) {
  let view = views.get(canvas);
  if (!view) views.set(canvas, (view = new View(canvas, canvas.dataset.bg || "#0d0f12")));
  const off = Number(canvas.dataset.off || 0), dim = Number(canvas.dataset.dim || 0), lift = Number(canvas.dataset.lift ?? 255);
  if (off !== view.off || dim !== view.dim || lift !== view.lift) { view.off = off; view.dim = dim; view.lift = lift; view.later(); }
  view.side = canvas.dataset.side || "board";
  const place = canvas.dataset.place || "/", board = canvas.dataset.board || "";
  if (place !== view.place || board !== view.board) { view.place = place; view.board = board; view.relight(); }
  // the probe the hub keeps (data-probe); a tap's own shows until it comes
  const probe = canvas.dataset.probe ?? "";
  if (probe !== view.hubProbe) { view.hubProbe = probe; if (probe !== view.probe) view.follow = true; view.probe = probe; view.relight(); }
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
    view.hl = null;
    if (!same) { view.plot = null; view.fitted = false; view.back = left.get(key) || null; }
    if (plots.has(key)) { view.show(plots.get(key)); prefetch(view); } else view.later();
  }
}

export function fit() {
  for (const v of views.values()) v.fit();
}

// a probe made elsewhere (the 3D view, Esc): every view lights it at once,
// before the hub's comes back
export function setProbe(v) {
  for (const w of views.values()) if (w.probe !== v) { w.probe = v; w.follow = true; w.relight(); }
}

// the probed part or net brought into view in each plot (the chip)
export function showProbe() {
  for (const v of views.values()) v.show_probe();
}
