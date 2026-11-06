// A helper-less hub on a temp home and a client that speaks the window's view protocol (native/gpui/src/hub.rs): view.open / view.action / view.update,
// the `send` commands of a reply forwarded as product RPCs, view.changed answered with view.update. Temp homes and stand-in agents only.
// The hub is started as the person's is (--home --port --host 127.0.0.1 --foreground), with NO --policy-bin; start() checks it has no child process.
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, writeFileSync, rmSync, readFileSync, existsSync, copyFileSync, chmodSync} from 'node:fs';
import {decode, encode} from '../wire.mjs';

export const sleep = ms => new Promise(r => setTimeout(r, ms));
const sh = (cwd, ...a) => execFileSync(a[0], a.slice(1), {cwd, stdio: 'pipe'}).toString();

export function makeProject(dir) {
  rmSync(dir, {recursive: true, force: true}); mkdirSync(dir, {recursive: true});
  const git = (...a) => sh(dir, 'git', '-c', 'user.name=t', '-c', 'user.email=t@t', ...a);
  git('init', '-q', '-b', 'main'); git('config', 'user.name', 'Rig'); git('config', 'user.email', 'rig@example.invalid');
  writeFileSync(dir + '/a.txt', 'one\ntwo\nthree\n'); writeFileSync(dir + '/gone.txt', 'bye\n');
  const fx = new URL('../files/fixtures/plot/', import.meta.url).pathname;
  for (const e of ['kicad_pcb', 'kicad_sch', 'kicad_pro']) if (existsSync(fx + 'ecc83-pp.' + e)) copyFileSync(fx + 'ecc83-pp.' + e, dir + '/ecc83-pp.' + e);
  git('add', '-A'); git('commit', '-q', '-m', 'first');
  return git;
}
// A bare remote, so Commit's push has somewhere to go (the person's threads have one; a temp repo has none)
export function addRemote(dir, bare) { rmSync(bare, {recursive: true, force: true}); sh('.', 'git', 'init', '-q', '--bare', bare); sh(dir, 'git', 'remote', 'add', 'origin', bare); }
// The checkpoint a thread's first turn takes (agents/hist.rs checkpoint0): the work tree as it is, under refs/backplane/checkpoints/<b64url thread>/turn/0
export function checkpoint0(dir, thread) {
  const git = (...a) => sh(dir, 'git', '-c', 'user.name=Backplane', '-c', 'user.email=backplane@localhost', ...a).trim();
  const idx = git('rev-parse', '--absolute-git-dir') + '/backplane-seed.index';
  const env = {...process.env, GIT_INDEX_FILE: idx}; rmSync(idx, {force: true});
  execFileSync('git', ['add', '-A'], {cwd: dir, env}); const tree = execFileSync('git', ['write-tree'], {cwd: dir, env}).toString().trim();
  const sha = git('commit-tree', '-m', 'checkpoint', tree); rmSync(idx, {force: true});
  git('update-ref', 'refs/backplane/checkpoints/' + Buffer.from(thread).toString('base64url') + '/turn/0', sha);
}
export function changeProject(dir) {   // real changes: a modified file, a deleted one, an untracked one
  writeFileSync(dir + '/a.txt', 'one\n2\nthree\nfour\n'); rmSync(dir + '/gone.txt'); writeFileSync(dir + '/new.txt', 'fresh\n');
}

