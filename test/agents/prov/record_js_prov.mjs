// Record the JS hub's events.jsonl for stand-in Muse and Antigravity turns (the oracle for the Rust port; stand-in agents only, no model, no credentials).
// usage (from the main gpui worktree, where build/gpui/ lives): bun test/agents/prov/record_js_prov.mjs OUTDIR [scenario...]
//   scenarios: muse-turn muse-perm muse-resume agy-turn agy-perm      JS_HUB=build/gpui/backplane-serve-current  PORT=3885
import {mkdirSync,readFileSync,writeFileSync,copyFileSync,chmodSync,rmSync} from 'node:fs';
import {startHub,stopHub,makeStandins,client,sleep} from '../../perf/lib.mjs';
const here=new URL('.',import.meta.url).pathname;
const out=process.argv[2];const scens=process.argv.slice(3).length?process.argv.slice(3):['muse-turn','muse-perm','muse-resume','agy-turn','agy-perm'];
const bin=process.env.JS_HUB||'build/gpui/backplane-serve-current';let port=Number(process.env.PORT||3885);
mkdirSync(out,{recursive:true});
for(const scen of scens){
  const work=`${out}/work-${scen}`;rmSync(work,{recursive:true,force:true});mkdirSync(`${work}/root`,{recursive:true});
  const home=`${work}/home`;mkdirSync(home,{recursive:true});
  const T='t1700000010-2',P='p1700000000-1',t0=1700000000;
  const muse=scen.startsWith('muse'),perm=scen.endsWith('perm');
  const L=[{$:'ProjectCreated',id:P,title:'fixture',root:`${work}/root`,at:t0},
    {$:'ThreadCreated',id:T,project:P,title:'prov',env:'local',branch:'',worktree:'',provider:muse?'muse':'antigravity',model:'',at:t0+1},
    {$:'ModesSet',id:T,runtime:perm?'approval-required':'full-access',interaction:'default',model:'',at:t0+2}];
  if(scen==='muse-resume')L.push({$:'SessionSet',id:T,session:'OLDSESSION',at:t0+3});
  writeFileSync(`${home}/events.jsonl`,L.map(o=>JSON.stringify(o)).join('\n')+'\n');
  const bins=`${work}/bin`;makeStandins(bins);
  copyFileSync(`${here}standin-muse.py`,`${bins}/muse`);chmodSync(`${bins}/muse`,0o755);
  if(!muse){const d=`${home}/antigravity/1.3.0`;mkdirSync(d,{recursive:true});copyFileSync(`${here}standin-agy.py`,`${d}/agy_acp_server.par`);copyFileSync(`${here}agy-turn.jsonl`,`${d}/agy-turn.jsonl`);
    writeFileSync(`${d}/localharness_external`,'#!/bin/sh\n');chmodSync(`${d}/localharness_external`,0o755);chmodSync(`${d}/agy_acp_server.par`,0o755);writeFileSync(`${d}/.ok`,'')}
  const h=await startHub({bin,home,port:port++,standins:bins,extraEnv:{MUSE_LOG:`${work}/muse.log`,AGY_LOG:`${work}/agy.log`}});
  await sleep(3000);
  const c=await client(h);
  c.send('turn.start',{thread:T,text:perm?'zq perm please':'zq hello'});
  if(perm){let ask='';for(let i=0;i<100&&!ask;i++){await sleep(200);for(const l of readFileSync(`${home}/events.jsonl`,'utf8').split('\n')){if(l.includes('"AskOpened"')){ask=JSON.parse(l).id}}}
    console.log(scen,'ask',ask||'NONE');if(ask){await sleep(300);const r=await c.rpc('ask.answer',{id:ask,answer:'accept'});console.log(scen,'answered',JSON.stringify(r?.o?.ok))}}
  let n=-1;for(;;){await sleep(2000);const m=c.msgs;if(m===n)break;n=m}
  await sleep(500);stopHub(h);await sleep(1500);
  copyFileSync(`${home}/events.jsonl`,`${out}/events-${scen}.jsonl`);
  for(const f of ['muse.log','agy.log'])try{copyFileSync(`${work}/${f}`,`${out}/${f.replace('.log','')}-${scen}.log`)}catch{}
  console.log('wrote',`${out}/events-${scen}.jsonl`);
}
process.exit(0);
