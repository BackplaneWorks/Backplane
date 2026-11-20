// Raw WebGL2 3D viewer for Backplane boards (+ streaming parts) and Mech STEP models.
// Mirrors native/gpui/src/viewers/solid.rs & solid_gpu.rs:
//   - Linear RGB colors (0xTTRRGGBB), sRGB gamma applied once in fragment shader
//   - Smooth vertex normals decoded off-thread by mesh-worker.ts
//   - 300 ms fade-in per streaming part frame; zero idle repaint loops!
//   - Free-orbit camera (35° FOV, left-drag orbit, right/middle/shift pan, wheel zoom, touch orbit+pinch)

import { fnv1a64Hex, type CborValue } from "./cbor.ts";

export interface Solid3DCallbacks {
  onStatus?: (msg: string, streaming?: boolean) => void;
  onUserMove?: () => void;
}

interface GpuBatch {
  vao: WebGLVertexArrayObject;
  posBuf: WebGLBuffer;
  nrmBuf: WebGLBuffer;
  colBuf: WebGLBuffer;
  vertCount: number;
  fadeStartMs: number; // 0 = board (always 1.0), >0 = streaming part fading in over 300ms
}

const VERT_SRC = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNrm;
layout(location = 2) in vec4 aCol;

uniform mat4 uMvp;
uniform mat3 uNrmMat;

out vec3 vNrm;
out vec4 vCol;

void main() {
  vNrm = normalize(uNrmMat * aNrm);
  vCol = aCol;
  gl_Position = uMvp * vec4(aPos, 1.0);
}
`;

const FRAG_SRC = `#version 300 es
precision highp float;
in vec3 vNrm;
in vec4 vCol;

uniform float uFade;
out vec4 outColor;

vec3 linearToSrgb(vec3 c) {
  vec3 lo = c * 12.92;
  vec3 hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
  return mix(lo, hi, step(vec3(0.0031308), c));
}

void main() {
  vec3 n = normalize(vNrm);
  if (!gl_FrontFacing) n = -n;

  // Studio three-point lighting in camera space matching Backplane's solid.wgsl
  vec3 keyDir = normalize(vec3(0.38, 0.52, 0.76));
  vec3 fillDir = normalize(vec3(-0.62, -0.25, 0.74));
  vec3 rimDir = normalize(vec3(0.10, -0.78, -0.62));

  float key = max(dot(n, keyDir), 0.0);
  float fill = max(dot(n, fillDir), 0.0);
  float rim = max(dot(n, rimDir), 0.0);

  vec3 viewDir = vec3(0.0, 0.0, 1.0);
  vec3 halfKey = normalize(keyDir + viewDir);
  float spec = pow(max(dot(n, halfKey), 0.0), 36.0) * 0.22;

  vec3 lit = vCol.rgb * (0.24 + 0.68 * key + 0.22 * fill + 0.10 * rim) + vec3(spec);
  float alpha = vCol.a * uFade;
  outColor = vec4(linearToSrgb(clamp(lit, 0.0, 1.0)), alpha);
}
`;

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

function linkProgram(gl: WebGL2RenderingContext): WebGLProgram {
  const vs = compileShader(gl, gl.VERTEX_SHADER, VERT_SRC);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, FRAG_SRC);
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
  private prog: WebGLProgram | null = null;
  private uMvp: WebGLUniformLocation | null = null;
  private uNrmMat: WebGLUniformLocation | null = null;
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
      this.prog = linkProgram(gl);
      this.uMvp = gl.getUniformLocation(this.prog, "uMvp");
      this.uNrmMat = gl.getUniformLocation(this.prog, "uNrmMat");
      this.uFade = gl.getUniformLocation(this.prog, "uFade");
      gl.enable(gl.DEPTH_TEST);
      gl.enable(gl.BLEND);
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

  private uploadBatch(
    positions: Float32Array,
    normals: Float32Array,
    triColors: Uint32Array,
    fadeStartMs: number
  ): GpuBatch | null {
    const gl = this.gl;
    if (!gl) return null;
    const vertCount = positions.length / 3;
    const triCount = triColors.length;
    const colFloats = new Float32Array(vertCount * 4);
    for (let t = 0; t < triCount; t++) {
      const c = triColors[t];
      // Wire color is 0xTTRRGGBB in linear light (TT = transparency, 0 = opaque)
      const a = 1 - ((c >>> 24) & 0xff) / 255;
      const r = ((c >>> 16) & 0xff) / 255;
      const g = ((c >>> 8) & 0xff) / 255;
      const b = (c & 0xff) / 255;
      for (let k = 0; k < 3; k++) {
        const o = (t * 3 + k) * 4;
        colFloats[o] = r;
        colFloats[o + 1] = g;
        colFloats[o + 2] = b;
        colFloats[o + 3] = a;
      }
    }

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
    return { vao, posBuf, nrmBuf, colBuf, vertCount, fadeStartMs };
  }

  private deleteBatch(b: GpuBatch | null): void {
    const gl = this.gl;
    if (!gl || !b) return;
    gl.deleteVertexArray(b.vao);
    gl.deleteBuffer(b.posBuf);
    gl.deleteBuffer(b.nrmBuf);
    gl.deleteBuffer(b.colBuf);
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
    if (this.lightTheme) {
      gl.clearColor(0.92, 0.93, 0.95, 1);
    } else {
      gl.clearColor(0.24, 0.27, 0.31, 1);
    }
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

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
    const nrmMat = new Float32Array([
      right[0], up[0], -fwd[0],
      right[1], up[1], -fwd[1],
      right[2], up[2], -fwd[2],
    ]);
    const near = Math.max(0.05, this.dist * 0.01);
    const far = Math.max(this.dist * 10, this.radius * 20);
    const proj = perspective((35 * Math.PI) / 180, pw / ph, near, far);
    const mvp = mul4(proj, view);

    gl.useProgram(this.prog);
    gl.uniformMatrix4fv(this.uMvp, false, mvp);
    gl.uniformMatrix3fv(this.uNrmMat, false, nrmMat);

    if (this.boardBatch) {
      gl.uniform1f(this.uFade, 1.0);
      gl.bindVertexArray(this.boardBatch.vao);
      gl.drawArrays(gl.TRIANGLES, 0, this.boardBatch.vertCount);
    }

    let stillFading = false;
    for (const batch of this.partBatches) {
      const elapsed = now - batch.fadeStartMs;
      const fade = batch.fadeStartMs === 0 ? 1 : Math.min(1, Math.max(0, elapsed / 300));
      if (fade < 1) stillFading = true;
      gl.uniform1f(this.uFade, fade);
      gl.bindVertexArray(batch.vao);
      gl.drawArrays(gl.TRIANGLES, 0, batch.vertCount);
    }
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
      const panScale = (this.dist * 0.0014);
      const cy = Math.cos(this.yaw);
      const sy = Math.sin(this.yaw);
      this.target[0] -= (dx * cy) * panScale;
      this.target[1] -= (dx * sy) * panScale;
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
    if (this.gl && this.prog) this.gl.deleteProgram(this.prog);
    this.worker?.terminate();
    this.canvas.remove();
  }
}
