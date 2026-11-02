// The real hub's stop -> same-session Codex resume path, with a delayed
// writer. No live home or account is read. Windows tree mechanics are tested
// separately by process_tree_test.mjs (including wrapper and grandchildren).
// bun test/tools/codex_steer_e2e.ts BINARY WIREDIR
import assert from 'node:assert/strict';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const [bin, wire] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const until = async (what: string, f: () => any) => {
  const end = Date.now() + 15000;
  while (Date.now() < end) { const value = f();if (value) return value;await sleep(30); }
  throw Error('timeout: ' + what);
};
const root = mkdtempSync(join(tmpdir(), 'bp-codex-steer-'));
const fake = join(root, 'bin'), home = join(root, 'home'), proj = join(root, 'project');
for (const p of [fake, home, proj]) mkdirSync(p);
// Quote paths as literal POSIX shell words; no expansion of fixture paths.
const q = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
writeFileSync(join(fake, 'codex'), '#!/bin/sh\nexec ' + q(process.execPath) + ' ' + q(resolve('test/tools/codex_steer_standin.ts')) + ' "$@"\n');
chmodSync(join(fake, 'codex'), 0o755);
for (const name of ['claude', 'grok', 'muse']) { writeFileSync(join(fake, name), '#!/bin/sh\nexit 1\n');chmodSync(join(fake, name), 0o755); }
const port = (() => { const s = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } });const p = s.port;s.stop(true);return p; })();
const hub = Bun.spawn([resolve(bin), '--home', home, '--port', String(port), '--no-tailscale', '--foreground'], {
  env: { ...process.env, DISPLAY: '', WAYLAND_DISPLAY: '', BACKPLANE_NO_UPDATE: '1', BACKPLANE_PEERS: '', BACKPLANE_AGENTS: fake,
    BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_TELEMETRY: 'off', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9',
    BACKPLANE_KEYRING_LOCKED: '0', STEER_FIXTURE: root },
  stdout: 'ignore', stderr: Bun.file(join(root, 'hub.err')),
});
const records = () => existsSync(join(root, 'agent.jsonl')) ? readFileSync(join(root, 'agent.jsonl'), 'utf8').trim().split('\n').filter(Boolean).map(s => JSON.parse(s)) : [];
let ws: WebSocket | undefined;
try {
  for (let i = 0; i < 300; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(30); }
  const changes: any[] = [], replies = new Map<string, any>();
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);ws.binaryType = 'arraybuffer';
  ws.onmessage = e => {
    const o = JSON.parse(typeof e.data === 'string' ? e.data : W.decode(new Uint8Array(e.data)));
    for (const c of o.items ?? []) changes.push(c);
    if (o.id !== undefined) replies.set(String(o.id), o);
  };
  await new Promise((resolve, reject) => { ws!.onopen = resolve;ws!.onerror = reject; });
  let id = 0;
  const send = (m: string, p: object) => { const n = String(++id);ws!.send(W.encode(JSON.stringify({ id: n, m, p })));return n; };
  send('project.add', { path: proj });
  const project = await until('project', () => changes.find(c => c.$ === 'ProjectCreated' && c.root === proj));
  send('thread.create', { project: project.id, title: 'slow Codex writer', env: 'local', provider: 'codex' });
  const thread = (await until('thread', () => changes.find(c => c.$ === 'ThreadCreated' && c.title === 'slow Codex writer'))).id;
  send('turn.start', { thread, text: 'FIRST', msg: 'first' });
  await until('first writer and session', () => records().filter(r => r.ev === 'start').length === 1 && changes.some(c => c.$ === 'SessionSet' && c.id === thread));
  // Allow TurnPid and the session event to be durably handled before steering.
  await sleep(100);
  for (let turn = 1; turn <= 3; turn++) {
    send('turn.start', { thread, text: 'STEER-' + turn, msg: 'steer-' + turn, mode: 'steer' });
    await until('old writer begins shutdown', () => records().filter(r => r.ev === 'stopping').length === turn);
    // A request on the same hub must still complete while cleanup is pending.
    const request = send('setting.set', { key: 'fixture.steer-ping', value: String(turn) });
    await until('hub remains responsive during handoff', () => replies.has(request));
    assert.equal(records().filter(r => r.ev === 'start').length, turn, 'replacement waits through slow shutdown');
    await until('replacement writer', () => records().filter(r => r.ev === 'start').length === turn + 1);
    const starts = records().filter(r => r.ev === 'start'), exited = records().filter(r => r.ev === 'exited');
    assert.equal(exited.length, turn);assert.ok(exited[turn - 1].at <= starts[turn].at);
    assert.deepEqual(starts[turn].args.slice(0, 3), ['exec', 'resume', 'fixture-session']);
    assert.ok(starts[turn].args.at(-1).includes('STEER-' + turn));
    assert.equal(records().some(r => r.ev === 'conflict'), false);
    assert.equal(changes.some(c => c.$ === 'TurnChanged' && c.thread === thread && /failed/i.test(String(c.state))), false);
  }
  send('turn.start', { thread, text: 'SUPERSEDED', msg: 'rapid-old', mode: 'steer' });
  await until('rapid handoff starts', () => records().filter(r => r.ev === 'stopping').length === 4);
  send('turn.start', { thread, text: 'LATEST-STEER', msg: 'rapid-new', mode: 'steer' });
  await until('latest rapid steer starts', () => records().filter(r => r.ev === 'start').length === 5);
  assert.ok(records().filter(r => r.ev === 'start').at(-1).args.at(-1).includes('LATEST-STEER'));
  assert.equal(records().some(r => r.ev === 'conflict'), false);
  assert.equal(changes.some(c => c.$ === 'TurnChanged' && c.thread === thread && /failed/i.test(String(c.state))), false);
  send('turn.interrupt', { thread });
  await until('last writer exits', () => records().filter(r => r.ev === 'exited').length === 5);
  console.log('ok real hub: repeated and rapid same-session steers serialize writer exit, preserve the latest input and keep the hub responsive');
} catch (error) {
  console.error(readFileSync(join(root, 'hub.err'), 'utf8').slice(-5000));console.error(JSON.stringify(records()));throw error;
} finally {
  ws?.close();
  // Only PIDs captured from our own fixture; no process-name or user-chat access.
  for (const r of records().filter(r => r.ev === 'start')) { try { process.kill(r.pid, 'SIGTERM'); } catch {} }
  await sleep(700);
  hub.kill('SIGTERM');await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
