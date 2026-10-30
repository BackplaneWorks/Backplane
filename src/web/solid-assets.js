// Mesh transport/decoding only. The renderer owns the camera and presentation.
// Work is sliced so a large saved revision cannot block pointer/key events for
// an entire decode. A generation owns each result, including errors/previews.
const MiB = 1024 * 1024;
const abortError = () => new DOMException('Model superseded', 'AbortError');
const yieldTask = () => new Promise(resolve => setTimeout(resolve, 0));

function validate(o, maxBytes) {
  if (!(o.mesh instanceof Uint8Array)) throw new Error('Invalid model bytes');
  if (!Number.isSafeInteger(o.n) || o.n < 1 || o.n * 108 > maxBytes) throw new Error('Model exceeds the viewer memory limit or has no triangles');
  // Every triangle contains ten varints, each at least one byte.
  if (o.mesh.length < o.n * 10) throw new Error('Incomplete model');
}

// Same n + exact wire bytes means the same revision. Source names alone never
// identify an asset; nor do short hashes, which can collide. No backend revision
// field is required, so older hubs retain the same correctness guarantee.
function sameWire(a, b) {
  if (a.n !== b.n || a.mesh.length !== b.mesh.length) return false;
  if (a.mesh === b.mesh) return true;
  for (let i = 0; i < a.mesh.length; i++) if (a.mesh[i] !== b.mesh[i]) return false;
  return true;
}

export async function decodeMesh(o, {signal, onProgress = () => {}, now = () => performance.now(), schedule = yieldTask, maxBytes = 128 * MiB, sliceTriangles = 2048, sliceMs = 6} = {}) {
  validate(o, maxBytes);
  const check = () => { if (signal?.aborted) throw abortError(); };
  check();
  // Let the received/decoding state paint before allocating and filling arrays.
  await schedule();check();
  const pos = new Float32Array(o.n * 9), nrm = new Float32Array(o.n * 9), col = new Float32Array(o.n * 9);
  const bounds = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
  const bytes = o.mesh;
  let offset = 0, color = 0, px = 0, py = 0, pz = 0, triangles = 0;
  const next = () => {
    let acc = 0;
    for (let shift = 0; shift <= 28; shift += 7) {
      if (offset >= bytes.length) throw new Error('Incomplete model');
      const v = bytes[offset++];acc += (v & 127) * 2 ** shift;
      if (v < 128) {
        if (acc > 0xffffffff) throw new Error('Invalid model integer');
        return acc;
      }
    }
    throw new Error('Invalid model integer');
  };
  const signed = () => { const z = next();return z % 2 ? -(z + 1) / 2 : z / 2; };
  const p = new Float32Array(9);
  const model = () => ({pos: pos.subarray(0, triangles * 9), nrm: nrm.subarray(0, triangles * 9), col: col.subarray(0, triangles * 9), count: triangles * 3, box: bounds.slice()});
  let lastProgress = -Infinity;
  while (triangles < o.n) {
    check();
    const begin = now(), stop = Math.min(o.n, triangles + Math.max(1, sliceTriangles));
    while (triangles < stop) {
      color = (color ^ next()) >>> 0;
      for (let k = 0; k < 3; k++) {
        px += signed();py += signed();pz += signed();
        if (![px, py, pz].every(Number.isSafeInteger)) throw new Error('Invalid model coordinate');
        // The wire is x right, y down, z up; WebGL is right handed.
        p[k * 3] = px;p[k * 3 + 1] = -py;p[k * 3 + 2] = pz;
      }
      const ux = p[3] - p[0], uy = p[4] - p[1], uz = p[5] - p[2];
      const vx = p[6] - p[0], vy = p[7] - p[1], vz = p[8] - p[2];
      const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      const length = Math.hypot(nx, ny, nz) || 1;
      const r = ((color >> 16) & 255) / 255, g = ((color >> 8) & 255) / 255, b = (color & 255) / 255;
      const w = triangles * 9;
      for (let k = 0; k < 9; k += 3) {
        pos.set(p.subarray(k, k + 3), w + k);
        nrm[w + k] = nx / length;nrm[w + k + 1] = ny / length;nrm[w + k + 2] = nz / length;
        col[w + k] = r;col[w + k + 1] = g;col[w + k + 2] = b;
        for (let a = 0; a < 3; a++) {
          bounds[a] = Math.min(bounds[a], p[k + a]);bounds[a + 3] = Math.max(bounds[a + 3], p[k + a]);
        }
      }
      triangles++;
      // Clock reads every 64 triangles bound the longest slice without making
      // clock overhead proportional to every vertex.
      if (triangles % 64 === 0 && now() - begin >= sliceMs) break;
    }
    const at = now();
    if (at - lastProgress >= 32 || triangles === o.n) {
      check();onProgress({completed: triangles, total: o.n, model: model()});lastProgress = at;
    }
    if (triangles < o.n) { await schedule();check(); }
  }
  if (offset !== bytes.length) throw new Error('Unexpected bytes after model');
  return model();
}

