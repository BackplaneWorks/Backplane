// Record the JS hub's events.jsonl for a stand-in Claude turn (the oracle for the Rust port).
// usage: bun test/agents/record_js.mjs OUTDIR [scenario]   (scenarios: turn, queue, interrupt)
import {mkdirSync,readFileSync,copyFileSync,writeFileSync} from 'node:fs';
import {startHub,stopHub,makeStandins,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
const out=process.argv[2],scen=process.argv[3]||'turn';
const bin=process.env.JS_HUB||'build/gpui/backplane-serve',port=Number(process.env.PORT||3840);
const work=out+'/work';mkdirSync(work+'/fixroot',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');makeStandins(work+'/bin');
const h=await startHub({bin,home,port,cap:'3700M',standins:work+'/bin',extraEnv:{FAKE_SECS:process.env.FAKE_SECS||'2',FAKE_PER:'2',FAKE_CYCLE:'5'}});
console.log(execFileSync('test/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(4000);
const c=await client(h);
const th='t1700000001-2';
const send=(t)=>c.send('turn.start',{thread:th,text:t});
const quiet=async(ms=3000)=>{let n=-1;for(;;){await sleep(ms);const m=c.msgs;if(m===n)return;n=m}};
if(scen==='turn'){send('zqshort hello');await sleep(1000);await quiet(2500)}
if(scen==='queue'){send('first');await sleep(300);send('second');await sleep(300);send('third');await sleep(1000);await quiet(2500)}
if(scen==='interrupt'){send('zqlong');await sleep(1500);c.send('turn.interrupt',{thread:th});await sleep(1000);await quiet(2500)}
await sleep(500);stopHub(h);await sleep(1500);
copyFileSync(home+'/events.jsonl',out+`/events-${scen}.jsonl`);
console.log('wrote',out+`/events-${scen}.jsonl`);process.exit(0);
