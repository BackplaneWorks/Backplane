// samples the hub on the store copy during the import: RSS, VmHWM, log progress; exits when the hub is gone or answers /hello
import {readFileSync,appendFileSync,writeFileSync,existsSync} from 'node:fs';
const OUT='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/perf/m0/import-samples.jsonl';writeFileSync(OUT,'');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const lock='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/perf-run/m0/home/hub.lock'; // the throwaway copy, never the store copyconst t0=Date.now();let pid=0,seenHello=false;
while(Date.now()-t0<1800000){
 if(!pid){const m=/^(\d+)/.exec(rd(lock));if(m)pid=Number(m[1])}
 if(pid){const st=rd(`/proc/${pid}/status`);if(!st){appendFileSync(OUT,JSON.stringify({t:(Date.now()-t0)/1000,gone:true,lastKnown:true})+'\n');break}
  const g=k=>Number(new RegExp(k+':\\s+(\\d+)').exec(st)?.[1]||0)/1024;
  let hello=false;try{hello=(await fetch('http://100.64.84.41:3830/hello',{signal:AbortSignal.timeout(500)})).status===200}catch{}
  appendFileSync(OUT,JSON.stringify({t:(Date.now()-t0)/1000,rssMB:g('VmRSS'),hwmMB:g('VmHWM'),hello})+'\n');if(hello&&!seenHello){seenHello=true}}
 await sleep(2000)}
