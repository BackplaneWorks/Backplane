// Shared helpers for the perf stream: hub launch in a capped scope, ws client, /proc sampling.
import {spawn} from 'node:child_process';
import {mkdirSync,readFileSync,readdirSync,writeFileSync,chmodSync,existsSync,statSync} from 'node:fs';
import {decode,encode} from '../wire.mjs';
import {standinScript} from '../tools/perf_standin.ts';
export const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export const pct=(xs,p)=>{if(!xs.length)return NaN;const s=[...xs].sort((a,b)=>a-b);return s[Math.min(s.length-1,Math.floor(p/100*s.length))]};
export const f1=x=>Number.isFinite(x)?x.toFixed(1):'-';
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
export const rssKb=pid=>Number(/VmRSS:\s+(\d+)/.exec(rd(`/proc/${pid}/status`))?.[1]??NaN);
export const hwmKb=pid=>Number(/VmHWM:\s+(\d+)/.exec(rd(`/proc/${pid}/status`))?.[1]??NaN);
export const threads=pid=>Number(/Threads:\s+(\d+)/.exec(rd(`/proc/${pid}/status`))?.[1]??NaN);
export const cpuTicks=pid=>{const s=rd(`/proc/${pid}/stat`);if(!s)return NaN;const f=s.split(') ')[1].split(' ');return Number(f[11])+Number(f[12])};
export const fds=pid=>{try{return readdirSync(`/proc/${pid}/fd`).length}catch{return NaN}};
export const sockets=pid=>{try{let n=0;for(const f of readdirSync(`/proc/${pid}/fd`)){try{if(require('node:fs').readlinkSync(`/proc/${pid}/fd/${f}`).startsWith('socket:'))n++}catch{}}return n}catch{return NaN}};
// descendants of pid (children, recursively)
export function tree(pid){const kids=new Map();for(const d of readdirSync('/proc')){if(!/^\d+$/.test(d))continue;const s=rd(`/proc/${d}/stat`);if(!s)continue;const pp=Number(s.split(') ')[1].split(' ')[1]);(kids.get(pp)??kids.set(pp,[]).get(pp)).push(Number(d))}
 const out=[];const go=p=>{for(const k of kids.get(p)??[]){out.push(k);go(k)}};go(pid);return out}
export const treeRssKb=pid=>[pid,...tree(pid)].reduce((a,p)=>a+(rssKb(p)||0),0);
export const size=p=>{try{return statSync(p).size}catch{return 0}};
export function dirSizes(home){const o={};for(const f of ['events.jsonl','store.db','store.db-wal','store.db-shm'])o[f]=size(`${home}/${f}`);return o}
export function makeStandins(dir,env={}){mkdirSync(dir,{recursive:true});writeFileSync(`${dir}/claude`,standinScript);chmodSync(`${dir}/claude`,0o755);
 for(const n of['codex','grok','muse']){writeFileSync(`${dir}/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${dir}/${n}`,0o755)}}
