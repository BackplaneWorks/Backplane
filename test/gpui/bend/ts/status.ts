// Plain-TS mirror of native/bend/status.bend.
import * as ts from '../../../perf/bend-vs-ts/equivalents';
const [nArg = '1000', repsArg = '50', innerArg = '1'] = process.argv.slice(2);
const n = Number(nArg), reps = Number(repsArg), inner = Number(innerArg) || 1, M = 4294967291;
const h1 = (x: number) => (x * 48271) % 2147483647;
const h = (i: number) => h1(h1(i + 12345));
const np = Math.floor(n / 10) + 1;
const turns = ['idle', 'running', 'completed', 'interrupted', 'failed'];
const kinds = ['approval', 'input', 'plan', 'browser', 'other'];
const tstates = ['queued', 'running', 'done', 'failed', 'cancelled'];
const s: ts.Metadata = { threads: [], asks: [], tasks: [], queued: {}, settings: {} };
let t0 = performance.now();
for (let i = 0; i < n; i++) {
  const hh = h(i), id = 't' + i;
  s.threads.push({ id, project: 'p' + ((i * 7) % np), turn: turns[Math.floor(hh / 8) % 5], pending: hh % 23 === 0 ? 1 : 0,
    env: i % 9 === 0 ? 'worktree' : 'local', worktree: i % 18 === 0, deleted: i % 41 === 0, archived: i % 13 === 0 });
  if (i % 11 === 0) s.asks.push({ id: 'a' + i, thread: id, kind: kinds[Math.floor(hh / 64) % 5], open: Math.floor(hh / 512) % 2 === 0 });
  if (i % 6 === 0) s.tasks.push({ id: 'k' + i, parent: id, child: 't' + (i + 1), state: tstates[Math.floor(hh / 32) % 5] });
  if (i % 17 === 0) s.settings['bg.' + id] = 'b\twork\nc\twork';
  if (i % 29 === 0) s.settings['watch.' + id] = 'x\ty';
  if (i % 31 === 0) s.settings['order.wait.' + id] = '1';
  if (i % 5 === 0) s.queued[id] = [Math.floor(hh / 4) % 2 === 0 ? 'start' : 'handoff:x'];
}
console.log('threads ' + s.threads.length); console.log('gen_ms ' + Math.round(performance.now() - t0));
const code: Record<string, number> = { approval: 0, input: 1, working: 2, monitoring: 3, failed: 4, complete: 5, stopped: 6, idle: 7 };
function once(): number {
  let acc = 0;
  for (const [, name, k] of ts.status(s)) acc = (acc * 31337 + code[name] + (name === 'monitoring' ? 11 * k : 0)) % M;
  return acc;
}
for (let r = 0; r < reps; r++) {
  t0 = performance.now(); let d = 0; for (let k = 0; k < inner; k++) d = once(); const ms = (performance.now() - t0) / inner;
  console.log('digest ' + d); console.log('ms ' + ms.toFixed(4));
}
