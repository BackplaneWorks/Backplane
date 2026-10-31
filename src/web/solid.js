import App from "./app.bend";
import { DepthOfField } from "./solid-dof.js";
import { fittedDistance } from "./solid-framing.js";
import { SolidAssets } from "./solid-assets.js";
import { vertexShader as VS, fragmentShader as FS } from "./solid-materials.js";

// A part in 3D on the web page.
//
// The hub sends a part's model as it does to the phones: a CBOR "plot"
// with a mesh (core/plot.bend's Solid: triangles in board axes, z up, the
// hub's units; each the XOR of its colour with the one before, then
// nine zigzag coordinate deltas). Bend decides which part is on show and
// asks for it (client.bend's Solid.want, the canvas's data-key); this
// file only decodes the mesh and draws it with WebGL, turned by the
// pointer. Nothing animates: a frame is drawn when something changed.

// CBOR, as much as plots use (ints, byte and text strings, arrays, maps;
// keys 1 and 31 are the hub's dictionary words "t" and "key")
export function cbor(b) {
  let i = 0;
  const arg = (ai) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[i++];
    if (ai === 25) { i += 2; return (b[i - 2] << 8) | b[i - 1]; }
    if (ai === 26) { i += 4; return ((b[i - 4] << 24) >>> 0) + (b[i - 3] << 16) + (b[i - 2] << 8) + b[i - 1]; }
    return 0;
  };
  const item = () => {
    const h = b[i++];
    const n = arg(h & 31);
    switch (h >> 5) {
      case 0: return n;
      case 1: return -1 - n;
      case 2: i += n; return b.subarray(i - n, i);
      case 3: i += n; return new TextDecoder().decode(b.subarray(i - n, i));
      case 4: return Array.from({ length: n }, item);
      case 5: {
        const o = {};
        for (let k = 0; k < n; k += 1) {
          const key = item();
          o[key === 1 ? "t" : key === 31 ? "key" : String(key)] = item();
        }
        return o;
      }
      default: return null;
    }
  };
  return item();
}

// a frame the hub sent is a plot: a map whose first key is "t" (1) and
// value the text "plot"
export function isPlot(b) {
  return b.length > 7 && b[0] >= 0xa0 && b[0] <= 0xb7 && b[1] === 1 && b[2] === 0x64 &&
    b[3] === 0x70 && b[4] === 0x6c && b[5] === 0x6f && b[6] === 0x74;
}

// The wire uses board axes (x right, y down, z up). WebGL's orbit is
// right-handed: turn the wire into (x, -y, z) before normals and bounds.
// Applying this to every vertex keeps all board layers and parts registered.
export function mesh(o) {
  const bytes = o.mesh;
  const n = o.n | 0;
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
  const signed = () => { const z = next(); return z % 2 ? -(z + 1) / 2 : z / 2; };
  const pos = new Float32Array(n * 9), nrm = new Float32Array(n * 9), col = new Float32Array(n * 9);
  let color = 0, px = 0, py = 0, pz = 0, w = 0;
  const p = new Float32Array(9);
  for (let t = 0; t < n && i < bytes.length; t += 1) {
    color = (color ^ next()) >>> 0;
    for (let k = 0; k < 3; k += 1) {
      px += signed(); py += signed(); pz += signed();
      p[3 * k] = px; p[3 * k + 1] = -py; p[3 * k + 2] = pz;
    }
    const ux = p[3] - p[0], uy = p[4] - p[1], uz = p[5] - p[2];
    const vx = p[6] - p[0], vy = p[7] - p[1], vz = p[8] - p[2];
    let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
    const l = Math.hypot(nx, ny, nz) || 1;
    nx /= l; ny /= l; nz /= l;
    const r = ((color >> 16) & 255) / 255, g = ((color >> 8) & 255) / 255, bl = (color & 255) / 255;
    for (let k = 0; k < 9; k += 3) {
      pos.set(p.subarray(k, k + 3), w + k);
      nrm[w + k] = nx; nrm[w + k + 1] = ny; nrm[w + k + 2] = nz;
      col[w + k] = r; col[w + k + 1] = g; col[w + k + 2] = bl;
    }
    w += 9;
  }
  // the box the camera fits, from the positions themselves
  const box = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
  for (let k = 0; k < w; k += 3) {
    for (let a = 0; a < 3; a += 1) {
      box[a] = Math.min(box[a], pos[k + a]);
      box[a + 3] = Math.max(box[a + 3], pos[k + a]);
    }
  }
  if (!w) box.splice(0, 6, 0, 0, 0, 1, 1, 1);
  return { pos: pos.subarray(0, w), nrm: nrm.subarray(0, w), col: col.subarray(0, w), count: w / 3, box };
}

// One revision-aware bounded asset store shared by reopened canvases.
const assets = new SolidAssets({ onChange(record) {
  if (view && view.key === record.key) view.receive(record);
} });
const mb = (n) => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

