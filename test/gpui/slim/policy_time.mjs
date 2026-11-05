// Time bp-policy: fold a log prefix in bounded frames, then render once. usage: bun policy_time.mjs BIN LOG N [CHUNK] [PORT]
import {spawn} from 'node:child_process';
import {createReadStream} from 'node:fs';
import {createInterface} from 'node:readline';
import net from 'node:net';
const [bin,log,N,CH='200',port='3821']=process.argv.slice(2);const NATIVE=process.env.NATIVE==='1';
const ascii=s=>s.replace(/[\u0080-￿]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0'));
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const sock=net.connect(Number(port),'127.0.0.1');await new Promise(r=>sock.once('connect',r));
let buf='',wait=null;sock.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);wait?.(l)}});
const ask=line=>new Promise(r=>{wait=r;sock.write(line+'\n')});
const rss=()=>Number(/VmRSS:\s+(\d+)/.exec(require('node:fs').readFileSync(`/proc/${p.pid}/status`,'utf8'))[1]);
const rl=createInterface({input:createReadStream(log)});
let items=[],n=0,first=true,tf=0;const T0=performance.now();const marks=[];
const flush=async()=>{if(!items.length)return;const fr=first?{t:'log',since:0,origin:'1',version:'x',info:{},items:[]}:{t:'changes',items:[]};fr.items=null;
 const body=ascii(JSON.stringify({...fr,items:0}).replace('"items":0','"items":['+items.join(',')+']'));first=false;items=[];
 const t=performance.now();const r=await ask('F '+body);tf+=performance.now()-t;if(r!=='ok')throw Error(r)};
for await(const l of rl){if(n>=Number(N))break;items.push(l);n++;if(items.length>=Number(CH))await flush();}
await flush();
const foldMs=tf;
const sess={sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}};
const t1=performance.now();
const r=await ask('V '+JSON.stringify({hub:'h',cid:'c',session:sess,target:{kind:'recent',id:''},action:'',value:'',...(NATIVE?{nativeSidebar:1,nativeKey:'00112233445566778899aabbccddeeff',epoch:'1',viewId:'1:1',generation:'1',revision:1}:{})}));
const renderMs=performance.now()-t1;
const t2=performance.now();await ask('V '+JSON.stringify({hub:'h',cid:'c',session:sess,target:{kind:'recent',id:''},action:'',value:'',...(NATIVE?{nativeSidebar:1,nativeKey:'00112233445566778899aabbccddeeff',epoch:'1',viewId:'1:1',generation:'1',revision:1}:{})}));
console.log(JSON.stringify({lines:n,chunk:Number(CH),foldMs:Math.round(foldMs),render1Ms:Math.round(renderMs),render2Ms:Math.round(performance.now()-t2),replyBytes:r.length,rssMB:Math.round(rss()/1024),wallMs:Math.round(performance.now()-T0)}));
p.kill();process.exit(0);
