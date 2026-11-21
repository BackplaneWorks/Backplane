// Swarms' Needs you rows, for real (native/host/src/present/swarm_need.rs): a throwaway hub (stand-in agents only) whose log holds four live swarms that want
// something of the person (all members idle, budget used, going in circles, a member whose Antigravity sign-in expired) and one a bot started whose idle
// pause is its starter's. Checked: each row's one line and its buttons (sidebar.sections), the bot's swarm not among them, the failed member classified by
// the hub's own tick from its error. With a client binary, the window (in a private Xvfb: nothing on a real display) is shot and Resume pressed on one row.
// usage: bun test/hub/swarm_need_e2e.mjs HUB_BIN PORT OUTDIR [CLIENT_BIN]   env: GPUI_DISP (:74), XVFB
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,existsSync,openSync} from 'node:fs';
import {execFileSync,spawn} from 'node:child_process';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
const [bin,port,out,clientBin]=process.argv.slice(2);
const work=out+'/swarm-need';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/proj',{recursive:true});mkdirSync(work+'/bin',{recursive:true});mkdirSync(work+'/home',{recursive:true});
const home=work+'/home';
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec ${process.execPath} ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse','agy','antigravity','opencode','pi','omp'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const now=Math.floor(Date.now()/1000);const L=[];const P=o=>L.push(JSON.stringify(o));
P({$:'ProjectCreated',id:'p1',title:'Web app',root:work+'/proj',at:now-3600});
P({$:'SettingSet',key:'restart.continue',value:'off',at:now-3600});
P({$:'BotSet',id:'b1',name:'Bub',thread:'t-bub',at:now-3600});
const swarm=(id,name,members,state,reason,by='',budget={posts:300,turns:120,tokens:4000000})=>{
  members.forEach(([t,n,prov],i)=>{P({$:'ThreadCreated',id:t,project:'p1',title:`${name} · ${n}`,env:'local',branch:'',worktree:'',provider:prov,model:'',at:now-3000+i});
    P({$:'SettingSet',key:`room.member.${id}.thread:${t}`,value:JSON.stringify({name:n,color:'#3b82f6',role:'',provider:prov,model:''}),at:now-3000});});
  P({$:'RoomSet',id,name,members:members.map(m=>'thread:'+m[0]).join('\n'),ephemeral:false,closed:false,kind:'swarm',project:'p1',goal:'',by,state,reason,budget,at:now-2900});
  P({$:'RoomPosted',id:`p-${id}-1`,room:id,from:'hub',text:`Swarm started with ${members.map(m=>m[1]).join(', ')}.`,hop:0,guard:true,held:'guard',at:now-2900});};
swarm('r1','Web UI',[['t-a1','opus','claude'],['t-a2','codex','codex']],'paused','idle');
swarm('r2','Board review',[['t-b1','opus','claude'],['t-b2','grok','grok']],'paused','budget','',{posts:300,turns:120,tokens:400000});
P({$:'SettingSet',key:'room.need.r2',value:JSON.stringify({for:'person',part:'tokens'}),at:now-2800});
swarm('r3','Firmware',[['t-c1','opus','claude'],['t-c2','gemini','antigravity']],'','');
P({$:'TurnChanged',thread:'t-c2',state:'running',at:now-2700});
P({$:'ActivityLogged',thread:'t-c2',act:'e-c2-1',tone:'error',kind:'spawn',summary:'You are not logged into Antigravity (silent auth failed). Run `agy` to sign in.',at:now-2690});
P({$:'TurnChanged',thread:'t-c2',state:'failed',at:now-2690});
swarm('r4','Docs pass',[['t-d1','opus','claude'],['t-d2','sonnet','claude']],'paused','circular');
swarm('r5','Bub refactor',[['t-e1','opus','claude'],['t-e2','codex','codex']],'paused','idle','t-bub');
P({$:'SettingSet',key:'room.need.r5',value:JSON.stringify({for:'starter'}),at:now-2600});
writeFileSync(home+'/events.jsonl',L.join('\n')+'\n');

