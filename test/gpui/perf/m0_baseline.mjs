// M0 baselines of the JS hub on the one store copy (~/.backplane-gpui-copy). Content is never printed or stored: sizes and times only.
// bun m0_baseline.mjs [phase] ; phases run in order: first, later, steady, load, click
import {mkdirSync,writeFileSync,readFileSync,existsSync,appendFileSync,readdirSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,pct,rssKb,hwmKb,cpuTicks,fds,threads,tree,treeRssKb,dirSizes} from './lib.mjs';
import {viewClient} from './view.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';const HOME=process.env.HOME+'/.backplane-gpui-copy';const PORT=3830,HOST='100.64.84.41';
const root=REPO+'/build/gpui/perf-run/m0';mkdirSync(root+'/fakehome',{recursive:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
const out=REPO+'/build/gpui/results/perf/m0';mkdirSync(out,{recursive:true});
const stat=a=>({n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:Math.max(...a)});
const R=existsSync(out+'/m0.json')?JSON.parse(readFileSync(out+'/m0.json','utf8')):{machine:'desktop 32 cores, 59 GiB, linux 7.0',hub:'build/gpui/backplane-serve 0.14.0 (JS on Bun)',store:'~/.backplane-gpui-copy',copyEnv:'BACKPLANE_COPY=1, fake HOME, stand-in agents'};
const save=()=>writeFileSync(out+'/m0.json',JSON.stringify(R,null,1));
// live hub reference, /proc only
function live(){const p=Number(readFileSync(process.env.HOME+'/.backplane-bend/hub.lock','utf8').split(' ')[0]);return {pid:p,rssMB:rssKb(p)/1024,cpuTicks:cpuTicks(p),fds:fds(p),threads:threads(p)}}
async function liveRef(secs){const a=live();await sleep(secs*1000);const b=live();return {rssMB:b.rssMB,cpuCorePct:(b.cpuTicks-a.cpuTicks)*10/(secs*1000)*100,fds:b.fds,threads:b.threads,secs}}
async function boot(tag,extra={}){
 const t0=performance.now();
 const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:HOME,port:PORT,host:HOST,standins:root+'/bin',cap:'12G',log:out+`/hub-${tag}.log`,extraEnv:{BACKPLANE_COPY:'1',HOME:root+'/fakehome',FAKE_SECS:'1200',FAKE_PER:'2',FAKE_CYCLE:'20',...extra}});
 const tHello=performance.now()-t0;
 const v=await viewClient(h,{mobile:true,delta:true});const tg=performance.now()-t0;
 const r=await v.open();const tv=performance.now()-t0;
 const peak=hwmKb(h.pid)/1024;
 return {h,v,b:{helloS:tHello/1000,greetingS:tg/1000,firstViewS:tv/1000,firstViewBytes:r?.bytes,rssMBAtUsable:rssKb(h.pid)/1024,hwmMB:peak,files:dirSizes(HOME),fds:fds(h.pid)}};
}
const phase=process.argv[2]||'all';
const run=(n)=>phase==='all'||phase===n;
let cur=null;
async function stopWait(h){stopHub(h);for(let i=0;i<300;i++){if(!existsSync('/proc/'+h.pid))break;await sleep(200)}await sleep(500)}
if(run('first')){
 if(existsSync(HOME+'/store.db')&&!process.env.ALLOW_RERUN_FIRST)throw Error('store.db exists: first start already done');
 R.liveReference=await liveRef(30);save();
 const rs=[];const t0=Date.now();
 const sampler=setInterval(()=>{},1000);
 cur=await boot('first');clearInterval(sampler);
 R.firstStart=cur.b;R.firstStart.note='import of events.jsonl into store.db (no store.db before)';save();
 await sleep(30000);R.firstStart.rssMBAfter30sIdle=rssKb(cur.h.pid)/1024;R.firstStart.hwmMBAfter30s=hwmKb(cur.h.pid)/1024;save();
 cur.v.close();await stopWait(cur.h);
}
if(run('later')){
 R.laterStarts=[];
 for(let i=0;i<3;i++){const c=await boot('later'+i);const b=c.b;await sleep(30000);b.rssMBAfter30sIdle=rssKb(c.h.pid)/1024;b.hwmMBAfter30s=hwmKb(c.h.pid)/1024;R.laterStarts.push(b);save();c.v.close();await stopWait(c.h)}
}
if(run('steady')||run('load')||run('click')){
 cur=await boot('main');R.mainStart=cur.b;save();
 const {h}=cur;
 if(run('steady')){await sleep(60000);const a=cpuTicks(h.pid);await sleep(60000);R.steadyIdle={rssMB:rssKb(h.pid)/1024,cpuCorePct:(cpuTicks(h.pid)-a)*10/60000*100,fds:fds(h.pid),threads:threads(h.pid),kids:tree(h.pid).length,hwmMB:hwmKb(h.pid)/1024};save()}
 // plain client joined near the end of the log (a full join of this log is dropped by the hub)
 const hl=readFileSync(out+'/hub-main.log','utf8');const cnt=Number(/log count (\d+)/.exec(hl)?.[1]);const origin=/hub: origin (\S+)/.exec(hl)?.[1];
 const per=new Map(),lag=[];let counting=false,got=0;
 const c=await client(h,{since:cnt,origin,onMsg:(o,now)=>{if(o.t!=='delta'||!counting)return;got++;(per.get(o.thread)??per.set(o.thread,[]).get(o.thread)).push(now);const m=/@(\d{13})@/.exec(o.text??'');if(m)lag.push(now-Number(m[1]))}});await sleep(500);
 c.send('project.add',{path:root+'/proj'});const pc=await c.until(20000,()=>c.seen.find(x=>x.$==='ProjectCreated'&&x.root?.includes('perf-run/m0')));
 if(!pc)throw Error('no project');
 const N=20;for(let i=0;i<N;i++)c.send('thread.create',{project:pc.id,title:'m0 sim '+i,env:'local'});
 const ths=await c.until(30000,()=>{const t=c.seen.filter(x=>x.$==='ThreadCreated'&&x.project===pc.id);return t.length>=N?t:undefined});
 if(!ths)throw Error('threads');
 // click latency, idle: sidebar click (view.action select), thread switch (view.open), composer popup (draft "/" then update)
 const v=cur.v;
 // real thread ids from the sidebar body, held in memory only
 const body=JSON.stringify(v.last?.result?.shown||{});const ids=[...new Set(body.match(/t\d{9,}-\d+/g)||[])];
 R.sidebarThreadIdsSeen=ids.length;
 const pick=()=>ids.length?ids[Math.floor(Math.random()*ids.length)]:ths[0].id;
 async function clicks(tag){const r={};
  const a=[],ab=[];for(let i=0;i<40;i++){v.target={kind:'thread',id:ths[i%N].id};v.viewId='';const x=await v.req('view.open',{},120000);if(x){a.push(x.ms);ab.push(x.bytes)}await sleep(100)}
  r.switchOwnThread={ms:stat(a),bytes:stat(ab)};
  const b=[],bb=[];for(let i=0;i<40;i++){v.target={kind:'thread',id:pick()};v.viewId='';const x=await v.req('view.open',{},120000);if(x){b.push(x.ms);bb.push(x.bytes)}await sleep(100)}
  r.switchSidebarThread={ms:stat(b),bytes:stat(bb)};
  const s=[],sb=[];v.target={kind:'thread',id:ths[0].id};v.viewId='';await v.req('view.open');
  for(let i=0;i<40;i++){const x=await v.req('view.action',{action:'select',value:ths[(i+1)%N].id},120000);if(x){s.push(x.ms);sb.push(x.bytes)}await sleep(100)}
  r.sidebarClick_select={ms:stat(s),bytes:stat(sb)};
  const p=[],pb=[];v.target={kind:'thread',id:ths[0].id};v.viewId='';await v.req('view.open');
  for(let i=0;i<40;i++){v.session.sel=ths[0].id;v.session.drafts[ths[0].id]=i%2?'':'/';const x=await v.req('view.update',{},120000);if(x){p.push(x.ms);pb.push(x.bytes)}await sleep(100)}
  v.session.drafts[ths[0].id]='';r.composerPopup_slashDraftUpdate={ms:stat(p),bytes:stat(pb)};
  return r}
 if(run('click')||run('load')){R.clickIdle=await clicks('idle');save()}
 if(run('load')){
  // 20 simulated agents
  for(const t of ths)c.send('turn.start',{thread:t.id,text:'simulated stream for the baseline'});
  await sleep(10000);
  const t0=Date.now(),cpu0=cpuTicks(h.pid);const rtt=[],ss=[];
  counting=true;const dt0=Date.now();
  const tick=setInterval(()=>ss.push({t:(Date.now()-t0)/1000,rss:rssKb(h.pid),fds:fds(h.pid),kids:tree(h.pid).length}),5000);
  const cl=clicks('load');
  const pingLoop=(async()=>{while(Date.now()-t0<90000){rtt.push((await c.rpc('load.ping',{},10000))?.ms);await sleep(200)}})();
  R.clickLoad=await cl;await pingLoop;clearInterval(tick);
  const wall=Date.now()-t0;
  R.load20={secs:wall/1000,cpuCorePct:(cpuTicks(h.pid)-cpu0)*10/wall*100,rssMBSamples:ss.map(x=>Math.round(x.rss/1024)),rssMBMax:Math.max(...ss.map(x=>x.rss))/1024,hwmMB:hwmKb(h.pid)/1024,kids:ss.at(-1)?.kids,fds:ss.at(-1)?.fds,ping:stat(rtt.filter(Number.isFinite))};
  // validity: deltas per stream (plain client was counting since turn start)
  counting=false;const rows=ths.map(t=>{const a=per.get(t.id)??[];let gap=0;for(let i=1;i<a.length;i++)gap=Math.max(gap,a[i]-a[i-1]);if(a.length)gap=Math.max(gap,a[0]-dt0);else gap=wall;return {rate:a.length/(wall/1000),gap}});
  R.load20.validity={streams:rows.filter(r=>r.rate>0).length,minRate:Math.min(...rows.map(r=>r.rate)),maxGapMs:Math.max(...rows.map(r=>r.gap)),valid:rows.every(r=>r.rate>=1&&r.gap<=2000)&&rows.length===N};
  R.load20.deltaLagMs=stat(lag);R.load20.deltasPerSec=got/(wall/1000);
  R.load20.note='20 simulated agents (stand-in), 90 s, view clicks measured during the same window';save();
 }
 v.close();c.ws.close();await stopWait(h);
}
save();console.log(JSON.stringify(R,null,1).slice(0,6000));process.exit(0);
