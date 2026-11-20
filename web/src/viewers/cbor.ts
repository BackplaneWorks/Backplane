// Minimal CBOR subset + LEB128/zigzag varint decoder for Backplane data/1 binary plot frames.
// Mirrors native/gpui/src/screens/plot.rs and native/host/src/kicad/plot.rs.

export type CborValue =
  | number
  | string
  | Uint8Array
  | CborValue[]
  | Record<string, CborValue>;

const MAX_NODES = 400_000;
const textDecoder = new TextDecoder();

/** Fast check: is this binary WebSocket frame a CBOR plot frame ({1:"plot", 31:key, ...})? */
export function isPlotFrame(b: Uint8Array): boolean {
  return (
    b.length > 7 &&
    b[0] >= 0xa0 &&
    b[0] <= 0xb7 &&
    b[1] === 1 &&
    b[2] === 0x64 &&
    b[3] === 0x70 && // 'p'
    b[4] === 0x6c && // 'l'
    b[5] === 0x6f && // 'o'
    b[6] === 0x74 // 't'
  );
}

/** Decode one CBOR item in the hub's plot dialect (major 0..5, map keys 1=>"t", 31=>"key"). */
export function decodeCbor(b: Uint8Array): CborValue | null {
  let i = 0;
  let nodes = 0;

  function readArg(ai: number): number | null {
    if (ai <= 23) return ai;
    if (ai === 24) {
      if (i >= b.length) return null;
      return b[i++];
    }
    if (ai === 25) {
      if (i + 2 > b.length) return null;
      const v = (b[i] << 8) | b[i + 1];
      i += 2;
      return v;
    }
    if (ai === 26) {
      if (i + 4 > b.length) return null;
      const v =
        ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
      i += 4;
      return v;
    }
    return null;
  }

  function readItem(depth: number): CborValue | null {
    nodes++;
    if (depth > 32 || nodes > MAX_NODES || i >= b.length) return null;
    const h = b[i++];
    const major = h >> 5;
    const n = readArg(h & 31);
    if (n === null) return null;

    switch (major) {
      case 0:
        return n;
      case 1:
        return -1 - n;
      case 2: {
        if (i + n > b.length) return null;
        const slice = b.subarray(i, i + n);
        i += n;
        return slice;
      }
      case 3: {
        if (i + n > b.length) return null;
        const s = textDecoder.decode(b.subarray(i, i + n));
        i += n;
        return s;
      }
      case 4: {
        if (n > b.length) return null;
        const arr: CborValue[] = new Array(n);
        for (let k = 0; k < n; k++) {
          const item = readItem(depth + 1);
          if (item === null) return null;
          arr[k] = item;
        }
        return arr;
      }
      case 5: {
        const map: Record<string, CborValue> = Object.create(null);
        for (let k = 0; k < n; k++) {
          const rawKey = readItem(depth + 1);
          if (rawKey === null) return null;
          let key: string;
          if (typeof rawKey === "string") key = rawKey;
          else if (typeof rawKey === "number") {
            key = rawKey === 1 ? "t" : rawKey === 31 ? "key" : String(rawKey);
          } else {
            return null;
          }
          const val = readItem(depth + 1);
          if (val === null) return null;
          map[key] = val;
        }
        return map;
      }
      default:
        return null;
    }
  }

  const v = readItem(0);
  return v !== null && i === b.length ? v : null;
}

export class VarintReader {
  i = 0;
  ok = true;
  constructor(private readonly b: Uint8Array) {}
  more(): boolean {
    return this.ok && this.i < this.b.length;
  }
  nextU32(): number {
    let acc = 0;
    for (let k = 0; k < 5; k++) {
      if (this.i >= this.b.length) {
        this.ok = false;
        return 0;
      }
      const v = this.b[this.i++];
      const shift = 7 * k;
      if (shift >= 28 && (v & 0xf0) !== 0) {
        this.ok = false;
        return 0;
      }
      acc = (acc | ((v & 0x7f) << shift)) >>> 0;
      if (v < 128) return acc;
    }
    this.ok = false;
    return 0;
  }
  nextI32(): number {
    const z = this.nextU32();
    return (z >>> 1) ^ -(z & 1);
  }
}

/** FNV-1a 64-bit hex digest matching native/host/src/kicad/plot.rs `mesh_id`. */
export function fnv1a64Hex(b: Uint8Array): string {
  let h = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (let i = 0; i < b.length; i++) {
    h = ((h ^ BigInt(b[i])) * prime) & mask;
  }
  return h.toString(16).padStart(16, "0");
}

