'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {PREFIX,LIMIT,MAX_BYTES,sanitize}=require('../connection-record.cjs'),{createConnectionLog}=require('../connection-log.cjs');
test('diagnostic schema never retains arbitrary text, URLs, payloads or unknown reasons',()=>{
 const secret='https://secret.invalid/?token=PRIVATE message prompt';
 const r=sanitize({event:'closed',at:10,connection:2,closeCode:1006,closeReason:secret,page:secret,bundle:secret,generation:secret,
  cause:secret,errorKind:secret,payload:secret,prompt:secret,headers:{Authorization:secret}});
 assert.equal(r.closeCode,1006);assert.equal(r.closeReason,'redacted');assert(!JSON.stringify(r).includes('PRIVATE'));
 assert.equal(sanitize({event:secret}),null);
});
test('local desktop file is private, rate limited, count bounded and survives a new writer',()=>{
 const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-connection-log-'));let now=100000;
 try{
  const log=createConnectionLog(home,{now:()=>now});
  assert.equal(log.capture('ordinary console containing PRIVATE'),false);
  for(let i=0;i<100;i++){now+=1000;assert(log.capture(PREFIX+JSON.stringify({event:'closed',connection:i,closeCode:1006,cause:'transport',payload:'PRIVATE'})));}
  const rows=JSON.parse(fs.readFileSync(log.file));assert.equal(rows.length,LIMIT);assert(fs.statSync(log.file).size<=MAX_BYTES);
  assert.equal(fs.statSync(log.file).mode&0o777,0o600);assert(!fs.readFileSync(log.file,'utf8').includes('PRIVATE'));
  const next=createConnectionLog(home,{now:()=>now});assert(next.record({event:'open'}));assert.equal(JSON.parse(fs.readFileSync(log.file)).length,LIMIT);
  let accepted=0;for(let i=0;i<30;i++)accepted+=next.record({event:'error'})?1:0;assert(accepted<=12);
 }finally{fs.rmSync(home,{recursive:true,force:true});}
});
test('renderer preserves initiator through close, tracks frame and heartbeat age, bounds local ring',async()=>{
 const {ConnectionLog}=await import('../../src/web/connection-log.js');let now=100000;const store=new Map(),entries=[];
 const journal=new ConnectionLog({storage:{getItem:k=>store.get(k),setItem:(k,v)=>store.set(k,v)},bundle:'chunk-fixture.js',now:()=>now,
  visibility:()=> 'hidden',emit:x=>entries.push(x)});
 const c=journal.connection();c.note('created');now+=100;c.frame(71);c.heartbeat();c.greeting({generation:'12',epoch:'1234'});
 now+=200;c.initiate('heartbeat-expired');c.closed({code:1000,wasClean:true,reason:'contains token PRIVATE'});
 const r=entries.at(-1);assert.equal(r.cause,'heartbeat-expired');assert.equal(r.frameAgeMs,200);assert.equal(r.heartbeatAgeMs,200);
 assert.equal(r.generation,'12');assert.equal(r.visibility,'hidden');assert.equal(r.closeReason,'redacted');assert(!JSON.stringify(r).includes('PRIVATE'));
 for(let i=0;i<100;i++){now+=1000;journal.record('open');}assert.equal(journal.rows.length,LIMIT);assert([...store.values()].every(x=>x.length<=MAX_BYTES));
});
