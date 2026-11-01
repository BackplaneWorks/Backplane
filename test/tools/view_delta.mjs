// The client's applier (src/web/view-delta.js) against the hub's diff and
// its Bend applier (src/server/thin-delta.bend), on synthetic view trees:
// web nodes with keyed and unkeyed kids, phone entries keyed by id, and
// plain objects and arrays. Every pair must round-trip exactly both ways,
// equal trees must give no ops, and untouched subtrees must stay the
// base's own objects.
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { applyViewDelta } from '../../src/web/view-delta.js';

const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
let seed = 12345;
const rand = n => { seed = (seed * 1103515245 + 12345) % 2147483648;return seed % n; };
const clone = x => JSON.parse(JSON.stringify(x));
const words = ['a', 'bé', 'line\nbreak', 'quote"d', '🦋', '', 'x'.repeat(30)];
let serial = 0;
function text() { return { type: 'text', text: words[rand(words.length)] + rand(100) }; }
function element(depth) {
  const kids = [], n = depth > 3 ? 0 : rand(5);
  const keyed = rand(2) === 0;
  for (let i = 0; i < n; i++) kids.push(rand(3) === 0 ? text() : element(depth + 1));
  if (keyed) for (const k of kids) if (k.type === 'element') k.key = 'r' + serial++;
  return { type: 'element', tag: ['div', 'span', 'bp-memo'][rand(3)], key: '', attrs: [['class', 'c' + rand(4)]], kids };
}
function entry() { return { id: 'e' + serial++, text: words[rand(words.length)], n: rand(9), on: rand(2) === 0, x: null }; }
function body() {
  return { shown: element(0), session: { sel: 't1', drafts: { t1: 'hi' }, rid: rand(1000) },
    window: { entries: Array.from({ length: rand(6) }, entry), hasMore: false } };
}
// one random change somewhere in the tree
function mutate(t) {
  const nodes = [];
  const walk = (n, parent, k) => { nodes.push([n, parent, k]); if (n && typeof n === 'object') for (const key of Object.keys(n)) walk(n[key], n, key); };
  walk(t, null, null);
  const [n, parent, k] = nodes[rand(nodes.length)];
  switch (rand(9)) {
    case 0: if (parent) parent[k] = text(); break;
    case 1: if (Array.isArray(n) && n.length) n.splice(rand(n.length), 1); break;
    case 2: if (Array.isArray(n)) n.splice(rand(n.length + 1), 0, rand(2) ? entry() : element(3)); break;
    case 3: if (Array.isArray(n) && n.length > 1) n.push(n.shift()); break;
    case 4: if (Array.isArray(n) && n.length > 1) n.reverse(); break;
    case 5: if (n && typeof n === 'object' && !Array.isArray(n)) n['f' + rand(3)] = rand(50); break;
    case 6: if (n && typeof n === 'object' && !Array.isArray(n) && Object.keys(n).length) delete n[Object.keys(n)[0]]; break;
    case 7: if (typeof n === 'string' && parent) parent[k] = n + '!'; break;
    case 8: if (Array.isArray(n)) n.push(n.length ? clone(n[0]) : 1); break;
  }
}
const sizes = [];
for (let i = 0; i < 1500; i++) {
  const a = body(), b = clone(a);
  for (let m = rand(4); m >= 0; m--) mutate(b);
  const A = JSON.stringify(a), B = JSON.stringify(b);
  const ops = JSON.parse(lib.diff(A, B));
  assert.ok(Array.isArray(ops), 'walk ran out of fuel on a small tree');
  const got = applyViewDelta(a, ops);
  assert.deepEqual(got, b, `client apply ${i}: ${A}\n→ ${B}\nops ${JSON.stringify(ops)}`);
  assert.equal(JSON.stringify(a), A, 'the base is never written');
  assert.deepEqual(JSON.parse(lib.round(A, B)), b, `Bend apply ${i}`);
  if (A === B) assert.equal(ops.length, 0);
  sizes.push(JSON.stringify(ops).length / Math.max(1, B.length));
  // identity: every top-level field no op reaches is the base's object
  const touched = new Set(ops.map(o => o[1][0]));
  for (const k of Object.keys(b)) if (!touched.has(k) && ops.every(o => o[1].length)) assert.equal(got[k], a[k], 'untouched kept');
}
// equal trees: no ops, the very base back
{
  const a = body(), ops = JSON.parse(lib.diff(JSON.stringify(a), JSON.stringify(a)));
  assert.deepEqual(ops, []);assert.equal(applyViewDelta(a, ops), a);
}
// a row appended to a long keyed list costs that row, not the list
{
  const rows = n => Array.from({ length: n }, (_, i) => ({ type: 'element', tag: 'bp-memo', key: 'm' + i, attrs: [['t', 'x'.repeat(200)]], kids: [] }));
  const a = { shown: { type: 'element', tag: 'div', key: '', attrs: [], kids: rows(200) } };
  const b = clone(a);b.shown.kids.shift();b.shown.kids.push({ ...b.shown.kids[0], key: 'new' });
  const ops = JSON.parse(lib.diff(JSON.stringify(a), JSON.stringify(b)));
  assert.ok(JSON.stringify(ops).length < 3000, 'keyed append is small: ' + JSON.stringify(ops).length);
  const got = applyViewDelta(a, ops);assert.deepEqual(got, b);
  assert.equal(got.shown.kids[0], a.shown.kids[1], 'a kept row is the same object');
}
// a draft echo: the request's own draft goes as ["t", path], none of its text
{
  const draft = 'pasted '.repeat(400); // the hub compares natively; this module does not
  const a = body();a.shown.kids.push({ type: 'element', tag: 'textarea', key: 'composer', attrs: [['value', 'hi']], kids: [] });
  const b = clone(a);b.session.drafts.t1 = draft;b.shown.kids.at(-1).attrs[0][1] = draft;
  const ops = lib.diffd(JSON.stringify(a), JSON.stringify(b), draft);
  assert.ok(ops.length < 1024, 'draft echo is small: ' + ops.length);
  assert.ok(ops.includes('"t"'));
  assert.deepEqual(applyViewDelta(a, JSON.parse(ops), draft), b);
  assert.throws(() => applyViewDelta(a, JSON.parse(ops)), 'a t op without the draft');
}
// a whole body with a long draft: ops from nothing, the draft by reference
{
  const draft = 'big paste ✓ '.repeat(300);
  const a = body();a.session.drafts.t1 = draft;a.shown.kids.push({ type: 'element', tag: 'textarea', key: 'composer', attrs: [['value', draft]], kids: [] });
  const ops = lib.whole(JSON.stringify(a), draft);
  assert.ok(!ops.includes('big paste'), 'the draft is not in a whole body');
  assert.deepEqual(applyViewDelta(null, JSON.parse(ops), draft), a);
}
// malformed ops throw
for (const bad of [[['x', [], 1]], [['s', ['nope'], 1]], [['a', ['shown'], [[0, 99]]]], 'no', [['s', [0], 1]]])
  assert.throws(() => applyViewDelta({ shown: [] }, bad));
sizes.sort((x, y) => x - y);
console.log(`view delta: 1500 pairs exact both ways; ops/new median ${sizes[750].toFixed(3)}`);
