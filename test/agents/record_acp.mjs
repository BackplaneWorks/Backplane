// Record the JS hub's events for stand-in Grok / OpenCode turns (the oracle for the Rust port of providers/{acp,grok,opencode}.rs).
// usage: PORT=3880 bun test/agents/record_acp.mjs OUTDIR SCENARIO      writes OUTDIR/js-<scenario>.jsonl, OUTDIR/fake-<scenario>.log
// scenarios: grok-turn, grok-resume, grok-ask-accept, grok-ask-session, grok-ask-reject, oc-turn, oc-resume, oc-lost, oc-model, oc-nomodel, oc-ask
// The hub runs under a MemoryMax scope on a synthetic fixture home with ONLY stand-in agents (verified from its environ by startHub).
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,copyFileSync,appendFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
const [out,scen]=process.argv.slice(2);
const prov=scen.startsWith('oc')?'opencode':'grok';
const work=out+'/work-'+scen;rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot/src',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
const base=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).length;
const th='t1700000009-9',ev=[];
ev.push({$:'ThreadCreated',id:th,project:'p1700000000-1',title:'Acp',env:'local',branch:'',worktree:'',provider:prov,model:scen==='oc-model'||scen==='oc-nomodel'?'opencode/deepseek-v4.1-flash':'',at:1700000005});
const runtime=scen.includes('ask')?'approval-required':'full-access';
if(runtime!=='full-access')ev.push({$:'ModesSet',id:th,runtime,interaction:'default',model:'',at:1700000006});
if(scen==='grok-resume'||scen==='oc-resume'||scen==='oc-lost')ev.push({$:'SessionSet',id:th,session:'old-session',at:1700000007});
appendFileSync(home+'/events.jsonl',ev.map(e=>JSON.stringify(e)).join('\n')+'\n');
const sh=readFileSync('test/agents/standin-acp.sh','utf8');
for(const n of['claude','codex','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\n# fake standin block\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
for(const n of['grok','opencode'])writeFileSync(work+'/bin/'+n,sh),chmodSync(work+'/bin/'+n,0o755);
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve-current',home,port:Number(process.env.PORT||3880),standins:work+'/bin',log:work+'/hub.log',extraEnv:{FAKE_LOG:work+'/fake.log',FAKE_LOST:scen==='oc-lost'?'1':'',FAKE_NOMODEL:scen==='oc-nomodel'?'1':'',BACKPLANE_COPY:''}});
console.log(execFileSync('test/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(4000);const c=await client(h);
const quiet=async(ms)=>{let n=-1;for(;;){await sleep(ms);if(c.msgs===n)return;n=c.msgs}};
const asked=async()=>{for(let i=0;i<100;i++){const t=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(l=>l.includes('"AskOpened"')).pop();if(t)return JSON.parse(t).id;await sleep(100)}};
const send=(t)=>c.send('turn.start',{thread:th,text:t});
if(scen.includes('ask')){send('zqask please');const id=await asked();await sleep(300);c.send('ask.answer',{id,answer:scen.endsWith('reject')?'decline':scen.endsWith('session')?'session':'accept'})}
else{send('hello');}
await quiet(2500);
if(scen==='grok-resume'||scen==='oc-resume'){send('second');await quiet(2500)}
stopHub(h);await sleep(1500);
const all=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean);
writeFileSync(out+`/js-${scen}.jsonl`,all.slice(base+ev.length).join('\n')+'\n');
copyFileSync(work+'/fake.log',out+`/fake-${scen}.log`);
console.log('wrote',scen,all.length-base-ev.length,'events');process.exit(0);
