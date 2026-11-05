// The viewer panel as the helper opens it: the `view` action for a kind, then the plain view on the session it answered.
// usage: OUT=file.json bun dump_panel.mjs BIN PORT   (synthetic fixtures only)
import {spawn} from 'node:child_process';import net from 'node:net';import {readFileSync,writeFileSync} from 'node:fs';
const [bin,port]=process.argv.slice(2);const dir=new URL('./fixtures/',import.meta.url).pathname;
const L=readFileSync(dir+'thread-log.jsonl','utf8').trim().split('\n').map(l=>JSON.parse(l));const pair=JSON.parse(readFileSync(dir+'thread-pair.json','utf8'));
const T=pair.params.page.thread;
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:L.slice(0,200)}));
for(let i=200;i<L.length;i+=200)await ask('F '+JSON.stringify({t:'changes',items:L.slice(i,i+200)}));
const base={hub:'h',cid:'c',now:pair.params.now,action:'',value:''};
const V=async(extra)=>JSON.parse(await ask('V '+JSON.stringify({...base,target:{kind:'thread',id:T},page:pair.params.page,...extra})));
const out={};
for(const kind of ['board','schematic','files','mech','3d','review']){
  const s0={sel:T,rid:1,drafts:{},open:{},adding:false,panel:false,info:{}};
  const a=await V({session:s0,action:'view',value:kind});
  const b=await V({session:a.session||s0});
  out[kind]={action:{session:a.session,cmds:a.cmds,wants:a.wants,local:a.local,remoteReady:a.remoteReady},viewer:b.shown?.thread?.viewer,after:{session:b.session,cmds:b.cmds}};
  console.log(kind,'open=',b.shown?.thread?.viewer?.open,'key=',b.shown?.thread?.viewer?.key,'cmds',JSON.stringify(a.cmds).slice(0,200));
}
writeFileSync(process.env.OUT||'panel.json',JSON.stringify(out));p.kill();process.exit(0);
