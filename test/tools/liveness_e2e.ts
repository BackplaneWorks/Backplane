// Real hub watchdog/effect integration, isolated home and echo-only agents.
// bun test/tools/liveness_e2e.ts [BINARY] [WIREDIR]
// Every wait shares a 70-second deadline. Fixture agents exit naturally on
// EOF, the cleanup marker, or their own 55-second lifetime; only captured
// throwaway hub PIDs are terminated to exercise crash recovery.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const [binary = 'build/backplane', wire = 'build/wire'] = process.argv.slice(2);
for (const name of fs.readdirSync(wire).filter(f => f.endsWith('.js')))
  (0, eval)(fs.readFileSync(join(wire, name), 'utf8'));
const W = (globalThis as any).Wire;
assert(W?.encode && W?.decode, 'compiled wire codec required');
const root = fs.mkdtempSync(join(tmpdir(), 'bp-liveness-e2e-'));
const home = join(root, 'home'), project = join(root, 'project'), fake = join(root, 'bin');
const journal = join(home, 'events.jsonl'), log = join(root, 'fixture.jsonl'), finish = join(root, 'finish');
for (const dir of [home, project, fake, join(root, 'locks')]) fs.mkdirSync(dir);
const quote = (s: string) => "'" + s.replace(/'/g, "'\\''") + "'";
const fixture = join(root, 'echo.cjs');
fs.writeFileSync(fixture, `
const fs=require('node:fs'),rl=require('node:readline');
if(process.argv.includes('--version')) { console.log('2.0.0'); process.exit(0); }
const args=process.argv.slice(2),mi=args.indexOf('--mcp-config');
let url='';try{url=JSON.parse(args[mi+1]).mcpServers.backplane.url}catch{}
const thread=(/\\/mcp\\/([^.?/]+)/.exec(url)||[])[1]||'';
const log=x=>fs.appendFileSync(process.env.FIXTURE_LOG,JSON.stringify({pid:process.pid,thread,...x})+'\\n');
const emit=x=>process.stdout.write(JSON.stringify(x)+'\\n');
process.stdout.on('error',()=>{});log({event:'start'});
const session='echo-'+process.pid;let initialized=false;
const input=rl.createInterface({input:process.stdin});
input.on('line',line=>{let x;try{x=JSON.parse(line)}catch{return}
 if(x.type==='control_request'){log({event:'control',subtype:x.request?.subtype});return}
 if(x.type!=='user')return;
 if(!initialized){initialized=true;emit({type:'system',subtype:'init',session_id:session,tools:[],model:'fake',claude_code_version:'2.0.0',capabilities:[]})}
 log({event:'echo',text:JSON.stringify(x.message)});
 emit({type:'user',message:x.message,uuid:x.uuid,session_id:session});
});
input.on('close',()=>{log({event:'eof'});process.exit(0)});
process.on('SIGTERM',()=>{log({event:'term'});process.exit(0)});
setInterval(()=>{if(fs.existsSync(process.env.FIXTURE_FINISH)){log({event:'finished'});process.exit(0)}},50);
setTimeout(()=>{log({event:'ttl'});process.exit(0)},55000);
`);
fs.writeFileSync(join(fake, 'claude'), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(fixture)} "$@"\n`);
fs.chmodSync(join(fake, 'claude'), 0o755);
for (const provider of ['codex', 'grok', 'muse', 'antigravity', 'opencode', 'pi', 'omp']) {
  fs.writeFileSync(join(fake, provider), '#!/bin/sh\nexit 127\n'); fs.chmodSync(join(fake, provider), 0o755);
}
const now = Math.floor(Date.now() / 1000), old = now - 1300;
const event = ($: string, fields: object, at = now) => ({ $, ...fields, at });
const thread = (id: string) => event('ThreadCreated', { id, project: 'p', title: id, env: 'local', branch: '', worktree: project, provider: 'claude', model: 'fake' }, old);
fs.writeFileSync(journal, [
  event('ProjectCreated', { id: 'p', title: 'Liveness fixture', root: project }, old),
  thread('worker'), thread('launcher'), thread('dependency'),
  event('MessagePosted', { thread: 'worker', msg: 'request', role: 'user', text: 'Wait for the dependency and finish the project' }, old),
  event('TurnChanged', { thread: 'worker', state: 'completed' }, old),
  event('ThreadLaunched', { id: 'worker', by: 'launcher', report: true }, old),
  event('WakeSet', { id: 'waiting', by: 'worker', kind: 'thread', target: 'dependency', events: ['end'], once: true, note: 'waiting for dependency' }, old),
  event('SettingSet', { key: 'liveness.worker', value: JSON.stringify({last: old, nudged: 0, stalled: false, reported: false, reason: 'waiting for dependency fixture'}) }, old),
  ...[['watchdog.minutes', '20'], ['watchdog.grace.seconds', '0'], ['mail.on', 'off'], ['telemetry.share', 'off'],
    ['settle.done', 'off'], ['restart.continue', 'on']].map(([key, value]) => event('SettingSet', { key, value }))
].map(JSON.stringify).join('\n') + '\n');
fs.writeFileSync(join(home, 'models.cache'), JSON.stringify({ claude: ['fake'], codex: [], grok: [], muse: [], antigravity: [] }) + '\nFixture providers\n');
// Keep the initial project empty: fixture creation would have a fresh ctime
// newer than the deliberately old durable progress watermark.
const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true);
assert.notEqual(port, 3787, 'refusing the live hub port');
// Reserve six seconds for captured-hub cleanup within the total budget.
const deadline = Date.now() + 64000;
const delay = (ms: number) => new Promise<void>(r => setTimeout(r, ms));
const read = (path: string) => fs.existsSync(path) ? fs.readFileSync(path, 'utf8') : '';
const records = (path: string) => read(path).split('\n').flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } });
const events = () => records(journal), agents = () => records(log);
const live = () => {
  const value = events().filter(e => e.$ === 'SettingSet' && e.key === 'liveness.worker').at(-1)?.value;
  try { return JSON.parse(value); } catch { return {}; }
};
const nudges = () => events().filter(e => e.$ === 'MessagePosted' && e.thread === 'worker' && /^You appear stalled/.test(e.text));
const reports = () => events().filter(e => e.$ === 'MessagePosted' && e.thread === 'launcher' && /^Stalled thread worker:/.test(e.text));
const alive = (pid: number) => {
  try { process.kill(pid, 0); } catch { return false; }
  try { if (/^\d+ \(.*\) Z/.test(fs.readFileSync(`/proc/${pid}/stat`, 'utf8'))) return false; } catch {}
  return true;
};
const until = async (label: string, predicate: () => boolean | Promise<boolean>, maximum = 12000) => {
  const end = Math.min(deadline, Date.now() + maximum);
  while (Date.now() < end) { if (await predicate()) return; await delay(75); }
  throw new Error(`timeout: ${label}; liveness=${JSON.stringify(live())}`);
};
const check = (condition: unknown, label: string) => { assert(condition, label); console.log('ok ' + label); };
let hub: ReturnType<typeof Bun.spawn> | undefined, ws: WebSocket | undefined, generation = 0;
const frames: any[] = [];
const start = async () => {
  hub = Bun.spawn([resolve(binary), '--home', home, '--port', String(port), '--no-tailscale', '--foreground', '--headless'], {
    env: { ...process.env, HOME: root, XDG_CACHE_HOME: join(root, 'cache'), DISPLAY: '', WAYLAND_DISPLAY: '',
      BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_WAKE_TICK_MS: '1000',
      BACKPLANE_NO_UPDATE: '1', BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_TELEMETRY: 'off',
      BACKPLANE_GOOGLE_BAKE: '0', BACKPLANE_PEERS: '', BACKPLANE_KEYRING_LOCKED: '0',
      BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_LOCK_DIR: join(root, 'locks'),
      BACKPLANE_DEPLOY_LOCKS: join(root, 'deploy.lock'), FIXTURE_LOG: log, FIXTURE_FINISH: finish, TMPDIR: root },
    stdout: Bun.file(join(root, `hub-${generation}.out`)), stderr: Bun.file(join(root, `hub-${generation++}.err`))
  });
  await until('isolated hub listening', async () => {
    if (hub!.exitCode !== null) throw new Error('fixture hub exited before listening');
    try { return (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  });
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = 'arraybuffer';
  ws.onmessage = e => {
    const value = JSON.parse(typeof e.data === 'string' ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer)));
    frames.push(...(value.items ?? []));
  };
  await until('client connected', () => ws!.readyState === WebSocket.OPEN, 5000);
};
const stop = async () => {
  ws?.close(); ws = undefined;
  const captured = hub; hub = undefined;
  if (captured && captured.exitCode === null) { captured.kill('SIGKILL'); await Promise.race([captured.exited, delay(2000)]); }
};
try {
  await start();
  await until('one watchdog nudge', () => nudges().length === 1);
  await until('stall persisted after nudge', () => live().stalled === true && reports().length === 1);
  await until('echo-only worker running', () => agents().some(a => a.event === 'echo' && a.thread === 'worker'));
  const workerPid = agents().find(a => a.event === 'start' && a.thread === 'worker')?.pid;
  check(workerPid && alive(workerPid), 'stalled worker process is still alive');
  check(live().last === old && live().nudged > 0, 'echoed injected text does not advance progress');
  check(/dependency fixture/.test(live().reason), 'stall retains its actionable reason');
  await until('stalled state broadcast to clients', () => frames.some(e => e.$ === 'SettingSet' && e.key === 'liveness.worker' && JSON.parse(e.value).stalled));
  const quietUntil = Date.now() + 2500;
  await until('three accelerated ticks remain quiet', () => {
    assert.equal(nudges().length, 1); assert.equal(reports().length, 1);
    assert(alive(workerPid), 'watchdog interrupted its worker');
    return Date.now() >= quietUntil;
  }, 4000);
  check(!agents().some(a => a.event === 'term' || a.event === 'control' && a.subtype === 'interrupt'), 'watchdog issues no termination or interrupt');
  const episode = live().nudged;
  await stop(); await start();
  const restartedUntil = Date.now() + 2500;
  await until('restart preserves the stalled episode', () => {
    assert.equal(nudges().length, 1, 'restart duplicated nudge');
    assert.equal(reports().length, 1, 'restart duplicated launcher notification');
    assert.equal(live().last, old, 'restart continuation was mistaken for real progress');
    assert.equal(live().nudged, episode); assert.equal(live().stalled, true);
    return Date.now() >= restartedUntil;
  }, 4000);
  check(true, 'restart retains episode and sends no duplicate notification');
  const beforeWrite = Math.floor(Date.now() / 1000);
  fs.writeFileSync(join(project, 'actual-progress.txt'), 'explicit fixture progress\n');
  await until('file write clears stall through registered sampler effect', () => live().stalled === false && live().last >= beforeWrite);
  check(live().nudged === 0 && live().reported === false, 'real file progress starts a fresh watchdog interval');
  const resumed = live().last, freshUntil = Date.now() + 2200;
  await until('new interval remains unnudged', () => {
    assert.equal(nudges().length, 1); assert.equal(reports().length, 1); assert.equal(live().last, resumed);
    return Date.now() >= freshUntil;
  }, 4000);
  check(true, 'unchanged rescans and fixture echoes do not invent further progress');
  console.log('all liveness e2e checks passed');
} catch (error) {
  console.error('fixture:', root); console.error(read(join(root, `hub-${Math.max(0, generation - 1)}.err`)).slice(-3000));
  throw error;
} finally {
  fs.writeFileSync(finish, 'finish naturally\n');
  await stop();
  const pids = [...new Set<number>(agents().filter(a => a.event === 'start').map(a => a.pid))];
  const end = Date.now() + 2500;
  while (pids.some(alive) && Date.now() < end) await delay(50);
  if (process.env.KEEP || pids.some(alive)) console.log('retained fixture:', root);
  else fs.rmSync(root, { recursive: true, force: true });
}
