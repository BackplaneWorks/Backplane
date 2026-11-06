// bun policy_latency.mjs BIN [N] : round-trip latency of the persistent bp-policy child over loopback TCP.
import net from 'node:net';import {spawn} from 'node:child_process';
const bin=process.argv[2],N=Number(process.argv[3]||5000);const port=34000+Math.floor(Math.random()*1000);
const p=spawn(bin,[String(port)]);await new Promise(r=>p.stdout.once('data',r));await new Promise(r=>setTimeout(r,300));
const s=net.connect(port,'127.0.0.1');s.setNoDelay(true);await new Promise(r=>s.on('connect',r));
let resolve;let buf='';s.on('data',d=>{buf+=d;if(buf.endsWith('\n')){buf='';resolve()}});
const call=line=>new Promise(r=>{resolve=r;s.write(line+'\n')});
const lat=[];
for(let i=0;i<200;i++)await call('P');
for(let i=0;i<N;i++){const t=performance.now();await call('P');lat.push((performance.now()-t)*1000)}
lat.sort((a,b)=>a-b);const q=x=>lat[Math.floor(x*lat.length)];
console.log(JSON.stringify({n:N,median_us:+q(.5).toFixed(1),p95_us:+q(.95).toFixed(1),p99_us:+q(.99).toFixed(1)}));
// a small fold + a view
const fr='F {"t":"changes","items":[{"$":"SettingSet","key":"k","value":"v","at":1}]}';const f=[];
for(let i=0;i<500;i++){const t=performance.now();await call(fr);f.push((performance.now()-t)*1000)}f.sort((a,b)=>a-b);
console.log(JSON.stringify({fold_one_event:{n:500,median_us:+f[250].toFixed(1),p95_us:+f[475].toFixed(1)}}));
s.end();p.kill();
