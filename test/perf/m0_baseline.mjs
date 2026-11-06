// M0 baselines of the JS hub on the one store copy (~/.backplane-gpui-copy). Content is never printed or stored: sizes and times only.
// bun m0_baseline.mjs [phase] ; phases run in order: first, later, steady, load, click
import {mkdirSync,writeFileSync,readFileSync,existsSync,appendFileSync,readdirSync} from 'node:fs';
import {execSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client,sleep,pct,rssKb,hwmKb,cpuTicks,fds,threads,tree,treeRssKb,dirSizes} from './lib.mjs';
import {viewClient} from './view.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';
// The store copy ~/.backplane-gpui-copy is read-only and never written by a hub. Every run works on a throwaway copy under build/gpui/ (cp --reflink=auto), removed afterwards.
const SRC=process.env.SRC_HOME||(process.env.HOME+'/.backplane-gpui-copy');const HOME=process.env.M0_HOME||(REPO+'/build/gpui/perf-run/m0/home'); // M0_HOME: the throwaway copy (under build/gpui or ~/.backplane-gpui-scratch), removed on exit
import {rmSync,chmodSync,realpathSync} from 'node:fs';import {execFileSync} from 'node:child_process';import pathMod from 'node:path';
const SCRATCH=process.env.HOME+'/.backplane-gpui-scratch',BUILD=REPO+'/build/gpui';
const under=(p,d)=>{const r=pathMod.resolve(p),b=pathMod.resolve(d);return r===b||r.startsWith(b+pathMod.sep)};
const REAL=[process.env.HOME+'/.backplane-gpui-copy',process.env.HOME+'/.backplane-bend',process.env.HOME+'/.backplane-gpui',process.env.HOME+'/.local/share/backplane'];
const isRealLog=p=>REAL.some(d=>under(p,d))||/\/\.backplane-gpui-scratch\/(store-copy-backup|person-)/.test(pathMod.resolve(p));
// the throwaway copy is the ONLY thing this script chmods and deletes: strictly under the scratch folder or build/gpui, never the source, never a real home
function assertSafePaths(){
  const h=pathMod.resolve(HOME),sr=pathMod.resolve(SRC);
  if(!((under(h,SCRATCH)&&h!==pathMod.resolve(SCRATCH))||(under(h,BUILD)&&h!==pathMod.resolve(BUILD))))throw Error('refusing M0_HOME '+h+': must be strictly under '+SCRATCH+' or '+BUILD);
  if(under(h,sr)||under(sr,h))throw Error('refusing: M0_HOME and the source home overlap');
  if(REAL.some(d=>under(h,d)||under(d,h)))throw Error('refusing: M0_HOME touches a real home');
  if(under(sr,process.env.HOME+'/.backplane-bend')||under(sr,process.env.HOME+'/.local/share/backplane'))throw Error('refusing SRC_HOME: a live hub\'s home');
  // the hold: no Rust hub (any non-default HUB_BIN) on a copy of the real log until the coordinator lifts it (COMMON.md)
  if(process.env.HUB_BIN&&isRealLog(sr)&&process.env.HOLD_LIFTED!=='coordinator-lifted')throw Error('refusing: a hold is on any Rust hub on a copy of the real log (COMMON.md); use a synthetic log, or set HOLD_LIFTED=coordinator-lifted only when the coordinator has lifted it');
}
function prepareCopy(){assertSafePaths();const free=Number(execSync("df -B1G --output=avail / | tail -1",{encoding:'utf8'}).trim());if(free<40)throw Error('df: only '+free+' GiB free on /, need >= 40');
 mkdirSync(HOME,{recursive:true});execFileSync('cp',['--reflink=auto',SRC+'/events.jsonl',HOME+'/events.jsonl']);if(process.env.WITH_STORE&&existsSync(SRC+'/store.db'))execFileSync('cp',['--reflink=auto',SRC+'/store.db',HOME+'/store.db']);
 if(existsSync(SRC+'/token'))execFileSync('cp',[SRC+'/token',HOME+'/token']);execFileSync('chmod',['-R','u+rwX',HOME]); // the source is read-only: a throwaway copy has to be writable for a hub to append to it (the guard above already proved HOME is ours)
 R.copy={made:new Date().toISOString(),dfFreeGiB:free,withStoreDb:!!process.env.WITH_STORE}}
