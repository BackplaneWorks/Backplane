// Recent and optional plain-thread helper-free smoke and RSS sample on a throwaway SYNTHETIC log copy.
// bun test/gpui/root/recent.mjs BINARY SYNTHETIC_LOG [PORT]
import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,mkdtempSync,writeFileSync,readFileSync,chmodSync,rmSync,statSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {decode,encode} from '../wire.mjs';
const [bin,source,port='3896']=process.argv.slice(2);
if (!bin || !source || !basename(source).startsWith('synthetic-')) throw Error('binary and synthetic log required');
if (process.env.BACKPLANE_POLICY_BIN || process.env.BACKPLANE_WINDOW_BIN) throw Error('unset helper environment before test');
const scratch=resolve(process.env.HOME,'.backplane-gpui-scratch');
mkdirSync(scratch,{recursive:true,mode:0o700});
const home=mkdtempSync(`${scratch}/root-recent-`); chmodSync(home,0o700);
const fake=`${home}/standins`; mkdirSync(fake,{mode:0o700});
for(const n of ['claude','codex','grok','muse']){writeFileSync(`${fake}/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${fake}/${n}`,0o700);}
execFileSync('cp',['--reflink=auto',source,`${home}/events.jsonl`]);
const out=resolve('build/gpui/results/root'); mkdirSync(out,{recursive:true});
const eventsBytes=statSync(source).size;
const env={...process.env,TZ:'Etc/UTC',BACKPLANE_COPY:'1',BACKPLANE_PASSIVE:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_AGENTS:fake,BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',DISPLAY:'',WAYLAND_DISPLAY:'',PATH:`${fake}:${process.env.PATH}`};
const start=performance.now();
const proc=spawn('systemd-run',['--user','--scope','--quiet','-p','MemoryMax=3700M','-p','MemorySwapMax=0',resolve(bin),'--home',home,'--port',port,'--no-tailscale','--foreground'],{env,stdio:['ignore','pipe','pipe']});
let pid=0,ws,log=''; const timer=setTimeout(()=>{if(pid)process.kill(pid,'SIGKILL');process.exitCode=1;console.error('root recent timeout');},120000);
const exited=new Promise(r=>proc.on('exit',(code,signal)=>r({code,signal})));
const ready=new Promise((res,rej)=>{
 let ready=false;
 for(const stream of [proc.stdout,proc.stderr]) stream.on('data',b=>{log+=b.toString();if(!ready&&log.includes('Backplane listening on')){ready=true;res();}});
 proc.once('exit',code=>{if(!ready)rej(Error(`hub exited ${code}: ${log.slice(-1000)}`));});
});
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const rss=()=>{const s=readFileSync(`/proc/${pid}/status`,'utf8');const n=k=>Number(new RegExp(`^${k}:\\s+(\\d+)`,'m').exec(s)?.[1]??0);return {rssKiB:n('VmRSS'),anonKiB:n('RssAnon'),fileKiB:n('RssFile'),peakKiB:n('VmHWM')};};
const summary=xs=>{const a=xs.map(x=>x.rssKiB).sort((a,b)=>a-b);return {samples:a.length,medianMiB:a[Math.floor(a.length/2)]/1024,p95MiB:a[Math.min(a.length-1,Math.floor(a.length*.95))]/1024,anonMiB:xs.at(-1).anonKiB/1024,fileMiB:xs.at(-1).fileKiB/1024,peakMiB:xs.at(-1).peakKiB/1024};};
const sample=async()=>{const xs=[];for(let i=0;i<5;i++){xs.push(rss());await delay(200);}return summary(xs);};
try {
 await ready;
 pid=Number(readFileSync(`${home}/hub.lock`,'utf8').split(' ')[0]);
 const vars=readFileSync(`/proc/${pid}/environ`,'utf8').split('\0');
 if(!vars.includes(`BACKPLANE_AGENTS=${fake}`)) throw Error('stand-in environment not installed on hub');
 const cgroup=readFileSync(`/proc/${pid}/cgroup`,'utf8').trim().split('::')[1];
 const cap=readFileSync(`/sys/fs/cgroup${cgroup}/memory.max`,'utf8').trim(),swap=readFileSync(`/sys/fs/cgroup${cgroup}/memory.swap.max`,'utf8').trim();
 if(Number(cap)>3700*1024**2||swap!=='0')throw Error('scope memory cap not installed');
 // The listener precedes the lean policies' boot fold. Allow that fold to finish before the before-screen sample.
 await delay(30000); const before=await sample();
 const token=readFileSync(`${home}/token`,'utf8').trim(),url=`http://127.0.0.1:${port}`;
 ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&token=${encodeURIComponent(token)}`);ws.binaryType='arraybuffer';
 let greet,viewId='',session={sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},id=2**47+1,revision=0;
 const waiting=new Map();
 const greeting=new Promise((res,rej)=>{ws.onerror=rej;ws.onmessage=e=>{const v=decode(e.data);if(v.t==='view'){greet=v;res();}if(v.t==='reply'){const w=waiting.get(v.id);if(w){waiting.delete(v.id);w(v);}}};});
 await greeting;
 let target={kind:'recent',id:''};
 const req=async(m,extra={})=>{const requestId=id++;const response=new Promise(r=>waiting.set(requestId,r));const t=performance.now();
  ws.send(encode({id:requestId,m,p:{nativeSidebar:1,version:1,hub:url,generation:greet.generation,epoch:greet.epoch,requestRevision:++revision,target,viewId,session,cid:'root-recent',...extra}}));
  const r=await response;if(r.ok){viewId=r.result.viewId;session=r.result.session;}return {r,ms:performance.now()-t};};
 const opened=await req('view.open');
 if(!opened.r.ok||!Array.isArray(opened.r.result?.shown?.projects))throw Error(`recent open failed: ${JSON.stringify(opened.r.error)}`);
 const shown=opened.r.result.shown;
 if(shown.thread!==null||shown.nativeSidebar!==1)throw Error('wrong recent root');
 const after=await sample();
 const updates=[];for(const m of ['view.update','view.page']){const r=await req(m);if(!r.r.ok)throw Error(`${m} failed`);updates.push({method:m,ms:r.ms});}
 const next=shown.sidebarPages?.[0]?.projects?.next;
 if(next){const page=await req('view.action',{action:next.action,value:next.value});if(!page.r.ok||page.r.result.shown.sidebarPages[0].projects.offset!==8)throw Error('project cursor did not advance');}
 const bad=await req('view.action',{action:'native-sidebar-page',value:'forged'});if(bad.r.ok||bad.r.error?.code!=='INVALID_REQUEST')throw Error('forged sidebar cursor was not refused');
 const threadChecks=[];
 if(process.env.ROOT_THREAD_ID){
  const thread=process.env.ROOT_THREAD_ID; target={kind:'thread',id:thread}; viewId='';
  const openedThread=await req('view.open');
  if(!openedThread.r.ok||openedThread.r.result.shown?.thread?.id!==`${url}|${thread}`)throw Error(`thread open failed: ${JSON.stringify(openedThread.r.error)}`);
  const firstWindow=openedThread.r.result.window,firstEntries=openedThread.r.result.shown.thread.entries;
  if(!firstEntries.length||!firstWindow.ids.length)throw Error('thread entries/window missing');
  threadChecks.push({method:'view.open',ms:openedThread.ms,entries:firstEntries.length,windowIds:firstWindow.ids.length,total:firstWindow.total,remaining:firstWindow.remaining});
  const updatedThread=await req('view.update');if(!updatedThread.r.ok)throw Error('thread update failed');
  threadChecks.push({method:'view.update',ms:updatedThread.ms});
  if(firstWindow.older){
   const older=await req('view.page',{cursor:firstWindow.older});if(!older.r.ok)throw Error(`older thread page failed: ${JSON.stringify(older.r.error)}`);
   if(JSON.stringify(older.r.result.window.ids)===JSON.stringify(firstWindow.ids))throw Error('older cursor did not change window');
   if(older.r.result.shown.thread.earlier!==older.r.result.window.remaining)throw Error('earlier does not match the native window');
   threadChecks.push({method:'view.page',ms:older.ms,windowIds:older.r.result.window.ids.length,newer:older.r.result.window.newer,remaining:older.r.result.window.remaining});
  }
  const draft=await req('view.action',{action:'draft',value:'unsupported draft'});
  if(draft.r.ok||draft.r.error?.code!=='PRESENTER_UNAVAILABLE'||!draft.r.error.retryable)throw Error('nonempty draft falsely acknowledged');
  target={kind:'recent',id:''};viewId='';if(!(await req('view.open')).r.ok)throw Error('return to recent failed');
 }
 const unsupported=[];
 for(const kind of ['thread','bot','room','project']){const r=await req('view.open',{target:{kind,id:'unported'}});if(r.r.ok||r.r.error?.code!=='PRESENTER_UNAVAILABLE'||r.r.error.retryable!==true)throw Error(`unported ${kind} was claimed`);unsupported.push({kind,retryable:r.r.error.retryable});}
 const children=readFileSync(`/proc/${pid}/task/${pid}/children`,'utf8').trim().split(/\s+/).filter(Boolean).map(Number);
 if(children.length)throw Error(`unexpected child processes after views: ${children.length}`);
 const usage=execFileSync('python3',['-c','import json,sys; f=open(sys.argv[1]); f.seek(int(sys.argv[2])); print(sum(1 for l in f if json.loads(l).get("$")=="TurnUsage"))',`${home}/events.jsonl`,String(eventsBytes)],{encoding:'utf8'}).trim();
 const result={machine:'desktop',commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),dirty:execFileSync('git',['status','--porcelain'],{encoding:'utf8'}).length>0,eventsBytes,scope:{memoryMax:cap,swapMax:swap},before,after,firstOpenMs:opened.ms,updates,projects:shown.projects.length,rows:shown.projects.reduce((n,p)=>n+['threads','settled','snoozed','archived'].reduce((n,k)=>n+(p[k]?.length??0),0),0),bodyBytes:Buffer.byteLength(JSON.stringify(opened.r.result)),threadChecks,unsupported,childrenAfter:children.length,turnUsageEvents:Number(usage),elapsedMs:performance.now()-start};
 writeFileSync(`${out}/${process.env.ROOT_RESULT_NAME||"recent-rss"}.json`,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
} finally {
 clearTimeout(timer);try{ws?.close();}catch{}if(pid){try{process.kill(pid,'SIGTERM');}catch{}}await exited;
 writeFileSync(`${out}/hub.log`,log,{mode:0o600});rmSync(home,{recursive:true,force:true});
}
