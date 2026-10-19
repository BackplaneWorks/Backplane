import assert from 'node:assert/strict';
import {verifyPerformanceBundle,type CandidateBinding,type PerformanceBundle,type RawPhase} from './perf_evidence';
import type {CadenceEvidence} from './perf_cadence';
const hash=(n:number)=>n.toString(16).padStart(64,'0');
const binding:CandidateBinding={commit:'a'.repeat(40),source_sha256:hash(1),binary_sha256:hash(2),
  compiler_sha256:hash(3),fixture_sha256:hash(4),harness_sha256:hash(5),host_sha256:hash(6),platform:'linux-native'};
const now=Date.now(),period=1000/60;
function content(count:number,step:number,surfaces=false):CadenceEvidence {
  const regions=['timeline','subagents','rooms','bots','status'];
  const demands=Array.from({length:count},(_,i)=>{
    const region=surfaces?regions[i%regions.length]:'timeline';
    return {id:`d${i}`,revision:`r${i}`,contentHash:hash(i+1),pixelHash:hash(i+1001),region,
      features:region==='timeline'?['timeline','long-history']:[region],scheduled:i*step,sent:i*step+.1,observed:i*step+.2};
  });
  const drawn=demands.map((d,i)=>({frame:i+1,at:d.sent+3,revision:d.revision,contentHash:d.contentHash,
    pixelHash:hash(i+1001),features:d.features,region:d.region}));
  return {boundary:'xserver-readback',start:0,end:20000,refreshHz:60,refresh:{source:'xvfb-reference',hz:60,intervals_ms:[]},demands,drawn,
    readbacks:drawn.map(f=>({at:f.at+.75,region:f.region,pixelHash:f.pixelHash})),presented:drawn.map(f=>({frame:f.frame,at:f.at+1}))};
}
function phase(name:string):RawPhase {
  const spans=Array.from({length:1200},(_,i)=>({name:'ui.frame',at_ms:i*period,duration_ms:5}));
  spans.push(...Array.from({length:200},(_,i)=>({name:'hub.agent_line',at_ms:i*100,duration_ms:.3})));
  if(name==='input') spans.push(...Array.from({length:100},(_,i)=>({name:'ui.input',at_ms:i*200,duration_ms:8})));
  return {name,start_ms:0,end_ms:20000,spans,streams:Array.from({length:10},(_,i)=>({id:`agent${i}`,
    received_ms:Array.from({length:40},(_,j)=>j*500)})),
    opens:name==='thread-switch'?Array.from({length:10},(_,i)=>({expected:`t${i}`,drawn:`t${i}`,duration_ms:20})):[],
    cadence:name==='continuous-scroll'?content(1200,period):undefined,
    content:name==='input'?content(100,200):name==='thread-switch'?content(10,2000,true):undefined};
}
const good:PerformanceBundle={schema:1,built:binding,measured:binding,started_at:now-100000,completed_at:now-1000,
  history_bytes:71_500_000,phases:['continuous-scroll','input','thread-switch','background'].map(phase)};
