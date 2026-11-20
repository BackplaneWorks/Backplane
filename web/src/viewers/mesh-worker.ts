// Web Worker for decoding Backplane 3D solid & streaming part frames off the main thread.
// Mirrors native/gpui/src/viewers/solid.rs (decode_mesh_faces, decode_part_faces)
// and native/gpui/src/viewers/raster.rs (smooth_normals_in).

const MAX_TRIANGLES = Math.floor((64 << 20) / 84);
const CREASE_COS = 0.574;

class Varints {
  at = 0;
  valid = true;
  constructor(private readonly b: Uint8Array) {}
  more(): boolean {
    return this.at < this.b.length;
  }
  next(): number {
    let v = 0;
    let shift = 0;
    while (true) {
      if (this.at >= this.b.length) {
        this.valid = false;
        return 0;
      }
      const byte = this.b[this.at++];
      if (shift < 32) {
        v = (v | ((byte & 0x7f) << shift)) >>> 0;
      }
      if ((byte & 0x80) === 0) return v;
      shift += 7;
      if (shift > 35) {
        this.valid = false;
        return 0;
      }
    }
  }
  signed(): number {
    const u = this.next();
    return (u >>> 1) ^ -(u & 1);
  }
}

function decodeFaceIds(runs: Uint8Array | undefined, n: number): Int32Array | null {
  if (!runs || runs.length === 0) return null;
  const v = new Varints(runs);
  const out = new Int32Array(n);
  let pos = 0;
  let face = 0;
  while (v.more()) {
    const k = v.next();
    if (!v.valid || k === 0 || pos + k > n) return null;
    out.fill(face, pos, pos + k);
    pos += k;
    face++;
  }
  return pos === n ? out : null;
}

function computeSmoothNormals(
  positions: Float32Array,
  keys: Int32Array,
  n: number,
  faces: Int32Array | null
): Float32Array {
  const faceN = new Float32Array(n * 3);
  const ang = new Float32Array(n * 3);
  const vid = new Int32Array(n * 3);
  const keyMap = new Map<string, number>();
  let nv = 0;

  for (let t = 0; t < n; t++) {
    const base = t * 9;
    const ax = positions[base], ay = positions[base + 1], az = positions[base + 2];
    const bx = positions[base + 3], by = positions[base + 4], bz = positions[base + 5];
    const cx = positions[base + 6], cy = positions[base + 7], cz = positions[base + 8];

    const e1x = bx - ax, e1y = by - ay, e1z = bz - az;
    const e2x = cx - ax, e2y = cy - ay, e2z = cz - az;
    const nx = e1y * e2z - e1z * e2y;
    const ny = e1z * e2x - e1x * e2z;
    const nz = e1x * e2y - e1y * e2x;
    const lenSq = nx * nx + ny * ny + nz * nz;
    if (lenSq > 1e-24) {
      const inv = 1 / Math.sqrt(lenSq);
      faceN[t * 3] = nx * inv;
      faceN[t * 3 + 1] = ny * inv;
      faceN[t * 3 + 2] = nz * inv;
    }

    const px = [ax, bx, cx];
    const py = [ay, by, cy];
    const pz = [az, bz, cz];
    for (let k = 0; k < 3; k++) {
      const k1 = (k + 1) % 3;
      const k2 = (k + 2) % 3;
      const u1x = px[k1] - px[k], u1y = py[k1] - py[k], u1z = pz[k1] - pz[k];
      const u2x = px[k2] - px[k], u2y = py[k2] - py[k], u2z = pz[k2] - pz[k];
      const crx = u1y * u2z - u1z * u2y;
      const cry = u1z * u2x - u1x * u2z;
      const crz = u1x * u2y - u1y * u2x;
      const d = u1x * u2x + u1y * u2y + u1z * u2z;
      ang[t * 3 + k] = Math.atan2(Math.sqrt(crx * crx + cry * cry + crz * crz), d);

      const ki = (t * 3 + k) * 3;
      const keyStr = `${keys[ki]},${keys[ki + 1]},${keys[ki + 2]}`;
      let id = keyMap.get(keyStr);
      if (id === undefined) {
        id = nv++;
        keyMap.set(keyStr, id);
      }
      vid[t * 3 + k] = id;
    }
  }
  keyMap.clear();

  const start = new Uint32Array(nv + 1);
  for (let c = 0; c < n * 3; c++) start[vid[c] + 1]++;
  for (let i = 0; i < nv; i++) start[i + 1] += start[i];
  const fill = new Uint32Array(start);
  const around = new Uint32Array(n * 3);
  for (let c = 0; c < n * 3; c++) {
    const v = vid[c];
    around[fill[v]++] = c;
  }

  const out = new Float32Array(n * 9);
  for (let c = 0; c < n * 3; c++) {
    const t = (c / 3) | 0;
    const k = c % 3;
    const ownX = faceN[t * 3], ownY = faceN[t * 3 + 1], ownZ = faceN[t * 3 + 2];
    const v = vid[c];
    const lo = start[v];
    const hi = start[v + 1];
    let ax = 0, ay = 0, az = 0;
    if (hi - lo <= 64 && ownX * ownX + ownY * ownY + ownZ * ownZ > 0) {
      for (let j = lo; j < hi; j++) {
        const d = around[j];
        const dt = (d / 3) | 0;
        const fx = faceN[dt * 3], fy = faceN[dt * 3 + 1], fz = faceN[dt * 3 + 2];
        const blend = faces
          ? faces[dt] === faces[t]
          : fx * ownX + fy * ownY + fz * ownZ >= CREASE_COS;
        if (blend) {
          const w = ang[d];
          ax += fx * w;
          ay += fy * w;
          az += fz * w;
        }
      }
    }
    const accSq = ax * ax + ay * ay + az * az;
    const o = t * 9 + k * 3;
    if (accSq > 1e-20) {
      const inv = 1 / Math.sqrt(accSq);
      out[o] = ax * inv;
      out[o + 1] = ay * inv;
      out[o + 2] = az * inv;
    } else {
      out[o] = ownX;
      out[o + 1] = ownY;
      out[o + 2] = ownZ;
    }
  }
  return out;
}

