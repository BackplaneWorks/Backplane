// bun reconnect_prove.mjs BIN PORT AGENTS [OUTFILE] [hub args...]
// The "dropped link" proof, stand-in agents only (perf/lib.mjs verifies BACKPLANE_AGENTS from the hub's own environ).
// reference client = every log/changes item from join on. `first` joins while AGENTS threads stream, holds N log+changes items
// (live-frame items are NOT log lines and are not counted), drops. A new socket resumes with since=N&origin=<first's origin>.
// Verdicts: (1) strict: first[0..N) + resume vs reference, element by element, naming every difference by kind;
//           (2) normalised to the wire form Hub::wire_item makes (native/host/src/hub.rs: a SettingSet wire_blank names, push.book,
//               launch.input.*, fallback.input.*, ride.*, travels with its value blank, Device* events as kind and time; the log keeps them
//               whole): must be identical, same order, same length;
//           (3) the reference, normalised, equals events.jsonl line for line.
import {mkdirSync,rmSync,writeFileSync,readFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {decode} from '../wire.mjs';
const [bin,port,agentsArg,outFile,...hubArgs]=process.argv.slice(2);const agents=Number(agentsArg||20);
const R=process.env.BP_ROOT||new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
const root=`${R}/build/gpui/hub-run/prove-${port}`;rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
const h=await startHub({bin,home:root+'/home',port:Number(port),standins:root+'/bin',cap:'3500M',extraArgs:hubArgs,extraEnv:{TZ:'Etc/UTC',FAKE_SECS:'60',FAKE_PER:'2',FAKE_CYCLE:'20'}});
const refLog=[];const ref=await client(h,{onMsg:(o)=>{if(o.t==='log'||o.t==='changes')refLog.push(...(o.items||[]))}});const c=await client(h,{});
// threads come from project.add (each makes a project and its first thread): the one method both hubs have for it
for(let i=0;i<agents;i++){mkdirSync(`${root}/proj${i}`,{recursive:true});c.send('project.add',{path:`${root}/proj${i}`})}
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=agents?t:undefined});
for(const t of ths)c.send('turn.start',{thread:t.id,text:'stream'});
await sleep(4000);
const firstLog=[];let firstOrigin=null,firstLive=0;
const first=await client(h,{onMsg:(o)=>{if(o.t==='log'){firstOrigin=o.origin}if(o.t==='log'||o.t==='changes')firstLog.push(...(o.items||[]));if(o.t==='live')firstLive+=(o.items||[]).length}});
await sleep(2500);
const held=firstLog.length;first.ws.close();await sleep(3000);
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?since=${held}&origin=${encodeURIComponent(firstOrigin)}&token=${h.token}`);ws.binaryType='arraybuffer';
const got=[];let resumeSince=null,resumeOrigin=null;
ws.onmessage=e=>{const o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data);if(o.t==='log'){resumeSince=o.since;resumeOrigin=o.origin}if(o.t==='log'||o.t==='changes')for(const it of o.items??[])got.push(it)};
await new Promise(r=>ws.onopen=r);await sleep(2500);
// the faulty client: its `since` counts every item of every frame, the live frame's included (what the failing run did)
const wrongSince=held+firstLive;const ws2=new WebSocket(`ws://127.0.0.1:${port}/ws?since=${wrongSince}&origin=${encodeURIComponent(firstOrigin)}&token=${h.token}`);ws2.binaryType='arraybuffer';
const got2=[];ws2.onmessage=e=>{const o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data);if(o.t==='log'||o.t==='changes')for(const it of o.items??[])got2.push(it)};
await new Promise(r=>ws2.onopen=r);await sleep(1500);
// the reference may have lines the resume did not see yet: settle the three streams
const fileLen=()=>readFileSync(root+'/home/events.jsonl','utf8').trimEnd().split('\n').length;
{let last=-1,since=Date.now(),t0=Date.now();
 while(Date.now()-t0<120000){const n=fileLen();if(n!==last){last=n;since=Date.now()}
  if(Date.now()-since>3000&&refLog.length===n&&held+got.length===n&&held+got2.length+firstLive===n)break;
  if(Date.now()-since>3000&&refLog.length===n&&held+got.length===n)break;await sleep(250)}}
