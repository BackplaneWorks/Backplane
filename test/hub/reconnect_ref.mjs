const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun reconnect_ref.mjs BIN PORT AGENTS : the JS-hub "dropped link" experiment, with the missing items named.
// reference client sees every item; `first` joins, holds N items, drops; a new socket resumes with since=N&origin=.
// Capped (3.5 GiB, no swap), stand-in agents only.
import {mkdirSync,rmSync,writeFileSync,readFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {decode} from '../wire.mjs';
const [bin,port,agentsArg]=process.argv.slice(2);const agents=Number(agentsArg||5);
const R=BP_ROOT,root=R+`/build/gpui/hub-run/reconnect-${port}`;rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
const h=await startHub({bin,home:root+'/home',port:Number(port),standins:root+'/bin',cap:'3500M',extraEnv:{FAKE_SECS:'60',FAKE_PER:'2',FAKE_CYCLE:'20'}});
const refLog=[];const ref=await client(h,{onMsg:(o)=>{if(o.t==='log'||o.t==='changes')refLog.push(...(o.items||[]))}});const c=await client(h,{});
c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
for(let i=0;i<agents;i++)c.send('thread.create',{project:pc.id,title:'t'+i,env:'local'});
const ths=await c.until(20000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=agents?t:undefined});
for(const t of ths)c.send('turn.start',{thread:t.id,text:'stream'});
await sleep(4000);
const frames=[],firstLog=[];const first=await client(h,{onMsg:(o,now,n)=>{frames.push([o.t,(o.items||[]).length,o.since]);if(o.t==='log'||o.t==='changes')firstLog.push(...(o.items||[]))}});first.frames=frames;await sleep(2500);
const origin=first.origin??'',held=firstLog.length;const firstLogSince=first.logSince;
first.ws.close();await sleep(3000);
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?since=${held}&origin=${encodeURIComponent(origin)}&token=${h.token}`);ws.binaryType='arraybuffer';
const got=[];let logSince=null;
ws.onmessage=e=>{const o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data);if(o.t==='log')logSince=o.since;for(const it of o.items??[])got.push(it)};
await new Promise(r=>ws.onopen=r);await sleep(2500);
const key=x=>JSON.stringify(x);const joined=[...firstLog.map(key),...got.map(key)];
const refAll=refLog.map(key),have=new Set(joined);
const missing=refAll.map((k,i)=>[i,k]).filter(([i,k])=>!have.has(k));
const lines=readFileSync(root+'/home/events.jsonl','utf8').trimEnd().split('\n');
const kinds={};for(const [i,k] of missing){const o=JSON.parse(k);const t=o.$+(o.key?':'+o.key.replace(/[0-9a-z-]+$/,'<id>'):'');kinds[t]=(kinds[t]||0)+1}
// which of the reference items are NOT lines of events.jsonl at all?
const lineSet=new Set(lines.map(l=>{try{return JSON.stringify(JSON.parse(l))}catch{return l}}));
const notInLog=refAll.filter(k=>!lineSet.has(k));const nk={};for(const k of notInLog){const o=JSON.parse(k);const t=o.$+(o.key?':'+o.key.replace(/[0-9a-z-]+$/,'<id>'):'');nk[t]=(nk[t]||0)+1}
const kind=k=>{const o=JSON.parse(k);return o.$+(o.key?':'+o.key.replace(/[0-9a-z-]+$/,'<id>'):'')+(o.value===''?'(blank)':'')};
const around=[];for(let i=Math.max(0,held-12);i<Math.min(refAll.length,held+12);i++)around.push([i,kind(refAll[i]),joined[i]===undefined?null:kind(joined[i]),joined[i]===refAll[i]]);
const firstIdx=refAll.findIndex((k,i)=>joined[i]!==k);
const seqDump={ref:refAll.map(kind),first:firstLog.map(x=>kind(JSON.stringify(x))),got:got.map(x=>kind(JSON.stringify(x))),file:lines.map(l=>kind(JSON.stringify(JSON.parse(l)))),refKeys:refAll.map(k=>{const o=JSON.parse(k);return (o.thread||o.id||o.key||'')+'|'+(o.msg||o.act||'')}),fileKeys:lines.map(l=>{const o=JSON.parse(l);return (o.thread||o.id||o.key||'')+'|'+(o.msg||o.act||'')}),firstKeys:firstLog.map(o=>(o.thread||o.id||o.key||'')+'|'+(o.msg||o.act||'')),firstFrames:first.frames};
writeFileSync(R+'/build/gpui/results/hub/reconnect-seqs.json',JSON.stringify(seqDump));
const exact=JSON.stringify(joined.slice(0,refAll.length))===JSON.stringify(refAll.slice(0,joined.length));
const out={joinedEqualsReferencePrefix:exact,firstDiffIndex:firstIdx,around,agents,held,firstLogSince,liveItemsInFirst:first.seen.length-firstLog.length,resumeLogSince:logSince,refItems:refAll.length,resumeItems:got.length,eventsJsonlLines:lines.length,missing:missing.length,missingKinds:kinds,missingPositions:missing.slice(0,8).map(m=>m[0]),refItemsNotInLog:notInLog.length,notInLogKinds:nk};
console.log(JSON.stringify(out,null,1));writeFileSync(R+'/build/gpui/results/hub/reconnect-ref.json',JSON.stringify(out,null,1));
ws.close();stopHub(h);await sleep(800);process.exit(0);
