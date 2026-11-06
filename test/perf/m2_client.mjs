// bun m2_client.mjs STATE.json HOSTOVERRIDE SCENARIO LABEL [arg]
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import net from 'node:net';
import crypto from 'node:crypto';
import {client,sleep,pct,f1} from './lib.mjs';import {viewClient} from './view.mjs';
const [statePath,hostO,scenario,label,arg]=process.argv.slice(2);
const st=JSON.parse(readFileSync(statePath,'utf8'));const h={host:hostO||'127.0.0.1',port:st.port,token:st.token,url:`http://${hostO||'127.0.0.1'}:${st.port}`};
const OUT=process.env.OUT||'.';mkdirSync(OUT,{recursive:true});
const stat=(a)=>({n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:Math.max(...a)});
const R={label,scenario,host:h.host,arg,when:new Date().toISOString(),machine:process.platform+'/'+process.arch};
if(scenario==='rtt'){
 const c=await client(h);const ping=[];for(let i=0;i<20;i++)await c.rpc('load.ping',{});for(let i=0;i<100;i++){ping.push((await c.rpc('load.ping',{})).ms);await sleep(50)}
 R.ping=stat(ping);
 for(const delta of [false,true]){const v=await viewClient(h,{mobile:true,delta});await v.open();
  v.target={kind:'thread',id:st.threads[0]};v.viewId='';await v.req('view.open');
  const up=[],ub=[];for(let i=0;i<100;i++){const r=await v.req('view.update');up.push(r.ms);ub.push(r.bytes);await sleep(100)}
  const sw=[],sb=[];for(let i=0;i<60;i++){v.target={kind:'thread',id:st.threads[(i+1)%2]};v.viewId='';const r=await v.req('view.open');sw.push(r.ms);sb.push(r.bytes);await sleep(100)}
  R[delta?'update_delta':'update_whole']={ms:stat(up),bytes:stat(ub)};R[delta?'switch_delta':'switch_whole']={ms:stat(sw),bytes:stat(sb)};v.close()}
}
if(scenario==='viewers'){ // K viewers on live thread, delta on/off, 30 s
 const K=Number(arg||5),delta=process.env.DELTA!=='0',secs=Number(process.env.SECS||30);
 const vs=[];for(let i=0;i<K;i++){const v=await viewClient(h,{mobile:true,delta,tag:'v'+i});await v.open();v.target={kind:'thread',id:st.threads[i%Math.max(1,st.agents||1)]};v.viewId='';await v.req('view.open');
  v.busy=false;v.dirty=false;v.upd=[];v.mark=()=>{v.dirty=true};v.onPush=(x)=>{if(x.t==='view.changed')v.dirty=true};vs.push(v)}
 const t0=Date.now();for(const v of vs){v.bytesIn=0;v.msgsIn=0;v.stats.length=0}
 const loops=vs.map(async v=>{while(Date.now()-t0<secs*1000){if(v.dirty){v.dirty=false;await v.req('view.update')}await sleep(100)}});
 await Promise.all(loops);const wall=(Date.now()-t0)/1000;
 R.viewers=K;R.delta=delta;R.secs=wall;R.bytesPerSecPerClient=vs.map(v=>v.bytesIn/wall);R.msgsPerSecPerClient=vs.map(v=>v.msgsIn/wall);
 const all=vs.flatMap(v=>v.stats.filter(s=>s.m==='view.update').map(s=>s.ms));R.updateMs=stat(all);R.updates=all.length;R.deltaShare=vs.flatMap(v=>v.stats).filter(s=>s.delta).length/Math.max(1,vs.flatMap(v=>v.stats).length);
 R.meanReplyBytes=vs.flatMap(v=>v.stats).reduce((a,s)=>a+s.bytes,0)/Math.max(1,vs.flatMap(v=>v.stats).length);
 vs.forEach(v=>v.close());
}
if(scenario==='slow'){ // stuck readers: raw sockets that finish the ws upgrade (plain log stream since=0) and never read
 const K=Number(arg||3),phase=Number(process.env.SECS||45);const rd=()=>{try{return Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${st.pid}/status`,'utf8'))[1])/1024}catch{return NaN}};
 const c=await client(h);const socks=[];const series=[];
 const run=async(name,secs)=>{const t0=Date.now();while(Date.now()-t0<secs*1000){const r=await c.rpc('load.ping',{},5000);series.push({phase:name,t:Date.now(),rss:rd(),ms:r?.ms});await sleep(500)}};
 await run('before',phase);
 for(let i=0;i<K;i++){const s=net.connect(st.port,h.host);await new Promise(r=>s.once('connect',r));s.pause();const key=crypto.randomBytes(16).toString('base64');
  s.write(`GET /ws?since=0&token=${encodeURIComponent(st.token)} HTTP/1.1\r\nHost: ${h.host}:${st.port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${key}\r\nSec-WebSocket-Version: 13\r\n\r\n`);socks.push(s)}
 await run('stuck',phase*2);
 for(const s of socks)s.destroy();
 await run('after',phase);
 R.stuckReaders=K;R.phases={};for(const ph of ['before','stuck','after']){const a=series.filter(x=>x.phase===ph);const rs=a.map(x=>x.rss).filter(Number.isFinite),ms=a.map(x=>x.ms).filter(Number.isFinite);R.phases[ph]={samples:a.length,rssMB:{min:Math.min(...rs),p50:pct(rs,50),max:Math.max(...rs)},pingMs:stat(ms),pingTimeouts:a.length-ms.length}}
}
if(scenario==='drop'){
 // reference client sees every item; dropper disconnects, reconnects with since=<items seen>
 const ref=await client(h);const ids=a=>a.map(x=>JSON.stringify(x));
 const first=await client(h,{});await sleep(1500);const origin=first.origin??'';
 // count items the first client holds, then drop it
 const held=first.logItems.length;R.heldAllSeen=first.seen.length;R.otherFrameItems=(first.otherItems||[]).map(x=>x[0]).reduce((a,t)=>(a[t]=(a[t]||0)+1,a),{});first.ws.close();const dropAt=Date.now();
 await sleep(Number(arg||3000));const t0=performance.now();
 // reconnect with since=held
 const h2={...h};
 const ws=new WebSocket(`ws://${h.host}:${h.port}/ws?since=${held}&origin=${encodeURIComponent(origin)}&token=${encodeURIComponent(st.token)}`);ws.binaryType='arraybuffer';
 const {decode}=await import('../wire.mjs');const got=[];let firstFrame=null,logFrame=null;
 ws.onmessage=e=>{const o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data);if(firstFrame===null)firstFrame=performance.now()-t0;if(o.t==='log'){logFrame=performance.now()-t0;R.logSince=o.since}if(o.t==='log'||o.t==='changes')for(const it of o.items??[])got.push(it)};
 await new Promise(r=>ws.onopen=r);await sleep(3000);
 const refAll=ids(ref.logItems),firstAll=ids(first.logItems.slice(0,held)),reAll=ids(got);
 // after resume the union first+re should equal ref (events, not live deltas)
 const joined=[...firstAll,...reAll];
 R.itemsHeldBeforeDrop=held;R.refItems=refAll.length;R.resumeItems=reAll.length;R.firstFrameMs=firstFrame;R.logFrameMs=logFrame;
 R.joinedEqualsRef=JSON.stringify(joined)===JSON.stringify(refAll.slice(0,joined.length))&&joined.length>=refAll.length-5;
 const setJ=new Set(refAll);R.dupInResume=reAll.filter(x=>firstAll.includes(x)).length;R.missing=refAll.filter(x=>!joined.includes(x)).length;
 R.dropGapMs=Number(arg||3000);ws.close();
}
writeFileSync(`${OUT}/${label}.json`,JSON.stringify(R,null,1));console.log(JSON.stringify(R,null,1).slice(0,3000));process.exit(0);
