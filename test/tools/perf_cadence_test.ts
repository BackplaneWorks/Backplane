import assert from 'node:assert/strict';
import {cadenceReport, type CadenceEvidence} from './perf_cadence';
const digest = (n: number) => n.toString(16).padStart(64,'0');
const period = 1000 / 60;
const demands = Array.from({length:600}, (_,i) => ({id:`d${i}`,revision:`r${i}`,
  contentHash:digest(i+1),pixelHash:digest(i+1001),region:'timeline',features:['timeline'],scheduled:i*period,sent:i*period+.1,observed:i*period+.2}));
const drawn = demands.map((d,i) => ({frame:i+1,at:d.scheduled+4,revision:d.revision,
  contentHash:d.contentHash,pixelHash:digest(i+1001),features:['timeline'],region:'timeline'}));
const good: CadenceEvidence = {boundary:'xserver-readback',start:0,end:10000,refreshHz:60,refresh:{source:'xvfb-reference',hz:60,intervals_ms:[]},
  demands,drawn,readbacks:drawn.map(f=>({at:f.at+.75,region:f.region,pixelHash:f.pixelHash})),presented:drawn.map(f => ({frame:f.frame,at:f.at+1}))};
assert.equal(cadenceReport(good).accepted,true);
assert.equal(cadenceReport(good).physical_scanout_verified,false);
const stalled=cadenceReport({...good,presented:good.presented.filter(p => p.at<3000 || p.at>4500)});
assert.equal(stalled.accepted,false);
assert(stalled.longest_stall_ms>1500);
assert(stalled.missed_refresh_deadlines>=89);
for(const patch of [
  {boundary:'xserver-ack' as const}, {readbacks:[]},
  {readbacks:good.readbacks!.map(p=>({...p,pixelHash:digest(1001)}))},
  {readbacks:good.readbacks!.map(p=>({...p,region:'wrong-roi'}))},
  {readbacks:good.readbacks!.map(p=>({...p,at:p.at+100}))},
  {boundary:'missing' as const}, {boundary:'native-display-link' as const},
  {presented:[]}, {drawn:[]}, {demands:[]}, {refreshHz:30}, {refreshHz:6000}, {end:9000},
  {presented:good.presented.slice(0,299)},
  {presented:good.presented.map(p => ({...p,at:5}))},
  {presented:[...good.presented].reverse()},
  {presented:good.presented.map(p => ({...p,frame:1}))},
  {presented:good.presented.map((p,i) => ({...p,at:i+1}))},
  {presented:good.presented.filter((_,i) => i%2===0)},
  {demands:demands.filter((_,i) => i!==200)},
  {demands:demands.map((d,i) => i===200 ? {...d,sent:d.scheduled+30} : d)},
  // New acknowledgements, old rendered revision: a frozen screen at 60 callbacks/s.
  {drawn:drawn.map(f => ({...f,revision:'r0',contentHash:digest(1)}))},
  // Claimed fresh revision, but cached image/pixels never changed.
  {drawn:drawn.map(f => ({...f,pixelHash:digest(1001)}))},
  {drawn:drawn.map(f => ({...f,pixelHash:digest(999999)}))},
  {drawn:drawn.map(f => ({...f,region:'unrelated-sidebar'}))},
  {drawn:drawn.map(f => ({...f,features:['status']}))},
  {demands:demands.map(d => ({...d,features:['timeline','rooms','bots','status']}))},
  {drawn:drawn.map(f => ({...f,contentHash:digest(777)}))},
  {drawn:drawn.map(f => ({...f,at:f.at+100}))},
  {drawn:drawn.map(f => ({...f,frame:1}))},
  {demands:demands.map(d => ({...d,id:'same-demand'}))},
]) assert.equal(cadenceReport({...good,...patch}).accepted,false,JSON.stringify(patch).slice(0,160));
console.log('PASS content-correlated cadence: stale pixels/revisions, callbacks, bogus Hz/counts, bursts, gaps and duplicate frames fail');

// Two inputs inside a refresh can legitimately render their cumulative final
// state in one frame. Both effects are in the independently expected prefix.
const {contentResponses}=await import('./perf_cadence');
const two=[{...demands[0],scheduled:0,sent:.1,observed:.2},{...demands[1],scheduled:1,sent:1.1,observed:1.2}];
const combined=[{...drawn[1],at:4}];
const responses=contentResponses(two,combined,[{frame:combined[0].frame,at:5}],16);
assert.equal(responses.filter(Boolean).length,2);
assert.equal(new Set(responses.map(r=>r!.frame)).size,1);
