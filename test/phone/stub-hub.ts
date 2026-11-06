// A stand-in hub for docs/PHONE-DATA.md (data/1): the reference the iPhone app is built against, and what the conformance script checks the Rust
// hub against. In memory, one process, stand-in turns only (no agent runs). usage: bun stub-hub.ts --port 3860 --host 100.x --token-file FILE [--root DIR]
import {existsSync, mkdirSync, readdirSync, readFileSync, statSync} from 'node:fs';
import {homedir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
const arg = (k: string, d = '') => { const i = process.argv.indexOf('--' + k); return i >= 0 ? process.argv[i + 1] : d };
const port = Number(arg('port', '3860')), host = arg('host', '127.0.0.1'), token = readFileSync(arg('token-file'), 'utf8').trim(), root = arg('root', join(homedir(), 'bp-stub-projects'));
mkdirSync(root, {recursive: true});
type Entry = {id: string, kind: string, text: string, at: number, clientId?: string, replaces?: string, live?: boolean, tool?: any, ask?: any, attachments?: any[]};
const files = new Map<string, {mime: string, data: string}>();
type Thread = {id: string, project: string, title: string, state: string, updatedAt: number, pinned: boolean, settled: boolean, archived: boolean, provider: string, model: string, preview: string, waiting: number};
type Project = {id: string, title: string, root: string, threadCount: number, updatedAt: number, waiting: number};
const projects = new Map<string, Project>(), threads = new Map<string, Thread>(), entries = new Map<string, Entry[]>(), sent = new Map<string, Entry>(), waiters = new Map<string, () => void>();
let seq = 1; const nid = (p: string) => `${p}${Date.now().toString(36)}${(seq++).toString(36)}`;
const subs = new Map<any, {lists: number | null, thread: {sub: number, id: string} | null, n: number}>();
const push = (ws: any, sub: number, k: string, body: any) => ws.send(JSON.stringify({t: 'd', sub, k, ...body}));
const toLists = (k: string, body: any) => { for (const [ws, s] of subs) if (s.lists) push(ws, s.lists, k, body) };
const toThread = (id: string, k: string, body: any) => { for (const [ws, s] of subs) if (s.thread?.id === id) push(ws, s.thread.sub, k, body) };
const sync = (p: Project) => { const ts = [...threads.values()].filter(t => t.project === p.id && !t.archived); p.threadCount = ts.length; p.waiting = ts.reduce((a, t) => a + t.waiting, 0); p.updatedAt = Math.max(p.updatedAt, ...ts.map(t => t.updatedAt)) };
const touch = (t: Thread) => { t.updatedAt = Date.now(); const p = projects.get(t.project)!; sync(p); toLists('thread', {thread: t}); toLists('project', {project: p}) };
const add = (tid: string, e: Entry) => { const l = entries.get(tid)!; const i = l.findIndex(x => x.id === e.id || (e.clientId && x.clientId === e.clientId) || (e.replaces && x.id === e.replaces)); if (i >= 0) l[i] = e; else l.push(e); toThread(tid, 'entry', {thread: tid, entry: e}); };
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function turn(t: Thread, text: string) {
  t.state = 'working'; touch(t);
  toThread(t.id, 'turn', {thread: t.id, state: 'working'});
  const tool: Entry = {id: nid('e'), kind: 'tool', text: '', at: Date.now(), tool: {name: 'Read', summary: 'notes.md', state: 'running'}};
  add(t.id, tool); await sleep(250); tool.tool.state = 'done'; add(t.id, {...tool});
  if (/ask|approve/i.test(text)) {
    const ask = {id: nid('a'), state: 'open', question: 'Run `ls -la` in the project folder?', options: [{id: 'accept', label: 'Allow'}, {id: 'session', label: 'Allow for this session'}, {id: 'decline', label: 'Decline'}]};
    const e: Entry = {id: nid('e'), kind: 'ask', text: ask.question, at: Date.now(), ask}; add(t.id, e); t.state = 'asking'; t.waiting = 1; touch(t);
    toThread(t.id, 'turn', {thread: t.id, state: 'asking'});
    const answer = await new Promise<string>(res => waiters.set(ask.id, () => res(ask.answered)));
    t.state = 'working'; t.waiting = 0; touch(t); toThread(t.id, 'turn', {thread: t.id, state: 'working'});
    if (answer === 'decline') { add(t.id, {id: nid('e'), kind: 'note', text: 'Denied: nothing was run.', at: Date.now()}); t.state = 'idle'; touch(t); toThread(t.id, 'turn', {thread: t.id, state: 'idle'}); return; }
  }
  const words = `You said: ${text}. Here is a short stand-in answer, streamed a few words at a time so the app shows live text, then settles into one message.`.split(' ');
  const e: Entry = {id: 'live:' + t.id, kind: 'assistant', text: '', at: Date.now(), live: true}; add(t.id, {...e});
  for (let i = 0; i < words.length; i++) { const w = (i ? ' ' : '') + words[i]; e.text += w; entries.get(t.id)!.find(x => x.id === e.id)!.text = e.text; toThread(t.id, 'text', {thread: t.id, entry: e.id, append: w, at: Date.now()}); await sleep(40); }
  const fin: Entry = {id: nid('e'), kind: 'assistant', text: e.text, at: Date.now(), replaces: e.id}; add(t.id, fin); t.preview = e.text.slice(0, 140); t.state = 'idle'; touch(t); toThread(t.id, 'turn', {thread: t.id, state: 'idle'});
}
const page = <T>(all: T[], limit: number, before?: string) => { const end = before ? Number(before) : all.length; const start = Math.max(0, end - limit); return {items: all.slice(start, end), next: start > 0 ? String(start) : null}; };
const methods: Record<string, (p: any, ws: any) => any> = {
  'projects.list': p => { const all = [...projects.values()].sort((a, b) => a.updatedAt - b.updatedAt); const r = page(all, p.limit ?? 50, p.before); return {projects: r.items.reverse(), next: r.next}; },
  'threads.list': p => { const all = [...threads.values()].filter(t => (!p.project || t.project === p.project) && (p.archived ? t.archived : !t.archived)).sort((a, b) => a.updatedAt - b.updatedAt); const r = page(all, p.limit ?? 50, p.before); return {threads: r.items.reverse(), next: r.next}; },
  'thread.entries': p => { const t = threads.get(p.thread); if (!t) throw 'NOT_FOUND'; const r = page(entries.get(t.id)!, p.limit ?? 40, p.before); return {thread: t, entries: r.items, next: r.next}; },
  'dirs.list': p => { const path = resolve((p.path || homedir()).replace(/^~/, homedir())); const dirs = existsSync(path) ? readdirSync(path).filter(n => !n.startsWith('.') && statSync(join(path, n)).isDirectory()).sort() : []; return {path, parent: dirname(path), dirs}; },
  'project.add': p => { const path = resolve(String(p.path || '').replace(/^~/, homedir())); if (!path || path === '/') throw 'BAD_PARAMS'; mkdirSync(path, {recursive: true}); let pr = [...projects.values()].find(x => x.root === path); if (!pr) { pr = {id: nid('p'), title: p.title || path.split('/').pop()!, root: path, threadCount: 0, updatedAt: Date.now(), waiting: 0}; projects.set(pr.id, pr); } toLists('project', {project: pr}); return {project: pr}; },
  'thread.create': p => { const pr = projects.get(p.project); if (!pr) throw 'NOT_FOUND'; const t: Thread = {id: nid('t'), project: pr.id, title: p.title || (p.text ? String(p.text).slice(0, 40) : 'New thread'), state: 'idle', updatedAt: Date.now(), pinned: false, settled: false, archived: false, provider: p.provider || 'claude', model: p.model || 'stand-in', preview: '', waiting: 0}; threads.set(t.id, t); entries.set(t.id, []); touch(t); if (p.text) methods['message.send']({thread: t.id, text: p.text, clientId: p.clientId}, null); return {thread: t}; },
  'message.send': p => { const t = threads.get(p.thread); if (!t) throw 'NOT_FOUND'; if (p.clientId && sent.has(p.clientId)) return {entry: sent.get(p.clientId)}; const att = (p.attachments || []).slice(0, 4).map((x: any) => { const id = nid('f'); files.set(id, {mime: x.mime, data: x.data}); return {id, name: x.name, mime: x.mime, bytes: Math.floor(String(x.data).length * 3 / 4)}; });
    const e: Entry = {id: nid('e'), kind: 'user', text: String(p.text || ''), at: Date.now(), clientId: p.clientId, attachments: att}; if (p.clientId) sent.set(p.clientId, e); add(t.id, e); void turn(t, e.text); return {entry: e}; },
  'ask.answer': p => { for (const l of entries.values()) for (const e of l) if (e.ask?.id === p.ask) { e.ask.state = 'answered'; e.ask.answered = String(p.answer); const t = [...threads.values()].find(x => entries.get(x.id) === l)!; add(t.id, {...e, ask: {...e.ask}}); waiters.get(p.ask)?.(); return {}; } throw 'NOT_FOUND'; },
  'attachment.get': p => { const f = files.get(p.id); if (!f) throw 'unknown attachment'; return {mime: f.mime, data: f.data}; },
  'turn.interrupt': () => ({}),
  'subscribe': (p, ws) => { const s = subs.get(ws)!; const sub = ++s.n; if (p.topic === 'lists') s.lists = sub; else if (p.topic === 'thread') s.thread = {sub, id: p.thread}; return {sub}; },
  'unsubscribe': (p, ws) => { const s = subs.get(ws)!; if (s.lists === p.sub) s.lists = null; if (s.thread?.sub === p.sub) s.thread = null; return {}; },
};
const authed = (req: Request, url: URL) => { const b = (req.headers.get('authorization') || '').replace(/^Bearer /, ''); return b === token || url.searchParams.get('token') === token; };
Bun.serve({port, hostname: host, fetch(req, server) {
  const url = new URL(req.url);
  if (url.pathname === '/hello') return Response.json({name: 'backplane-stub', proto: 'data/1'});
  if (!authed(req, url)) return new Response('paired authentication required', {status: 401});
  if (url.pathname === '/client/capabilities') return Response.json({version: 1, views: {version: 1, websocketQuery: 'view=1', renderers: ['mobile'], compatibility: 'explicit-only'}, data: {proto: 'data/1', features: ['images']}});
  if (url.pathname === '/ws' && server.upgrade(req)) return;
  return new Response('not found', {status: 404});
}, websocket: {
  open(ws) { subs.set(ws, {lists: null, thread: null, n: 0}); ws.send(JSON.stringify({t: 'hello', proto: 'data/1', features: ['images'], hub: `${host}:${port}`, version: 'stub', epoch: String(Date.now()), now: Date.now()})); const hb = setInterval(() => { try { ws.send('{"t":"hb"}') } catch { clearInterval(hb) } }, 15000); (ws as any).hb = hb; },
  close(ws) { clearInterval((ws as any).hb); subs.delete(ws); },
  message(ws, raw) { let m: any; try { m = JSON.parse(String(raw)) } catch { return } const f = methods[m.m]; if (!f) return ws.send(JSON.stringify({t: 'reply', id: m.id, ok: false, error: 'unknown method'}));
    try { ws.send(JSON.stringify({t: 'reply', id: m.id, ok: true, result: f(m.p || {}, ws)})) } catch (e) { ws.send(JSON.stringify({t: 'reply', id: m.id, ok: false, error: {code: typeof e === 'string' ? e : 'INTERNAL', message: String(e)}})) } },
}});
console.log(`stub hub data/1 on ${host}:${port}`);
