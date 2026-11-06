// Counts only, never content: bun probe_threads.mjs PORT TOKEN_FILE EVENTS.jsonl [N]  -> how many real threads the helper-free hub can open.
import {viewClient} from '../perf/view.mjs';
import {readFileSync,createReadStream} from 'node:fs';
import readline from 'node:readline';
const [port,tf,ev,nS]=process.argv.slice(2); const N=Number(nS||200); const token=readFileSync(tf,'utf8').trim();
const h={host:'127.0.0.1',port:Number(port),url:`http://127.0.0.1:${port}`,token};
const ids=[];const rl=readline.createInterface({input:createReadStream(ev)});
for await(const l of rl){if(l.startsWith('{"$":"ThreadCreated"')){const m=/"id":"([^"]*)"/.exec(l);if(m)ids.push(m[1])}}
for(let i=ids.length-1;i>0;i--){const j=Math.floor(Math.random()*(i+1));[ids[i],ids[j]]=[ids[j],ids[i]]}
let v;try{v=await viewClient(h,{mobile:true,delta:false})}catch(e){console.log('connect failed');process.exit(1)}
let ok=0,bad=0,codes={},ms=[],bytes=[],entries=[];
for(const id of ids.slice(0,N)){v.target={kind:'thread',id};v.viewId='';const t=await v.req('view.open',{nativeSidebar:1},20000);
 if(t&&t.x.ok){ok++;ms.push(t.ms);bytes.push(t.bytes)}else{bad++;const c=t?.x?.error?.code||'timeout';codes[c]=(codes[c]||0)+1}}
const p=(a,q)=>a.length?a.sort((x,y)=>x-y)[Math.min(a.length-1,Math.floor(q*a.length))]:null;
console.log(JSON.stringify({threads_in_log:ids.length,tried:Math.min(N,ids.length),opened:ok,refused:bad,codes,open_ms_p50:p(ms,.5),open_ms_p95:p(ms,.95),bytes_p50:p(bytes,.5)}));
v.close();process.exit(0);
