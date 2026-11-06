// bun test/e2e/run.mjs --client gpui|android|ios [--journey all|stories|day|everything|story:S04|<id>] [--hub-bin PATH] [--gpui-bin PATH] [--out DIR] [--hz N] [--record] [--threads N]
//   [--seed fixture|lived-in] [--copy-of HOME] [--display N] [--port N]
// `all` = the 12 gate journeys (docs/final-gate.json, unchanged); `stories` = the user-story journeys (stories.md), `day` = the day-in-the-life
// walks; they run on their own hub (the story stand-in, the lived-in workspace) and never touch the gate mapping.
// --copy-of HOME: every journey runs on a COPY of that home (lib/hubfx.mjs startCopyHub: read-only source, BACKPLANE_COPY=1); journeys that need
// the fixture's seeded files, bots or disk changes are blocked there. A real store also needs E2E_COPY_REAL=person.
// Runs on Boat (Xvfb) for GPUI; never starts a display or hub on the desktop without E2E_ALLOW_DESKTOP=1 (the person's word).
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';import {dirname,resolve} from 'node:path';
import {frictions,markdown,DEFAULT_BUDGETS} from './lib/report.mjs';
const here=dirname(fileURLToPath(import.meta.url));
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const flag=k=>process.argv.includes('--'+k);
const hubKind=arg('hub-kind','js'),realHw=flag('real-hardware');const client=arg('client','gpui'),which=arg('journey','all'),out=resolve(arg('out','out/e2e')+'/'+client),hubBin=arg('hub-bin',process.env.HUB_BIN),gpuiBin=arg('gpui-bin',process.env.GPUI_BIN);
const copyOf=arg('copy-of',null);
const J=JSON.parse(readFileSync(here+'/journeys.json','utf8'));const hz=Number(arg('hz',J.refresh_hz_budget[client]));const budgets={...DEFAULT_BUDGETS,...(J.step_budgets_ms||{})};
const adapter=await import(`./adapters/${client==='gpui'?'desktop':client}.mjs`);mkdirSync(out,{recursive:true});
const gate=J.journeys.map(j=>({...j,gate:true})),stories=(J.stories||[]).map(j=>({...j,gate:false}));
const todo=which==='all'?gate:which==='stories'?stories.filter(j=>j.story!=='day'):which==='day'?stories.filter(j=>j.story==='day'):which==='everything'?[...gate,...stories]
  :which.startsWith('story:')?stories.filter(j=>j.story===which.slice(6)):[...gate,...stories].filter(j=>j.id===which);
