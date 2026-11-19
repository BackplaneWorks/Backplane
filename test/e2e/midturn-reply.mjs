// #1304: a message the person sends into a running, tool-heavy turn: how long until the agent answers it in the thread (an assistant message after it)?
// A REAL model on a private hub (temp home, own port; nothing of the person's hub or display is touched). Credentials come from files the caller names
// (secret_run `files`), handed to the CLI by a wrapper in a stand-in folder: the hub strips login variables from its children.
// usage: CC_TOKEN_FILE=… [OPENAI_KEY_FILE=…] bun test/e2e/midturn-reply.mjs HUB_BIN OUT_DIR [PORT] [provider:model ...]
//   default runs: claude:claude-haiku-5-5 three times. Each run: a new thread, a turn of ~25 slow shell steps, "Status?" sent after the third tool call,
//   then the seconds from the send to the first assistant message posted after it (none in WAIT s: "no reply"). -> OUT_DIR/midturn.json
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, rmSync, writeFileSync, readFileSync, openSync, chmodSync, existsSync} from 'node:fs';
import {join} from 'node:path';
import {encode, decode} from '../wire.mjs';

const [BIN, OUT, PORT_ARG, ...RUNS_ARG] = process.argv.slice(2);
if (!BIN || !OUT) { console.error('usage: bun midturn-reply.mjs HUB_BIN OUT_DIR [PORT] [provider:model ...]'); process.exit(2) }
const PORT = Number(PORT_ARG || 3931); const WAIT = Number(process.env.WAIT || 240);
const RUNS = RUNS_ARG.length ? RUNS_ARG : ['claude:claude-haiku-5-5', 'claude:claude-haiku-5-5', 'claude:claude-haiku-5-5'];
const sleep = ms => new Promise(r => setTimeout(r, ms));
rmSync(OUT, {recursive: true, force: true}); mkdirSync(join(OUT, 'proj'), {recursive: true}); mkdirSync(join(OUT, 'bin'), {recursive: true});
const HOME = join(OUT, 'home'), PROJ = join(OUT, 'proj'), BINS = join(OUT, 'bin');
writeFileSync(join(PROJ, 'readme.txt'), 'mid-turn reply fixture\n');
try { execFileSync('git', ['init', '-q'], {cwd: PROJ}); execFileSync('git', ['add', '.'], {cwd: PROJ}); execFileSync('git', ['-c', 'user.email=t@example.invalid', '-c', 'user.name=t', 'commit', '-qm', 'fixture'], {cwd: PROJ}) } catch {}

// the stand-in folder: wrappers that exec the real CLIs with the test login (and their own config folder)
const real = c => execFileSync('sh', ['-lc', `command -v ${c}`], {encoding: 'utf8'}).trim();
if (process.env.CC_TOKEN_FILE) {
  writeFileSync(join(BINS, 'claude'), `#!/bin/sh\nexport CLAUDE_CODE_OAUTH_TOKEN="$(cat '${process.env.CC_TOKEN_FILE}')"\nexport CLAUDE_CONFIG_DIR='${join(OUT, 'cc')}'\nexec '${real('claude')}' "$@"\n`);
  chmodSync(join(BINS, 'claude'), 0o755);
}
if (process.env.OPENAI_KEY_FILE) {
  // a paid API key: only through scripts/codex-apikey.sh, which pins the cheapest model and refuses any other (Nov 19: a run naming no model spent the key on gpt-6.2-sol)
  writeFileSync(join(BINS, 'codex'), `#!/bin/sh\nexport CODEX_APIKEY_FILE='${process.env.OPENAI_KEY_FILE}'\nexport CODEX_APIKEY_HOME='${join(OUT, 'cx')}'\nexport CODEX_APIKEY_REAL='${real('codex')}'\nexport CODEX_APIKEY_TURNS=3\nexec '${join(import.meta.dir, '../../scripts/codex-apikey.sh')}' "$@"\n`);
  chmodSync(join(BINS, 'codex'), 0o755);
}