function decodeSolid(n: number, bytes: Uint8Array, runs?: Uint8Array) {
  if (n === 0 || n > MAX_TRIANGLES || bytes.length < n * 10) {
    throw new Error("Invalid or oversized model");
  }
  const v = new Varints(bytes);
  let color = 0;
  let px = 0, py = 0, pz = 0;
  const positions = new Float32Array(n * 9);
  const colors = new Uint32Array(n);
  const keys = new Int32Array(n * 9);
  let loX = Infinity, loY = Infinity, loZ = Infinity;
  let hiX = -Infinity, hiY = -Infinity, hiZ = -Infinity;

  for (let t = 0; t < n; t++) {
    color = (color ^ v.next()) >>> 0;
    colors[t] = color;
    const base = t * 9;
    for (let k = 0; k < 3; k++) {
      px = (px + v.signed()) | 0;
      py = (py + v.signed()) | 0;
      pz = (pz + v.signed()) | 0;
      // 10 µm -> mm, right-handed (x, -y, z) matching solid.rs
      const x = px * 0.01;
      const y = -py * 0.01;
      const z = pz * 0.01;
      const o = base + k * 3;
      positions[o] = x;
      positions[o + 1] = y;
      positions[o + 2] = z;
      keys[o] = px;
      keys[o + 1] = py;
      keys[o + 2] = pz;
      if (x < loX) loX = x;
      if (y < loY) loY = y;
      if (z < loZ) loZ = z;
      if (x > hiX) hiX = x;
      if (y > hiY) hiY = y;
      if (z > hiZ) hiZ = z;
    }
    if (!v.valid) throw new Error("Incomplete model");
  }
  if (v.more() || !v.valid) throw new Error("Unexpected model bytes");

  const center: [number, number, number] = [
    (loX + hiX) * 0.5,
    (loY + hiY) * 0.5,
    (loZ + hiZ) * 0.5,
  ];
  const dx = hiX - loX, dy = hiY - loY, dz = hiZ - loZ;
  const radius = Math.max(Math.sqrt(dx * dx + dy * dy + dz * dz) * 0.5, 1e-3);
  const normals = computeSmoothNormals(positions, keys, n, decodeFaceIds(runs, n));
  return { positions, normals, colors, center, radius, triCount: n };
}

