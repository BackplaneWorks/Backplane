// Eight bounded runtime regressions. BEFORE is the pre-fix binary, AFTER the
// candidate. Only temporary homes, stand-ins and captured process identities.
// bun test/tools/runlife_e2e.ts AFTER WIRE BEFORE [--full]
// --full additionally opts into the existing legacy mailtriage/adopt suites.
import { appendFileSync, chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
const NL = String.fromCharCode(10), TAB = String.fromCharCode(9);
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
const read = (p: string) => existsSync(p) ? readFileSync(p, 'utf8') : '';
function identity(pid: number) {
  try { const f = readFileSync(`/proc/${pid}/stat`, 'utf8').split(') ').at(-1)!.split(' '); return f[0] === 'Z' ? '' : f[19]; } catch { return ''; }
}
const alive = (p: any) => !!p && !!p.ident && identity(p.pid) === p.ident;
async function until(f: () => boolean, ms = 12000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (f()) return true; await sleep(40); }
  return false;
}

if (process.argv[2] === '--mcp-child') {
  const path = process.argv[3];
  const server = Bun.serve({ unix: path, fetch: () => new Response('fixture MCP') });
  process.on('SIGTERM', () => { server.stop(true); rmSync(path, { force: true }); process.exit(0); });
} else if (process.argv[2] === '--agent') {
  const log = (o: object) => appendFileSync(process.env.RUNLIFE_LOG!, JSON.stringify({ pid: process.pid, ident: identity(process.pid), ...o }) + NL);
  const sock = join(process.env.RUNLIFE_ROOT!, `mcp-${process.pid}.sock`);
  const child = Bun.spawn([process.execPath, resolve(import.meta.path), '--mcp-child', sock], { stdout: 'ignore', stderr: 'ignore' });
  await until(() => existsSync(sock));
  log({ event: 'start', child: { pid: child.pid, ident: identity(child.pid) }, sock });
  let closing = false;
  const finish = async () => {
    if (closing) return; closing = true;
    child.kill('SIGTERM'); await child.exited; log({ event: 'retired' }); process.exit(0);
  };
  process.on('SIGTERM', finish);
  const emit = (o: object) => console.log(JSON.stringify({ session_id: `fixture-${process.pid}`, ...o }));
  const lc = (u: string, state: string) => emit({ type: 'command_lifecycle', command_uuid: u, state });
  emit({ type: 'system', subtype: 'init', claude_code_version: '2.1.286', capabilities: ['msg_lifecycle_v1'] });
  const pending = new Set<string>();
  const rl = createInterface({ input: process.stdin });
  rl.on('close', finish);
  rl.on('line', async line => {
    let o: any; try { o = JSON.parse(line); } catch { return; }
    if (o.type === 'control_request' && o.request?.subtype === 'interrupt') {
      for (const u of pending) lc(u, 'cancelled'); pending.clear(); return;
    }
    if (o.type !== 'user') return;
    const text = typeof o.message?.content === 'string' ? o.message.content : (o.message?.content ?? []).map((b: any) => b.text ?? '').join(' ');
    const u = o.uuid; pending.add(u); lc(u, 'queued'); lc(u, 'started');
    log({ event: 'input', mode: text.includes('WAIT') ? 'wait' : text.includes('APPROVE') ? 'approval' : 'complete' });
    if (text.includes('APPROVE')) {
      const args = process.argv.slice(3), cfg = JSON.parse(args[args.indexOf('--mcp-config') + 1]);
      await fetch(cfg.mcpServers.backplane.url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'permission_prompt', arguments: { tool_name: 'Bash', input: { command: 'printf fixture' }, tool_use_id: 'fixture-approval' } } }) });
      log({ event: 'approval-returned' }); return;
    }
    if (text.includes('WAIT')) return;
    for (const done of (text.includes('COMPLETE_ALL') ? [...pending] : [u])) {
      emit({ type: 'result', subtype: 'success', is_error: false, result: 'fixture completed' });
      lc(done, 'completed'); pending.delete(done);
    }
  });
} else {
  const [afterArg, wireArg, beforeArg] = process.argv.slice(2);
  if (!afterArg || !wireArg || !beforeArg) throw Error('usage: AFTER WIRE BEFORE [--full]');
  const after = resolve(afterArg), before = resolve(beforeArg), wire = resolve(wireArg);
  for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), 'bp-runlife-')), home = join(root, 'home'), fake = join(root, 'fake'), prefix = join(root, 'install'), project = join(root, 'project');
  for (const p of [home, fake, prefix, project, join(prefix, 'web')]) mkdirSync(p);
  const logPath = join(root, 'agents.jsonl'), journal = join(home, 'events.jsonl'); writeFileSync(logPath, '');
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(fake, 'claude'), `#!/bin/sh${NL}exec ${quote(process.execPath)} ${quote(resolve(import.meta.path))} --agent "$@"${NL}`); chmodSync(join(fake, 'claude'), 0o755);
  for (const n of ['codex', 'grok', 'muse', 'agy', 'opencode']) { writeFileSync(join(fake, n), ['#!/bin/sh', 'exit 1', ''].join(NL)); chmodSync(join(fake, n), 0o755); }
  const ev = ($: string, o: object) => ({ $, ...o, at: 1 });
  const thread = (id: string) => ev('ThreadCreated', { id, project: 'p', title: id, env: 'local', branch: '', worktree: project, provider: 'claude', model: 'sonnet' });
  const privateIds = ['old', 'active', 'held', 'new1', 'new2', 'new3', 'killed'];
  const initial = [ev('ProjectCreated', { id: 'p', title: 'Fixture', root: project }), thread('foh'), thread('source'),
    ev('BotSet', { id: 'b', name: 'Bub fixture', thread: 'foh', look: 0, persona: '' }), ev('SettingSet', { key: 'uber.bot', value: 'b' }),
    ev('SettingSet', { key: 'mail.on', value: 'off' }), ev('SettingSet', { key: 'restart.continue', value: 'off' }),
    ev('SettingSet', { key: 'text.provider', value: 'claude' }),
    ...privateIds.flatMap(id => [thread(id), ev('RunSet', { thread: id, bot: 'b', conv: `c-${id}`, mail: `m-${id}` }),
      ev('SettingSet', { key: `run.bound.${id}`, value: `m-${id}` }),
      ...(['held', 'killed'].includes(id) ? [] : [ev('SettingSet', { key: `run.disposition.${JSON.stringify([id, `m-${id}`])}`, value: 'done' })]),
      ev('TurnChanged', { thread: id, state: 'completed' })])];
  writeFileSync(journal, initial.map(JSON.stringify).join(NL) + NL);
  copyFileSync(before, join(prefix, 'backplane')); chmodSync(join(prefix, 'backplane'), 0o755); writeFileSync(join(prefix, 'LICENSE'), 'fixture');
  const stage = (name: string) => { const d = join(root, name); mkdirSync(join(d, 'web'), { recursive: true }); copyFileSync(after, join(d, 'backplane')); chmodSync(join(d, 'backplane'), 0o755); writeFileSync(join(d, 'LICENSE'), name); return d; };
  const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); const port = listener.port; listener.stop(true);
  const env = { ...process.env, HOME: root, TMPDIR: root, DISPLAY: '', WAYLAND_DISPLAY: '', BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_GOOGLE_BAKE: '0', BACKPLANE_PEERS: '', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_LOCK_DIR: join(root, 'locks'), BACKPLANE_DEPLOY_LOCKS: join(root, 'deploy-lock'), RUNLIFE_LOG: logPath, RUNLIFE_ROOT: root };
  const proc = Bun.spawn([join(prefix, 'backplane'), '--home', home, '--port', String(port), '--no-tailscale', '--foreground'], { env, stdout: Bun.file(join(root, 'stdout')), stderr: Bun.file(join(root, 'stderr')) });
  const logs = () => read(logPath).trim().split(NL).filter(Boolean).map(l => JSON.parse(l));
  const events = () => read(journal).trim().split(NL).filter(Boolean).map(l => JSON.parse(l));
  const records = () => read(join(home, 'agents.pids')).trim().split(NL).filter(Boolean).map(l => { const [thread, pid, ident] = l.split(TAB); return { thread, pid: Number(pid), ident: ident.split(':').at(-1) }; });
  const record = (id: string) => records().find(r => r.thread === id);
  const resources = (id: string) => { const p = record(id), l = p && logs().find(l => l.pid === p.pid && l.event === 'start'); return p && l ? { ...p, ...l } : undefined; };
  const gone = (p: any) => !!p && !alive(p) && !alive(p.child) && !existsSync(p.sock) && !records().some(r => r.pid === p.pid);
  const state = (id: string) => events().filter(e => e.$ === 'TurnChanged' && e.thread === id).at(-1)?.state;
  let ws: WebSocket | undefined, request = 0, failures = 0;
  const check = (ok: boolean, label: string) => { console.log(`${ok ? 'ok' : 'FAIL'} ${label}`); if (!ok) failures++; };
  const connect = async () => { ws = new WebSocket(`ws://127.0.0.1:${port}/ws?since=999999999`); await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; }); };
  const send = (m: string, p: object) => ws!.send(W.encode(JSON.stringify({ id: ++request, m, p })));
  const input = (id: string, text: string) => send('turn.start', { thread: id, msg: `input-${++request}`, text });
  const upgrade = async (name: string) => {
    const d = stage(name); send('deploy.restart', { stage: d, note: 'isolated runlife regression' });
    if (!await until(() => !existsSync(d), 20000)) throw Error('isolated stage not applied');
    ws?.close(); await sleep(700); await connect();
  };
  try {
    let ready = false;
    for (let n = 0; n < 300; n++) { try { ready = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (ready) break; await sleep(40); }
    if (!ready) throw Error('isolated hub not ready');
    await connect();
    input('old', 'COMPLETE'); input('active', 'WAIT first'); input('held', 'WAIT held');
    send('thread.modes', { thread: 'source', runtime: 'approval-required' }); input('source', 'APPROVE');
    await until(() => state('old') === 'completed' && !!resources('old') && events().some(e => e.$ === 'AskOpened' && e.thread === 'source'));
    const old = resources('old'), active = resources('active'), held = resources('held'), source = resources('source');
    const ask = events().find(e => e.$ === 'AskOpened' && e.thread === 'source');
    check(alive(old) && alive(old?.child) && existsSync(old?.sock ?? ''), '1 old build reproduces completed SDK plus MCP socket retention');
    input('active', 'WAIT second accepted input'); send('turn.interrupt', { thread: 'held' });
    await until(() => state('held') === 'interrupted');
    await upgrade('fixed-stage');
    check(await until(() => gone(old)), '2 upgrade retires completed inherited SDK, MCP child, socket and registry entry');
    check(alive(active) && record('active')?.pid === active?.pid && state('active') === 'running' && logs().filter(l => l.pid === active?.pid && l.event === 'input').length === 2, '3 active SDK and accepted pending input survive adoption without duplication');
    check(!!ask && alive(source) && record('source')?.pid === source?.pid && !events().some(e => e.$ === 'AskClosed' && e.id === ask.id) && !logs().some(l => l.pid === source?.pid && l.event === 'approval-returned'), '4 legitimate source approval remains held and unanswered across upgrade');
    check(await until(() => gone(held)) && events().some(e => e.$ === 'SettingSet' && e.key.startsWith('triage.hold.') && e.value === 'held'), '5 stopped private SDK retires while durable conversation hold remains');
    let bounded = true;
    for (const id of ['new1', 'new2', 'new3']) {
      input(id, 'WAIT capture resources'); await until(() => !!resources(id)); const resource = resources(id);
      input(id, 'COMPLETE'); // first command must complete too before retirement
      // Cancel the earlier WAIT command through its own lifecycle, without user Stop.
      send('turn.start', { thread: id, msg: `flush-${id}`, text: 'COMPLETE_ALL' });
      bounded = (await until(() => gone(resource))) && bounded;
    }
    check(bounded, '6 repeated terminal runs release SDK, MCP child, sockets and adoption entries');
    input('killed', 'WAIT unfinished'); await until(() => !!resources('killed')); const killed = resources('killed');
    if (alive(killed)) process.kill(killed.pid, 'SIGTERM');
    check(await until(() => state('killed') === 'failed') && !events().some(e => e.$ === 'SettingSet' && e.key === `run.disposition.${JSON.stringify(['killed', 'm-killed'])}`), '7 externally killed unfinished work fails without inventing a receipt');
    const starts = logs().filter(l => l.event === 'start').length;
    await upgrade('fixed-stage-again'); await sleep(500);
    check(logs().filter(l => l.event === 'start').length === starts && !records().some(r => ['old', 'held', 'new1', 'new2', 'new3'].includes(r.thread)) && !events().slice(initial.length).some(e => e.$ === 'TurnChanged' && ['old', 'new1', 'new2', 'new3'].includes(e.thread) && e.state === 'failed'), '8 restart never revives finished sessions or emits false worker failures');
    writeFileSync(join(root, 'receipt.json'), JSON.stringify({ tests: 8, failures, registryCount: records().length, liveSdk: logs().filter(l => l.event === 'start' && alive(l)).length, liveMcp: logs().filter(l => l.event === 'start' && alive(l.child)).length }, null, 2));
  } finally {
    ws?.close();
    if (proc.exitCode === null) proc.kill('SIGTERM'); await proc.exited;
    for (const l of logs().filter(l => l.event === 'start')) {
      if (alive(l)) process.kill(l.pid, 'SIGTERM');
      if (alive(l.child)) process.kill(l.child.pid, 'SIGTERM');
    }
    console.log(`artifacts ${root}`);
  }
  if (!failures && process.argv.includes('--full')) {
    for (const f of ['mailtriage_e2e.ts', 'adopt_e2e.ts']) {
      const p = Bun.spawn([process.execPath, resolve('test/tools', f), after, wire], { stdout: 'inherit', stderr: 'inherit' });
      if (await p.exited) failures++;
    }
  }
  process.exit(failures ? 1 : 0);
}
