const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const { DatabaseSync } = require('node:sqlite');
const source = fs.readFileSync(path.join(__dirname,'../../src/server/effects/wire_log.js'),'utf8');
function fixture(t, texts, overrides={}) {
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-wire-page-')), db=new DatabaseSync(path.join(home,'store.db'));
  t.after(()=>{db.close();fs.rmSync(home,{recursive:true,force:true});});
  db.exec('CREATE TABLE line(seq INTEGER PRIMARY KEY, off INTEGER, len INTEGER, wire TEXT)');
  const add=db.prepare('INSERT INTO line VALUES(?,?,?,?)');let off=0;
  for(let i=0;i<texts.length;i++){const b=Buffer.byteLength(texts[i]);add.run(i+1,off,b,overrides[i+1]||'');off+=b+1;}
  fs.writeFileSync(path.join(home,'events.jsonl'),texts.join('\n')+'\n');
  const stats={opened:0,closed:0,prepared:0,finalized:0,connections:0,released:0}, io=new Proxy(fs,{get(o,k){
    if(k==='openSync')return(...a)=>{stats.opened++;return o.openSync(...a)};
    if(k==='closeSync')return(...a)=>{stats.closed++;return o.closeSync(...a)};return o[k];}});
  class Database {
    constructor(file, opts){assert.equal(opts.readonly,true);this.db=new DatabaseSync(file,{readOnly:true});stats.connections++;}
    exec(sql){return this.db.exec(sql);}
    prepare(sql){stats.prepared++;const stmt=this.db.prepare(sql);return {iterate:(...a)=>stmt.iterate(...a),finalize(){stats.finalized++}};}
    close(){this.db.close();stats.released++;}
  }
  const effects={},context=vm.createContext({require:n=>n==='fs'?io:n==='bun:sqlite'?{Database}:require(n),Buffer,CID:x=>x,io_eff:(id,fn)=>effects[id]=fn,
    WireLog:{page:'page'},WirePage:'WirePage',Con:'Con',Nil:'Nil'});
  vm.runInContext(source,context);
  return {home,db,stats,page:(a,z)=>effects.page('owned',home,a,z),array:a=>{const r=[];for(;a.$==='Con';a=a.tail)r.push(a.head);return r;}};
}
test('disk cursors reconstruct exact ordered history across row and byte limits with public wire overrides', t=>{
  const texts=Array.from({length:2500},(_,i)=>JSON.stringify({i,text:'é'.repeat(450)}));
  const f=fixture(t,texts,{2:'{"public":"redacted"}'}), expected=texts.slice();expected[1]='{"public":"redacted"}';
  let at=0;const got=[];
  while(at<texts.length){const p=f.page(at,texts.length);assert.equal(p.error,'');const xs=f.array(p.lines);assert.ok(xs.length>0&&xs.length<=1024);assert.ok(xs.reduce((n,x)=>n+Buffer.byteLength(x),0)<=1048576);got.push(...xs);at+=xs.length;}
  assert.deepEqual(got,expected);assert.equal(f.stats.opened,f.stats.closed);assert.equal(f.stats.prepared,f.stats.finalized);assert.equal(f.stats.connections,f.stats.released);
});
test('an indivisible large event travels alone and no following event is lost',t=>{
  const texts=['x'.repeat(1048577),'tail'],f=fixture(t,texts);const p=f.page(0,2);
  assert.deepEqual(f.array(p.lines),[texts[0]]);assert.deepEqual(f.array(f.page(1,2).lines),['tail']);
  assert.equal(f.stats.opened,f.stats.closed);
});
test('missing or truncated indexed history returns no partial page and releases resources',t=>{
  for(const mode of ['gap','tail','truncate']){
    const f=fixture(t,['one','two','three']);
    if(mode==='gap')f.db.exec('DELETE FROM line WHERE seq=2');
    if(mode==='tail')f.db.exec('DELETE FROM line WHERE seq=3');
    if(mode==='truncate')fs.truncateSync(path.join(f.home,'events.jsonl'),5);
    const p=f.page(0,3);assert.notEqual(p.error,'');assert.deepEqual(f.array(p.lines),[]);
    assert.equal(f.stats.opened,f.stats.closed);assert.equal(f.stats.prepared,f.stats.finalized);assert.equal(f.stats.connections,f.stats.released);
  }
});