export function got(o, at = { bytes: 0, t: performance.now() }) {
  if (view && view.key === o?.key && Array.isArray(o.box3) && o.box3.length === 6) {
    const b = o.box3.map(x => x | 0);
    view.expectedBox = [b[0],-b[4],b[2],b[3],-b[1],b[5]];
  }
  return assets.receive(o, at);
}
export function reset() {
  assets.reset();
  if (view) { view.count = 0; view.box = null; view.expectedBox = null; view.restoreWanted = null; view.saying = false; view.ticking(true); view.later(); }
}

export function fit() {
  if (view && view.box) { view.fit(); view.later(); }
}

function persp(fov, a, near, far) {
  const f = 1 / Math.tan(fov / 2), nf = 1 / (near - far);
  return [f / a, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0];
}

function ortho(half, aspect, near, far) {
  return [1/(half*aspect),0,0,0,0,1/half,0,0,0,0,-2/(far-near),0,0,0,-(far+near)/(far-near),1];
}

function look(e, c, u) {
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const norm = (a) => { const l = Math.hypot(...a) || 1; return a.map((x) => x / l); };
  const z = norm(sub(e, c)), x = norm(cross(u, z)), y = cross(z, x);
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, e), -dot(y, e), -dot(z, e), 1];
}

function mul(a, b) {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c += 1) for (let r = 0; r < 4; r += 1) for (let k = 0; k < 4; k += 1) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return o;
}

// A camera turning freely about a pivot (the model's centre), as the phones'
// and the window's do: r, u, f are the view's right, up and forward; sx, sy
// the pan in the view plane; dist from the eye to the pivot's depth; home
// the fitted distance, which bounds the zoom so any model zooms alike
const V3 = {
  cross: (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  norm: (a) => { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / l, a[1] / l, a[2] / l]; },
  // v turned by angle a about the unit axis k (Rodrigues)
  turned: (v, k, a) => {
    const c = Math.cos(a), s = Math.sin(a), kv = V3.cross(k, v), d = (k[0] * v[0] + k[1] * v[1] + k[2] * v[2]) * (1 - c);
    return [0, 1, 2].map((i) => v[i] * c + kv[i] * s + k[i] * d);
  },
};

export class Orbit {
  constructor() {
    this.pivot = [0, 0, 0]; this.r = [1, 0, 0]; this.u = [0, 0, 1]; this.f = [0, 1, 0];
    this.sx = 0; this.sy = 0; this.dist = 1; this.home = 0; this.fov = 0.7;
  }

  target() { return [0, 1, 2].map((i) => this.pivot[i] + this.r[i] * this.sx + this.u[i] * this.sy); }
  eye() { const t = this.target(); return [0, 1, 2].map((i) => t[i] - this.f[i] * this.dist); }

  // world units per pixel at the pivot's depth
  unit(h) { return (this.dist * 2 * Math.tan(this.fov / 2)) / Math.max(h, 1); }

  // orthonormal again (no drift)
  square() {
    this.f = V3.norm(this.f);
    this.r = V3.norm(V3.cross(this.f, this.u));
    this.u = V3.norm(V3.cross(this.r, this.f));
    return this;
  }

  // looking at the pivot from yaw and pitch (radians), z up
  aim(yaw, pitch) {
    this.f = [-Math.sin(yaw) * Math.cos(pitch), Math.cos(yaw) * Math.cos(pitch), -Math.sin(pitch)];
    this.u = [0, 0, 1];
    return this.square();
  }

  turn(k, a) {
    this.r = V3.turned(this.r, k, a); this.u = V3.turned(this.u, k, a); this.f = V3.turned(this.f, k, a);
    return this.square();
  }

  // the pointer moved (mx, my) pixels: the model under it follows
  spin(mx, my, k = 0.008) {
    const l = Math.hypot(mx, my);
    if (l < 1e-6) return this;
    return this.turn([0, 1, 2].map((i) => (this.u[i] * mx + this.r[i] * my) / l), -l * k);
  }

  // the model turned clockwise on screen by a
  roll(a) { return this.turn(this.f.slice(), -a); }

  // the point under the pointer moves with it (s: world units per pixel)
  pan(mx, my, s) { this.sx -= mx * s; this.sy += my * s; return this; }

  // dist over k, keeping the point (x, y) world units from the view's centre
  // where it is
  zoom(k, x, y) {
    const lo = this.home > 0 ? this.home / 60 : 1e-9, hi = this.home > 0 ? this.home * 20 : 1e9;
    const d = Math.min(Math.max(this.dist / k, lo), hi), q = d / this.dist;
    this.sx += x * (1 - q); this.sy += y * (1 - q); this.dist = d;
    return this;
  }
}

