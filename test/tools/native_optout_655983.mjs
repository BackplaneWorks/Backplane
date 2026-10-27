// Synthetic captured fixtures; actual emitted bridges and unchanged source validators.
// This checks compatibility only. No hub, native UI, or performance claim.
import fs from 'node:fs';
import vm from 'node:vm';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { cbor, plain } from '../../src/web/wire.js';
const fixtureRoot = '/home/h/Documents/i2cjak/backplane-bend/.backplane/worktrees/chat-loading-v2-679512/build/chat-loading-v2';
const manifestPath = path.join(fixtureRoot, 'NATIVE-OPTOUT-8.json');
const handoffPath = path.join(fixtureRoot, 'HANDOFF.json');
const bundlePaths = process.argv.slice(2);
assert.equal(bundlePaths.length, 2, 'expected frozen and candidate bundles');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const handoffBytes=fs.readFileSync(handoffPath), manifestBytes=fs.readFileSync(manifestPath);
assert.equal(sha(handoffBytes),'9c12794e277563a944879ecbf6264962eaef22cb3215ef3268230d5a02e8c940','handoff hash');
assert.equal(sha(manifestBytes),'912816f40286a70da12e9333757cd4798d19fcf868a6e8138ff91ebbdb3a0aff','manifest hash');
const handoff=JSON.parse(handoffBytes), manifest=JSON.parse(manifestBytes);
assert.equal(manifest.candidate,8); assert.equal(manifest.sourceHead,handoff.source.head);
assert.equal(manifest.service.sha256,handoff.artifactsVerifiedNow['backplane-serve']);
assert.equal(sha(fs.readFileSync(manifest.cbor.path)),manifest.cbor.sha256,'CBOR source hash');
assert.equal(manifest.cbor.sha256,'81463f1d4ad8c0d99cb71f35caa50742e4009f57e196f70075a02a08d139a56c');
const inputs = {};
for (const name of ['closed','expanded','explicit-body']) {
 const f=manifest.fixtures[name], p=path.join(fixtureRoot,'native-optout-8',name+'.json');
 assert.equal(f.path,p); const data=fs.readFileSync(p);
 assert.equal(sha(data),f.sha256,name+' manifest fixture hash');
 assert.equal(handoff.acceptance.native.frames.find(x=>x.path===p)?.sha256,f.sha256,name+' handoff fixture hash');
 inputs[name]=JSON.parse(data);
}
const expected=['d4d3eaa649c060694a53eecbfc01c7f3cac908c778c4495ba4364f12bea53e13','c7aab6184cab1d8b5484fd22325797ff5d7325086292fea80f68961f34bebf4a'];
const bundles=bundlePaths.map((p,i)=>{const data=fs.readFileSync(p);assert.equal(sha(data),expected[i],'bundle hash');return data.toString('utf8');});
const temp=fs.mkdtempSync(path.join(os.tmpdir(),'native-optout-check-'));
for(const n of ['src/mobile/thin-native-host-307265.js','src/web/thin.js','src/web/thin-results.js','src/web/startup.js','src/web/mail-drafts.js','src/web/terminal-input.js','src/web/model-menu-input.js','src/web/sidebar-region.js']){const q=path.join(temp,n);fs.mkdirSync(path.dirname(q),{recursive:true});fs.copyFileSync(n,q);}
fs.writeFileSync(path.join(temp,'package.json'),'{"type":"module"}');
const {validateNativeResult307265}=await import(pathToFileURL(path.join(temp,'src/mobile/thin-native-host-307265.js')));
const {validateThinResult}=await import(pathToFileURL(path.join(temp,'src/web/thin-results.js')));
for(const name of ['closed','expanded']) assert.equal(validateNativeResult307265(inputs[name].reply.result,inputs[name].request.p),true,name+' native source validator');
assert.equal(validateThinResult(inputs['explicit-body'].reply.result,inputs['explicit-body'].request.p),true,'source message validator');
const corrupt=structuredClone(inputs.expanded.reply.result);corrupt.window.partial[0].complete=true;
assert.equal(validateNativeResult307265(corrupt,inputs.expanded.request.p),false,'invalid partial rejected by actual source validator');
function rejectNewFields(value){if(!value||typeof value!=='object')return;for(const [k,v] of Object.entries(value)){assert(!['timelineEntries','pageLookahead','lookahead'].includes(k),'no mandatory candidate fields');rejectNewFields(v);}}
for(const value of Object.values(inputs))rejectNewFields(value);
const head=(m,n)=>n<24?Buffer.from([(m<<5)|n]):n<256?Buffer.from([(m<<5)|24,n]):n<65536?Buffer.from([(m<<5)|25,n>>8,n&255]):Buffer.from([(m<<5)|26,n>>>24,(n>>>16)&255,(n>>>8)&255,n&255]);
const enc=v=>v===null?Buffer.from([246]):typeof v==='boolean'?Buffer.from([v?245:244]):typeof v==='number'?(v>4294967295?Buffer.concat([Buffer.from([199]),enc(String(v))]):head(0,v)):typeof v==='string'?Buffer.concat([head(3,Buffer.byteLength(v)),Buffer.from(v)]):Array.isArray(v)?Buffer.concat([head(4,v.length),...v.map(enc)]):Buffer.concat([head(5,Object.keys(v).length),...Object.entries(v).flatMap(([k,x])=>[enc(k),enc(x)])]);
let checks=0;const ok=(condition,label)=>{assert(condition,label);checks++;};
const hub=inputs.closed.request.p.hub, target=inputs.closed.request.p.target, greeting=inputs.closed.greeting;
function create(source){const ctx=vm.createContext({console:{log(){},warn(){},error(){}},Date});vm.runInContext(source,ctx,{timeout:10000});const call=(name,...args)=>{ctx.args=args;ctx.name=name;return vm.runInContext('Backplane[name](...args)',ctx,{timeout:10000});};const out=(name,...args)=>JSON.parse(call(name,...args));const dict=JSON.parse(call('headerKeys'));const requests=o=>(o.cmds||[]).filter(x=>x.type==='send').map(x=>plain(cbor(new Uint8Array(Buffer.from(x.data,'base64')),dict.keys,dict.words)));const recv=(v,epoch='1')=>out('recv',hub,enc(v).toString('base64'),epoch,false,false);return {call,out,requests,recv};}
// Rebase ONLY outer reply id and result.requestRevision to the actual generated
// request. All captured body/session/window/command data, viewId, target, hub,
// generation and epoch remain intact. Assert every other authority field agrees.
function rebase(fixture,request){const r=structuredClone(fixture.reply);const actual=request.p;assert.equal(request.m,fixture.request.m);assert.deepEqual(actual.target,r.result.target);assert.equal(actual.hub,r.result.hub);assert.equal(actual.generation,r.result.generation);assert.equal(actual.epoch,r.result.epoch);if(request.m!=='view.open')assert.equal(actual.viewId,r.result.viewId);r.id=request.id;r.result.requestRevision=actual.requestRevision;return r;}
function noAuto(c,o){const calls=c.requests(o);if(calls.length) console.log('emitted method types',calls.map(r=>r.m));ok(!calls.some(r=>r.m==='view.message'),'no automatic body read');}
for(const [i,source] of bundles.entries()){
 const before=checks;
 const c=create(source);c.call('start','synthetic-optout','{}');c.call('hubs',[hub]);c.call('local','mailroom-renderer','ios-v1');c.call('connection',hub,'1','view-v1');c.out('act','select',hub+'|'+target.id);
 let out=c.recv(greeting),req=c.requests(out)[0];ok(req?.m==='view.open','generated open');
 const old=rebase(inputs.closed,req);const stale=c.recv(old,'stale');ok(stale.sync.length===0&&c.requests(stale).length===0,'stale epoch ignored');
 out=c.recv(old);ok(out.sync.some(s=>s.phase==='presented'),'closed reply accepted');noAuto(c,out);
 let rows=out.screen?.thread?.entries||[];ok(rows.filter(x=>x.kind==='fold').length===1,'closed fold header visible');ok(!rows.some(x=>/^qa-tool-[0-5]$/.test(x.id)),'six hidden bodies absent');
 const value=inputs.expanded.request.p.value;out=c.out('act','fold',value);req=c.requests(out)[0];ok(req?.m==='view.action'&&req.p.action==='fold'&&req.p.value===value,'explicit fold action');
 out=c.recv(rebase(inputs.expanded,req));ok(out.sync.some(s=>s.phase==='presented'),'expanded reply accepted');noAuto(c,out);
 rows=out.screen?.thread?.entries||[];const tools=rows.filter(x=>/^qa-tool-[0-5]$/.test(x.id));ok(tools.length===6&&tools.every(x=>x.partial?.hasMore===true&&x.partial?.complete===false),'six expanded prefixes retain partial authority');
 const part=tools.find(x=>x.id===inputs['explicit-body'].request.p.entryId);assert(part);
 const capturedPrefix=inputs.expanded.reply.result.shown.thread.entries.find(x=>x.id===part.id);ok(part.text===capturedPrefix.text,'expanded prefix exactly matches captured projection');
 out=c.out('act','thin-message-more',JSON.stringify({entryId:part.id,cursor:part.partial.cursor}));req=c.requests(out)[0];ok(req?.m==='view.message'&&req.p.entryId===part.id&&req.p.cursor==='','explicit body read only on request');
 out=c.recv(rebase(inputs['explicit-body'],req));ok(!out.sync.some(s=>s.phase==='presented'||s.phase==='presenting'),'body chunk carries no readiness authority');noAuto(c,out);
 const current=out.screen?.thread?.entries?.find(x=>x.id===part.id);ok(current?.partial?.complete===false&&current.partial.hasMore===true,'captured chunk remains incomplete');ok(current.text===inputs['explicit-body'].reply.result.message.text&&current.text.length>part.text.length,'explicit partial body assembled from requested start');
 console.log(JSON.stringify({candidate:8,bundleSha256:expected[i],checks:checks-before,scope:'synthetic captured fixtures; emitted bridge only'}));
}
