// The client's half of the canonical sidebar corpus: wire-v1.jsonl, made by
// test/tools/canonical_sidebar_corpus.bend from the real CSID.Side.output,
// delivered unmodified through ThinViews.reply with the real validateThinResult.
// The link allocates the ids the producer wrote, so a reply goes as it came.
// COMPONENT scope: no hub, socket or render. Runs in a capped Boat job only.
//   node test/tools/canonical_sidebar_corpus_client.mjs build/canonical-sidebar/wire-v1.jsonl
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import { ThinViews } from '../../src/web/thin.js';
import { validateThinResult } from '../../src/web/thin-results.js';

const bytes = text => new TextEncoder().encode(text).length;
const frozen = value => JSON.stringify(value);
const asideIn = node => {
  if (node?.tag === 'aside' && node.key === 'sidebar') return node;
  for (const k of node?.kids || []) { const hit = asideIn(k);if (hit) return hit; }
  return null;
};
const composed = rec => Object.hasOwn(rec.reply.result, 'canonicalSidebar');
// What the corpus must have made this client do. A corpus that stops reaching
// one of these fails: 23/23 once passed with no row ever added or moved.
const seen = { insert: 0, move: 0, remove: 0, keptRows: 0, escapedOps: 0, aliases: 0, aliasedHeld: 0 };
const ESCAPED = /[\u0008\u000c]/, WIDE = /[^\u0000-\u007f]/, ASTRAL = /[\u{10000}-\u{10ffff}]/u;
function strings(value, out = []) {
  if (typeof value === 'string') out.push(value);
  else if (value && typeof value === 'object') for (const v of Object.values(value)) strings(v, out);
  return out;
}
// a keyed rebuild of the sidebar's rows: what it did, and that every row it
// kept is the very object the base held, unless another op then reached it
function rebuilt(rec, side, held, aside) {
  const ops = side.ops || [];
  for (const op of ops) {
    if (op[0] !== 'a') continue;
    const row = op[1].length === 1 && op[1][0] === 'kids';
    if (!row) continue;
    const specs = op[2], kept = specs.filter(x => x[0] === 0).map(x => x[1]);
    assert.ok(specs.every(x => Array.isArray(x) && x.length === 2 && (x[0] === 0 ? Number.isSafeInteger(x[1]) : x[0] === 1)), `${rec.name}: keyed specs`);
    assert.equal(new Set(kept).size, kept.length, `${rec.name}: a row kept twice`);
    if (specs.some(x => x[0] === 1)) seen.insert++;
    if (kept.length < held.node.kids.length) seen.remove++;
    if (kept.some((k, i) => i > 0 && k < kept[i - 1])) seen.move++;
    specs.forEach((x, i) => {
      if (x[0] !== 0 || ops.some(o => o !== op && o[1][0] === 'kids' && o[1][1] === i)) return;
      assert.equal(aside.kids[i], held.node.kids[x[1]], `${rec.name}: kept row ${i} is the object held`);
      seen.keptRows++;
    });
  }
  const sets = ops.filter(op => op[0] === 's').flatMap(op => strings(op[2]));
  if (sets.some(t => ESCAPED.test(t)) && sets.some(t => WIDE.test(t)) && sets.some(t => ASTRAL.test(t))) seen.escapedOps++;
}
function link(rec) {
  const p = rec.request.p;
  const v = new ThinViews({ now: () => 0, send: (_h, r) => r, command: c => c, validateResult: validateThinResult, canonicalSidebar: true }, rec.request.id - 1);
  assert.notEqual(v.connect(p.hub, rec.greeting).compatibilityRequired, true, `${rec.name}: greeting`);
  return { v, hub: p.hub, l: v.links.get(p.hub) };
}
// the same request the hub answered, from this link
function ask(at, rec) {
  const { v, hub, l } = at, { m, p } = rec.request;
  l.revision = p.requestRevision - 1;
  if (m === 'view.open') return v.select(hub, p.target, p.session, 'cid');
  const { version, hub: _h, generation, epoch, viewControl, windowProjection, requestRevision, target, viewId, accountProfiles, accountBinding, bodyPages, sidebarRegion, viewDelta, canonicalSidebar, ...extra } = p;
  assert.deepEqual(l.target, target, `${rec.name}: a ${m} stays on its target`);
  assert.equal(l.viewId, viewId, `${rec.name}: a ${m} names the view the last reply gave`);
  return v.request(hub, m, extra);
}
function deliver(at, rec) {
  const { v, hub, l } = at, e = rec.reply, id = e.id;
  if (bytes(JSON.stringify(e)) <= l.greeting.limits.replyBytes) return v.reply(hub, e);
  // a client component test of collection only: the production framing and
  // socket stay unqualified (the corpus's README)
  // (5461 characters a part, as Part.take cuts them: a part is bounded in UTF-8 bytes)
  const { shown, ...rest } = e.result, text = JSON.stringify(rest), n = Math.ceil(text.length / 5461);
  for (let i = 0; i < n; i++) v.control(hub, { t: 'view.control', id, part: i, last: i === n - 1, text: text.slice(i * 5461, (i + 1) * 5461) });
  return v.reply(hub, { t: 'reply', ok: true, id, result: { shown, controlId: id } });
}
function step(at, rec, held) {
  const issued = ask(at, rec), call = at.v.calls.get(issued.id);
  assert.equal(issued.id, rec.request.id, `${rec.name}: the id the producer wrote`);
  assert.deepEqual(call.input.canonicalSidebar, rec.request.p.canonicalSidebar, `${rec.name}: the link offers what the hub answered`);
  const before = held ? frozen(held.node) : '', raw = frozen(rec.reply);
  const result = deliver(at, rec);
  assert.equal(frozen(rec.reply), raw, `${rec.name}: the reply as received was written to`);
  assert.equal(result.failed, undefined, `${rec.name}: ${result.failed}`);
  assert.deepEqual(result.shown, rec.expectedRoot, `${rec.name}: reconstructed root`);
  assert.deepEqual(result.cmds, rec.reply.result.cmds, `${rec.name}: commands`);
  if (composed(rec)) assert.equal(rec.logicalBytesExact, true, `${rec.name}: logicalBytes`);
  if (held) assert.equal(frozen(held.node), before, `${rec.name}: the base was written to`);
  const side = rec.reply.result.canonicalSidebar;
  if (side?.ops?.length === 0) assert.equal(asideIn(result.shown), held.node, `${rec.name}: no ops is the object held`);
  if (side?.ops) rebuilt(rec, side, held, asideIn(result.shown));
  if (composed(rec)) {
    assert.equal(at.l.canonical.entries[0].token, side.token, `${rec.name}: kept`);
    // what the page gets is the very sidebar kept, aliases or none
    assert.equal(asideIn(result.shown), at.l.canonical.entries[0].node, `${rec.name}: the page holds the sidebar kept`);
    if (rec.reply.result.entryAliases) { seen.aliases++;if (side.ops?.length === 0) seen.aliasedHeld++; }
  }
  else assert.equal(at.l.canonical.entries.length, 0, `${rec.name}: a whole reply clears the history`);
  return at.l.canonical.entries[0] || null;
}
function single(rec) {
  assert.equal(typeof rec.probe, 'boolean', `${rec.name}: probe`);
  // a server eligibility probe: the component left it alone, nothing more
  if (rec.probe) { assert.deepEqual(rec.reply, rec.original, `${rec.name}: left alone`);return; }
  const at = link(rec);
  if (rec.base) at.l.canonical.entries = [{ ...rec.base, bytes: bytes(JSON.stringify(rec.base.node)) }];
  step(at, rec, at.l.canonical.entries[0] || null);
}
// one link for the whole chain: its history is what the steps before made
function chain(rec) {
  const at = link(rec.steps[0]);
  let held = null;
  rec.steps.forEach((s, i) => {
    const side = s.reply.result.canonicalSidebar, last = i === rec.steps.length - 1;
    if (i === 0) assert.ok(side?.node, `${rec.name}: starts with a seed`);
    else if (last) assert.deepEqual(side?.ops, [], `${rec.name}: ends on no ops`);
    else assert.ok(side?.ops?.length > 0, `${rec.name}/${s.name}: a change is ops`);
    if (held) assert.deepEqual(s.base.node, held.node, `${rec.name}/${s.name}: the hub's base is the sidebar this link rebuilt`);
    held = step(at, s, held);
  });
}

