// Two isolated hubs: replace only the temporary owner's history while keeping
// its peer and thread IDs, then compare real thin-view cursor identities.
// bun test/tools/remote_replacement_probe.ts HUB WIREDIR
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ThinViews } from '../../src/web/thin.js';

const [bin, wire] = process.argv.slice(2);
if (!bin || !wire) throw new Error('usage: remote_replacement_probe.ts HUB WIREDIR');
for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), 'bp-replacement-probe-'));
const fake = join(root, 'bin'), owner = join(root, 'owner'), mirror = join(root, 'mirror'), project = join(root, 'project');
for (const p of [fake, owner, mirror, project]) mkdirSync(p);
for (const n of ['claude', 'codex', 'grok', 'muse']) { writeFileSync(join(fake, n), '#!/bin/sh\nexit 1\n'); chmodSync(join(fake, n), 0o755); }
const port = () => { const s = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const a = port(), b = port(), now = Math.floor(Date.now() / 1000);
const events = (label: string, at: number) => [
  { $: 'ProjectCreated', id: 'project', title: 'Example project', root: project, at },
  { $: 'ThreadCreated', id: 'thread', project: 'project', title: 'Example thread', env: 'local', branch: '', worktree: '', provider: 'claude', model: '', at },
  ...Array.from({length: 60}, (_, i) => ({ $: 'MessagePosted', thread: 'thread', msg: `${label}-${i}`, role: i % 2 ? 'assistant' : 'user', text: `${label} example message ${i}`, at: at + i })),
];
const writeEvents = (xs: any[]) => writeFileSync(join(owner, 'events.jsonl'), xs.map(x => JSON.stringify(x)).join('\n') + '\n');
writeEvents([{ $: 'SettingSet', key: 'restart.continue', value: 'off', at: now - 100 }, ...events('old', now - 60)]);
const spawn = (home: string, name: string, p: number, peer: number) => Bun.spawn([resolve(bin), '--home', home, '--port', String(p), '--host', '127.0.0.1', '--no-tailscale', '--headless', '--foreground'], {
  env: { PATH: `${fake}:/usr/bin:/bin`, HOME: root, DISPLAY: '', BACKPLANE_NAME: name, BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_PEERS: `http://127.0.0.1:${peer}`, BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_GOOGLE_BAKE: '0' }, stdout: 'ignore', stderr: 'ignore'
});
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(ms: number, f: () => T | undefined | Promise<T | undefined>): Promise<T> {
  const end = Date.now() + ms; while (Date.now() < end) { const x = await f(); if (x !== undefined) return x; await sleep(100); }
  throw new Error('isolated probe timed out');
}
async function ready(p: number) { await until(120000, async () => { try { return (await fetch(`http://127.0.0.1:${p}/hello`)).ok ? true : undefined; } catch { return undefined; } }); }
async function view(target: {kind: string; id: string}) {
  const token = readFileSync(join(mirror, 'token'), 'utf8').trim();
  const ws = new WebSocket(`ws://127.0.0.1:${b}/ws?view=1&frontend=1&token=${token}`); ws.binaryType = 'arraybuffer';
  const frames: any[] = []; ws.onmessage = e => frames.push(JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))));
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  try {
    const greeting: any = await until(15000, () => frames.find(f => f.t === 'view' && f.epoch && f.generation));
    const views = new ThinViews({ now: () => performance.now(), send: (_h: any, r: any) => r, command: (c: any) => c });
    if (!views.connect('fixture', greeting).connected) throw new Error('greeting rejected');
    const session = { sel: target.id, rid: 1, drafts: {}, open: {}, info: {}, adding: false, panel: false };
    const call = views.select('fixture', target, session, 'replacement-probe');
    if (call.failed || call.cmds.length !== 1) throw new Error('open was not issued');
    ws.send(W.encode(JSON.stringify(call.cmds[0])));
    const reply: any = await until(30000, () => frames.find(f => f.t === 'reply' && Number(f.id) === call.id));
    const accepted = views.reply('fixture', reply);
    if (!reply.ok || !accepted?.current || !reply.result?.window?.older) throw new Error('view missing accepted older cursor');
    const cursor = JSON.parse(reply.result.window.older);
    if (cursor.version !== 1 || cursor.query !== '' || cursor.target?.id !== target.id || !cursor.epoch) throw new Error('invalid cursor identity');
    return { greetingEpoch: greeting.epoch, cursorEpoch: cursor.epoch, before: cursor.before };
  } finally { ws.close(); }
}
let pa: ReturnType<typeof spawn> | undefined, pb: ReturnType<typeof spawn> | undefined;
try {
  pa = spawn(owner, 'owner', a, b); pb = spawn(mirror, 'mirror', b, a); await Promise.all([ready(a), ready(b)]);
  const peers = join(owner, 'secrets', 'peers');
  const link = await until(300000, () => { try { return readdirSync(peers)[0]; } catch { return undefined; } });
  const path = join(mirror, 'far', `${link}.jsonl`);
  await until(180000, () => { try { return readFileSync(path, 'utf8').includes('old-59') ? true : undefined; } catch { return undefined; } });
  const target = { kind: 'thread', id: `${link}~thread` };
  const first = await view(target);
  pa.kill('SIGTERM'); await pa.exited; pa = undefined;
  const peer = readFileSync(join(owner, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).map(s => JSON.parse(s)).find(x => x.$ === 'PeerSet' && x.id === link);
  if (!peer) throw new Error('fixture peer event missing');
  writeEvents([{ ...peer, at: now + 1 }, ...events('new', now + 1)]);
  pa = spawn(owner, 'owner', a, b); await ready(a);
  await until(180000, () => { try { const s = readFileSync(path, 'utf8'); return s.includes('new-59') && !s.includes('old-59') ? true : undefined; } catch { return undefined; } });
  const second = await view(target);
  if (first.greetingEpoch !== second.greetingEpoch) throw new Error('mirror hub epoch unexpectedly changed');
  if (first.cursorEpoch === second.cursorEpoch) throw new Error('owner replacement did not change cursor epoch');
  console.log(JSON.stringify({ sameLinkAndThread: true, sameMirrorGreetingEpoch: true, owningCursorEpochChanged: true, oldMirrorRowsRemoved: true, scope: 'real isolated hub transport and thin view; no browser or patched visual bridge' }, null, 2));
} finally {
  for (const p of [pa, pb]) if (p) { p.kill('SIGTERM'); await p.exited; }
  rmSync(root, { recursive: true, force: true });
}
