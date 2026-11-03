// Owner-supplied PRIVATE copied store, never the installed home or binary.
// No fixture or reply body enters results. Build once elsewhere, then supply
// its exact service SHA; no compiler, live target, pairing or provider path.
import {createReadStream,readFileSync,writeFileSync,mkdirSync,mkdtempSync,rmSync,chmodSync,openSync,closeSync,existsSync,lstatSync} from 'node:fs';
import {createHash,randomBytes} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import assert from 'node:assert/strict';
import {cbor,plain} from '../../src/web/wire.js';
import {ThinViews} from '../../src/web/thin.js';
const {verdict}=require('./real_store_policy.cjs');
const opts=new Map<string,string>();
for(let i=2;i<process.argv.length;i+=2){const k=process.argv[i],v=process.argv[i+1];assert(['--bin','--sha256','--fixture','--out','--seconds'].includes(k)&&v&&!opts.has(k));opts.set(k,v);}
for(const k of ['--bin','--sha256','--fixture','--out'])assert(opts.has(k));
const bin=resolve(opts.get('--bin')!),fixture=resolve(opts.get('--fixture')!),out=resolve(opts.get('--out')!);
const seconds=Number(opts.get('--seconds')||1800);assert(seconds>=30&&seconds<=3600&&/^[a-f0-9]{64}$/.test(opts.get('--sha256')!));
assert(!bin.includes('/.local/bin/')&&!bin.includes('/.local/share/backplane/'),'use an immutable artifact copy');
mkdirSync(dirname(out),{recursive:true});
const root=mkdtempSync(join(tmpdir(),'bp-real-recovery-'));chmodSync(root,0o700);
const home=join(root,'home'),user=join(root,'user'),fake=join(root,'standins');
for(const p of [home,user,fake])mkdirSync(p,{mode:0o700});
const hash=async(p:string)=>{const h=createHash('sha256');for await(const b of createReadStream(p))h.update(b);return h.digest('hex');};
const mono=()=>Number(process.hrtime.bigint())/1e6,sleep=(ms:number)=>new Promise(r=>setTimeout(r,ms));
const report:any={schema:2,scope:'owned-copy-of-real-store',seconds,copyMode:true,networkFence:true,forcedGC:false,inspector:false,ok:false};
let child:ReturnType<typeof Bun.spawn>|undefined,phase='prepare',timer:ReturnType<typeof setInterval>|undefined,fault:Error|undefined;
const sockets:WebSocket[]=[],samples:any[]=[];
const samplesFd=openSync(out+'.samples.jsonl','w',0o600);
function proc(){
 assert(child);const status=readFileSync(`/proc/${child.pid}/status`,'utf8'),m:any={};
 for(const line of status.split('\n'))if(/^Vm(RSS|HWM|Swap):/.test(line))m[line.split(':')[0]]=Number(line.match(/\d+/)![0]);
 const stat=readFileSync(`/proc/${child.pid}/stat`,'utf8').split(') ')[1].split(' ');m.cpuTicks=Number(stat[11])+Number(stat[12]);return m;
}
async function until(f:()=>boolean,deadline:number){while(!f()){if(fault)throw fault;if(child?.exitCode!==null)throw Error('child-exited');if(mono()>deadline)throw Error('deadline');await sleep(50)}}
const dictSource=readFileSync(resolve(import.meta.dir,'../../src/core/cbor.bend'),'utf8');
function dictionary(name:string){const p=dictSource.indexOf(`def Cbor.${name}()`);assert(p>=0);return JSON.parse(dictSource.slice(dictSource.indexOf('[',p),dictSource.indexOf(']',p)+1));}
const keys=dictionary('keys'),words=dictionary('words');
function encode(value: any): Uint8Array {
  const out: number[] = [];
  const head = (major: number, n: number) => {
    if (n < 24) out.push(major << 5 | n);
    else if (n < 256) out.push(major << 5 | 24, n);
    else if (n < 65536) out.push(major << 5 | 25, n >> 8, n & 255);
    else out.push(major << 5 | 26, n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255);
  };
  const put = (v: any) => {
    if (v === null) out.push(246);
    else if (typeof v === "boolean") out.push(v ? 245 : 244);
    else if (typeof v === "number") { out.push(199); put(String(v)); }
    else if (typeof v === "string") { const b = new TextEncoder().encode(v); head(3, b.length); for (const byte of b) out.push(byte); }
    else if (Array.isArray(v)) { head(4, v.length); for (const x of v) put(x); }
    else { const entries = Object.entries(v); head(5, entries.length); for (const [k, x] of entries) { put(k); put(x); } }
  };
  put(value); return new Uint8Array(out);
}

