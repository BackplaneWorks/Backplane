// bun rpc_floor.mjs HOME PORT : /hello over HTTP, load.ping over the WebSocket at several gaps (the JS hub:
// 0.95 ms and a flat 20.3-20.5 ms whatever the gap), and the durable write ack (turn.start).
import {readFileSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
const [home,port]=process.argv.slice(2);const token=readFileSync(home+'/token','utf8').trim();
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const pct=(a,p)=>{const s=[...a].sort((x,y)=>x-y);return +s[Math.min(s.length-1,Math.floor(p/100*s.length))].toFixed(3)};
const st=a=>({n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:+Math.max(...a).toFixed(3)});
const R={};
const hello=[];for(let i=0;i<200;i++){const t=performance.now();const r=await fetch(`http://127.0.0.1:${port}/hello`);await r.arrayBuffer();hello.push(performance.now()-t);if(i<20)hello.length=0}
R.hello_http=st(hello);
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';
const waiting=new Map();ws.onmessage=({data})=>{const x=decode(data);if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}};
await new Promise(r=>ws.onopen=r);await sleep(300);
let n=1000;const rpc=async(m,p)=>{const id=++n;const pr=new Promise(r=>waiting.set(id,r));const t=performance.now();ws.send(encode({id,m,p}));const x=await pr;return [performance.now()-t,x]};
for(let i=0;i<30;i++)await rpc('load.ping',{});
R.ping={};for(const gap of [0,3,30,200]){const a=[];const reps=gap>=30?60:300;for(let i=0;i<reps;i++){a.push((await rpc('load.ping',{}))[0]);if(gap)await sleep(gap)}R.ping['gap'+gap]=st(a)}
const ack=[];for(let i=0;i<1000;i++){const [ms,x]=await rpc('turn.start',{thread:'t'+(i%50),text:'hello',msg:'f'+i});if(!x.ok)throw Error(JSON.stringify(x));ack.push(ms)}
R.write_ack=st(ack);
// 20 clients writing at once: group commit shares fsyncs
const conc=await Promise.all(Array.from({length:20},async(_,c)=>{const w=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);w.binaryType='arraybuffer';const wt=new Map();w.onmessage=({data})=>{const x=decode(data);if(x.t==='reply'&&wt.has(x.id)){wt.get(x.id)(x);wt.delete(x.id)}};await new Promise(r=>w.onopen=r);await sleep(200);
 const lat=[];for(let i=0;i<50;i++){const id=1+i;const pr=new Promise(r=>wt.set(id,r));const t=performance.now();w.send(encode({id,m:'turn.start',p:{thread:'t'+c,text:'x',msg:`c${c}-${i}`}}));await pr;lat.push(performance.now()-t)}w.close();return lat}));
R.write_ack_20_writers=st(conc.flat());
console.log(JSON.stringify(R));ws.close();process.exit(0);
