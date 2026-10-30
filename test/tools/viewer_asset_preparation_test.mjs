// Production preparation lifecycle: a PCB revision can overlap authenticated
// asset work, but never starts an original render or promotes another revision.
import assert from 'node:assert/strict';
import { ViewerQuality } from '../../src/web/viewer-quality.js';
const calls = [];
const quality = Object.assign(Object.create(ViewerQuality.prototype), {
  alive: true, state: { quality: true }, generation: 0, cleanup: [],
  root: { dataset: { key: '2|/project|/saved/board.kicad_pcb', scope: 'thread' } },
  services: { asset(...args) {
    let resolve, reject;
    const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
    const call = { args, resolve, reject };
    args[4].signal.addEventListener('abort', () => reject(new DOMException('Canceled', 'AbortError')), { once: true });
    calls.push(call); return promise;
  } },
  show() {},
});
const first = quality.prepareAsset('revision-a');
assert.equal(quality.prepareAsset('revision-a'), first);
assert.equal(calls.length, 1);
assert.deepEqual(calls[0].args.slice(0, 4), ['thread', '/saved/board.kicad_pcb', '2', 'revision-a']);
assert.equal(quality.renderer, undefined, 'preparation cannot create a renderer');
quality.stop('revision-a');
assert.equal(calls[0].args[4].signal.aborted, false, 'independent matching assembly confirmation reuses preparation');
assert.equal(quality.prepareAsset('revision-a'), first);
const rejected = assert.rejects(first, { name: 'AbortError' });
const second = quality.prepareAsset('revision-b');
await rejected;
assert.equal(calls[0].args[4].signal.aborted, true);
assert.equal(quality.preparedAsset.promise, second, 'late old rejection cannot erase the replacement');
calls[1].resolve({ source: '/saved/board.kicad_pcb', revision: 'revision-b' });
assert.equal((await second).revision, 'revision-b');
quality.stop();
assert.equal(quality.preparedAsset, null, 'Fast/disposal releases prepared ownership');
const third = quality.prepareAsset('revision-c');
const thirdRejected = assert.rejects(third, { name: 'AbortError' });
quality.dispose();
await thirdRejected;
assert.equal(quality.prepareAsset('revision-d'), null);
assert.equal(calls.length, 3, 'a disposed viewer cannot issue speculative reads');
console.log('ok original asset preparation: exact source/revision reuse, supersession, cancellation and late rejection ownership; no render promotion');
