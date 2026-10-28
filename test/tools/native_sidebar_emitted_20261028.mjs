// Actual emitted bridge materializes real Actor fixtures into CLIENT screens.
// No live service, app, simulator, signing or delivery acceptance.
import fs from 'node:fs';
import vm from 'node:vm';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { cbor, plain } from '../../src/web/wire.js';
const [bundle, directory, output] = process.argv.slice(2);
assert(bundle && directory && output, 'usage: node native_sidebar_emitted_20261028.mjs BUNDLE FIXTURE_DIRECTORY CLIENT_JSONL');
const source = fs.readFileSync(bundle, 'utf8'), receipt = JSON.parse(fs.readFileSync(path.join(directory, 'RECEIPT.json')));
const fixtures = new Map(receipt.fixtures.map(item => {
  const bytes = fs.readFileSync(path.join(directory, item.name + '.json'));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), item.sha256);
  return [item.name, JSON.parse(bytes)];
}));
const head = (m, n) => n < 24 ? Buffer.from([(m << 5) | n]) : n < 256 ? Buffer.from([(m << 5) | 24, n]) :
  n < 65536 ? Buffer.from([(m << 5) | 25, n >> 8, n & 255]) : Buffer.from([(m << 5) | 26, n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255]);
const encode = v => v === null ? Buffer.from([246]) : typeof v === 'boolean' ? Buffer.from([v ? 245 : 244]) :
  typeof v === 'number' ? (v > 4294967295 ? Buffer.concat([Buffer.from([199]), encode(String(v))]) : head(0, v)) :
  typeof v === 'string' ? Buffer.concat([head(3, Buffer.byteLength(v)), Buffer.from(v)]) : Array.isArray(v) ?
    Buffer.concat([head(4, v.length), ...v.map(encode)]) : Buffer.concat([head(5, Object.keys(v).length), ...Object.entries(v).flatMap(([k, x]) => [encode(k), encode(x)])]);
const screens = [];
for (const [name, fixture] of fixtures) {
  if (!fixture.result.shown || fixture.request.action) continue;
  const context = vm.createContext({ console, Date });
  vm.runInContext(source, context, { timeout: 10000 });
  const call = (method, ...args) => { context.args = args; context.method = method; return vm.runInContext('Backplane[method](...args)', context, { timeout: 10000 }); };
  const out = (method, ...args) => JSON.parse(call(method, ...args));
  const dictionaries = JSON.parse(call('headerKeys'));
  const requests = o => o.cmds.filter(c => c.type === 'send').map(c => plain(cbor(new Uint8Array(Buffer.from(c.data, 'base64')), dictionaries.keys, dictionaries.words)));
  const recv = frame => out('recv', 'h', encode(frame).toString('base64'), '1', false, false);
  call('start', 'native-sidebar-fixture', '{}'); call('hubs', ['h']); call('local', 'mailroom-renderer', 'ios-v1');
  assert.equal(call('nativeSidebar', 1), true);
  call('connection', 'h', '1', 'view-v1');
  if (fixture.request.target.kind === 'thread') out('act', 'select', 'h|' + fixture.request.target.id);
  const opened = recv({ t: 'view', version: 1, renderer: 'mobile', generation: '1', epoch: 'origin',
    limits: { replyBytes: 262144, entries: 32, recent: 24, searchResults: 25, searchCandidates: 128, chunkBytes: 65536 } });
  const request = requests(opened)[0];
  assert.equal(request?.m, 'view.open', name);
  assert.equal(request.p.nativeSidebar, 1);
  assert.deepEqual(request.p.target, fixture.request.target);
  const result = { ...fixture.result, version: 1, hub: 'h', generation: '1', epoch: 'origin', target: request.p.target,
    requestRevision: request.p.requestRevision, viewId: request.p.viewId || 'fixture-view', viewRevision: '1', renderer: 'mobile' };
  const presented = recv({ t: 'reply', id: request.id, ok: true, result });
  assert(!presented.sync.some(s => s.phase === 'failed'), name);
  assert(presented.screen && presented.sync.some(s => s.phase === 'presented'), name);
  assert(!Object.hasOwn(presented.screen, 'nativeSidebar'), 'Wire marker leaked into aggregate client screen');
  assert.equal(presented.screen.projects.length, fixture.result.shown.projects.length, name);
  if (fixture.result.shown.nativeSidebar === 1) assert.equal(presented.screen.sidebarPages.length, 1);
  screens.push({ name, clientScreen: presented.screen });
}
assert.equal(screens.length, 9);
fs.writeFileSync(output, screens.map(s => JSON.stringify(s)).join('\n') + '\n');
console.log(JSON.stringify({ pass: true, scope: 'Actual emitted bridge + Actor fixtures, CLIENT materialization only',
  bundleSHA256: createHash('sha256').update(source).digest('hex'), clientScreens: screens.length,
  outputSHA256: createHash('sha256').update(fs.readFileSync(output)).digest('hex') }));
