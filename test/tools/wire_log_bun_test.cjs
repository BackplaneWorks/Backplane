// Run on Bun: the actual effect closure must work without db.js globals.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm');
const {Database}=require('bun:sqlite');
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-wire-real-bun-'));
let writer;
try {
  writer=new Database(path.join(home,'store.db'));
  writer.exec('PRAGMA journal_mode=WAL;CREATE TABLE line(seq INTEGER PRIMARY KEY,off INTEGER,len INTEGER,wire TEXT)');
  const texts=['{"first":1}','{"private":"blank"}','{"last":3}'];
  fs.writeFileSync(path.join(home,'events.jsonl'),texts.join('\n')+'\n');
  const insert=writer.prepare('INSERT INTO line VALUES(?,?,?,?)');let off=0;
  for(let i=0;i<texts.length;i++){insert.run(i+1,off,Buffer.byteLength(texts[i]),i===1?'{}':'');off+=Buffer.byteLength(texts[i])+1;}insert.finalize();
  const effects={},ctx=vm.createContext({require,Buffer,CID:x=>x,io_eff:(k,f)=>effects[k]=f,WireLog:{page:'page'},WirePage:'WirePage',Nil:'Nil',Con:'Con'});
  vm.runInContext(fs.readFileSync(path.join(__dirname,'../../src/server/effects/wire_log.js'),'utf8'),ctx);
  const array=xs=>{const out=[];for(;xs.$==='Con';xs=xs.tail)out.push(xs.head);return out;};
  for(let i=0;i<20;i++){const p=effects.page('hub-owned',home,0,3);assert.equal(p.error,'');assert.deepEqual(array(p.lines),[texts[0],'{}',texts[2]]);}
  assert.equal(effects.page('hub-owned',home,3,3).error,'');
  fs.truncateSync(path.join(home,'events.jsonl'),1);assert.notEqual(effects.page('hub-owned',home,0,3).error,'');
  writer.close();writer=null;
  const again=new Database(path.join(home,'store.db'));assert.equal(again.query('SELECT count(*) n FROM line').get().n,3);again.close();
  console.log('ok real Bun effect closure, readonly WAL pages, public overrides, repeated close/reopen and truncated-log failure');
} finally {if(writer)writer.close();fs.rmSync(home,{recursive:true,force:true});}
