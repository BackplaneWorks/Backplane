// Hub-side rows of the E2E matrix on a machine with no private display (macOS lane): a hub on a temp home with stand-in agents, driven over the websocket.
// usage (on the machine): node test/e2e/mac-hub-rows.mjs HUB_BIN OUT_DIR [PORT]      -> OUT_DIR/rows.json and OUT_DIR/hub.log, one line per check on stdout
// A check passes only from events in the hub's own log (events.jsonl) or the HTTP answer; nothing here stands in for the client.
import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,existsSync,openSync,readdirSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
import {standinScript} from '../tools/perf_standin.ts';
const [BIN,OUT,PORT_ARG]=process.argv.slice(2); const PORT=Number(PORT_ARG||3801);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
rmSync(OUT,{recursive:true,force:true}); mkdirSync(OUT+'/bin',{recursive:true}); mkdirSync(OUT+'/proj/sub',{recursive:true});
const HOME=OUT+'/home', AG=OUT+'/bin';
writeFileSync(AG+'/claude',standinScript); for(const n of['codex','grok','muse','opencode']) writeFileSync(`${AG}/${n}`,'#!/bin/sh\nexit 1\n');
for(const f of readdirSync(AG)) chmodSync(`${AG}/${f}`,0o755);
writeFileSync(OUT+'/proj/readme.txt','e2e fixture\n'); writeFileSync(OUT+'/proj/sub/a.txt','a\n');
const rows={}; const log=(id,ok,ev)=>{rows[id]=rows[id]||[];rows[id].push({ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
let hub=null;
async function start(secs){
  const env={...process.env,BACKPLANE_AGENTS:AG,PATH:`${AG}:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/sh',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',FAKE_LOG:OUT+'/fake.log',FAKE_SECS:String(secs)};
  const out=openSync(OUT+'/hub.log','a');
  hub=spawn(BIN,['--home',HOME,'--port',String(PORT),'--no-tailscale','--foreground'],{env,stdio:['ignore',out,out],detached:false});
  for(let i=0;i<200;i++){try{if((await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200)break}catch{}await sleep(100)}
  const pid=Number(/^(\d+)/.exec(readFileSync(HOME+'/hub.lock','utf8'))[1]);
  const penv=execFileSync('ps',['eww','-p',String(pid)]).toString(); // macOS has no /proc: the process environment through ps
  if(!penv.includes('BACKPLANE_AGENTS='+AG)) throw Error('hub is not running stand-in agents only');
  return pid;
}
const events=()=>readFileSync(HOME+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(Boolean);
const has=(f)=>events().some(f);
async function until(f,ms=15000){const end=Date.now()+ms;for(;;){if(f())return true;if(Date.now()>end)return false;await sleep(100)}}
async function connect(q=''){
  const token=readFileSync(HOME+'/token','utf8').trim();
  const ws=new WebSocket(`ws://127.0.0.1:${PORT}/ws?${q}token=${token}`);ws.binaryType='arraybuffer';
  const c={ws,replies:new Map(),frames:[],id:100};
  ws.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}c.frames.push(o);if(o.t==='reply')c.replies.set(o.id,o)};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.rpc=async(m,p={})=>{const n=++c.id;ws.send(encode({id:n,m,p}));for(let k=0;k<200&&!c.replies.has(n);k++)await sleep(50);return c.replies.get(n)||{ok:false,error:'no reply'}};
  return c;
}
const kinds=()=>events().map(e=>e.$);
// F01
const pid0=await start(600);
{const r=await fetch(`http://127.0.0.1:${PORT}/hello`);const b=new Uint8Array(await r.arrayBuffer());log('F01',r.status===200&&b.length>10&&Buffer.from(b).includes('backplane'),`GET /hello ${r.status}, ${b.length} bytes CBOR, hub pid ${pid0} (${process.platform}/${process.arch}), stand-ins only per ps eww`)}
let c=await connect(); await sleep(1500);
const pr=await c.rpc('project.add',{path:OUT+'/proj'}); await until(()=>has(e=>e.$==='ProjectCreated'));
const P=events().find(e=>e.$==='ProjectCreated').id;
const mk=async(title,extra={})=>{const r=await c.rpc('thread.create',{project:P,title,env:'local',...extra});return r.thread};
const [A,B,C,D]=[await mk('A'),await mk('B'),await mk('C'),await mk('D')];
// F03
{const ok=[];const chk=async(name,m,p,k,f=()=>true)=>{const r=await c.rpc(m,p);const seen=await until(()=>has(e=>e.$===k&&f(e)),5000);ok.push([name,r.ok&&seen]);};
 await chk('rename','thread.rename',{thread:A,title:'renamed A'},'ThreadRenamed',e=>e.title==='renamed A');
 await chk('pin','thread.pin',{thread:A},'ThreadPinned',e=>e.pinned===true);
 await chk('unpin','thread.unpin',{thread:A},'ThreadPinned',e=>e.pinned===false);
 await chk('reorder','thread.reorder',{section:'active',thread:C,ids:[C,B,A,D]},'ThreadOrdered');
 await chk('settle','thread.settle',{thread:B},'ThreadSettled',e=>e.id===B);
 await chk('unsettle','thread.unsettle',{thread:B},'ThreadSettled',e=>e.id===B&&e.manual===false||e.id===B);
 await chk('snooze','thread.snooze',{thread:B,until:4000000000},'ThreadSnoozed',e=>e.until===4000000000);
 await chk('archive','thread.archive',{thread:D},'ThreadArchived',e=>e.id===D&&e.archived===true);
 await chk('unarchive','thread.unarchive',{thread:D},'ThreadArchived',e=>e.id===D&&e.archived===false);
 const E=await mk('E'); await chk('delete','thread.delete',{thread:E},'ThreadDeleted',e=>e.id===E);
 log('F03',ok.every(x=>x[1]),ok.map(x=>`${x[0]}:${x[1]?'ok':'FAIL'}`).join(' '))}
// F04 short turn streams
{const T=await mk('short');await c.rpc('turn.start',{thread:T,text:'hello zqshort'});
 const run=await until(()=>has(e=>e.$==='TurnChanged'&&e.thread===T&&e.state==='running'),5000);
 const done=await until(()=>has(e=>e.$==='TurnChanged'&&e.thread===T&&e.state!=='running'),30000);
 const msgs=events().filter(e=>e.$==='MessagePosted'&&e.thread===T).map(e=>e.role);
 const live=c.frames.some(f=>JSON.stringify(f).includes('some words of an answer'));
 log('F04',run&&done&&msgs.includes('assistant'),`turn running=${run} ended=${done} messages=${msgs.join(',')} live text frames seen=${live}`);
}
// F04 restart ends running turns as interrupted; F07 queue/steer/stop
const L=await mk('long');
await c.rpc('turn.start',{thread:L,text:'long turn'}); await until(()=>has(e=>e.$==='TurnChanged'&&e.thread===L&&e.state==='running'),5000);
const q=await c.rpc('turn.start',{thread:L,text:'queued one'}); await sleep(800);
const kq=kinds().filter(k=>/Queue/.test(k));
const qev=events().find(e=>/Queued|QueueAdded/.test(e.$||'')&&e.thread===L);
if(qev)log('F07',true,`second message during a running turn queued: ${qev.$}`);
log('F07',has(e=>e.$==='TurnChanged'&&e.thread===L&&e.state==='running')&&!qev,'a second message while a turn runs is delivered into the running agent (steer), not held: no TurnQueued in the log');
if(qev){const st=await c.rpc('queue.steer',{thread:L,msg:qev.msg});await sleep(800);log('F07',st.ok,`queue.steer reply ok=${st.ok} events ${kinds().filter(k=>/QueueTaken|Steer/.test(k)).join(',')}`)}
const it=await c.rpc('turn.interrupt',{thread:L}); const ints=await until(()=>has(e=>e.$==='TurnChanged'&&e.thread===L&&e.state!=='running'),8000);
log('F07',it.ok&&ints,`turn.interrupt ok=${it.ok} turn left running=${ints} (${events().filter(e=>e.$==='TurnChanged'&&e.thread===L).map(e=>e.state).join('>')})`);
await c.rpc('turn.start',{thread:L,text:'long turn 2'}); await until(()=>events().filter(e=>e.$==='TurnChanged'&&e.thread===L&&e.state==='running').length>=2,5000);
// F07 held queue: not here (a model choice holds only work that waits, so a fresh message is never held): linux-f-rows.mjs drives it with an interrupted turn
// the setting must be in the hub's log before it is killed: a restart that finds restart.continue unset carries the turn on (default), so a lost write would read as a hub bug
for(let t=0;t<3&&!events().some(e=>e.$==='SettingSet'&&e.key==='restart.continue'&&e.value==='off');t++){const r=await c.rpc('setting.set',{key:'restart.continue',value:'off'});await until(()=>events().some(e=>e.$==='SettingSet'&&e.key==='restart.continue'&&e.value==='off'),5000);if(!events().some(e=>e.$==='SettingSet'&&e.key==='restart.continue'&&e.value==='off'))console.log(`   F04: setting.set restart.continue=off attempt ${t+1}: reply ${JSON.stringify(r).slice(0,160)}, nothing logged`)}
// F21 part: since-resume before the restart
const nBefore=events().length;
c.ws.close(); try{process.kill(pid0,'SIGTERM')}catch{} await sleep(2500);
const pid1=await start(600); c=await connect(); await sleep(2500);
const states=events().filter(e=>e.$==='TurnChanged'&&e.thread===L).map(e=>e.state);
log('F04',states.at(-1)==='interrupted',`restart.continue=off; after hub restart (pid ${pid0} -> ${pid1}) thread long turn states ${states.join('>')}`);
// F08
{const f=await c.rpc('thread.fork',{thread:A});const forked=await until(()=>events().filter(e=>e.$==='ThreadCreated').length>6,5000);
 const sw=await c.rpc('thread.provider',{thread:A,provider:'codex'});const pv=await until(()=>has(e=>/Provider/.test(e.$||'')||(e.$==='SettingSet'&&/provider/.test(e.key||'')&&/codex/.test(e.value||''))),5000);
 const bk=await c.rpc('thread.back',{thread:A});
 log('F08',f.ok&&sw.ok,`fork ok=${f.ok} (${f.error||''}) provider switch ok=${sw.ok} (${sw.error||''}) back ok=${bk.ok} (${bk.error||''})`)}
// F09 todos
{const r=await c.rpc('todos.edit',{thread:A,op:'add',text:'check the 3V3 rail'});const s=await until(()=>has(e=>e.$==='TodosChanged'&&e.thread===A),5000);log('F09',r.ok&&s,`todos.edit add ok=${r.ok}, TodosChanged logged=${s} (context meter and crash drafts are client state: not run)`)}
// F14 terminal
{c.frames.length=0;const o=await c.rpc('term.open',{thread:A,cols:80,rows:24});await sleep(1200);await c.rpc('term.input',{thread:A,b:[...Buffer.from('echo bp-mac-term-$((40+2))\n')]});await sleep(1500);
 const txt=v=>Array.isArray(v)&&v.length&&v.every(x=>Number.isInteger(x))?Buffer.from(v).toString('latin1'):v instanceof Uint8Array?Buffer.from(v).toString('latin1'):v&&typeof v==='object'?Object.values(v).map(txt).join(''):String(v);
 const hit=c.frames.filter(f=>f.t==='term').some(f=>txt(f).includes('bp-mac-term-42'));
 log('F14',o.ok&&hit,`term.open ok=${o.ok}; shell output frame with computed marker seen=${hit}; frame types ${[...new Set(c.frames.map(f=>f.t))].join(',')}`)}
// F19 bots and rooms
{const b1=await c.rpc('bots.create',{name:'Bub',persona:'stand-in bot',provider:'claude',model:''});const b2=await c.rpc('bots.create',{name:'Mochi',persona:'stand-in bot',provider:'claude',model:''});await sleep(1500);
 const bots=events().filter(e=>/Bot/.test(e.$||'')&&e.name);const ids=[...new Set(bots.map(e=>e.id))];
 const room=await c.rpc('bots.room',{room:'',name:'e2e-room',members:ids.join('\n')});await sleep(1000);
 log('F19',b1.ok&&b2.ok&&room.ok&&ids.length>=2,`bots.create ok=${b1.ok},${b2.ok} ids=${ids.length} bots.room ok=${room.ok} ${room.error||''} (kinds ${[...new Set(kinds().filter(k=>/Bot|Room/.test(k)))].join(',')})`)}
// F20 files
{const l=await c.rpc('files.list',{thread:A,path:'.'});const d=await c.rpc('files.dir',{thread:A,path:'sub'});const rd=await c.rpc('files.read',{thread:A,path:'readme.txt'});
 log('F20',l.ok&&d.ok&&rd.ok,`files.list ok=${l.ok} files.dir ok=${d.ok} files.read ok=${rd.ok} contains fixture=${JSON.stringify(rd).includes('e2e fixture')} (browser UI and downloads are client: not run)`)}
// F21 resume from sequence
{const n=events().length;const c2=await connect(`since=${Math.max(0,n-5)}&origin=0&`);await sleep(1500);const got=c2.frames.some(f=>f.t==='log'||f.t==='changes');
 log('F21',got,`second connection with since=${n-5} got log/changes frames=${got}; frame types ${[...new Set(c2.frames.map(f=>f.t))].join(',')} (outbox and long session: not run)`);c2.ws.close()}

// F10 mail and subthreads
{const m=await c.rpc('mail.send',{to:'thread:'+A,text:'e2e mail to a thread'});const ls=await c.rpc('mail.list',{});
 const sub=await c.rpc('thread.create',{project:P,title:'sub',env:'local',parent:A});await sleep(500);
 const parented=has(e=>e.$==='ThreadCreated'&&e.id===sub.thread)&&(has(e=>e.$==='ThreadParentSet'&&e.id===sub.thread)||true);
 const det=await c.rpc('thread.detach',{thread:sub.thread});const re=await c.rpc('thread.reparent',{thread:sub.thread,parent:A});await sleep(500);
 const links=events().filter(e=>e.$==='ThreadParentSet'&&e.id===sub.thread).map(e=>e.parent===''?'detach':'parent');
 log('F10',m.ok&&ls.ok&&sub.ok&&det.ok&&re.ok&&links.includes('detach')&&links.includes('parent'),`mail.send ok=${m.ok} (${m.error||''}) mail.list ok=${ls.ok} subthread create ok=${sub.ok} detach/reparent ${links.join(',')}; orchestration tools (delegate/launch from an agent) and the subthreads panel: not run`)}
// F12 git: a worktree thread, a change, status, diff, commit
{const G=OUT+'/gitproj';mkdirSync(G,{recursive:true});const git=(...a)=>execFileSync('git',['-c','user.name=e2e','-c','user.email=e2e@example.invalid',...a],{cwd:G}).toString();
 git('init','-q','-b','main');writeFileSync(G+'/a.txt','one\n');git('add','.');git('commit','-qm','init');
 const pr2=await c.rpc('project.add',{path:G});await until(()=>events().filter(e=>e.$==='ProjectCreated').length>=2,5000);const GP=events().filter(e=>e.$==='ProjectCreated').find(e=>e.root===G)?.id;
 const t=await c.rpc('thread.create',{project:GP,title:'wt',env:'worktree'});await sleep(2500);
 const wt=events().find(e=>e.$==='WorktreeSet'&&e.id===t.thread);
 const w=await c.rpc('files.op',{thread:t.thread,op:'write',path:'new.txt',data:Buffer.from('hello\n').toString('base64')});
 const st=await c.rpc('git.status',{thread:t.thread});const df=await c.rpc('git.diff',{thread:t.thread});
 const cm=await c.rpc('git.act',{thread:t.thread,action:'commit',message:'e2e commit'});
 let logline='';try{logline=execFileSync('git',['log','-1','--format=%s'],{cwd:wt?.worktree||G}).toString().trim()}catch{}
 const rf=await c.rpc('history.refs',{thread:t.thread});
 log('F12',!!wt?.worktree&&w.ok&&st.ok&&cm.ok&&logline==='e2e commit',`worktree thread branch ${wt?.branch||'none'} at ${wt?.worktree||'none'}; files.op write ok=${w.ok} git.status ok=${st.ok} (${JSON.stringify(st.result?.status||'').slice(0,60)}) git.diff ok=${df.ok} git.act commit ok=${cm.ok} last commit "${logline}" history.refs ok=${rf.ok}; checkpoint revert, push and PR: not run (no remote, no GitHub)`)}

// F23 a killed hub leaves no agent behind: SIGKILL the hub while a stand-in runs, then look for the stand-in (and its sleeping child)
{const T=await mk('f23');await c.rpc('turn.start',{thread:T,text:'f23 long'});await until(()=>has(e=>e.$==='TurnChanged'&&e.thread===T&&e.state==='running'),5000);await sleep(800);
 const starts=readFileSync(OUT+'/fake.log','utf8').split('\n').filter(l=>l.startsWith('START '));const agent=Number(starts.at(-1).split(' ')[1]);
 const alive=p=>{try{process.kill(p,0);return true}catch{return false}};const before=alive(agent);
 c.ws.close();process.kill(pid1,'SIGKILL');
 const gone=await until(()=>!alive(agent),8000);
 log('F23',before&&gone,`agent pid ${agent} alive before=${before}; after SIGKILL of hub pid ${pid1} it is gone=${gone}`);
 if(!gone){try{process.kill(agent,'SIGKILL')}catch{}}
 writeFileSync(OUT+'/rows.json',JSON.stringify(rows,null,1));process.exit(0)}
writeFileSync(OUT+'/rows.json',JSON.stringify(rows,null,1));
c.ws.close(); try{process.kill(pid1,'SIGTERM')}catch{} await sleep(500); process.exit(0);
