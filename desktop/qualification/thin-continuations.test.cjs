'use strict';
const { test } = require('node:test'), assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm'), crypto = require('node:crypto');
let fixture;
async function setup() {
  if (fixture) return fixture;
  const path = process.env.BP_ROUTE_BEND_BUNDLE;
  assert(path, 'Set BP_ROUTE_BEND_BUNDLE to the pinned retained-emission fixture; see README.md');
  const bytes = fs.readFileSync(path);assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'), '9c7fb73cee7f7f42ff48e65e1eb7c08d33aa2c2cb41314e0cde55d5b0a71598a');
  const source = bytes.toString('utf8'), stop = source.lastIndexOf('var WIU=0;');assert(stop > 0);
  const App = vm.runInNewContext('(function(){' + source.slice(0, stop) + ';return FL;})()', { console: { log() { throw Error('Old suite forbidden'); } }, TextEncoder, TextDecoder, Buffer, URL, performance, crypto: crypto.webcrypto }, { timeout: 10000 });
  const wire = await import('../../src/web/wire.js');wire.learnDom(App.dom_tags());
  const { ThinContinuations } = await import('../../src/web/thin-continuations.js');
  const node = value => App.presentation_node(App.presentation_json(JSON.stringify(value)));
  fixture = { App, node, ThinContinuations, is: wire.is };return fixture;
}
const txt = text => ({ type: 'text', text });
const el = (tag, key, attrs = [], kids = []) => ({ type: 'element', tag, key, attrs, kids });
const at = (name, value) => ({ name, value });
const button = id => el('button', '', [at('class', 'thin-message-more'), at('title', 'More'), at('data-value', JSON.stringify({ entryId: id, cursor: '' })), { event: 'click', action: 'thin-message-more' }], [txt('More')]);
const memo = (id, fn = 'msg') => el('bp-memo', id, [at('fn', fn), at('who', 'a'), at('stamp', ''), at('tone', 'info'), at('kind', 'run')], [txt('prefix 日本語')]);
const timeline = kids => el('div', 'timeline', [at('id', 'timeline')], kids);
const parts = ids => ids.map(id => ({ id, version: '1', cursor: 'issued:' + id, reason: 'byte-limit', hasMore: true, complete: false }));
const detail = (ids, overrides = {}) => ({ epoch: 'epoch', target: { kind: 'thread', id: 't' }, window: { partial: parts(ids) }, ...overrides });
function kids(n) { const out = [];for (let xs = n.kids; xs?.head; xs = xs.tail) out.push(xs.head);return out; }
function attrs(n) { const out = {};for (let xs = n.attrs; xs?.head; xs = xs.tail) { const a = xs.head;out[a.name || 'on:' + a.event] = a.value ?? a.action; }return out; }
function all(n) { return [n, ...kids(n).flatMap(all)]; }
const textOf = n => n.text ?? kids(n).map(textOf).join('');
const controls = n => all(n).filter(x => attrs(x)['on:click'] === 'thin-message-more');
async function project(tree, ids) {
  const f = await setup(), c = new f.ThinContinuations();c.bind('https://fixture', detail(ids));
  const original = f.node(tree), frozen = JSON.stringify(original), cache = new Map();
  const expand = n => { if (n.tag !== 'bp-memo') return n;if (!cache.has(n.key)) cache.set(n.key, f.App.make(n));return cache.get(n.key); };
  return { ...f, c, original, frozen, expand, result: c.project(original, expand), cache };
}
test('RED generic server footer groups indistinguishable More controls; GREEN attaches exact IDs after real Bend memo expansion', async () => {
  const p = await project(timeline([memo('a'), memo('b', 'act'), button('a'), button('b')]), ['a', 'b']);
  assert.equal(kids(p.original).slice(-2).every(n => textOf(n) === 'More'), true);
  const a = all(p.result).find(n => attrs(n)['data-id'] === 'a'), b = all(p.result).find(n => attrs(n)['data-id'] === 'b');
  assert.equal(controls(a).length, 0, 'message text is whole: no read-more');assert.equal(textOf(controls(b)[0]), 'Read full activity text');
  assert.equal(kids(p.result).filter(n => n.tag === 'button').length, 0);assert.equal(JSON.stringify(p.original), p.frozen);
  for (const memoValue of p.cache.values()) assert.equal(controls(memoValue).length, 0);
});
test('folded and outgoing entries add no orphan Additional message text panel; unfolding restores exact-row reader', async () => {
  const p = await project(timeline([el('button', '', [{ event: 'click', action: 'select' }, at('data-value', 'other-thread')], [txt('outgoing')]), button('hidden'), button('linked')]), ['hidden', 'linked']);
  assert.equal(all(p.result).find(n => n.tag === 'details'), undefined);assert.equal(controls(p.result).length, 0);
  assert(!textOf(p.result).includes('Additional message text'));
  assert.equal(kids(p.result)[0].kids.head.text, 'outgoing');
  const shown = p.c.project(p.node(timeline([memo('hidden', 'act'), button('hidden'), button('linked')])), p.expand);
  const row = all(shown).find(n => attrs(n)['data-id'] === 'hidden');assert.equal(controls(row).length, 1);assert.equal(controls(shown).length, 1);
  assert.equal(p.c.partial.size, 2);assert.equal(p.c.readers.size, 0);
});
test('ambiguous repeated rows and duplicate controls never create an orphan reader or extra-text panel', async () => {
  const p = await project(timeline([memo('a'), memo('a'), button('a'), button('a')]), ['a']);
  assert.equal(controls(p.result).length, 0);assert(!all(p.result).some(n => n.tag === 'details'));assert.equal(p.c.partial.size, 1);
});
test('earlier history is separately labelled and no longer inherits hidden one-pixel class', async () => {
  const p = await project(timeline([el('button', '', [at('class', 'earlier'), { event: 'click', action: 'view-page' }, at('data-value', '')], [txt('More')])]), []);
  const b = kids(p.result)[0];assert.equal(textOf(b), 'Load earlier messages');assert.equal(attrs(b).class, 'thin-earlier');assert.equal(attrs(b)['on:click'], 'view-page');
});
test('explicit assembled plain Unicode activity text shows inline; next cursor retained and no auto-read occurs', async () => {
  const p = await project(timeline([memo('a', 'act'), button('a')]), ['a']);
  assert(p.c.accept({ id: 'a', version: '1', text: '<script>日本語 👩‍💻</script>', hasMore: true, cursor: 'opaque-next', fromStart: true, complete: false }));
  const out = p.c.project(p.original, p.expand), row = all(out).find(n => attrs(n)['data-id'] === 'a'), pre = all(row).find(n => n.tag === 'pre');
  assert.equal(textOf(pre), '<script>日本語 👩‍💻</script>');assert.equal(kids(pre)[0].tag, undefined);
  assert.equal(textOf(controls(row)[0]), 'Read next part');
  assert.deepEqual(JSON.parse(attrs(controls(row)[0])['data-value']), { entryId: 'a', cursor: 'opaque-next' });
  assert.equal(p.c.readers.size, 1);assert.equal(JSON.stringify(p.original), p.frozen);
});
test('complete reader removes continuation and unchanged current refresh retains text', async () => {
  const p = await project(timeline([memo('a', 'act'), button('a')]), ['a']);p.c.accept({ id: 'a', version: '1', text: 'whole', hasMore: false, cursor: '', complete: true, fromStart: true });
  p.c.bind('https://fixture', detail(['a']));const out = p.c.project(p.original, p.expand);assert.equal(controls(out).length, 0);assert.match(textOf(out), /whole/);
});
test('current version change/removal retires only affected reader and rejects stale response without reading', async () => {
  const p = await project(timeline([memo('a', 'act'), memo('b', 'act'), button('a'), button('b')]), ['a', 'b']);
  for (const id of ['a', 'b']) p.c.accept({ id, version: '1', text: id, hasMore: false });
  const d = detail(['a', 'b']);d.window.partial[0].version = '2';p.c.bind('https://fixture', d);assert(!p.c.readers.has('a'));assert(p.c.readers.has('b'));assert(!p.c.accept({ id: 'a', version: '1', text: 'stale' }));
  p.c.bind('https://fixture', detail([]));assert.equal(p.c.readers.size, 0);
});
test('hub, epoch and target switches clear inline text; unknown partial ID is rejected', async () => {
  const f = await setup(), c = new f.ThinContinuations();
  for (const [hub, d] of [['h', detail(['a'])], ['h', detail(['a'], { epoch: 'new' })], ['other', detail(['a'])], ['h', detail(['a'], { target: { kind: 'thread', id: 'new' } })]]) {
    c.bind(hub, d);assert.equal(c.readers.size, 0);assert(!c.accept({ id: 'unknown', version: '1', text: 'bad' }));c.accept({ id: 'a', version: '1', text: 'owned' });
  }
  c.clear();assert.equal(c.readers.size, 0);assert(!c.accept({ id: 'a', version: '1', text: 'late' }));
});
test('synthetic current live message uses exact data-id and readers stay bounded to four', async () => {
  const ids = ['@live:t', 'a', 'b', 'c', 'd'], p = await project(timeline(ids.flatMap(id => [memo(id), button(id)])), ids);
  for (const id of ids) assert(p.c.accept({ id, version: '1', text: id, hasMore: false }));
  assert.equal(p.c.readers.size, 4);assert(!p.c.readers.has('@live:t'));assert.equal(controls(all(p.result).find(n => attrs(n)['data-id'] === '@live:t')).length, 1);
});
test('actual host message callback updates inline presentation without creating a modal or issuing reads', async () => {
  const { ThinContinuations } = await setup(), c = new ThinContinuations();c.bind('h', detail(['a']));let renders = 0;
  const source = fs.readFileSync(require('node:path').join(__dirname, '../../src/web/host.js'), 'utf8');
  const callback = source.slice(source.indexOf('function showThinMessage(message)'), source.indexOf('function stopThin()'));
  const show = vm.runInNewContext('(' + callback + ')', { viewMode: 'view-v1', thinContinuations: c, later() { renders++; }, showViewStatus() {}, resultPanel() { throw Error('No modal'); } });
  show({ id: 'a', version: '1', text: 'explicit response', hasMore: false });assert.equal(renders, 1);assert.equal(c.readers.get('a').text, 'explicit response');
  show({ id: 'a', version: '0', text: 'stale', hasMore: false });assert.equal(renders, 1);
});
test('projected next-click value passes actual ThinHost cursor guard and each invocation requests only one chunk', async () => {
  const { ThinHost } = await import('../../src/web/thin-host.js'), { MessageParts } = await import('../../src/web/thin-results.js');
  const parts = new MessageParts();parts.accept({ id: 'a', thread: 't', version: '1', text: 'prefix', cursor: 'opaque', hasMore: true, complete: false }, '');
  const requests = [], host = { ready: true, window: { partial: [{ id: 'a', cursor: 'issued' }] }, messages: parts, hub: 'h', client: { request(...args) { requests.push(args);return args; } }, flush(x) { return x; } };
  ThinHost.prototype.message.call(host, 'a', 'opaque');assert.equal(requests.length, 1);assert.deepEqual(requests[0], ['h', 'view.message', { entryId: 'a', cursor: 'opaque' }]);
  assert.equal(ThinHost.prototype.message.call(host, 'a', 'wrong'), false);assert.equal(ThinHost.prototype.message.call(host, 'other', ''), false);assert.equal(requests.length, 1);
});
