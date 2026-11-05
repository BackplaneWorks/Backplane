// bun fixture.ts OUT.db : a fresh store.db with the real hub schema (schema.sql, taken from a store
// the hub itself created) and 4 threads x 200 entries, text lengths as native/bend/history.bend.
import {Database} from 'bun:sqlite';
import {readFileSync, rmSync} from 'node:fs';
const out = process.argv[2];
rmSync(out, {force: true});
const db = new Database(out, {create: true});
db.exec('PRAGMA journal_mode=WAL');
db.exec(readFileSync(new URL('./schema.sql', import.meta.url), 'utf8'));
const h1 = (x: number) => (x * 48271) % 2147483647;
const h = (i: number) => h1(h1(i + 12345));
const text = (len: number) => { let s = ''; for (let p = 0; p < len; p++) s += p % 97 === 96 ? 'é' : p % 211 === 210 ? '\n' : p % 307 === 306 ? '\t' : 'x'; return s; };
const ins = db.query('INSERT INTO entry(seq,ord,id,thread,src,kind,role,akind,text,at) VALUES (?,?,?,?,?,?,?,?,?,?)');
db.exec('BEGIN');
let n = 0;
for (let ord = 1; ord <= 800; ord++) {
  const msg = ord % 2 === 0, len = 40 + (h(ord) % 1900);
  ins.run(ord, ord, 'e' + ord, 't' + (ord % 4), '', msg ? 'msg' : 'act', msg ? (ord % 4 === 0 ? 'user' : 'assistant') : 'tool', msg ? '' : 'bash', text(len), 1000000 + ord);
  n++;
}
db.exec('COMMIT');
console.log('fixture rows', n);
