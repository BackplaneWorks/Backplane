// Record the JS hub's events for a stand-in Pi / Oh My Pi turn (the oracle for the Rust drivers). Stand-ins only.
// usage: bun test/agents/prov-pi/record_pi.mjs OUTDIR pi|omp SCENE "message" [approval-required|full-access]  (run from the repo root; JS_HUB=<backplane-serve>, PORT)
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,copyFileSync,appendFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../../perf/lib.mjs';
import {makeFixture} from '../../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
const [out0,prov,scene,msg,runtime='full-access',answer]=process.argv.slice(2);
const out=resolve(out0);
const name=`${prov}-${scene}`,work=out+'/work-'+name;rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
const th='t1700000001-2';
// the fixture's short thread, on this provider
const ev=readFileSync(home+'/events.jsonl','utf8').replace(/("id":"t1700000001-2".*?"provider":")claude/,`$1${prov}`);writeFileSync(home+'/events.jsonl',ev);
const base=ev.split('\n').filter(Boolean).length;
const here=new URL('.',import.meta.url).pathname;
for(const p of ['pi','omp']){copyFileSync(here+`standin-${p}.sh`,work+'/bin/'+p);chmodSync(work+'/bin/'+p,0o755)}
for(const n of['claude','codex','grok','muse']){writeFileSync(work+'/bin/'+n,'#!/bin/sh\n# standin fake\nexit 1\n');chmodSync(work+'/bin/'+n,0o755)}
const fake=work+'/fake.log';
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve-current',home,port:Number(process.env.PORT||3891),standins:work+'/bin',log:work+'/hub.log',extraEnv:{FAKE_LOG:fake}});
console.log(execFileSync('test/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(4000);const c=await client(h);
await c.rpc('thread.modes',{thread:th,runtime});
c.send('turn.start',{thread:th,text:msg});
if(answer!==undefined){ // wait for the ask, answer it
  const asked=await c.until(30000,()=>c.logItems.map(x=>typeof x==='string'?x:JSON.stringify(x)).find(l=>l.includes('AskOpened')));
  const id=/"id":"(q[^"]+)"/.exec(asked||'')?.[1];console.log('ask',id);await sleep(300);await c.rpc('ask.answer',{id,answer});}
let n=-1;for(;;){await sleep(2500);if(c.msgs===n)break;n=c.msgs}
stopHub(h);await sleep(1500);
const all=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean);
writeFileSync(out+`/js-${name}.jsonl`,all.slice(base).join('\n')+'\n');
try{copyFileSync(fake,out+`/fake-${name}.log`)}catch{}
console.log('wrote',name,all.length-base,'events');process.exit(0);
