// Soak lane: shared helpers. Runs on Boat (no systemd) or here (SOAK_SCOPE=3700M wraps the hub in a capped scope).
// Everything the client measures is resolved inside onmessage (event driven). Nothing here polls with sleep() to time a
// reply: the 20 ms floor of perf/lib.mjs `until()` was that bug (docs/gpui/RESULTS.md, CORRECTION). selftest.mjs guards it.
import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,readFileSync,readdirSync,writeFileSync,chmodSync,existsSync,statSync,readlinkSync,openSync,readSync,closeSync,createReadStream} from 'node:fs';
import {createHash} from 'node:crypto';
import {decode,encode} from '../wire.mjs';
import {standinScript} from '../../tools/perf_standin.ts';

export const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export const pct=(xs,p)=>{if(!xs.length)return NaN;const s=[...xs].sort((a,b)=>a-b);return s[Math.min(s.length-1,Math.floor(p/100*s.length))]};
export const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const num=(re,s)=>{const m=re.exec(s);return m?Number(m[1]):NaN};

// log-scale histogram (2% buckets): exact enough percentiles over any span without keeping every sample
export const hb=x=>Math.round(Math.log(Math.max(x,0.001))/Math.log(1.02));
export function histOf(a){const h={};for(const x of a){if(!Number.isFinite(x))continue;const b=hb(x);h[b]=(h[b]??0)+1}return h}

// ---- /proc sampling -----------------------------------------------------------------------------------------------
export const rssKb=pid=>num(/VmRSS:\s+(\d+)/,rd(`/proc/${pid}/status`));
export const hwmKb=pid=>num(/VmHWM:\s+(\d+)/,rd(`/proc/${pid}/status`));
export const nthreads=pid=>num(/Threads:\s+(\d+)/,rd(`/proc/${pid}/status`));
export const alive=pid=>{const s=rd(`/proc/${pid}/stat`);if(!s)return false;return s.split(') ')[1][0]!=='Z'};
export const exists=pid=>existsSync(`/proc/${pid}`);
export const cpuTicks=pid=>{const s=rd(`/proc/${pid}/stat`);if(!s)return NaN;const f=s.split(') ')[1].split(' ');return Number(f[11])+Number(f[12])};
export function fdInfo(pid){try{let n=0,sock=0;for(const f of readdirSync(`/proc/${pid}/fd`)){n++;try{if(readlinkSync(`/proc/${pid}/fd/${f}`).startsWith('socket:'))sock++}catch{}}return {fds:n,sockets:sock}}catch{return {fds:NaN,sockets:NaN}}}
// every process as {pid,ppid,state,comm}
export function procs(){const out=[];for(const d of readdirSync('/proc')){if(!/^\d+$/.test(d))continue;const s=rd(`/proc/${d}/stat`);if(!s)continue;const i=s.lastIndexOf(') ');const f=s.slice(i+2).split(' ');out.push({pid:Number(d),ppid:Number(f[1]),state:f[0],comm:s.slice(s.indexOf('(')+1,i)})}return out}
export function descendants(pid,all=procs()){const kids=new Map();for(const p of all)(kids.get(p.ppid)??kids.set(p.ppid,[]).get(p.ppid)).push(p);const out=[];const go=q=>{for(const k of kids.get(q)??[]){out.push(k);go(k.pid)}};go(pid);return out}
// only the hub's own: children/descendants of the hub, or the stand-ins' and policy's names (other processes on a shared machine leave git zombies)
export const zombies=(all=procs(),hubPid)=>{const mine=new Set(hubPid?descendants(hubPid,all).map(p=>p.pid):[]);return all.filter(p=>p.state==='Z'&&(mine.has(p.pid)||p.ppid===hubPid||/^(claude|bp-policy.*)$/.test(p.comm)))};
export const size=p=>{try{return statSync(p).size}catch{return 0}};
export function dirBytes(home){const o={};for(const f of readdirSync(home)){try{const st=statSync(`${home}/${f}`);if(st.isFile())o[f]=st.size}catch{}}return o}
export function diskFreeKb(path){try{return Number(execFileSync('df',['-Pk',path],{encoding:'utf8'}).trim().split('\n').pop().split(/\s+/)[3])}catch{return NaN}}

// ---- stand-in agents ----------------------------------------------------------------------------------------------
export function makeStandins(dir){mkdirSync(dir,{recursive:true});writeFileSync(`${dir}/claude`,standinScript);chmodSync(`${dir}/claude`,0o755);
 for(const n of['codex','grok','muse']){writeFileSync(`${dir}/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${dir}/${n}`,0o755)}}