function decodePart(n: number, bytes: Uint8Array, atBytes: Uint8Array, runs?: Uint8Array) {
  if (n === 0 || n > MAX_TRIANGLES || bytes.length < n * 10) {
    throw new Error("Invalid or oversized part");
  }
  if (atBytes.length === 0 || atBytes.length % 48 !== 0) {
    throw new Error("Invalid part placement");
  }
  const instCount = atBytes.length / 48;
  if (n * instCount > MAX_TRIANGLES) throw new Error("Too many part triangles");

  const v = new Varints(bytes);
  let color = 0;
  let px = 0, py = 0, pz = 0;
  const raw = new Int32Array(n * 9);
  const rawCols = new Uint32Array(n);
  for (let t = 0; t < n; t++) {
    color = (color ^ v.next()) >>> 0;
    rawCols[t] = color;
    const base = t * 9;
    for (let k = 0; k < 3; k++) {
      px = (px + v.signed()) | 0;
      py = (py + v.signed()) | 0;
      pz = (pz + v.signed()) | 0;
      raw[base + k * 3] = px;
      raw[base + k * 3 + 1] = py;
      raw[base + k * 3 + 2] = pz;
    }
    if (!v.valid) throw new Error("Incomplete part");
  }
  if (v.more() || !v.valid) throw new Error("Unexpected part bytes");

  const total = n * instCount;
  const positions = new Float32Array(total * 9);
  const colors = new Uint32Array(total);
  const keys = new Int32Array(total * 9);
  const dv = new DataView(atBytes.buffer, atBytes.byteOffset, atBytes.byteLength);
  let outT = 0;

  for (let inst = 0; inst < instCount; inst++) {
    const off = inst * 48;
    const a0 = dv.getFloat32(off, true);
    const a1 = dv.getFloat32(off + 4, true);
    const a2 = dv.getFloat32(off + 8, true);
    const a3 = dv.getFloat32(off + 12, true);
    const a4 = dv.getFloat32(off + 16, true);
    const a5 = dv.getFloat32(off + 20, true);
    const a6 = dv.getFloat32(off + 24, true);
    const a7 = dv.getFloat32(off + 28, true);
    const a8 = dv.getFloat32(off + 32, true);
    const a9 = dv.getFloat32(off + 36, true);
    const a10 = dv.getFloat32(off + 40, true);
    const a11 = dv.getFloat32(off + 44, true);

    for (let t = 0; t < n; t++) {
      colors[outT] = rawCols[t];
      const rb = t * 9;
      const ob = outT * 9;
      for (let k = 0; k < 3; k++) {
        const x = raw[rb + k * 3];
        const y = raw[rb + k * 3 + 1];
        const z = raw[rb + k * 3 + 2];
        const wx = a0 * x + a1 * y + a2 * z + a3;
        const wy = a4 * x + a5 * y + a6 * z + a7;
        const wz = a8 * x + a9 * y + a10 * z + a11;
        keys[ob + k * 3] = Math.round(wx);
        keys[ob + k * 3 + 1] = Math.round(wy);
        keys[ob + k * 3 + 2] = Math.round(wz);
        positions[ob + k * 3] = wx * 0.01;
        positions[ob + k * 3 + 1] = -wy * 0.01;
        positions[ob + k * 3 + 2] = wz * 0.01;
      }
      outT++;
    }
  }

  const singleFaces = decodeFaceIds(runs, n);
  let allFaces: Int32Array | null = null;
  if (singleFaces) {
    allFaces = new Int32Array(total);
    let maxFace = 0;
    for (let i = 0; i < n; i++) if (singleFaces[i] > maxFace) maxFace = singleFaces[i];
    const stride = maxFace + 1;
    for (let inst = 0; inst < instCount; inst++) {
      const fBase = inst * stride;
      const tBase = inst * n;
      for (let t = 0; t < n; t++) allFaces[tBase + t] = fBase + singleFaces[t];
    }
  }

  const normals = computeSmoothNormals(positions, keys, total, allFaces);
  return { positions, normals, colors, triCount: total };
}

self.onmessage = (ev: MessageEvent) => {
  const msg = ev.data;
  try {
    if (msg.kind === "solid") {
      const res = decodeSolid(msg.n, msg.mesh, msg.faces);
      (self as unknown as Worker).postMessage(
        { id: msg.id, kind: "solid", key: msg.key, ...res },
        [res.positions.buffer, res.normals.buffer, res.colors.buffer]
      );
    } else if (msg.kind === "part") {
      const res = decodePart(msg.n, msg.mesh, msg.at, msg.faces);
      (self as unknown as Worker).postMessage(
        { id: msg.id, kind: "part", key: msg.key, partId: msg.partId, refs: msg.refs, done: msg.done, ...res },
        [res.positions.buffer, res.normals.buffer, res.colors.buffer]
      );
    }
  } catch (err) {
    (self as unknown as Worker).postMessage({
      id: msg.id,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};
