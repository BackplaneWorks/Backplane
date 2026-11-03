// The reviewed diagnostics schema has aggregates, not action history.
// Project only closed metadata. Never infer UI intent or a reconnect from
// a latency/span/join, and never copy unknown fields or text/hash payloads.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
const events=new Set(['latency','sample','error','connection']);
const errors=new Set(['store_write','mail_append','disk_full','io','timeout','protocol','agent_exit','log_error','log_warn']);
const connections=new Set(['join','mobile_join','leave','drop','connect','disconnect','reconnect','failed']);
const gauges=new Set(['hub.clients','hub.agents','hub.busy']);
const uint=n=>Number.isSafeInteger(n)&&n>=0&&n<=0xffffffff;
export function projectDiagnostics(value){
  if(!value||value.version!==1||!Array.isArray(value.events))throw Error('unknown diagnostics schema');
  const out=[];
  for(const r of value.events.slice(-256)){
    if(!r||r.v!==1||!events.has(r.event)||!uint(r.at)||!uint(r.count)||r.count===0)continue;
    if(r.event==='error'&&!errors.has(r.code))continue;
    if(r.event==='connection'&&!(typeof r.name==='string'&&r.name.startsWith('connection.')&&connections.has(r.name.slice(11))))continue;
    if(r.event==='sample'&&!(gauges.has(r.name)&&uint(r.value)))continue;
    // Latency names come from a real closed allowlist in the reviewed source.
    // Read that list, never accept a name merely because it starts 'hub.'.
    if(r.event==='latency'&&!(operationNames().has(r.name)&&Number.isSafeInteger(r.us)&&r.us>=0&&Number.isFinite(r.maxMs)&&r.maxMs>=0))continue;
    out.push({kind:r.event,at:r.at,count:r.count});
  }
  return {version:1,events:out,replayableUiActions:0,
    limitation:'aggregate diagnostics do not contain selection, drafts, source entries or action order; cannot reconstruct a UI regression'};
}
let operations;
function operationNames(){
  if(!operations){const text=readFileSync(new URL('../../src/server/effects/diagnostics.js',import.meta.url),'utf8');
    const m=/const diagnostics_operations = new Set\(\[([\s\S]*?)\]\);/.exec(text);
    if(!m)throw Error('reviewed diagnostics allowlist shape changed');
    operations=new Set([...m[1].matchAll(/"([a-z0-9_.-]+)"/g)].map(x=>x[1]));}
  return operations;
}
if(process.argv[1]&&resolve(process.argv[1])===resolve(new URL(import.meta.url).pathname)){
  function readBounded(file){const bytes=readFileSync(file);if(bytes.length>1048576)throw Error('diagnostics export exceeds bound');return JSON.parse(bytes.toString('utf8'));}
  const input=process.argv[2]?readBounded(process.argv[2]):{version:1,status:{private:'MUST_NOT_COPY'},events:[
    {v:1,at:1,event:'connection',name:'connection.join',count:1,text:'PRIVATE'},
    {v:1,at:2,event:'latency',name:'hub.rpc',us:1000,maxMs:1,count:2,hash:'PRIVATE'},
    {v:1,at:3,event:'exit',count:1,text:'PRIVATE'},
    {v:1,at:3,event:'error',code:'PRIVATE',count:1}]};
  const result=projectDiagnostics(input);
  assert(!JSON.stringify(result).includes('PRIVATE'));
  console.log(JSON.stringify(result));
}
