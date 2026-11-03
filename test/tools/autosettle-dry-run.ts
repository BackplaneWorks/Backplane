// bun test/tools/autosettle-dry-run.ts COPIED-store.db COMPILED.mjs [UNIX_TIME]
// Make the backup first with sqlite3.Connection.backup from a mode=ro source.
// This tool opens ONLY that copy, read-only, and evaluates the pure Bend policy.
import {Database} from 'bun:sqlite';
import {realpathSync} from 'node:fs';
import {resolve, dirname, join} from 'node:path';
import {pathToFileURL} from 'node:url';
const [file, moduleFile, stamp] = process.argv.slice(2);
if (!file || !moduleFile) throw Error('usage: COPIED-store.db COMPILED.mjs [UNIX_TIME]');
const copy = realpathSync(file);
const liveHome = realpathSync(process.env.BACKPLANE_LIVE_HOME || join(process.env.HOME!, '.backplane-bend'));
if (dirname(copy) === liveHome || copy.startsWith(liveHome + '/')) throw Error('Refusing live hub home: supply a separate SQLite backup');
const lib = (await import(pathToFileURL(resolve(moduleFile)).href)).default;
const db = new Database(copy, {readonly:true});
const asStrings = (sql: string, ...params: any[]) => db.query(sql).values(...params).map(row=>row.map(v=>v==null?'':String(v)));
const data: [string, string[][]][] = [];
for (const table of ['project','thread','task','ask','queue','bot','room','routine','conv','mark']) {
  data.push([table, asStrings(`SELECT ${lib.columns(table)} FROM ${table}`)]);
}
for (const [domain] of db.query('SELECT DISTINCT domain FROM obj').values()) {
  data.push([`obj.${domain}`, asStrings('SELECT domain, key, json FROM obj WHERE domain = ?', domain)]);
}
data.push(['setting', asStrings('SELECT k, v, cold FROM setting WHERE cold = 0')]);
data.push(['mail', asStrings('SELECT seq, id, conv, src, dst, kind, ref, want, head, size, at FROM mail ORDER BY seq')]);
data.push(['post', asStrings('SELECT seq, id, room, src, text, hop, at FROM post ORDER BY seq')]);
const ids = lib.open_ids(JSON.stringify(data));
data.push(['entry', asStrings(lib.window(), ids, 200)]);
const now = stamp ? BigInt(stamp) : BigInt(Math.floor(Date.now()/1000));
const seq = db.query("SELECT v FROM meta WHERE k='applied_seq'").get() as {v:string};
const t = performance.now();
const result = JSON.parse(lib.run(JSON.stringify(data), now));
db.close();
console.log(JSON.stringify({copy, snapshotSequence:seq.v, evaluatedAt:String(now), policyMs:performance.now()-t, ...result},null,2));
