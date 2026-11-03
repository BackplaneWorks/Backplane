const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
function fixture(){
  const effects={};let now=0,walks=0,memoryReads=0;
  const ctx=vm.createContext({Perf:new Proxy({},{get:(_,k)=>k}),CID:x=>x,io_eff:(k,f)=>effects[k]=f,
    performance:{now:()=>now},Bun:{},process:{memoryUsage:()=>{memoryReads++;return{rss:10000,heapUsed:10,heapTotal:20}},resourceUsage:()=>({}),env:{}},
    require:n=>{assert.equal(n,'bun:jsc');return{heapStats:()=>{walks++;return{heapSize:30,heapCapacity:40,objectTypeCounts:{Object:99999}}}}}});
  vm.runInContext(fs.readFileSync(path.join(__dirname,'../../src/server/effects/perf.js'),'utf8'),ctx);
  return{effects,ctx,get walks(){return walks},get memoryReads(){return memoryReads},time:t=>now=t};
}
test('discovery reads its gauge without a diagnostic snapshot, heap walk or new slot',()=>{
  const f=fixture();assert.equal(f.effects.gauge('hub.busy'),0);f.effects.set('hub.busy',17);
  for(let i=0;i<10000;i++)assert.equal(f.effects.gauge('hub.busy'),17);
  assert.equal(f.walks,0);assert.equal(f.memoryReads,0);assert.equal(f.ctx.BACKPLANE_PERF.tab.size,1);
  f.effects.set('hub.busy',0);assert.equal(f.effects.gauge('hub.busy'),0);
  assert.equal(f.effects.gauge('not-present'),0);assert.equal(f.ctx.BACKPLANE_PERF.tab.size,1);
});
test('repeated explicit diagnostic reads retain only a minute-sampled numeric heap pair',()=>{
  const f=fixture();assert.match(f.effects.take(),/proc.heap_used_bytes/);assert.equal(f.walks,1);
  for(let i=0;i<100;i++){f.time(i*500);f.effects.take();}assert.equal(f.walks,1);
  f.time(60000);f.effects.take();assert.equal(f.walks,2);
  assert.deepEqual(Object.keys(vm.runInContext('perf_heap_sample',f.ctx)).sort(),['at','total','used']);
});
