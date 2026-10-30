const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/server/effects/push-gate.js'),'utf8');
const bend=fs.readFileSync(path.resolve(__dirname,'../../src/server/push.bend'),'utf8');
function engine(){return vm.runInNewContext(source+'\n({claim:pushgate_claim,finish:pushgate_finish,slots:bp_push_circuits,leases:bp_push_leases})',{require,io_eff(){},CID:x=>x,PushGate:{claim:1,finish:2}})}
const claim=(g,n=0,kind='activity',registration='1',topic='test.app')=>g.claim('https://example.invalid/3/device/private-token',topic,kind,registration,n);
test('permanent TopicDisallowed sends once for a registration, even under 30k repeated demands',()=>{
 const g=engine(),id=claim(g);assert.match(id,/^\d+$/);g.finish(id,'400','TopicDisallowed',0);
 for(let i=0;i<30000;i++)assert.equal(claim(g,i/16),'');
 assert.equal(g.slots.size,1);assert.equal(g.leases.size,0);
 assert.match(claim(g,2000,'activity','2'),/^\d+$/);
 assert.match(claim(g,2000,'activity','1','different.app'),/^\d+$/);
});
test('inflight activities coalesce and successful activity is bounded to one per second',()=>{
 const g=engine(),id=claim(g);assert.equal(claim(g),'');g.finish(id,'200','',0);
 assert.equal(claim(g,0.9),'');assert.match(claim(g,1),/^\d+$/);
});
test('retry failures back off exponentially and cap failed jobs in a 30 minute window',()=>{
 const g=engine();let now=0;
 for(let i=0;i<6;i++){
  const id=claim(g,now);assert.match(id,/^\d+$/);
  g.finish(id,['000','429','500','503','403','500'][i],'transient',now);
  const slot=[...g.slots.values()][0];assert(slot.until>now);assert.equal(claim(g,slot.until-.01),'');
  if(i===5)assert(slot.until>=1800);now=slot.until;
 }
 assert.match(claim(g,now),/^\d+$/);
});
test('alert holds are explicit once, bounded concurrency and finish releases every lease',()=>{
 const g=engine(),ids=Array.from({length:4},()=>claim(g,0,'alert'));
 assert(ids.every(x=>/^\d+$/.test(x)));assert.equal(claim(g,0,'alert'),'-busy');assert.equal(claim(g,0,'alert'),'');
 for(const id of ids)g.finish(id,'503','temporary',0);
 assert.equal(g.leases.size,0);assert.equal(claim(g,1,'alert'),'-backoff');assert.equal(claim(g,1,'alert'),'');
});
test('a late old success cannot erase a newer failed alert cooldown',()=>{
 const g=engine(),old=claim(g,0,'alert'),recent=claim(g,0,'alert');
 g.finish(recent,'503','temporary',0);g.finish(old,'200','',1);
 assert.equal(claim(g,2,'alert'),'-backoff');assert.equal(g.leases.size,0);
});
test('slot pressure fails closed, never evicts a permanent refusal, and retains no token/topic/content',()=>{
 const g=engine();for(let i=0;i<1024;i++){const id=g.claim('https://example.invalid/'+i,'private-topic','activity','1',0);g.finish(id,'400','TopicDisallowed',0)}
 assert.equal(g.slots.size,1024);assert.equal(claim(g,99999),'-capacity');assert.equal(claim(g,99999),'');
 assert.equal(g.claim('https://example.invalid/0','private-topic','activity','1',99999),'');
 const text=JSON.stringify([...g.slots]);assert(!text.includes('private-topic'));assert(!text.includes('example.invalid'));
 assert.equal(g.slots.size,1024);assert.equal(g.leases.size,0);
});
test('expired healthy slots can be released; activity and alert circuits are isolated',()=>{
 const g=engine(),id=claim(g);g.finish(id,'200','',0);
 const alert=claim(g,0,'alert');assert.match(alert,/^\d+$/);g.finish(alert,'200','',0);
 const next=claim(g,2000,'activity','new-registration');assert.match(next,/^\d+$/);assert.equal(g.slots.size,1);
});
test('every curl transport uses bounded transient retries, never blanket permanent-error retries',()=>{
 assert.match(bend,/"--retry", "2", "--retry-max-time", "20"/);
 assert.equal((bend.match(/List\.append\(&2, String, (?:"--http2" <> )?Curl\.retry\(\)/g)||[]).length,2);
 assert(!bend.includes('"--retry-all-errors"'));
 assert.match(bend,/String\.eq\(reason, "TopicDisallowed"\)/);
 assert.match(bend,/PG\.PushGate\.finish\(lease, code, reason,/);
});
