// Two disposable hubs, loopback-only peers, synthetic journals and provider
// stand-ins. No live home, settings, credentials, telemetry or person messages.
// bun test/tools/mailowner_e2e.ts HUB WIRE_DIR
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createHmac, randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
const pause = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until(f: () => unknown, ms = 20000) {
  const end = Date.now() + ms;
  do { if (f()) return true; await pause(40); } while (Date.now() < end);
  return false;
}
async function agent() {
  const args = process.argv.slice(3), cfg = JSON.parse(args[args.indexOf('--mcp-config') + 1]);
  const url = cfg.mcpServers.backplane.url;
  const log = (data: any) => appendFileSync(process.env.OWNER_LOG!, JSON.stringify({ site: process.env.OWNER_SITE, ...data }) + '\n');
  const emit = (x: any) => console.log(JSON.stringify({ session_id: `owner-${process.pid}`, uuid: randomUUID(), ...x }));
  emit({ type: 'system', subtype: 'init', tools: [], mcp_servers: [], capabilities: ['msg_lifecycle_v1'] });
  let chain = Promise.resolve();
  createInterface({ input: process.stdin }).on('line', line => {
    let o: any; try { o = JSON.parse(line); } catch { return; }
    if (o.type !== 'user') return;
    const text = (Array.isArray(o.message.content) ? o.message.content : [o.message.content]).map((x: any) => typeof x === 'string' ? x : x.text ?? '').join(' ');
    chain = chain.then(async () => {
      const input = text.match(/Current actionable input: (\S+)/)?.[1];
      if (!input) throw Error('stand-in was launched outside private mail triage');
      const current = text.split(`\n[mail ${input} from `)[1]?.split(/\n\[mail |<\/mail_history>/)[0] ?? text;
      log({ input, prompt: current });
      emit({ type: 'command_lifecycle', command_uuid: o.uuid, state: 'started' });
      if (current.includes('OWNER_GATE_FIRST')) {
        if (!await until(() => existsSync(process.env.OWNER_GATE!), 30000)) throw Error('test gate timed out');
      }
      const result = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: randomUUID(), method: 'tools/call', params: { name: 'mail_dispose', arguments: { input, mode: 'done', reason: 'synthetic unchanged FYI' } } }) });
      const reply = await result.text();
      if (!result.ok || reply.includes('"isError":true')) throw Error(`disposition failed: ${reply}`);
      log({ disposed: input });
      emit({ type: 'result', subtype: 'success', is_error: false, result: '', usage: {} });
      emit({ type: 'command_lifecycle', command_uuid: o.uuid, state: 'completed' });
    }).catch(e => { log({ error: String(e) }); process.exit(1); });
  });
}

