// Milestone 1: hub under load. node/bun m1_load.mjs N [secs] [outdir]
import {mkdirSync,writeFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,pct,f1,rssKb,hwmKb,cpuTicks,fds,sockets,threads,tree,treeRssKb,dirSizes,debugPerf} from './lib.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';
const n=Number(process.argv[2]||20),secs=Number(process.argv[3]||60),port=Number(process.argv[4]||3801);
const warm=10;
const bin=process.env.HUB_BIN||REPO+'/build/gpui/backplane-serve';
const root=`${REPO}/build/gpui/perf-run/m1-${n}`;rmSync(root,{recursive:true,force:true});mkdirSync(root,{recursive:true});
const out=`${REPO}/build/gpui/results/perf/m1-${n}agents`;mkdirSync(out,{recursive:true});
makeStandins(root+'/bin');
const total=secs+warm+30;
const h=await startHub({bin,home:root+'/home',port,standins:root+'/bin',extraEnv:{FAKE_SECS:String(total),FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'});
const idleRss=rssKb(h.pid);
const per=new Map(); // thread -> {t:[ms...]}
const lag=[];let counting=false,got=0;
const c=await client(h,{onMsg:(o,now)=>{if(o.t!=='delta')return;if(!counting)return;got++;(per.get(o.thread)??per.set(o.thread,[]).get(o.thread)).push(now);const m=/@(\d{13})@/.exec(o.text??'');if(m)lag.push(now-Number(m[1]))}});
mkdirSync(root+'/proj',{recursive:true});
c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
for(let i=0;i<n;i++)c.send('thread.create',{project:pc.id,title:'load '+i,env:'local'});
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=n?t:undefined});
const ping=async()=>(await c.rpc('load.ping',{}))?.ms;
for(let i=0;i<5;i++)await ping();
for(const t of ths)c.send('turn.start',{thread:t.id,text:'stream for the load test'});
await sleep(warm*1000);
const idleAfterWarm={rss:rssKb(h.pid)};
counting=true;c.bytes=0;c.msgs=0;
const t0=Date.now(),cpu0=cpuTicks(h.pid);
const rtt=[],samples=[];
let nextS=0;
while(Date.now()-t0<secs*1000){rtt.push(await ping());
 if(Date.now()-t0>=nextS){nextS+=5000;samples.push({t:(Date.now()-t0)/1000,rss:rssKb(h.pid),fds:fds(h.pid),sockets:sockets(h.pid),thr:threads(h.pid),kids:tree(h.pid).length,treeRss:treeRssKb(h.pid),...dirSizes(h.home)})}
 await sleep(200)}
const wall=Date.now()-t0,cpu1=cpuTicks(h.pid);
const msgs=c.msgs,bytes=c.bytes;
counting=false;
// validity: per thread rate and longest gap
const rows=[];let valid=ths.every(t=>per.has(t.id));
for(const t of ths){const a=per.get(t.id)??[];let gap=0;for(let i=1;i<a.length;i++)gap=Math.max(gap,a[i]-a[i-1]);if(a.length)gap=Math.max(gap,a[0]-t0,t0+wall-a[a.length-1]);else gap=wall;
 const rate=a.length/(wall/1000);rows.push({thread:t.id,deltas:a.length,rate,maxGapMs:gap});if(rate<1||gap>2000)valid=false}
const perf=await debugPerf(h);writeFileSync(out+'/debug-perf.txt',perf);
const res={n,secs:wall/1000,warm,machine:'desktop (32 cores, 59 GiB), linux 7.0',binary:bin,samples:rows.length,
 hubStartRss_MB:idleRss/1024,rssAfterWarm_MB:idleAfterWarm.rss/1024,rssEnd_MB:rssKb(h.pid)/1024,rssHwm_MB:hwmKb(h.pid)/1024,
 cpu_core_pct:((cpu1-cpu0)*10)/wall*100,
 rtt:{n:rtt.length,p50:pct(rtt,50),p95:pct(rtt,95),p99:pct(rtt,99),max:Math.max(...rtt)},
 lag:{n:lag.length,p50:pct(lag,50),p95:pct(lag,95),p99:pct(lag,99),max:Math.max(...lag)},
 deltasReceived:got,deltasPerSec:got/(wall/1000),clientMsgsPerSec:msgs/(wall/1000),clientKBps:bytes/1024/(wall/1000),
 valid,validity:{minRate:Math.min(...rows.map(r=>r.rate)),maxGapMs:Math.max(...rows.map(r=>r.maxGapMs)),streamsSeen:per.size},
 files:dirSizes(h.home),samples:samples};
writeFileSync(out+'/result.json',JSON.stringify({...res,perThread:rows},null,1));
console.log(JSON.stringify({...res,samples:undefined,files:res.files},null,1));
stopHub(h);await sleep(1500);process.exit(0);
