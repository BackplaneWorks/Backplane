// The thread status word (the sidebar row's `status`): Bend (bp-policy's screen) vs the Rust signal index, on the same log.
//   bun compare_status.mjs POLICY_BIN SIDEBAR_CHECK_BIN LOG [PORT] [--show]    (--show prints ids: hub-generated logs only)
import {spawn,execFileSync} from 'node:child_process';
import {createReadStream} from 'node:fs';
import {createInterface} from 'node:readline';
import net from 'node:net';
const [bin,rust,log,port='3830',flag]=process.argv.slice(2);
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
const reply=JSON.parse(await ask('V '+JSON.stringify({hub:'h',cid:'c',session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},target:{kind:'recent',id:''},action:'',value:'',now:Date.now()})));
p.kill();
const un=id=>id.split('|').pop();
const bend=new Map();
const rows=(xs)=>{for(const t of xs||[]){bend.set(un(t.id),t.status);rows(t.children_rows)}};
for(const pr of reply.shown?.projects||[]){rows(pr.threads);rows(pr.snoozed);rows(pr.settled);rows(pr.archived)}
const rs=JSON.parse(execFileSync(rust,['--statuses',log],{maxBuffer:1<<30}).toString());
let eq=0,diff=[],onlyBend=0,onlyRust=0;
for(const [id,w] of bend){if(!(id in rs)){onlyBend++;continue}if(rs[id]===w)eq++;else diff.push([id,w,rs[id]])}
for(const id of Object.keys(rs))if(!bend.has(id))onlyRust++;
const hist={};for(const d of diff){const k=d[1]+'->'+d[2];hist[k]=(hist[k]||0)+1}
console.log(JSON.stringify({lines:n,bendRows:bend.size,rustThreads:Object.keys(rs).length,equal:eq,different:diff.length,onlyInBend:onlyBend,onlyInRust:onlyRust,differences:hist}));
if(flag==='--show')for(const d of diff.slice(0,20))console.log(' ',d.join(' '));
process.exit(0);