// the wire's form of a log line, as Hub::wire_item makes it (native/host/src/hub.rs): device events carry only their kind and time, and a setting
// wire_blank names (push.book, launch.input.*, fallback.input.*, ride.*) travels with its value blank
const wireBlank=k=>k==='push.book'||k.startsWith('launch.input.')||k.startsWith('fallback.input.')||k.startsWith('ride.');
const wireAt=a=>Number.isInteger(a)&&a>=0?a:0; // serde's as_u64().unwrap_or(0)
const norm=x=>{x=JSON.parse(JSON.stringify(x));if(['DeviceRegistered','DeviceDropped','DevicePrefs'].includes(x.$))x={$:x.$,at:wireAt(x.at)};else if(x.$==='SettingSet'&&typeof x.key==='string'&&wireBlank(x.key))x={$:'SettingSet',key:x.key,value:'',at:wireAt(x.at)};return JSON.stringify(x)};
const key=x=>JSON.stringify(x);
const joined=[...firstLog.slice(0,held),...got];
const kind=o=>o.$+(o.key?':'+o.key.replace(/[0-9a-z-]+$/,'<id>'):'');
const strictDiff=[];const n=Math.max(refLog.length,joined.length);
for(let i=0;i<n;i++){if(refLog[i]===undefined||joined[i]===undefined||key(refLog[i])!==key(joined[i]))strictDiff.push([i,refLog[i]?kind(refLog[i]):null,joined[i]?kind(joined[i]):null])}
const kinds={};for(const [,a,b] of strictDiff){const k=(a||'-')+' | '+(b||'-');kinds[k]=(kinds[k]||0)+1}
const normEq=refLog.length===joined.length&&refLog.every((x,i)=>norm(x)===norm(joined[i]));
const lines=readFileSync(root+'/home/events.jsonl','utf8').trimEnd().split('\n');
const fileEq=lines.length===refLog.length&&lines.every((l,i)=>norm(JSON.parse(l))===norm(refLog[i]));//((l,i)=>norm(JSON.parse(l))===norm(refLog[i]));
const j2=[...firstLog.slice(0,held),...got2];const have2=new Set(j2.map(norm));const absent2=refLog.map((x,i)=>[i,x]).filter(([i,x])=>!have2.has(norm(x)));
const missingStrict=refLog.filter(x=>!new Set(joined.map(key)).has(key(x)));
const out={bin,agents,held,firstLiveItems:firstLive,firstOrigin,resumeOrigin,resumeSince,refItems:refLog.length,joinedItems:joined.length,eventsJsonlLines:lines.length,
 strictDifferences:strictDiff.length,strictByKind:kinds,strictAbsent:missingStrict.length,strictAbsentKinds:[...new Set(missingStrict.map(kind))],
 normalisedIdentical:normEq,faultyClient:{sinceUsed:wrongSince,liveItemsCountedIn:firstLive,absentLines:absent2.length,absentPositions:absent2.map(a=>a[0]).slice(0,40),absentKinds:[...new Set(absent2.map(a=>kind(a[1])))]},referenceEqualsFile:fileEq,
 // items the resume delivered twice: repeats in the joined stream beyond the reference's own (blanked settings written in the same second are identical lines on the wire)
 duplicates:(()=>{const c=new Map();for(const x of refLog){const k=key(x);c.set(k,(c.get(k)||0)-1)}for(const x of joined){const k=key(x);c.set(k,(c.get(k)||0)+1)}let d=0;for(const v of c.values())if(v>0)d+=v;return d})(),
 // what differs, so a FAIL says why: the first lines where the file and the reference disagree, and the kinds that repeat (in the joined stream and in the reference)
 fileDiffs:Array.from({length:Math.max(lines.length,refLog.length)},(_,i)=>[i,lines[i],refLog[i]]).filter(([i,l,r])=>l===undefined||r===undefined||norm(JSON.parse(l))!==norm(r)).slice(0,5).map(([i,l,r])=>({at:i,file:l===undefined?null:norm(JSON.parse(l)).slice(0,300),wire:r===undefined?null:JSON.stringify(r).slice(0,300)})), // the file in its wire form: no value the wire hides
 duplicateKinds:(()=>{const seen=new Set(),d={};for(const x of joined){const k=key(x);if(seen.has(k))d[kind(x)]=(d[kind(x)]||0)+1;seen.add(k)}return d})(),
 referenceDuplicates:refLog.length-new Set(refLog.map(key)).size};
console.log(JSON.stringify(out,null,1));if(outFile)writeFileSync(outFile,JSON.stringify(out,null,1));
const checks={'first+resume has every log line once, in order (normalised)':normEq&&joined.length===refLog.length&&joined.length===lines.length,
 'no duplicates':out.duplicates===0,'reference equals events.jsonl line for line, through the end of the run':fileEq,
 'strict differences only blanked turn inputs':Object.keys(kinds).every(k=>/SettingSet:(fallback|launch)\.input\./.test(k)),
 'resume honoured since/origin':resumeSince===held&&resumeOrigin===firstOrigin,
 'live frame carried items that are not log lines':firstLive>0,
 'strict-absent items are exactly the blanked inputs (one per thread)':out.strictAbsent===0||(out.strictAbsentKinds.length===1&&/fallback|launch/.test(out.strictAbsentKinds[0])),
 'a client counting live items into since is short by exactly that many':out.faultyClient.absentLines===firstLive};
let ok=true;for(const [k,v] of Object.entries(checks)){console.log((v?'PASS ':'FAIL ')+k);ok=ok&&v}
console.log(ok?'RESULT PASS':'RESULT FAIL');out.checks=checks;out.pass=ok;if(outFile)writeFileSync(outFile,JSON.stringify(out,null,1));
ws.close();ws2.close();stopHub(h);await sleep(800);process.exit(ok?0:1);
