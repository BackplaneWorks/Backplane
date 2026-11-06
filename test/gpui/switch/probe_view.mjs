// Counts only, never content: bun probe_view.mjs PORT TOKEN_FILE   -> projects, threads, a real thread's entry count, timings.
import {viewClient} from '../perf/view.mjs';
import {readFileSync} from 'node:fs';
const [port,tf]=[process.argv[2],process.argv[3]]; const token=readFileSync(tf,'utf8').trim();
const h={host:'127.0.0.1',port:Number(port),url:`http://127.0.0.1:${port}`,token};
let v;try{v=await viewClient(h,{mobile:true,delta:false})}catch(e){console.log('connect failed');process.exit(1)}
let r=await v.open(); const res=r?.x?.result||{};
const walk=(o,f,d=0)=>{if(d>12||!o||typeof o!=='object')return;f(o);for(const k of Object.keys(o))walk(o[k],f,d+1)};
const threads=[];const projects=[];
walk(res.shown||res,o=>{if(Array.isArray(o.projects))for(const p of o.projects){projects.push(p);for(const t of [...(p.threads||[]),...(p.settled||[])])threads.push(t)}});
console.log(JSON.stringify(r.x.error||null));console.log(JSON.stringify({open_ms:+r.ms.toFixed(1),bytes:r.bytes,ok:r.x.ok,projects:projects.length,threads_listed:threads.length}));
if(threads.length){const id=String(threads[0].id).split('|').pop();v.target={kind:'thread',id};v.viewId='';const t=await v.req('view.open');const sh=t?.x?.result?.shown||{};let entries=0;walk(sh,o=>{if(Array.isArray(o.entries))entries=Math.max(entries,o.entries.length)});
 console.log(JSON.stringify({thread_open_ms:+t.ms.toFixed(1),bytes:t.bytes,ok:t.x.ok,entries}))}
v.close();process.exit(0);
