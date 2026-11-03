'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const {createBrowserRecovery}=require('../browser-recovery.cjs');
const turn=()=>new Promise(r=>setImmediate(r));
function fixture(attempt){const queue=[];let live=true;const recovery=createBrowserRecovery({attempt,active:()=>live,schedule:(fn,ms)=>{const t={fn,ms};queue.push(t);return t;},cancel:t=>{t.cancelled=true;}});return{recovery,queue,stop:()=>{live=false;}};}
test('connection recovery backs off, coalesces, resets after success and retires on close',async()=>{
 let calls=0;const h=fixture(async()=>({phase:++calls===8?'ready':'error'}));
 h.recovery.changed({phase:'error'});h.recovery.changed({phase:'error'});assert.equal(h.queue.length,1);
 for(const ms of [500,1000,2000,4000,8000,16000,30000,30000]){const t=h.queue.at(-1);assert.equal(t.ms,ms);t.fn();await turn();}
 assert.equal(calls,8);h.recovery.changed({phase:'error'});assert.equal(h.queue.at(-1).ms,500);
 h.recovery.dispose();assert(h.queue.at(-1).cancelled);h.queue.at(-1).fn();await turn();assert.equal(calls,8);
});
test('resume/focus recovery shares one pending attempt and never replays an operation',async()=>{
 let finish,calls=0;const h=fixture(()=>{calls++;return new Promise(r=>finish=r);});
 const a=h.recovery.retry(),b=h.recovery.retry();assert.equal(a,b);await turn();assert.equal(calls,1);
 finish({phase:'ready'});await a;assert.equal(h.queue.length,0);h.stop();await h.recovery.retry();assert.equal(calls,1);
});
