// A failed durable commit must complete its RPC and leave the hub usable.
// Run with Bun and an already-built server; no build, live home or UI access.
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const assert = require('node:assert/strict');
const { Database } = require('bun:sqlite');
const { cbor, plain } = require('../../src/web/wire.js');
const binary = path.resolve(process.argv[2] || 'build/backplane-serve');
const port = Number(process.env.E2E_PORT || 3947);
assert(Number.isInteger(port) && port > 1024 && port !== 3787);
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-store-rpc-'));
const home = path.join(root, 'home'), agents = path.join(root, 'agents');
fs.mkdirSync(home); fs.mkdirSync(agents);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp'])
  fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
const token = 'synthetic-checkpoint-rpc-token';
fs.writeFileSync(path.join(home, 'token'), token, { mode: 0o600 });
fs.writeFileSync(path.join(home, 'events.jsonl'), [
  { $: 'ProjectCreated', id: 'checkpoint-project', title: 'Checkpoint', root, at: 1 },
  { $: 'ThreadCreated', id: 'checkpoint-thread', project: 'checkpoint-project',
    title: 'Original', env: 'local', branch: '', worktree: root,
    provider: 'codex', model: '', at: 2 },
].map(JSON.stringify).join('\n') + '\n');
const source = fs.readFileSync(path.join(__dirname, '../../src/core/cbor.bend'), 'utf8');
const dict = name => JSON.parse(new RegExp('def Cbor\\.' + name + '\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])').exec(source)[1]);
const keys = dict('keys'), words = dict('words');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let hub, ws;
async function stop() {
  ws?.close(); ws = undefined;
  if (!hub) return;
  hub.kill('SIGTERM');
  await Promise.race([hub.exited, sleep(3000)]);
  if (hub.exitCode === null) { hub.kill('SIGKILL'); await hub.exited; }
  hub = undefined;
}
async function start() {
  hub = Bun.spawn([binary, '--home', home, '--port', String(port), '--host', '127.0.0.1', '--no-tailscale', '--headless'], {
    env: { ...process.env, HOME: root, DISPLAY: '', WAYLAND_DISPLAY: '',
      DBUS_SESSION_BUS_ADDRESS: '', BACKPLANE_AGENTS: agents, BACKPLANE_PEERS: '',
      BACKPLANE_LOCK_DIR: path.join(root, 'locks'), BACKPLANE_TELEMETRY: 'off', BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9' },
    stdout: 'ignore', stderr: Bun.file(path.join(root, 'service.log')),
  });
  for (let i = 0; i < 200; i++) {
    assert.equal(hub.exitCode, null, 'owned server exited during boot');
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) })).ok) return; } catch {}
    await sleep(50);
  }
  throw Error('disposable service readiness deadline');
}
// Request IDs fit u32; strings carry all other values in this regression.
function encode(v) {
  const head = (m, n) => n < 24 ? Buffer.from([m << 5 | n]) : n < 256 ? Buffer.from([m << 5 | 24, n]) : Buffer.from([m << 5 | 25, n >> 8, n & 255]);
  if (typeof v === 'number') { assert(Number.isInteger(v) && v >= 0 && v < 65536); return head(0, v); }
  if (typeof v === 'string') { const b = Buffer.from(v); return Buffer.concat([head(3, b.length), b]); }
  const ks = Object.keys(v);
  return Buffer.concat([head(5, ks.length), ...ks.flatMap(k => [encode(k), encode(v[k])])]);
}
const db = () => new Database(path.join(home, 'store.db'));
const title = () => { const d = db(); try { return d.query('SELECT title FROM thread WHERE id=?').get('checkpoint-thread').title; } finally { d.close(); } };
const marker = 'CHECKPOINT_REJECTED_TEST';
let pending;
function request(id, value) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending = undefined; reject(Error('RPC error/success completion deadline')); }, 5000);
    pending = { id, resolve: o => { clearTimeout(timer); pending = undefined; resolve(o); }, reject };
    ws.send(encode({ id, m: 'thread.rename', p: { thread: 'checkpoint-thread', title: value } }));
  });
}
(async () => {
  try {
    await start(); await stop();
    const d = db();
    d.exec(`CREATE TRIGGER checkpoint_reject BEFORE INSERT ON thread WHEN NEW.title='${marker}' BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END`);
    d.close(); // Installed before the new hub opens SQLite or prepares statements.
    await start();
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}&since=0`);
    ws.binaryType = 'arraybuffer';
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('legacy greeting deadline')), 5000);
      ws.onerror = () => { clearTimeout(timer); reject(Error('disposable socket error')); };
      ws.onmessage = e => {
        try {
          const o = plain(cbor(new Uint8Array(e.data), keys, words));
          if (o?.t === 'log') { clearTimeout(timer); resolve(); }
          if (o?.t === 'reply' && o.id === pending?.id) pending.resolve(o);
        } catch (error) { pending?.reject(error); reject(error); }
      };
    });
    const failed = await request(77, marker);
    assert.equal(failed.ok, false); assert.equal(typeof failed.error, 'string');
    console.log('ok rejected durable commit explicitly completes RPC with an error');
    assert.equal(title(), 'Original');
    console.log('ok failed transaction preserves the canonical record');
    assert(!fs.readFileSync(path.join(home, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).map(JSON.parse).some(e => e.$ === 'ThreadRenamed' && e.title === marker));
    console.log('ok rejected append is absent from the authoritative log');
    const remove = db(); remove.exec('DROP TRIGGER checkpoint_reject'); remove.close();
    const accepted = await request(78, 'Accepted after refusal');
    assert.equal(accepted.ok, true); assert.equal(title(), 'Accepted after refusal');
    console.log('ok subsequent commit succeeds on the same socket');
    assert((await fetch(`http://127.0.0.1:${port}/hello`)).ok);
    console.log('ok service stays responsive after a rejected write');
  } finally { await stop(); fs.rmSync(root, { recursive: true, force: true }); }
})().catch(error => { console.error(error); process.exitCode = 1; });
