// Aggregate-only profile of a pair of isolated linked hubs. No real user data.
// bun test/tools/remote_transport_profile.ts HUB
import { createHmac } from 'node:crypto';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { gzipSync, gunzipSync } from 'node:zlib';

const binary = process.argv[2];
if (!binary) throw new Error('usage: remote_transport_profile.ts HUB');
const mirrorBinary = process.argv[3] || binary;
const version = Number(process.env.BP_FAR_VERSION || 3);
if (version !== 3 && version !== 4) throw new Error('BP_FAR_VERSION must be 3 or 4');
const delayMs = Number(process.env.BP_DELAY_MS || 0);
if (!Number.isFinite(delayMs) || delayMs < 0 || delayMs > 500) throw new Error('invalid BP_DELAY_MS');
const corpus = process.env.BP_CORPUS || 'conversation';
if (!['conversation', 'docs'].includes(corpus)) throw new Error('invalid BP_CORPUS');
const root = mkdtempSync(join(tmpdir(), 'bp-remote-profile-'));
const fake = join(root, 'bin'); mkdirSync(fake);
for (const n of ['claude', 'codex', 'grok', 'muse']) { writeFileSync(join(fake, n), '#!/bin/sh\nexit 1\n'); chmodSync(join(fake, n), 0o755); }
const port = () => { const s = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const pa = port(), pb = port(), a = join(root, 'a'), b = join(root, 'b'), proj = join(root, 'project');
for (const p of [a, b, proj]) mkdirSync(p);
const now = Math.floor(Date.now() / 1000);
const seed: any[] = [
  { $: 'SettingSet', key: 'restart.continue', value: 'off', at: now },
  { $: 'ProjectCreated', id: 'p', title: 'Example project', root: proj, at: now },
  { $: 'ThreadCreated', id: 't', project: 'p', title: 'Example conversation', env: 'local', branch: '', worktree: '', provider: 'claude', model: '', at: now },
];
const examples = [
  'Please inspect the latest change and explain what remains before release.',
  'The build completed successfully. I checked the configuration, then ran the focused tests. The remaining issue is the reconnect path when the network briefly drops.',
  'I will compare the two results and report the timings.\n\n- Verify the first page\n- Reopen the thread\n- Check that the draft is still present',
  'Thanks. Please continue with the next change and include the result of the regression test.',
];
// Public repository prose supplies diverse lengths and content; never read a
// person's events or log. Truncate each passage to a normal message size.
const docs = readFileSync('README.md', 'utf8').split(/\n\s*\n/).map(s => s.trim()).filter(s => s.length >= 60).map(s => s.slice(0, 1000));
const texts = corpus === 'docs' ? docs : examples;
if (!texts.length) throw new Error('empty public fixture corpus');
for (let i = 0; i < 1000; i++) seed.push({ $: 'MessagePosted', thread: 't', msg: `m${i}`, role: i % 2 ? 'assistant' : 'user', text: `${texts[i % texts.length]} (${i})`, at: now - 1000 + i });
writeFileSync(join(a, 'events.jsonl'), seed.map(x => JSON.stringify(x)).join('\n') + '\n');
const spawn = (exe: string, home: string, name: string, p: number, peer: number) => Bun.spawn([resolve(exe), '--home', home, '--port', String(p), '--host', '127.0.0.1', '--no-tailscale', '--headless', '--foreground'], {
  env: { PATH: `${fake}:/usr/bin:/bin`, HOME: root, DISPLAY: '', BACKPLANE_NAME: name, BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_PEERS: `http://127.0.0.1:${peer}`, BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_GOOGLE_BAKE: '0' }, stdout: 'ignore', stderr: 'ignore'
});
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(ms: number, f: () => Promise<T | undefined> | T | undefined): Promise<T> {
  const end = Date.now() + ms;
  while (Date.now() < end) { const x = await f(); if (x !== undefined) return x; await sleep(100); }
  throw new Error('timed out waiting for isolated hubs');
}
let one: ReturnType<typeof spawn> | undefined, two: ReturnType<typeof spawn> | undefined, proxy: ReturnType<typeof Bun.serve> | undefined;
try {
  const start = performance.now(); one = spawn(binary, a, 'alpha', pa, pb); two = spawn(mirrorBinary, b, 'beta', pb, pa);
  await Promise.all([pa, pb].map(p => until(120000, async () => { try { return (await fetch(`http://127.0.0.1:${p}/hello`)).ok ? true : undefined; } catch { return undefined; } })));
  const readyMs = performance.now() - start;
  const cpu = (pid: number) => { const s = readFileSync(`/proc/${pid}/stat`, 'utf8'); const f = s.slice(s.lastIndexOf(')') + 2).split(' '); return Number(f[11]) + Number(f[12]); };
  const cpuStart = { owner: cpu(one.pid), mirror: cpu(two.pid) };
  const dir = join(a, 'secrets', 'peers');
  const link = await until(300000, () => { try { return readdirSync(dir)[0]; } catch { return undefined; } });
  const secret = JSON.parse(readFileSync(join(dir, link), 'utf8')).secret;
  if (delayMs) proxy = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const data = await request.arrayBuffer(); await sleep(delayMs);
    const res = await fetch(`http://127.0.0.1:${pa}/far/log`, { method: 'POST', headers: request.headers, body: data });
    const result = await res.arrayBuffer(); await sleep(delayMs);
    return new Response(result, { status: res.status, headers: { 'content-type': 'application/json' } });
  } });
  let nonce = 0, epoch = '';
  let encodedResponses = 0;
  async function pull(since: number, v: number) {
    const body = JSON.stringify({ since, epoch, v, nonce: `${Date.now()}-${++nonce}` });
    const ts = Math.floor(Date.now() / 1000);
    const sig = 'sha256=' + createHmac('sha256', Buffer.from(secret, 'hex')).update(`${ts}.${body}`).digest('hex');
    const t = performance.now();
    const r = await fetch(`http://127.0.0.1:${proxy?.port || pa}/far/log`, { method: 'POST', body, headers: { 'content-type': 'application/json', 'x-backplane-peer': link, 'x-backplane-timestamp': String(ts), 'x-backplane-signature': sig } });
    const wireText = await r.text();
    let text = wireText;
    if (r.ok) {
      const obj = JSON.parse(wireText);
      if (obj.farEncoding === 'gzip-base64-v1') {
        encodedResponses++;
        text = gunzipSync(Buffer.from(obj.data, 'base64'), { maxOutputLength: 1048576 }).toString('utf8');
      }
    }
    return { status: r.status, requestBytes: Buffer.byteLength(body), responseBytes: Buffer.byteLength(wireText), ms: performance.now() - t, text };
  }
  const initial = await until(300000, async () => { const x = await pull(0, version); return x.status === 200 ? x : undefined; });
  const mirrorApplied = await until(180000, () => {
    try { return readFileSync(join(b, 'far', `${link}.jsonl`), 'utf8').includes('"msg":"m999"') ? true : undefined; }
    catch { return undefined; }
  });
  // This is a local codec experiment, not a claim about bytes sent by the hub.
  // Retain no fixture content and verify exact UTF-8 recovery.
  const compression = (text: string) => {
    const plain = Buffer.from(text), start = performance.now(), zipped = gzipSync(plain, { level: 1 });
    const encodeMs = performance.now() - start, decodeStart = performance.now();
    if (!gunzipSync(zipped).equals(plain)) throw new Error('gzip roundtrip mismatch');
    return { inputBytes: plain.length, gzipBytes: zipped.length, encodeMs, decodeMs: performance.now() - decodeStart };
  };
  const snapshot = JSON.parse(initial.text); epoch = snapshot.epoch;
  if (!Array.isArray(snapshot.items) || !snapshot.head || snapshot.more) throw new Error('unexpected snapshot');
  // Starting at the first sequence with the authoritative epoch exercises
  // the paged history, including exact sequence and payload fidelity.
  let since = 1, requests = 0, requestBytes = 0, responseBytes = 0, elapsedMs = 0, previous = 1;
  const compressedHistory: ReturnType<typeof compression>[] = [];
  const got = new Map<number, any>();
  do {
    const r = await pull(since, version); if (r.status !== 200) throw new Error(`pull status ${r.status}`);
    compressedHistory.push(compression(r.text));
    requests++; requestBytes += r.requestBytes; responseBytes += r.responseBytes; elapsedMs += r.ms;
    const page = JSON.parse(r.text);
    for (const item of page.items) { if (item.n <= previous) throw new Error('out of order or duplicate'); previous = item.n; got.set(item.n, item.c); }
    if (!page.more) break;
    if (page.head <= since) throw new Error('page did not advance'); since = page.head;
  } while (true);
  for (let i = 1; i < seed.length; i++) if (!isDeepStrictEqual(got.get(i + 1), seed[i])) throw new Error(`history mismatch at sequence ${i + 1}`);
  const current = await pull(snapshot.head, version);
  if (current.status !== 200 || JSON.parse(current.text).items.length !== 0) throw new Error('caught-up pull is not empty');
  if (process.env.BP_EXPECT_ENCODED === '1' && encodedResponses < 2) throw new Error('expected compressed snapshot and history responses');
  if (process.env.BP_EXPECT_ENCODED === '0' && encodedResponses !== 0) throw new Error('unexpected compressed response');
  const memory = (pid: number) => { const s = readFileSync(`/proc/${pid}/status`, 'utf8'); return Number(s.match(/^VmRSS:\s+(\d+)/m)?.[1] ?? 0); };
  console.log(JSON.stringify({ workload: { projects: 1, threads: 1, messages: 1000, generated: true, corpus }, version, injectedDelayEachDirectionMs: delayMs, encodedResponses, mirrorApplied, startupMs: readyMs, snapshot: { requestBytes: initial.requestBytes, responseBytes: initial.responseBytes, ms: initial.ms, items: snapshot.items.length }, history: { requests, requestBytes, responseBytes, elapsedMs, items: got.size, exact: true }, caughtUp: { requestBytes: current.requestBytes, responseBytes: current.responseBytes, ms: current.ms }, offlineGzipLevel1: { snapshot: compression(initial.text), history: compressedHistory, caughtUp: compression(current.text) }, cpuTicksSinceStartup: { owner: cpu(one.pid) - cpuStart.owner, mirror: cpu(two.pid) - cpuStart.mirror }, rssKiBAtEnd: { owner: memory(one.pid), mirror: memory(two.pid) }, scope: 'HTTP bodies only; direct pull uses loopback or a delay proxy; mirror replication is direct; current RSS; CPU includes pairing and background hub activity; offlineGzipLevel1 is codec sizing, not network transfer' }, null, 2));
} finally {
  proxy?.stop(true);
  for (const p of [one, two]) if (p) { p.kill('SIGTERM'); await p.exited; }
  rmSync(root, { recursive: true, force: true });
}
