// The browser's store of last plots (src/web/plot-keep.js) over a small
// in-memory IndexedDB: a plot comes back by its exact key; one too big is not
// kept and drops the older one of its key; a source gone (bytes Infinity, as
// plot2d.js saves on the hub's "none") is dropped; past 8 plots or 24 MB the
// least recently drawn go (Keep.over, plot2d.js drops them); no IndexedDB at all is simply nothing kept.
import assert from 'node:assert/strict';

function fakeIDB() {
  const stores = new Map();
  const later = (req, f) => setTimeout(() => { try { req.result = f();req.onsuccess?.(); } catch (e) { req.error = e;req.onerror?.(); } }, 0);
  const store = (name) => {
    const m = stores.get(name);
    const op = (f) => { const req = {};later(req, f);return req; };
    return { put: (v, k) => op(() => { m.set(k, structuredClone(v));return k; }), delete: (k) => op(() => { m.delete(k); }),
      getAll: () => op(() => [...m.values()].map((v) => structuredClone(v))), getAllKeys: () => op(() => [...m.keys()]) };
  };
  const db = { objectStoreNames: { contains: (n) => stores.has(n) }, createObjectStore: (n) => { stores.set(n, new Map());return store(n); }, transaction: (n) => ({ objectStore: () => store(n) }) };
  return { open: () => { const req = {};setTimeout(() => { if (!stores.size) { req.result = db;req.onupgradeneeded?.(); }req.result = db;req.onsuccess?.(); }, 0);return req; }, stores };
}

const idb = fakeIDB();
globalThis.indexedDB = idb;
const Keep = await import('../../src/web/plot-keep.js');
const entry = (at, bytes = 100) => ({ raw: [{ m: 1, w: 2, d: new Uint8Array([1, 2, 3]), c: 5, a: 255, s: 7, l: 1 }], box: [0, 0, 1, 1], fam: 'f', thick: 1600, at, bytes });

await Keep.save('0|/p/a.kicad_pcb', entry(1));
let all = await Keep.loadAll();
assert.equal(all.size, 1);
assert.deepEqual([...all.get('0|/p/a.kicad_pcb').raw[0].d], [1, 2, 3], 'a plot comes back with its wire bytes');
assert.equal(all.get('1|/p/a.kicad_sch'), undefined, 'only by its exact key');
await Keep.save('0|/p/a.kicad_pcb', entry(2, 7 << 20));
all = await Keep.loadAll();
assert.equal(all.has('0|/p/a.kicad_pcb'), false, 'one too big is not kept, and the older one of its key goes');
await Keep.save('0|/p/b.kicad_pcb', entry(3));
await Keep.save('0|/p/b.kicad_pcb', { bytes: Infinity });
assert.equal((await Keep.loadAll()).has('0|/p/b.kicad_pcb'), false, 'a source the hub says is gone is dropped');
const index = new Map();
for (let i = 0; i < 10; i++) index.set('0|/p/n' + i, { at: 10 + i, bytes: 100 });
assert.deepEqual(Keep.over(index).sort(), ['0|/p/n0', '0|/p/n1'], '8 plots at most, the least recently drawn go first');
const bigs = new Map();
for (let i = 0; i < 6; i++) bigs.set('0|/p/big' + i, { at: 100 + i, bytes: 5 << 20 });
assert.deepEqual(Keep.over(bigs).sort(), ['0|/p/big0', '0|/p/big1'], '24 MB in all at most, the newest stay');
await Keep.save('0|/p/c.kicad_pcb', entry(4));
await Keep.drop(['0|/p/c.kicad_pcb']);
assert.equal((await Keep.loadAll()).has('0|/p/c.kicad_pcb'), false, 'what plot2d.js lets go is gone');
delete globalThis.indexedDB;
console.log('plot_keep_test: ok');
