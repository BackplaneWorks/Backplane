// Raw WebGL2 3D viewer for Backplane boards (+ streaming parts) and Mech STEP models.
// Ports native/gpui/src/viewers/solid.wgsl & raster.rs:
//   - Linear RGB colors (0xTTRRGGBB, TT = transparency 0..255), sRGB transfer once on output
//   - PBR color-estimate material model (estimate, GGX lit, analytic studio env, fresnel, glossy soldermask coat)
//   - Two-pass rendering: opaque pass (depthMask true) -> translucent pass (TT > 0, depthMask false) -> fading parts
//   - Vertical linear-light Backdrop gradient (DARK #4d535e->#353a43, LIGHT #f5f6f8->#e1e4e9)
//   - Smooth vertex normals decoded off-thread by mesh-worker.ts; zero idle repaint loops

import { fnv1a64Hex, type CborValue } from "./cbor.ts";

export interface Solid3DCallbacks {
  onStatus?: (msg: string, streaming?: boolean) => void;
  onUserMove?: () => void;
}

interface GpuSubBatch {
  vao: WebGLVertexArrayObject;
  posBuf: WebGLBuffer;
  nrmBuf: WebGLBuffer;
  colBuf: WebGLBuffer;
  vertCount: number;
}

interface GpuBatch {
  opaque: GpuSubBatch | null;
  translucent: GpuSubBatch | null;
  fadeStartMs: number; // 0 = board (always 1.0), >0 = streaming part fading in over 300ms
}

const BG_VERT_SRC = `#version 300 es
precision highp float;
out vec2 vUv;
void main() {
  float x = float((gl_VertexID & 1) * 4 - 1);
  float y = float((gl_VertexID >> 1) * 4 - 1);
  vUv = vec2(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
  gl_Position = vec4(x, y, 0.0, 1.0);
}
`;

const BG_FRAG_SRC = `#version 300 es
precision highp float;
in vec2 vUv;
uniform vec3 uBgTop;
uniform vec3 uBgBot;
out vec4 outColor;

vec3 linearToSrgb(vec3 c) {
  vec3 lo = c * 12.92;
  vec3 hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
  return mix(lo, hi, step(vec3(0.0031308), c));
}

void main() {
  vec3 lin = mix(uBgTop, uBgBot, clamp(vUv.y, 0.0, 1.0));
  outColor = vec4(linearToSrgb(clamp(lin, 0.0, 1.0)), 1.0);
}
`;

const VERT_SRC = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNrm;
layout(location = 2) in vec4 aCol;

uniform mat4 uMvp;

out vec3 vPos;
out vec3 vNrm;
out vec4 vCol;

void main() {
  vPos = aPos;
  vNrm = aNrm;
  vCol = aCol;
  gl_Position = uMvp * vec4(aPos, 1.0);
}
`;

const FRAG_SRC = `#version 300 es
precision highp float;
in vec3 vPos;
in vec3 vNrm;
in vec4 vCol;

uniform vec3 uEye;
uniform vec3 uRight;
uniform vec3 uUp;
uniform vec3 uFwd;
uniform float uFade;

out vec4 outColor;

const float PI = 3.14159265;

vec3 linearToSrgb(vec3 c) {
  vec3 lo = c * 12.92;
  vec3 hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
  return mix(lo, hi, step(vec3(0.0031308), c));
}

vec3 estimate(vec3 c) {
  float hi = max(c.r, max(c.g, c.b));
  float lo = min(c.r, min(c.g, c.b));
  if (hi < 0.12 && hi - lo < max(0.015, hi * 0.45)) return vec3(0.63, 0.0, 0.12);
  if (lo > 0.9) return vec3(0.82, 0.0, 0.0);
  if (c.r > 0.18 && c.g > c.r * 0.55 && c.g < c.r * 1.02 && c.b < c.g * 0.45) return vec3(0.3, 0.86, 0.0);
  if (hi - lo < hi * 0.16 && lo > 0.18 && hi < 0.9) return vec3(0.28, 0.82, 0.0);
  if (c.r > c.b * 1.25 && c.g > c.b * 1.12 && c.r > 0.06) return vec3(0.72, 0.0, 0.0);
  if (hi - lo > max(0.015, hi * 0.28)) return vec3(0.36, 0.0, 0.35);
  return vec3(0.55, 0.0, 0.0);
}

