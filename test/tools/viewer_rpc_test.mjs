// Bounded viewer reads against the production controller, with a deterministic
// clock: no real socket, application state, or elapsed-time sleeps.
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const sourceRoot = resolve(process.env.VIEWER_REVIEW_SOURCE_ROOT || '.');
const {ViewerRpc} = await import(pathToFileURL(resolve(sourceRoot, 'src/web/viewer-rpc.js')).href);
const original = {setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout};
let now = 0, nextTimer = 0;
const timers = new Map();
globalThis.setTimeout = (fn, ms) => { const id = ++nextTimer;timers.set(id, {fn, at: now + ms});return id; };
globalThis.clearTimeout = id => timers.delete(id);
function advance(ms) {
  now += ms;
  for (const [id, timer] of [...timers]) if (timer.at <= now) { timers.delete(id);timer.fn(); }
}
try {
  const sent = [], rpc = new ViewerRpc(request => sent.push(request), {timeoutMs: 100});
  await assert.rejects(rpc.components(''), /selected thread/);
  assert.equal(timers.size, 0);assert.equal(sent.length, 0);
  const calls = Array.from({length: 4}, (_, i) => rpc.components(`thread-${i}`));
  await assert.rejects(rpc.components('fifth'), /busy/);
  assert.equal(sent.length, 4);assert.equal(timers.size, 4);
  for (const [index, request] of sent.entries()) {
    assert.ok(Number.isSafeInteger(request.id) && request.id > 2 ** 46 && request.id < 2 ** 47);
    assert.deepEqual(request, {id: request.id, m: 'kicad.components', p: {thread: `thread-${index}`}});
  }
  assert.equal(new Set(sent.map(x => x.id)).size, 4);
  assert.equal(rpc.receive({t: 'reply', id: 1, result: {components: {wrong: true}}}), false);
  assert.equal(rpc.receive({t: 'notice', id: sent[0].id, result: {components: {wrong: true}}}), false);
  assert.equal(rpc.pending.size, 4);
  const components = index => ({source: `saved-${index}`, revision: `r${index}`, rows: [{ref: `U${index}`} ]});
  for (const index of [3, 1, 0, 2]) {
    const envelope = {t: 'reply', id: sent[index].id, ok: true, result: {components: components(index)}};
    assert.equal(rpc.receive(envelope), true);
    assert.equal(rpc.receive(envelope), false, 'duplicate reply must not be consumed twice');
  }
  assert.deepEqual(await Promise.all(calls), [0,1,2,3].map(components));
  assert.equal(timers.size, 0);assert.equal(rpc.pending.size, 0);

  const timed = rpc.components('timed-out');
  const timedRejected = assert.rejects(timed, /timed out/);
  const timedID = sent.at(-1).id;
  advance(99);assert.equal(rpc.pending.size, 1);
  advance(1);await timedRejected;
  assert.equal(rpc.pending.size, 0);assert.equal(timers.size, 0);
  assert.equal(rpc.receive({t: 'reply', id: timedID, result: {components: components(9)}}), false);

  const canceled = [rpc.components('old-hub-A'), rpc.components('old-hub-B')];
  const oldIDs = sent.slice(-2).map(x => x.id);
  const rejected = canceled.map(p => assert.rejects(p, /Connection changed/));
  rpc.reset();await Promise.all(rejected);
  assert.equal(timers.size, 0);assert.equal(rpc.pending.size, 0);
  const fresh = rpc.components('new-hub');const freshID = sent.at(-1).id;
  assert.ok(oldIDs.every(id => freshID > id), 'reset must not reuse an old hub request id');
  for (const id of oldIDs) assert.equal(rpc.receive({t:'reply', id, result:{components:components(8)}}), false);
  assert.equal(rpc.pending.size, 1);
  rpc.receive({t: 'reply', id: freshID, result: {components: components(4)}});
  assert.deepEqual(await fresh, components(4));

  for (const envelope of [
    {ok: false, error: 'not allowed'},
    {result: {error: 'source changed'}},
    {ok: true, result: {}},
  ]) {
    const p = rpc.components('bad-source');const rejection = assert.rejects(p);
    assert.equal(rpc.receive({...envelope, t: 'reply', id: sent.at(-1).id}), true);
    await rejection;assert.equal(rpc.pending.size, 0);assert.equal(timers.size, 0);
  }
  const broken = new ViewerRpc(() => { throw new Error('socket closed'); }, {timeoutMs: 100});
  await assert.rejects(broken.components('source'), /socket closed/);
  assert.equal(broken.pending.size, 0);assert.equal(timers.size, 0);
  const owner = new AbortController();
  const abandoned = Array.from({length: 4}, (_, i) => rpc.components(`disposed-${i}`, {signal: owner.signal}));
  const abandonedIDs = sent.slice(-4).map(x => x.id);
  const aborted = abandoned.map(p => assert.rejects(p, {name: 'AbortError'}));
  owner.abort();await Promise.all(aborted);
  assert.equal(rpc.pending.size, 0, 'closing a viewer immediately releases all its request slots');
  assert.equal(timers.size, 0);
  const replacement = rpc.components('reopened');const replacementID = sent.at(-1).id;
  for (const id of abandonedIDs) assert.equal(rpc.receive({t:'reply',id,result:{components:components(8)}}),false);
  rpc.receive({t:'reply',id:replacementID,result:{components:components(5)}});
  assert.deepEqual(await replacement,components(5));
  const sentBefore = sent.length;
  await assert.rejects(rpc.components('already-disposed',{signal:owner.signal}),{name:'AbortError'});
  await assert.rejects(rpc.renderAsset('already-disposed','/saved/board',2,'rev',{signal:owner.signal}),{name:'AbortError'});
  assert.equal(sent.length,sentBefore,'an already disposed owner cannot send another read');
  // A canceled/failed callback must not leave a timer that fires into a later
  // request, even after several whole timeout periods.
  advance(1000);assert.equal(timers.size, 0);
  console.log('ok production viewer RPC: read-only envelope, four-call bound, out-of-order ownership, exact timeout, reset, unknown/duplicate/late replies, errors and send failure cleanup');
} finally { globalThis.setTimeout = original.setTimeout;globalThis.clearTimeout = original.clearTimeout; }
