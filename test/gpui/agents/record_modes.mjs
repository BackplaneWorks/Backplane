// Record the JS hub's events AND the raw lines claude_standin.ts wrote, for a scenario, as fixtures for the Rust ledger/bg/todo tests.
// usage: bun test/gpui/agents/record_modes.mjs OUTDIR NAME MODE "message1" ["message2" ...]   (env passes through: BP_STREAM_N, BP_WORK_MS ...)
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,copyFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
const [out,name,mode,...msgs]=process.argv.slice(2);
const work=out+'/work-'+name;rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');const lines=work+'/lines.log';
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec bun ${process.cwd()}/test/tools/claude_standin.ts "$@" | tee -a ${lines}\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve',home,port:Number(process.env.PORT||3844),standins:work+'/bin',log:work+'/hub.log',extraEnv:{FAKE_LOG:work+'/fake.log',BP_STANDIN_MODE:mode,BP_STREAM_N:process.env.BP_STREAM_N||'6',BP_STREAM_MS:process.env.BP_STREAM_MS||'60',BP_WORK_MS:process.env.BP_WORK_MS||'800'}});
console.log(execFileSync('test/gpui/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(4000);const c=await client(h);const th='t1700000001-2';
const board=work+'/fixroot/b.kicad_pcb';if(process.env.KICAD==='ecc83'){for(const f of ['ecc83-pp.kicad_pcb','ecc83-pp.kicad_pro','ecc83-pp.kicad_sch'])try{copyFileSync('/usr/share/kicad/demos/ecc83/'+f,work+'/fixroot/'+f)}catch{}}else if(process.env.KICAD)writeFileSync(board,'(kicad_pcb (version 20240108))\n');
let k=0;for(const m of msgs){c.send('turn.start',{thread:th,text:m});await sleep(Number(process.env.GAP_MS||300));if(process.env.KICAD==='1'&&k++===0){await sleep(3000);writeFileSync(board,'(kicad_pcb (version 20240108))\n(footprint "R1")\n')}}
const quiet=async(ms)=>{let n=-1;for(;;){await sleep(ms);if(c.msgs===n)return;n=c.msgs}};await quiet(Number(process.env.QUIET_MS||4000));
if(process.env.INTERRUPT_MS){}
stopHub(h);await sleep(1500);
const ev=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean);
writeFileSync(out+`/js-${name}.jsonl`,ev.slice(125).join('\n')+'\n');
try{copyFileSync(lines,out+`/lines-${name}.jsonl`)}catch{}
console.log('wrote',name,ev.length-125,'events');process.exit(0);