const required=['timeline','long-history','subagents','rooms','bots','status'];
const verify=(b:PerformanceBundle,expect=binding)=>verifyPerformanceBundle(b,expect,required,now);
assert.equal(verify(good).accepted,true,JSON.stringify(verify(good)));
// Mandatory coverage cannot be reduced by a wrapper or direct CLI caller.
const timelineOnly=structuredClone(good);timelineOnly.phases[2].content=content(10,2000);
assert.equal(verifyPerformanceBundle(timelineOnly,binding,['timeline'],now).accepted,false);
const relabelled=structuredClone(good);
for(const p of relabelled.phases) (p.cadence??p.content)?.drawn.forEach(f=>f.features=[...required]);
assert.equal(verify(relabelled).accepted,false,'one timeline ROI cannot claim every surface');
// Optional feature content cannot borrow a different time window, an infinite
// budget, or a different evidence field while its stream/span phase stays green.
for(const defect of ['before','after','zero-hz','missing-hz','missing-refresh','bogus-refresh','late','wrong-field']) {
  const b=structuredClone(good);b.phases[2].content=content(10,2000);
  const extra=phase('normal');extra.content=content(10,2000,true);
  const e=extra.content;
  if(defect==='before' || defect==='after') {
    const offset=defect==='before'?-30000:30000;e.start+=offset;e.end+=offset;
    for(const d of e.demands) {d.scheduled+=offset;d.sent+=offset;d.observed+=offset;}
    for(const f of e.drawn) f.at+=offset;
    for(const p of e.presented) p.at+=offset;
    for(const p of e.readbacks!) p.at+=offset;
  } else if(defect==='zero-hz') e.refreshHz=0;
  else if(defect==='missing-hz') delete (e as any).refreshHz;
  else if(defect==='missing-refresh') delete (e as any).refresh;
  else if(defect==='bogus-refresh') e.refresh.hz=120;
  else if(defect==='late') {
    for(const f of e.drawn) f.at+=30;
    for(const p of e.presented) p.at+=30;
    for(const p of e.readbacks!) p.at+=30;
  } else {extra.cadence=e;delete extra.content;}
  b.phases.push(extra);
  assert.equal(verify(b).accepted,false,`optional feature witness ${defect}`);
}
const invalidExtra=structuredClone(good),invalidNormal=phase('normal');
invalidNormal.content=content(10,2000,true);invalidNormal.content.refreshHz=0;
invalidExtra.phases.push(invalidNormal);
assert.equal(verify(invalidExtra).accepted,false,'invalid supplied optional evidence cannot hide behind other valid coverage');
const droppedSurface=structuredClone(good);
for(const p of droppedSurface.phases) {
  const e=p.cadence??p.content;if(!e) continue;
  for(const d of e.demands) d.features=d.features.filter(f=>f!=='long-history');
  for(const f of e.drawn) f.features=f.features.filter(f=>f!=='long-history');
}
const mergedInput=droppedSurface.phases[1].content!;
mergedInput.demands[0].features=['timeline','long-history'];
Object.assign(mergedInput.demands[1],{scheduled:1,sent:1.1,observed:1.2});
mergedInput.drawn=mergedInput.drawn.slice(1);mergedInput.drawn[0].at=4;
mergedInput.presented=mergedInput.presented.slice(1);mergedInput.presented[0].at=5;
mergedInput.readbacks=mergedInput.readbacks!.slice(1);mergedInput.readbacks[0].at=4.5;
assert(verify(droppedSurface).failures.includes('feature scenario missing: long-history'),
  'coalescing cannot credit an earlier surface absent from the actually presented prefix');
const mutate=(f:(b:PerformanceBundle)=>void)=>{const b=structuredClone(good);f(b);assert.equal(verify(b).exitCode,1);};
for(const key of Object.keys(binding) as (keyof CandidateBinding)[]) mutate(b=>{(b.measured as any)[key]='wrong';});
for(const key of ['commit','source_sha256','binary_sha256','compiler_sha256']) mutate(b=>{(b.built as any)[key]='wrong';});
mutate(b=>{b.started_at=now-25*3600*1000;});
mutate(b=>{b.completed_at=now+1000;});
mutate(b=>{b.history_bytes=1000;});
mutate(b=>{b.phases=[];});
mutate(b=>{b.phases=b.phases.slice(1);});
mutate(b=>{b.phases[0].spans=[];});
mutate(b=>{b.phases[0].streams.pop();});
mutate(b=>{b.phases[0].streams[0].received_ms=[0,1,2,3];});
mutate(b=>{b.phases[0].streams[1].id=b.phases[0].streams[0].id;});
mutate(b=>{b.phases[0].spans[0].duration_ms=17;});
mutate(b=>{for(const s of b.phases[0].spans) if(s.name==='ui.frame') s.duration_ms=9;});
mutate(b=>{b.phases[0].cadence!.drawn.forEach(f=>f.revision='stale');});
mutate(b=>{b.phases[0].cadence!.drawn.forEach(f=>f.features=[]);b.phases[1].content!.drawn.forEach(f=>f.features=[]);b.phases[2].content!.drawn.forEach(f=>f.features=[]);});
mutate(b=>{b.phases[0].cadence!.end=19999;});
mutate(b=>{delete b.phases[1].content;});
mutate(b=>{delete b.phases[1].content!.readbacks;});
mutate(b=>{b.phases[0].cadence!.boundary='xserver-ack';});
mutate(b=>{b.phases[0].cadence!.readbacks!.forEach(p=>p.pixelHash=hash(1));});
mutate(b=>{b.phases[1].content!.drawn.forEach(f=>f.pixelHash=hash(1));});
mutate(b=>{b.phases[2].opens[0].drawn='wrong-thread';});
mutate(b=>{b.phases[2].opens[0].duration_ms=51;});
const simulator={...binding,platform:'ios-simulator' as const};
assert.equal(verify({...good,built:simulator,measured:simulator},simulator).accepted,false);
const device={...binding,platform:'ios-device' as const};
assert.equal(verify({...good,built:device,measured:device},device).accepted,false);
// An old report's summary cannot override failing raw observations.
mutate(b=>{(b as any).accepted=true;(b as any).checks={'all green':true};b.phases[0].spans=[];});
console.log('PASS exact candidate/source/binary/compiler/fixture/harness/host bindings and raw budget/coverage/device evidence fail closed');
