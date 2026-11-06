// node/bun session.mjs NAME BIN PORT LOG OUT [POLICY_BIN]
// Starts BIN (the JS hub or backplane-hub) on a fresh home holding LOG as events.jsonl, with stand-in
// agents only, then speaks the GPUI client's view protocol: greeting, view.open recent, view.update,
// select the first thread, view.open thread, view.update. All frames go to OUT (decoded).
import {mkdirSync,rmSync,copyFileSync,writeFileSync,existsSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep,rssKb,treeRssKb} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const [name,bin,port,logPath,out,policy]=process.argv.slice(2);
const R='/home/h/Documents/i2cjak/backplane-gpui';
const root=`${R}/build/gpui/hub-run/${name}`;rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});
makeStandins(root+'/bin');
if(logPath&&existsSync(logPath))copyFileSync(logPath,root+'/home/events.jsonl');
const t0=performance.now();
const h=await startHub({bin,home:root+'/home',port:Number(port),standins:root+'/bin',extraEnv:policy?{BACKPLANE_POLICY_BIN:policy}:{}});
const started=performance.now()-t0;
const frames=[];const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';
let greet,rev=0,target={kind:'recent',id:''},viewId='',next=2**47+100;
const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
const pending=new Map();
const req=(m,extra={})=>{const id=++next;const p={...extra,version:1,hub:h.url,generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target,viewId,session,cid:'parity'};
 const t=performance.now();pending.set(id,t);frames.push({dir:'out',o:{id,m,p}});ws.send(encode({id,m,p}));return id};
const replies=new Map();
ws.onmessage=({data})=>{const x=decode(data);const now=performance.now();frames.push({dir:'in',ms:now,n:data.byteLength,o:x});
 if(x.t==='view')greet=x;
 if(x.t==='reply'){replies.set(x.id,{ms:now-(pending.get(x.id)??now),x});if(x.ok&&x.result?.session)Object.assign(session,x.result.session);if(x.ok&&x.result?.viewId)viewId=x.result.viewId}};
await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws error'))});
for(let i=0;i<200&&!greet;i++)await sleep(10);
const ask=async(m,extra)=>{const id=req(m,extra);for(let i=0;i<3000&&!replies.has(id);i++)await sleep(5);return replies.get(id)};
const o1=await ask('view.open');
const u1=await ask('view.update');
const shown=o1?.x?.result?.shown;const tid=shown?.projects?.[0]?.threads?.[0]?.id;
let o2,sel;
if(tid){sel=await ask('view.action',{action:'select',value:tid});
 const nt=sel?.x?.result?.nextTarget;if(nt){target=nt;viewId='';o2=await ask('view.open')}}
const rss=h.pid?rssKb(h.pid):NaN,tree=h.pid?treeRssKb(h.pid):NaN;
writeFileSync(out,JSON.stringify({name,bin,startupMs:started,openRecentMs:o1?.ms,updateMs:u1?.ms,openThreadMs:o2?.ms,rssKb:rss,treeRssKb:tree,frames},null,0));
console.log(JSON.stringify({name,startupMs:Math.round(started),openRecentMs:o1&&Math.round(o1.ms),updateMs:u1&&Math.round(u1.ms),openThreadMs:o2&&Math.round(o2.ms),rssKb:rss,treeRssKb:tree,frames:frames.length,ok:!!o1?.x?.ok,threadOk:!!o2?.x?.ok}));
ws.close();stopHub(h);await sleep(300);process.exit(0);
