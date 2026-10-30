// bun test/tools/solid_assets_test.mjs
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {decodeMesh, SolidAssets} from '../../src/web/solid-assets.js';

function wire(n = 1, x = 0) {
  const bytes = [];
  const put = v => { do { const b = v % 128;v = Math.floor(v / 128);bytes.push(b | (v ? 128 : 0)); } while (v); };
  let color = 0, prev = [0, 0, 0];
  for (let i = 0; i < n; i++) {
    put((color ^ 0x345678) >>> 0);color = 0x345678;
    for (const p of [[x + i, 0, 0], [x + i, 2, 0], [x + i + 1, 0, 0]]) for (let k = 0; k < 3; k++) {
      const delta = p[k] - prev[k];put(delta < 0 ? -2 * delta - 1 : 2 * delta);prev[k] = p[k];
    }
  }
  return {key: '2|board', n, mesh: new Uint8Array(bytes)};
}

const fixture = JSON.parse(readFileSync(new URL('../fixtures/solid-handedness/wire.json', import.meta.url)));
const decoded = await decodeMesh({n: fixture.n, mesh: new Uint8Array(Buffer.from(fixture.mesh_base64, 'base64'))});
assert.equal(decoded.count, fixture.n * 3);
const [x0, y0, z0, x1, y1, z1] = fixture.wire_bounds.map(x => x | 0);
assert.deepEqual(decoded.box, [x0, -y1, z0, x1, -y0, z1]);
const simple = await decodeMesh(wire());
assert.deepEqual([...simple.pos].map(x => x || 0), [0, 0, 0, 0, -2, 0, 1, 0, 0]);
assert.ok(simple.nrm.every((v, i) => Math.abs(v - (i % 3 === 2 ? 1 : 0)) < 1e-6));

// Meaningful partial geometry arrives before completion, and each slice hands
// the event loop back to input. No synthetic progress while awaiting export.
let inputRan = false, yielded = 0;
const progresses = [];
await decodeMesh(wire(1000), {sliceTriangles: 64, schedule: async () => { yielded++;await new Promise(resolve => setTimeout(() => { inputRan = true;resolve(); }, 0)); }, onProgress: p => progresses.push(p)});
assert.ok(inputRan && yielded > 2);
assert.ok(progresses[0].completed > 0 && progresses[0].completed < 1000);
assert.equal(progresses[0].model.count, progresses[0].completed * 3);
assert.equal(progresses.at(-1).completed, 1000);
for (const bad of [{n: 1, mesh: new Uint8Array(0)}, {n: 1e9, mesh: new Uint8Array(10)}, {n: 1, mesh: Uint8Array.from([...wire().mesh, 0])}, {n: 1, mesh: new Uint8Array(20).fill(255)}]) {
  await assert.rejects(decodeMesh(bad));
}
const aborted = new AbortController();aborted.abort();
await assert.rejects(decodeMesh(wire(), {signal: aborted.signal}), {name: 'AbortError'});
console.log('ok actual geometry progress, event loop yields, bounds/normals, malformed and oversized payloads');

let calls = 0;
const changes = [];
const assets = new SolidAssets({onChange: r => changes.push({key: r.key, status: r.status, stale: r.stale}), decode: async (o, options) => { calls++;return decodeMesh(o, options); }});
assets.pin('2|board');
const first = await assets.receive(wire());
const firstModel = first.model;
assert.equal(assets.reuse('2|board').cached, true);
const same = await assets.receive({...wire(), mesh: wire().mesh.slice()});
assert.equal(calls, 1);assert.equal(same.model, firstModel);assert.equal(same.cached, true);
const changed = await assets.receive(wire(1, 5));
assert.equal(calls, 2);assert.notEqual(changed.model, firstModel);assert.equal(changed.model.box[0], 5);
assert.ok(changes.some(r => r.status === 'decoding' && r.stale));
const failed = await assets.receive({key: '2|board', none: 'Export failed'});
assert.equal(failed.status, 'failed');assert.equal(failed.model, changed.model);assert.equal(failed.stale, true);
const retry = await assets.receive(wire(1, 5));
assert.equal(retry.status, 'ready');assert.equal(calls, 3);
console.log('ok exact revisions, warm reuse without decode, explicit stale model during update/failure, retry');

// A late result (or late failure) from an old revision or retired hub cannot
// publish geometry, replace the new revision, or resurrect an evicted source.
const jobs = [], seen = [];
const pending = new SolidAssets({onChange: r => seen.push(`${r.revision}:${r.status}`), decode: () => new Promise((resolve, reject) => jobs.push({resolve, reject}))});
const old = pending.receive(wire(1, 1));
const latest = pending.receive(wire(1, 2));
jobs[1].resolve({pos: new Float32Array(), nrm: new Float32Array(), col: new Float32Array(), count: 3, box: [2, 0, 0, 3, 1, 1]});
const newest = await latest;
jobs[0].reject(new Error('Late old failure'));await old;
assert.equal(pending.get('2|board'), newest);
assert.deepEqual(seen, ['1:decoding', '2:decoding', '2:ready']);
const retired = pending.receive(wire(1, 3));pending.reset();
jobs[2].resolve(newest.model);await retired;
assert.equal(pending.get('2|board'), undefined);assert.equal(seen.at(-1), '3:decoding');
console.log('ok stale revisions/failures, canceled jobs and hub resets cannot publish');

const bounded = new SolidAssets({maxEntries: 2, maxStates: 2});
await bounded.receive({...wire(), key: 'a'});bounded.pin('a');
await bounded.receive({...wire(), key: 'b'});await bounded.receive({...wire(), key: 'c'});
assert.equal(bounded.get('b'), undefined);assert.ok(bounded.get('a'));assert.ok(bounded.get('c'));
const state = {camera: {pivot: [1, 2, 3], r: [1, 0, 0], dist: 10}, selection: 'U3', layers: ['F.Cu']};
bounded.saveState('a', state);state.camera.pivot[0] = 999;
const restored = bounded.getState('a');assert.equal(restored.camera.pivot[0], 1);
restored.camera.dist = 999;assert.equal(bounded.getState('a').camera.dist, 10);
bounded.saveState('b', {});bounded.saveState('c', {});assert.equal(bounded.getState('a'), undefined);
bounded.reset();assert.equal(bounded.getState('c'), undefined);
console.log('ok bounded cache, current source pin, camera/selection/layer snapshots survive independently of canvases');
