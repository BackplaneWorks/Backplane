// The exact production query against an independent old-query oracle. Private
// copied databases are optional inputs and neither rows nor identifiers escape.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {Database}=require('bun:sqlite');
const source=fs.readFileSync(path.join(__dirname,'../../src/core/thinq.bend'),'utf8');
const sql=source.match(/def Q.sql.recent\(\)[\s\S]*?\n  "([^"\n]+)"/)[1];
const old="SELECT thread, max(CASE WHEN kind = 'msg' AND role = 'user' THEN at ELSE 0 END) FROM entry GROUP BY thread";
function compare(db){
 const receipt={};let expected;
 for(const [name,q] of [['before',old],['after',sql]]){
  db.exec('PRAGMA shrink_memory');const start=performance.now(),rows=db.query(q).values();
  receipt[name]={ms:performance.now()-start,rows:rows.length,plan:db.query('EXPLAIN QUERY PLAN '+q).values().map(r=>r[3])};
  if(expected)assert.deepEqual(rows,expected);else expected=rows;
 }
 assert(receipt.after.plan.some(p=>p.includes('COVERING INDEX entry_thread')));
 assert(receipt.after.plan.some(p=>p.includes('entry_user')));
 receipt.equal=true;return receipt;
}
const fixture=new Database(':memory:');
fixture.exec("CREATE TABLE entry(thread TEXT,seq INTEGER,kind TEXT,role TEXT,at INTEGER,text TEXT); CREATE INDEX entry_thread ON entry(thread,seq); CREATE INDEX entry_user ON entry(thread,at) WHERE kind='msg' AND role='user'");
const insert=fixture.prepare('INSERT INTO entry VALUES(?,?,?,?,?,?)');
for(const r of [['orphan',1,'act','tool',7,'large'],['a',2,'msg','user',1793700000,'u'],['a',3,'msg','assistant',1793700010,'a'],['a',4,'msg','user',1793700005,'v'],['b',5,'act','step',9,'s'],['猫',6,'msg','user',0,'z'],['',7,'msg','user',281474976710655,'edge']])insert.run(...r);
const results={synthetic:compare(fixture)};fixture.close();
if(process.argv[2]){assert.equal(process.argv[2],'--db');const db=new Database(process.argv[3],{readonly:true});results.privateCopy=compare(db);db.close();}
console.log(JSON.stringify(results));
