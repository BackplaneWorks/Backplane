// seed.mjs PORT TOKENFILE PROJECT: one project, a thread, two bots (Kit, Miso).
import {client,sleep} from '../perf/lib.mjs';
import {readFileSync} from 'node:fs';
const [port,tf,path]=process.argv.slice(2);
const h={host:'127.0.0.1',port:+port,url:`http://127.0.0.1:${port}`,token:readFileSync(tf,'utf8').trim()};
const c=await client(h);
const call=async(m,p)=>(await c.rpc(m,p,10000))?.o;
const pr=await call('project.add',{path});await sleep(1000);
const project=c.seen.find(x=>x.$==='ProjectCreated')?.id||pr?.project;
await call('thread.create',{project,title:'alpha thread',env:'local'});
await call('bots.create',{name:'Kit',persona:'cat',provider:'claude'});await call('bots.create',{name:'Miso',persona:'cat2',provider:'claude'});
await call('setting.set',{key:'onboard.done',value:'1'});await sleep(500);process.exit(0)