if(!todo.length){console.error('no journey matches '+which);process.exit(2)}
// one hub per profile: the gate journeys keep the perf stand-in and the fixture seed they were written for; stories get the story stand-in
const profileOf=j=>copyOf?'copy':j.gate?'gate':(arg('seed',j.seed||'lived-in')==='lived-in'?'lived':'storyfx');
const PROFILES={gate:{standin:'perf',seed:'fixture'},storyfx:{standin:'story',seed:'fixture'},lived:{standin:'story',seed:'lived-in'}};
// a copy holds someone's real threads: nothing seeded is there, and the harness writes nothing into its projects
const FIXTURE_STEPS=new Set(['paste_image','attach','open_bot','open_room','post_room','download','fs','pair']);
const copyBlocked=j=>[...new Set(j.steps.filter(s=>FIXTURE_STEPS.has(s.do)).map(s=>'copy: '+s.do+' needs the fixture seed'))];
const summary={client,when:new Date().toISOString(),hz,hub:hubBin||null,copyOf:copyOf?resolve(copyOf):null,budgets,results:[],captures:[],env:{},hubs:{}};
let display=null,fx=null;
try{
  if(client==='gpui'){
    const cap=await import('./lib/capture.mjs');summary.env=cap.installMissing();
    if(!summary.env.tools.Xvfb)throw Error('no Xvfb on this machine (run on h@backplane or Boat): '+JSON.stringify(summary.env));
    if(!gpuiBin)throw Error('--gpui-bin (the Linux GPUI binary) is required');
    display=await cap.startDisplay(Number(arg('display',62)));
  }
  const {startFixtureHub,startCopyHub,startAgents}=await import('./lib/hubfx.mjs');
  const groups=[...new Set(todo.map(profileOf))];
  for(const g of groups){
    if(client==='gpui'||hubBin){
      if(!hubBin)throw Error('--hub-bin is required');
      const root=resolve(arg('root','out/e2e-run')+'/'+client+(g==='gate'?'':'-'+g)),port=Number(arg('port',3811));
      fx=g==='copy'?await startCopyHub({bin:resolve(hubBin),source:resolve(copyOf),root,port})
        :await startFixtureHub({bin:resolve(hubBin),root,port,threads:Number(arg('threads',20)),...PROFILES[g]});
      summary.seedReport=fx.seedReport;summary.hubPid=fx.hub.pid;summary.hubs[g]={pid:fx.hub.pid,root,seed:fx.seedReport,lived:fx.state.lived?{stuck:fx.state.lived.stuck,threads:fx.state.lived.threads.length}:undefined};
    }
    for(const j of todo.filter(x=>profileOf(x)===g)){
      if(j.needs.includes('agents')&&fx)startAgents(fx,20);
      let r;const cb=g==='copy'?copyBlocked(j):[];
      if(cb.length)r={journey:j.id,client,status:'blocked',blocked:cb,metrics:{},notes:[]};
      else try{r=await adapter.run(j,{gpuBin:gpuiBin?resolve(gpuiBin):null,display,hub:fx,outDir:out,hz,budgets,record:flag('record')})}catch(e){r={journey:j.id,client,status:'fail',metrics:{},notes:['harness error: '+String(e.stack||e).slice(0,400)]}}
      // a hub that refused a seed RPC the journey needs makes the journey blocked, not failed
      r.item=j.item;r.story=j.story||null;r.gate=j.gate;summary.results.push(r);for(const k of ['capture','screenshot'])if(r[k])summary.captures.push(r[k]);
      const sl=r.stepSummary?.slowest;console.log(`${r.status.padEnd(8)} ${j.id.padEnd(22)} ${(r.blocked||[]).join(', ')} ${sl?`slowest ${sl.step} ${Math.round(sl.ms)} ms (${sl.verdict})`:''} ${r.notes?.[0]||''}`.slice(0,220));
    }
    try{await fx?.stop()}catch{}fx=null;
  }
}catch(e){summary.fatal=String(e.stack||e).slice(0,600);console.error('fatal:',summary.fatal)}
finally{try{await fx?.stop()}catch{}try{display?.stop()}catch{}}
// pass rule (CONTRACT.md): a pass needs the Rust hub on real hardware; anything else is demoted
for(const r of summary.results){if(r.status==='pass'&&!(hubKind==='rust'&&realHw&&r.gpu&&r.gpu.software===false)){r.status='partial';r.notes=[...(r.notes||[]),`would pass, but not on rust hub + real hardware (hub=${hubKind}, real-hardware=${realHw})`]}}
summary.hubKind=hubKind;summary.realHardware=realHw;
// gate mapping (gate journeys only): pass -> pass, fail -> fail, everything else stays open
summary.gate=Object.fromEntries(summary.results.filter(r=>r.gate).map(r=>[r.item,r.status==='pass'?'pass':r.status==='fail'?'fail':'open']));
summary.counts=summary.results.reduce((a,r)=>(a[r.status]=(a[r.status]||0)+1,a),{});
// what a person would trip over, ranked by harm (fails before slow before cosmetic); blocked journeys are coverage gaps, listed apart
summary.frictions=frictions(summary.results);
writeFileSync(out+'/summary.json',JSON.stringify(summary,null,1));writeFileSync(out+'/report.md',markdown(summary));
console.log(JSON.stringify(summary.counts),'->',out+'/summary.json',out+'/report.md');
process.exit(flag('strict')&&(summary.fatal||summary.counts.fail)?1:0);
