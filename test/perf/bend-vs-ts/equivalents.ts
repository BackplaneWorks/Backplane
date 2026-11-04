/** Plain mutable Maps/arrays; this is a benchmark oracle, not hub code. */
export interface Thread {
  id: string; project: string; turn: string; pending: number;
  env: string; worktree: boolean; deleted: boolean; archived: boolean;
}
export interface Ask { id: string; thread: string; kind: string; open: boolean }
export interface Task { id: string; parent: string; child: string; state: string }
export interface Metadata {
  threads: Thread[]; asks: Ask[]; tasks: Task[];
  queued: Record<string, string[]>; settings: Record<string, string>;
}
export interface Index {
  by: Map<string, Thread[]>; asks: Map<string, Ask[]>;
  kids: Map<string, number>; work: Map<string, number>;
}
export const terminal = (state: string) => ['done', 'failed', 'cancelled'].includes(state);
export function sideIndex(s: Metadata): Index {
  const by = new Map<string, Thread[]>(), asks = new Map<string, Ask[]>();
  const kids = new Map<string, number>(), work = new Map<string, number>();
  for (const t of s.threads) {
    if (t.deleted) continue;
    let list = by.get(t.project);
    if (!list) by.set(t.project, list = []);
    list.push(t);
  }
  for (const a of s.asks) if (a.open) {
    let list = asks.get(a.thread);
    if (!list) asks.set(a.thread, list = []);
    list.push(a);
  }
  for (const t of s.tasks) if (!terminal(t.state)) kids.set(t.parent, (kids.get(t.parent) ?? 0) + 1);
  for (const [key, value] of Object.entries(s.settings)) {
    let th: string, count: number;
    if (key.startsWith('bg.')) { th = key.slice(3); count = value.split('\n').filter(Boolean).length; }
    else if (key.startsWith('watch.')) { th = key.slice(6); count = value.split('\n').filter(Boolean).length; }
    else if (key.startsWith('order.wait.')) { th = key.slice(11); count = value ? 1 : 0; }
    else continue;
    work.set(th, (work.get(th) ?? 0) + count);
  }
  return { by, asks, kids, work };
}
export function phase(ix: Index, s: Metadata, t: Thread): [string, number] {
  const asks = ix.asks.get(t.id) ?? [];
  if (t.pending > 0 || asks.some(a => a.kind === 'approval')) return ['approval', 0];
  if (asks.some(a => ['input', 'plan', 'browser'].includes(a.kind))) return ['input', 0];
  if (t.turn === 'running') return ['working', 0];
  if (t.turn === 'failed') return ['failed', 0];
  const tasks = (ix.kids.get(t.id) ?? 0) + (ix.work.get(t.id) ?? 0);
  if (tasks > 0) return ['monitoring', tasks];
  if (t.env === 'worktree' && !t.worktree && (s.queued[t.id] ?? []).some(q => !q.startsWith('handoff:'))) return ['working', 0];
  return [t.turn === 'completed' ? 'complete' : t.turn === 'interrupted' ? 'stopped' : 'idle', 0];
}
export function status(s: Metadata): [string, string, number][] {
  const ix = sideIndex(s);
  return s.threads.map(t => [t.id, ...phase(ix, s, t)]);
}
export interface Change { $: string; id?: string; thread?: string; msg?: string; key?: string; parent?: string; child?: string; by?: string; source?: string }
const ownId = new Set(['ThreadCreated','ThreadSettled','ThreadUnsettled','ThreadWoke','ThreadArchived','ThreadPinned','ThreadDeleted','WorktreeSet','SessionSet','ModesSet','ThreadSnoozed','ThreadOrdered','WorktreeSkipped','ThreadMerged','WorktreeRemoved','ProviderSet']);
const ownThread = new Set(['TurnChanged','PendingChanged','ActivityLogged','TurnQueued','QueueTaken','AskOpened','QueueEdited','QueueRemoved','QueueMoved','QueueTakenOne','TodosChanged','TodosSet','UsageSet','WorkSet','MessageFrom','TurnUsage']);
const mail = new Set(['ConvSet','MailPosted','MailMarked','MailTagged','RunSet','CharterSet','DecisionLogged']);
const privatePrefixes = ['quiet','output','read','delivery','report','audience','seed'].map(x => 'autosettle.'+x+'.');
function touchId(id: string): string {
  return id.startsWith('@sub:') ? id.slice(5) : id.startsWith('@') && id !== '@mail' ? '*' : id;
}
export function touched(e: Change): string[] {
  const tag = e.$.slice(e.$.lastIndexOf('.')+1);
  let result: string[];
  if (tag === 'CtxNoted') return [];
  if (ownId.has(tag)) result = [e.id!];
  else if (ownThread.has(tag)) result = [e.thread!];
  else if (mail.has(tag)) result = ['@mail'];
  else if (tag === 'MessagePosted') {
    const at = e.msg!.startsWith('t:') ? e.msg!.indexOf(':',2) : -1;
    result = at > 2 ? [e.thread!, e.msg!.slice(2,at)] : [e.thread!];
  } else if (tag === 'SettingSet') {
    const key = e.key!;
    if (key === 'autosettle.version' || privatePrefixes.some(p => key.startsWith(p) && key.length > p.length)) return [];
    if (key.startsWith('liveness.') && key.length > 9) return [touchId(key.slice(9))];
    result = key.startsWith('launch.') || key.startsWith('launched.') ? ['*','!'] : ['*'];
  } else if (tag === 'TaskDelegated') result = [e.parent!,e.child!];
  else if (tag === 'ThreadForked') result = [e.id!,e.source!];
  else if (tag === 'ThreadLaunched') result = [e.id!,e.by!];
  else if (tag === 'ThreadParentSet') result = ['*','!',e.id!,e.parent!];
  else if (tag === 'DeployAsked' || tag === 'DeployBuilt') result = ['*','!'];
  else result = ['*'];
  return result.map(touchId);
}
export interface Pace { last: number; owed: number }
export interface Paced { send: boolean; pace: Pace; wait: number }
export function pace(p: Pace, now: number, level: number): Paced {
  const owed = Math.max(p.owed, level);
  if (!owed) return { send: false, pace: { last: p.last, owed: 0 }, wait: 0 };
  const gap = owed >= 2 ? 50 : 500, elapsed = Math.max(0, now - p.last);
  return elapsed >= gap ? { send: true, pace: { last: now, owed: 0 }, wait: 0 }
    : { send: false, pace: { last: p.last, owed }, wait: gap - elapsed };
}
export function level(ids: string[], th: string, client: string): number {
  if (client && ids.includes('\r'+client)) return 2;
  if (ids.includes('*') || (th && (ids.includes(th) || ids.includes('\t'+th)))) return 2;
  return ids.some(x => !x.startsWith('\t') && !x.startsWith('\r')) ? 1 : 0;
}
export interface Sqlite {
  query(sql: string): { values(...args: (string|number)[]): unknown[][]; finalize(): void };
}
export const TOP = 140737488355328;
export const columns = "seq, ord, id, thread, src, kind, role, akind, CASE WHEN kind = 'msg' THEN text ELSE substr(text, 1, 513) END AS text, at";
export const pageSql = `SELECT * FROM (SELECT * FROM (SELECT ${columns} FROM entry WHERE thread = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3) UNION ALL SELECT * FROM (SELECT ${columns} FROM entry WHERE src = ?1 AND src <> '' AND seq < ?2 ORDER BY seq DESC LIMIT ?3)) ORDER BY seq DESC LIMIT ?3`;
function values(db: Sqlite, sql: string, args: (string|number)[]): unknown[][] {
  const q = db.query(sql);
  try { return q.values(...args); } finally { q.finalize(); }
}
export interface Page { ids: string[]; below: number; above: number; rows: unknown[][]; blocks?: Map<number, string[]> }
export function page(db: Sqlite, thread: string, before: number, limit = 32): Page {
  const cap = Math.min(limit,32), lim = cap+17;
  const bound = before || TOP;
  const seq = Number(values(db,'SELECT coalesce((SELECT seq FROM entry WHERE ord >= ?1 ORDER BY ord LIMIT 1), 140737488355328)',[bound])[0][0]);
  const rows = values(db,pageSql,[thread,seq,lim]);
  const below = rows.length === lim ? Number(values(db,'SELECT (SELECT count(*) FROM entry WHERE thread = ?1 AND seq < ?2) + (SELECT count(*) FROM entry WHERE src = ?1 AND src <> \'\' AND seq < ?2)',[thread,Number(rows.at(-1)![0])])[0][0]) : 0;
  const above = before ? Number(values(db,'SELECT (SELECT count(*) FROM entry WHERE thread = ?1 AND seq >= ?2) + (SELECT count(*) FROM entry WHERE src = ?1 AND src <> \'\' AND seq >= ?2)',[thread,seq])[0][0]) : 0;
  // Window's observable contract: insertion ordinals, never user message IDs.
  return { ids: [...new Set(rows.map(r=>Number(r[1])))].sort((a,b)=>b-a).filter(n=>!before || n<before).slice(0,cap).map(String), below, above, rows };
}

// Same 512-byte, code-point-aligned blocks as T.Blocks.of. Arrays replace
// linked character accumulators; no body or renderer-ready block is omitted.
export function blocks(text: string): string[] {
  const out: string[] = [], chars: string[] = [];
  let bytes = 0;
  for (const c of text) {
    const cp = c.codePointAt(0)!;
    const width = cp < 128 ? 1 : cp < 2048 ? 2 : cp < 65536 ? 3 : 4;
    if (bytes + width > 512) {
      out.push(chars.join('')); chars.length = 0; bytes = 0;
    }
    chars.push(c); bytes += width;
  }
  out.push(chars.join(''));
  return out;
}
export function fullPage(db: Sqlite, thread: string, before: number, limit = 32): Page {
  const result = page(db, thread, before, limit);
  result.blocks = new Map(result.rows.map(r => [Number(r[1]), blocks(String(r[8]))]));
  return result;
}
