import { Database } from 'bun:sqlite';
import { createReadStream } from 'node:fs';
import { createGunzip } from 'node:zlib';
import { createInterface } from 'node:readline';
import { mkdir } from 'node:fs/promises';
const folder = process.argv[2];
const manifest=await Bun.file('test/perf/bend-vs-ts/input/manifest.json').json();
await mkdir(folder,{recursive:true});
for (const scale of [1,2]) {
  const path = `${folder}/history-${scale}.db`;
  const db = new Database(path, { create:true });
  db.exec("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; CREATE TABLE entry(seq INTEGER PRIMARY KEY, ord INTEGER NOT NULL UNIQUE, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER)");
  const insert = db.query('INSERT INTO entry VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)');
  let rows = 0;
  const lengths = new Map<number,string>();
  for(let copy=0;copy<scale;copy++) {
    db.exec('BEGIN');
    const lines = createInterface({ input:createReadStream('test/perf/bend-vs-ts/input/entries.jsonl.gz').pipe(createGunzip()) });
    for await (const line of lines) {
      const r = JSON.parse(line);
      const n = r[8];
      let text=lengths.get(n);
      if(text===undefined) { text='x'.repeat(n); if(lengths.size<20000) lengths.set(n,text); }
      insert.run(r[0]+copy*manifest.events,r[1]+copy*manifest.entries,r[2],r[3],r[4],r[5],r[6],r[7],text,r[9]);
      rows++;
    }
    db.exec('COMMIT');
  }
  db.exec("CREATE INDEX entry_thread ON entry(thread,seq); CREATE INDEX entry_src ON entry(src,seq) WHERE src<>''; ANALYZE;");
  const threads = db.query('SELECT thread,count(*) AS n FROM entry GROUP BY thread ORDER BY n DESC').all() as {thread:string,n:number}[];
  const chosen = [...threads.slice(0,4), ...threads.slice(Math.floor(threads.length/2),Math.floor(threads.length/2)+4)];
  const queries = chosen.flatMap(t => [{thread:t.thread,before:0}, {thread:t.thread,before:Number(db.query('SELECT ord FROM entry WHERE thread=?1 ORDER BY seq DESC LIMIT 1 OFFSET ?2').get(t.thread,Math.floor(t.n/2))!.ord)}]);
  await Bun.write(`${folder}/queries-${scale}.json`,JSON.stringify(queries));
  await Bun.write(`${folder}/db-${scale}.json`,JSON.stringify({rows,bytes:Bun.file(path).size,queries:queries.length,plan:db.query('EXPLAIN QUERY PLAN SELECT * FROM entry WHERE thread=?1 AND seq<?2 ORDER BY seq DESC LIMIT 49').all(chosen[0].thread,1450000)}));
  insert.finalize(); db.close();
}
