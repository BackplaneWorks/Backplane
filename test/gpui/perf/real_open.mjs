// Lane 7: the real-log open test. A hub and the GPUI client on a THROWAWAY copy of a log, each in its own capped scope, stand-in agents only.
// Measures: hub start to /hello, the client's first frame and `usable` (sidebar drawn, from its own GPUI_METRIC), memory per scope (cgroup anon/file, top processes),
// client PSS, CPU, and classifies the outcome (opened / hub-exited / oom / no-usable / refused). A hub that is OOM-killed is a RESULT, not a harness error.
//   bun real_open.mjs --log PATH --hub-bin PATH [--hub-arg A ...] --gpui-bin PATH [--display :62] [--cap 3700M] [--steady 60] [--out DIR] [--home DIR] [--port 3831]
// HOLD (COMMON.md): refuses a copy of the real log unless REAL_OPEN_HOLD_LIFTED=bub-lifted, which only Bub's word (relayed by the coordinator) justifies.
import {mkdirSync,writeFileSync,readFileSync,statSync,existsSync as _ex,existsSync,rmSync,readdirSync,appendFileSync} from 'node:fs';
import {realpathSync} from 'node:fs';import {execFileSync,spawn} from 'node:child_process';import pathMod from 'node:path';
import {makeStandins,startHub,stopHub,sleep,client} from './lib.mjs';
const argv=process.argv.slice(2);const arg=(k,d)=>{const i=argv.indexOf('--'+k);return i>=0?argv[i+1]:d};const args=k=>argv.flatMap((a,i)=>a==='--'+k?[argv[i+1]]:[]);
const REPO=process.env.PERF_REPO||'/home/h/Documents/i2cjak/backplane-gpui';const HOMEDIR=process.env.HOME;
const SCRATCH=HOMEDIR+'/.backplane-gpui-scratch',BUILD=REPO+'/build/gpui';
const log=arg('log'),hubBin=arg('hub-bin'),gpuiBin=arg('gpui-bin');const AGENTS=Number(arg('agents','10')),VIEWS=Number(arg('views','5'));const cap=arg('cap','3700M'),steady=Number(arg('steady','60')),port=Number(arg('port','3831')),display=arg('display');
const stamp=new Date().toISOString().replace(/[:.]/g,'-');
const copyHome=pathMod.resolve(arg('home',SCRATCH+'/real-open-'+stamp)),out=pathMod.resolve(arg('out',BUILD+'/results/perf/real-open-'+stamp));
if(!log||!hubBin||!gpuiBin){console.error('usage: real_open.mjs --log events.jsonl --hub-bin B --gpui-bin G [--hub-arg ...]');process.exit(4)}
const under=(p,d)=>{const r=pathMod.resolve(p),b=pathMod.resolve(d);return r===b||r.startsWith(b+pathMod.sep)};
const REAL=[HOMEDIR+'/.backplane-gpui-copy',HOMEDIR+'/.backplane-bend',HOMEDIR+'/.backplane-gpui',HOMEDIR+'/.local/share/backplane'];
const isRealLog=p=>{const r=pathMod.resolve(p);let rr=r;try{rr=realpathSync(r)}catch{}return [r,rr].some(x=>REAL.some(d=>under(x,d))||/\/\.backplane-gpui-scratch\/(store-copy-backup|person-)/.test(x))};
if(!display||display===process.env.DISPLAY||/^:0(\.\d+)?$/.test(display)){console.error('--display is required and must be an isolated Xvfb (e.g. :61), never the session display');process.exit(4)}
const result={test:'real-open',when:new Date().toISOString(),log:pathMod.basename(pathMod.dirname(pathMod.resolve(log)))+'/'+pathMod.basename(log),logBytes:0,cap,steadySecs:steady,outcome:null,notes:[]};
mkdirSync(out,{recursive:true});
const finish=(code)=>{if(code===0&&result.outcome!=="opened")code=2;writeFileSync(out+'/result.json',JSON.stringify(result,null,1));console.log(JSON.stringify({outcome:result.outcome,...(result.summary||{})}));process.exit(code)};
// refusals, before anything is made
if(isRealLog(log)&&process.env.REAL_OPEN_HOLD_LIFTED!=='bub-lifted'){result.outcome='refused';result.notes.push('a hold is on any hub on a copy of the real log (COMMON.md); lifted only by Bub\'s word, relayed by the coordinator');finish(3)}
const h=pathMod.resolve(copyHome);
if(!((under(h,SCRATCH)&&h!==SCRATCH)||(under(h,BUILD)&&h!==pathMod.resolve(BUILD))||process.env.REAL_OPEN_ALLOW_HOME===h)){result.outcome='refused';result.notes.push('throwaway home must be strictly under '+SCRATCH+' or '+BUILD);finish(3)}
if(under(h,pathMod.resolve(log))||under(pathMod.resolve(log),h)||REAL.some(d=>under(h,d)||under(d,h))){result.outcome='refused';result.notes.push('throwaway home touches a real home or the source');finish(3)}
// the throwaway copy
const df=Number(execFileSync('sh',['-c',"df -B1G --output=avail "+JSON.stringify(h.startsWith('/')?pathMod.dirname(h):'/')+" 2>/dev/null | tail -1"],{encoding:'utf8'}).trim()||0);
if(process.env.REAL_OPEN_MIN_FREE_GIB!=='0'&&df<Number(process.env.REAL_OPEN_MIN_FREE_GIB||40)){result.outcome='refused';result.notes.push('only '+df+' GiB free; need '+(process.env.REAL_OPEN_MIN_FREE_GIB||40));finish(3)}
let made=false;const cleanup=()=>{try{if(gp&&gp.pid)process.kill(gp.pid,'SIGTERM')}catch{}try{if(hub)stopHub(hub)}catch{}if(made)try{rmSync(h,{recursive:true,force:true})}catch{}};process.on('exit',cleanup);for(const sg of ['SIGINT','SIGTERM'])process.on(sg,()=>{cleanup();process.exit(130)});
let hub=null,gp=null,gMetrics=[];
made=true;mkdirSync(h,{recursive:true});execFileSync('cp',['--reflink=auto',log,h+'/events.jsonl']);execFileSync('chmod',['-R','u+rwX',h]);result.logBytes=statSync(h+'/events.jsonl').size;appendFileSync(h+'/events.jsonl',JSON.stringify({$:'SettingSet',key:'restart.continue',value:'off',at:Math.floor(Date.now()/1000)})+'\n');result.notes.push('restart.continue off appended to the throwaway copy (hold condition 5)');
const standins=out+'/bin';makeStandins(standins);const fake=out+'/fakehome';mkdirSync(fake,{recursive:true});
// cgroup helpers
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const cgOf=pid=>{const m=/0::(.*)/.exec(rd(`/proc/${pid}/cgroup`));return m?'/sys/fs/cgroup'+m[1].trim():''};
const num=(s,k)=>Number(new RegExp('^'+k+':?\\s+(\\d+)','m').exec(s)?.[1]||0);
const memOf=cg=>{const ms=rd(cg+'/memory.stat');const ev=rd(cg+'/memory.events');return {currentMB:Number(rd(cg+'/memory.current'))/1048576,anonMB:num(ms,'anon')/1048576,fileMB:num(ms,'file')/1048576,oomKills:num(ev,'oom_kill')}};
const procsIn=cg=>{const out=[];if(!cg)return out;for(const d of readdirSync('/proc')){if(!/^\d+$/.test(d))continue;if(!rd(`/proc/${d}/cgroup`).includes(cg.replace('/sys/fs/cgroup','')))continue;const s=rd(`/proc/${d}/status`);if(!s)continue;out.push({pid:+d,cmd:(rd(`/proc/${d}/cmdline`).split('\0')[0]||'').split('/').pop(),anonMB:Math.round(num(s,'RssAnon')/1024)})}return out.sort((a,b)=>b.anonMB-a.anonMB).slice(0,3)};
const samples=out+'/samples.jsonl';writeFileSync(samples,'');
let hubCg='',cliCg='',peak={hubScopeMB:0,hubAnonMB:0,policyAnonMB:0,clientScopeMB:0},sampling=true,oomSeen=0,lastHub=null;
const sampler=(async()=>{const t0=Date.now();while(sampling){const row={t:+(((Date.now()-t0)/1000).toFixed(1))};
  if(hubCg){const m=memOf(hubCg);if(m.currentMB>0||m.oomKills){row.hub=m;row.top=procsIn(hubCg);lastHub=row;peak.hubScopeMB=Math.max(peak.hubScopeMB,m.currentMB);peak.hubAnonMB=Math.max(peak.hubAnonMB,m.anonMB);oomSeen=Math.max(oomSeen,m.oomKills);for(const p of row.top)if(/policy|pager/.test(p.cmd))peak.policyAnonMB=Math.max(peak.policyAnonMB,p.anonMB)}}
  if(cliCg){const m=memOf(cliCg);if(m.currentMB>0){row.client=m;peak.clientScopeMB=Math.max(peak.clientScopeMB,m.currentMB)}}
  appendFileSync(samples,JSON.stringify(row)+'\n');await sleep(500)}})();