// a wheel event from a touchpad (two fingers scrolling: pan) or from a mouse
// wheel (zoom). A pinch comes as a wheel with ctrl. Touchpads scroll in small
// pixel steps, sideways too; a wheel in notches of about 100 px, or in lines.
// A gesture keeps the kind its first event had.
export function wheelKind(e, last) {
  if (e.ctrlKey) return "pinch";
  if (last && last.kind !== "pinch" && e.timeStamp - last.t < 250) return last.kind;
  if (e.deltaMode !== 0) return "wheel";
  if (e.deltaX !== 0 || Math.abs(e.deltaY) < 40 || !Number.isInteger(e.deltaY)) return "pad";
  return "wheel";
}

// one canvas's view of a model, turned, moved and zoomed by the pointer:
// a mouse's left drag turns (shift or ctrl, or the right or middle button,
// moves it; alt rolls), the wheel zooms at the pointer, a touchpad's two
// fingers move it and a pinch zooms; on a touch screen one finger turns and
// two move, pinch and twist it, as on the phones. Double-click fits.
class View {
  constructor(canvas) {
    this.canvas = canvas;
    this.key = "";
    this.gl = canvas.getContext("webgl", { antialias: true, alpha: true, premultipliedAlpha: true });
    this.count = 0;
    this.cam = new Orbit().aim(-0.6, 0.5);
    this.r = 1;
    this.projection = "perspective"; this.exposure = 0;
    this.thermal = { enabled: false, temperature: 25, liquid: 0 };
    this.aperture = "off"; this.focusLocked = false; this.focusDistance = 0;
    if (!this.gl) return;
    const gl = this.gl;
    this.events = new AbortController();
    const listen = (type,fn,options={}) => canvas.addEventListener(type,fn,{...options,signal:this.events.signal});
    const sh = (type, src) => {
      const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(`3D shader: ${gl.getShaderInfoLog(s)}`);
      return s;
    };
    this.prog = gl.createProgram();
    const vertex = sh(gl.VERTEX_SHADER, VS), fragment = sh(gl.FRAGMENT_SHADER, FS);
    gl.attachShader(this.prog, vertex);
    gl.attachShader(this.prog, fragment);
    gl.linkProgram(this.prog);
    gl.deleteShader(vertex); gl.deleteShader(fragment);
    if (!gl.getProgramParameter(this.prog, gl.LINK_STATUS)) throw new Error(`3D program: ${gl.getProgramInfoLog(this.prog)}`);
    this.bufs = ["p", "n", "c"].map((a) => ({ a: gl.getAttribLocation(this.prog, a), b: gl.createBuffer() }));
    this.dof = new DepthOfField(gl);
    if (!this.dof.available) console.info(`3D depth of field disabled: ${this.dof.reason}`);
    this.pts = new Map();
    this.drag = null;
    this.gesture = null;
    this.wheelLast = null;
    const local = (e) => { const b = canvas.getBoundingClientRect(); return { x: e.clientX - b.left, y: e.clientY - b.top }; };
    const two = () => {
      const [a, b] = [...this.pts.values()];
      return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, d: Math.hypot(a.x - b.x, a.y - b.y), a: Math.atan2(b.y - a.y, b.x - a.x) };
    };
    listen("pointerdown", (e) => {
      try { canvas.setPointerCapture(e.pointerId); } catch {}
      this.pts.set(e.pointerId, local(e));
      canvas.focus({preventScroll:true});
      const p = local(e);
      // a press that stays put is a tap: it probes the part under it
      this.tap = this.pts.size === 1 && e.button === 0 ? { x: e.clientX, y: e.clientY } : null;
      const mode = e.pointerType === "touch" ? "turn"
        : e.button === 1 || e.button === 2 || e.shiftKey || e.ctrlKey || e.metaKey ? "pan" : e.altKey ? "roll" : "turn";
      this.drag = this.pts.size === 1 ? { x: p.x, y: p.y, mode } : null;
      this.gesture = this.pts.size === 2 ? two() : null;
      canvas.classList.add("dragging");
    });
    listen("pointermove", (e) => {
      if (!this.pts.has(e.pointerId)) return;
      this.pts.set(e.pointerId, local(e));
      const h = canvas.clientHeight;
      if (this.gesture && this.pts.size === 2) {
        const g = two(), was = this.gesture;
        this.cam.pan(g.x - was.x, g.y - was.y, this.cam.unit(h));
        this.zoomAt(g.d / Math.max(was.d, 1), g.x, g.y);
        let da = g.a - was.a;
        if (da > Math.PI) da -= 2 * Math.PI; else if (da < -Math.PI) da += 2 * Math.PI;
        this.cam.roll(da);
        this.gesture = g;
        this.later();
        return;
      }
      if (!this.drag) return;
      if (this.tap && Math.hypot(e.clientX - this.tap.x, e.clientY - this.tap.y) > 5) this.tap = null;
      const p = local(e), dx = p.x - this.drag.x, dy = p.y - this.drag.y;
      this.drag.x = p.x; this.drag.y = p.y;
      if (this.drag.mode === "pan") this.cam.pan(dx, dy, this.cam.unit(h));
      else if (this.drag.mode === "roll") this.cam.roll(dx * 0.008);
      else this.cam.spin(dx, dy);
      this.later();
    });
    const up = (e) => {
      const tap = e.type === "pointerup" && this.tap && this.pts.size === 1 ? this.tap : null;
      this.tap = null;
      if (tap) this.pick(e.clientX, e.clientY);
      this.pts.delete(e.pointerId);
      this.gesture = null;
      const [rest] = [...this.pts.values()];
      this.drag = rest ? { x: rest.x, y: rest.y, mode: "turn" } : null;
      if (!rest) canvas.classList.remove("dragging");
    };
    listen("pointerup", up);
    listen("pointercancel", up);
    listen("wheel", (e) => {
      e.preventDefault();
      const kind = wheelKind(e, this.wheelLast);
      this.wheelLast = { kind, t: e.timeStamp };
      const px = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? canvas.clientHeight : 1;
      const p = local(e);
      if (kind === "pad") this.cam.pan(-e.deltaX * px, -e.deltaY * px, this.cam.unit(canvas.clientHeight));
      else this.zoomAt(Math.exp(-e.deltaY * px * (kind === "pinch" ? 0.01 : 0.0015)), p.x, p.y);
      this.later();
    }, { passive: false });
    listen("dblclick", () => { this.fit(); this.later(); });
    listen("contextmenu", (e) => e.preventDefault());
    listen("keydown", (e) => {
      const action = {f:"fit",F:"fit",Home:"fit",0:"iso",1:"top",2:"bottom",3:"front",4:"right",5:"back",6:"left"}[e.key];
      if (action) cameraAction(action);
      else if (["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(e.key)) {
        const dx = e.key === "ArrowLeft" ? -16 : e.key === "ArrowRight" ? 16 : 0;
        const dy = e.key === "ArrowUp" ? -16 : e.key === "ArrowDown" ? 16 : 0;
        if (e.shiftKey) this.cam.pan(dx,dy,this.cam.unit(canvas.clientHeight)); else this.cam.spin(dx,dy);
        this.later();
      } else if (["+","=","-","_"].includes(e.key)) { this.cam.zoom(e.key === "-" || e.key === "_" ? 1/1.15 : 1.15,0,0); this.later(); }
      else return;
      e.preventDefault();
    });
    // a pane resized (the viewer's edge dragged) draws again at its new size
    this.resize = new ResizeObserver(() => this.later());
    this.resize.observe(canvas);
  }

