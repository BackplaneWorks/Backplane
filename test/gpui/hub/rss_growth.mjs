// bun rss_growth.mjs HOME PORT HUBPID WRITES : durable-write ack latency and the hub's RSS slope per committed event.
import {readFileSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
const [home,port,pid,writes]=process.argv.slice(2);const N=Number(writes);
const token=readFileSync(home+'/token','utf8').trim();
const rss=()=>Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${pid}/status`,'utf8'))[1]);
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);ws.binaryType='arraybuffer';
const waiting=new Map();let greeted;
ws.onmessage=({data})=>{const x=decode(data);if(x.t==='view')greeted=true;if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}};
await new Promise(r=>ws.onopen=r);for(let i=0;i<200&&!greeted;i++)await new Promise(r=>setTimeout(r,10));
const lat=[],marks=[{writes:0,rss_kb:rss()}];
for(let i=1;i<=N;i++){
 const id=100+i,t=performance.now();
 const p=new Promise(r=>waiting.set(id,r));
 ws.send(encode({id,m:'turn.start',p:{thread:'t'+(i%50),text:'a message of some length, '.repeat(3),msg:'g'+i}}));
 const x=await p;if(!x.ok)throw Error('refused '+JSON.stringify(x));lat.push(performance.now()-t);
 if(i%Math.floor(N/4)===0)marks.push({writes:i,rss_kb:rss()});
}
lat.sort((a,b)=>a-b);const q=f=>lat[Math.min(lat.length-1,Math.floor(f*lat.length))];
const slope=(marks.at(-1).rss_kb-marks[0].rss_kb)*1024/(marks.at(-1).writes*2); // two events (MessagePosted, TurnQueued) per write
console.log(JSON.stringify({writes:N,events:N*2,ack_median_ms:+q(.5).toFixed(3),ack_p95_ms:+q(.95).toFixed(3),ack_p99_ms:+q(.99).toFixed(3),marks,bytes_per_event_rss:+slope.toFixed(1)}));
ws.close();process.exit(0);
