// Settings effects (S01-S14): for each control, one observable change in the hub's behaviour, on a temp home with stand-in agents.
//   bun test/e2e/s-effects.mjs HUB_BIN OUTDIR [port]   (NO_SCOPE=1 PERF_REPO=<repo root>)
// A line per probe: {id, control, ok, evidence}. `ok:false` is a control that saves but does nothing observable here: fix it or report it.
import {mkdirSync,rmSync,writeFileSync,chmodSync,readFileSync,existsSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {argvStandins,argvs,argOf,sock,idof,setting,waitFor,sleep,startHub,stopHub,client} from './lib/hubrows.mjs';
const [bin,out,port0]=process.argv.slice(2);const port=Number(port0||3870);process.env.NO_SCOPE='1';
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/bin',{recursive:true});mkdirSync(out+'/argv',{recursive:true});mkdirSync(out+'/proj',{recursive:true});
argvStandins(out+'/bin');
mkdirSync(out+'/kbp',{recursive:true});writeFileSync(out+'/kbp/kicad-cli','#!/bin/sh\necho 10.99.0-bp.4\n');chmodSync(out+'/kbp/kicad-cli',0o755); // a Backplane KiCad as far as the agent prompt can tell
const res=[];const rec=(id,control,ok,evidence)=>{const o={id,control,ok,evidence};res.push(o);console.log(JSON.stringify(o))};
const hub=await startHub({bin,home:out+'/home',port,standins:out+'/bin',extraEnv:{ARGV_DIR:out+'/argv',BACKPLANE_KICAD_CLI:out+'/kbp/kicad-cli',FAKE_SECS:'600',FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'});
for(const ev of ['uncaughtException','unhandledRejection'])process.on(ev,e=>{console.error(e);stopHub(hub);process.exit(1)}); // a probe that throws must not leave its hub running
const c=await sock(hub,port);const set=(k,v)=>setting(hub,k,v);
const p=idof((await c.rpc('project.add',{path:out+'/proj'})).result?.project);
const mk=async(title,x={})=>idof((await c.rpc('thread.create',{project:p,title,...x})).result?.thread);
/** start a fresh thread, send one short turn, return {thread, argv of the process it started} */
async function turn(title,x={}){const n0=argvs(out+'/argv').length;const t=await mk(title,x);await c.rpc('message.send',{thread:t,text:'zqshort '+title});
  const a=await waitFor(()=>{const l=argvs(out+'/argv');return l.length>n0&&l[l.length-1]},20000);return {t,a}}
const prompt=a=>{const f=argOf(a||[],'--append-system-prompt-file');if(f)try{return readFileSync(f,'utf8')}catch{return ''} return argOf(a||[],'--append-system-prompt')||''}; // Windows: the hub passes it in a file
// ---- S03 agents: the defaults for new threads, thinking
await set('agent.model','haiku');{const t=await mk('model default');const th=(await c.rpc('threads.list',{limit:50})).result?.threads?.find(x=>x.id===t);const r=await turn('model arg');
  rec('S03','agent.model chip',th?.model==='haiku'&&argOf(r.a,'--model')==='haiku',`new thread model=${th?.model}; stand-in argv --model ${argOf(r.a,'--model')}`)}
await set('agent.model','');
await set('agent.provider','codex');{const t=await mk('provider default');const th=(await c.rpc('threads.list',{limit:50})).result?.threads?.find(x=>x.id===t);rec('S03','agent.provider chip',th?.provider==='codex',`new thread provider=${th?.provider}`)}
await set('agent.provider','claude');
await set('agent.thinking','off');const off=await turn('think off');await set('agent.thinking','on');const on=await turn('think on');
rec('S03','agent.thinking chip',!(off.a||[]).includes('--thinking-display')&&(on.a||[]).includes('--thinking-display'),`off argv has --thinking-display: ${(off.a||[]).includes('--thinking-display')}; on: ${(on.a||[]).includes('--thinking-display')}`);
await set('agent.thinking','off');
// ---- S05 cad: kicad.ipc and freecad.path change what the agent is told
await set('kicad.ipc','on');const ki1=await turn('ipc on');await set('kicad.ipc','off');const ki0=await turn('ipc off');
const diff=(a,b)=>{const x=prompt(a),y=prompt(b);return {same:x===y,lenA:x.length,lenB:y.length}};
{const d=diff(ki1.a,ki0.a);rec('S05','kicad.ipc chip',!d.same,`system prompt differs between on and off: ${!d.same} (${d.lenA} vs ${d.lenB} chars); mentions kicad_ipc on=${prompt(ki1.a).includes('kicad_ipc')} off=${prompt(ki0.a).includes('kicad_ipc')}`)}
await set('kicad.ipc','on');
// freecad.path: proven by test/e2e/mac-c03.mjs (the hub starts the typed program), not by the agent prompt
// ---- S06 ordering
await set('order.enabled','on');const o1=await turn('order on');await set('order.enabled','off');const o0=await turn('order off');
{const d=diff(o1.a,o0.a);rec('S06','order.enabled chip',!d.same,`system prompt differs between on and off (${d.lenA} vs ${d.lenB} chars)`)}
await set('order.enabled','on');
// ---- S08 network: update.check answers the channel
{const k=await client(hub);let ok=true,ev=[];for(const ch of ['stable','beta','nightly']){await set('update.channel',ch);const r=await k.rpc('update.check',{},10000);const got=r?.o?.result?.channel;ev.push(`${ch}->${got}`);if(got!==(ch==='nightly'?'beta':ch))ok=false}k.ws.close();rec('S08','update.channel chip',ok,'update.check answers '+ev.join(', ')+' (nightly is the 0.14 name of beta)');await set('update.channel','stable')}
// ---- S11 timezone: the times a thread shows follow the zone
{const {viewClient}=await import('../perf/view.mjs');const k=await client(hub);const t=await mk('tz');await c.rpc('message.send',{thread:t,text:'zqshort tz'});await sleep(2500);
 const times=async zone=>{const r=await k.rpc('zone.set',{zone},10000);await sleep(300);const v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();v.target={kind:'thread',id:t};v.viewId='';const o=await v.req('view.open');v.close();
  const j=JSON.stringify(o?.x?.result||{});return {ok:r?.o?.ok,len:j.length,view_ok:o?.x?.ok,times:[...new Set((j.match(/\b\d\d:\d\d\b/g)||[]))].sort()}};
 const a=await times('UTC'),b=await times('Pacific/Auckland');k.ws.close();
 rec('S11','timezone Set',a.ok&&b.ok&&a.times.length>0&&b.times.length>0&&JSON.stringify(a.times)!==JSON.stringify(b.times),`times shown in UTC: ${a.times.join(' ')} (view ${a.len} bytes, ok ${a.view_ok}); in Pacific/Auckland: ${b.times.join(' ')} (view ${b.len} bytes, ok ${b.view_ok})`)}
// ---- S02 threads: the attachment limit answers 413 before the body is sent
{const net=await import('node:net');const t=await mk('att');
 const post=mb=>new Promise(res=>{const sk=net.connect(port,'127.0.0.1',()=>sk.write(`POST /attach?thread=${t}&key=k1&name=big.bin HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer ${hub.token}\r\nContent-Length: ${mb*1024*1024}\r\nConnection: close\r\n\r\n`));let d='';sk.on('data',x=>{d+=x;if(d.includes('\r\n'))sk.destroy()});sk.on('close',()=>res(d.split('\r\n')[0]));setTimeout(()=>{sk.destroy();res(d.split('\r\n')[0]||'no answer yet (waiting for the body)')},3000)});
 await set('files.max.mb','128');const small=await post(200);await set('files.max.mb','512');const big=await post(200);
 rec('S02','files.max.mb chip',/ 413/.test(small)&&!/ 413/.test(big),`a 200 MB upload: limit 128 -> "${small}"; limit 512 -> "${big}"`)}
// ---- the view, for the rest: a fresh view socket reads what the hub now shows
const {viewClient}=await import('../perf/view.mjs');
const viewOf=async(open)=>{const v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();const r=await open(v);v.close();return r};
const shown=async()=>viewOf(async v=>v.last?.result?.shown);
// ---- S01 appearance: the theme the clients are told, the thread list the sidebar offers
{const read=async()=>viewOf(async v=>{const r=await v.req('view.open');const res=r?.x?.result||{};return {theme:res.shown?.theme,list:JSON.stringify(res.shown?.sidebarPages||res.shown?.active||'').length,all:JSON.stringify(res.shown).length}});
 await set('ui.theme','dark');const d=await read();await set('ui.theme','light');const l=await read();
 rec('S01','theme chip',d.theme!==l.theme&&l.theme!==undefined,`shown.theme with dark: ${JSON.stringify(d.theme)}, with light: ${JSON.stringify(l.theme)}`);await set('ui.theme','');
 await set('sidebar.view','projects');const a=await read();await set('sidebar.view','active');const b=await read();
 rec('S01','thread list chip',a.all!==b.all,`the view is ${a.all} bytes with projects, ${b.all} with active`);await set('sidebar.view','projects')}
// ---- S10 keys: add through the hub, listed on the page, told to the agent by name, forgotten in two presses
{const k=await client(hub);const r=await k.rpc('bots.voice',{op:'api-add',name:'E2E_TEST_KEY',value:'dummy-value-not-a-key'},10000);
 const page=async()=>viewOf(async v=>{await v.req('view.action',{action:'flag',value:'settings'});const g=await v.req('view.action',{action:'settings-page',value:'keys'});return {rows:g?.x?.result?.shown?.settings?.rows||[],v}});
 const listed=JSON.stringify((await page()).rows).includes('E2E_TEST_KEY');const t=await turn('keys');const told=prompt(t.a).includes('E2E_TEST_KEY');
 const leaked=prompt(t.a).includes('dummy-value-not-a-key');
 const v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();await v.req('view.action',{action:'flag',value:'settings'});await v.req('view.action',{action:'settings-page',value:'keys'});
 const del=async()=>{const x=await v.req('view.action',{action:'key-del',value:'E2E_TEST_KEY'});for(const cm of x?.x?.result?.cmds||[])if(cm.type==='send')await k.rpc(JSON.parse(cm.text).m,JSON.parse(cm.text).p,10000)};await del();await del();v.close();
 const gone=!JSON.stringify((await page()).rows).includes('E2E_TEST_KEY');k.ws.close();
 rec('S10','API key add / list / forget',r?.o?.ok&&listed&&told&&!leaked&&gone,`api-add ok=${r?.o?.ok}; listed on the page ${listed}; agent prompt names it ${told}, leaks the value ${leaked}; two Forget presses remove it ${gone}`)}
// ---- S13 about: the page's version is the hub's
{const rows=await viewOf(async v=>{await v.req('view.action',{action:'flag',value:'settings'});const g=await v.req('view.action',{action:'settings-page',value:'about'});return g?.x?.result?.shown?.settings?.rows||[]});
 const ver=c.hello?.version;rec('S13','About version row',!!ver&&JSON.stringify(rows).includes(ver),`data/1 hello version ${ver}; the page says ${JSON.stringify(rows[1]?.label)}`)}
// ---- S07 devices: Add a device makes a link that pairs one device once; the page lists it; Revoke on the page ends its credential
{const call=(m,path,h)=>fetch(hub.url+path,{method:m,headers:h}).then(async r=>({s:r.status,j:await r.json().catch(()=>({}))}));
 const {viewClient}=await import('../perf/view.mjs');const v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();await v.req('view.action',{action:'flag',value:'settings'});
 const pg=async()=>(await v.req('view.action',{action:'settings-page',value:'devices'}))?.x?.result?.shown?.settings?.rows||[];
 await pg();await v.req('view.action',{action:'device-invite',value:''});const rows=await pg();
 const link=rows.find(r=>r.label==='Pairing link')?.note||'';const secret=new URLSearchParams(link.split('#')[1]||'').get('invite')||'';
 const enroll=n=>call('POST','/auth/enroll',{Authorization:`Bearer ${secret}`,'X-Backplane-Client':'native','X-Backplane-Name':n});
 const en=await enroll('e2e phone'),twice=await enroll('e2e twice');const tok=en.j?.token,id=en.j?.id;
 const before=(await call('GET','/auth/devices',{Authorization:`Bearer ${tok}`})).s;await sleep(500);const listed=(await pg()).find(r=>r.label==='e2e phone');
 await v.req('view.action',{action:'device-revoke',value:id});await sleep(500);
 const after=(await call('GET','/auth/devices',{Authorization:`Bearer ${tok}`})).s;const shown=(await pg()).find(r=>r.label==='e2e phone');v.close();
 rec('S07','Add a device, Revoke',!!secret&&en.s===200&&!!tok&&twice.s!==200&&before===200&&listed?.note==='Paired'&&after===401&&shown?.note==='Revoked',`Add a device: a Pairing link row ${!!link} with an invite secret ${!!secret}; enroll with it -> ${en.s}, the same link again -> ${twice.s}; the device's token on /auth/devices -> ${before}; the page lists "e2e phone" as ${listed?.note}; after Revoke on the page: the token -> ${after}, the page says ${shown?.note}`)}
// ---- S14 privacy: in Off and in On the hub opens no connection off this machine; On only through the page's own choice (setting.set refuses it on purpose, settings/telemetry.rs `reserved`)
{const ev=()=>readFileSync(hub.home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(Boolean);
 const g=key=>{let x='';for(const e of ev())if(e.$==='SettingSet'&&e.key===key)x=e.value;return x};
 const tm=()=>JSON.stringify(['telemetry.share','telemetry.asked','telemetry.usage.asked','telemetry.usage.generation','telemetry.choice.revision'].map(g));
 const mine=()=>{try{if(process.platform==='win32')return execFileSync('netstat',['-ano'],{stdio:['ignore','pipe','ignore']}).toString().split('\n').map(l=>l.trim().split(/\s+/)).filter(x=>x[0]==='TCP'&&x[4]===String(hub.pid));
   return execFileSync('lsof',['-nP','-a','-p',String(hub.pid),'-i'],{stdio:['ignore','pipe','ignore']}).toString().split('\n').slice(1).filter(Boolean).map(l=>{const f=l.trim().split(/\s+/);const [, r]=(f[8]||'').split('->');return ['TCP',f[8],r||'',f[9]||'']})}catch{return []}};
 const off_box=x=>x[2]&&!/^(127\.0\.0\.1|0\.0\.0\.0|\[::1\]|\[::\]|localhost|\*)(:|$)/.test(x[2]);
 const seen={};let sockets=0;const watch=async n=>{const f=new Set();for(let i=0;i<20;i++){const m=mine();sockets=Math.max(sockets,m.length);m.filter(off_box).forEach(x=>f.add(x[2]));await sleep(500)}seen[n]=[...f]};
 const k=await client(hub);await set('telemetry.share','off');const off=g('telemetry.share');await watch('off');
 await k.rpc('setting.set',{key:'telemetry.share',value:'on'},10000);await sleep(300);const still=g('telemetry.share');
 const on=await k.rpc('telemetry.choice',{value:'on',expected:tm(),decision:'',popup:false},10000);await sleep(400);const onv=g('telemetry.share');await watch('on');
 await set('telemetry.share','off');k.ws.close();
 rec('S14','telemetry.share',off==='off'&&still==='off'&&onv==='on'&&sockets>0&&seen.off.length===0&&seen.on.length===0,`Off via setting.set: stored ${off}; On via setting.set: refused, still ${still}; On via telemetry.choice with the page's token: ok=${on?.o?.ok}, stored ${onv}; the hub's own TCP sockets seen (${process.platform==='win32'?'netstat -ano':'lsof'}): ${sockets}, any to another machine over 10 s: Off ${seen.off.join(' ')||'none'}, On ${seen.on.join(' ')||'none'}`)}
writeFileSync(out+'/s-effects.json',JSON.stringify(res,null,1));c.ws.close();stopHub(hub);process.exit(0);
