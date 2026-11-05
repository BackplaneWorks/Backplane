// bun oracle.ts DB THREAD... : the JS answer for /history on the same store: bun:sqlite + the repo's TS
// equivalents (test/perf/bend-vs-ts); prints one JSON line per thread, the host's canonical body.
import {Database} from 'bun:sqlite';
import * as ts from '../../../perf/bend-vs-ts/equivalents';
export function answer(db: Database, thread: string): string {
  const seq = Number((db.query('SELECT coalesce((SELECT seq FROM entry WHERE ord >= ?1 ORDER BY ord LIMIT 1), 140737488355328)').values(140737488355328)[0] as any)[0]);
  const rows = db.query(ts.pageSql).values(thread, seq, 49) as any[][];
  const built = rows.map(r => ({ord: Number(r[1]), lens: ts.blocks(String(r[8])).map(b => [...b].length)}));
  const ids = [...new Set(built.map(r => r.ord))].sort((a, b) => b - a).slice(0, 32);
  built.sort((a, b) => b.ord - a.ord);
  return JSON.stringify({thread, ids, rows: built.map(r => [r.ord, r.lens])});
}
if (import.meta.main) {
  const [path, ...threads] = process.argv.slice(2);
  const db = new Database(path, {readonly: true});
  for (const t of threads) console.log(answer(db, t));
}
