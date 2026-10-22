'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const capability={version:1,views:{version:1,websocketQuery:'view=1',renderers:['web','mobile'],compatibility:'explicit-only'}};
function response(text,status=200){return{status,headers:new Headers({'content-type':'application/json'}),body:new ReadableStream({start(c){c.enqueue(new TextEncoder().encode(text));c.close();}})};}
async function run(fetch,extra={}){const {discoverViews}=await import('../../src/web/thin-discovery.js'),{viewCapability}=await import('../../src/web/thin.js');return discoverViews({fetch,validate:viewCapability,...extra});}
test('capability discovery uses same-origin credentials without URL tokens or body credentials',async()=>{
 let request;assert.deepEqual(await run(async(url,options)=>{request={url,options};return response(JSON.stringify(capability));}),{supported:true});assert.match(request.url,/^\/client\/capabilities\?bpProbe=[0-9a-f]{32}$/);assert.equal(request.options.credentials,'same-origin');assert.equal(request.options.cache,'no-store');assert.equal(request.options.headers,undefined);
});
test('missing/denied/invalid capability stays unsupported rather than choosing a legacy protocol',async()=>{
 for(const r of [response('',404),response('',401),response('',403),response('{}'),response('{')])assert.deepEqual(await run(async()=>r),{supported:false});
});
test('oversized headers or streaming bodies are rejected before validation',async()=>{
 const {discoverViews}=await import('../../src/web/thin-discovery.js');let validations=0;
 const large=response(' '.repeat(16385));assert.deepEqual(await discoverViews({fetch:async()=>large,validate:()=>{validations++;return true;}}),{supported:false});assert.equal(validations,0);
 const declared=response('{}');declared.headers.set('content-length','20000');assert.deepEqual(await run(async()=>declared),{supported:false});
});
test('malformed UTF8 is not accepted as a different capability',async()=>{
 const r={status:200,headers:new Headers({'content-type':'application/json'}),body:new ReadableStream({start(c){c.enqueue(new Uint8Array([0xff]));c.close();}})};assert.deepEqual(await run(async()=>r),{supported:false});
});
// A stall is a hub that is down or restarting: unsupported, and offline, so
// the page tries again later (host.js thinReconnect) instead of parking.
test('stalled fetch is bounded and a late success cannot turn its result into supported',async()=>{
 let release;const result=await run(()=>new Promise(r=>release=r),{timeoutMs:5});assert.deepEqual(result,{supported:false,offline:true});release(response(JSON.stringify(capability)));await new Promise(r=>setImmediate(r));assert.deepEqual(result,{supported:false,offline:true});
});
test('stalled body has the same bounded failure as a stalled connection',async()=>{
 let controller;const r={status:200,headers:new Headers({'content-type':'application/json'}),body:new ReadableStream({start(c){controller=c;}})};const result=await run(async()=>r,{timeoutMs:5});assert.deepEqual(result,{supported:false,offline:true});controller.enqueue(new TextEncoder().encode(JSON.stringify(capability)));controller.close();await new Promise(r=>setImmediate(r));assert.deepEqual(result,{supported:false,offline:true});
});

test('a refused connection or a starting hub (502-504) is offline, not unsupported',async()=>{
 assert.deepEqual(await run(async()=>{throw new TypeError('fetch failed');}),{supported:false,offline:true});
 for(const code of [502,503,504])assert.deepEqual(await run(async()=>response('',code)),{supported:false,offline:true});
});