// ---- synthetic log ------------------------------------------------------------------------------------------------
export const T0=1785000000;
export function seedLines(projectRoot,agents){const L=[JSON.stringify({$:'ProjectCreated',id:'p1',title:'soak',root:projectRoot,at:T0})];
 const ids=[];for(let i=0;i<agents;i++){const id=`ts${String(i).padStart(3,'0')}`;ids.push(id);L.push(JSON.stringify({$:'ThreadCreated',id,project:'p1',title:'soak '+i,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:T0+1+i}))}
 return {lines:L,ids}}
export const originOf=home=>{const fd=openSync(`${home}/events.jsonl`,'r');try{const b=Buffer.alloc(65536);const n=readSync(fd,b,0,65536,0);const s=b.subarray(0,n).toString('utf8');const nl=s.indexOf('\n');if(nl<0)return null;return createHash('sha256').update(s.slice(0,nl)).digest('hex').slice(0,12)}finally{closeSync(fd)}};
// lines = newlines; valid = lines that look like a whole object ('{' first, '}' last): the hub's cursor counts those, not torn fragments
export function countLines(path){return new Promise((res,rej)=>{let n=0,valid=0,last=10,first=-1,prev=10;const s=createReadStream(path);s.on('data',b=>{for(let i=0;i<b.length;i++){const c=b[i];if(c===10){n++;if(first===123&&prev===125)valid++;first=-1}else{if(first<0)first=c}prev=c}last=b[b.length-1]});s.on('end',()=>res({lines:n,valid,endsWithNewline:last===10||n===0}));s.on('error',rej)})}

// ---- the hub as a child -------------------------------------------------------------------------------------------
// Returns {proc,pid,home,port,host,token,url,exited:Promise,bootMs}. pid is the hub's own (hub.lock), also under a scope.
export async function startHub({bin,home,port,standins,policy,extraEnv={},extraArgs=[],log,cap=process.env.SOAK_SCOPE,host='127.0.0.1',helloTimeoutMs=300000}){
 if(/\.backplane-bend|\/\.backplane\/|\.local\/share\/backplane/.test(home))throw Error('live home refused: '+home);
 mkdirSync(home,{recursive:true});
 const args=['--home',home,'--port',String(port),'--no-tailscale','--foreground','--host',host,...extraArgs];
 const env={...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_PEERS:'',BACKPLANE_AGENTS:standins,PATH:`${standins}:${process.env.PATH}`,FAKE_LOG:`${home}/fake.log`,...extraEnv};
 if(policy)env.BACKPLANE_POLICY_BIN=policy;else delete env.BACKPLANE_POLICY_BIN;
 const [cmd,cargs]=cap?['systemd-run',['--user','--scope','--quiet','-p',`MemoryMax=${cap}`,'-p','MemorySwapMax=0',bin,...args]]:[bin,args];
 const prevPid=Number(/^(\d+)/.exec(rd(`${home}/hub.lock`))?.[1]??NaN);
 const lf=log?openSync(log,'a'):null;
 const proc=spawn(cmd,cargs,{env,stdio:['ignore',lf??'ignore',lf??'ignore']});
 const exited=new Promise(r=>proc.on('exit',(code,sig)=>r({code,sig})));
 const t0=Date.now();let up=false,done=false;exited.then(()=>done=true);
 while(Date.now()-t0<helloTimeoutMs){
  try{const r=await fetch(`http://${host}:${port}/hello`);if(r.status===200){up=true;break}}catch{}
  if(done&&!cap){if(lf!==null)closeSync(lf);const e=new Error('hub exited before /hello (see log)');e.hubExited=true;throw e}
  await sleep(25)}
 if(!up)throw Error('hub did not answer /hello in time');
 const bootMs=Date.now()-t0;
 let pid=NaN;for(let i=0;i<200;i++){const m=/^(\d+)/.exec(rd(`${home}/hub.lock`));if(m&&Number(m[1])!==prevPid&&exists(Number(m[1]))){pid=Number(m[1]);break}await sleep(25)}
 // COMMON.md: read BACKPLANE_AGENTS from the hub's own environ before any turn
 const environ=rd(`/proc/${pid}/environ`).split('\0');const a=environ.find(x=>x.startsWith('BACKPLANE_AGENTS='));
 if(!a||a.slice(17)!==standins){try{process.kill(pid,'SIGKILL')}catch{}throw Error('hub does not run stand-in agents only: '+a)}
 if(lf!==null)closeSync(lf);
 return {proc,pid,home,port,host,token:rd(`${home}/token`).trim(),url:`http://${host}:${port}`,exited,bootMs,standins,policy};
}
export const killHub=(h,sig='SIGKILL')=>{try{process.kill(h.pid,sig)}catch{}};

