// One turn on a benchmark hub: bun turn.mjs WORK PROJECT_DIR TEXT [nowait]  -> {thread, state, ms}
import {readFileSync} from 'node:fs';
import {client} from '../perf/lib.mjs';
const [work,proj,text,nowait]=process.argv.slice(2);
const h=JSON.parse(readFileSync(`${work}/hub.json`,'utf8'));h.host='127.0.0.1';
const c=await client(h,{});
c.send('project.add',{path:proj});const pc=await c.until(15000,()=>c.seen.find(x=>x.$==='ProjectCreated'&&x.root===proj));
if(!pc){console.log(JSON.stringify({error:'no project',seen:c.seen.slice(-3)}));process.exit(1)}
await c.until(1500,()=>false); // the log's replay first: the thread and the turn's end below are this run's, never an earlier run's in the same folder
const n0=c.seen.length;c.send('thread.create',{project:pc.id,title:'bench',env:'local',...(process.env.KIPC_BENCH_PROVIDER?{provider:process.env.KIPC_BENCH_PROVIDER,model:process.env.KIPC_BENCH_THREAD_MODEL??''}:{})}); // KIPC_BENCH_PROVIDER=grok: a Grok thread (its model KIPC_BENCH_THREAD_MODEL)
const th=await c.until(15000,()=>c.seen.slice(n0).find(x=>x.$==='ThreadCreated'&&x.project===pc.id));
if(!th)throw Error('thread.create did not produce a thread');
const t0=Date.now();const started=await c.rpc('turn.start',{thread:th.id,text});
if(!started?.o.ok)throw Error(`turn.start failed: ${JSON.stringify(started?.o??'no reply')}`);
if(nowait){await c.until(3000,()=>false);console.log(JSON.stringify({thread:th.id}));process.exit(0)}
const end=await c.until(Number(process.env.TURN_MS??1500000),()=>c.seen.slice(n0).find(x=>x.$==='TurnChanged'&&x.thread===th.id&&x.state!=='running'));
console.log(JSON.stringify({thread:th.id,state:end?.state??'timeout',ms:Date.now()-t0}));process.exit(0);
