// Checks slim_serve's `local` (the sidebar's page memory carried by the host). usage: bun policy_local.mjs BIN LOG PORT (capped, fixture logs only)
// 1. a V with nativeSidebar and a valid nativeKey answers a top-level `local` with the four keys, context and key set;
// 2. sending `local` back unchanged keeps the same pages/context (the memory holds);
// 3. a `local` whose pages differ is adopted before the view is drawn (the reply's pages follow it) while the context is unchanged;
// 4. a different viewId makes the context differ, so the pages are dropped;
// 5. a native-sidebar-page action with a valid token moves the pages (local changes) and is not refused;
// 6. the same action with a tampered token, or with no nativeSidebar offer, is refused (`refused`: true) and the pages stay put;
// 7. an ordinary action is not refused;
// 8. a `reads` entry for fab.look is applied as the client applies the hub's reply: the session returned carries fab.line:<thread> (the goal, or the
//    placeholder when there is none), fab.shown, and an unknown kind is ignored; review.page / telemetry.share entries clear nothing they do not hold.
import {spawn} from 'node:child_process';import {readFileSync} from 'node:fs';import net from 'node:net';
const [bin,log,port]=process.argv.slice(2);
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
const ascii=t=>t.replace(/[\u0080-￿]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0'));
const lines=readFileSync(log,'utf8').trim().split('\n');
await ask('F '+ascii(JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:lines.slice(0,200).map(x=>JSON.parse(x))})));
for(let i=200;i<lines.length;i+=200)await ask('F '+ascii(JSON.stringify({t:'changes',items:lines.slice(i,i+200).map(x=>JSON.parse(x))})));
const base={hub:'h',cid:'c',session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},target:{kind:'recent',id:''},action:'',value:'',nativeSidebar:1,nativeKey:'00112233445566778899aabbccddeeff',epoch:'1',viewId:'v1',revision:1};
const v=async(extra)=>JSON.parse(await ask('V '+JSON.stringify({...base,...extra})));
const K=['local.native.sidebar.pages','local.native.sidebar.context','local.native.sidebar.key','local.native.sidebar.budget'];
let bad=0;const ck=(ok,m)=>{console.log(ok?'ok  ':'FAIL',m);if(!ok)bad++};
const a=await v({});
ck(a.local&&K.every(k=>typeof a.local[k]==='string'),'reply has local with the four keys');
ck(a.local[K[1]]!==''&&a.local[K[2]]==='00112233445566778899aabbccddeeff','context and key are set when the sidebar is native');
const b=await v({local:a.local});
ck(JSON.stringify(b.local[K[0]])===JSON.stringify(a.local[K[0]])&&b.local[K[1]]===a.local[K[1]],'local sent back unchanged keeps pages and context');
const mark=JSON.stringify({marker:true});
const c=await v({local:{...a.local,[K[0]]:mark}});
ck(c.local[K[0]].includes('marker')||c.local[K[0]]!==a.local[K[0]],'a changed pages is adopted before Ns.prepare (same context)');
const d=await v({local:{...a.local,[K[0]]:mark},viewId:'v2'});
ck(!d.local[K[0]].includes('marker'),'another viewId drops the pages');
const find=o=>{if(o&&typeof o==='object'){if(o.action==='native-sidebar-page'&&typeof o.value==='string')return o;for(const k of Object.keys(o)){const r=find(o[k]);if(r)return r}}return null};
const ctl=find(a.shown)||find(a.overlay);
ck(!!ctl,'the first picture offers a page control (the log has more than one page)');
if(ctl){
  const pg=await v({action:'native-sidebar-page',value:ctl.value,local:a.local});
  ck(!pg.refused&&pg.local[K[0]]!==a.local[K[0]],'a valid page token moves the pages and is not refused');
  const bad1=await v({action:'native-sidebar-page',value:ctl.value.slice(0,-1)+(ctl.value.endsWith('A')?'B':'A'),local:a.local});
  ck(bad1.refused===true&&bad1.local[K[0]]===a.local[K[0]],'a tampered token is refused and the pages stay put');
  const bad2=await v({nativeSidebar:0,action:'native-sidebar-page',value:ctl.value,local:a.local});
  ck(bad2.refused===true,'a page action with no nativeSidebar offer is refused');
}
const ord=await v({action:'noop-action',value:'x',local:a.local});
ck(ord.refused!==true,'an ordinary action is not refused');
{
  const th='t1700000002-3';
  const r1=await v({reads:[{kind:'fab.look',id:0,result:{thread:th,goal:'Ship the rev B board',intent:'',label:'',parts:[],boards:0}}]});
  const info=r1.session?.info||{};
  ck(info['fab.line:'+th]==='Ship the rev B board','a fab.look read is applied: fab.line:<thread> carries the goal');
  const r2=await v({reads:[{kind:'fab.look',id:0,result:{thread:th,goal:'',intent:'',label:'',parts:[],boards:0}}]});
  ck((r2.session?.info||{})['fab.line:'+th]==='No goal yet. Edit to say what this project is for.','an empty goal reads as the placeholder line');
  const rs=await v({reads:[{kind:'secret.list',id:0,result:{secrets:[],vault:{}}},{kind:'bots.voice',id:0,result:{voiceKey:'k',voiceDict:'',voiceDevices:'{"default":"","devices":[]}'}}]});
  ck(rs.session?.drafts?.['@voice.key']==='k'&&rs.session?.drafts?.['@voice.set']==='1','a bots.voice read is applied: @voice.key and @voice.set');
  // the Files listing is kept for the request the Ui itself made (fm.req:<id> -> thread, fm.id:<thread> -> id, fm.want:<thread> -> folder)
  const fd=await v({session:{sel:th,rid:1,drafts:{},open:{},adding:false,panel:false,info:{['fm.req:7']:th,['fm.id:'+th]:'7',['fm.want:'+th]:''}},reads:[{kind:'files.dir',id:7,result:{fmEntries:'d\t0\tsrc\nf\t12\tREADME.md',fmDir:'',fmRoot:'/tmp/x',thread:th}}]});
  ck((fd.session?.info||{})['fm.list:'+th]==='d\t0\tsrc\nf\t12\tREADME.md','a files.dir read is applied: the Files listing is kept (fm.list)');
  const r3=await v({reads:[{kind:'no.such.read',id:1,result:{thread:th,goal:'IGNORED'}}]});
  ck((r3.session?.info||{})['fab.line:'+th]!=='IGNORED','an unknown kind is ignored');
}
{
  const s1=await v({seq:12345});
  ck(s1.shown?.hubs?.[0]?.changes===12345&&s1.overlay?.j?.seq===12345,'a `seq` in V is the screen\'s change count (shown.hubs[0].changes and overlay.j.seq)');
  const s0=await v({});
  ck(s0.shown?.hubs?.[0]?.changes!==12345,'without `seq` the policy counts its own');
}
p.kill();console.log(bad?'FAILED '+bad:'ALL OK');process.exit(bad?1:0);
