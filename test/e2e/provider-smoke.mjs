// Provider smoke: every agent provider on its REAL CLI and the account it is already signed in with, through a private hub on a temp home.
// Run before a release on each hub platform (Linux x64/arm64, macOS, Windows). Real model turns: a few cheap ones per provider.
// usage: bun test/e2e/provider-smoke.mjs HUB_BIN OUT_DIR [PORT] [provider ...]   -> OUT_DIR/smoke.json and one line per check
// Each provider gets a thread, run side by side on one hub:
//   start    the CLI is found (models.cache lists it) and a turn starts and streams an assistant message
//   tool     a shell command it runs writes a marker file in the project (checked on disk)
//   mcp      it calls Backplane's MCP tool thread_update to retitle its own thread (checked as the hub's ThreadRenamed event)
//   usage    the hub recorded TurnUsage for the thread; limits: whether an allowance reading exists for the provider (reported, not judged)
//   interrupt  a turn running `sleep` is stopped: state interrupted within 20 s and the sleep is gone
//   resume   the hub is restarted; the next turn remembers the word given in the first turn
//   model    a second model from the provider's catalog is chosen; the next turn runs on it (TurnUsage.model or the CLI's argv)
// A provider that is not installed or not signed in is "untestable" with the reason, never a pass. Nothing of the person's hub is touched:
// the CLIs use their own default login from $HOME, and the hub's home is OUT_DIR/home.
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, rmSync, writeFileSync, readFileSync, existsSync, openSync, readdirSync} from 'node:fs';
import {join} from 'node:path';
import {encode, decode} from '../wire.mjs';

const [BIN, OUT, PORT_ARG, ...ONLY] = process.argv.slice(2);
if (!BIN || !OUT) { console.error('usage: bun provider-smoke.mjs HUB_BIN OUT_DIR [PORT] [provider ...]'); process.exit(2) }
const PORT = Number(PORT_ARG || 3911);
const ALL = ['claude', 'codex', 'antigravity', 'grok', 'opencode', 'muse', 'pi', 'omp'];
const PROVIDERS = ONLY.length ? ONLY : ALL;
const WIN = process.platform === 'win32';
const sleep = ms => new Promise(r => setTimeout(r, ms));
rmSync(OUT, {recursive: true, force: true}); mkdirSync(OUT + '/proj', {recursive: true});
const HOME = join(OUT, 'home'), PROJ = join(OUT, 'proj');
writeFileSync(join(PROJ, 'readme.txt'), 'provider smoke fixture\n');
try { execFileSync('git', ['init', '-q'], {cwd: PROJ}); execFileSync('git', ['-c', 'user.email=smoke@example.invalid', '-c', 'user.name=smoke', 'commit', '-qam', 'fixture', '--allow-empty'], {cwd: PROJ}) } catch {}
try { execFileSync('git', ['add', '.'], {cwd: PROJ}); execFileSync('git', ['-c', 'user.email=smoke@example.invalid', '-c', 'user.name=smoke', 'commit', '-qm', 'fixture'], {cwd: PROJ}) } catch {}

const cells = {}; // provider -> {check: {ok, ev}}
const mark = (p, id, ok, ev) => { (cells[p] ||= {})[id] = {ok, ev}; console.log(`${ok === true ? 'ok  ' : ok === false ? 'FAIL' : 'n/a '} ${p.padEnd(12)} ${id.padEnd(9)} ${ev}`) };

