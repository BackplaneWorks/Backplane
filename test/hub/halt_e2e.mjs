// Halt cards, for real (native/host/src/agents/halt.rs; Linux with a user systemd): a throwaway hub whose agents run in scopes of a test slice,
// each capped at 400 MB. Thread A's stand-in agent eats 1.5 GB itself, so the kernel kills the agent (not a child): A's turn ends on an "Out of
// memory" card that says what was killed, how big it was and the limit it hit, with Resume, Retry and Raise limit. Resume runs the same session
// again; Raise limit takes the cap of the next agent to 600 MB. Thread B's agent says its prompt is too long: a "Context full" card. With a
// client binary, the real window (in a private Xvfb, nothing on a real display) is shot showing the cards and Resume is pressed on A's card.
// usage: bun test/hub/halt_e2e.mjs HUB_BIN PORT OUTDIR [CLIENT_BIN]   env: GPUI_DISP (:73), XVFB, SHOTS_ONLY=1 (the shots, no checks: an older hub)
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,existsSync} from 'node:fs';
import {execFileSync,spawn} from 'node:child_process';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
const [bin,port,out,clientBin]=process.argv.slice(2);
const work=out+'/halt';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec ${process.execPath} ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const SLICE='bphalttest.slice';
const h=await startHub({bin,home,port:Number(port),standins:work+'/bin',log:work+'/hub.log',cap:'2G',
  extraEnv:{FAKE_LOG:work+'/fake.log',BP_STANDIN_MODE:'plain',BACKPLANE_SCOPE_SLICE:SLICE,BACKPLANE_SCOPE_MEM_MAX:'400M',BACKPLANE_SLICE_MEM_MAX:'3G'}});
const only=!!process.env.SHOTS_ONLY;let fails=0;const check=(what,ok,detail='')=>{if(only)return;console.log(`${ok?'PASS':'FAIL'} ${what}${ok?'':' ('+detail+')'}`);if(!ok)fails++};
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}});
const turns=(th,st)=>events().filter(e=>e.$==='TurnChanged'&&e.thread===th&&e.state===st).length;
const setting=k=>{let v='';for(const e of events())if(e.$==='SettingSet'&&e.key===k)v=e.value;return v};
const halts=th=>events().filter(e=>e.$==='ActivityLogged'&&e.thread===th&&e.kind==='halt');
const fake=()=>{try{return readFileSync(work+'/fake.log','utf8')}catch{return ''}};
const until=async(ms,f)=>{const end=Date.now()+ms;for(;;){const v=f();if(v)return v;if(Date.now()>end)return undefined;await sleep(250)}};
await sleep(2000);const c=await client(h);
const A='t1700000001-2',B='t1700000002-3';

