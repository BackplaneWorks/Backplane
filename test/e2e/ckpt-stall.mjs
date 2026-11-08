// The hub keeps answering while a turn waits for the earlier turn's checkpoint.   bun test/e2e/ckpt-stall.mjs HUB_BIN OUT [port]
// A git that is slow at `write-tree` while a flag file exists makes the end capture of turn 1 take SLOW seconds; turn 2 is sent at once and waits for it (the "Saving the last turn's
// checkpoint…" activity appears). Meanwhile a SECOND client and the SAME client time setting.set and threads.list, and a turn on another thread is started: all must answer in about a second.
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,copyFileSync,openSync} from 'node:fs';
import {spawn,execFileSync} from 'node:child_process';
import {encode,decode} from '../wire.mjs';
const [BIN,OUT,PORT_ARG]=process.argv.slice(2);const PORT=Number(PORT_ARG||3884);const SLOW=Number(process.env.SLOW||10);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));const HERE=new URL('.',import.meta.url).pathname;
rmSync(OUT,{recursive:true,force:true});for(const d of['home','bin','proj'])mkdirSync(`${OUT}/${d}`,{recursive:true});
copyFileSync(HERE+'lib/standin_rows.py',OUT+'/bin/claude');chmodSync(OUT+'/bin/claude',0o755);
const FLAG=OUT+'/git.slow';writeFileSync(OUT+'/bin/git',`#!/bin/sh\n# a git that is slow at write-tree while ${FLAG} exists\n[ -f "${FLAG}" ] && case "$*" in *write-tree*) sleep ${SLOW};; esac\nexec /usr/bin/git "$@"\n`);chmodSync(OUT+'/bin/git',0o755);
const git=(...a)=>execFileSync('/usr/bin/git',['-c','user.name=t','-c','user.email=t@t',...a],{cwd:OUT+'/proj'});
git('init','-q','-b','main');writeFileSync(OUT+'/proj/a.txt','one\n');git('add','.');git('commit','-qm','init');
const now=Math.floor(Date.now()/1000);
writeFileSync(OUT+'/home/events.jsonl',[{$:'ProjectCreated',id:'p1',title:'ck',root:OUT+'/proj',at:now},...[1,2].map(i=>({$:'ThreadCreated',id:'t'+i,project:'p1',title:'T'+i,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:now+i}))].map(e=>JSON.stringify(e)+'\n').join(''));
const env={...process.env,BACKPLANE_AGENTS:OUT+'/bin',PATH:`${OUT}/bin:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/sh',FAKE_LOG:OUT+'/fake.log',FAKE_SECS:'600'};
const hub=spawn(BIN,['--home',OUT+'/home','--port',String(PORT),'--no-tailscale','--foreground'],{env,stdio:['ignore',openSync(OUT+'/hub.log','a'),openSync(OUT+'/hub.log','a')],cwd:OUT+'/proj'});
for(let i=0;i<300;i++){try{if((await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200)break}catch{}await sleep(100)}
const token=readFileSync(OUT+'/home/token','utf8').trim();
async function connect(){const ws=new WebSocket(`ws://127.0.0.1:${PORT}/ws?token=${token}`);ws.binaryType='arraybuffer';const c={ws,replies:new Map(),id:100};
  ws.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}if(o.t==='reply')c.replies.set(o.id,o)};await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.send=(m,p={})=>{const n=++c.id;ws.send(encode({id:n,m,p}));return n};
  c.timed=async(m,p,max=30000)=>{const t=Date.now();const n=c.send(m,p);while(!c.replies.has(n)&&Date.now()-t<max)await sleep(20);return {ms:Date.now()-t,ok:c.replies.get(n)?.ok,got:c.replies.has(n)}};return c};
const events=()=>readFileSync(OUT+'/home/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(Boolean);
const until=async(f,ms=30000)=>{const end=Date.now()+ms;while(Date.now()<end){if(f())return true;await sleep(50)}return false};
const A=await connect(),B=await connect();await sleep(1000);
writeFileSync(FLAG,'');                                           // from now every write-tree takes SLOW seconds
A.send('turn.start',{thread:'t1',text:'zqshort one'});
await until(()=>events().some(e=>e.$==='TurnChanged'&&e.thread==='t1'&&e.state==='completed'),60000);
const t0=Date.now();const n2=A.send('turn.start',{thread:'t1',text:'zqshort two'});      // turn 2 at once: it waits for turn 1's end capture
const said=await until(()=>events().some(e=>e.$==='ActivityLogged'&&/Saving the last turn's checkpoint/.test(e.summary||'')),20000);
console.log(`turn 2 sent; the waiting activity appeared after ${Date.now()-t0} ms: ${said}`);
const probe=[];
for(const [who,c] of [['second client',B],['same client',A]]){
  probe.push([who+' setting.set',await c.timed('setting.set',{key:'restart.continue',value:'off'})]);
  probe.push([who+' threads.list',await c.timed('threads.list',{limit:10,archived:false})]);
}
const turn2done=A.replies.has(n2);const other=await B.timed('turn.start',{thread:'t2',text:'zqshort other'});
const setOk=events().some(e=>e.$==='SettingSet'&&e.key==='restart.continue'&&e.value==='off');
for(const [k,v] of probe)console.log(`${v.ms>2000?'STALL':'ok   '} ${k}: ${v.ms} ms ${v.got?'answered':'NO ANSWER'}`);
console.log(`${other.ms>2000?'STALL':'ok   '} turn.start on another thread: ${other.ms} ms; setting written=${setOk}; turn 2's own reply came before the probes ended=${turn2done}`);
await until(()=>A.replies.has(n2),SLOW*3000);console.log(`turn 2's reply came ${Date.now()-t0} ms after it was sent (the capture takes ~${SLOW} s)`);
const run=await until(()=>events().filter(e=>e.$==='TurnChanged'&&e.thread==='t1'&&e.state==='running').length>=2,20000);console.log(`turn 2 started after the capture: ${run}`);
hub.kill();process.exit(0);