let hub = null;
const env0 = {...process.env, BACKPLANE_NO_UPDATE: '1'}; delete env0.BACKPLANE_AGENTS; delete env0.BACKPLANE_AGENT_OWNER; delete env0.BACKPLANE_COPY;
async function start() {
  const out = openSync(join(OUT, 'hub.log'), 'a');
  hub = spawn(BIN, ['--home', HOME, '--port', String(PORT), '--no-tailscale', '--foreground'], {env: env0, stdio: ['ignore', out, out]});
  for (let i = 0; i < 600; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) return } catch {} await sleep(100) }
  throw new Error('hub did not answer /hello');
}
async function stop() {
  if (!hub) return; const h = hub; hub = null;
  const gone = new Promise(r => h.once('exit', r)); try { h.kill('SIGTERM') } catch {}
  await Promise.race([gone, sleep(20000)]); try { h.kill('SIGKILL') } catch {}
}
let evCache = {len: 0, list: []};
function events() {
  let txt; try { txt = readFileSync(join(HOME, 'events.jsonl'), 'utf8') } catch { return [] }
  if (txt.length < evCache.len) evCache = {len: 0, list: []};
  const tail = txt.slice(evCache.len); const cut = tail.lastIndexOf('\n'); if (cut < 0) return evCache.list;
  for (const l of tail.slice(0, cut).split('\n')) { if (!l) continue; try { evCache.list.push(JSON.parse(l)) } catch {} }
  evCache.len += cut + 1; return evCache.list;
}
const of = (t, $) => events().filter(e => e.$ === $ && (e.thread === t || e.id === t));
async function until(f, ms) { const end = Date.now() + ms; for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return null; await sleep(250) } }
const token = () => readFileSync(join(HOME, 'token'), 'utf8').trim();
async function connect() {
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws?token=${token()}`); ws.binaryType = 'arraybuffer';
  const c = {ws, replies: new Map(), id: 100};
  ws.onmessage = e => { let o; try { o = typeof e.data === 'string' ? JSON.parse(e.data) : decode(e.data) } catch { return } if (o.t === 'reply') c.replies.set(o.id, o) };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j });
  c.rpc = async (m, p = {}) => { const n = ++c.id; ws.send(encode({id: n, m, p})); for (let k = 0; k < 600 && !c.replies.has(n); k++) await sleep(50); return c.replies.get(n) || {ok: false, error: 'no reply'} };
  return c;
}
const states = t => of(t, 'TurnChanged').map(e => e.state);
const ended = (t, n) => until(() => states(t).filter(s => s !== 'running').length >= n, 300000);
const said = (t, since = 0) => events().filter(e => e.$ === 'MessagePosted' && e.thread === t && e.role === 'assistant' && (e.at || 0) >= since).map(e => e.text || '').join('\n');
const errs = t => events().filter(e => e.$ === 'ActivityLogged' && e.thread === t && e.tone === 'error').map(e => String(e.summary || '').slice(0, 160));
const why = t => { const e = errs(t); return e.length ? ` errors: ${JSON.stringify(e.slice(-2))}` : '' };
const now = () => Math.floor(Date.now() / 1000);
const SIGNIN = /not logged in|\/login|sign in once|authentication required|401 unauthorized|no default model selected|no api key|insufficient balance|top up/i;
// child processes still holding a marker in their argv (Linux and macOS: ps; Windows: wmic is gone, use PowerShell)
function procsWith(s) {
  try {
    const out = WIN ? execFileSync('powershell', ['-NoProfile', '-Command', 'Get-CimInstance Win32_Process | ForEach-Object { $_.CommandLine }'], {encoding: 'utf8'}) : execFileSync('ps', ['-axo', 'pid,ppid,command'], {encoding: 'utf8'});
    return out.split(/\r?\n/).filter(l => l.includes(s) && !l.includes('provider-smoke') && !/\bps\b -axo/.test(l));
  } catch { return [] }
}

for (const ev of ['uncaughtException', 'unhandledRejection']) process.on(ev, async e => { console.error(e); await stop(); process.exit(1) });
await start();
let c = await connect();
// the catalog: the hub probes every CLI after start (models.cache)
const catalog = await until(() => { try { const m = JSON.parse(readFileSync(join(HOME, 'models.cache'), 'utf8')); return Object.keys(m).length ? m : null } catch { return null } }, 120000) || {};
writeFileSync(join(OUT, 'models.cache.json'), JSON.stringify(catalog, null, 1));
// forward slashes: a backslash path is refused by hubs before the fs.rs slashed() fix
const added = await c.rpc('project.add', {path: PROJ.replace(/\\/g, '/')}); await until(() => events().some(e => e.$ === 'ProjectCreated'), 30000);
const P = events().find(e => e.$ === 'ProjectCreated')?.id;
if (!P) { console.error(`project.add made no project: ${JSON.stringify(added).slice(0, 300)}`); await stop(); process.exit(1) }

function modelsOf(p) {
  const v = catalog[p] ?? catalog.providers?.[p] ?? catalog.agents?.[p];
  const xs = Array.isArray(v) ? v : Array.isArray(v?.models) ? v.models : Array.isArray(v?.values) ? v.values : [];
  return xs.map(x => typeof x === 'string' ? x : x?.value || x?.id || x?.model).filter(Boolean);
}
function installed(p) {
  const v = catalog[p] ?? catalog.providers?.[p] ?? catalog.agents?.[p];
  if (v && (v.installed === false || v.available === false)) return false;
  return v !== undefined ? true : null;
}

// ---- run every provider's thread side by side ----
const T = {}, WORD = {}, MARK = {};
const run = async (p, f) => { try { await f() } catch (e) { mark(p, 'error', false, String(e?.stack || e).slice(0, 300)) } };
await Promise.all(PROVIDERS.map(p => run(p, async () => {
  const inst = installed(p);
  if (inst === false) { mark(p, 'start', null, `not installed on this machine (models.cache: ${JSON.stringify(catalog[p]).slice(0, 120)})`); return }
  const r = await c.rpc('thread.create', {project: P, title: `smoke ${p}`, env: 'local', provider: p});
  const t = r.thread; if (!t) { mark(p, 'start', false, `thread.create refused: ${r.error}`); return }
  T[p] = t; WORD[p] = `zephyr${Math.floor(Math.random() * 1e6)}`; MARK[p] = `smoke-${p}-${Math.floor(Math.random() * 1e6)}.txt`;
  await c.rpc('thread.modes', {thread: t, runtime: 'full-access'});
  const at = now();
  await c.rpc('turn.start', {thread: t, text:
    `This is an automated smoke test of Backplane. Remember this word: ${WORD[p]}.\n` +
    `1. Use your shell tool to run exactly: echo ok > ${MARK[p]}\n` +
    `2. Call the Backplane MCP tool thread_update with title "smoke-done-${p}" (no threadId; it renames this thread).\n` +
    `3. Then reply with the single word DONE.`});
  const fin = await ended(t, 1); const st = states(t).at(-1);
  const text = said(t, at);
  // not testable here: the CLI is missing (the hub moves the thread on: ProviderSet) or not signed in. Said once, and the provider's later steps are skipped.
  const moved = of(t, 'ProviderSet').map(e => e.provider).filter(x => x && x !== p);
  const spawn = events().filter(e => e.$ === 'ActivityLogged' && e.thread === t && e.kind === 'spawn').map(e => String(e.summary || ''));
  const signin = errs(t).find(e => SIGNIN.test(e));
  if (moved.length || spawn.some(x => /not installed/i.test(x))) { mark(p, 'start', null, `not installed here: ${JSON.stringify((spawn[0] || '').slice(0, 140))}; the hub moved the thread to ${moved.join(',') || 'nothing'} (${JSON.stringify(of(t, 'ActivityLogged').filter(e => e.kind === 'fallback').map(e => e.summary).slice(-1))})`); delete T[p]; return }
  if (signin && st !== 'completed') { mark(p, 'start', null, `installed, launched, not signed in: ${JSON.stringify(signin)}`); delete T[p]; return }
  mark(p, 'start', !!fin && st === 'completed' && text.length > 0, `turn ${st || 'never ended'}; assistant said ${JSON.stringify(text.slice(0, 80))}${why(t)}`);
  const file = existsSync(join(PROJ, MARK[p]));
  const shellActs = events().filter(e => e.$ === 'ActivityLogged' && e.thread === t && e.tone === 'tool').map(e => e.kind);
  mark(p, 'tool', file, `marker file written=${file}; tool activity kinds ${JSON.stringify([...new Set(shellActs)])}`);
  const ren = of(t, 'ThreadRenamed').map(e => e.title);
  mark(p, 'mcp', ren.includes(`smoke-done-${p}`), `ThreadRenamed ${JSON.stringify(ren)}`);
  const use = of(t, 'TurnUsage');
  const allow = events().filter(e => e.$ === 'SettingSet' && (e.key === `allowance.${p}` || String(e.key).startsWith('account.limit.'))).map(e => e.key);
  mark(p, 'usage', use.length > 0, `TurnUsage ${use.length} (model ${JSON.stringify([...new Set(use.map(u => u.model))])}, output ${use.reduce((a, u) => a + (u.output || 0), 0)} tokens); limits readings: ${allow.length ? [...new Set(allow)].join(',') : 'none'}`);
})));

// interrupt: a turn running sleep is stopped (under 600 s: Claude Code moves a longer command to the background and ends the turn)
await Promise.all(PROVIDERS.filter(p => T[p]).map(p => run(p, async () => {
  const t = T[p], secs = 400 + Math.floor(Math.random() * 150), n0 = states(t).filter(s => s !== 'running').length;
  await c.rpc('turn.start', {thread: t, text: `Use your shell tool to run exactly: sleep ${secs}\nWait for it to finish, then say finished.`});
  const sleeping = await until(() => procsWith(`sleep ${secs}`).length > 0, 180000);
  const live = states(t).at(-1) === 'running';
  if (!live) { mark(p, 'interrupt', null, `the turn ended before it could be interrupted (state ${states(t).at(-1)}): ${JSON.stringify(said(t).slice(-120))}`); return }
  const r = await c.rpc('turn.interrupt', {thread: t});
  const stopped = await until(() => states(t).filter(s => s !== 'running').length > n0, 20000);
  await until(() => procsWith(`sleep ${secs}`).length === 0, 20000); const left = procsWith(`sleep ${secs}`);
  // a child that outlives the Stop is reported, not failed: Claude Code keeps a command it moved to a background task until the CLI exits (its own behaviour, 2.1.322)
  mark(p, 'interrupt', !!sleeping && r.ok && !!stopped && states(t).at(-1) === 'interrupted',
    `sleep started=${!!sleeping}; interrupt ok=${r.ok}${r.error ? ' ' + r.error : ''}; state ${states(t).at(-1)}; ${left.length ? 'WARN ' : ''}sleep left running 20 s later ${left.length}${left.length ? ' ' + JSON.stringify(left.map(l => l.trim().slice(0, 140))) : ''}${why(t)}`);
})));

// restart the hub, then resume and choose a second model
await stop(); await sleep(1500); await start(); c = await connect(); await sleep(3000);
await Promise.all(PROVIDERS.filter(p => T[p]).map(p => run(p, async () => {
  const t = T[p], n0 = states(t).filter(s => s !== 'running').length, at = now();
  await until(() => states(t).at(-1) !== 'running', 60000);
  await c.rpc('turn.start', {thread: t, text: 'What was the word I asked you to remember in my first message? Reply with only that word.'});
  const fin = await until(() => states(t).filter(s => s !== 'running').length > n0, 300000);
  const text = said(t, at); const sess = of(t, 'SessionSet').map(e => e.session);
  mark(p, 'resume', !!fin && states(t).at(-1) === 'completed' && text.toLowerCase().includes(WORD[p]), `after a hub restart: turn ${states(t).at(-1)}, said ${JSON.stringify(text.slice(0, 60))} (want ${WORD[p]}); sessions ${JSON.stringify([...new Set(sess)]).slice(0, 120)}${why(t)}`);

  const ms = modelsOf(p); const cur = events().filter(e => e.$ === 'ModesSet' && e.id === t).map(e => e.model).filter(Boolean).at(-1) || '';
  // a model that is neither the thread's nor the provider's default nor the one the first turn ran on, so the turn's usage proves the choice reached the CLI
  const ran = new Set(of(t, 'TurnUsage').map(u => u.model)), labels = catalog.about?.[p]?.labels || {};
  const plain = m => !/default/i.test(m) && !/default/i.test(labels[m] || '') && !/\[1m\]/.test(m) && !ran.has(m) && ![...ran].some(r => r && (r.endsWith('/' + m) || m.endsWith('/' + r)));
  const pick = ms.find(m => m !== cur && plain(m)) || ms.find(m => m !== cur && !ran.has(m));
  if (!pick) { mark(p, 'model', null, `no second model in the catalog (${JSON.stringify(ms).slice(0, 120)})`); return }
  await c.rpc('thread.modes', {thread: t, model: pick}); await sleep(500); await c.rpc('queue.resume', {thread: t});
  const n1 = states(t).filter(s => s !== 'running').length, at2 = now();
  await c.rpc('turn.start', {thread: t, text: 'Reply with the single word OK.'});
  const fin2 = await until(() => states(t).filter(s => s !== 'running').length > n1, 300000);
  const use = of(t, 'TurnUsage').filter(u => (u.at || 0) >= at2).map(u => u.model);
  // exactly the chosen model (or it under its provider's prefix): a family name that merely overlaps is not proof (grok-4.7 for grok-4.7-build-fast)
  const onIt = use.some(m => m && (m === pick || m.endsWith('/' + pick) || pick.endsWith('/' + m)));
  mark(p, 'model', !!fin2 && states(t).at(-1) === 'completed' && (onIt || use.length === 0 ? onIt || null : false),
    `chose ${pick} (of ${ms.length}); turn ${states(t).at(-1)}; TurnUsage models ${JSON.stringify(use)}${use.length ? '' : ' (no usage model to prove it)'}${why(t)}`);
})));

await stop();
writeFileSync(join(OUT, 'smoke.json'), JSON.stringify({platform: `${process.platform}-${process.arch}`, hub: BIN, at: new Date().toISOString(), cells}, null, 1));
const bad = Object.values(cells).some(x => Object.values(x).some(v => v.ok === false));
console.log(`wrote ${join(OUT, 'smoke.json')}`); process.exit(bad ? 1 : 0);
