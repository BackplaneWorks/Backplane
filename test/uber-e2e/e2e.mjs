// bun e2e.mjs RUN HUB_BIN BUB.jsonl [PORT]: the uberbot end to end on a throwaway home: a built hub, Bub's real history, stand-in agents only.
// RUN must not exist yet. Prints PASS/FAIL lines with counts only: no message text, memory, prompt or routine name ever reaches the output.
// Needs: hub.sh, prep.py and standin-claude.py beside it. The fixture stays private: never commit it or RUN.
import {execFileSync} from 'node:child_process';
import {readFileSync,existsSync,appendFileSync,mkdirSync,copyFileSync,chmodSync} from 'node:fs';
import {client,sleep} from '../perf/lib.mjs';
import {viewClient} from './vc.mjs';
let [R,BIN,FIX,PORT='3811']=process.argv.slice(2);  // FIX 'synth': a synthetic history (synth.py), no private data
const HERE=import.meta.dir;
if(existsSync(R+'/home/events.jsonl'))throw Error('RUN exists: '+R);
mkdirSync(R,{recursive:true,mode:0o700});copyFileSync(BIN,R+'/backplane-hub');chmodSync(R+'/backplane-hub',0o755);
const env={...process.env,E2E_PORT:PORT};
const hub=c=>execFileSync(HERE+'/hub.sh',[c,R],{env,encoding:'utf8'}).trim();
let fails=0;const check=(n,ok,detail='')=>{console.log(`${ok?'PASS':'FAIL'} ${n}${detail!==''?` (${detail})`:''}`);if(!ok)fails++};
const events=()=>readFileSync(R+'/home/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
const since=n=>events().slice(n);
const turns=()=>existsSync(R+'/turns.jsonl')?readFileSync(R+'/turns.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l)):[];
const until=async(ms,f)=>{const end=Date.now()+ms;for(;;){const v=f();if(v)return v;if(Date.now()>end)return f();await sleep(250)}};
const nextTick=async()=>{const s=Date.now()/1000;await sleep(((60-s%60)+6)*1000)};
const opK=e=>JSON.parse(e.op).k;
const lastTurn=(evs,t)=>evs.filter(e=>e.$==='TurnChanged'&&e.thread===t).at(-1)?.state;
const live=pid=>execFileSync('ps',['-o','pid=','--ppid',String(pid)],{encoding:'utf8'}).split('\n').filter(s=>s.trim()).map(s=>s.trim());
const hubPid=()=>Number(readFileSync(R+'/home/hub.lock','utf8').split(' ')[0]);
const connect=async()=>{const h={host:'127.0.0.1',port:Number(PORT),url:`http://127.0.0.1:${PORT}`,token:readFileSync(R+'/home/token','utf8').trim()};return {h,c:await client(h)}};

// ---- the home: an empty start, Bub's history appended while stopped
console.log(hub('start'));await sleep(1500);hub('stop');
if(FIX==='synth'){FIX=R+'/synth.jsonl';execFileSync(HERE+'/synth.py',[FIX],{stdio:'ignore'})}
execFileSync(HERE+'/prep.py',[FIX,R+'/bub-moved.jsonl',R+'/bubdir'],{stdio:'ignore'});
const fix=readFileSync(R+'/bub-moved.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
const B=fix.find(e=>e.$==='UberSet').bot,T=fix.find(e=>e.$==='BotSet'&&e.id===B&&e.thread).thread,persona=fix.filter(e=>e.$==='BotSet'&&e.id===B).at(-1).persona;
const mems=new Set();for(const e of fix){if(e.$==='MemorySaved')mems.add(e.key);if(e.$==='MemoryForgot')mems.delete(e.key)}
const routines=new Set(fix.filter(e=>e.$==='RoutineSet').map(e=>e.id));
appendFileSync(R+'/home/events.jsonl',readFileSync(R+'/bub-moved.jsonl'));
console.log(hub('start'));await sleep(1000);
let {h,c}=await connect();
const say=(text,msg)=>c.rpc('bots.say',{bot:B,text,msg});

// ---- 1: listed, Uber.init once, lease
check('no turn at boot',turns().filter(t=>t.ev==='turn').length===0);
await nextTick();
let ev=events();const me=ev.findLast(e=>e.$==='SettingSet'&&e.key==='machine.id').value;
const mine=()=>events().filter(e=>e.$==='UberOp'&&e.origin===me);
const ops1=mine();const tally=o=>o.reduce((a,e)=>(a[opK(e)]=(a[opK(e)]||0)+1,a),{});const t1=tally(ops1);
check('Uber.init: who, lease, bind, space once',t1.who===1&&t1.lease===1&&t1.bind===1&&t1.space===1,JSON.stringify({who:t1.who,lease:t1.lease,bind:t1.bind,space:t1.space}));
check('Uber.init: every memory and routine',t1.mem===mems.size&&t1.routine===routines.size,`${t1.mem}/${mems.size} memories, ${t1.routine}/${routines.size} routines`);
await nextTick();check('Uber.init does not repeat',mine().length===ops1.length,`${mine().length} ops after another tick`);
const v=await viewClient(h);let r=await v.open({kind:'recent',id:''});
const row=(r?.result?.shown?.bots||[]).find(b=>b.key===B);check('Bub is in the sidebar',!!row);
const keys=(r?.result?.shown?.bots||[]).map(b=>b.key);check('the uberbot leads the sidebar, ahead of an older bot',keys[0]===B&&keys.includes('bsyn0')&&keys.indexOf('bsyn0')>0,keys.join(','));
r=await v.ask('view.action',{action:'bot',value:`${h.url}|${B}`});check('the bot row opens the bot',r?.ok&&r.result?.nextTarget?.kind==='bot');
r=await v.open({kind:'bot',id:B});let s=r?.result?.shown;
check('view.open bot answers on Chat with the thread',r?.ok&&s?.bot?.tab==='chat'&&s?.bot?.uber===true&&s?.thread?.id!==undefined,`${s?.thread?.entries?.length} entries`);
r=await v.ask('view.action',{action:'bot-tab',value:'memory'});s=r?.result?.shown;
check('Memory tab counts every memory',r?.ok&&(s?.bot?.memory?.length+s?.bot?.memoryMore)===mems.size,`${s?.bot?.memory?.length}+${s?.bot?.memoryMore}`);
check('a view stays under the reply limit',JSON.stringify(r).length<262144,JSON.stringify(r).length);
r=await v.ask('view.action',{action:'bot-tab',value:'routines'});check('Routines tab',r?.ok&&r.result.shown.bot.routines.length===routines.size,r?.result?.shown?.bot?.routines?.length);
r=await v.ask('view.update');check('the tab holds across an update',r?.result?.shown?.bot?.tab==='routines');
r=await v.ask('view.action',{action:'bot-tab',value:'chat'});
r=await v.open({kind:'thread',id:T});check('view.open of his thread',r?.ok&&!!r.result.shown.thread);
r=await v.open({kind:'bot',id:'bnone'});check('an unknown bot is stale',r?.error?.code==='STALE_VIEW');
const term0=ops1.find(e=>opK(e)==='lease');
r=await c.rpc('bots.uber_home',{});check('bots.uber_home',r?.o?.ok===true);
const lease=fix.filter(e=>e.$==='UberOp'&&opK(e)==='lease').map(e=>JSON.parse(e.op).term);
check('the lease moved here one term on',mine().filter(e=>opK(e)==='lease').some(e=>{const b=JSON.parse(e.op);return b.holder===me&&b.term===Math.max(0,...lease)+1}),`old term ${Math.max(0,...lease)}`);

// ---- 2: a person's message, a memory
let L=events().length;
r=await say('zqsave:e2e-note hello','m-e2e-1');check('bots.say',r?.o?.ok===true);
await until(15000,()=>lastTurn(events(),T)==='completed'&&since(L).some(e=>e.$==='MemorySaved'));
let n=since(L);
const start=turns().filter(t=>t.ev==='start'&&t.argv.includes('--input-format')&&t.argv.join(' ').includes(persona.slice(0,150)));
check('the turn starts with his persona',start.length>0);
check('and in his own folder',start[0]?.cwd===R+'/bubdir');
const tl=turns().filter(t=>t.ev==='turn').find(t=>t.line.includes('zqsave:e2e-note'));
check('the turn carries his memory block',!!tl&&tl.line.includes('<memor'));
check('memory_save over MCP',turns().some(t=>t.ev==='tool'&&t.name==='memory_save'&&JSON.parse(t.answer).result?.isError===false));
check('MemorySaved and its UberOp',n.some(e=>e.$==='MemorySaved'&&e.key==='e2e-note')&&n.some(e=>e.$==='UberOp'&&opK(e)==='mem'&&JSON.parse(e.op).key==='e2e-note'));

// the composer on his Chat, as a client sends it: the request the view's reply tells it to send
r=await v.open({kind:'bot',id:B});L=events().length;v.session.drafts[T]='zqsave:e2e-view from the page';
r=await v.ask('view.action',{action:'send',value:''});check('Send on his Chat is answered',r?.ok===true,r?.ok?'':JSON.stringify(r?.error));
const sent=(r?.result?.cmds||[]).filter(x=>x.type==='send').map(x=>JSON.parse(x.text));check('as bots.say',sent.some(x=>x.m==='bots.say'&&x.p.bot===B));
for(const x of sent)await c.rpc(x.m,x.p);
await until(15000,()=>since(L).some(e=>e.$==='MemorySaved'&&e.key==='e2e-view')&&lastTurn(events(),T)==='completed');
check('and starts his turn',since(L).some(e=>e.$==='MemorySaved'&&e.key==='e2e-view'));

// ---- 2b: alerts reach the desk used last, unless a focused desk shows the thread (desk.look, the notify frame)
{const notes=[];const c2=await client(h,{onMsg:o=>{if(o.t==='notify')notes.push(o)}});
const idle=()=>until(15000,()=>lastTurn(events(),T)==='completed');
r=await c2.rpc('desk.look',{thread:T,focused:true});check('desk.look is answered',r?.o?.ok===true);
await say('zqsave:al-1 x','m-al-1');await sleep(500);await idle();await sleep(2500);
check('a done on a thread a focused desk shows alerts no one',notes.length===0,notes.length);
await sleep(1100);r=await c2.rpc('desk.look',{thread:'elsewhere',focused:true});
await say('zqsave:al-2 x','m-al-2');await until(15000,()=>notes.length>0);
check('the same done reaches the desk when it shows another thread',notes.some(n=>n.kind==='done'&&n.thread===T&&n.title&&n.body),JSON.stringify(notes.map(n=>[n.kind,n.title,n.body])));
const n0=notes.length;await idle();await say('zqnotify:needs_you x','m-al-3');await until(15000,()=>notes.some(n=>n.kind==='note'));
const nn=notes.find(n=>n.kind==='note');check('notify_person reaches it as a note carrying its text',nn?.body==='stand-in alert'&&nn.thread===T,JSON.stringify(nn));
r=await c2.rpc('desk.look',{thread:T,focused:true});await idle();const n1=notes.length;await sleep(1100);
await say('zqnotify:needs_you y','m-al-4');await sleep(4000);await idle();
check('a note is never held, even by a desk showing the thread',notes.length>n1,`${notes.length-n1} more`);
c2.ws.close();}

// ---- 3: routines: what was overdue fires once, a due one fires once a slot
L=events().length;await nextTick();await nextTick();
const fired=()=>since(L).filter(e=>e.$==='RoutineFired').reduce((a,e)=>(a[e.id]=(a[e.id]||0)+1,a),{});
let f=fired();const onCron=new Map();for(const e of fix)if(e.$==='RoutineSet')onCron.set(e.id,e);
const every30=[...onCron.values()].filter(e=>e.on&&e.cron.startsWith('*/30')).map(e=>e.id);
check('an overdue routine fires once, not per missed minute',Object.entries(f).every(([id,k])=>k===1||every30.includes(id)),JSON.stringify(Object.values(f)));
check('off and one-time routines stay quiet',Object.keys(f).every(id=>onCron.get(id)?.on&&onCron.get(id)?.cron),`${Object.keys(f).length} fired of ${routines.size}`);
const runs=()=>since(L).filter(e=>e.$==='RunSet'&&e.conv.startsWith('routine:')).length;
await until(20000,()=>runs()>=Object.keys(f).length);
check('each firing gets a run',runs()===Object.values(f).reduce((a,b)=>a+b,0),`${runs()} runs`);
const L3=events().length;r=await c.rpc('bots.routine',{bot:B,routine:'zq-minute',name:'zq',cron:'* * * * *',prompt:'zqtick',on:true});check('a new routine',r?.o?.ok===true);
await nextTick();const zq=()=>events().slice(L3).filter(e=>e.$==='RoutineFired'&&e.id==='zq-minute').length;
check('a due routine fires once in its minute',zq()===1,zq());
r=await c.rpc('bots.routine',{bot:B,routine:'zq-minute',name:'zq',prompt:'zqtick',on:false,keep:true});check('turned off',r?.o?.ok===true);
await until(20000,()=>since(L).filter(e=>e.$==='RunSet').every(x=>lastTurn(events(),x.thread)==='completed'));

// ---- 4: inbound: a thread_send, a launch report, a room post: filed, a triage run, Bub's Chat untouched
r=await c.rpc('bots.create',{name:'kit'});const KB=r.o.bot;await until(5000,()=>events().some(e=>e.$==='BotSet'&&e.id===KB&&e.thread));
const KT=events().findLast(e=>e.$==='BotSet'&&e.id===KB&&e.thread).thread;
const chat=()=>events().filter(e=>e.$==='MessagePosted'&&e.thread===T&&e.role==='user').length;
let chat0=chat();L=events().length;
await c.rpc('bots.say',{bot:KB,text:`zqsend:${T} now`,msg:'m-kit-1'});
await until(15000,()=>since(L).some(e=>e.$==='RunSet'));n=since(L);
check('thread_send to Bub is filed',n.some(e=>e.$==='MailPosted'&&e.to===`bot:${B}`&&e.want==='act'),n.filter(e=>e.$==='MailPosted').map(e=>`${e.kind}/${e.want}/${e.conv.split(':')[0]}`).join(' '));
check('and wakes a triage run',n.some(e=>e.$==='RunSet'&&e.bot===B));check('and stays out of his Chat',chat()===chat0);
execFileSync('mkdir',['-p',R+'/proj']);execFileSync('git',['init','-q',R+'/proj']);r=await c.rpc('project.add',{path:R+'/proj'});check('a project',r?.o?.ok===true);
const P=events().findLast(e=>e.$==='ProjectCreated').id,PT=r.o.thread;
await until(20000,()=>lastTurn(events(),T)!=='running');chat0=chat();L=events().length;
await say(`zqlaunch:${P} go`,'m-e2e-2');chat0++;
const child=await until(15000,()=>since(L).find(e=>e.$==='ThreadLaunched'));check('Bub launches into a project',child?.by===T);
await until(20000,()=>since(L).some(e=>e.$==='MailPosted'&&e.kind==='launch'));n=since(L);
check('the launch report is filed',n.some(e=>e.$==='MailPosted'&&e.kind==='launch'&&e.to===`bot:${B}`));
await until(15000,()=>since(L).some(e=>e.$==='RunSet'&&e.conv.startsWith('launch:')));
check('and wakes a triage run',since(L).some(e=>e.$==='RunSet'&&e.conv.startsWith('launch:')));check('and stays out of his Chat',chat()===chat0,`${chat()-chat0}`);
r=await c.rpc('bots.room',{name:'zqroom',members:`${B}\n${KB}`});const RM=events().findLast(e=>e.$==='RoomSet').id;L=events().length;chat0=chat();
await c.rpc('bots.post',{room:RM,text:'@bub a word zqmention'});
await until(15000,()=>since(L).some(e=>e.$==='RunSet'&&e.conv===`room:${RM}`));n=since(L);
check('a room post is filed',n.some(e=>e.$==='MailPosted'&&e.conv===`room:${RM}`));check('and wakes a triage run',n.some(e=>e.$==='RunSet'&&e.conv===`room:${RM}`));check('and stays out of his Chat',chat()===chat0);
const runIds=()=>events().filter(e=>e.$==='RunSet').map(e=>e.thread);
await until(30000,()=>runIds().every(t=>lastTurn(events(),t)!=='running'));
check('every run ended',runIds().every(t=>lastTurn(events(),t)!=='running'),`${new Set(runIds()).size} runs`);
await sleep(1500);
const startsOf=new Map(turns().filter(t=>t.ev==='start').map(t=>[String(t.pid),t.argv.join(' ')]));
const runProcs=live(hubPid()).filter(p=>/\/mcp\/(run|mailcoord-)/.test(startsOf.get(p)||''));
check('no agent process left behind by a finished run',runProcs.length===0,`${runProcs.length} of ${live(hubPid()).length}`);

// ---- 5: restart
const t2=(await c.rpc('thread.create',{project:P,title:'zq2'})).o.thread,t3=(await c.rpc('thread.create',{project:P,title:'zq3'})).o.thread;
const cut=[T,KT,PT,t2,t3];
await say('zqhold a','m-h-1');await c.rpc('bots.say',{bot:KB,text:'zqhold b',msg:'m-h-2'});for(const t of [PT,t2,t3])await c.rpc('turn.start',{thread:t,text:'zqhold c'});
await until(10000,()=>cut.every(t=>lastTurn(events(),t)==='running'));check('five turns running',cut.every(t=>lastTurn(events(),t)==='running'));
L=events().length;hub('term');console.log(hub('start'));({h,c}=await connect());
await until(25000,()=>cut.every(t=>lastTurn(events(),t)==='completed'));n=since(L);
for(const t of cut)check(`${t===T?'Bub':'a thread'} continues after SIGTERM`,lastTurn(events(),t)==='completed'&&n.filter(e=>e.$==='MessageFrom'&&e.thread===t&&e.kind==='carry').length===1);
const hadSession=cut.filter(t=>events().slice(0,L).some(x=>x.$==='SessionSet'&&x.id===t)).length;  // a turn cut before its agent said its session has nothing to resume
const resumed=turns().filter(t=>t.ev==='turn').slice(-8).filter(t=>t.resume).length;check('with their sessions',resumed>=hadSession&&hadSession>=2,`${resumed} resumed, ${hadSession} had one`);

// ---- no real model ever ran
const pid=hubPid();const envs=readFileSync(`/proc/${pid}/environ`,'utf8').split('\0');check('stand-ins only',envs.includes(`BACKPLANE_AGENTS=${R}/ag`));
hub('stop');console.log(fails?`${fails} FAILED`:'ALL PASS');process.exit(fails?1:0);