function modelBytes(m) { return m ? m.pos.buffer.byteLength + m.nrm.buffer.byteLength + m.col.buffer.byteLength : 0; }

function fullBox(box) {
  if (!Array.isArray(box) || box.length !== 6 || !box.every(Number.isFinite)) return undefined;
  const b = box.map(x => x | 0);
  return [b[0], -b[4], b[2], b[3], -b[1], b[5]];
}

export class SolidAssets {
  constructor({onChange = () => {}, decode = decodeMesh, now = () => performance.now(), maxBytes = 128 * MiB, maxEntries = 8, maxStates = 64} = {}) {
    this.onChange = onChange;this.decode = decode;this.now = now;
    this.maxBytes = maxBytes;this.maxEntries = Math.max(1, maxEntries);this.maxStates = maxStates;
    this.entries = new Map();this.states = new Map();this.serial = 0;this.pinned = '';
  }
  get(key) {
    const entry = this.entries.get(key);
    if (entry) { this.entries.delete(key);this.entries.set(key, entry); }
    return entry;
  }
  reuse(key) {
    const entry = this.get(key);
    if (entry?.status === 'ready') { entry.cached = true;entry.stats.cached = true; }
    return entry;
  }
  pin(key) { this.pinned = key || '';this.prune(); }
  saveState(key, value) {
    if (!key) return;
    this.states.delete(key);this.states.set(key, structuredClone(value));
    while (this.states.size > this.maxStates) this.states.delete(this.states.keys().next().value);
  }
  getState(key) { const value = this.states.get(key);return value === undefined ? undefined : structuredClone(value); }
  prune() {
    let bytes = 0;
    const size = entry => (entry.wire?.mesh.byteLength || 0) + modelBytes(entry.model) + (entry.previous !== entry.model ? modelBytes(entry.previous) : 0);
    for (const entry of this.entries.values()) bytes += size(entry);
    for (const [key, entry] of this.entries) {
      if (bytes <= this.maxBytes && this.entries.size <= this.maxEntries) break;
      if (key === this.pinned) continue;
      entry.controller?.abort();this.entries.delete(key);
      bytes -= size(entry);
    }
  }
  reset() {
    for (const entry of this.entries.values()) entry.controller?.abort();
    this.entries.clear();this.states.clear();this.serial++;this.pinned = '';
  }
  receive(o, at = {bytes: 0, t: this.now()}) {
    if (!o || typeof o.key !== 'string') return Promise.resolve(undefined);
    if (!o.mesh && typeof o.none !== 'string') return Promise.resolve(this.get(o.key));
    const old = this.get(o.key);
    // Failures are retryable: receiving the same revision again starts fresh.
    if (o.mesh && old?.wire && old.status !== 'failed' && sameWire(old.wire, o)) {
      if (old.status === 'ready') {
        old.cached = true;old.stats.cached = true;this.onChange(old);
      }
      return old.promise || Promise.resolve(old);
    }
    old?.controller?.abort();
    const previous = old?.status === 'ready' ? old.model : old?.previous;
    const entry = {key: o.key, revision: ++this.serial, box: fullBox(o.box3), status: 'decoding', model: previous,
      previous, cached: false, stale: !!previous,
      progress: {completed: 0, total: o.n || 0}, error: '', stats: {bytes: at.bytes || o.mesh?.byteLength || 0, build: 0, firstGeometry: null, cached: false},
      controller: new AbortController(), wire: o.mesh ? {n: o.n, mesh: o.mesh} : null};
    this.entries.set(o.key, entry);
    const current = () => this.entries.get(o.key) === entry && !entry.controller.signal.aborted;
    const emit = () => { if (current()) this.onChange(entry); };
    if (typeof o.none === 'string') {
      entry.status = 'failed';entry.error = o.none || 'Model unavailable';emit();this.prune();return Promise.resolve(entry);
    }
    const started = this.now();emit();
    entry.promise = (async () => {
      try {
        const m = await this.decode(o, {signal: entry.controller.signal, maxBytes: this.maxBytes,
          onProgress: progress => {
            if (!current()) return;
            entry.model = progress.model;entry.stale = false;
            entry.progress = {completed: progress.completed, total: progress.total};
            if (entry.stats.firstGeometry === null) entry.stats.firstGeometry = this.now() - started;
            emit();
          }});
        if (!current()) return undefined;
        entry.model = m;entry.previous = undefined;entry.stale = false;entry.status = 'ready';
        entry.progress = {completed: o.n, total: o.n};entry.stats.build = this.now() - started;
        if (entry.stats.firstGeometry === null) entry.stats.firstGeometry = entry.stats.build;
        emit();this.prune();return entry;
      } catch (error) {
        if (!current()) return undefined;
        // Do not promote a truncated prefix to a cached successful revision.
        entry.model = entry.previous;entry.stale = !!entry.previous;entry.status = 'failed';entry.error = String(error.message || error);
        entry.stats.build = this.now() - started;emit();this.prune();return entry;
      }
    })();
    this.prune();
    return entry.promise;
  }
}