const lines = readFileSync(process.argv[2], 'utf8').split('\n').filter(Boolean);
if (!lines.length) throw Error('empty corpus');
let failed = 0;
for (const line of lines) {
  const rec = JSON.parse(line);
  assert.equal(rec.schema, 1);assert.equal(rec.scope, 'COMPONENT');
  try { rec.kind === 'chain' ? chain(rec) : single(rec);console.log('ok', rec.name); }
  catch (e) { failed++;console.log('FAIL', rec.name, e.message); }
}
console.log(`${lines.length - failed}/${lines.length} records`);
console.log('coverage', JSON.stringify(seen));
// exact: more is as wrong as fewer, it means the corpus is not the one reviewed
const EXPECT = { insert: 1, move: 1, remove: 1, keptRows: 18, escapedOps: 1, aliases: 1, aliasedHeld: 1 };
for (const [name, n] of Object.entries(EXPECT)) if (seen[name] !== n) { failed++;console.log('FAIL coverage', name, seen[name], 'expected', n); }
const records = lines.map(line => JSON.parse(line)), chains = records.filter(r => r.kind === 'chain').map(r => r.steps.length);
if (records.length !== 25 || chains.join() !== '6,5') { failed++;console.log('FAIL corpus shape', records.length, chains.join()); }
process.exit(failed ? 1 : 0);
