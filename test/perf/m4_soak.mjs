const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// Milestone 4: hub soak. bun m4_soak.mjs N HOURS port
import {mkdirSync,writeFileSync,appendFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,rssKb,cpuTicks,fds,sockets,threads,tree,treeRssKb,dirSizes,debugPerf} from './lib.mjs';
const REPO=BP_ROOT;
const n=Number(process.argv[2]||20),hours=Number(process.argv[3]||2.1),port=Number(process.argv[4]||3802);
const root=`${REPO}/build/gpui/perf-run/m4-soak`;rmSync(root,{recursive:true,force:true});mkdirSync(root,{recursive:true});
const out=`${REPO}/build/gpui/results/perf/m4-hub-soak`;mkdirSync(out,{recursive:true});
makeStandins(root+'/bin');
const dur=hours*3600;
const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:root+'/home',port,standins:root+'/bin',extraEnv:{FAKE_SECS:String(dur+600),FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'});
const lagw=[];const c=await client(h,{onMsg:(o,now)=>{if(o.t==='delta'){const m=/@(\d{13})@/.exec(o.text??'');if(m)lagw.push(now-Number(m[1]))}}});
mkdirSync(root+'/proj',{recursive:true});
c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
for(let i=0;i<n;i++)c.send('thread.create',{project:pc.id,title:'soak '+i,env:'local'});
const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated');return t.length>=n?t:undefined});
for(const t of ths)c.send('turn.start',{thread:t.id,text:'stream'});
c.seen.length=0; // do not keep the log in the client
const t0=Date.now();let tick=0;
writeFileSync(out+'/samples.jsonl','');
while(Date.now()-t0<dur*1000){
 const cp=cpuTicks(h.pid);const l=lagw.splice(0);l.sort((a,b)=>a-b);
 const s={t:Math.round((Date.now()-t0)/1000),rss:rssKb(h.pid),treeRss:treeRssKb(h.pid),fds:fds(h.pid),sockets:sockets(h.pid),thr:threads(h.pid),kids:tree(h.pid).length,cpuTicks:cp,lagP50:l[l.length>>1],lagMax:l[l.length-1],nd:l.length,...dirSizes(h.home)};
 const p=await c.rpc('load.ping',{},5000);s.rtt=p?.ms;
 appendFileSync(out+'/samples.jsonl',JSON.stringify(s)+'\n');
 if(tick%20===0){try{writeFileSync(`${out}/debug-perf-${String(tick).padStart(4,'0')}.txt`,await debugPerf(h))}catch{}}
 c.seen.length=0;tick++;
 await sleep(30000-(Date.now()-t0)%30000);
}
writeFileSync(out+'/debug-perf-final.txt',await debugPerf(h));
writeFileSync(out+'/done','1');
stopHub(h);await sleep(1500);process.exit(0);
