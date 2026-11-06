// A workspace that looks used, for the user-story journeys and the day-in-the-life walk (was skills/backplane-e2e/trial.mjs + hub.sh's
// `multi` project): five projects (the fixture `proj`, `multi` with boards alpha/beta + an archived copy + a build snapshot, `amp`,
// `sensor-node`, `psu`), ~19 threads of 1-5 exchanges, three long ones (24-36), replies that start with tool calls, and some threads
// pinned, settled, snoozed or archived. Needs the 'story' stand-in (claude_standin.ts, stream mode). E2E_LIVED_SCALE (default 1) scales
// the exchange counts down for a quick run (0.2: ~1 min).
import {mkdirSync,copyFileSync,existsSync,readFileSync,writeFileSync} from 'node:fs';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
// what a person asks; the last word tags the stand-in's reply ("tools:N" runs N Bash calls first)
const ASKS=['Can you check the decoupling near the regulator? tools:2 decoupling','Walk me through the power tree from the input jack. power',
  'Are the differential pairs length matched? tools:3 pairs','Move the test points to the bottom side. tools:1 testpoints','What does DRC still complain about? tools:2 drc',
  'Summarise what changed since this morning. summary','Is the ground pour stitched well enough? tools:1 stitching','Pick a cheaper part for the 10 uF caps. caps',
  'Check the silkscreen for overlaps. tools:2 silk','Why is the ratsnest still showing two nets? nets'];
export const PLAN=[['proj','Route the USB-C connector',30,'pin'],['proj','Check decoupling on U3',3],['proj','BOM cleanup',2,'archive'],['proj','Silkscreen fixes',1,'settle'],
  ['proj','Why does DRC flag the via?',4],['multi','Alpha: power tree review',24],['multi','Beta: swap the MCU',3],['multi','Compare the alpha and beta stackups',2],
  ['amp','Bias the tube stage',5],['amp','Lower the hum',2,'settle'],['amp','Grounding plan',3],['sensor-node','Bring-up notes',36,'pin'],['sensor-node','I2C pull-ups',2],
  ['sensor-node','Battery life estimate',3],['sensor-node','Antenna keepout',1,'settle'],['sensor-node','Firmware pin map',2],['psu','Buck converter layout',4],
  ['psu','Thermal relief question',1,'settle'],['psu','Output ripple',2,'snooze']];
const BOARD='test/files/fixtures/plot/ecc83-pp';
function kicad(repo,dir,stem){mkdirSync(dir,{recursive:true});for(const x of ['kicad_pcb','kicad_sch'])if(existsSync(`${repo}/${BOARD}.${x}`))copyFileSync(`${repo}/${BOARD}.${x}`,`${dir}/${stem}.${x}`);
  writeFileSync(`${dir}/${stem}.kicad_pro`,JSON.stringify({meta:{filename:stem+'.kicad_pro',version:1}}))}
const events=root=>{try{return readFileSync(root+'/home/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}})}catch{return []}};
// turns of thread t that have ended (any state but running)
const ends=(root,t)=>events(root).filter(e=>e.$==='TurnChanged'&&e.thread===t&&e.state!=='running').length;
export async function seedLivedIn({c,call,root,repo,project}){
  const scale=Number(process.env.E2E_LIVED_SCALE||1);
  // multi: two real boards, an archived copy and a build snapshot (the board picker offers alpha and beta only)
  kicad(repo,root+'/multi/boards/alpha','alpha');kicad(repo,root+'/multi/boards/beta','beta');kicad(repo,root+'/multi/archive/alpha_revA','alpha');
  mkdirSync(root+'/multi/boards/alpha/build',{recursive:true});if(existsSync(root+'/multi/boards/alpha/alpha.kicad_pcb'))copyFileSync(root+'/multi/boards/alpha/alpha.kicad_pcb',root+'/multi/boards/alpha/build/before-reroute.kicad_pcb');
  writeFileSync(root+'/multi/.backplane.json',JSON.stringify({project:'boards/alpha/alpha.kicad_pro'}));
  for(const d of ['amp','sensor-node','psu'])kicad(repo,root+'/'+d,d);
  const ids={proj:project};
  for(const d of ['multi','amp','sensor-node','psu']){await call('project.add',{path:root+'/'+d});
    for(let i=0;i<80&&!ids[d];i++){const e=events(root).find(x=>x.$==='ProjectCreated'&&(x.title===d||String(x.root||'').endsWith('/'+d)));if(e)ids[d]=e.id;else await sleep(100)}}
  const threads=[];let k=0;
  for(const [d,title,n,then] of PLAN){const r=await call('thread.create',{project:ids[d],title,env:'local'});if(r?.thread)threads.push({thread:r.thread,project:d,title,n:Math.max(1,Math.round(n*scale)),then:then||''})}
  // each thread's exchanges in order, at most 6 threads at once; a turn that never ends is written down, not waited on forever
  const stuck=[];const run=async t=>{for(let i=1;i<=t.n;i++){const before=ends(root,t.thread);c.send('turn.start',{thread:t.thread,text:ASKS[k++%ASKS.length],msg:`lived-${t.thread}-${i}`,mode:''});
    const until=Date.now()+60000;while(ends(root,t.thread)<=before){if(Date.now()>until){stuck.push(`${t.title} turn ${i}`);return}await sleep(200)}}};
  const todo=[...threads];await Promise.all(Array.from({length:6},async()=>{for(let t;(t=todo.shift());)await run(t)}));
  for(const t of threads){await call('thread.rename',{thread:t.thread,title:t.title}); // the stand-in's answers may have retitled it
    if(t.then==='pin')await call('thread.pin',{thread:t.thread});
    await call(t.then==='settle'?'thread.settle':'thread.unsettle',{thread:t.thread}); // a finished turn may settle by itself; only the planned ones stay settled
    if(t.then==='archive')await call('thread.archive',{thread:t.thread});if(t.then==='snooze')await call('thread.snooze',{thread:t.thread,for:'1h'})}
  writeFileSync(root+'/lived-in.json',JSON.stringify({projects:ids,threads,stuck},null,1));
  return {projects:ids,threads,stuck}}
