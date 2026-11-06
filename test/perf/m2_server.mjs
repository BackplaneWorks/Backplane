// bun m2_server.mjs HOST PORT AGENTS OUTNAME   (AGENTS=0 idle). Runs until <root>/stop exists.
import {mkdirSync,writeFileSync,existsSync,appendFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,rssKb,cpuTicks,fds} from './lib.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';
const [host,port,agents,name]=[process.argv[2],Number(process.argv[3]),Number(process.argv[4]),process.argv[5]];
const root=`${REPO}/build/gpui/perf-run/m2-${name}`;rmSync(root,{recursive:true,force:true});mkdirSync(root,{recursive:true});makeStandins(root+'/bin');
const out=`${REPO}/build/gpui/results/perf/m2-${name}`;mkdirSync(out,{recursive:true});
const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:root+'/home',port,host,standins:root+'/bin',extraEnv:{FAKE_SECS:'3000',FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'});
const c=await client({...h,host:'127.0.0.1'}).catch(async()=>client(h));
mkdirSync(root+'/proj',{recursive:true});c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
const n=Math.max(agents,Number(process.env.THREADS||0),3);
for(let i=0;i<n;i++)c.send('thread.create',{project:pc.id,title:'perf '+i,env:'local'});
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=n?t:undefined});
// seed some history in thread 0 and 1 (a short stand-in turn each) so views have content
for(let i=0;i<agents;i++)c.send('turn.start',{thread:ths[i].id,text:'stream'});
await sleep(1000);
writeFileSync(`${root}/state.json`,JSON.stringify({url:h.url,port,token:h.token,threads:ths.map(t=>t.id),project:pc.id,pid:h.pid,home:h.home,agents}));
console.log('ready',h.url,'pid',h.pid);
const t0=Date.now();
while(!existsSync(root+'/stop')){appendFileSync(out+'/hub-samples.jsonl',JSON.stringify({t:(Date.now()-t0)/1000,rss:rssKb(h.pid),cpu:cpuTicks(h.pid),fds:fds(h.pid)})+'\n');await sleep(2000)}
stopHub(h);await sleep(1500);process.exit(0);