let hub = null;
const env0 = {...process.env, BACKPLANE_NO_UPDATE: '1', BACKPLANE_AGENTS: BINS}; delete env0.BACKPLANE_AGENT_OWNER; delete env0.BACKPLANE_COPY;
async function start() {
  const out = openSync(join(OUT, 'hub.log'), 'a');
  hub = spawn(BIN, ['--home', HOME, '--port', String(PORT), '--no-tailscale', '--foreground'], {env: env0, stdio: ['ignore', out, out]});
  for (let i = 0; i < 600; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) return } catch {} await sleep(100) }
  throw new Error('hub did not answer /hello');
}
let evCache = {len: 0, list: []};
function events() {
  let txt; try { txt = readFileSync(join(HOME, 'events.jsonl'), 'utf8') } catch { return [] }
  const tail = txt.slice(evCache.len); const cut = tail.lastIndexOf('\n'); if (cut < 0) return evCache.list;
  for (const l of tail.slice(0, cut).split('\n')) { if (!l) continue; try { evCache.list.push(JSON.parse(l)) } catch {} }
  evCache.len += cut + 1; return evCache.list;
}
async function until(f, ms) { const end = Date.now() + ms; for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return null; await sleep(200) } }
async function connect() {
  const token = readFileSync(join(HOME, 'token'), 'utf8').trim();
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws?token=${token}`); ws.binaryType = 'arraybuffer';
  const c = {ws, replies: new Map(), id: 100};
  ws.onmessage = e => { let o; try { o = typeof e.data === 'string' ? JSON.parse(e.data) : decode(e.data) } catch { return } if (o.t === 'reply') c.replies.set(o.id, o) };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j });
  c.rpc = async (m, p = {}) => { const n = ++c.id; ws.send(encode({id: n, m, p})); for (let k = 0; k < 600 && !c.replies.has(n); k++) await sleep(50); return c.replies.get(n) || {ok: false, error: 'no reply'} };
  return c;
}

const TASK = process.env.TASK_FILE ? readFileSync(process.env.TASK_FILE, 'utf8') : 'Audit how long shell steps take on this machine. Run these 25 shell commands one at a time, each as its own tool call, waiting for each to finish before the next: '
  + Array.from({length: 25}, (_, i) => `\`sleep 4; echo step ${i + 1} >> steps.log\``).join(', ')
  + '. When all 25 are done, read steps.log and summarize it in one line.';

await start(); const c = await connect();
await c.rpc('project.add', {path: PROJ}); const P = (await until(() => events().find(e => e.$ === 'ProjectCreated'), 30000))?.id;
if (!P) { console.error('no project'); process.exit(1) }
const results = [];
for (const [i, run] of RUNS.entries()) {
  const [prov, model] = run.split(':');
  const r = await c.rpc('thread.create', {project: P, title: `midturn ${i} ${run}`, env: 'local', provider: prov}); const t = r.thread;
  if (!t) { results.push({run, error: `thread.create: ${r.error}`}); continue }
  await c.rpc('thread.modes', {thread: t, runtime: 'full-access', ...(model ? {model} : {})}); await sleep(300);
  const t0 = Date.now(); await c.rpc('turn.start', {thread: t, text: TASK});
  const tools = () => events().filter(e => e.$ === 'ActivityLogged' && e.thread === t && e.tone === 'tool').length;
  const AFTER = Number(process.env.AFTER_TOOLS || 3); const busy = await until(() => tools() >= AFTER, 300000);
  const turnEnded = since => events().some(e => e.$ === 'TurnChanged' && e.thread === t && e.state !== 'running' && e.at >= since);
  if (!busy) { results.push({run, thread: t, error: `the turn made ${tools()} tool calls in 300 s`}); console.log(JSON.stringify(results.at(-1))); continue }
  const sentAt = Date.now(); const sentS = Math.floor(sentAt / 1000); const toolsBefore = tools();
  await c.rpc('turn.start', {thread: t, text: process.env.MSG || 'Status?'});
  const reply = await until(() => events().find(e => e.$ === 'MessagePosted' && e.thread === t && e.role === 'assistant' && e.at >= sentS && !String(e.msg).endsWith(':before')), WAIT * 1000);
  const replyMs = reply ? Date.now() - sentAt : null; // polled at 200 ms: the event's own `at` is whole seconds
  const toolsBetween = tools() - toolsBefore;
  const res = {run, thread: t, reply_s: replyMs == null ? null : Math.round(replyMs / 100) / 10, reply_at_s: reply ? reply.at - sentS : null, tools_before_reply: toolsBetween,
    ended_without_reply: !reply && turnEnded(sentS), text: reply ? String(reply.text).slice(0, 200) : ''};
  results.push(res); console.log(JSON.stringify(res));
  await c.rpc('turn.interrupt', {thread: t}); await until(() => turnEnded(sentS), 30000); void t0;
}
writeFileSync(join(OUT, 'midturn.json'), JSON.stringify(results, null, 1));
try { hub.kill('SIGTERM') } catch {} await sleep(1500); try { hub.kill('SIGKILL') } catch {}
process.exit(0);