function removeCopy(){try{assertSafePaths();rmSync(HOME,{recursive:true,force:true})}catch(e){console.error('not removing the copy: '+e.message)}}const PORT=3830,HOST='100.64.84.41';
const root=REPO+'/build/gpui/perf-run/m0';mkdirSync(root+'/fakehome',{recursive:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
const out=process.env.M0_OUT||(REPO+'/build/gpui/results/perf/m0');mkdirSync(out,{recursive:true});
const stat=a=>({n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:Math.max(...a)});
const R=existsSync(out+'/m0.json')?JSON.parse(readFileSync(out+'/m0.json','utf8')):{machine:'desktop 32 cores, 59 GiB, linux 7.0',hub:process.env.HUB_LABEL||'build/gpui/backplane-serve 0.14.0 (JS on Bun)',store:'~/.backplane-gpui-copy',copyEnv:'BACKPLANE_COPY=1, fake HOME, stand-in agents'};
const save=()=>writeFileSync(out+'/m0.json',JSON.stringify(R,null,1));
// live hub reference, /proc only
function live(){const p=Number(readFileSync(process.env.HOME+'/.backplane-bend/hub.lock','utf8').split(' ')[0]);return {pid:p,rssMB:rssKb(p)/1024,cpuTicks:cpuTicks(p),fds:fds(p),threads:threads(p)}}
async function liveRef(secs){const a=live();await sleep(secs*1000);const b=live();return {rssMB:b.rssMB,cpuCorePct:(b.cpuTicks-a.cpuTicks)*10/(secs*1000)*100,fds:b.fds,threads:b.threads,secs}}
async function boot(tag,extra={}){
 const t0=performance.now();
 const h=await startHub({bin:process.env.HUB_BIN||(REPO+'/build/gpui/backplane-serve'),extraArgs:JSON.parse(process.env.HUB_ARGS||'[]'),home:HOME,port:PORT,host:HOST,standins:root+'/bin',cap:'3700M',log:out+`/hub-${tag}.log`,extraEnv:{BACKPLANE_COPY:'1',HOME:root+'/fakehome',FAKE_SECS:'1200',FAKE_PER:'2',FAKE_CYCLE:'20',...extra}});
 const tHello=performance.now()-t0;
 const v=await viewClient(h,{mobile:true,delta:true});const tg=performance.now()-t0;
 const r=await v.open();const tv=performance.now()-t0;
 globalThis.__hubpid=h.pid;const peak=hwmKb(h.pid)/1024;
 return {h,v,b:{helloS:tHello/1000,greetingS:tg/1000,firstViewS:tv/1000,firstViewBytes:r?.bytes,rssMBAtUsable:rssKb(h.pid)/1024,hwmMB:peak,files:dirSizes(HOME),fds:fds(h.pid)}};
}
process.on('exit',()=>{try{if(globalThis.__hubpid)process.kill(globalThis.__hubpid,'SIGTERM')}catch{}});
const phase=process.argv[2]||'all';
process.on('exit',()=>{if(!process.env.KEEP_COPY)removeCopy()});
assertSafePaths(); // once, before ANY phase (also a `later` phase on a kept copy), then the copy is made unless REUSE_COPY
if(!process.env.REUSE_COPY)prepareCopy();
const run=(n)=>phase==='all'||phase===n;
let cur=null;
async function stopWait(h){stopHub(h);for(let i=0;i<300;i++){if(!existsSync('/proc/'+h.pid))break;await sleep(200)}await sleep(500)}
if(run('first')){

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
 await sleep(4000); // let the hub's log lines (piped) reach the file before reading the log count
 const hl=readFileSync(out+'/hub-main.log','utf8');const cnt=Number([...hl.matchAll(/log count (\d+)/g)].pop()?.[1]);const origin=[...hl.matchAll(/hub: origin (\S+)/g)].pop()?.[1];
 const per=new Map(),lag=[];let counting=false,got=0;
 const c=await client(h,{since:cnt,origin,onMsg:(o,now)=>{if(o.t!=='delta'||!counting)return;got++;(per.get(o.thread)??per.set(o.thread,[]).get(o.thread)).push(now);const m=/@(\d{13})@/.exec(o.text??'');if(m)lag.push(now-Number(m[1]))}});await sleep(500);
 c.send('project.add',{path:root+'/proj'});let pc=await c.until(4000,()=>c.seen.find(x=>x.$==='ProjectCreated'&&x.root?.includes('perf-run/m0')));
 if(!pc){ /* already added in an earlier phase: its id from the log line of my own project (grep, nothing else read) */ const g=execSync(`grep -m1 '"root":"${root}/proj"' ${HOME}/events.jsonl`,{encoding:'utf8'});pc=JSON.parse(g.trim().split('\n')[0])}
 if(!pc)throw Error('no project');
 const N=20;const ths=[];
 for(let i=0;i<N;i++){const r=await c.rpc('thread.create',{project:pc.id,title:'m0 sim '+i,env:'local'});if(!r?.o?.ok)throw Error('thread.create failed');ths.push({id:r.o.thread})}
 for(const t of ths){const r=await c.rpc('thread.provider',{thread:t.id,provider:'claude',model:''});if(!r?.o?.ok)throw Error('thread.provider refused')}
 R.simThreadsClaude='thread.provider acknowledged for all 20; validity checked by streams below';
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