export async function startHub({bin,home,port,standins,host,extraEnv={},extraArgs=[],cap='3700M',log,unsetEnv=[]}){
 if(home.includes('.backplane-bend')||home.includes('/.backplane/'))throw Error('live home refused');
 mkdirSync(home,{recursive:true});
 const direct=!!process.env.NO_SCOPE; // on Boat the sandbox is the hard cap; on the desktop every hub runs in its own capped scope
 const args=direct?[]:['--user','--scope','--quiet','-p',`MemoryMax=${cap}`,'-p','MemorySwapMax=0',...(unsetEnv.length?['env',...unsetEnv.flatMap(k=>['-u',k])]:[]),bin];if(direct)args.push('--home',home,'--port',String(port),'--no-tailscale','--foreground');else args.push('--home',home,'--port',String(port),'--no-tailscale','--foreground');
 if(host)args.push('--host',host);
 for(const a of extraArgs)args.push(a); // e.g. the Rust hub's --policy-bin and --window-bin
 const env={...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_PEERS:'',BACKPLANE_AGENTS:standins,PATH:`${standins}:${process.env.PATH}`,FAKE_LOG:`${home}/fake.log`,...extraEnv};
 for(const k of unsetEnv)if(direct)delete env[k];delete env.BUN_BE_BUN; // the script runner may be the hub binary itself in bun mode; the hub it starts must not be
 const proc=direct?spawn(bin,args,{env,stdio:['ignore',log?'pipe':'ignore',log?'pipe':'ignore']}):spawn('systemd-run',args,{env,stdio:['ignore',log?'pipe':'ignore',log?'pipe':'ignore']});
 if(log){const fs=require('node:fs');const o=fs.openSync(log,'a');proc.stdout.on('data',d=>fs.writeSync(o,d));proc.stderr.on('data',d=>fs.writeSync(o,d))}
 const h=host||'127.0.0.1';
 let seenPid=NaN;const tStart=Date.now();
 for(let i=0;i<9000;i++){try{if((await fetch(`http://${h}:${port}/hello`)).status===200)break}catch{}
  if(i%10===0){const m=/^(\d+)/.exec(rd(`${home}/hub.lock`));if(m){const q=Number(m[1]);if(q!==seenPid)seenPid=q}
   if(Number.isFinite(seenPid)&&!existsSync(`/proc/${seenPid}`)&&Date.now()-tStart>3000){const e=new Error('hub exited before answering /hello after '+((Date.now()-tStart)/1000).toFixed(0)+' s (pid '+seenPid+')');e.hubExited=true;throw e}}
  await sleep(100)}
 // the hub pid: read hub.lock "<pid> <port> flock"
 let pid=NaN;for(let i=0;i<50;i++){const m=/^(\d+)/.exec(rd(`${home}/hub.lock`));if(m){pid=Number(m[1]);break}await sleep(100)}
 // COMMON.md rule: verify from the hub's own environ that it runs stand-in agents only, before any turn
 try{const env=readFileSync(`/proc/${pid}/environ`,'utf8').split('\0');const a=env.find(x=>x.startsWith('BACKPLANE_AGENTS='));if(!a||a.slice(17)!==standins){try{process.kill(pid,'SIGTERM')}catch{}throw Error('hub does not run stand-in agents only: '+a)}}catch(e){if(String(e).includes('stand-in'))throw e;throw Error('cannot verify BACKPLANE_AGENTS of hub '+pid+': '+e)}
 return {proc,pid,home,port,host:h,token:rd(`${home}/token`).trim(),url:`http://${h}:${port}`};
}
export function stopHub(h){try{process.kill(h.pid,'SIGTERM')}catch{}}
// Plain ws client (log + deltas), like agents_load.
export async function client(h,{view=false,onMsg,since,origin}={}){
 const ws=new WebSocket(`ws://${h.host}:${h.port}/ws?${view?'view=1&mobile=1&':''}${since!==undefined?`since=${since}&origin=${origin??1785000000}&`:''}token=${encodeURIComponent(h.token)}`);ws.binaryType='arraybuffer';
 const c={ws,waiters:new Map(),seen:[],logItems:[],replies:new Map(),id:0,bytes:0,msgs:0,counting:true,lastMsg:Date.now()};
 ws.onmessage=e=>{const now=Date.now();const text=typeof e.data==='string';const n=text?e.data.length:e.data.byteLength;let o;try{o=text?JSON.parse(e.data):decode(e.data)}catch{return}
  if(!o||typeof o!=='object')return;if(c.counting){c.bytes+=n;c.msgs++}c.lastMsg=now;if(o.t==='log'&&o.origin!==undefined){c.origin=o.origin;c.logSince=o.since}
  for(const it of o.items??[]){c.seen.push(it);if(o.t==='log'||o.t==='changes')c.logItems.push(it);else (c.otherItems??=[]).push([o.t,it])}if(o.t==='reply'){c.replies.set(Number(o.id),{now,o});const w=c.waiters.get(Number(o.id));if(w){c.waiters.delete(Number(o.id));w({now,o})}}
  onMsg?.(o,now,n)};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
 c.send=(m,p)=>{const i=++c.id;ws.send(encode({id:i,m,p}));return i};
 c.until=async(ms,f)=>{const end=Date.now()+ms;for(;;){const v=f();if(v)return v;if(Date.now()>end)return undefined;await sleep(2)}};
 // event-driven: resolved in onmessage (the old version polled with sleep(20), which quantised every timed RPC to ~20 ms)
 c.rpc=(m,p,ms=30000)=>new Promise(res=>{const t0=performance.now();const i=++c.id;
  const to=setTimeout(()=>{c.waiters.delete(i);res(undefined)},ms);
  c.waiters.set(i,({now,o})=>{clearTimeout(to);res({ms:performance.now()-t0,o})});
  ws.send(encode({id:i,m,p}))});
 return c;
}
export async function debugPerf(h){return await (await fetch(`${h.url}/debug/perf?token=${encodeURIComponent(h.token)}`)).text()}
