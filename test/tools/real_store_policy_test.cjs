const {test}=require('node:test'),assert=require('node:assert/strict'),{verdict}=require('./real_store_policy.cjs');
const samples=(f=()=>700000)=>Array.from({length:361},(_,i)=>({elapsedMs:i*5000,VmRSS:f(i),VmHWM:900000,VmSwap:0}));
test('inclusive 15s startup and flat 30m real-store samples pass',()=>{assert(verdict(samples(),15000,1800000).ok);assert(!verdict(samples(),15001,1800000).ok)});
test('warmup does not conceal persistent growth, swap, missing samples or early shutdown',()=>{
 assert(!verdict(samples(i=>700000+i*1000),1000,1800000).ok);
 const swap=samples();swap[230].VmSwap=1;assert(!verdict(swap,1000,1800000).ok);
 const missing=samples();missing.splice(100,5);assert(!verdict(missing,1000,1800000).ok);
 assert(!verdict(samples().slice(0,100),1000,500000).ok);
 assert(!verdict(samples(()=>1100000),1000,1800000).ok);
 assert(!verdict(samples(),undefined,1800000).ok);
});