async function view(port:number,token:string,target:any){
 const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}&view=1&frontend=1`);sockets.push(ws);ws.binaryType='arraybuffer';
 const views=new ThinViews({now:()=>performance.now(),sidebarRegions:true,viewDelta:true,send:(_:string,r:any)=>r,command:(c:any)=>c});
 let greeting:any,session:any={sel:target.kind==='thread'?target.id:'',rid:1,drafts:{},open:{},info:{},adding:false,panel:false},reply:any;
 const metrics={requests:0,replies:0,totalBytes:0,maxReplyBytes:0,maxRttMs:0};let pending=0;
 ws.onerror=()=>{fault=Error('view-transport')};ws.onclose=()=>{if(phase!=='cleanup')fault=Error('view-closed')};
 ws.onmessage=e=>{try{
  const bytes=typeof e.data==='string'?Buffer.byteLength(e.data):e.data.byteLength;metrics.totalBytes+=bytes;metrics.maxReplyBytes=Math.max(metrics.maxReplyBytes,bytes);
  const r=typeof e.data==='string'?JSON.parse(e.data):plain(cbor(new Uint8Array(e.data as ArrayBuffer),keys,words));
  if(r.t==='view')greeting=r;
  if(r.t==='view.control'){views.control('fixture',r);return;}
  if(r.t==='reply'&&r.id===pending)reply=r;
 }catch{fault=Error('view-protocol')}};
 await until(()=>ws.readyState===WebSocket.OPEN&&!!greeting,mono()+10000);
 assert(views.connect('fixture',greeting).connected);
 async function issue(call:any){
  assert(call?.id&&call.cmds?.length===1);pending=call.id;reply=undefined;const start=mono();metrics.requests++;
  ws.send(encode(call.cmds[0]));await until(()=>!!reply,mono()+15000);
  const taken=views.reply('fixture',reply);assert(taken?.current&&taken.remoteReady&&taken.shown);session=taken.session;
  metrics.replies++;metrics.maxRttMs=Math.max(metrics.maxRttMs,mono()-start);
 }
 await issue(views.select('fixture',target,session,'copy-soak'));
 return{metrics,update:()=>issue(views.request('fixture','view.update',{session,anchor:{id:'',offset:0}}))};
}
try{
 report.binarySha256=await hash(bin);assert.equal(report.binarySha256,opts.get('--sha256'),'artifact mismatch');
 const listing=Bun.spawnSync(['tar','-tzf',fixture]);assert.equal(listing.exitCode,0);
 const names=listing.stdout.toString().trim().split('\n'),allowed=['RECEIPT.json','events.jsonl','store.db','providers/accounts-v1/profiles.json'];
 assert(names.every(n=>allowed.includes(n))&&new Set(names).size===names.length&&allowed.slice(0,3).every(n=>names.includes(n)));
 const extracted=Bun.spawnSync(['tar','-xzf',fixture,'--no-same-owner','--no-same-permissions','-C',home]);assert.equal(extracted.exitCode,0);
 for(const name of names){const st=lstatSync(join(home,name));assert(st.isFile()&&!st.isSymbolicLink()&&st.nlink===1)}
 const receipt=JSON.parse(readFileSync(join(home,'RECEIPT.json'),'utf8'));
 assert(receipt.private&&Number(receipt.meta.applied_seq)>=1400000);assert.equal(await hash(join(home,'events.jsonl')),receipt.logSha256);
 report.fixture={meta:receipt.meta,counts:receipt.counts,logSha256:receipt.logSha256};
 const {Database}=await import('bun:sqlite');const db=new Database(join(home,'store.db'),{readonly:true});
 const target:any=db.query('SELECT id FROM thread ORDER BY activity DESC LIMIT 1').get();assert(target?.id);db.close();
 const token=randomBytes(32).toString('hex');writeFileSync(join(home,'token'),token,{mode:0o600});
 for(const name of ['claude','codex','grok','muse','opencode','agy']){const p=join(fake,name);writeFileSync(p,'#!/bin/sh\nexit 127\n');chmodSync(p,0o700)}
 const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port=listener.port;listener.stop(true);assert(port!==3787);
 phase='boot';const t0=mono(),logFd=openSync(join(root,'stderr'),'w',0o600);
 child=Bun.spawn([bin,'--home',home,'--host','127.0.0.1','--port',String(port),'--no-tailscale','--headless','--foreground'],{
  cwd:root,env:{HOME:user,USERPROFILE:user,XDG_CONFIG_HOME:join(user,'config'),XDG_CACHE_HOME:join(user,'cache'),XDG_DATA_HOME:join(user,'data'),
   PATH:`${fake}:${dirname(process.execPath)}:/usr/bin:/bin`,TMPDIR:root,BACKPLANE_HOME:home,BACKPLANE_COPY:'1',BACKPLANE_AGENTS:fake,
   BACKPLANE_ADOPT:'0',BACKPLANE_PEERS:'',BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BEND_NO_TELEMETRY:'1',
   BUN_OPTIONS:`--preload=${resolve(import.meta.dir,'real_store_fence.cjs')}`,BUN_JSC_maxPerThreadStackUsage:'268435456',DISPLAY:'',WAYLAND_DISPLAY:'',CI:'1'},stdout:logFd,stderr:logFd});closeSync(logFd);
 report.hubPid=child.pid;
 const sample=()=>{try{const s={elapsedMs:mono()-t0,...proc()};samples.push(s);writeFileSync(samplesFd,JSON.stringify(s)+'\n');if(s.VmSwap>0||s.VmRSS>3*1024*1024)fault=Error('resource-boundary')}catch{fault=Error('sample-or-child')}};
 sample();timer=setInterval(sample,5000);
 // Observe the child's own listening log, then issue ONE hello. There is
 // no fixed delay, abandoned-request queue, debugger or heap walk.
 await until(()=>/Backplane listening on http:\/\/|web server on:/.test(readFileSync(join(root,'stderr'),'utf8')),mono()+120000);report.listenObservedMs=mono()-t0;
 const helloAt=mono();const hello=await fetch(`http://127.0.0.1:${port}/hello`,{signal:AbortSignal.timeout(15000)});assert(hello.ok);await hello.arrayBuffer();report.helloRttMs=mono()-helloAt;
 const own=await view(port,token,{kind:'thread',id:target.id}),sidebar=await view(port,token,{kind:'recent',id:''});
 report.usableMs=mono()-t0;report.views=[own.metrics,sidebar.metrics];phase='steady';
 // Both connections serialize their own reads; no accumulating requests.
 while(mono()-t0<seconds*1000){if(fault)throw fault;await Promise.all([own.update(),sidebar.update()]);await sleep(1000)}
 sample();report.finalProc=proc();report.acceptance=verdict(samples,report.usableMs,mono()-t0);report.ok=report.acceptance.ok;
}catch(e){report.failurePhase=phase;report.error=e instanceof Error?e.name:'unknown';report.failureCode=['resource-boundary','child-exited','deadline','view-transport','view-closed','view-protocol','sample-or-child'].find(x=>e instanceof Error&&e.message===x)||'fixture-or-contract';try{report.failureProc=proc()}catch{}}
finally{
 phase='cleanup';clearInterval(timer);for(const s of sockets)s.close();
 if(child&&child.exitCode===null){child.kill('SIGTERM');const timer=setTimeout(()=>{if(child!.exitCode===null)child!.kill('SIGKILL')},3000);await child.exited;clearTimeout(timer)}
 const text=existsSync(join(root,'stderr'))?readFileSync(join(root,'stderr'),'utf8'):'',durations:Record<string,number[]>={};
 for(const line of text.split('\n'))for(const m of line.matchAll(/(hub\.[a-z_]+).*?([0-9]+(?:\.[0-9]+)?)\s*ms/g)){const a=durations[m[1]]||=[];if(a.length<10000)a.push(Number(m[2]))}
 report.slowHandlers=Object.fromEntries(Object.entries(durations).map(([k,v])=>[k,{count:v.length,min:Math.min(...v),max:Math.max(...v)}]));
 report.startupMarkers={hotBoot:/hot boot/i.test(text),listening:/Backplane listening on http:\/\/|web server on:/.test(text),stackError:/RangeError.*stack|Maximum call stack/i.test(text)};
 closeSync(samplesFd);writeFileSync(out,JSON.stringify(report,null,2)+'\n');rmSync(root,{recursive:true,force:true});
 console.log(JSON.stringify({scope:report.scope,ok:report.ok,usableMs:report.usableMs,finalProc:report.finalProc,failurePhase:report.failurePhase,failureCode:report.failureCode}));process.exitCode=report.ok?0:1;
}
