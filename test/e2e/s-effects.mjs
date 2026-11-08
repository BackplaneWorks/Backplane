// Settings effects (S01-S14): for each control, one observable change in the hub's behaviour, on a temp home with stand-in agents.
//   bun test/e2e/s-effects.mjs HUB_BIN OUTDIR [port]   (NO_SCOPE=1 PERF_REPO=<repo root>)
// A line per probe: {id, control, ok, evidence}. `ok:false` is a control that saves but does nothing observable here: fix it or report it.
import {mkdirSync,rmSync,writeFileSync,chmodSync,readFileSync,existsSync} from 'node:fs';
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
// S14 telemetry: proven by test/e2e/mac-effects.mjs (setting.set refuses On by design; the page's token turns it on)
writeFileSync(out+'/s-effects.json',JSON.stringify(res,null,1));c.ws.close();stopHub(hub);process.exit(0);
