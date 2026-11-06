// The sidebar's shown.projects: Bend (bp-policy's screen) vs the Rust sidebar.rs, on the same log, field by field.
//   bun compare_projects.mjs POLICY_BIN SIDEBAR_CHECK_BIN LOG [PORT] [--show]   (--show prints paths with values: hub-generated logs only)
import {spawn,execFileSync} from 'node:child_process';
import {createReadStream} from 'node:fs';
import {createInterface} from 'node:readline';
import net from 'node:net';
const [bin,rust,log,port='3840',flag]=process.argv.slice(2);
const NOW_MS=Number(process.env.NOW_MS||1785100000000);
const ascii=s=>s.replace(/[\u0080-￿]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0'));
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const sock=net.connect(Number(port),'127.0.0.1');await new Promise(r=>sock.once('connect',r));
let buf='',wait=null;sock.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);wait?.(l)}});
const ask=l=>new Promise(r=>{wait=r;sock.write(l+'\n')});
let items=[],first=true,n=0;
const flush=async()=>{if(!items.length)return;const body=ascii(`{"t":"${first?'log':'changes'}"${first?',"since":0,"origin":"1","version":"x","info":{}':''},"items":[${items.join(',')}]}`);first=false;items=[];const r=await ask('F '+body);if(r!=='ok')throw Error(r)};
for await(const l of createInterface({input:createReadStream(log)})){if(!l)continue;items.push(l);n++;if(items.length>=200)await flush()}
await flush();
const reply=JSON.parse(await ask('V '+JSON.stringify({hub:'h',cid:'c',session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},target:{kind:'recent',id:''},action:'',value:'',now:NOW_MS})));
p.kill();
const A=reply.shown?.projects||[];
const B=JSON.parse(execFileSync(rust,['--projects','h',String(Math.floor(NOW_MS/1000)),'',log],{maxBuffer:1<<30}).toString());
const diffs=[];
const walk=(a,b,path)=>{
  if(JSON.stringify(a)===JSON.stringify(b))return;
  if(Array.isArray(a)&&Array.isArray(b)){if(a.length!==b.length)diffs.push([path+'.length',a.length,b.length]);for(let i=0;i<Math.min(a.length,b.length);i++)walk(a[i],b[i],path+'['+i+']');return}
  if(a&&b&&typeof a==='object'&&typeof b==='object'){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))walk(a[k],b[k],path+'.'+k);return}
  diffs.push([path,a,b]);
};
walk(A,B,'');
const kinds={};for(const [pth] of diffs){const k=pth.replace(/\[\d+\]/g,'[]');kinds[k]=(kinds[k]||0)+1}
console.log(JSON.stringify({lines:n,bendProjects:A.length,rustProjects:B.length,differences:diffs.length,byField:kinds}));
if(flag==='--show')for(const d of diffs.slice(0,15))console.log(' ',JSON.stringify(d).slice(0,200));
{ // content-safe order diagnostic: where each of Bend's projects sits in Rust's list, and the quiet flags of both
  const rpos=new Map(B.map((x,i)=>[x.id,i]));
  console.log('order: bend index -> rust index:',JSON.stringify(A.map(x=>rpos.has(x.id)?rpos.get(x.id):-1)));
  console.log('quiet flags bend:',A.map(x=>x.quiet?1:0).join(''),' rust:',B.map(x=>x.quiet?1:0).join(''));
  console.log('thread rows per project bend:',A.map(x=>x.threads.length+'/'+x.settled.length+'/'+x.snoozed.length+'/'+x.archived.length).join(' '));
  console.log('thread rows per project rust:',B.map(x=>x.threads.length+'/'+x.settled.length+'/'+x.snoozed.length+'/'+x.archived.length).join(' '));
}
process.exit(0);
