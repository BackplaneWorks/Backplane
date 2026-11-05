// A hub that names every component page's offset in the first page: the
// production reader asks for the later pages at once (a few at a time), takes
// them in order, and gives the same rows as reading them one by one.
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const sourceRoot = resolve(process.env.VIEWER_REVIEW_SOURCE_ROOT || '.');
const {ViewerRpc} = await import(pathToFileURL(resolve(sourceRoot, 'src/web/viewer-rpc.js')).href);
const all = Array.from({length: 9}, (_, i) => ({id: `uuid-${i}`, reference: `R${i + 1}`}));
const starts = [0, 2, 3, 5, 8];
function page(offset, listed) {
  const at = starts.indexOf(offset), next = at + 1 < starts.length ? starts[at + 1] : all.length;
  const p = {offset, total: all.length, next: next === all.length ? null : next, complete: next === all.length};
  if (offset === 0 && listed) p.offsets = listed;
  return {schema: 1, source: '/saved/board.kicad_pcb', schematic: '/saved/board.kicad_sch', sourceRevision: 'pcb-r1', schematicRevision: 'sch-r1',
    fingerprint: 'source-hash', thicknessMm: 1.6, components: all.slice(offset, next), page: p};
}
// replies come back only when asked to, last asked first
function endpoint(listed) {
  const sent = [], rpc = new ViewerRpc(request => sent.push(request), {timeoutMs: 60000});
  const answer = request => rpc.receive({t: 'reply', id: request.id, ok: true, result: {components: page(request.p.offset || 0, listed)}});
  return {sent, rpc, answer};
}
const tick = () => new Promise(r => setImmediate(r));
{
  const {sent, rpc, answer} = endpoint(starts);
  const read = rpc.components('thread-a');
  assert.equal(sent.length, 1);
  answer(sent[0]);await tick();
  // every later page is asked for before any of them is answered
  assert.deepEqual(sent.slice(1).map(s => s.p.offset), [2, 3, 5, 8]);
  for (const s of sent.slice(1)) assert.deepEqual([s.p.sourceRevision, s.p.schematicRevision, s.p.thread], ['pcb-r1', 'sch-r1', 'thread-a']);
  for (const s of sent.slice(1).reverse()) answer(s);
  const got = await read;
  assert.deepEqual(got.components.map(c => c.id), all.map(c => c.id));
  assert.equal(rpc.pending.size, 0);
}
{
  // no offsets (an older hub): one page at a time, as before
  const {sent, rpc, answer} = endpoint(null);
  const read = rpc.components('thread-a');
  for (let i = 0; i < starts.length; i++) { await tick();assert.equal(sent.length, i + 1);answer(sent[i]); }
  assert.deepEqual((await read).components.map(c => c.id), all.map(c => c.id));
}
{
  // offsets that do not match the pages' own cursors: each page is still the
  // one its predecessor names (asked for then if it was not asked ahead)
  const {sent, rpc, answer} = endpoint([0, 2, 4, 5, 8]);
  let done = false, n = 0;
  const read = rpc.components('thread-a').finally(() => { done = true; });
  for (let i = 0; i < 20 && !done; i++) { while (n < sent.length) answer(sent[n++]);await tick(); }
  assert.deepEqual((await read).components.map(c => c.id), all.map(c => c.id));
  assert.ok(sent.some(s => s.p.offset === 3));
}
{
  // a list that is not offsets (not increasing) is ignored: pages one at a time
  const {sent, rpc, answer} = endpoint([0, 5, 3]);
  const read = rpc.components('thread-a');
  answer(sent[0]);await tick();
  assert.equal(sent.length, 2);assert.equal(sent[1].p.offset, 2);
  for (let i = 1; i < starts.length; i++) { await tick();answer(sent[i]); }
  assert.deepEqual((await read).components.map(c => c.id), all.map(c => c.id));
}
{
  // pages in flight ahead never take a separate read's place: the original
  // model's asset asked meanwhile is sent (it was refused as busy after a
  // reload, and never asked again); separate reads keep their own bound
  const {sent, rpc, answer} = endpoint(starts);
  const read = rpc.components('thread-a');
  answer(sent[0]);await tick();
  assert.equal(sent.length, 5);
  const asset = rpc.renderAsset('thread-a', 'board.kicad_pcb', '2', 'pcb-r1');
  assert.equal(sent.length, 6);assert.equal(sent[5].m, 'kicad.renderAsset');
  const more = [rpc.renderAsset('thread-a', 'b.kicad_pcb', '2', 'r'), rpc.renderAsset('thread-a', 'c.kicad_pcb', '2', 'r'), rpc.renderAsset('thread-a', 'd.kicad_pcb', '2', 'r')];
  assert.equal(sent.length, 9);
  await assert.rejects(rpc.renderAsset('thread-a', 'e.kicad_pcb', '2', 'r'), /busy/);
  for (const s of sent.slice(1, 5).reverse()) answer(s);
  assert.deepEqual((await read).components.map(c => c.id), all.map(c => c.id));
  rpc.receive({t: 'reply', id: sent[5].id, ok: true, result: {asset: {url: '/a'}}});
  assert.deepEqual(await asset, {url: '/a'});
  rpc.reset();await Promise.allSettled(more);
}
console.log('ok component pages asked ahead: listed offsets asked at once, taken in order, same rows; older hubs and bad lists page one by one, a wrong list falls back to the cursors; pages ahead never refuse a separate read');