vec3 fresnel(vec3 f0, float h) {
  return f0 + (vec3(1.0) - f0) * pow(1.0 - h, 5.0);
}

vec3 lit(vec3 N, vec3 V, vec3 L, vec3 base, vec3 f0, float rough, float metal) {
  vec3 H = normalize(V + L);
  float nl = max(dot(N, L), 0.0);
  float nv = max(dot(N, V), 0.001);
  float nh = max(dot(N, H), 0.0);
  float a = rough * rough;
  float a2 = a * a;
  float d = nh * nh * (a2 - 1.0) + 1.0;
  float D = a2 / max(PI * d * d, 0.0001);
  float k = (rough + 1.0) * (rough + 1.0) / 8.0;
  float G = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
  vec3 F = fresnel(f0, max(dot(V, H), 0.0));
  return (base * (1.0 - metal) * (vec3(1.0) - F) / PI + F * D * G / max(4.0 * nl * nv, 0.001)) * nl;
}

vec3 studio(vec3 R, float rough, float metal) {
  vec3 q = vec3(dot(R, uRight), dot(R, uUp), -dot(R, uFwd));
  float power = mix(110.0, 3.0, rough * rough);
  float key = pow(max(dot(q, normalize(vec3(-0.55, 0.75, 0.65))), 0.0), power);
  float fill = pow(max(dot(q, normalize(vec3(0.85, 0.25, 0.35))), 0.0), power * 0.65);
  float sky = 0.12 + 0.2 * smoothstep(-0.4, 0.25, q.y) + 0.7 * smoothstep(0.45, 0.85, q.y);
  return vec3(sky * mix(0.15, 1.0, metal)) + vec3(1.5, 1.48, 1.42) * key + vec3(0.65, 0.72, 0.85) * fill;
}

