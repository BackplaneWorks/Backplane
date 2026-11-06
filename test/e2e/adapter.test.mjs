// bun adapter.test.mjs  (Boat; no display needed: the GPUI binary is a fake speaking the metrics contract)
import assert from 'node:assert/strict';import {mkdirSync,chmodSync} from 'node:fs';import {run} from './adapters/gpui.mjs';
chmodSync(new URL('./lib/fake-gpui.mjs',import.meta.url).pathname,0o755);
const fake=new URL('./lib/fake-gpui.mjs',import.meta.url).pathname;const out='/tmp/e2e-adapter-test';mkdirSync(out+'/h/home',{recursive:true});
const ctx=(env={})=>{Object.assign(process.env,{FAKE_SOFTWARE:'0',FAKE_SHIFT:'0',FAKE_NOPOPUP:'0',...env});return {gpuBin:fake,display:{display:':99',env:process.env},hub:{state:{url:'http://x',root:out+'/h'}},outDir:out,hz:60,record:false}};
const J=(id,steps,measure)=>({id,item:id,measure,steps,needs:[]});
// clean run: acks at 9.5 ms < 16.7 -> click pass; frames 60 fps -> pass; layout steady -> pass
let r=await run(J('t1',[{do:'select_thread',index:0},{do:'select_thread',index:1},{do:'scroll',target:'thread',screens:2}],['click','scroll','layout']),ctx());
assert.equal(r.metrics.click.pass,true,JSON.stringify(r.metrics.click));assert.ok(r.metrics.frames?.pass,'frames');assert.equal(r.metrics.layout.pass,true);assert.equal(r.status,'pass',JSON.stringify(r.notes));
// a layout jump with no input in the previous 500 ms is a failure
r=await run(J('t2',[{do:'select_thread',index:0},{do:'select_thread',index:1}],['click','layout']),ctx({FAKE_SHIFT:'1'}));
assert.equal(r.metrics.layout.pass,false);assert.equal(r.status,'fail');
// popups the client cannot open fail the journey and are named
r=await run(J('t3',[{do:'popup',trigger:'/'},{do:'popup',trigger:'@'}],['click']),ctx({FAKE_NOPOPUP:'1'}));
assert.equal(r.status,'fail');assert.deepEqual(r.metrics.failedSteps,['popup:@ (timeout)']);
// dead-control sweep: one control whose click changes nothing
r=await run(J('t4',[{do:'select_thread',index:0},{do:'click_every_control',scope:'screen'}],['click','dead']),ctx());
assert.equal(r.metrics.dead.deadCount,1);assert.equal(r.metrics.dead.dead[0].id,'dead-btn');assert.equal(r.status,'fail');
// steps the client does not have stay blocked
r=await run(J('t5',[{do:'open_viewer',kind:'board'}],['click']),ctx());assert.equal(r.status,'blocked');
// software rendering is noted
r=await run(J('t6',[{do:'select_thread',index:0}],['click']),ctx({FAKE_SOFTWARE:'1'}));assert.ok(r.notes.some(n=>n.includes('software-rendered')));
console.log('adapter.test.mjs: all assertions passed');
