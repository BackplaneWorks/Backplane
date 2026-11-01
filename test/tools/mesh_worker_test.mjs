// mesh-worker.js's meshDecode against solid-assets.js's decodeMesh: the same
// arrays and box on random models, the same errors on broken ones, and the
// worker's contract (fallback when no Worker exists).
//   node test/tools/mesh_worker_test.mjs
import assert from "node:assert/strict";
import { meshDecode, workerDecode } from "../../src/web/mesh-worker.js";
import { decodeMesh } from "../../src/web/solid-assets.js";

const varint = (out, v) => { while (v >= 128) { out.push((v & 127) | 128);v = Math.floor(v / 128); } out.push(v); };
const zig = v => v < 0 ? -2 * v - 1 : 2 * v;
let seed = 7;const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
function model(n) {
  const out = [];
  for (let t = 0; t < n; t++) {
    varint(out, Math.floor(rnd() * 0xffffff));
    for (let k = 0; k < 9; k++) varint(out, zig(Math.floor((rnd() - 0.5) * 20000)));
  }
  return { n, mesh: Uint8Array.from(out) };
}
const now = () => performance.now();
for (const n of [1, 3, 64, 2049, 5000]) {
  const o = model(n);
  const a = meshDecode(o.mesh, o.n);
  const b = await decodeMesh(o, { schedule: async () => {}, now });
  assert.equal(a.count, b.count);
  assert.deepEqual([...a.box], [...b.box]);
  for (const f of ["pos", "nrm", "col"]) assert.deepEqual(a[f], b[f], `${f} at n=${n}`);
}
// broken wires fail the same way
const o = model(10);
for (const bad of [{ n: 10, mesh: o.mesh.subarray(0, o.mesh.length - 3) }, { n: 9, mesh: o.mesh }]) {
  let e1 = "", e2 = "";
  try { meshDecode(bad.mesh, bad.n); } catch (e) { e1 = e.message; }
  try { await decodeMesh(bad, { schedule: async () => {}, now }); } catch (e) { e2 = e.message; }
  assert.ok(e1 && e1 === e2, `${e1} vs ${e2}`);
}
// no Worker here (node): the main-thread decoder answers
let used = 0;
const dec = workerDecode((x, opts) => { used++;return decodeMesh(x, { ...opts, schedule: async () => {}, now }); });
const m = await dec(model(100));
assert.equal(m.count, 300);assert.equal(used, 1);
console.log("ok mesh_worker");