const h=await startHub({bin,home,port:Number(port),standins:work+'/bin',log:work+'/hub.log'});
let fails=0;const check=(what,ok,detail='')=>{console.log(`${ok?'PASS':'FAIL'} ${what}${ok?'':' ('+detail+')'}`);if(!ok)fails++};
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}});
const until=async(ms,f)=>{const end=Date.now()+ms;for(;;){const v=await f();if(v)return v;if(Date.now()>end)return undefined;await sleep(500)}};
await sleep(1500);const c=await client(h);
const board=async r=>(await c.rpc('swarm.get',{room:r}))?.o?.result?.room?.need;
// the hub's minute tick reads the failed member's error: a sign-in, the person's
const n3=await until(90000,async()=>{const n=await board('r3');return n?.why==='sign-in'?n:undefined});
const want={r1:['Paused: all members idle 10 min',['Resume']],r2:['Paused: budget used (tokens)',['Raise budget','Resume']],r4:['Paused: going in circles',['Read board','Resume']],r3:['Member gemini failed: Gemini sign-in expired',['Sign in to Gemini','Move to Claude']]};
for(const [r,[line,labels]] of Object.entries(want)){const n=r==='r3'?n3:await board(r);
  check(`${r}: "${line}" with ${labels.join(' / ')}`,n?.line===line&&JSON.stringify((n?.actions??[]).map(a=>a.label))===JSON.stringify(labels),JSON.stringify(n));}
check("the bot's routine idle pause asks nothing of the person",(await board('r5'))==null,JSON.stringify(await board('r5')));
check("the failed member's own thread says why",events().some(e=>e.$==='ActivityLogged'&&e.thread==='t-c2'&&/^Gemini sign-in expired: this swarm member/.test(e.summary??'')));

let shots=[];
if(clientBin&&existsSync(clientBin)){
  const R=work+'/x';mkdirSync(R+'/run',{recursive:true});mkdirSync(R+'/fake/.cache',{recursive:true});chmodSync(R+'/run',0o700);
  const DISP=process.env.GPUI_DISP??':74';const XV=process.env.XVFB??`${process.env.HOME}/.local/xvfb-root/usr/bin/Xvfb`;
  const xenv={...process.env,XDG_RUNTIME_DIR:R+'/run',HOME:R+'/fake',XDG_CACHE_HOME:R+'/fake/.cache',XDG_CONFIG_HOME:R+'/fake/.config',XDG_DATA_HOME:R+'/fake/.local/share',LC_ALL:'C',DISPLAY:DISP,WAYLAND_DISPLAY:'',I3SOCK:'',DBUS_SESSION_BUS_ADDRESS:'',
    VK_ICD_FILENAMES:'/usr/share/vulkan/icd.d/lvp_icd.json',LP_NUM_THREADS:'4',BACKPLANE_GPUI_RECTS:'1',BACKPLANE_GPUI_ASSETS:process.env.BACKPLANE_GPUI_ASSETS??`${process.cwd()}/assets`};
  const xv=spawn(XV,[DISP,'-screen','0','1440x900x24','-nolisten','tcp','-xkbdir','/usr/share/X11/xkb'],{env:xenv,stdio:'ignore'});await sleep(2000);
  writeFileSync(R+'/i3.conf',`font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nbar {\n  mode invisible\n}\nipc-socket ${R}/i3.sock\n`);
  const wm=spawn('i3',['-c',R+'/i3.conf'],{env:xenv,stdio:'ignore'});await sleep(1000);
  const shot=name=>{const f=`${out}/swarm-need-${name}.png`;try{execFileSync('import',['-window','root',f],{env:xenv});shots.push(f)}catch(e){console.log('shot failed',name,String(e))}};
  writeFileSync(R+'/script.json',JSON.stringify([{wait:6000},{click:'swarm-need:Web UI:0'},{wait:60000}]));
  const log=R+'/client.log';
  const cl=spawn(clientBin,['--url',`http://127.0.0.1:${port}`,'--token-file',`${home}/token`,'--theme',process.env.THEME??'dark','--script',R+'/script.json','--exit-after','30'],{env:xenv,stdio:['ignore','ignore',openSync(log,'w')]});
  await sleep(5000);shot('rows');
  const resumed=await until(20000,()=>events().some(e=>e.$==='RoomPosted'&&e.room==='r1'&&e.guard===true&&/^Resumed by the person/.test(e.text??'')));
  check('Resume pressed in the window resumes the swarm',!!resumed,readFileSync(log,'utf8').split('\n').filter(l=>/click/.test(l)).join(' | '));
  await sleep(3000);shot('after-resume');
  try{cl.kill()}catch{} try{wm.kill();xv.kill()}catch{}
} else {
  const r=await c.rpc('swarm.act',{room:'r1',action:'resume'});check('Resume answers ok',r?.o?.ok===true,JSON.stringify(r?.o));
}
check('r1 is no longer paused',(()=>{let s='paused';for(const e of events())if(e.$==='RoomSet'&&e.id==='r1')s=e.state;return s===''})());
stopHub(h);
for(const s of shots)console.log('shot '+s);
console.log(`swarm_need_e2e: ${fails?'FAILED':'ok'} (${fails} failed) artifacts=${work}`);process.exit(fails?1:0);