const scopeOK=(()=>{try{execFileSync('systemd-run',['--user','--scope','-q','true'],{stdio:'ignore',timeout:8000});return true}catch{return false}})();
result.capEnforced=scopeOK;if(!scopeOK){result.outcome='refused';result.notes.push('systemd-run --user --scope unavailable: a hub on this desktop without a cap breaks COMMON');await stopAll();finish(3)}
const stopAll=async()=>{sampling=false;try{if(gp)process.kill(gp.pid,'SIGTERM')}catch{}if(hub)try{stopHub(hub)}catch{}await sleep(2500);try{rmSync(h,{recursive:true,force:true})}catch{}await sampler.catch(()=>{})};
try{
  const t0=Date.now();
  // hub: its scope is found by pid once it has written hub.lock; startHub throws if the hub dies before /hello
  const poll=setInterval(()=>{if(hubCg)return;const m=/^(\d+)/.exec(rd(h+'/hub.lock'));if(m){hubCg=cgOf(Number(m[1]))}},200);
  try{hub=await startHub({bin:pathMod.resolve(hubBin),home:h,port,host:'127.0.0.1',standins,cap,extraArgs:args('hub-arg'),log:out+'/hub.log',unsetEnv:['DBUS_SESSION_BUS_ADDRESS','XDG_RUNTIME_DIR'],extraEnv:{FAKE_SECS:String(steady+200),FAKE_PER:'2',FAKE_CYCLE:'20',BACKPLANE_COPY:'1',HOME:fake,TZ:'Etc/UTC'}})}
  catch(e){clearInterval(poll);result.hubStartMs=Date.now()-t0;const capMB=Number(String(cap).replace(/[^0-9]/g,''))*(/G/i.test(cap)?1024:1);const nearCap=peak.hubScopeMB>=0.95*capMB;if(oomSeen===0&&nearCap)result.notes.push('the scope was gone before memory.events could be read; peak '+Math.round(peak.hubScopeMB)+' MB of '+capMB+' MB, so classified oom');result.outcome=((oomSeen>0||nearCap)?'oom':'hub-exited');result.notes.push(String(e.message).slice(0,200));if(oomSeen>0)result.notes.push('the kernel OOM-killed the hub scope (memory.events oom_kill='+oomSeen+')');
    result.summary={hubStartMs:result.hubStartMs,peak,lastTop:lastHub?.top};result.peak=peak;result.lastHubSample=lastHub;await stopAll();finish(2)}
  clearInterval(poll);result.hubStartMs=Date.now()-t0;if(!hubCg)hubCg=cgOf(hub.pid);
  // the whole startup (store build included) up to /hello; anon is what the cap is about, current also counts page cache
  result.startup={helloMs:result.hubStartMs,peakAnonMB:peak.hubAnonMB,peakScopeMB:peak.hubScopeMB};
  // load: AGENTS stand-in agents streaming and VIEWS views open, through the hub's own protocol
  const loadC=await client(hub,{});const proj=h+'/loadproj';mkdirSync(proj,{recursive:true});
  loadC.send('project.add',{path:proj});const pc=await loadC.until(20000,()=>loadC.seen.find(x=>x.$==='ProjectCreated'&&x.root===proj));
  for(let i=0;i<AGENTS;i++)loadC.send('thread.create',{project:pc?.id,title:'open-test '+i,env:'local'});
  const mine=await loadC.until(30000,()=>{const t=loadC.seen.filter(x=>x.$==='ThreadCreated'&&String(x.title).startsWith('open-test'));return t.length>=AGENTS?t:undefined});
  result.load={agentsAsked:AGENTS,agentsCreated:mine?.length??0,viewsAsked:VIEWS,viewsOpened:0};
  if(mine){for(const t of mine)loadC.send('turn.start',{thread:t.id,text:'stream for the open test'});
    for(let i=0;i<VIEWS;i++){const v=await client(hub,{view:true});v.target={kind:'thread',id:mine[i%mine.length].id};const r=await v.rpc('view.open',{target:v.target,viewId:''},120000);if(r)result.load.viewsOpened++}}
  // the client, in its own scope; the hub's token file is the only credential
  const tClient=Date.now();const sf=out+'/open.script.json';writeFileSync(sf,JSON.stringify([{select:0},{wait:(steady+30)*1000}]));
  const cenv={...process.env,DISPLAY:display,HOME:fake,...(process.env.VK_ICD_FILENAMES?{VK_ICD_FILENAMES:process.env.VK_ICD_FILENAMES}:{})};
  const cargs=['--url',hub.url,'--token-file',h+'/token','--script',sf];
  gp=spawn('systemd-run',['--user','--scope','--quiet','-p','MemoryMax='+cap,'-p','MemorySwapMax=0','env','-i','PATH='+process.env.PATH,...Object.entries(cenv).filter(([k])=>/^(DISPLAY|HOME|VK_ICD_FILENAMES|BACKPLANE_GPUI_ASSETS|LD_LIBRARY_PATH)$/.test(k)).map(([k,v])=>k+'='+v),gpuiBin,...cargs],{env:cenv,stdio:['ignore','pipe','pipe']});
  let buf='';gp.stderr.on('data',d=>{appendFileSync(out+'/client.stderr.log',d);buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{gMetrics.push({...JSON.parse(l.slice(k+12)),wall:Date.now()})}catch{}}});gp.stdout.on('data',d=>appendFileSync(out+'/client.stdout.log',d));
  let gExit=null;gp.on('exit',(c,s)=>{gExit={code:c,signal:s}});
  await sleep(500);cliCg=cgOf(gp.pid);
  const waitFor=async(f,ms)=>{const d=Date.now()+ms;while(Date.now()<d){const v=f();if(v)return v;if(gExit)return null;await sleep(100)}return null};
  const usable=await waitFor(()=>gMetrics.find(m=>m.event==='usable'),120000);
  const ff=gMetrics.find(m=>m.event==='first_frame'),gpu=gMetrics.find(m=>m.event==='gpu');
  result.client={firstFrameMs:ff?ff.epoch_ms-tClient:null,usableMs:usable?usable.epoch_ms-tClient:null,threads:usable?.threads??null,entries:usable?.entries??null,gpu:gpu?{device:gpu.device,software:gpu.software}:null,exit:gExit,notUsable:gMetrics.find(m=>m.event==='not_usable')||null};
  result.startupToUsableMs=usable?(usable.epoch_ms-t0):null; // hub spawn -> client's sidebar drawn
  if(!usable){result.outcome='no-usable';result.notes.push('the client never reported usable (see client.stderr.log)')}
  else{ // steady: memory and CPU while the client sits on a thread
    const cpu=pid=>{const f=rd(`/proc/${pid}/stat`).split(') ')[1]?.split(' ');return f?Number(f[11])+Number(f[12]):0};
    const c0=cpu(hub.pid),g0=cpu(gp.pid),w0=Date.now();await sleep(steady*1000);const w=(Date.now()-w0)/1000;
    const rssMB=pid=>num(rd(`/proc/${pid}/status`),'VmRSS')/1024;const sm=pid=>{const s=rd(`/proc/${pid}/smaps_rollup`);return {rssMB:num(s,'Rss')/1024,pssMB:num(s,'Pss')/1024,privMB:(num(s,'Private_Clean')+num(s,'Private_Dirty'))/1024}};
    result.steady={hubRssMB:rssMB(hub.pid),hubCpuCorePct:(cpu(hub.pid)-c0)*10/(w*1000)*100,clientCpuCorePct:(cpu(gp.pid)-g0)*10/(w*1000)*100,hubProcess:sm(hub.pid),client:sm(gp.pid),hubScope:memOf(hubCg),clientScope:memOf(cliCg),hubTop:procsIn(hubCg)};
    const hubAlive=(()=>{try{process.kill(hub.pid,0);return true}catch{return false}})();const oomNow=hubCg?memOf(hubCg).oomKills:0;oomSeen=Math.max(oomSeen,oomNow);
    result.steadyChecks={hubAlive,clientExit:gExit,oomKills:oomSeen,helperAlive:procsIn(hubCg).length};
    if(oomSeen>0){result.outcome='oom';result.notes.push('hub scope OOM-killed during steady (oom_kill='+oomSeen+')')}
    else if(!hubAlive){result.outcome='hub-exited';result.notes.push('hub died during steady')}
    else if(gExit){result.outcome='client-exited';result.notes.push('client exited during steady: '+JSON.stringify(gExit))}
    else result.outcome='opened'}
  result.peak=peak;result.summary={outcome:result.outcome,hubStartMs:result.hubStartMs,startupToUsableMs:result.startupToUsableMs,clientUsableMs:result.client?.usableMs,peak,steady:result.steady&&{hubScopeMB:Math.round(result.steady.hubScope.currentMB),clientScopeMB:Math.round(result.steady.clientScope.currentMB),clientPssMB:Math.round(result.steady.client.pssMB),hubCpu:+result.steady.hubCpuCorePct.toFixed(1),clientCpu:+result.steady.clientCpuCorePct.toFixed(1)}};
  {const st=result.steady,su=result.startup;const crit={startupUnder1GB:su.peakAnonMB<1024,helloAnswered:true,noOom:oomSeen===0,steadyHubAtMost500MB:st?(st.hubScope.anonMB>0&&st.hubScope.anonMB<=500):false,agentsAndViews:(result.load?.agentsCreated===AGENTS)&&(result.load?.viewsOpened===VIEWS),clientUsable:!!result.client?.usableMs};
    result.pass=Object.values(crit).every(Boolean)&&result.outcome==='opened';result.criteria=crit;
    result.summary={...result.summary,pass:result.pass,criteria:crit,startupPeakAnonMB:Math.round(su.peakAnonMB),startupPeakScopeMB:Math.round(su.peakScopeMB),steadyHubAnonMB:st?Math.round(st.hubScope.anonMB):null,steadyHubScopeMB:st?Math.round(st.hubScope.currentMB):null,steadyHubRssMB:st?Math.round(st.hubRssMB):null}}
  await stopAll();finish(result.pass?0:2)
}catch(e){result.outcome=result.outcome||'harness-error';result.notes.push('harness error: '+String(e.stack||e).slice(0,400));try{await stopAll()}catch{}finish(4)}