export async function startHub({bin, rig, port}) {
  rmSync(rig + '/home', {recursive: true, force: true}); mkdirSync(rig + '/home', {recursive: true}); mkdirSync(rig + '/agents', {recursive: true});
  for (const n of ['claude', 'codex', 'grok', 'muse']) { writeFileSync(rig + '/agents/' + n, '#!/bin/sh\nexit 1\n'); chmodSync(rig + '/agents/' + n, 0o755); }
  const log = rig + '/hub.log'; const out = require_fs().openSync(log, 'w');
  const p = spawn(bin, ['--home', rig + '/home', '--port', String(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'],
    {env: {...process.env, BACKPLANE_AGENTS: rig + '/agents', BACKPLANE_NO_UPDATE: '1', HOME: rig + '/fakehome'}, stdio: ['ignore', out, out]});
  for (let i = 0; i < 100; i++) { try { const r = await fetch(`http://127.0.0.1:${port}/hello`); if (r.ok) break } catch {} await sleep(150) }
  const kids = (() => { try { return sh('.', 'pgrep', '-P', String(p.pid)).trim() } catch { return '' } })();
  const env = readFileSync(`/proc/${p.pid}/environ`, 'utf8').split('\0');
  const cmd = readFileSync(`/proc/${p.pid}/cmdline`, 'utf8').split('\0').join(' ');
  return {proc: p, pid: p.pid, log, token: readFileSync(rig + '/home/token', 'utf8').trim(), children: kids, agentsEnv: env.find(x => x.startsWith('BACKPLANE_AGENTS=')), cmd,
    stop() { p.kill('SIGTERM') }};
}
import * as fs from 'node:fs'; const require_fs = () => fs;

export class Client {
  constructor(port, token) { this.port = port; this.token = token; this.session = {sel: '', rid: Date.now() % 100000 + 1000, drafts: {}, open: {}, adding: false, panel: false, info: {}}; this.target = {kind: 'recent', id: ''}; this.viewId = ''; this.rev = 0; this.next = 2 ** 47 + 100; this.rpcid = 5000; this.waiters = []; this.frames = []; this.dirty = false; }
  async connect() {
    this.ws = new WebSocket(`ws://127.0.0.1:${this.port}/ws?view=1&mobile=1&token=${encodeURIComponent(this.token)}`); this.ws.binaryType = 'arraybuffer';
    this.ws.onmessage = ({data}) => { const x = typeof data === 'string' ? JSON.parse(data) : decode(data); if(!x)return; this.frames.push(x); if (x.t === 'view') this.greet = x; if (x.t === 'view.changed') this.dirty = true; for (const w of [...this.waiters]) w(x) };
    await new Promise((ok, no) => { this.ws.onopen = ok; this.ws.onerror = no });
    while (!this.greet) await sleep(20);
  }
  wait(pred, ms = 8000) { return new Promise((ok, no) => { const t = setTimeout(() => { this.waiters = this.waiters.filter(f => f !== w); no(new Error('timeout')) }, ms); const w = x => { if (pred(x)) { clearTimeout(t); this.waiters = this.waiters.filter(f => f !== w); ok(x) } }; this.waiters.push(w) }) }
  rpc(m, p = {}) { const id = ++this.rpcid; this.ws.send(encode({id, m, p})); return this.wait(x => x.t === 'reply' && x.id === id).then(x => ({id, ...x})) }
  async view(method, extra = {}) {
    const id = ++this.next; const p = {version: 1, hub: `http://127.0.0.1:${this.port}`, generation: this.greet.generation, epoch: this.greet.epoch, requestRevision: ++this.rev, target: this.target, viewId: this.viewId, session: this.session, cid: 'viewers-rig', nativeSidebar: 1, ...extra};
    this.ws.send(encode({id, m: method, p}));
    const r = await this.wait(x => x.t === 'reply' && x.id === id);
    if (!r.ok) { this.last = r; return r }
    const res = r.result; this.viewId = res.viewId || this.viewId; if (res.session) this.session = res.session;
    for (const c of res.cmds || []) if (c.type === 'send') this.ws.send(encode(JSON.parse(c.text)));
    if (res.nextTarget) { this.target = res.nextTarget; this.viewId = ''; return this.view('view.open') }
    this.last = r; return r;
  }
  thread() { return this.last?.result?.shown?.thread ?? this.last?.result?.shown }
  async action(action, value = '', settle = 1500) { const r = await this.view('view.action', {action, value}); await sleep(settle); return r }
  async update(settle = 300) { await sleep(settle); this.dirty = false; return this.view('view.update') }
  async selectFirstThread() {
    await this.view('view.open');
    const shown = this.last.result.shown; const th = (shown.projects?.[0]?.threads?.[0]) || null;
    if (!th) throw new Error('no thread to select: ' + JSON.stringify(shown).slice(0, 400));
    const id = th.id.split('|').pop(); this.target = {kind: 'thread', id}; this.session.sel = id; this.viewId = ''; await this.view('view.open'); return id;
  }
  close() { this.ws.close() }
}
