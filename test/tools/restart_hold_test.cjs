const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/server/effects/restart-hold.js'),'utf8');
function latch(){return vm.runInNewContext(source+'\n({offer:restarthold_offer,take:restarthold_take})',{Buffer,io_eff(){},CID:x=>x,RestartHold:{offer:1,take:2}})}
const a=JSON.stringify({path:'/tmp/staged-hub',args:['--home','/tmp/isolated-home','--port','3799']});
test('deferred exec is retained once, same request idempotent, conflicting request cannot replace it',()=>{
 const g=latch();assert.equal(g.take(),'');assert.equal(g.offer(a),true);assert.equal(g.offer(a),true);
 assert.equal(g.offer(JSON.stringify({path:'/other',args:[]})),false);assert.equal(g.take(),a);assert.equal(g.take(),'');
});
test('new running turn can re-hold the same consumed request without executing or losing it',()=>{
 const g=latch();g.offer(a);const candidate=g.take();assert.equal(g.offer(candidate),true);assert.equal(g.take(),a);
});
test('malformed, oversized and NUL-bearing handoffs fail closed without changing prior slot',()=>{
 const g=latch();g.offer(a);
 for(const x of ['not json','x'.repeat(65537),JSON.stringify({path:'',args:[]}),JSON.stringify({path:'/x\0',args:[]}),JSON.stringify({path:'/x',args:[1]}),JSON.stringify({path:'/x',args:['a\0']}),JSON.stringify({path:'/x',args:Array(129).fill('a')})])assert.equal(g.offer(x),false);
 assert.equal(g.take(),a);
});
