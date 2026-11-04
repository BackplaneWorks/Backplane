// Real SQLite and unchanged production effects; fixture-only parity controls.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {Database}=require('bun:sqlite');
const [base,root,out]=process.argv.slice(2);
const relative='test/tools/store_promote_test.cjs';
const texts={before:fs.readFileSync(path.join(base,relative),'utf8'),after:fs.readFileSync(path.join(root,relative),'utf8')};
function split(text){const a=text.indexOf('function open(c, file) {'),b=text.indexOf('\nfunction read(file)',a);assert(a>=0&&b>a);return [text.slice(0,a),text.slice(a,b),text.slice(b)];}
const old=split(texts.before),next=split(texts.after);assert.equal(old[0],next[0]);assert.equal(old[2],next[2]);
function load(text,dir){
  const end=text.indexOf('let count = 0, failures = 0;');assert(end>0);
  return new Function('require','__dirname',text.slice(0,end)+'\nreturn {effects,open,build,schema,codec};')(require,path.join(dir,'test/tools'));
}
const suites={before:load(texts.before,base),after:load(texts.after,root)};
function normalize(v){
  if(typeof v==='bigint')return {$bigint:String(v)};
  if(ArrayBuffer.isView(v))return {$bytes:Buffer.from(v.buffer,v.byteOffset,v.byteLength).toString('hex')};
  if(Array.isArray(v))return v.map(normalize);
  if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,normalize(v[k])]));
  return v;
}
function state(db){
  const master=db.query('SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name').all();
  const pragmas={};
  for(const name of ['journal_mode','synchronous','busy_timeout','page_size','user_version','wal_autocheckpoint','integrity_check'])pragmas[name]=db.query('PRAGMA '+name).values();
  assert.deepEqual(pragmas.integrity_check,[['ok']]);
  const tables={};for(const entry of master.filter(m=>m.type==='table')){
    const rows=db.query('SELECT * FROM "'+entry.name.replaceAll('"','""')+'"').all().map(normalize);
    rows.sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));tables[entry.name]=rows;
  }
  return normalize({master,tables,pragmas});
}
const samples=[];
for(let trial=0;trial<5;trial++){
  const pair={};for(const variant of (trial%2?['after','before']:['before','after'])){
    const dir=fs.mkdtempSync(path.join(out,'setup-'+variant+'-')),file=path.join(dir,'store.db'),suite=suites[variant],c=suite.effects();
    let h;
    try {
      const began=performance.now();h=suite.open(c,file);const milliseconds=performance.now()-began;
      const db=c.bp_db_get(h);assert.equal(db.inTransaction,false);
      pair[variant]={milliseconds,state:state(db)};
      assert.deepEqual(pair[variant].state.pragmas.journal_mode,[['wal']]);
      assert.equal(Number(db.query('PRAGMA synchronous').values()[0][0]),1);
      assert.equal(c.db_close(h),'ok');h=null;
      pair[variant].files=fs.readdirSync(dir).sort().map(name=>({name,bytes:fs.statSync(path.join(dir,name)).size}));
    }finally{if(h!==undefined&&h!==null)c.db_close(h);fs.rmSync(dir,{recursive:true,force:true});}
  }
  assert.deepEqual(pair.before.state,pair.after.state,'schema/meta rows/live effect PRAGMA parity');
  assert.deepEqual(pair.before.files.map(f=>f.name),pair.after.files.map(f=>f.name));samples.push(pair);
}
// Build's marker/PERSIST_WAL/checkpoint/copy/close behavior is original code.
const modes=[];
for(const mode of ['persistent','empty-wal','wal-only','no-sidecars']){
  const pair={};for(const variant of ['before','after']){
    const dir=fs.mkdtempSync(path.join(out,'mode-'+variant+'-')),file=path.join(dir,'store.db');
    try{
      suites[variant].build(file,'parity marker 日本語',mode);
      const names=fs.readdirSync(dir).sort();
      const db=new Database(file,{readonly:true,safeIntegers:true});
      try{pair[variant]={state:state(db),files:names.map(name=>({name,bytes:fs.statSync(path.join(dir,name)).size})),
        wal_empty:fs.existsSync(file+'-wal')?fs.statSync(file+'-wal').size===0:null};}
      finally{db.close();}
    }finally{fs.rmSync(dir,{recursive:true,force:true});}
  }
  assert.deepEqual(pair.before.state,pair.after.state,'mode '+mode+' logical parity');
  assert.deepEqual(pair.before.files.map(f=>f.name),pair.after.files.map(f=>f.name),'mode sidecar set');
  assert.equal(pair.before.wal_empty,pair.after.wal_empty,'mode WAL emptiness');
  modes.push({mode,...pair});
}
// Exercise actual changed helper with real SQLite failures, no fake Database.
const failures=[];
for(const kind of ['schema','meta']){
  const suite=suites.after,c=suite.effects(),dir=fs.mkdtempSync(path.join(out,'rollback-')),file=path.join(dir,'store.db');
  const sql=suite.schema+(kind==='schema'?';INSERT INTO __missing_fixture_table VALUES (1);':
    ";CREATE TRIGGER fixture_meta_failure BEFORE INSERT ON meta BEGIN SELECT RAISE(ABORT,'fixture meta failure'); END;");
  const failing=new Function('assert','schema','codec',next[1]+'\nreturn open;')(assert,sql,suite.codec);
  const original=c.db_open;let handle,actual,error;
  c.db_open=file=>{const result=original(file);assert.match(result,/^ok /);handle=result.slice(3);actual=c.bp_db_get(handle);return result;};
  try{
    try{failing(c,file);assert.fail('injected setup must fail');}catch(e){error=e;}
    if(kind==='schema'){assert.equal(error.code,'ERR_ASSERTION');assert.match(error.message,/no such table: __missing_fixture_table/);}
    else{assert.equal(error.name,'SQLiteError');assert.match(error.message,/fixture meta failure/);}
    assert.equal(c.bp_db_get(handle),null,'actual slot must be released');
    assert.throws(()=>actual.query('SELECT 1').get(),/closed/i,'actual handle must close');
    const reopened=new Database(file,{readonly:true});
    try{assert.deepEqual(reopened.query("SELECT name FROM sqlite_master WHERE type='table'").all(),[]);}
    finally{reopened.close();}
    failures.push({kind,name:error.name,code:error.code??null,message:error.message,partial_tables:0,slot_released:true,handle_closed:true});
  }finally{fs.rmSync(dir,{recursive:true,force:true});}
}
const stats={};for(const variant of ['before','after']){
  const values=samples.map(p=>p[variant].milliseconds).sort((a,b)=>a-b);
  stats[variant]={min_ms:values[0],median_ms:values[2],max_ms:values[4],samples_ms:samples.map(p=>p[variant].milliseconds)};
}
fs.writeFileSync(path.join(out,'FIXTURE-PARITY.json'),JSON.stringify({stats,samples,modes,failures,unchanged_test_prefix_suffix:true,
  note:'Open-helper timing excludes state inspection and close; mode snapshots compare logical and required sidecar semantics, not physical DB/WAL byte identity.'},null,2)+'\n');
console.log(JSON.stringify({fixture_parity:true,stats,modes:modes.map(m=>m.mode),failures}));