void main() {
  vec3 base = vCol.rgb;
  float alpha = vCol.a;
  vec3 m = estimate(base);
  // Translucent non-silk layer (soldermask): glossy coat matching solid.wgsl / raster.rs
  if (alpha < 0.996 && m.x < 0.8) {
    m = vec3(0.24, 0.0, 1.0);
  }
  float rough = m.x;
  float metal = m.y;
  float coat = m.z;

  vec3 V = normalize(uEye - vPos);
  vec3 N = normalize(vNrm);
  if (dot(N, V) < 0.0) N = -N;
  float nv = max(dot(N, V), 0.0);
  vec3 f0 = mix(vec3(0.04), base, metal);

  vec3 keyDir = normalize(-uRight * 0.55 + uUp * 0.75 - uFwd * 0.65);
  vec3 fillDir = normalize(uRight * 0.85 + uUp * 0.25 - uFwd * 0.35);

  vec3 rgb = base * (1.0 - metal) * (mix(0.34, 0.80, step(0.8, rough)) + 0.10 * max(dot(N, uUp), 0.0));
  rgb += lit(N, V, keyDir, base, f0, rough, metal) * 1.5;
  rgb += lit(N, V, fillDir, base, f0, rough, metal) * 0.85;

  vec3 R = reflect(-V, N);
  rgb += studio(R, rough, metal) * fresnel(f0, nv) * (1.0 - rough * 0.35);
  if (coat > 0.0) {
    rgb += studio(R, 0.3, 0.0) * fresnel(vec3(0.04), nv) * coat;
  }
  // Soft highlight shoulder
  rgb = rgb / (vec3(1.0) + max(rgb - vec3(0.8), vec3(0.0)) * 0.28);
  outColor = vec4(linearToSrgb(clamp(rgb, 0.0, 1.0)), alpha * uFade);
}
`;

function srgbToLinearByte(c: number): number {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

// Backdrop::DARK (#4d535e -> #353a43) and Backdrop::LIGHT (#f5f6f8 -> #e1e4e9) in linear light
const BG_DARK_TOP: [number, number, number] = [
  srgbToLinearByte(0x4d),
  srgbToLinearByte(0x53),
  srgbToLinearByte(0x5e),
];
const BG_DARK_BOT: [number, number, number] = [
  srgbToLinearByte(0x35),
  srgbToLinearByte(0x3a),
  srgbToLinearByte(0x43),
];
const BG_LIGHT_TOP: [number, number, number] = [
  srgbToLinearByte(0xf5),
  srgbToLinearByte(0xf6),
  srgbToLinearByte(0xf8),
];
const BG_LIGHT_BOT: [number, number, number] = [
  srgbToLinearByte(0xe1),
  srgbToLinearByte(0xe4),
  srgbToLinearByte(0xe9),
];

function compileShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type);
  if (!s) throw new Error("createShader failed");
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(s) || "shader compile error";
    gl.deleteShader(s);
    throw new Error(log);
  }
  return s;
}

function linkProgram(gl: WebGL2RenderingContext, vertSrc: string, fragSrc: string): WebGLProgram {
  const vs = compileShader(gl, gl.VERTEX_SHADER, vertSrc);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, fragSrc);
  const prog = gl.createProgram();
  if (!prog) throw new Error("createProgram failed");
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(prog) || "program link error";
    gl.deleteProgram(prog);
    throw new Error(log);
  }
  return prog;
}

// Column-major 4x4 matrix helpers
function mul4(a: Float32Array, b: Float32Array): Float32Array {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      o[c * 4 + r] =
        a[0 * 4 + r] * b[c * 4 + 0] +
        a[1 * 4 + r] * b[c * 4 + 1] +
        a[2 * 4 + r] * b[c * 4 + 2] +
        a[3 * 4 + r] * b[c * 4 + 3];
    }
  }
  return o;
}

function perspective(fovRad: number, aspect: number, near: number, far: number): Float32Array {
  const f = 1 / Math.tan(fovRad * 0.5);
  const nf = 1 / (near - far);
  const m = new Float32Array(16);
  m[0] = f / aspect;
  m[5] = f;
  m[10] = (far + near) * nf;
  m[11] = -1;
  m[14] = 2 * far * near * nf;
  return m;
}

function normalize3(v: [number, number, number]): [number, number, number] {
  const l = Math.hypot(v[0], v[1], v[2]) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
}

function cross3(a: [number, number, number], b: [number, number, number]): [number, number, number] {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

function dot3(a: [number, number, number], b: [number, number, number]): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export class Solid3DViewer {
  private canvas: HTMLCanvasElement;
  private gl: WebGL2RenderingContext | null = null;
  private bgProg: WebGLProgram | null = null;
  private bgVao: WebGLVertexArrayObject | null = null;
  private uBgTop: WebGLUniformLocation | null = null;
  private uBgBot: WebGLUniformLocation | null = null;
  private prog: WebGLProgram | null = null;
  private uMvp: WebGLUniformLocation | null = null;
  private uEye: WebGLUniformLocation | null = null;
  private uRight: WebGLUniformLocation | null = null;
  private uUp: WebGLUniformLocation | null = null;
  private uFwd: WebGLUniformLocation | null = null;
  private uFade: WebGLUniformLocation | null = null;
  private worker: Worker | null = null;

  private boardBatch: GpuBatch | null = null;
  private partBatches: GpuBatch[] = [];
  private activeKey = "";
  private lastMeshId = "";
  private nextJobId = 1;
  private partsTotal = 0;
  private partsDone = 0;

  // Orbit camera state
  private target: [number, number, number] = [0, 0, 0];
  private radius = 50;
  private dist = 120;
  private yaw = 0.55;
  private pitch = 0.65;
  private lightTheme = false;

  private dirty = false;
  private rafId = 0;
  private pointers = new Map<number, { x: number; y: number; button: number; shift: boolean }>();
  private pinchDist = 0;
  private resizeObs: ResizeObserver;
  private cbs: Solid3DCallbacks;

  constructor(container: HTMLElement, cbs: Solid3DCallbacks = {}) {
    this.cbs = cbs;
    this.canvas = document.createElement("canvas");
    this.canvas.className = "bp-viewer-canvas";
    this.canvas.style.width = "100%";
    this.canvas.style.height = "100%";
    this.canvas.style.display = "block";
    this.canvas.style.touchAction = "none";
    container.appendChild(this.canvas);

    const gl = this.canvas.getContext("webgl2", {
      antialias: true,
      alpha: false,
      preserveDrawingBuffer: true, // allows headless screenshot inspection
    });
    if (gl) {
      this.gl = gl;
      this.bgProg = linkProgram(gl, BG_VERT_SRC, BG_FRAG_SRC);
      this.bgVao = gl.createVertexArray();
      this.uBgTop = gl.getUniformLocation(this.bgProg, "uBgTop");
      this.uBgBot = gl.getUniformLocation(this.bgProg, "uBgBot");

      this.prog = linkProgram(gl, VERT_SRC, FRAG_SRC);
      this.uMvp = gl.getUniformLocation(this.prog, "uMvp");
      this.uEye = gl.getUniformLocation(this.prog, "uEye");
      this.uRight = gl.getUniformLocation(this.prog, "uRight");
      this.uUp = gl.getUniformLocation(this.prog, "uUp");
      this.uFwd = gl.getUniformLocation(this.prog, "uFwd");
      this.uFade = gl.getUniformLocation(this.prog, "uFade");
      gl.enable(gl.DEPTH_TEST);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    } else {
      this.cbs.onStatus?.("WebGL2 is unavailable in this browser");
    }

    try {
      this.worker = new Worker("/assets/mesh-worker.js", { type: "module" });
      this.worker.onmessage = (ev) => this.onWorkerMessage(ev.data);
    } catch {
      this.cbs.onStatus?.("3D worker unavailable");
    }

    this.resizeObs = new ResizeObserver(() => this.scheduleDraw());
    this.resizeObs.observe(this.canvas);

    this.canvas.addEventListener("pointerdown", this.onPointerDown);
    this.canvas.addEventListener("pointermove", this.onPointerMove);
    this.canvas.addEventListener("pointerup", this.onPointerUp);
    this.canvas.addEventListener("pointercancel", this.onPointerUp);
    this.canvas.addEventListener("wheel", this.onWheel, { passive: false });
    this.canvas.addEventListener("dblclick", this.onDblClick);
    this.canvas.addEventListener("contextmenu", this.onContextMenu);
  }

  /** Cached mesh key+id for `viewer.watch {kept}` to skip re-sending unchanged boards. */
  keptToken(): string {
    return this.activeKey && this.lastMeshId ? `${this.activeKey}\t${this.lastMeshId}` : "";
  }

  setTheme(light: boolean): void {
    if (this.lightTheme !== light) {
      this.lightTheme = light;
      this.scheduleDraw();
    }
  }

  /** Handle a decoded binary CBOR solid/parts/part frame. */
  handleFrame(map: Record<string, CborValue>): void {
    const key = typeof map.key === "string" ? map.key : "";
    if (!key) return;

    // 1. Bare/full solid mesh frame
    if (map.mesh instanceof Uint8Array && typeof map.n === "number" && !map.part) {
      this.activeKey = key;
      this.lastMeshId = fnv1a64Hex(map.mesh);
      this.clearParts();
      this.cbs.onStatus?.("Building 3D model…");
      const meshCopy = new Uint8Array(map.mesh);
      const facesCopy = map.faces instanceof Uint8Array ? new Uint8Array(map.faces) : undefined;
      const transfers: Transferable[] = [meshCopy.buffer];
      if (facesCopy) transfers.push(facesCopy.buffer);
      this.worker?.postMessage(
        { id: this.nextJobId++, kind: "solid", key, n: map.n, mesh: meshCopy, faces: facesCopy },
        transfers
      );
      return;
    }

    // 2. Parts reset frame: `{parts: {of, rev}}`
    if (map.parts && typeof map.parts === "object" && !Array.isArray(map.parts) && !(map.parts instanceof Uint8Array)) {
      if (this.activeKey && key !== this.activeKey) return;
      this.activeKey = key;
      this.clearParts();
      this.partsTotal = typeof map.parts.of === "number" ? map.parts.of : 0;
      this.partsDone = 0;
      if (this.partsTotal > 0) {
        this.cbs.onStatus?.(`Loading parts 0/${this.partsTotal}…`);
      }
      this.scheduleDraw();
      return;
    }

    // 3. Streaming part frame: `{part, n, mesh, at, refs, done}`
    if (typeof map.part === "string" && map.mesh instanceof Uint8Array && map.at instanceof Uint8Array && typeof map.n === "number") {
      if (this.activeKey && key !== this.activeKey) return;
      const meshCopy = new Uint8Array(map.mesh);
      const atCopy = new Uint8Array(map.at);
      const facesCopy = map.faces instanceof Uint8Array ? new Uint8Array(map.faces) : undefined;
      const transfers: Transferable[] = [meshCopy.buffer, atCopy.buffer];
      if (facesCopy) transfers.push(facesCopy.buffer);
      this.worker?.postMessage(
        {
          id: this.nextJobId++,
          kind: "part",
          key,
          partId: map.part,
          n: map.n,
          mesh: meshCopy,
          at: atCopy,
          faces: facesCopy,
          refs: Array.isArray(map.refs) ? map.refs : [],
          done: typeof map.done === "number" ? map.done : 0,
        },
        transfers
      );
    }
  }

  private onWorkerMessage(msg: any): void {
    if (msg.error) {
      this.cbs.onStatus?.(msg.error);
      return;
    }
    if (msg.key !== this.activeKey || !this.gl) return;

    if (msg.kind === "solid") {
      this.deleteBatch(this.boardBatch);
      this.boardBatch = this.uploadBatch(msg.positions, msg.normals, msg.colors, 0);
      this.target = msg.center;
      this.radius = msg.radius;
      this.fit();
      this.cbs.onStatus?.("");
      this.scheduleDraw();
    } else if (msg.kind === "part") {
      const batch = this.uploadBatch(msg.positions, msg.normals, msg.colors, performance.now());
      if (batch) this.partBatches.push(batch);
      this.partsDone = msg.done || this.partsDone + 1;
      if (this.partsTotal > 0 && this.partsDone < this.partsTotal) {
        this.cbs.onStatus?.(`Loading parts ${this.partsDone}/${this.partsTotal}…`, true);
      } else {
        this.cbs.onStatus?.("", true);
      }
      this.scheduleDraw();
    }
  }

  private createSubBatch(
    positions: Float32Array,
    normals: Float32Array,
    colFloats: Float32Array,
    vertCount: number
  ): GpuSubBatch | null {
    const gl = this.gl;
    if (!gl || vertCount === 0) return null;
    const vao = gl.createVertexArray();
    const posBuf = gl.createBuffer();
    const nrmBuf = gl.createBuffer();
    const colBuf = gl.createBuffer();
    if (!vao || !posBuf || !nrmBuf || !colBuf) return null;

    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);

    gl.bindBuffer(gl.ARRAY_BUFFER, nrmBuf);
    gl.bufferData(gl.ARRAY_BUFFER, normals, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);

    gl.bindBuffer(gl.ARRAY_BUFFER, colBuf);
    gl.bufferData(gl.ARRAY_BUFFER, colFloats, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 0, 0);

    gl.bindVertexArray(null);
    return { vao, posBuf, nrmBuf, colBuf, vertCount };
  }

  /** Split triangles into opaque (TT == 0) and translucent (TT > 0) sub-batches for two-pass rendering. */
  private uploadBatch(
    positions: Float32Array,
    normals: Float32Array,
    triColors: Uint32Array,
    fadeStartMs: number
  ): GpuBatch | null {
    if (!this.gl) return null;
    const triCount = triColors.length;
    let opqTris = 0;
    let trnTris = 0;
    for (let t = 0; t < triCount; t++) {
      if ((triColors[t] >>> 24) === 0) opqTris++;
      else trnTris++;
    }

    const opqPos = opqTris > 0 ? new Float32Array(opqTris * 9) : null;
    const opqNrm = opqTris > 0 ? new Float32Array(opqTris * 9) : null;
    const opqCol = opqTris > 0 ? new Float32Array(opqTris * 12) : null;

    const trnPos = trnTris > 0 ? new Float32Array(trnTris * 9) : null;
    const trnNrm = trnTris > 0 ? new Float32Array(trnTris * 9) : null;
    const trnCol = trnTris > 0 ? new Float32Array(trnTris * 12) : null;

    let oi = 0;
    let ti = 0;
    for (let t = 0; t < triCount; t++) {
      const c = triColors[t];
      const clear = (c >>> 24) & 0xff;
      const a = 1 - clear / 255;
      const r = ((c >>> 16) & 0xff) / 255;
      const g = ((c >>> 8) & 0xff) / 255;
      const b = (c & 0xff) / 255;
      const src9 = t * 9;

      if (clear === 0 && opqPos && opqNrm && opqCol) {
        const dst9 = oi * 9;
        for (let j = 0; j < 9; j++) {
          opqPos[dst9 + j] = positions[src9 + j];
          opqNrm[dst9 + j] = normals[src9 + j];
        }
        const dst12 = oi * 12;
        for (let k = 0; k < 3; k++) {
          opqCol[dst12 + k * 4] = r;
          opqCol[dst12 + k * 4 + 1] = g;
          opqCol[dst12 + k * 4 + 2] = b;
          opqCol[dst12 + k * 4 + 3] = 1;
        }
        oi++;
      } else if (trnPos && trnNrm && trnCol) {
        const dst9 = ti * 9;
        for (let j = 0; j < 9; j++) {
          trnPos[dst9 + j] = positions[src9 + j];
          trnNrm[dst9 + j] = normals[src9 + j];
        }
        const dst12 = ti * 12;
        for (let k = 0; k < 3; k++) {
          trnCol[dst12 + k * 4] = r;
          trnCol[dst12 + k * 4 + 1] = g;
          trnCol[dst12 + k * 4 + 2] = b;
          trnCol[dst12 + k * 4 + 3] = a;
        }
        ti++;
      }
    }

    const opaque = opqPos && opqNrm && opqCol ? this.createSubBatch(opqPos, opqNrm, opqCol, opqTris * 3) : null;
    const translucent = trnPos && trnNrm && trnCol ? this.createSubBatch(trnPos, trnNrm, trnCol, trnTris * 3) : null;
    if (!opaque && !translucent) return null;
    return { opaque, translucent, fadeStartMs };
  }

  private deleteSubBatch(sb: GpuSubBatch | null): void {
    const gl = this.gl;
    if (!gl || !sb) return;
    gl.deleteVertexArray(sb.vao);
    gl.deleteBuffer(sb.posBuf);
    gl.deleteBuffer(sb.nrmBuf);
    gl.deleteBuffer(sb.colBuf);
  }

  private deleteBatch(b: GpuBatch | null): void {
    if (!b) return;
    this.deleteSubBatch(b.opaque);
    this.deleteSubBatch(b.translucent);
  }

  private clearParts(): void {
    for (const b of this.partBatches) this.deleteBatch(b);
    this.partBatches = [];
  }

  fit(): void {
    const fov = (35 * Math.PI) / 180;
    this.dist = (this.radius / Math.sin(fov * 0.5)) * 1.15;
    this.yaw = 0.55;
    this.pitch = 0.65;
    this.scheduleDraw();
  }

  private scheduleDraw(): void {
    if (this.dirty) return;
    this.dirty = true;
    this.rafId = requestAnimationFrame((now) => {
      this.dirty = false;
      this.rafId = 0;
      const animating = this.draw(now);
      // Only keep rAF alive while a streaming part fade-in is in flight (zero idle repaint loop!)
      if (animating) this.scheduleDraw();
    });
  }

  private drawSubBatch(sb: GpuSubBatch | null): void {
    if (!sb || !this.gl) return;
    this.gl.bindVertexArray(sb.vao);
    this.gl.drawArrays(this.gl.TRIANGLES, 0, sb.vertCount);
  }

  private draw(now: number): boolean {
    const gl = this.gl;
    if (!gl || !this.prog) return false;

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

    gl.viewport(0, 0, pw, ph);
    gl.depthMask(true);
    gl.clear(gl.DEPTH_BUFFER_BIT);

    // Pass 0: vertical linear-light Backdrop gradient matching native/gpui/src/viewers/raster.rs
    if (this.bgProg && this.bgVao) {
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.BLEND);
      gl.depthMask(false);
      gl.useProgram(this.bgProg);
      const top = this.lightTheme ? BG_LIGHT_TOP : BG_DARK_TOP;
      const bot = this.lightTheme ? BG_LIGHT_BOT : BG_DARK_BOT;
      gl.uniform3f(this.uBgTop, top[0], top[1], top[2]);
      gl.uniform3f(this.uBgBot, bot[0], bot[1], bot[2]);
      gl.bindVertexArray(this.bgVao);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.bindVertexArray(null);
    }

    if (!this.boardBatch && this.partBatches.length === 0) return false;

    const cp = Math.cos(this.pitch);
    const sp = Math.sin(this.pitch);
    const cy = Math.cos(this.yaw);
    const sy = Math.sin(this.yaw);
    const eye: [number, number, number] = [
      this.target[0] + this.dist * cp * sy,
      this.target[1] - this.dist * cp * cy,
      this.target[2] + this.dist * sp,
    ];
    const fwd = normalize3([
      this.target[0] - eye[0],
      this.target[1] - eye[1],
      this.target[2] - eye[2],
    ]);
    const worldUp: [number, number, number] = Math.abs(fwd[2]) > 0.99 ? [0, 1, 0] : [0, 0, 1];
    const right = normalize3(cross3(fwd, worldUp));
    const up = cross3(right, fwd);

    const view = new Float32Array([
      right[0], up[0], -fwd[0], 0,
      right[1], up[1], -fwd[1], 0,
      right[2], up[2], -fwd[2], 0,
      -dot3(right, eye), -dot3(up, eye), dot3(fwd, eye), 1,
    ]);
    const near = Math.max(0.05, this.dist * 0.01);
    const far = Math.max(this.dist * 10, this.radius * 20);
    const proj = perspective((35 * Math.PI) / 180, pw / ph, near, far);
    const mvp = mul4(proj, view);

    gl.useProgram(this.prog);
    gl.uniformMatrix4fv(this.uMvp, false, mvp);
    gl.uniform3f(this.uEye, eye[0], eye[1], eye[2]);
    gl.uniform3f(this.uRight, right[0], right[1], right[2]);
    gl.uniform3f(this.uUp, up[0], up[1], up[2]);
    gl.uniform3f(this.uFwd, fwd[0], fwd[1], fwd[2]);

    let stillFading = false;
    const settledParts: GpuBatch[] = [];
    const fadingParts: Array<{ batch: GpuBatch; fade: number }> = [];
    for (const batch of this.partBatches) {
      const elapsed = now - batch.fadeStartMs;
      const fade = batch.fadeStartMs === 0 ? 1 : Math.min(1, Math.max(0, elapsed / 300));
      if (fade < 1) {
        stillFading = true;
        if (fade > 0) fadingParts.push({ batch, fade });
      } else {
        settledParts.push(batch);
      }
    }

    // Pass 1: Opaque geometry (depth write ON, blending OFF)
    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.uniform1f(this.uFade, 1.0);
    if (this.boardBatch) this.drawSubBatch(this.boardBatch.opaque);
    for (const b of settledParts) this.drawSubBatch(b.opaque);

    // Pass 2: Translucent geometry (TT > 0, e.g. soldermask & courtyard outlines; depth write OFF, blending ON)
    gl.depthMask(false);
    gl.enable(gl.BLEND);
    gl.uniform1f(this.uFade, 1.0);
    if (this.boardBatch) this.drawSubBatch(this.boardBatch.translucent);
    for (const b of settledParts) this.drawSubBatch(b.translucent);

    // Pass 3: Streaming parts currently fading in over 300 ms
    for (const { batch, fade } of fadingParts) {
      gl.uniform1f(this.uFade, fade);
      this.drawSubBatch(batch.opaque);
      this.drawSubBatch(batch.translucent);
    }

    gl.depthMask(true);
    gl.bindVertexArray(null);
    return stillFading;
  }

  private onPointerDown = (ev: PointerEvent): void => {
    this.canvas.setPointerCapture(ev.pointerId);
    this.pointers.set(ev.pointerId, {
      x: ev.clientX,
      y: ev.clientY,
      button: ev.button,
      shift: ev.shiftKey,
    });
    if (this.pointers.size === 2) {
      const [a, b] = Array.from(this.pointers.values());
      this.pinchDist = Math.hypot(b.x - a.x, b.y - a.y) || 1;
    }
  };

  private onPointerMove = (ev: PointerEvent): void => {
    const prev = this.pointers.get(ev.pointerId);
    if (!prev) return;
    const dx = ev.clientX - prev.x;
    const dy = ev.clientY - prev.y;
    prev.x = ev.clientX;
    prev.y = ev.clientY;

    if (this.pointers.size === 2) {
      const [a, b] = Array.from(this.pointers.values());
      const dist = Math.hypot(b.x - a.x, b.y - a.y) || 1;
      if (this.pinchDist > 0) {
        this.dist = Math.max(this.radius * 0.05, Math.min(this.radius * 20, this.dist * (this.pinchDist / dist)));
      }
      this.pinchDist = dist;
      this.cbs.onUserMove?.();
      this.scheduleDraw();
      return;
    }

    if (prev.button === 2 || prev.button === 1 || prev.shift) {
      // Pan
      const panScale = this.dist * 0.0014;
      const cy = Math.cos(this.yaw);
      const sy = Math.sin(this.yaw);
      this.target[0] -= dx * cy * panScale;
      this.target[1] -= dx * sy * panScale;
      this.target[2] += dy * panScale;
    } else {
      // Orbit
      this.yaw += dx * 0.008;
      this.pitch = Math.max(-Math.PI * 0.49, Math.min(Math.PI * 0.49, this.pitch + dy * 0.008));
    }
    this.cbs.onUserMove?.();
    this.scheduleDraw();
  };

  private onPointerUp = (ev: PointerEvent): void => {
    this.pointers.delete(ev.pointerId);
    if (this.pointers.size < 2) this.pinchDist = 0;
  };

  private onWheel = (ev: WheelEvent): void => {
    ev.preventDefault();
    const factor = ev.deltaY < 0 ? 1 / 1.16 : 1.16;
    this.dist = Math.max(this.radius * 0.05, Math.min(this.radius * 20, this.dist * factor));
    this.cbs.onUserMove?.();
    this.scheduleDraw();
  };

  private onDblClick = (): void => {
    this.fit();
  };

  private onContextMenu = (ev: MouseEvent): void => {
    ev.preventDefault();
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
    this.canvas.removeEventListener("contextmenu", this.onContextMenu);
    this.deleteBatch(this.boardBatch);
    this.clearParts();
    if (this.gl) {
      if (this.bgVao) this.gl.deleteVertexArray(this.bgVao);
      if (this.bgProg) this.gl.deleteProgram(this.bgProg);
      if (this.prog) this.gl.deleteProgram(this.prog);
    }
    this.worker?.terminate();
    this.canvas.remove();
  }
}