  // closer by k (over 1 in), keeping the point under (x, y) on the canvas where it is
  zoomAt(k, x, y) {
    const w = this.canvas.clientWidth, h = this.canvas.clientHeight, s = this.cam.unit(h);
    this.cam.zoom(k, (x - w / 2) * s, (h / 2 - y) * s);
  }

  fit() {
    const [x0, y0, z0, x1, y1, z1] = this.box;
    this.r = Math.max(Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2, 1);
    // the field of view is vertical: a narrow canvas fits the width instead
    const a = this.canvas.clientWidth / Math.max(this.canvas.clientHeight, 1);
    const cam = new Orbit();
    for (const axis of ["r", "u", "f"]) cam[axis] = this.cam[axis].slice();
    cam.fov = this.cam.fov;
    cam.pivot = [(x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2];
    cam.dist = fittedDistance(this.box, cam, a, this.projection === "orthographic");
    cam.home = cam.dist;
    this.cam = cam;
  }

  // the part under a tap (a ray from the eye through it, to the nearest
  // triangle; then the smallest part whose box on that side of the board
  // holds the point), probed; nothing under it clears the probe
  pick(cx, cy) {
    if (!probed || !this.pos || !board || !isBoard(this.key)) return;
    const c = this.canvas, b = c.getBoundingClientRect(), w = c.clientWidth, h = c.clientHeight;
    const cam = this.cam, s = cam.unit(h), eye = cam.eye(), t = cam.target();
    const px = (cx - b.left - w / 2) * s, py = (h / 2 - (cy - b.top)) * s;
    const q = [0, 1, 2].map((i) => t[i] + cam.r[i] * px + cam.u[i] * py);
    const d = V3.norm([q[0] - eye[0], q[1] - eye[1], q[2] - eye[2]]);
    const hit = rayHit(this.pos, eye, d);
    const key = boardOf(this.key);
    if (!hit) { probed(""); return; }
    // board coordinates: micrometres, y down
    const bx = hit[0] * 10, by = -hit[1] * 10, back = hit[2] * 10 < board.thick(key) / 2;
    Promise.resolve(board.need(key)).then(() => {
      const ps = board.parts(key);
      let best = "", a = Infinity;
      if (ps) for (const [ref, e] of ps) {
        const bb = e.box, m = 300;
        if ((e.back > e.front) !== back) continue;
        if (bx < bb[0] - m || bx > bb[2] + m || by < bb[1] - m || by > bb[3] + m) continue;
        const ar = (bb[2] - bb[0]) * (bb[3] - bb[1]);
        if (ar < a) { a = ar; best = ref; }
      }
      const v = best ? `c\t${best}` : "";
      this.probe = v;
      this.later();
      probed(v);
    });
  }

  // the probed part's box on the model (or none): x0 y0 x1 y1 in the
  // model's axes (y up), and the heights of its side of the board
  lit() {
    const ref = this.probe.startsWith("c\t") ? this.probe.slice(2) : "";
    if (!ref || !board || !isBoard(this.key)) return null;
    const key = boardOf(this.key), ps = board.parts(key);
    if (!ps) { board.need(key); return null; }
    const e = ps.get(ref);
    if (!e) return null;
    const m = 25, th = board.thick(key) / 10;
    return { box: [e.box[0] / 10 - m, -e.box[3] / 10 - m, e.box[2] / 10 + m, -e.box[1] / 10 + m], z: e.back > e.front ? [-1e7, 4] : [th - 4, 1e7] };
  }

  // the probed part brought into view: turned about its middle, close
  show_probe() {
    const l = this.lit();
    if (!l) return;
    const [x0, y0, x1, y1] = l.box;
    const cam = this.cam;
    cam.pivot = [(x0 + x1) / 2, (y0 + y1) / 2, l.z[0] > 0 ? l.z[0] : 0];
    cam.sx = 0; cam.sy = 0;
    cam.dist = Math.max(Math.hypot(x1 - x0, y1 - y0) * 2.5, (cam.home || 1) / 20);
    this.later();
  }

  show(m) {
    this.saying = false;
    this.ticking(false);
    if (!this.gl || !m) return;
    this.pos = m.pos;
    const gl = this.gl;
    if (this.uploadedBuffer !== m.pos.buffer || this.uploadedCount !== m.count) {
      [m.pos, m.nrm, m.col].forEach((d, k) => { gl.bindBuffer(gl.ARRAY_BUFFER, this.bufs[k].b); gl.bufferData(gl.ARRAY_BUFFER, d, gl.STATIC_DRAW); });
      this.uploadedBuffer = m.pos.buffer; this.uploadedCount = m.count;
    }
    const first = this.count === 0;
    this.count = m.count;
    this.box = this.expectedBox || m.box;
    this.r = Math.max(Math.hypot(this.box[3]-this.box[0],this.box[4]-this.box[1],this.box[5]-this.box[2])/2,1);
    if (first) {
      if (this.restoreWanted) { this.restore(this.restoreWanted); this.restoreWanted = null; }
      else this.fit();
    }
    this.later();
  }

  later() {
    if (!this.focusLocked) this.focusDistance = this.cam.dist / 100;
    this.canvas.dispatchEvent(new CustomEvent("solid-camera-change", {bubbles:true}));
    if (this.queued) return;
    this.queued = true;
    this.frame = requestAnimationFrame(() => { this.queued = false; this.draw(); });
  }

  draw() {
    const gl = this.gl, c = this.canvas;
    if (!gl) { if (this.note) this.note.textContent = "3D unavailable: WebGL could not start"; return; }
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(c.clientWidth * dpr)), h = Math.max(1, Math.round(c.clientHeight * dpr));
    if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
    gl.bindFramebuffer(gl.FRAMEBUFFER,null);
    gl.viewport(0, 0, w, h);
    const dof = this.count && this.projection === "perspective" && this.aperture !== "off" && this.dof?.begin(w,h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (this.note && !this.saying) this.note.textContent = this.count ? "" : assets.get(this.key)?.error || this.waiting();
    const record = assets.get(this.key);
    if (this.stat) this.stat.textContent = this.count && record?.status === "ready" ? this.statLine({ ...record.stats, build: record.stats.build / 1000, waited: this.waited || 0, tris: this.count / 3 }) : "";
    if (!this.count) return;
    gl.enable(gl.DEPTH_TEST);
    gl.useProgram(this.prog);
    const cam = this.cam, eye = cam.eye();
    const reach = cam.dist + Math.hypot(cam.sx, cam.sy) + this.r * 4;
    const near = Math.max(cam.dist / 100, this.r / 1000);
    const projection = this.projection === "orthographic"
      ? ortho(cam.dist * Math.tan(cam.fov / 2), w / h, near, reach)
      : persp(cam.fov, w / h, near, reach);
    const m = mul(projection, look(eye, cam.target(), cam.u));
    gl.uniformMatrix4fv(gl.getUniformLocation(this.prog, "m"), false, m);
    gl.uniform3fv(gl.getUniformLocation(this.prog, "eye"), eye);
    const lit = this.lit();
    gl.uniform1f(gl.getUniformLocation(this.prog, "hon"), lit ? 1 : 0);
    gl.uniform4fv(gl.getUniformLocation(this.prog, "hl"), lit ? lit.box : [0, 0, 0, 0]);
    gl.uniform2fv(gl.getUniformLocation(this.prog, "hz"), lit ? lit.z : [0, 0]);
    // a board model's faces, for its glossy soldermask (solid-materials.js)
    gl.uniform3fv(gl.getUniformLocation(this.prog, "bz"), board && isBoard(this.key) ? [1, 0, board.thick(boardOf(this.key)) / 10] : [0, 0, 0]);
    gl.uniform1f(gl.getUniformLocation(this.prog, "orthographic"), Number(this.projection === "orthographic"));
    for (const [name, value] of [["right", cam.r], ["up", cam.u], ["forward", cam.f]]) gl.uniform3fv(gl.getUniformLocation(this.prog, name), value);
    gl.uniform1f(gl.getUniformLocation(this.prog, "exposure"), this.exposure);
    gl.uniform3fv(gl.getUniformLocation(this.prog, "thermal"), [Number(this.thermal.enabled), this.thermal.temperature, this.thermal.liquid]);
    for (const { a, b } of this.bufs) {
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.enableVertexAttribArray(a);
      gl.vertexAttribPointer(a, 3, gl.FLOAT, false, 0, 0);
    }
    gl.drawArrays(gl.TRIANGLES, 0, this.count);
    if (dof) this.dof.finish(w,h,near,reach,this.focusDistance,12/Math.tan(cam.fov/2),this.aperture);
  }
}

// Progress: while the hub prepares a model (a board's export, a STEP's
// meshing: the long part, and it has no progress to report) the note counts
// the seconds; when it comes, its size; once drawn, a line says how it came
View.prototype.waiting = function () {
  const s = this.asked ? Math.floor((performance.now() - this.asked) / 1000) : 0;
  return App.solid_waiting(document.documentElement.lang, BigInt(s));
};

View.prototype.statLine = function (st) {
  const k = st.tris >= 1000 ? `${Math.round(st.tris / 1000)}k` : `${st.tris}`;
  return App.solid_stats(document.documentElement.lang, mb(st.bytes), k, st.waited.toFixed(1), st.build.toFixed(1), st.waited > 0.5);
};

View.prototype.say = function (text) {
  this.saying = true;
  if (this.note) this.note.textContent = text;
};

// the seconds tick while a model is awaited, and stop once it is in
View.prototype.ticking = function (on) {
  if (on && !this.tick) this.tick = setInterval(() => { if (!this.count && assets.get(this.key)?.status !== "failed") this.later(); else this.ticking(false); }, 1000);
  if (!on && this.tick) { clearInterval(this.tick); this.tick = null; }
};

View.prototype.receive = function(record) {
  this.record = record;
  this.expectedBox = record.box || this.expectedBox;
  if (record.model) this.show(record.model);
  if (record.status === "decoding") {
    const {completed,total} = record.progress;
    this.say((completed ? `Building model ${Math.round(completed/Math.max(total,1)*100)}%` : `Received ${mb(record.stats.bytes)}, building the model…`) + (record.stale ? " — previous revision shown" : ""));
  } else if (record.status === "failed") {
    if (!record.model) this.count = 0;
    this.say(record.error + (record.stale ? " — previous revision shown" : ""));
  } else {
    this.saying = false; this.waited = this.asked ? (performance.now()-this.asked)/1000 : 0;
  }
  this.ticking(false); this.later();
};
View.prototype.snapshot = function() {
  return { cam: { ...this.cam }, projection: this.projection, exposure: this.exposure, thermal: { ...this.thermal }, aperture:this.aperture, focusDistance:this.focusDistance, focusLocked:this.focusLocked, selection:this.selection || null };
};
View.prototype.restore = function(state) {
  this.cam = Object.assign(new Orbit(),state.cam);
  this.projection = state.projection; this.exposure = state.exposure; this.thermal = { ...state.thermal };
  this.aperture = state.aperture || "off"; this.focusDistance = state.focusDistance || this.cam.dist/100; this.focusLocked = !!state.focusLocked; this.selection = state.selection || null;
};
View.prototype.dispose = function() {
  this.ticking(false); this.resize?.disconnect(); this.events?.abort(); this.canvas.classList.remove("dragging"); this.dof?.dispose();
  if (this.frame) cancelAnimationFrame(this.frame);
  if (this.gl) { for (const {b} of this.bufs || []) this.gl.deleteBuffer(b); this.gl.deleteProgram(this.prog); }
};
let view = null;

// Cross-probing (probe.js): a board model's parts are found on the board's
// own plot (plot2d.js, through host.js: parts(key), thick(key), need(key)),
// and a tap's probe goes to the page (host.js tells the hub)
let board = null;
let probed = null;
export function useBoard(b) { board = b; }
export function onProbe(fn) { probed = fn; }
const boardOf = (key) => `0|${key.slice(2)}`;
// a board's model (the project's own, or a .kicad_pcb), not a part's (Mech)
const isBoard = (key) => { const path = key.split("|").slice(2).join("|"); return path === "" || path.endsWith(".kicad_pcb"); };

// the nearest triangle a ray from o along d meets (Möller–Trumbore), its
// point, or null
function rayHit(pos, o, d) {
  let best = Infinity;
  for (let i = 0; i + 8 < pos.length; i += 9) {
    const ax = pos[i], ay = pos[i + 1], az = pos[i + 2];
    const e1x = pos[i + 3] - ax, e1y = pos[i + 4] - ay, e1z = pos[i + 5] - az;
    const e2x = pos[i + 6] - ax, e2y = pos[i + 7] - ay, e2z = pos[i + 8] - az;
    const px = d[1] * e2z - d[2] * e2y, py = d[2] * e2x - d[0] * e2z, pz = d[0] * e2y - d[1] * e2x;
    const det = e1x * px + e1y * py + e1z * pz;
    if (Math.abs(det) < 1e-12) continue;
    const inv = 1 / det, tx = o[0] - ax, ty = o[1] - ay, tz = o[2] - az;
    const u = (tx * px + ty * py + tz * pz) * inv;
    if (u < 0 || u > 1) continue;
    const qx = ty * e1z - tz * e1y, qy = tz * e1x - tx * e1z, qz = tx * e1y - ty * e1x;
    const v = (d[0] * qx + d[1] * qy + d[2] * qz) * inv;
    if (v < 0 || u + v > 1) continue;
    const t = (e2x * qx + e2y * qy + e2z * qz) * inv;
    if (t > 0 && t < best) best = t;
  }
  return best < Infinity ? [o[0] + d[0] * best, o[1] + d[1] * best, o[2] + d[2] * best] : null;
}

// the probed part in view (the chip's name)
export function showProbe() {
  view?.show_probe();
}

export function relight() {
  view?.later();
}

// a probe made elsewhere (a plot's tap, Esc), shown at once
export function setProbe(v) {
  if (view) { view.probe = v; view.later(); }
}

// after each render: the canvas on the page (if any) shows the model its
// data-key names, once the hub has sent it. Each source keeps its own pose
// when tabs/canvases are remounted.
export function mount(canvas) {
  if (!canvas || (view && view.canvas !== canvas)) {
    if (view) { assets.saveState(view.stateKey,view.snapshot()); view.dispose(); }
    view = null;
  }
  if (!canvas) { assets.pin(""); return; }
  if (!view) { view = new View(canvas); view.probe = ""; }
  const probe = canvas.dataset.probe ?? "";
  if (probe !== view.hubProbe) { view.hubProbe = probe; view.probe = probe; view.later(); }
  view.note = canvas.parentElement?.querySelector(".viewer-note") ?? null;
  view.stat = canvas.parentElement?.querySelector(".viewer-stat") ?? null;
  const key = canvas.dataset.key || "";
  const stateKey = `${canvas.dataset.scope || ""}\0${key}`;
  if (key !== view.key || stateKey !== view.stateKey) {
    assets.saveState(view.stateKey,view.snapshot());
    view.key = key; view.stateKey = stateKey; view.count = 0; view.saying = false; view.expectedBox = null;
    view.box = null; view.restoreWanted = assets.getState(stateKey);
    if (!view.restoreWanted) {
      view.cam = new Orbit().aim(-0.6,0.5); view.projection = "perspective"; view.exposure = 0;
      view.thermal = {enabled:false,temperature:25,liquid:0}; view.selection = null;
      view.aperture = "off"; view.focusLocked = false;
    }
    view.asked = performance.now(); assets.pin(key);
    const record = assets.reuse(key);
    if (record) view.receive(record);
    else { view.ticking(true); view.later(); }
  } else view.later();
}

// Read-only status for the host: no transport buffers are exposed.
export function assetState() {
  const r = view && assets.get(view.key);
  return r ? { key:r.key, status:r.status, progress:{...r.progress}, error:r.error, stale:r.stale, cached:r.cached } : null;
}

// Presentation hooks for the Bend-owned toolbar. No animation or CAD mutation.
const bounded = (value, lo, hi, fallback) => Number.isFinite(Number(value)) ? Math.max(lo, Math.min(hi, Number(value))) : fallback;
export function cameraAction(action, value) {
  if (!view) return;
  const poses = { iso: [-0.6,0.5], top: [0,Math.PI/2], bottom: [Math.PI,-Math.PI/2],
    front: [0,0], back: [Math.PI,0], left: [-Math.PI/2,0], right: [Math.PI/2,0] };
  if (action === "fit") { if(view.box) view.fit(); }
  else if (poses[action]) view.cam.aim(...poses[action]);
  else if (action === "projection") view.projection = value === "orthographic" ? value : "perspective";
  else if (action === "exposure") view.exposure = bounded(value,-3,3,0);
  else if (action === "aperture" && view.dof?.available) view.aperture = value === "off" ? "off" : bounded(value,1.4,64,14);
  else if (action === "focus-lock" && view.dof?.available) view.focusLocked = !view.focusLocked;
  else if (action === "focus" && view.dof?.available) { view.focusDistance = bounded(value,0.01,1e6,view.cam.dist/100); view.focusLocked = true; }
  else if (action === "focal") {
    const fov = 2*Math.atan(12/bounded(value,15,250,33));
    const scale = Math.tan(view.cam.fov/2)/Math.tan(fov/2);
    view.cam.dist *= scale; view.cam.home *= scale; view.cam.fov = fov;
  } else return;
  view.later();
}
export function setThermal(value = {}) {
  if (!view) return;
  view.thermal = { enabled: value.enabled === true, temperature: bounded(value.temperature,0,400,25), liquid: bounded(value.liquid,0,1,0) };
  view.later();
}
export function cameraState() {
  return view ? { projection: view.projection, focal: 12/Math.tan(view.cam.fov/2), exposure: view.exposure,
    selection:view.selection || null, materialSource: "color-estimate", thermal: { ...view.thermal },
    ...(view.dof?.available ? { aperture:view.aperture, focusDistance:view.focusDistance, focusLocked:view.focusLocked } : {}) } : null;
}

// A read-only pose for higher-quality renderers of the same saved geometry.
export function cameraSnapshot() { return view ? structuredClone(view.snapshot()) : null; }
// whether the camera means anything yet: fitted to this view's model, or a
// saved pose about to be restored; a fresh default orbit looks at nothing
export function cameraFitted() { return !!view && (!!view.box || !!view.restoreWanted); }

// Fit verified original-model bounds without replacing the whole-scene box.
// Board coordinates are millimetres, matching component metadata/projection.
export function frameBounds(min, max) {
  if (!view?.box || !Array.isArray(min) || !Array.isArray(max) || min.length !== 3 || max.length !== 3 || ![...min, ...max].every(Number.isFinite) || min.some((v, i) => v > max[i])) return;
  const cam = view.cam, aspect = view.canvas.clientWidth / Math.max(1, view.canvas.clientHeight);
  cam.pivot = [(min[0] + max[0]) * 50, -(min[1] + max[1]) * 50, (min[2] + max[2]) * 50];
  const box = [min[0] * 100, -max[1] * 100, min[2] * 100, max[0] * 100, -min[1] * 100, max[2] * 100];
  cam.sx = 0; cam.sy = 0; cam.dist = fittedDistance(box, cam, aspect, view.projection === "orthographic", 1.15); cam.home = cam.dist;
  view.later();
}

// Saved-PCB metadata overlays use board millimetres, while the mesh wire is
// 10um and reflected in Y. These helpers do not infer component/triangle IDs.
export function projectPoint(point) {
  if (!view?.box || !Array.isArray(point) || point.length < 3 || !point.every(Number.isFinite)) return null;
  const cam=view.cam,w=view.canvas.clientWidth,h=view.canvas.clientHeight;
  const p=[point[0]*100,-point[1]*100,point[2]*100],eye=cam.eye(),d=p.map((v,i)=>v-eye[i]);
  const dot=a=>d.reduce((s,v,i)=>s+v*a[i],0),depth=dot(cam.f);
  if(depth<=0)return null;
  const half=(view.projection==='orthographic'?cam.dist:depth)*Math.tan(cam.fov/2);
  const x=w/2+dot(cam.r)*h/(2*half),y=h/2-dot(cam.u)*h/(2*half);
  return {x,y,depth:depth/100,visible:x>=0&&x<=w&&y>=0&&y<=h};
}
export function rayToBoard(x,y,z=0) {
  if(!view?.box || ![x,y,z].every(Number.isFinite))return null;
  const cam=view.cam,w=view.canvas.clientWidth,h=Math.max(1,view.canvas.clientHeight),scale=Math.tan(cam.fov/2);
  const nx=(x-w/2)*2/h,ny=(h/2-y)*2/h;
  const orthographic=view.projection==='orthographic',eye=cam.eye();
  const origin=orthographic?eye.map((v,i)=>v+cam.dist*scale*(cam.r[i]*nx+cam.u[i]*ny)):eye;
  const dir=orthographic?cam.f:V3.norm(cam.f.map((v,i)=>v+scale*(cam.r[i]*nx+cam.u[i]*ny)));
  if(Math.abs(dir[2])<1e-9)return null;
  const distance=(z*100-origin[2])/dir[2];if(distance<0)return null;
  const p=origin.map((v,i)=>v+dir[i]*distance);return [p[0]/100,-p[1]/100,p[2]/100];
}
export function setSelection(selection) {
  if(!view)return;
  view.selection=selection==null?null:structuredClone(selection);view.later();
}
