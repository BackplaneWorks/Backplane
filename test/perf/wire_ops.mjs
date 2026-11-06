// aggregate delta ops of an idle mobile client by generic path. bun wire_ops.mjs STATE LABEL SECS
import {readFileSync,writeFileSync} from 'node:fs';import {viewClient} from './view.mjs';import {sleep} from './lib.mjs';
const [sp,label,secs]=process.argv.slice(2);const st=JSON.parse(readFileSync(sp,'utf8'));
const h={host:'127.0.0.1',port:st.port,token:st.token,url:`http://127.0.0.1:${st.port}`};
const v=await viewClient(h,{mobile:true,delta:true});await v.open();v.target={kind:'thread',id:st.threads[0]};v.viewId='';await v.req('view.open');
const agg={};const frames=[];const t0=Date.now();
while(Date.now()-t0<Number(secs)*1000){const r=await v.req('view.update');const d=r?.x?.result?.viewDelta;const ops=d?.ops||[];let by=0;
 for(const op of ops){const g=op[1].map(x=>typeof x==='number'?'*':x).join('.');const b=JSON.stringify(op).length;by+=b;const a=(agg[g]??={n:0,bytes:0,ex:null});a.n++;a.bytes+=b;if(a.ex===null||Math.random()<.02)a.ex=JSON.stringify(op[2]).slice(0,60)}
 frames.push({t:(Date.now()-t0)/1000,bytes:r?.bytes,ops:ops.length});await sleep(1000)}
const top=Object.entries(agg).sort((a,b)=>b[1].bytes-a[1].bytes).slice(0,14).map(([p,a])=>({path:p,...a}));
const big=frames.filter(f=>f.bytes>2000).map(f=>[Math.round(f.t),f.bytes,f.ops]);
writeFileSync(`${process.env.OUT}/${label}-ops.json`,JSON.stringify({label,threads:st.threads.length,top,bigFrames:big,frames:frames.length,meanBytes:frames.reduce((a,f)=>a+f.bytes,0)/frames.length},null,1));
console.log(JSON.stringify({top,big}).slice(0,3500));process.exit(0);
