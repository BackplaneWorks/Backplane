// Crash and recovery scenarios for the Rust hub. bun crash.mjs bin=PATH out=DIR scenario=NAME rounds=N [policy=PATH] [events=N]
//  kill-write  SIGKILL while 8 writers have acked/in-flight project.add (the group commit's fsync is what the ack waits for)
//  kill-turn   SIGKILL with 20 stand-in agents streaming; then orphans, zombies, turn states, resume
//  torn        append a half line to the log of a stopped hub (several shapes), restart, write more
//  agent       SIGKILL stand-in agent processes under a live hub
//  policy      SIGKILL the bp-policy child under a live hub (needs policy=)
//  diskfull    hub home on a small filesystem (sudo loop/tmpfs mount), fill it, write through ENOSPC, free it, restart
//  restart     kill -9 and restart a synthetic log of `events` lines (default 1,000,000) `rounds` times with writers running
// Every scenario ends with verifyLog over the whole home: acked writes present once, no duplicates, only torn fragments bad.
import {mkdirSync,rmSync,writeFileSync,appendFileSync,existsSync,statSync,truncateSync,readFileSync,openSync} from 'node:fs';
import {execSync,spawnSync} from 'node:child_process';
import {makeStandins,seedLines,startHub,killHub,client,sleep,pct,rssKb,hwmKb,procs,descendants,zombies,originOf,countLines,alive,exists,hello,fdInfo} from './lib.mjs';
import {verifyLog} from './verify.mjs';
import {guard} from './lib.mjs';
const A=Object.fromEntries(process.argv.slice(2).map(a=>{const i=a.indexOf('=');return [a.slice(0,i),a.slice(i+1)]}));
const bin=A.bin,out=A.out,scenario=A.scenario,rounds=Number(A.rounds??10),policy=A.policy||undefined,port0=Number(A.port??3850);
if(!bin||!out||!scenario){console.error('need bin= out= scenario=');process.exit(2)}
try{guard({home:A.work??out,port:port0,paths:[out,bin]})}catch(e){console.error(e.message);process.exit(3)}
mkdirSync(out,{recursive:true});
const work=A.work??`${out}/work-${scenario}`;
const log=(...a)=>{const s=`${new Date().toISOString()} [${scenario}] ${a.join(' ')}`;console.log(s);appendFileSync(`${out}/${scenario}.log`,s+'\n')};
const R={scenario,bin,rounds,policy:!!policy,machine:{kernel:sh('uname -r'),nproc:sh('nproc'),host:sh('hostname')},rounds_data:[],assert:[],failures:[]};
function sh(c){try{return execSync(c,{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim()}catch{return ''}}
const check=(name,cond,detail)=>{R.assert.push({name,ok:!!cond,detail});if(!cond){R.failures.push({name,detail});log('ASSERT FAILED:',name,JSON.stringify(detail)??'')}};
const ids=[...Array(20)].map((_,i)=>`ts${String(i).padStart(3,'0')}`);
const acked=new Set(),attempted=new Set();let seq=0;
let kills=0;
const rig={home:`${work}/home`,standins:`${work}/standins`,proj:`${work}/proj`};
function fresh(events){rmSync(work,{recursive:true,force:true});mkdirSync(rig.home,{recursive:true});mkdirSync(rig.proj,{recursive:true});makeStandins(rig.standins);
 const {lines}=seedLines(rig.proj,20);writeFileSync(`${rig.home}/events.jsonl`,lines.join('\n')+'\n')}
let hubN=0;
async function boot(extra={}){hubN++;const n0=(await countLines(`${rig.home}/events.jsonl`));const org=originOf(rig.home);
 const t0=performance.now();const h=await startHub({bin,home:rig.home,port:port0+(hubN%400),standins:rig.standins,policy:extra.policy===false?undefined:policy,log:`${out}/${scenario}-hub.log`,extraEnv:{FAKE_SECS:'86400',FAKE_PER:'2',FAKE_CYCLE:'20',...(extra.env??{})}});
 rig.lastHub=h.pid;rig.lastHub=h.pid;const bootMs=performance.now()-t0;return {h,n0:n0.lines,tailUnterminated:!n0.endsWithNewline,origin:org,bootMs,rss:rssKb(h.pid)}}
// closed-loop writers: each keeps one project.add in flight; `acked` gets a path only when the hub says ok
async function writers(h,n,stopRef,tag,pathLen=0){const o=await boot_origin(h);const cs=[];
 for(let i=0;i<n;i++){cs.push(client(h,{since:o.n,origin:o.origin}).then(async c=>{let mine=0,fails=0;
  while(!stopRef.stop){const pad=pathLen?'x'.repeat(pathLen):'';const path=`${rig.proj}/w/${tag}-${i}-${seq++}${pad}`;attempted.add(path);
   const r=await c.rpc('project.add',{path},15000);if(r?.o?.ok){acked.add(path);mine++}else{fails++;if(!r)break;if(stopRef.stopOnFail&&fails>3)break}}
  c.close();return {mine,fails}}).catch(e=>({err:String(e)})))}
 return Promise.all(cs)}
async function boot_origin(h){const n=(await countLines(`${rig.home}/events.jsonl`)).lines;return {n,origin:originOf(rig.home)}}
async function hardKill(h){killHub(h,'SIGKILL');kills++;const t=Date.now();while(exists(h.pid)&&alive(h.pid)&&Date.now()-t<30000)await sleep(20);return Date.now()-t}
async function orphans(before){const t=Date.now();let left=before;while(Date.now()-t<12000){left=before.filter(p=>exists(p)&&alive(p));if(!left.length)break;await sleep(100)}return left}
const rand=(a,b)=>a+Math.random()*(b-a);
async function finish(extra={}){const v=await verifyLog(rig.home,{acked,expectFragments:kills+(extra.fragments??0)});
 R.verify={lines:v.lines,valid:v.valid,bad:v.bad.length,badSample:v.bad.slice(0,5),tail:v.tail,dups:v.dups.slice(0,10),dupCount:v.dups.length,missing:v.missing.slice(0,10),missingCount:v.missing.length,multiCount:v.multi.length,acked:v.ackedChecked,attempted:attempted.size,kills};
 check('every acked write is in the log exactly once',v.missing.length===0&&v.multi.length===0,{missing:v.missing.length,multi:v.multi.length,acked:acked.size});
 check('no duplicate events (same message or activity twice)',v.dups.length===0,v.dups.slice(0,5));
 R.verify.idCollisions=v.idDups.length;R.verify.idCollisionSample=v.idDups.slice(0,6);
 check('ProjectCreated/ThreadCreated ids are unique (project.add derives them from store.len(); concurrent adds collide)',v.idDups.length===0,{collisions:v.idDups.length,sample:v.idDups.slice(0,4)});
 check('only torn fragments are invalid lines (at most one per kill)',v.bad.length<=kills+(extra.fragments??0),{bad:v.bad.length,kills,sample:v.bad.slice(0,3)});
 return v}
// ---------------------------------------------------------------------------------------------------------------------
const S={};
S['kill-write']=async()=>{fresh();
 for(let r=1;r<=rounds;r++){const b=await boot();const stopRef={stop:false};
  const wp=writers(b.h,8,stopRef,`r${r}`);await sleep(rand(300,2500)); // kill mid-write: writers always have requests in flight
  const ackedBefore=acked.size;const stopMs=await hardKill(b.h);stopRef.stop=true;const res=await wp;
  const z=zombies(procs(),rig.lastHub).length;
  // the killed hub must not leave a half-day of state: restart reads whole lines, ignores/closes off the torn tail
  const b2=await boot({policy:false});const check1=await client(b2.h,{since:0,origin:'x'}); // origin mismatch: whole log, counts what the hub indexed
  let total=0;check1.onItems=its=>total+=its.length;await check1.waitFor(o=>o.t==='info',20000);check1.close();
  const v=await verifyLog(rig.home,{acked,expectFragments:kills});
  const w=await client(b2.h,{since:total,origin:originOf(rig.home)});const probe=`${rig.proj}/w/post-${r}-${seq++}`;attempted.add(probe);const pr=await w.rpc('project.add',{path:probe},15000);if(pr?.o?.ok)acked.add(probe);w.close();
  R.rounds_data.push({r,ackedThisRound:acked.size-ackedBefore,ackedTotal:acked.size,stopMs,zombies:z,logLines:v.lines,validLines:v.valid,bad:v.bad.length,tail:v.tail?.len??0,missing:v.missing.length,dups:v.dups.length,hubIndexed:total,bootMs:+b2.bootMs.toFixed(0),postWriteOk:!!pr?.o?.ok});
  check(`round ${r}: hub indexed exactly the valid lines`,total===v.valid,{indexed:total,valid:v.valid,bad:v.bad.length});
  check(`round ${r}: write after restart acked`,!!pr?.o?.ok,pr?.o);
  log(`round ${r}: acked+${acked.size-ackedBefore} lines=${v.lines} bad=${v.bad.length} tail=${v.tail?.len??0} missing=${v.missing.length} dups=${v.dups.length} indexed=${total} boot=${b2.bootMs.toFixed(0)}ms post=${pr?.o?.ok}`);
  await hardKill(b2.h);kills; // next round starts from a killed hub too
 }
 await finish()};
S['kill-turn']=async()=>{fresh();
 for(let r=1;r<=rounds;r++){const b=await boot();const c=await client(b.h,{since:b.n0,origin:b.origin});let deltas=0;c.onItems=null;
  const perThread=new Map();const c2=c;c2.ws.addEventListener('message',()=>{});
  const wr=await client(b.h,{since:b.n0,origin:b.origin,onFrame:o=>{if(o.t==='delta'){deltas++;perThread.set(o.thread,(perThread.get(o.thread)??0)+1)}}});
  for(const id of ids)await wr.rpc('turn.start',{thread:id,text:'stream',msg:`kt${r}-${id}`},20000);
  await sleep(rand(3000,6000));const streaming=perThread.size;
  const wp=writers(b.h,2,{stop:false,get stop(){return false}},`kt${r}`).catch(()=>{});
  await sleep(500);
  const kidsBefore=descendants(b.h.pid).map(p=>p.pid);const stopMs=await hardKill(b.h);wr.close();c.close();
  const left=await orphans(kidsBefore);const z=zombies(procs(),rig.lastHub).length;
  const b2=await boot({policy:false});await sleep(1000);
  // after the restart: does the hub resume streaming turns, or leave them marked running with nothing behind them?
  const perThread2=new Map();const w2=await client(b2.h,{since:b2.n0,origin:originOf(rig.home),onFrame:o=>{if(o.t==='delta')perThread2.set(o.thread,(perThread2.get(o.thread)??0)+1)}});
  await sleep(15000);const resumed=perThread2.size;const standinsAlive=descendants(b2.h.pid).filter(p=>p.comm==='claude').length;
  const v=await verifyLog(rig.home,{acked,expectFragments:kills});const running=[...v.turn].filter(([,s])=>s==='running').map(([t])=>t);
  R.rounds_data.push({r,streamingBeforeKill:streaming,orphansAfter:left.length,zombies:z,stopMs,bootMs:+b2.bootMs.toFixed(0),resumedThreads:resumed,standinsAfterRestart:standinsAlive,threadsMarkedRunning:running.length,turnStatesAfterRestart:Object.fromEntries([...v.turn.values()].reduce((m,s)=>m.set(s,(m.get(s)??0)+1),new Map())),dups:v.dups.length,bad:v.bad.length,missing:v.missing.length});
  check(`round ${r}: no stand-in outlives the killed hub`,left.length===0,{orphans:left});
  check(`round ${r}: no zombies`,z===0,{zombies:z});
  check(`round ${r}: no thread is left "running" without an agent process`,running.length===0||standinsAlive>=running.length,{marked:running.length,standins:standinsAlive,resumedThreads:resumed});
  log(`round ${r}: streaming=${streaming} orphans=${left.length} zombies=${z} boot=${b2.bootMs.toFixed(0)}ms resumed=${resumed} standins=${standinsAlive} markedRunning=${running.length} dups=${v.dups.length}`);
  w2.close();await hardKill(b2.h);}
 await finish()};
S['torn']=async()=>{fresh();
 const shapes=[['half object','{"$":"MessagePosted","thread":"ts000","msg":"torn-1","role":"user","te'],['one brace','{'],['dollar key','{"$"'],['multibyte cut',Buffer.concat([Buffer.from('{"$":"SettingSet","key":"x","value":"日'),Buffer.from([0xe6,0x97])])],['complete json without newline','{"$":"SettingSet","key":"torn.complete","value":"1","at":1785000000}'],['long fragment','{"$":"SettingSet","key":"big","value":"'+'y'.repeat(70000)]];
 let frags=0;
 for(let r=1;r<=rounds;r++){const [name,frag]=shapes[(r-1)%shapes.length];
  const b=await boot();const stopRef={stop:false};const wp=writers(b.h,4,stopRef,`tn${r}`);await sleep(600);stopRef.stop=true;await wp;await hardKill(b.h);
  const before=await countLines(`${rig.home}/events.jsonl`);
  const fb=Buffer.isBuffer(frag)?frag:Buffer.from(frag,'utf8');appendFileSync(`${rig.home}/events.jsonl`,fb);frags++;   // the half line a crash leaves behind
  const b2=await boot({policy:false});
  const stderr=sh(`tail -c 4000 ${out}/${scenario}-hub.log`);const reported=/torn tail bytes: (\d+)/.exec(stderr.split('\n').filter(l=>/torn tail bytes/.test(l)).pop()??'');
  const w=await client(b2.h,{since:0,origin:'x'});let total=0;w.onItems=its=>total+=its.length;await w.waitFor(o=>o.t==='info',20000);w.close();
  const vPre=await verifyLog(rig.home,{acked,expectFragments:kills+frags});const unindexedComplete=name==='complete json without newline'?1:0;
  check(`round ${r} (${name}): hub indexed every whole line and nothing else`,total===vPre.valid-unindexedComplete,{indexed:total,valid:vPre.valid});
  const wc=await client(b2.h,{since:total,origin:originOf(rig.home)});const ps=[];for(let i=0;i<5;i++){const p=`${rig.proj}/w/after-${r}-${i}-${seq++}`;attempted.add(p);const x=await wc.rpc('project.add',{path:p},15000);if(x?.o?.ok){acked.add(p);ps.push(p)}}wc.close();
  const v=await verifyLog(rig.home,{acked,expectFragments:kills+frags});
  R.rounds_data.push({r,shape:name,tornBytesReported:reported?Number(reported[1]):null,fragmentBytes:fb.length,bootMs:+b2.bootMs.toFixed(0),indexedAtJoin:total,validLinesBeforePostWrites:vPre.valid,validLines:v.valid,postWritesAcked:ps.length,missing:v.missing.length,dups:v.dups.length,bad:v.bad.length});
  check(`round ${r} (${name}): hub starts and reports the torn tail`,reported&&Number(reported[1])>0,{reported:reported?.[1]});
  check(`round ${r} (${name}): writes after the torn tail are acked and readable`,ps.length===5&&v.missing.length===0,{acked:ps.length,missing:v.missing.length});
  log(`round ${r} (${name}): tornBytes=${reported?.[1]} indexed=${total} valid=${v.valid} bad=${v.bad.length} missing=${v.missing.length}`);
  await hardKill(b2.h);}
 await finish({fragments:frags})};
S['agent']=async()=>{fresh();const b=await boot();const per=new Map();
 const c=await client(b.h,{since:b.n0,origin:b.origin,onFrame:o=>{if(o.t==='delta')per.set(o.thread,(per.get(o.thread)??0)+1)}});
 for(const id of ids)await c.rpc('turn.start',{thread:id,text:'stream',msg:`ag-${id}`},20000);await sleep(5000);
 for(let r=1;r<=rounds;r++){
  const ag=descendants(b.h.pid).filter(p=>p.comm==='claude');const victims=ag.sort(()=>Math.random()-.5).slice(0,3);check(`round ${r}: there were agents to kill`,victims.length>0,{agents:ag.length});
  const t0=Date.now();for(const v of victims)try{process.kill(v.pid,'SIGKILL')}catch{}
  await sleep(6000);const zl=zombies(procs(),b.h.pid);const z=zl.length;const zinfo=zl.map(p=>({pid:p.pid,ppid:p.ppid,comm:p.comm,parentIsHub:p.ppid===b.h.pid}));await sleep(9000);const zlate=zombies(procs(),b.h.pid).filter(p=>zl.some(q=>q.pid===p.pid)).length;const hubUp=!!(await hello(b.h))&&alive(b.h.pid);
  const after=descendants(b.h.pid).filter(p=>p.comm==='claude').length;
  // a killed agent's thread: restart it with a new message; the others must not notice
  const ack=await c.rpc('turn.start',{thread:ids[r%20],text:'stream again',msg:`ag2-${r}`},20000);
  const v=await verifyLog(rig.home,{acked,expectFragments:0});const failed=[...v.turn].filter(([,s])=>s==='failed').length;
  R.rounds_data.push({r,killed:victims.length,agentsAfter:after,zombies:z,zombieDetail:zinfo,zombiesStillThere9sLater:zlate,hubUp,turnsFailedInLog:failed,turnStartAcked:!!ack?.o?.ok,dups:v.dups.length});
  check(`round ${r}: hub survives an agent crash`,hubUp,{});check(`round ${r}: no zombies after agent crash`,z===0,{z,zinfo,stillThereLater:zlate});
  log(`round ${r}: killed ${victims.length}, agents now ${after}, zombies ${z}, failedInLog=${failed}`);
 }
 c.close();await hardKill(b.h);await finish()};
S['policy']=async()=>{if(!policy){log('policy= required');process.exit(2)}fresh();const b=await boot();
 const v=await (await import('./view.mjs')).viewClient(b.h,{maxPerSec:5});
 for(let r=1;r<=rounds;r++){
  const pol=descendants(b.h.pid).filter(p=>/policy/.test(p.comm));if(!pol.length){check(`round ${r}: a policy child exists`,false,{kids:descendants(b.h.pid).map(p=>p.comm)});break}
  const pid0=pol[0].pid;const ok0=await v.ask('view.update');try{process.kill(pid0,'SIGKILL')}catch{}
  const t0=Date.now();let recovered=null;const errs=[];
  for(let i=0;i<40&&recovered===null;i++){const x=await v.ask('view.update');if(x.ok)recovered=Date.now()-t0;else errs.push(x.x?.error??'timeout');if(recovered===null)await sleep(500)}
  const now=descendants(b.h.pid).filter(p=>/policy/.test(p.comm)).map(p=>p.pid);
  const w=await client(b.h,{since:0,origin:'x'});w.close();const p1=await b.h&&(await hello(b.h));
  const ack=await (async()=>{const c=await client(b.h,{since:(await countLines(`${rig.home}/events.jsonl`)).lines,origin:originOf(rig.home)});const p=`${rig.proj}/w/pol-${r}-${seq++}`;attempted.add(p);const x=await c.rpc('project.add',{path:p},15000);if(x?.o?.ok)acked.add(p);c.close();return !!x?.o?.ok})();
  R.rounds_data.push({r,policyPidKilled:pid0,viewOkBefore:ok0.ok,viewRecoveredMs:recovered,errorsSeen:[...new Set(errs)],policyPidsAfter:now,hubAnswersHello:Number.isFinite(p1),writeAckedWhileDown:ack});
  check(`round ${r}: hub stays up and keeps acking writes with the policy dead`,Number.isFinite(p1)&&ack,{});
  check(`round ${r}: views recover after a policy crash (a new policy child serves)`,recovered!==null,{errors:[...new Set(errs)].slice(0,3)});
  log(`round ${r}: killed policy ${pid0}; views recovered=${recovered}ms errors=${[...new Set(errs)].join(',')} policyAfter=${now.join(',')} hubUp=${Number.isFinite(p1)} write=${ack}`);
  if(recovered===null){log('policy never came back: stopping rounds');break}
 }
 v.c.close();await hardKill(b.h);await finish()};
S['diskfull']=async()=>{
 const img=`${work}.img`,mnt=`${work}-mnt`;rmSync(work,{recursive:true,force:true});sh(`sudo -n umount ${mnt} 2>/dev/null`);mkdirSync(mnt,{recursive:true});
 const mb=Number(A.fs_mb??48);let kind='ext4-loop';
 let ok=spawnSync('sh',['-c',`rm -f ${img}; truncate -s ${mb}M ${img} && mkfs.ext4 -q -F ${img} && sudo -n mount -o loop ${img} ${mnt} && sudo -n chown $(id -u):$(id -g) ${mnt}`],{encoding:'utf8'});
 if(ok.status!==0){kind='tmpfs';ok=spawnSync('sh',['-c',`sudo -n mount -t tmpfs -o size=${mb}m tmpfs ${mnt} && sudo -n chown $(id -u):$(id -g) ${mnt}`],{encoding:'utf8'})}
 if(ok.status!==0){log('cannot mount a small filesystem here (no sudo): ',ok.stderr);R.skipped='no loop or tmpfs mount available (needs sudo)';return}
 R.fs={kind,mb};log(`small filesystem: ${kind} ${mb} MB at ${mnt}`);
 rig.home=`${mnt}/home`;rig.proj=`${work}/proj`;rig.standins=`${work}/standins`;mkdirSync(rig.home,{recursive:true});mkdirSync(rig.proj,{recursive:true});makeStandins(rig.standins);
 const {lines}=seedLines(rig.proj,20);writeFileSync(`${rig.home}/events.jsonl`,lines.join('\n')+'\n');
 const df=()=>Number(sh(`df -Pk ${mnt} | tail -1 | awk '{print $4}'`));
 try{
 for(let r=1;r<=rounds;r++){
  const b=await boot({policy:false});const o=await boot_origin(b.h);
  // fill to within a few KB, leaving the hub to hit ENOSPC in the middle of a group commit
  const filler=`${mnt}/filler`;const room=Math.max(df()-Number(A.margin_kb??24),0);sh(`rm -f ${filler}; fallocate -l ${room}K ${filler} 2>/dev/null || dd if=/dev/zero of=${filler} bs=1K count=${room} 2>/dev/null`);
  const c=await client(b.h,{since:o.n,origin:o.origin});const okA=[],failA=[];
  for(let i=0;i<60;i++){const p=`${rig.proj}/w/df-${r}-${i}-${seq++}${'p'.repeat(1500)}`;attempted.add(p);const x=await c.rpc('project.add',{path:p},15000);if(x?.o?.ok){acked.add(p);okA.push(p)}else failA.push(x?.o?.error??'timeout')}
  const hubUp=Number.isFinite(await hello(b.h))&&alive(b.h.pid);const vFull=await verifyLog(rig.home,{acked,expectFragments:kills});
  // nothing acked may be lost and a failed write must be an error reply, not an ack
  check(`round ${r}: hub survives ENOSPC`,hubUp,{});check(`round ${r}: ENOSPC writes are refused with an error`,failA.length>0&&failA.every(e=>/store write failed|failed/.test(e)),{failed:failA.length,errs:[...new Set(failA)]});
  check(`round ${r}: acked writes survive an ENOSPC mid-commit`,vFull.missing.length===0,{missing:vFull.missing.length});
  check(`round ${r}: no corrupt line in the middle of the log after ENOSPC`,vFull.bad.length<=kills,{bad:vFull.bad.length,sample:vFull.bad.slice(0,3)});
  // free the space: do writes resume without a restart?
  sh(`rm -f ${filler}`);const resumed=[];for(let i=0;i<5;i++){const p=`${rig.proj}/w/df-${r}-ok-${i}-${seq++}`;attempted.add(p);const x=await c.rpc('project.add',{path:p},15000);if(x?.o?.ok){acked.add(p);resumed.push(p)}}
  const vFree=await verifyLog(rig.home,{acked,expectFragments:kills});
  check(`round ${r}: writes resume once space is free`,resumed.length===5,{resumed:resumed.length});
  check(`round ${r}: the writes after ENOSPC are in the log (no line merged into a fragment)`,vFree.missing.length===0,{missing:vFree.missing.length,bad:vFree.bad.length,sample:vFree.bad.slice(0,2)});
  c.close();await hardKill(b.h);const b2=await boot({policy:false});const vR=await verifyLog(rig.home,{acked,expectFragments:kills});
  let total=0;const w=await client(b2.h,{since:0,origin:'x'});w.onItems=its=>total+=its.length;await w.waitFor(o=>o.t==='info',20000);w.close();
  R.rounds_data.push({r,acked:okA.length,refused:failA.length,errors:[...new Set(failA)],hubUp,badAtFull:vFull.bad.length,missingAtFull:vFull.missing.length,resumedAfterFree:resumed.length,badAfterFree:vFree.bad.length,missingAfterFree:vFree.missing.length,restartBootMs:+b2.bootMs.toFixed(0),indexedAfterRestart:total,validLines:vR.valid,badLines:vR.bad.length});
  check(`round ${r}: restart after ENOSPC indexes the valid lines`,total===vR.valid,{indexed:total,valid:vR.valid});
  log(`round ${r}: acked ${okA.length}, refused ${failA.length} (${[...new Set(failA)].join('|')}), bad@full=${vFull.bad.length} missing@full=${vFull.missing.length}, resumed=${resumed.length}, bad@free=${vFree.bad.length} missing@free=${vFree.missing.length}, indexed=${total}/${vR.valid}`);
  await hardKill(b2.h);}
 await finish();
 }finally{sh(`sudo -n umount ${mnt}`);sh(`rm -f ${img}`)}};
S['restart']=async()=>{const N=Number(A.events??1000000);
 rmSync(work,{recursive:true,force:true});mkdirSync(rig.home,{recursive:true});mkdirSync(rig.proj,{recursive:true});makeStandins(rig.standins);
 const g=spawnSync('python3',[new URL('./gen.py',import.meta.url).pathname,String(N),rig.proj],{encoding:'utf8',maxBuffer:1<<20,stdio:['ignore',openSync(`${rig.home}/events.jsonl`,'w'),'pipe']});
 if(g.status!==0){log('gen.py failed',g.stderr);process.exit(3)}
 const n0=await countLines(`${rig.home}/events.jsonl`);log(`synthetic log: ${n0.lines} lines, ${statSync(`${rig.home}/events.jsonl`).size} bytes`);R.logLines=n0.lines;
 for(let r=1;r<=rounds;r++){const b=await boot();const stopRef={stop:false};
  const per=new Map();const wc=await client(b.h,{since:b.n0,origin:b.origin,onFrame:o=>{if(o.t==='delta')per.set(o.thread,(per.get(o.thread)??0)+1)}});
  const wp=writers(b.h,4,stopRef,`rs${r}`);
  for(const id of ids)await wc.rpc('turn.start',{thread:id,text:'stream',msg:`rs${r}-${id}`},20000);
  const t0=performance.now();let view=null;
  if(policy){try{const vc=await (await import('./view.mjs')).viewClient(b.h,{maxPerSec:5});view={firstViewMs:+vc.openMs.toFixed(0)};vc.c.close()}catch(e){view={error:e.message}}}
  await sleep(rand(4000,9000));
  const rss=rssKb(b.h.pid),hwm=hwmKb(b.h.pid),fd=fdInfo(b.h.pid);const kidsBefore=descendants(b.h.pid).map(p=>p.pid);
  const ackedBefore=acked.size;const stopMs=await hardKill(b.h);stopRef.stop=true;await wp;wc.close();const left=await orphans(kidsBefore);
  R.rounds_data.push({r,eventsAtBoot:b.n0,bootMs:+b.bootMs.toFixed(0),bootRssMB:+(b.rss/1024).toFixed(0),rssBeforeKillMB:+(rss/1024).toFixed(0),hwmMB:+(hwm/1024).toFixed(0),fds:fd.fds,streamingThreads:per.size,ackedThisRound:acked.size-ackedBefore,stopMs,orphans:left.length,zombies:zombies(procs(),rig.lastHub).length,view});
  check(`round ${r}: no orphan agents`,left.length===0,{orphans:left.length});
  log(`round ${r}: events at boot ${b.n0}, listening after ${b.bootMs.toFixed(0)} ms, boot RSS ${(b.rss/1024).toFixed(0)} MB, before kill ${(rss/1024).toFixed(0)} MB (hwm ${(hwm/1024).toFixed(0)}), acked +${acked.size-ackedBefore}, orphans ${left.length}`);
  if(r%5===0||r===rounds){const v=await verifyLog(rig.home,{acked,expectFragments:kills});log(`  verify @${r}: lines=${v.lines} bad=${v.bad.length} missing=${v.missing.length} dups=${v.dups.length}`);check(`after round ${r}: acked present once, no duplicates`,v.missing.length===0&&v.dups.length===0&&v.multi.length===0,{missing:v.missing.length,dups:v.dups.length})}}
 await finish()};
if(!S[scenario]){console.error('unknown scenario',scenario);process.exit(2)}
const t0=Date.now();
try{await S[scenario]()}catch(e){check('scenario ran to completion',false,String(e?.stack??e))}
R.seconds=Math.round((Date.now()-t0)/1000);R.passed=R.failures.length===0&&!R.skipped;
const bs=R.rounds_data.map(x=>x.bootMs).filter(Boolean);if(bs.length)R.bootMs={n:bs.length,p50:pct(bs,50),p95:pct(bs,95),max:Math.max(...bs)};
writeFileSync(`${out}/${scenario}.json`,JSON.stringify(R,null,1));
log(R.passed?'PASS':R.skipped?'SKIPPED':'FAIL',`${R.assert.length-R.failures.length}/${R.assert.length} assertions`,R.failures.map(f=>f.name).slice(0,8).join(' | '));
process.exit(R.passed?0:R.skipped?77:1);