// ---- the event-driven wire client ---------------------------------------------------------------------------------
// Plain wire stream (log, changes, delta). send() is fire and forget; rpc() resolves in onmessage; waitFor() is a list of
// predicates checked once per incoming frame, never a timer loop. now()/performance.now() are read in the handler.
export async function client(h,{since,origin,onFrame,view=false}={}){
 const q=view?'view=1&mobile=1&enc=cbor&':(since!==undefined?`since=${since}&origin=${origin}&`:'');
 const ws=new WebSocket(`ws://${h.host}:${h.port}/ws?${q}token=${encodeURIComponent(h.token)}`);ws.binaryType='arraybuffer';
 const c={ws,id:0,waiters:new Map(),preds:new Set(),frames:0,bytes:0,items:0,last:performance.now(),closed:false,greeting:null,onItems:null};
 c.closed_p=new Promise(r=>{ws.onclose=()=>{c.closed=true;for(const [,w] of c.waiters)w(undefined);c.waiters.clear();r()}});
 ws.onmessage=e=>{const t=performance.now();const wall=Date.now();c.last=t;let o;try{o=decode(e.data)}catch{return}
  if(!o||typeof o!=='object')return;c.frames++;c.bytes+=e.data.byteLength;
  if(o.t==='view')c.greeting=o;
  c.onView?.(o);
  if(o.items){c.items+=o.items.length;c.onItems?.(o.items,o,t)}
  if(o.t==='reply'){const w=c.waiters.get(Number(o.id));if(w){c.waiters.delete(Number(o.id));w({t,o})}}
  onFrame?.(o,t,wall);
  for(const p of c.preds){let v;try{v=p(o)}catch{v=undefined}if(v){c.preds.delete(p);p.res(v)}}};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws error'))});
 c.enc=encode;
 c.send=(m,p)=>{const id=++c.id;ws.send(encode({id,m,p}));return id};
 // rpc: {ms,o} or undefined on timeout/close. The clock starts at send and stops in onmessage.
 c.rpc=(m,p,ms=30000)=>new Promise(res=>{const id=++c.id;const t0=performance.now();
  const to=setTimeout(()=>{c.waiters.delete(id);res(undefined)},ms);
  c.waiters.set(id,r=>{clearTimeout(to);res(r?{ms:r.t-t0,o:r.o}:undefined)});
  try{ws.send(encode({id,m,p}))}catch{clearTimeout(to);c.waiters.delete(id);res(undefined)}});
 // waitFor: resolves with the first truthy predicate result for a frame arriving after the call (or undefined at timeout)
 c.waitFor=(pred,ms)=>new Promise(res=>{const to=setTimeout(()=>{c.preds.delete(pred);res(undefined)},ms);pred.res=v=>{clearTimeout(to);res(v)};c.preds.add(pred)});
 c.close=()=>{try{ws.close()}catch{}};
 return c;
}
// Plain GET with a timeout, for /hello probes. Returns ms or NaN.
export async function hello(h,ms=2000){const t=performance.now();try{const r=await fetch(`${h.url}/hello`,{signal:AbortSignal.timeout(ms)});await r.arrayBuffer();return r.status===200?performance.now()-t:NaN}catch{return NaN}}
// refuses before anything is deleted or started: a live home, a protected port, or the desktop unless SOAK_ALLOW_LOCAL=1 (the desktop is off limits)
export function guard({home,port,paths=[]}){const all=[home,...paths].filter(Boolean).map(p=>String(p));
 for(const p of all)if(/\.backplane-bend|\/\.backplane\/|\.local\/share\/backplane|\.backplane-gpui(?!-scratch)/.test(p))throw Error('live home refused: '+p);
 if([3787,3788,3799].includes(Number(port)))throw Error('protected port '+port);
 if(process.env.SOAK_ALLOW_LOCAL!=='1'&&(/^\/home\/h(\/|$)/.test(process.env.HOME??'')||process.env.USER==='h'))throw Error('refusing to run on the desktop (Boat only); SOAK_ALLOW_LOCAL=1 overrides for a unit check');}
export const jsonl=path=>rd(path).split('\n').filter(Boolean);
