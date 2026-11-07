const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// M4 churn: reconnect, open/leave threads, create/delete threads, on a hub with no streaming agents. bun m4_churn.mjs MINUTES PORT
import {execSync} from 'node:child_process';import {mkdirSync,writeFileSync,appendFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,rssKb,fds,sockets,threads,tree,dirSizes} from './lib.mjs';import {viewClient} from './view.mjs';
const REPO=process.env.PERF_REPO||BP_ROOT;const mins=Number(process.argv[2]||20),port=Number(process.argv[3]||3806);
const root=REPO+'/build/gpui/perf-run/m4-churn';rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');execSync('git init -q',{cwd:root+'/proj'});
const out=process.env.PERF_OUT||REPO+'/build/gpui/results/perf/m4-churn';mkdirSync(out,{recursive:true});writeFileSync(out+'/samples.jsonl','');
const h=await startHub({bin:process.env.HUB_BIN||REPO+'/build/gpui/backplane-serve',home:root+'/home',port,standins:root+'/bin',log:out+'/hub.log'});
const c=await client(h);c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
for(let i=0;i<20;i++)c.send('thread.create',{project:pc.id,title:'churn '+i,env:'local'});
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=20?t:undefined});
for(const t of ths)c.send('turn.start',{thread:t.id,text:'zqshort seed history'});await sleep(20000); // seeded log: 20 threads with a finished short turn each
let ops={reconnect:0,open:0,create:0,del:0,err:0};const t0=Date.now();let stop=false;
const sampler=(async()=>{while(!stop){appendFileSync(out+'/samples.jsonl',JSON.stringify({t:Math.round((Date.now()-t0)/1000),rss:rssKb(h.pid),fds:fds(h.pid),sockets:sockets(h.pid),thr:threads(h.pid),kids:tree(h.pid).length,...dirSizes(h.home),...ops})+'\n');await sleep(10000)}})();
const worker1=(async()=>{while(Date.now()-t0<mins*60000){try{const v=await viewClient(h,{mobile:true,delta:true});await v.open();for(let k=0;k<4;k++){v.target={kind:'thread',id:ths[Math.floor(Math.random()*20)].id};v.viewId='';await v.req('view.open');ops.open++}v.close();ops.reconnect++}catch{ops.err++}await sleep(200)}})();
const worker2=(async()=>{const cc=await client(h);while(Date.now()-t0<mins*60000){try{const r=await cc.rpc('thread.create',{project:pc.id,title:'tmp',env:'local'},10000);ops.create++;const id=r?.o?.thread;if(id){const d=await cc.rpc('thread.delete',{thread:id},10000);if(d?.o?.ok)ops.del++;else ops.err++}}catch{ops.err++}await sleep(300)}cc.ws.close()})();
const worker3=(async()=>{while(Date.now()-t0<mins*60000){try{const p=await client(h);await sleep(300);p.ws.close();ops.reconnect++}catch{ops.err++}await sleep(100)}})();
await Promise.all([worker1,worker2,worker3]);stop=true;await sleep(11000);
writeFileSync(out+'/done.json',JSON.stringify({ops,mins,final:{rssMB:rssKb(h.pid)/1024,fds:fds(h.pid),sockets:sockets(h.pid),kids:tree(h.pid).length}}));
console.log(JSON.stringify(ops));stopHub(h);await sleep(1500);process.exit(0);
