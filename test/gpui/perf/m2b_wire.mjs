// What the hub sends a mobile-renderer view client. bun m2b_wire.mjs STATE LABEL SECS MODE(push|poll) DELTA(0|1)
import {readFileSync,writeFileSync} from 'node:fs';import {viewClient} from './view.mjs';import {sleep,pct} from './lib.mjs';
const [sp,label,secsS,mode,deltaS]=process.argv.slice(2);const secs=Number(secsS),delta=deltaS==='1';
const st=JSON.parse(readFileSync(sp,'utf8'));const h={host:'127.0.0.1',port:st.port,token:st.token,url:`http://127.0.0.1:${st.port}`};
const v=await viewClient(h,{mobile:true,delta});await v.open();
v.target={kind:'thread',id:st.threads[0]};v.viewId='';const first=await v.req('view.open');
const R={label,threads:st.threads.length,agents:st.agents,mode,delta,secs,firstOpenBytes:first.bytes};
const body=first.x.result;const sz=o=>JSON.stringify(o).length;
R.firstOpenFields=Object.fromEntries(Object.entries(body).map(([k,x])=>[k,sz(x)]).sort((a,b)=>b[1]-a[1]).slice(0,8));
let sh=body.shown;R.shownTop=Object.fromEntries(Object.entries(sh||{}).map(([k,x])=>[k,sz(x)]).sort((a,b)=>b[1]-a[1]).slice(0,8));
v.bytesIn=0;v.msgsIn=0;v.push=0;let pushBytes=0;v.onPush=(x,n)=>{pushBytes+=n;v.dirty=true};v.dirty=false;v.stats.length=0;
const fieldBytes={},fieldCount={};const frames=[];
const t0=Date.now();
while(Date.now()-t0<secs*1000){
 if(mode==='poll'||v.dirty){v.dirty=false;const r=await v.req('view.update');
  if(r){const d=r.x.result?.viewDelta;let ops=null;if(d&&Object.hasOwn(d,'ops')){ops=d.ops;for(const op of ops){const p=op[1].slice(0,3).join('.');const b=JSON.stringify(op).length;fieldBytes[p]=(fieldBytes[p]||0)+b;fieldCount[p]=(fieldCount[p]||0)+1}}
   frames.push({t:(Date.now()-t0)/1000,bytes:r.bytes,delta:!!ops,nops:ops?.length??null,ms:r.ms})}}
 await sleep(mode==='poll'?1000:50)}
const wall=(Date.now()-t0)/1000;const bs=frames.map(f=>f.bytes);
R.bytesPerSec=v.bytesIn/wall;R.msgsPerSec=v.msgsIn/wall;R.framesPerSec=frames.length/wall;R.pushPerSec=v.push/wall;R.pushBytesPerSec=pushBytes/wall;
R.frameBytes={p50:pct(bs,50),p95:pct(bs,95),max:Math.max(...bs,0),mean:bs.reduce((a,b)=>a+b,0)/Math.max(1,bs.length)};
R.deltaFrames=frames.filter(f=>f.delta).length;R.wholeFrames=frames.length-R.deltaFrames;
R.topChangedPaths=Object.entries(fieldBytes).sort((a,b)=>b[1]-a[1]).slice(0,10).map(([p,b])=>({path:p,bytes:b,ops:fieldCount[p]}));
R.sample=frames.slice(0,5);
// when a frame was whole: what dominated it
const whole=v.last;writeFileSync(`${process.env.OUT||'.'}/${label}.json`,JSON.stringify(R,null,1));
console.log(JSON.stringify({...R,sample:undefined},null,1));process.exit(0);