// A: a first turn that works, so there is a session to resume
c.send('turn.start',{thread:A,text:'hello first'});await until(20000,()=>turns(A,'completed')>0);
const sess=events().filter(e=>e.$==='SessionSet'&&e.id===A&&e.session).map(e=>e.session).pop()??'';
const agentStarts=()=>fake().split('\n').filter(l=>l.startsWith('ARGV ')&&!l.includes('--no-session-persistence'));
// A eats memory itself: the kernel kills the agent at its scope's cap
c.send('turn.start',{thread:A,text:'eatself:1500 eat'});
await until(40000,()=>turns(A,'failed')>0);
const ha=halts(A).pop();
check("A's turn ended on a halt row",!!ha,JSON.stringify(events().filter(e=>e.thread===A&&e.$==='ActivityLogged').slice(-3)));
check('the row says it plainly: stopped for memory, how big, the limit',/was stopped for memory: \S+ was using about \d+ MB and reached this thread's memory limit \(400 MB\)/.test(ha?.summary??''),ha?.summary);
const card=ha?JSON.parse(setting(`halt.${A}.${ha.act}`)||'{}'):{};
check('its card: memory, the cap, a raise to 600M',card.cause==='memory'&&card.why==='cap'&&card.cap==='400 MB'&&card.raise==='600M',JSON.stringify(card));
check('no raw stack in the row; the provider words under Details only',!/\n/.test(ha?.summary??'')&&/out-of-memory killer/.test(card.detail??''),JSON.stringify(card.detail));
check('A was not told "the agent goes on" for its own death',!events().some(e=>e.thread===A&&e.$==='MessagePosted'&&/Killed for memory/.test(e.text??'')));

// B: the context is full
c.send('turn.start',{thread:B,text:'toolong please'});
await until(20000,()=>turns(B,'failed')>0);
const hb=halts(B).pop();const cardB=hb?JSON.parse(setting(`halt.${B}.${hb.act}`)||'{}'):{};
check('one card for the turn, though Claude says it twice',halts(B).length===1,String(halts(B).length));
check("B's turn ended on a context card",/no longer fits in the model's context/.test(hb?.summary??'')&&cardB.cause==='context'&&cardB.compact===true,JSON.stringify([hb,cardB]));

// the window: both cards, then Resume pressed on A's card
let shots=[];
if(clientBin&&existsSync(clientBin)){
  const R=work+'/x';mkdirSync(R+'/run',{recursive:true});mkdirSync(R+'/fake/.cache',{recursive:true});chmodSync(R+'/run',0o700);
  const DISP=process.env.GPUI_DISP??':73';const XV=process.env.XVFB??`${process.env.HOME}/.local/xvfb-root/usr/bin/Xvfb`;
  const xenv={...process.env,XDG_RUNTIME_DIR:R+'/run',HOME:R+'/fake',XDG_CACHE_HOME:R+'/fake/.cache',XDG_CONFIG_HOME:R+'/fake/.config',XDG_DATA_HOME:R+'/fake/.local/share',LC_ALL:'C',DISPLAY:DISP,WAYLAND_DISPLAY:'',I3SOCK:'',DBUS_SESSION_BUS_ADDRESS:'',
    VK_ICD_FILENAMES:'/usr/share/vulkan/icd.d/lvp_icd.json',LP_NUM_THREADS:'4',BACKPLANE_GPUI_RECTS:'1',BACKPLANE_GPUI_ASSETS:process.env.BACKPLANE_GPUI_ASSETS??`${process.cwd()}/assets`};
  const xv=spawn(XV,[DISP,'-screen','0','1440x900x24','-nolisten','tcp','-xkbdir','/usr/share/X11/xkb'],{env:xenv,stdio:'ignore'});await sleep(2000);
  writeFileSync(R+'/i3.conf',`font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nbar {\n  mode invisible\n}\nipc-socket ${R}/i3.sock\n`);
  const wm=spawn('i3',['-c',R+'/i3.conf'],{env:xenv,stdio:'ignore'});await sleep(1000);
  const shot=name=>{const f=`${out}/halt-${name}.png`;try{execFileSync('import',['-window','root',f],{env:xenv});shots.push(f)}catch(e){console.log('shot failed',name,String(e))}};
  const run=async(steps,secs,name)=>{writeFileSync(R+'/script.json',JSON.stringify(steps));const log=`${R}/client-${name}.log`;
    const cl=spawn(clientBin,['--url',`http://127.0.0.1:${port}`,'--token-file',`${home}/token`,'--theme',process.env.THEME??'dark','--script',R+'/script.json','--exit-after',String(secs)],{env:xenv,stdio:['ignore','ignore',(await import('node:fs')).openSync(log,'w')]});
    return {cl,log}};
  { const {cl}=await run([{wait:2500},{action:["link-open",`backplane://thread/${A}`]},{wait:60000}],40,'a');await sleep(9000);shot('memory');try{cl.kill()}catch{} await sleep(500)}
  { const {cl}=await run([{wait:2500},{action:["link-open",`backplane://thread/${B}`]},{wait:60000}],40,'b');await sleep(9000);shot('context');try{cl.kill()}catch{} await sleep(500)}
  { const before=turns(A,'completed');
    const {cl,log}=await run([{wait:2500},{action:["link-open",`backplane://thread/${A}`]},{wait:3500},{click:'halt:Resume'},{wait:9000},{read:'timeline'},{wait:60000}],45,'resume');
    const clicked=await until(25000,()=>/"id":"halt:Resume","changed":true,"action":"go halt [^"|]+\|resume"/.test(readFileSync(log,'utf8')));
    check('the window drew the card and its Resume was pressed',!!clicked,readFileSync(log,'utf8').split('\n').filter(l=>/click/.test(l)).join(' | '));
    await until(20000,()=>turns(A,'completed')>before);await sleep(4000);shot('resumed');
    const tl=await until(20000,()=>readFileSync(log,'utf8').split('\n').map(l=>{try{return JSON.parse(l.replace(/^.*?GPUI_METRIC /,''))}catch{return null}}).find(m=>m?.event==='read'&&m.what==='timeline')?.text)??'';
    check('the timeline the window holds has the halt card',/halt: \S+ was stopped for memory/.test(tl),tl.slice(0,600));
    try{cl.kill()}catch{} }
  try{wm.kill();xv.kill()}catch{}
} else {
  const r=await c.rpc('thread.halt',{thread:A,act:ha?.act??'',do:'resume'});
  check('Resume answers ok',r?.o?.ok===true,JSON.stringify(r?.o));
}
// Resume: the same session again, and the card says it was done
await until(20000,()=>turns(A,'completed')>=2);
const resumed=agentStarts().pop()??'';
check('Resume ran the same session again',!!sess&&resumed.includes(`"--resume","${sess}"`),`${sess}: ${resumed.slice(0,200)}`);
check("A's turn after Resume completed",turns(A,'completed')>=2,`completed ${turns(A,'completed')}`);
check('the card says Resumed in place of its buttons',JSON.parse(setting(`halt.${A}.${ha?.act}`)||'{}').took==='resume');

// Raise limit: the next agent's scope gets the raised cap
c.send('turn.start',{thread:A,text:'eatself:1500 again'});await until(40000,()=>halts(A).length>=2&&turns(A,'failed')>=2);
const ha2=halts(A).pop();const r2=await c.rpc('thread.halt',{thread:A,act:ha2?.act??'',do:'raise'});
check('Raise limit answers ok',r2?.o?.ok===true,JSON.stringify(r2?.o));
check('the raised cap is kept (memory.agent.max)',setting('memory.agent.max')==='600M',setting('memory.agent.max'));
let cap='';await until(15000,()=>{try{const base=`/sys/fs/cgroup/user.slice/user-${process.getuid()}.slice/user@${process.getuid()}.service/${SLICE}`;
  for(const d of (execFileSync('ls',[base]).toString().split('\n')).filter(d=>d.startsWith('bp-agent-')))cap=readFileSync(`${base}/${d}/memory.max`,'utf8').trim()}catch{} return cap==='629145600'});
check("the next agent's scope has the raised cap (600 MB)",cap==='629145600',cap);
let up=false;try{up=(await fetch(`http://127.0.0.1:${port}/hello`)).status===200}catch{}
check('the hub answered all along',up);
stopHub(h);await sleep(1500);try{execFileSync('systemctl',['--user','stop',SLICE])}catch{}
for(const s of shots)console.log('shot '+s);
console.log(`halt_e2e: ${fails?'FAILED':'ok'} (${fails} failed) artifacts=${work}`);process.exit(fails?1:0);
