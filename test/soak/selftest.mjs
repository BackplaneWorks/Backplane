// bun selftest.mjs : the measuring client must be event driven. A local echo hub answers each rpc after a fixed delay; the
// client's timing must track that delay to about a millisecond. A polling client (perf/lib.mjs until(), sleep(20)) reads 20+ ms.
import {readFileSync} from 'node:fs';
import {client,pct,sleep} from './lib.mjs';
import {encode,decode} from '../wire.mjs';
const fails=[];const ok=(c,m)=>{if(!c){fails.push(m);console.log('FAIL',m)}else console.log('ok  ',m)};
let delay=0;
const srv=Bun.serve({port:0,hostname:'127.0.0.1',fetch(req,s){if(s.upgrade(req))return;return new Response('x')},websocket:{
 open(ws){ws.send(encode({t:'view',version:1,generation:'1',epoch:'e'}))},
 message(ws,m){const x=decode(m);const send=()=>ws.send(encode({t:'reply',id:x.id,ok:true}));delay?setTimeout(send,delay):send()}}});
const h={host:'127.0.0.1',port:srv.port,token:'x'};
for(const d of[0,7,13]){delay=d;const c=await client(h);const xs=[];for(let i=0;i<300;i++){const r=await c.rpc('x',{});xs.push(r.ms)}c.close();
 const p50=pct(xs,50),p95=pct(xs,95);console.log(`delay ${d} ms: p50 ${p50.toFixed(2)} p95 ${p95.toFixed(2)} max ${Math.max(...xs).toFixed(2)}`);
 ok(p50>=d-0.2&&p50<d+3,`delay ${d}: p50 ${p50.toFixed(2)} tracks the server delay`);
 ok(p95<d+8,`delay ${d}: p95 ${p95.toFixed(2)} shows no 20 ms quantisation`)}
// 20 concurrent rpcs in flight resolve independently
delay=5;{const c=await client(h);const rs=await Promise.all([...Array(20)].map(()=>c.rpc('x',{})));ok(rs.every(r=>r&&r.ms<15),'20 concurrent rpcs each resolve at their own reply');c.close()}
// timeouts return undefined
{const c=await client(h);delay=400;const r=await c.rpc('x',{},50);ok(r===undefined,'rpc timeout resolves undefined');c.close()}
// static guard: the client body must not poll
const src=readFileSync(new URL('./lib.mjs',import.meta.url),'utf8');const body=src.slice(src.indexOf('export async function client'),src.indexOf('// Plain GET'));
ok(!/sleep\(|setInterval|\.until\(/.test(body),'client() has no sleep/setInterval/until loop');
srv.stop(true);if(fails.length){console.log(fails.length,'failed');process.exit(1)}console.log('selftest passed');process.exit(0);
