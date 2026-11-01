// 3D models decoded off the page's main thread. A board's model or a STEP
// part is tens of MB of triangles; decoding it on the main thread (even in
// slices) held the page while the viewer loaded: clicks, typing and the
// view socket's frames all waited. Here one worker decodes every model the
// page receives and hands back its arrays (transferred, not copied), so the
// page stays free while a heavy view loads. Where a worker cannot start
// (no Worker, a CSP that forbids blob: workers) solid-assets.js's sliced
// decodeMesh runs on the main thread as before.
//
// The worker's code is meshDecode below, made into a blob: URL from its own
// source text, so it must reference nothing outside itself. It decodes
// exactly as solid-assets.js's decodeMesh does (the same wire, checks and
// errors; test/tools/mesh_worker_test.mjs compares the two).

// Decode a model's wire (n triangles, each a colour xor and three zigzag
// varint deltas per corner) into flat position/normal/colour arrays.
export function meshDecode(bytes, n) {
  const pos = new Float32Array(n * 9), nrm = new Float32Array(n * 9), col = new Float32Array(n * 9);
  const bounds = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
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
  while (triangles < n) {
    color = (color ^ next()) >>> 0;
    for (let k = 0; k < 3; k++) {
      px += signed();py += signed();pz += signed();
      if (!Number.isSafeInteger(px) || !Number.isSafeInteger(py) || !Number.isSafeInteger(pz)) throw new Error('Invalid model coordinate');
      p[k * 3] = px;p[k * 3 + 1] = -py;p[k * 3 + 2] = pz;
    }
    const ux = p[3] - p[0], uy = p[4] - p[1], uz = p[5] - p[2];
    const vx = p[6] - p[0], vy = p[7] - p[1], vz = p[8] - p[2];
    const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
    const length = Math.hypot(nx, ny, nz) || 1;
    const r = ((color >> 16) & 255) / 255, g = ((color >> 8) & 255) / 255, b = (color & 255) / 255;
    const w = triangles * 9;
    for (let k = 0; k < 9; k += 3) {
      pos[w + k] = p[k];pos[w + k + 1] = p[k + 1];pos[w + k + 2] = p[k + 2];
      nrm[w + k] = nx / length;nrm[w + k + 1] = ny / length;nrm[w + k + 2] = nz / length;
      col[w + k] = r;col[w + k + 1] = g;col[w + k + 2] = b;
      for (let a = 0; a < 3; a++) {
        if (p[k + a] < bounds[a]) bounds[a] = p[k + a];
        if (p[k + a] > bounds[a + 3]) bounds[a + 3] = p[k + a];
      }
    }
    triangles++;
  }
  if (offset !== bytes.length) throw new Error('Unexpected bytes after model');
  return {pos, nrm, col, count: n * 3, box: bounds};
}

// the worker's body: one job a message, answered with its id
function workerMain(decode) {
  self.onmessage = event => {
    const {id, bytes, n} = event.data || {};
    try {
      const m = decode(bytes, n);
      self.postMessage({id, model: m}, [m.pos.buffer, m.nrm.buffer, m.col.buffer]);
    } catch (error) {
      self.postMessage({id, error: String(error?.message || error)});
    }
  };
}

let worker = null, broken = false, serial = 0;
const jobs = new Map();
function start() {
  if (worker || broken) return worker;
  try {
    if (typeof Worker !== 'function' || typeof Blob !== 'function' || typeof URL?.createObjectURL !== 'function') throw Error('no workers');
    const src = `(${workerMain.toString()})(${meshDecode.toString()});`;
    const url = URL.createObjectURL(new Blob([src], {type: 'text/javascript'}));
    worker = new Worker(url, {name: 'backplane-mesh-decode'});
    URL.revokeObjectURL(url);
    worker.onmessage = event => {
      const {id, model, error} = event.data || {};
      const job = jobs.get(id);if (!job) return;
      jobs.delete(id);
      if (error) job.reject(new Error(error));else job.resolve(model);
    };
    // A worker that cannot run at all (a CSP refusal arrives as an error)
    // hands every waiting job back to the main thread, and no other starts.
    worker.onerror = event => {
      event?.preventDefault?.();
      broken = true;try { worker.terminate(); } catch {}
      worker = null;
      for (const job of jobs.values()) job.fallback();
      jobs.clear();
    };
  } catch { broken = true;worker = null; }
  return worker;
}

const abortError = () => new DOMException('Model superseded', 'AbortError');

// decodeMesh's contract (solid-assets.js): the same checks first, the same
// model out, an AbortError once superseded. `fallback` is the main-thread
// decoder, used when no worker can run.
export function workerDecode(fallback) {
  return (o, opts = {}) => {
    const {signal, maxBytes = 128 * 1024 * 1024} = opts;
    if (!(o?.mesh instanceof Uint8Array)) return fallback(o, opts);
    if (!Number.isSafeInteger(o.n) || o.n < 1 || o.n * 108 > maxBytes || o.mesh.length < o.n * 10) return fallback(o, opts);
    const w = start();
    if (!w) return fallback(o, opts);
    if (signal?.aborted) return Promise.reject(abortError());
    return new Promise((resolve, reject) => {
      const id = ++serial;
      const job = {
        resolve: model => { signal?.removeEventListener?.('abort', onAbort);if (signal?.aborted) reject(abortError());else resolve(model); },
        reject: error => { signal?.removeEventListener?.('abort', onAbort);reject(signal?.aborted ? abortError() : error); },
        fallback: () => { signal?.removeEventListener?.('abort', onAbort);fallback(o, opts).then(resolve, reject); },
      };
      // A superseded model's answer is dropped when it comes; the worker
      // finishes it (one decode is short off the main thread).
      const onAbort = () => { jobs.delete(id);reject(abortError()); };
      signal?.addEventListener?.('abort', onAbort, {once: true});
      jobs.set(id, job);
      // The wire stays with the page (a later revision is compared with it),
      // so the worker gets a copy, transferred.
      const bytes = o.mesh.slice();
      try { w.postMessage({id, bytes, n: o.n}, [bytes.buffer]); }
      catch { jobs.delete(id);job.fallback(); }
    });
  };
}
