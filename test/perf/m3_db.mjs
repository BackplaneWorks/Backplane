const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// M3: bun m3_db.mjs EVENTS_JSONL LABEL PORT   (hub store at scale; synthetic log only)
import {mkdirSync,rmSync,copyFileSync,writeFileSync,readFileSync,existsSync} from 'node:fs';
import {spawn} from 'node:child_process';
import {makeStandins,startHub,stopHub,client,sleep,pct,rssKb,hwmKb,cpuTicks,dirSizes,fds} from './lib.mjs';
import {viewClient} from './view.mjs';
const REPO=BP_ROOT;
const [src,label,portS]=process.argv.slice(2);const port=Number(portS||3808);
const root=`${REPO}/build/gpui/perf-run/m3`;rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});makeStandins(root+'/bin');
const out=`${REPO}/build/gpui/results/perf/m3-${label}`;mkdirSync(out,{recursive:true});
copyFileSync(src,root+'/home/events.jsonl');
const R={label,machine:'desktop, nvme, linux 7.0, page cache warm',events:0,binary:'build/gpui/backplane-serve 0.14.0'};
R.events=Number(process.env.EVENTS||0)||undefined;
function save(){writeFileSync(`${out}/result.json`,JSON.stringify(R,null,1))}
const stat=a=>({n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:Math.max(...a)});
async function boot(tag){
 const t0=performance.now();const cpu0=0;
 const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:root+'/home',port,standins:root+'/bin',extraEnv:{},cap:'3700M',log:out+`/hub-${tag}.log`});
 const tHello=performance.now()-t0;
 const v=await viewClient(h,{mobile:true,delta:true});const tg=performance.now()-t0;
 const r=await v.open();const tView=performance.now()-t0;
 return {h,v,boot:{helloMs:tHello,greetingMs:tg,firstViewMs:tView,firstViewBytes:r?.bytes,rssMB:rssKb(h.pid)/1024,hwmMB:hwmKb(h.pid)/1024,files:dirSizes(h.home)}};
}
// 1: cold start from log only (no store.db): the import
let b=await boot('import');R.import=b.boot;save();b.v.close();
await sleep(5000);R.afterImportIdle={rssMB:rssKb(b.h.pid)/1024,files:dirSizes(b.h.home)};
stopHub(b.h);await sleep(3000);R.afterStopFiles=dirSizes(root+'/home');
// 2,3: restart with store.db (warm FS cache), twice
for(const k of ['restart1','restart2']){b=await boot(k);R[k]=b.boot;if(k==='restart1')break;b.v.close();stopHub(b.h);await sleep(3000)}
const {h,v}=b;
// threads
const N=Number(process.env.EVENTS||0);const nth=Math.max(5,Math.floor(N/150));
let c,ths;
if(N>=500000){ // a full-log join of this size is dropped by the hub (see hub log): join near the end instead
 c=await client(h,{since:N-2000});ths=Array.from({length:nth},(_,i)=>`t1785000000-${1000+i}`);
}else{c=await client(h);await sleep(500);ths=c.seen.filter(x=>x.$==='ThreadCreated').map(x=>x.id);}
save();
const th0=ths[0];
// 4 thread switch: view.open on random threads
const sw=[],swb=[];
for(let i=0;i<60;i++){const t=ths[Math.floor(Math.random()*ths.length)];v.target={kind:'thread',id:t};v.viewId='';const r=await v.req('view.open',{},180000);if(!r){R.threadSwitchTimeouts=(R.threadSwitchTimeouts||0)+1;continue}sw.push(r.ms);swb.push(r.bytes);await sleep(30)}
R.threadSwitch=sw.length?{ms:stat(sw),bytes:stat(swb)}:null;save();
// long thread: open + page older x20
v.target={kind:'thread',id:th0};v.viewId='';let r=await v.req('view.open',{},180000);R.longThreadOpen={ms:r.ms,bytes:r.bytes,window:r.x.result.window&&{older:!!r.x.result.window.older,total:r.x.result.window.total,remaining:r.x.result.window.remaining}};
const pg=[],pgb=[];let cursor=r.x.result.window?.older;
for(let i=0;i<30&&cursor;i++){const q=await v.req('view.page',{cursor,anchor:{id:'',offset:0}});if(!q?.x?.ok){R.pageErr=JSON.stringify(q?.x?.error||q?.x).slice(0,200);break}pg.push(q.ms);pgb.push(q.bytes);cursor=q.x.result.window?.older}
R.pageOlder=pg.length?{ms:stat(pg),bytes:stat(pgb)}:null;
// 5 write latency: thread.rename acks, sequential, then a fast burst sampling WAL
const wr=[];for(let i=0;i<300;i++){const q=await c.rpc('thread.rename',{thread:th0,title:'w'+i},10000);wr.push(q?q.ms:NaN);}
R.writeRename=stat(wr.filter(Number.isFinite));R.writeRenameErr=wr.filter(x=>!Number.isFinite(x)).length;
R.writeSample=JSON.stringify((await c.rpc('thread.rename',{thread:th0,title:'x'}))?.o).slice(0,200);
save();const w0=dirSizes(h.home);const lat=[];const walSeries=[];const t0=performance.now();let nextS=0;
for(let i=0;i<4000;i++){const q=await c.rpc('thread.rename',{thread:th0,title:'b'+i},10000);lat.push(q?q.ms:NaN);if(performance.now()-t0>=nextS){nextS+=500;walSeries.push({t:(performance.now()-t0)/1000,...dirSizes(h.home)})}}
R.writeBurst={...stat(lat.filter(Number.isFinite)),perSec:4000/((performance.now()-t0)/1000),over50ms:lat.filter(x=>x>50).length,over200ms:lat.filter(x=>x>200).length};
R.walSeries=walSeries;R.filesBefore=w0;R.filesAfter=dirSizes(h.home);
// ping while idle at size
const pg2=[];for(let i=0;i<50;i++){pg2.push((await c.rpc('load.ping',{})).ms);await sleep(20)}R.ping=stat(pg2);
R.rssEndMB=rssKb(h.pid)/1024;R.hwmMB=hwmKb(h.pid)/1024;
writeFileSync(`${out}/result.json`,JSON.stringify(R,null,1));console.log(JSON.stringify({...R,walSeries:undefined},null,1));
stopHub(h);await sleep(2000);process.exit(0);