export interface PlotPiece {
  info: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  /** Offset into chunk.pts (in floats, x/y pairs). */
  at: number;
  /** Point count. */
  n: number;
  /** 0 = fill polygon, 1 = stroke polyline, 2 = dot */
  kind: 0 | 1 | 2;
}

export interface PlotChunk {
  layer: number;
  color: number;
  alpha: number;
  mode: number;
  /** Radius in micrometres (width / 2). */
  r: number;
  /** True when mode==0 and w!=0 (comparison tint / own-color fill). */
  own: boolean;
  /** Flat x, y pairs in micrometres. */
  pts: Float32Array;
  pieces: PlotPiece[];
}

export interface Plot2DFrame {
  key: string;
  fam: string;
  base: string;
  box: [number, number, number, number];
  edge: [number, number, number, number];
  thick: number;
  note: string;
  warn: boolean;
  chunks: PlotChunk[];
}

export function decodePlotChunk(o: Record<string, CborValue>): PlotChunk {
  const num = (k: string, d: number): number =>
    typeof o[k] === "number" ? (o[k] as number) : d;
  const layer = num("l", 0);
  const color = num("c", 0);
  const alpha = num("a", 255) / 255;
  const mode = num("m", 1);
  const w = num("w", 0);
  const r = w / 2;
  const own = mode === 0 && w !== 0;
  const data = o.d instanceof Uint8Array ? o.d : new Uint8Array(0);

  const vr = new VarintReader(data);
  const ptsBuf: number[] = [];
  const pieces: PlotPiece[] = [];
  let px = 0;
  let py = 0;

  while (vr.more()) {
    const info = vr.nextU32();
    const startAt = ptsBuf.length;
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;

    if (mode === 2) {
      px = (px + vr.nextI32()) | 0;
      py = (py + vr.nextI32()) | 0;
      ptsBuf.push(px, py);
      x0 = x1 = px;
      y0 = y1 = py;
    } else {
      const n = Math.min(vr.nextU32(), 1 << 20);
      for (let k = 0; k < n && vr.more(); k++) {
        px = (px + vr.nextI32()) | 0;
        py = (py + vr.nextI32()) | 0;
        ptsBuf.push(px, py);
        if (px < x0) x0 = px;
        if (px > x1) x1 = px;
        if (py < y0) y0 = py;
        if (py > y1) y1 = py;
      }
    }

    const ptCount = (ptsBuf.length - startAt) / 2;
    if (ptCount > 0) {
      const kind: 0 | 1 | 2 =
        mode === 2 || ptCount === 1 ? 2 : mode === 0 ? 0 : 1;
      // A fill's bbox is never padded by `w` (for comparison tints, `w` encodes colour).
      const pad = mode === 0 ? 0 : r;
      pieces.push({
        info,
        x0: x0 - pad,
        y0: y0 - pad,
        x1: x1 + pad,
        y1: y1 + pad,
        at: startAt,
        n: ptCount,
        kind,
      });
    }
  }

  return {
    layer,
    color,
    alpha,
    mode,
    r,
    own,
    pts: new Float32Array(ptsBuf),
    pieces,
  };
}

export function decodePlot2DFrame(
  map: Record<string, CborValue>,
  held: PlotChunk[]
): Plot2DFrame {
  const rect = (v: CborValue | undefined): [number, number, number, number] =>
    Array.isArray(v) && v.length >= 4
      ? [
          Number(v[0]) || 0,
          Number(v[1]) || 0,
          Number(v[2]) || 0,
          Number(v[3]) || 0,
        ]
      : [0, 0, 0, 0];

  const cs = Array.isArray(map.cs) ? map.cs : [];
  const chunks: PlotChunk[] = [];
  for (let i = 0; i < cs.length; i++) {
    const entry = cs[i];
    if (typeof entry === "number") {
      if (entry >= 0 && entry < held.length) chunks.push(held[entry]);
    } else if (entry && typeof entry === "object" && !Array.isArray(entry) && !(entry instanceof Uint8Array)) {
      chunks.push(decodePlotChunk(entry));
    }
  }

  return {
    key: typeof map.key === "string" ? map.key : "",
    fam: typeof map.fam === "string" ? map.fam : "",
    base: typeof map.base === "string" ? map.base : "",
    box: rect(map.box),
    edge: rect(map.edge),
    thick: typeof map.thick === "number" ? map.thick : 1600,
    note: typeof map.note === "string" ? map.note : "",
    warn: typeof map.warn === "number" ? map.warn !== 0 : false,
    chunks,
  };
}
