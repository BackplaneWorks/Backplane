// The thread-folder boundary (#960, law thread_folder_refuses_job) as explicit expectations, not as "same as the JS hub":
// in a recording of scenario `unresolved`, every RPC about an unknown, empty or deleted thread id must be refused (ok:false) and every
// HTTP probe must be a 4xx. usage: bun expect_unresolved.mjs RECORDING.json
import {readFileSync} from 'node:fs';
const d=JSON.parse(readFileSync(process.argv[2],'utf8'));
const sent=new Map(),bad=[];let rpcs=0,refused=0,http=0,http4=0;
for(const f of d.frames){const j=f.json;
  if(f.dir==='out'&&j&&typeof j==='object'&&j.m)sent.set(j.id,[j.m,j.p?.thread,j.p?.path]);
  if(f.dir==='in'&&j?.t==='reply'&&sent.has(j.id)){const [m,th,pa]=sent.get(j.id);rpcs++;if(j.ok===false)refused++;else bad.push(`${m} thread=${JSON.stringify(th)}${pa!==undefined?' path='+JSON.stringify(pa):''}: answered ok`)}
  if(f.kind==='http'&&f.dir==='in'&&f.status!==undefined&&f.c){http++;if(f.status>=400&&f.status<500)http4++;else bad.push(`http ${f.status}`)}}
console.log(JSON.stringify({scenario:d.meta.scenario,rpcs,refused,answeredOk:rpcs-refused,http,http4xx:http4,pass:bad.length===0}));
for(const b of bad.slice(0,40))console.log('  FAIL',b);
process.exit(bad.length?1:0);
