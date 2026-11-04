// Actual SQLite fixture state/rollback and setup cost; no production gate.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {Database}=require('bun:sqlite');
const [baseRoot,root,out]=process.argv.slice(2);
const before=fs.readFileSync(path.join(baseRoot,'test/tools/store_checkpoint_io_test.cjs'),'utf8');
const after=fs.readFileSync(path.join(root,'test/tools/store_checkpoint_io_test.cjs'),'utf8');
const storedb=fs.readFileSync(path.join(root,'src/core/storedb.bend'),'utf8');
const C=/def Sdb\.codec\(\) -> Nat:\s*(\d+)n/.exec(storedb)[1];
const schemaSource=storedb.split('def Sdb.schema() -> String:')[1].split('# a table')[0];
const schema=[...schemaSource.matchAll(/"(?:[^"\\]|\\.)*"/g)].map(m=>JSON.parse(m[0])).filter(s=>s.startsWith('CREATE ')).join(';');
assert(!schema.split(';').some(s=>/^(?:BEGIN|COMMIT|END|ROLLBACK|SAVEPOINT|RELEASE)(?:\s|$)/i.test(s.trim())), 'nested transaction statement in schema');
function parts(text){const a=text.indexOf('function db('),b=text.indexOf('\nfunction pending(',a);assert(a>=0&&b>a);return [text.slice(0,a),text.slice(a,b),text.slice(b)];}
const oldParts=parts(before),newParts=parts(after);
assert.equal(oldParts[0],newParts[0]);assert.equal(oldParts[2],newParts[2]);
function make(body,sqlite=Database,sql=schema){return new Function('Database','schema','C',body+'\nreturn db;')(sqlite,sql,C);}
const create={before:make(oldParts[1]),after:make(newParts[1])};
function normalize(v){
  if(typeof v==='bigint')return {$bigint:v.toString()};
  if(ArrayBuffer.isView(v))return {$bytes:Buffer.from(v.buffer,v.byteOffset,v.byteLength).toString('hex')};
  if(Array.isArray(v))return v.map(normalize);
  if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,normalize(v[k])]));
  return v;
}
function snapshot(file){
  const db=new Database(file,{readonly:true,safeIntegers:true});
  try {
    const master=db.query('SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name').all();
    const pragmas={};for(const name of ['journal_mode','page_size','synchronous','user_version','wal_autocheckpoint','integrity_check'])pragmas[name]=db.query('PRAGMA '+name).values();
    assert.deepEqual(pragmas.integrity_check,[['ok']]);
    const tables={};for(const m of master.filter(m=>m.type==='table')){
      const rows=db.query('SELECT * FROM "'+m.name.replaceAll('"','""')+'"').all().map(normalize);
      rows.sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));tables[m.name]=rows;
    }
    return normalize({master,tables,pragmas});
  }finally{db.close();}
}
const samples=[];
for(let trial=0;trial<5;trial++){
  const pair={};
  for(const variant of (trial%2?['after','before']:['before','after'])){
    const dir=fs.mkdtempSync(path.join(out,'fixture-'+variant+'-'));const file=path.join(dir,'fixture.db');
    try {
      const start=performance.now();create[variant](file,'old',C);const ms=performance.now()-start;
      const files=fs.readdirSync(dir).sort().map(name=>({name,bytes:fs.statSync(path.join(dir,name)).size}));
      pair[variant]={milliseconds:ms,files,state:snapshot(file)};
    }finally{fs.rmSync(dir,{recursive:true,force:true});}
  }
  assert.deepEqual(pair.before.state,pair.after.state,'logical schema/rows/default PRAGMA parity');
  assert.deepEqual(pair.before.files,pair.after.files,'same fixture file set and sizes');
  samples.push(pair);
}
// The actual candidate helper must roll back schema+rows and close on an error.
let captured;
const Tracked=new Proxy(Database,{construct(target,args){captured=Reflect.construct(target,args,target);return captured;}});
const failDir=fs.mkdtempSync(path.join(out,'rollback-')),failFile=path.join(failDir,'fixture.db');
let rollback;
try {
  const failing=make(newParts[1],Tracked,schema+';INSERT INTO __missing_fixture_table VALUES (1);');
  assert.throws(()=>failing(failFile,'old',C),/no such table/i);
  assert.throws(()=>captured.query('SELECT 1').get(),/closed/i,'finally must close real handle');
  const reopened=new Database(failFile,{readonly:true});
  try {assert.deepEqual(reopened.query("SELECT name FROM sqlite_master WHERE type='table'").all(),[]);}
  finally{reopened.close();}
  rollback={partial_tables:0,closed:true};
}finally{fs.rmSync(failDir,{recursive:true,force:true});}
const stats={};for(const variant of ['before','after']){
  const values=samples.map(p=>p[variant].milliseconds).sort((a,b)=>a-b);
  stats[variant]={min_ms:values[0],median_ms:values[2],max_ms:values[4],samples_ms:samples.map(p=>p[variant].milliseconds)};
}
fs.writeFileSync(path.join(out,'FIXTURE-PARITY.json'),JSON.stringify({samples,stats,rollback,schema_transaction_free:true,
  unchanged_test_prefix_suffix:true,source_codec:C,qualification_scope:'fixture construction only; not original timeout root cause or full CI'},null,2)+'\n');
console.log(JSON.stringify({fixture_parity:true,stats,rollback}));
