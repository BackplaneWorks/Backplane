const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// Comparison hub: JS hub, stand-in agents only, 20 threads with short seeded history. bun cmp_hub.mjs PORT   (runs until <root>/stop)
import {mkdirSync,writeFileSync,existsSync,rmSync,appendFileSync} from 'node:fs';
import {execSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client,sleep,rssKb,cpuTicks,fds} from './lib.mjs';
const REPO=BP_ROOT;const port=Number(process.argv[2]||3807);
const root=REPO+'/build/gpui/perf-run/cmp';rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
execSync('git init -q',{cwd:root+'/proj'});
const out=REPO+'/build/gpui/results/perf/cmp';mkdirSync(out,{recursive:true});
const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:root+'/home',port,host:'127.0.0.1',standins:root+'/bin',extraEnv:{FAKE_SECS:'3000',FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'});
const c=await client(h);c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
const N=20;for(let i=0;i<N;i++)c.send('thread.create',{project:pc.id,title:'cmp thread '+i,env:'local'});
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated'&&x.project===pc.id);return t.length>=N?t:undefined});
for(const t of ths)c.send('turn.start',{thread:t.id,text:'zqshort seed history'});
await sleep(20000); // 8 s turns finish
writeFileSync(root+'/state.json',JSON.stringify({port,token:h.token,threads:ths.map(t=>t.id),project:pc.id,pid:h.pid,home:h.home,url:h.url}));
console.log('ready',h.url,'pid',h.pid);
globalThis.startLoad=()=>{for(const t of ths)c.send('turn.start',{thread:t.id,text:'stream for the comparison'})};
const t0=Date.now();let loaded=false;
while(!existsSync(root+'/stop')){ if(!loaded&&existsSync(root+'/load')){loaded=true;startLoad()}
 appendFileSync(out+'/hub-samples.jsonl',JSON.stringify({t:(Date.now()-t0)/1000,rss:rssKb(h.pid),cpu:cpuTicks(h.pid),loaded})+'\n');await sleep(2000)}
stopHub(h);await sleep(1500);process.exit(0);
