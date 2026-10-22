'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function fixture(hash=''){
 const {pairBrowser}=await import('../../src/web/auth.js');const requests=[],removed=[],history=[];
 const context={location:{hash,pathname:'/',search:''},history:{replaceState:(...args)=>history.push(args)},storage:{getItem:()=>null,removeItem:k=>removed.push(k)},fetch:async(...args)=>{requests.push(args);return{ok:true};}};
 return{context,requests,removed,history,run:()=>pairBrowser(context)};
}
test('an already paired page needs no JavaScript credential exchange',async()=>{const f=await fixture();assert.equal(await f.run(),true);assert.equal(f.requests.length,0);});
test('fragment pairing uses only the POST header and removes the fragment',async()=>{const token='a'.repeat(32),f=await fixture('#token='+token);assert.equal(await f.run(),true);assert.equal(f.requests[0][0],'/auth/pair');assert.equal(f.requests[0][1].headers.Authorization,'Bearer '+token);assert.equal(f.history[0][2],'/');});
test('malformed or refused pairing does not fall back to an older credential',async()=>{const f=await fixture('#token=invalid');assert.equal(await f.run(),false);assert.equal(f.requests.length,0);const g=await fixture('#token='+'a'.repeat(32));g.context.fetch=async()=>({ok:false});assert.equal(await g.run(),false);});
test('a stalled pairing exchange is bounded',async()=>{const f=await fixture('#token='+'a'.repeat(32));f.context.fetch=()=>new Promise(()=>{});f.context.timeoutMs=5;f.context.unreachableMs=30;assert.equal(await f.run(),false);});
