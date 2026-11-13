// Memory scopes, for real (native/host/src/memscope.rs; Linux with a user systemd): a throwaway hub whose agents and jobs run in scopes of their own
// in a test slice, each capped at 400 MB. Thread A's agent starts a child that eats 1.5 GB; thread B runs a long turn at the same time; then A's
// agent starts a work_run job that eats 1.5 GB. Checked: the kernel kills each eater alone (its scope's oom_kill), A's agent goes on and finishes
// its turn, B's turn finishes, the hub answers throughout, and A is told why each died ("Killed for memory", the job's end says it too).
// usage: bun test/hub/memscope_e2e.mjs HUB_BIN PORT OUTDIR
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
const [bin,port,out]=process.argv.slice(2);
const work=out+'/memscope';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec ${process.execPath} ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const SLICE='bpmemscopetest.slice';
const h=await startHub({bin,home,port:Number(port),standins:work+'/bin',log:work+'/hub.log',cap:'2G',
  extraEnv:{FAKE_LOG:work+'/fake.log',BP_STANDIN_MODE:'stream',BP_STREAM_N:'6',BP_STREAM_MS:'100',BACKPLANE_SCOPE_SLICE:SLICE,BACKPLANE_SCOPE_MEM_MAX:'400M',BACKPLANE_SLICE_MEM_MAX:'3G'}});
let fails=0;const check=(what,ok,detail='')=>{console.log(`${ok?'PASS':'FAIL'} ${what}${ok?'':' ('+detail+')'}`);if(!ok)fails++};
const hello=async()=>{try{return (await fetch(`http://127.0.0.1:${port}/hello`)).status===200}catch{return false}};
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}});
const said=(th,re)=>events().filter(e=>e.$==='MessagePosted'&&e.thread===th&&re.test(e.text||''));
const turns=(th)=>events().filter(e=>e.$==='TurnChanged'&&e.thread===th&&e.state==='completed').length;
const kills=()=>{try{return Number(/oom_kill (\d+)/.exec(readFileSync(`/sys/fs/cgroup/user.slice/user-${process.getuid()}.slice/user@${process.getuid()}.service/${SLICE}/memory.events`,'utf8'))[1])}catch{return 0}};
await sleep(3000);const c=await client(h);
const log=readFileSync(work+'/hub.log','utf8');
check('the hub runs agents and jobs in scopes of their own',/memscope: agents and jobs run in scopes of their own in bpmemscopetest.slice/.test(log),log.split('\n').filter(l=>/memscope/.test(l)).join(' | '));
const A='t1700000001-2',B='t1700000002-3';const k0=kills(),a0=turns(A),b0=turns(B);
c.send('turn.start',{thread:B,text:'hold:9000 a long turn beside the eater'});
c.send('turn.start',{thread:A,text:'eatchild:1500 eat'});
await sleep(1500);
{ // what the agents run sits above the hub for a machine-wide OOM: its oom_score_adj is the hub's + 300
  const base=`/sys/fs/cgroup/user.slice/user-${process.getuid()}.slice/user@${process.getuid()}.service/${SLICE}`;let adj=[];
  try{for(const d of (await import('node:fs')).readdirSync(base).filter(d=>d.startsWith('bp-agent-')))for(const p of readFileSync(`${base}/${d}/cgroup.procs`,'utf8').split('\n').filter(Boolean))try{adj.push(Number(readFileSync(`/proc/${p}/oom_score_adj`,'utf8')))}catch{}}catch{}
  const hubAdj=Number(readFileSync(`/proc/${h.pid}/oom_score_adj`,'utf8'));
  check("the agents' processes are in bp-agent scopes, each above the hub's oom_score_adj",adj.length>0&&adj.every(a=>a>=Math.min(1000,hubAdj+300)),`hub ${hubAdj}, agents ${adj}`);
}
let up=true;for(let i=0;i<40;i++){if(!(await hello()))up=false;await sleep(500);if(turns(A)>a0&&turns(B)>b0)break}
const k1=kills();
check('the eating child was killed for memory in its scope',k1>k0,`oom_kill ${k0} -> ${k1}`);
check("A's agent saw its child killed and went on",said(A,/EATCHILD (signal SIGKILL|exit 137)/).length>0,JSON.stringify(said(A,/EATCHILD/).map(e=>e.text)));
check("A's turn finished",turns(A)>a0,`completed ${a0} -> ${turns(A)}`);
check("B's turn, beside it, finished",turns(B)>b0,`completed ${b0} -> ${turns(B)}`);
check('A was told why: killed for memory',said(A,/Killed for memory: python3/).length>0,JSON.stringify(said(A,/memory/).map(e=>e.text.slice(0,200))));
// a work_run job that eats memory: killed alone, its end says why
const a1=turns(A);
c.send('turn.start',{thread:A,text:'mcpcall:'+JSON.stringify({name:'work_run',arguments:{command:'python3 -c "import time; b=[bytearray(64<<20) for _ in range(24)]; time.sleep(30)"',note:'eater job'}})});
let told=[];for(let i=0;i<60;i++){if(!(await hello()))up=false;told=said(A,/eater job/);if(told.some(e=>/Killed for memory/.test(e.text)))break;await sleep(500)}
check("the job's end says it was killed for memory",told.some(e=>/Killed for memory/.test(e.text)),JSON.stringify(told.map(e=>e.text.slice(0,300))));
check('a second kill in the slice',kills()>k1,`oom_kill ${k1} -> ${kills()}`);
check('the hub answered all along',up&&await hello());
const hubPid=h.pid;let alive=false;try{process.kill(hubPid,0);alive=true}catch{}
check('the hub process is the one started',alive,`pid ${hubPid}`);
let scope='';try{scope=execFileSync('systemctl',['--user','show','-p','MemoryMax',SLICE]).toString().trim()}catch{}
check("the test slice has its cap (never the person's bpagent.slice)",/MemoryMax=3221225472/.test(scope),scope);
stopHub(h);await sleep(1500);try{execFileSync('systemctl',['--user','stop',SLICE])}catch{}
console.log(`memscope_e2e: ${fails?'FAILED':'ok'} (${fails} failed) artifacts=${work}`);process.exit(fails?1:0);
