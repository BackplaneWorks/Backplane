// Mock-only qualification of the EXISTING fence; no sockets, subprocesses,
// private fixture or hub. Includes forwarding controls for the owner's fix.
const assert = require('node:assert/strict'), vm = require('node:vm'), fs = require('node:fs');
const { join } = require('node:path');
const { dirname } = require('node:path'), { createHash } = require('node:crypto');
const P = require('./real_store_active_policy.cjs');
const prerequisite = process.env.REAL_ACTIVE_FENCE_PREREQUISITE;
assert(prerequisite, 'explicit corrected-fence prerequisite required; canonical shared file untouched');
const qualified = P.fencePrerequisite(JSON.parse(fs.readFileSync(prerequisite, 'utf8')));
const f = qualified.files.find(f => f.name === 'real_store_fence.cjs');
const sibling = join(dirname(prerequisite), f.name), path = fs.existsSync(sibling) ? sibling : f.path;
const source = fs.readFileSync(path, 'utf8');
assert.equal(createHash('sha256').update(source).digest('hex'), f.sha256);
const calls = [], receipts = [];
function record(name) { return function (...args) { calls.push({ name, args }); return { end() {}, on() {}, destroy() {} }; }; }
const net = { connect: record('net.connect'), createConnection: record('net.createConnection') };
const http = { request: record('http.request'), get: record('http.get') };
const https = { request: record('https.request'), get: record('https.get') };
const context = { URL, Promise, Bun: { connect: record('bun.connect') },
  fetch(...args) { calls.push({ name: 'fetch', args }); return Promise.resolve({ ok: true }); },
  WebSocket: class { constructor(...args) { calls.push({ name: 'websocket', args }); } },
  process: { env: { BACKPLANE_COPY: '1', BACKPLANE_OWNED_FENCE_RECEIPT: 'mock-only' } },
  require(name) { const x = { 'node:net': net, 'node:http': http, 'node:https': https,
    'node:fs': { writeFileSync(_path, text) { receipts.push(JSON.parse(text)); } } }[name]; assert(x); return x; } };
vm.runInNewContext(source, context, { timeout: 1000 });
async function main() {
  const callback = () => {}, headers = { Authorization: 'synthetic-control-only', 'x-test': 'yes' }, options = { method: 'POST', headers };
  // The known old fence loses callback/option overloads. This test must fail
  // against that version, rather than silently adjusting the client protocol.
  http.request('http://127.0.0.1:43210/rpc', options, callback);
  assert.strictEqual(calls.at(-1).args[1], options, 'HTTP options forwarded unchanged');
  assert.strictEqual(calls.at(-1).args[2], callback, 'HTTP callback forwarded unchanged');
  https.get({ hostname: '127.0.0.1', port: 43210, path: '/hello', headers }, callback);
  assert.strictEqual(calls.at(-1).args[1], callback, 'object overload callback retained');
  const n = calls.length;
  assert.throws(() => http.request('http://127.0.0.1.evil/rpc', options, callback));
  assert.throws(() => https.get('https://external.invalid', callback));
  assert.throws(() => net.connect({ host: 'external.invalid', port: 443 }));
  assert.throws(() => net.createConnection({ path: '/mock/foreign.sock' }));
  assert.throws(() => context.Bun.connect({ hostname: 'external.invalid', port: 443 }));
  assert.throws(() => new context.WebSocket('ws://external.invalid'));
  await assert.rejects(context.fetch('https://external.invalid'));
  assert.equal(calls.length, n, 'denied operations never reach original transport');
  await context.fetch('http://127.0.0.1:43210/rpc', { ...options, body: 'synthetic' });
  assert.strictEqual(calls.at(-1).args[1].headers, headers); assert.equal(calls.at(-1).args[1].body, 'synthetic');
  assert.equal(calls.at(-1).args[1].redirect, 'error', 'redirects cannot escape fence');
  assert.equal(receipts.length, 1); assert.equal(receipts[0].installed, true);
  assert.equal(receipts[0].externalFetchBlocked, true); assert.equal(receipts[0].externalConnectBlocked, true);
  console.log(JSON.stringify({ scope: 'combined-real-store-existing-fence-mocks', ok: true, hubStarted: false, actualNetworkCalls: 0 }));
}
main().catch(() => { console.error('FAIL existing fence forwarding/denial/receipt controls'); process.exitCode = 1; });
