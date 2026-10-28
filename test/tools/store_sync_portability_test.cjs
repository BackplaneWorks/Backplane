// Real Bun/SQLite file flush and promotion in disposable ASCII/Unicode homes.
// No hub, provider, live profile, credential, or device is opened.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const vm=require('node:vm'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {Database}=require('bun:sqlite');
const source=fs.readFileSync(path.join(__dirname,'../../src/server/effects/storeio.js'),'utf8');
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
function effects(io=fs){const c={require:n=>n==='fs'?io:require(n),Buffer,process,console:{warn(){}},io_eff(){},CID:x=>x,Nil:'Nil',Con:'Con',Disk:{}};vm.createContext(c);vm.runInContext(source,c);return c;}
function database(file,value){const db=new Database(file,{create:true});try{db.exec('CREATE TABLE probe(value TEXT)');db.query('INSERT INTO probe VALUES (?)').run(value);}finally{db.close();}}
function value(file){const db=new Database(file,{readonly:true});try{return db.query('SELECT value FROM probe').get().value;}finally{db.close();}}
let count=0;
for(const label of ['ascii','日本語 space']){
 const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-store-sync-'+label+'-'));
 try{
  const log=path.join(home,'events.jsonl'),target=path.join(home,'store.db');fs.writeFileSync(log,'authoritative 日本語\n');const logBefore=hash(log);database(target,'old');const oldHash=hash(target);
  if(process.platform==='win32'){
   const fd=fs.openSync(target,'r');try{assert.throws(()=>fs.fsyncSync(fd),e=>['EPERM','EACCES','EBADF'].includes(e.code),'Establish actual Windows read-handle flush rejection');}finally{fs.closeSync(fd);}
   count++;
  }
  assert.equal(effects().store_sync(target),'ok');assert.equal(hash(target),oldHash);count++;
  const temp=path.join(home,'.store-rebuild-positive');fs.mkdirSync(temp);fs.symlinkSync(log,path.join(temp,'events.jsonl'));const next=path.join(temp,'store.db');database(next,'new');
  // Actual Mac startup retained this exact closed-cache residue: empty WAL
  // plus a 32 KiB shared index. Neither contains pending authoritative data.
  fs.writeFileSync(next+'-wal',Buffer.alloc(0));fs.writeFileSync(next+'-shm',Buffer.alloc(32768));
  const trace=[],opened=new Map(),io=Object.create(fs);
  io.openSync=(p,...args)=>{const fd=fs.openSync(p,...args);opened.set(fd,p);return fd;};
  io.fsyncSync=fd=>{fs.fsyncSync(fd);trace.push({kind:'sync',path:opened.get(fd)});};
  io.closeSync=fd=>{opened.delete(fd);return fs.closeSync(fd);};
  io.renameSync=(from,to)=>{trace.push({kind:'rename',from,to});return fs.renameSync(from,to);};
  assert.equal(effects(io).store_promote(home,temp),'ok');assert.equal(value(target),'new');const backups=fs.readdirSync(home).filter(n=>/^store\.db\.previous-\d+$/.test(n));assert.equal(backups.length,1);const backup=path.join(home,backups[0]);assert.equal(hash(backup),oldHash);assert.equal(value(backup),'old');assert(!fs.existsSync(temp));
  const renamed=trace.findIndex(e=>e.kind==='rename'&&e.to===target);assert(renamed>0);assert(trace.slice(0,renamed).some(e=>e.kind==='sync'&&e.path===next));assert(trace.slice(0,renamed).some(e=>e.kind==='sync'&&e.path===backup));assert.equal(hash(log),logBefore);count++;
  const failTemp=path.join(home,'.store-rebuild-rejected');fs.mkdirSync(failTemp);fs.symlinkSync(log,path.join(failTemp,'events.jsonl'));database(path.join(failTemp,'store.db'),'must-not-promote');const currentHash=hash(target),failing=Object.create(fs);failing.fsyncSync=()=>{throw Object.assign(Error('injected real sync failure'),{code:'EPERM'});};
  assert.match(effects(failing).store_promote(home,failTemp),/^fail injected real sync failure/);assert.equal(hash(target),currentHash);assert.equal(value(target),'new');assert(fs.existsSync(path.join(failTemp,'store.db')));assert.equal(hash(log),logBefore);count++;
  assert.match(effects().store_sync(path.join(home,'missing.db')),/^fail /);count++;
  for(const kind of ['nonempty-wal','unexpected-file','sidecar-directory']){
   const rejected=path.join(home,'.store-rebuild-'+kind);fs.mkdirSync(rejected);fs.symlinkSync(log,path.join(rejected,'events.jsonl'));database(path.join(rejected,'store.db'),'must-stay-staged');
   if(kind==='nonempty-wal')fs.writeFileSync(path.join(rejected,'store.db-wal'),'uncheckpointed bytes');
   if(kind==='unexpected-file')fs.writeFileSync(path.join(rejected,'keep.me'),'retained');
   if(kind==='sidecar-directory')fs.mkdirSync(path.join(rejected,'store.db-shm'));
   const current=hash(target),names=fs.readdirSync(rejected).sort();assert.match(effects().store_promote(home,rejected),/^fail /);assert.equal(hash(target),current);assert.equal(value(target),'new');assert.deepEqual(fs.readdirSync(rejected).sort(),names);assert.equal(hash(log),logBefore);count++;
  }
  console.log('PASS '+label+': real file flush, ordered atomic promotion/backup, fatal sync errors, unchanged log');
 }finally{fs.rmSync(home,{recursive:true,force:true});}
}
console.log(JSON.stringify({status:'PASS',platform:process.platform,arch:process.arch,bun:Bun.version,checks:count,scope:'Actual filehandle/promotion and error propagation only; no full service or desktop qualification'}));
