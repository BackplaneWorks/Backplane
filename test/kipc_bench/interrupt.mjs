// Interrupt a thread's turn on a benchmark hub: bun interrupt.mjs WORK THREAD  -> {ok, state}
import {readFileSync} from 'node:fs';
import {client} from '../perf/lib.mjs';
const [work,thread]=process.argv.slice(2);
const h=JSON.parse(readFileSync(`${work}/hub.json`,'utf8'));h.host='127.0.0.1';
const c=await client(h,{});
const r=await c.rpc('turn.interrupt',{thread},15000);
const end=await c.until(20000,()=>c.seen.findLast(x=>x.$==='TurnChanged'&&x.thread===thread&&x.state!=='running'));
console.log(JSON.stringify({ok:r?.o?.ok??r?.ok,state:end?.state??'still running'}));process.exit(0);