if (process.argv[2] === '--agent') await agent();
else {
  const binary = resolve(process.argv[2]), wire = resolve(process.argv[3]);
  for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), 'bp-owner-mail-')), fake = join(root, 'agents'), logPath = join(root, 'agents.jsonl'), gate = join(root, 'gate');
  mkdirSync(fake); writeFileSync(logPath, '');
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) {
    writeFileSync(join(fake, name), name === 'claude' ? `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve(import.meta.path))} --agent "$@"\n` : '#!/bin/sh\nexit 127\n');
    chmodSync(join(fake, name), 0o755);
  }
  const port = () => { const s = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
  type Hub = { site: string; home: string; port: number; proc?: ReturnType<typeof Bun.spawn>; ws?: WebSocket; next: number; replies: Map<number, any> };
  const a: Hub = { site: 'A', home: join(root, 'A'), port: port(), next: 0, replies: new Map() };
  const b: Hub = { site: 'B', home: join(root, 'B'), port: port(), next: 0, replies: new Map() };
  const uid = 'owner-mail-fixture';
  const parseLines = (p: string) => readFileSync(p, 'utf8').trim().split('\n').flatMap(l => { try { return [JSON.parse(l)]; } catch { return []; } });
  const events = (h: Hub) => parseLines(join(h.home, 'events.jsonl'));
  const logs = () => parseLines(logPath);
  const setting = (h: Hub, key: string) => events(h).filter(e => e.$ === 'SettingSet' && e.key === key).at(-1)?.value;
  const leases = (h: Hub) => events(h).filter(e => e.$ === 'UberOp').map(e => JSON.parse(e.op)).filter(e => e.k === 'lease');
  const seed = (h: Hub) => {
    mkdirSync(h.home); mkdirSync(join(h.home, 'bot'));
    const ev = ($: string, x: any) => ({ $, at: 1, ...x });
    const op = (origin: string, n: number, data: any) => ev('UberOp', { uid, origin, n, stamp: `0000000001.${String(n).padStart(6, '0')}.${origin}`, op: JSON.stringify(data) });
    writeFileSync(join(h.home, 'events.jsonl'), [
      ev('ThreadCreated', { id: `foh${h.site}`, project: '', title: 'Synthetic owner', env: 'local', branch: '', worktree: join(h.home, 'bot'), provider: 'claude', model: 'sonnet' }),
      ev('BotSet', { id: `bot${h.site}`, name: 'FixtureOwner', thread: `foh${h.site}`, look: 0, persona: 'Handle synthetic FYI silently.' }),
      ev('SettingSet', { key: 'machine.id', value: `m${h.site}` }), ev('UberSet', { bot: `bot${h.site}`, uid }),
      ...['A', 'B'].map(site => op(`m${site}`, 1, { k: 'bind', machine: `m${site}`, bot: `bot${site}`, thread: `foh${site}`, name: 'FixtureOwner' })),
      op('mA', 2, { k: 'lease', holder: 'mA', term: 1 }),
      ...[['mail.on', 'off'], ['restart.continue', 'off'], ['text.provider', 'claude'], ['text.model', 'sonnet']].map(([key, value]) => ev('SettingSet', { key, value })),
    ].map(JSON.stringify).join('\n') + '\n');
  };
  const start = async (h: Hub, peer: Hub) => {
    h.proc = Bun.spawn([binary, '--home', h.home, '--port', String(h.port), '--no-tailscale', '--foreground'], { env: { ...process.env,
      HOME: h.home, PATH: `${fake}:/usr/bin:/bin`, BACKPLANE_NAME: `owner-${h.site}`, BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_NO_UPDATE: '1',
      BACKPLANE_TELEMETRY: 'off', BACKPLANE_GOOGLE_BAKE: '0', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_PEERS: `http://127.0.0.1:${peer.port}`,
      BACKPLANE_UBER_GRACE: '3600', BACKPLANE_WAKE_TICK_MS: '250', BACKPLANE_LOCK_DIR: join(h.home, 'locks'), BACKPLANE_DEPLOY_LOCKS: join(h.home, 'deploy'),
      OWNER_SITE: h.site, OWNER_LOG: logPath, OWNER_GATE: gate, DISPLAY: '', WAYLAND_DISPLAY: '' }, stdout: Bun.file(join(h.home, 'stdout')), stderr: Bun.file(join(h.home, 'stderr')) });
    let ready = false;
    for (let i = 0; i < 250 && !ready; i++) { try { ready = (await fetch(`http://127.0.0.1:${h.port}/hello`)).ok; } catch {} if (!ready) await pause(40); }
    if (!ready) throw Error(`hub ${h.site} did not start`);
    h.ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws?since=999999999`);
    h.ws.binaryType = 'arraybuffer';
    h.ws.onmessage = e => { const o = JSON.parse(W.decode(new Uint8Array(e.data))); if (o.t === 'reply') h.replies.set(Number(o.id), o); };
    await new Promise((ok, no) => { h.ws!.onopen = ok; h.ws!.onerror = no; });
  };
  const rpc = async (h: Hub, m: string, p: any) => { const id = ++h.next; h.ws!.send(W.encode(JSON.stringify({ id, m, p }))); if (!await until(() => h.replies.has(id))) throw Error(`RPC ${m} timeout`); const r = h.replies.get(id); if (!r.ok) throw Error(`RPC ${m}: ${JSON.stringify(r)}`); return r; };
  const mail = (h: Hub, text: string) => rpc(h, 'mail.send', { to: `bot:bot${h.site}`, text, subject: 'Synthetic waiting FYI', want: 'fyi', conv: 'owner-test-queue' });
  const stop = async (h: Hub) => { h.ws?.close(); if (h.proc?.exitCode === null) h.proc.kill('SIGTERM'); if (h.proc) await h.proc.exited; };
  let failures = 0;
  const check = (name: string, ok: boolean) => { console.log(`${ok ? 'ok' : 'FAIL'} ${name}`); if (!ok) failures++; };
  try {
    seed(a); seed(b); await start(a, b); await start(b, a);
    const invite = await rpc(a, 'bots.invite', { url: `http://127.0.0.1:${a.port}` }); await rpc(b, 'bots.join', { invite: invite.invite });
    check('isolated replicas learn owner links', await until(() => setting(a, `uber.link.${uid}.mB`) && setting(b, `uber.link.${uid}.mA`), 150000));
    await mail(a, 'OWNER_GATE_FIRST');
    check('same owner binds first input', await until(() => logs().some(e => e.site === 'A' && e.prompt?.includes('OWNER_GATE_FIRST'))));
    await mail(a, 'OWNER_WAITING_SECOND');
    check('second input is durably queued', await until(() => events(a).some(e => e.$ === 'SettingSet' && e.key.startsWith('triage.queue.') && e.value !== '[]')));
    await rpc(b, 'bots.uber_home', {});
    check('lease transition reaches origin', await until(() => leases(a).at(-1)?.holder === 'mB'));
    check('queued input follows remote owner', await until(() => logs().some(e => e.site === 'B' && e.prompt?.includes('OWNER_WAITING_SECOND'))));
    check('old holder did not start waiting input', !logs().some(e => e.site === 'A' && e.prompt?.includes('OWNER_WAITING_SECOND')));
    writeFileSync(gate, 'release');
    check('already running input finishes on original run', await until(() => logs().some(e => e.site === 'A' && e.disposed)));
    await mail(a, 'OWNER_REMOTE_THIRD');
    check('new remote-owner mail uses holder', await until(() => logs().some(e => e.site === 'B' && e.prompt?.includes('OWNER_REMOTE_THIRD'))));
    const attempted = events(a).filter(e => e.$ === 'SettingSet' && e.key.startsWith('owner.mail.pending.') && e.value).map(e => JSON.parse(e.value)).findLast(r => r.to === 'mB' && r.mail.head.includes('OWNER_REMOTE_THIRD'));
    if (!attempted) throw Error('no durable remote attempt');
    const sourceBody = readdirSync(join(a.home, 'mail')).filter(f => f.endsWith('.jsonl')).flatMap(f => parseLines(join(a.home, 'mail', f))).find(r => r.mail.id === attempted.id);
    const link = setting(b, `uber.link.${uid}.mA`), secret = JSON.parse(readFileSync(join(b.home, 'secrets', 'peers', link), 'utf8')).secret;
    const retry = async () => {
      const packet = JSON.stringify({ n: randomUUID(), m: 'owner.mail', p: { uid, sender: 'mA', term: attempted.lease, ticket: attempted, body: sourceBody, hold: '' } });
      const ts = Math.floor(Date.now() / 1000), sig = 'sha256=' + createHmac('sha256', Buffer.from(secret, 'hex')).update(`${ts}.${packet}`).digest('hex');
      return fetch(`http://127.0.0.1:${b.port}/far/rpc`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-backplane-peer': link, 'x-backplane-timestamp': String(ts), 'x-backplane-signature': sig }, body: packet });
    };
    const before = logs().filter(e => e.prompt?.includes('OWNER_REMOTE_THIRD')).length;
    check('duplicate exact handoff returns durable ACK', (await retry()).status === 200);
    await rpc(a, 'bots.uber_home', {});
    check('lease returns to origin', await until(() => leases(b).at(-1)?.holder === 'mA'));
    check('old receipt answers lost ACK after move', (await retry()).status === 200);
    check('handoff retry creates no second run', logs().filter(e => e.prompt?.includes('OWNER_REMOTE_THIRD')).length === before);
    await stop(a); await start(a, b);
    check('sender restart preserves dedupe', logs().filter(e => e.prompt?.includes('OWNER_REMOTE_THIRD')).length === before);
    check('stand-ins never post in either person conversation', [a, b].every(h => !events(h).some(e => e.$ === 'MessagePosted' && e.thread === `foh${h.site}` && e.role === 'assistant')));
    check('stand-ins report no errors', !logs().some(e => e.error));
  } catch (e) { console.error(e); failures++; }
  finally { await stop(a); await stop(b); if (!failures && process.env.OWNER_E2E_KEEP !== '1') rmSync(root, { recursive: true, force: true }); else console.log(`fixture retained: ${root}`); }
  process.exit(failures ? 1 : 0);
}
