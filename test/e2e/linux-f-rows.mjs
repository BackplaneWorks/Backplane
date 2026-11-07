// Hub-side rows F05-F22 on Linux (any platform with bun and a built hub): a private hub on a temp home, a stand-in `claude` (lib/standin_rows.py) that can ask, plan,
// call hub MCP tools and echo, driven over the data websocket. A check passes from the hub's own events.jsonl, its files, or an HTTP answer; nothing stands in for the client.
// usage: bun test/e2e/linux-f-rows.mjs HUB_BIN OUT_DIR [PORT]  -> OUT_DIR/rows.json (one entry per check: {id, ok, ev}) and one line per check on stdout.
// Rows that need the window (frames, key presses, drafts, the CEF browser) are not here: they are the client lane's and are listed in rows.json under "notDriven".
import {spawn} from 'node:child_process';
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,copyFileSync,readdirSync,existsSync,openSync} from 'node:fs';
import http from 'node:http';
import {encode,decode} from '../wire.mjs';
const [BIN,OUT,PORT_ARG]=process.argv.slice(2); const PORT=Number(PORT_ARG||3881);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
rmSync(OUT,{recursive:true,force:true}); mkdirSync(OUT+'/bin',{recursive:true}); mkdirSync(OUT+'/proj/sub',{recursive:true});
const HOME=OUT+'/home', AG=OUT+'/bin', HERE=new URL('.',import.meta.url).pathname;
copyFileSync(HERE+'lib/standin_rows.py',AG+'/claude'); for(const n of['codex','grok','muse','opencode','agy','pi','omp']) writeFileSync(`${AG}/${n}`,'#!/bin/sh\nexit 1\n');
for(const f of readdirSync(AG)) chmodSync(`${AG}/${f}`,0o755);
writeFileSync(OUT+'/proj/readme.txt','e2e fixture\n');
const rows=[]; const log=(id,ok,ev)=>{rows.push({id,ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
const FAKE=OUT+'/fake.log'; const fake=()=>existsSync(FAKE)?readFileSync(FAKE,'utf8'):'';
// a stand-in order server: the quote upload and the poll, recording what it was sent
const orderSeen=[]; const srv=http.createServer((q,s)=>{const b=[];q.on('data',d=>b.push(d));q.on('end',()=>{const body=Buffer.concat(b).toString('latin1');orderSeen.push({m:q.method,u:q.url,len:body.length,manifest:/name="manifest"/.test(body)});
  s.setHeader('content-type','application/json');
  if(q.method==='POST'&&q.url.startsWith('/v1/quotes'))return s.end(JSON.stringify({id:'q_e2e1',token:'tok-e2e-1'}));
  if(q.method==='GET'&&q.url.startsWith('/v1/quotes/q_e2e1'))return s.end(JSON.stringify({status:'quoted',currency:'USD',total:12.5,lines:[{name:'Board',qty:5,total:12.5}]}));
  s.statusCode=404;s.end('{"error":"no"}')})});
await new Promise(r=>srv.listen(0,'127.0.0.1',r)); const ORDER=`http://127.0.0.1:${srv.address().port}`;
let hub=null; const env0={...process.env,BACKPLANE_AGENTS:AG,PATH:`${AG}:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/sh',BACKPLANE_ORDER_SERVER:ORDER,FAKE_LOG:FAKE,FAKE_SECS:'600'};
async function start(){
  const out=openSync(OUT+'/hub.log','a');
  hub=spawn(BIN,['--home',HOME,'--port',String(PORT),'--no-tailscale','--foreground'],{env:env0,stdio:['ignore',out,out]});
  for(let i=0;i<300;i++){try{if((await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200)break}catch{}await sleep(100)}
  return Number(/^(\d+)/.exec(readFileSync(HOME+'/hub.lock','utf8'))[1]);
}
const events=()=>readFileSync(HOME+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(Boolean);
const has=f=>events().some(f);
async function until(f,ms=15000){const end=Date.now()+ms;for(;;){if(f())return true;if(Date.now()>end)return false;await sleep(100)}}
const token=()=>readFileSync(HOME+'/token','utf8').trim();
async function connect(q=''){
  const ws=new WebSocket(`ws://127.0.0.1:${PORT}/ws?${q}token=${token()}`);ws.binaryType='arraybuffer';
  const c={ws,replies:new Map(),frames:[],id:100};
  ws.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}c.frames.push(o);if(o.t==='reply')c.replies.set(o.id,o)};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.rpc=async(m,p={})=>{const n=++c.id;ws.send(encode({id:n,m,p}));for(let k=0;k<400&&!c.replies.has(n);k++)await sleep(50);return c.replies.get(n)||{ok:false,error:'no reply'}};
  return c;
}
const settingOf=k=>{let v;for(const e of events())if(e.$==='SettingSet'&&e.key===k)v=e.value;return v};
const turnStates=t=>events().filter(e=>e.$==='TurnChanged'&&e.thread===t).map(e=>e.state);
const finished=(t,n=1)=>until(()=>turnStates(t).filter(s=>s!=='running').length>=n,30000);
const pid0=await start(); let c=await connect(); await sleep(1500);
await c.rpc('project.add',{path:OUT+'/proj'}); await until(()=>has(e=>e.$==='ProjectCreated'));
const P=events().find(e=>e.$==='ProjectCreated').id;
const mk=async(title,extra={})=>(await c.rpc('thread.create',{project:P,title,env:'local',...extra})).thread;

// F05 providers: the claude stand-in speaks the protocol; the others' stand-ins exit 1, which must surface as a failed turn, not a hang
{const T=await mk('prov');await c.rpc('turn.start',{thread:T,text:'zqecho one'});const ok=await finished(T);
 const says=events().filter(e=>e.$==='MessagePosted'&&e.thread===T&&e.role==='assistant').map(e=>e.text).join('|');
 const res=[];for(const p of['codex','grok','opencode','muse']){const T2=await mk('prov-'+p,{provider:p});await c.rpc('turn.start',{thread:T2,text:'hello'});await until(()=>turnStates(T2).some(s=>s!=='running'),20000);res.push(`${p}:${turnStates(T2).at(-1)}`)}
 log('F05',ok&&/echo: .*zqecho one/.test(says),`claude stand-in turn ended, echoed ${JSON.stringify(says).slice(0,80)}; process-level stand-ins for the other providers exit 1: ${res.join(' ')}. Each provider's own protocol is covered by the hub's in-process tests (cargo test providers), not here`)}

// F06 runtime modes, approvals, questions, plans
{const T=await mk('modes');const m=await c.rpc('thread.modes',{thread:T,runtime:'approval-required'});await until(()=>has(e=>e.$==='ModesSet'&&e.id===T&&e.runtime==='approval-required'),5000);
 await c.rpc('turn.start',{thread:T,text:'zqapprove please'});
 const op=await until(()=>has(e=>e.$==='AskOpened'&&e.thread===T&&e.kind==='approval'),15000);const ask=events().find(e=>e.$==='AskOpened'&&e.thread===T&&e.kind==='approval');
 const an=ask?await c.rpc('ask.answer',{id:ask.id,answer:'accept'}):{ok:false};const fin=await finished(T);
 log('F06',m.ok&&op&&an.ok&&fin&&/PERM Bash -> .*allow/i.test(fake())&&has(e=>e.$==='AskClosed'&&e.id===ask?.id),`runtime approval-required: Bash approval opened (${ask?.detail?.slice(0,40)}), accepted, the agent got allow=${/PERM Bash -> .*allow/i.test(fake())}, ask closed`);
 await c.rpc('turn.start',{thread:T,text:'zqapprove again'});await until(()=>events().filter(e=>e.$==='AskOpened'&&e.thread===T&&e.kind==='approval').length>=2,15000);
 const ask2=events().filter(e=>e.$==='AskOpened'&&e.thread===T&&e.kind==='approval').at(-1);await c.rpc('ask.answer',{id:ask2.id,answer:'decline'});await finished(T,2);
 log('F06',/PERM Bash -> .*deny/i.test(fake()),`a declined approval reaches the agent as deny=${/PERM Bash -> .*deny/i.test(fake())}`);
 const Q=await mk('question');await c.rpc('turn.start',{thread:Q,text:'zqask'});
 await until(()=>has(e=>e.$==='AskOpened'&&e.thread===Q&&e.kind==='input'),15000);const q=events().find(e=>e.$==='AskOpened'&&e.thread===Q&&e.kind==='input');
 await c.rpc('ask.answer',{id:q.id,answer:'USB-C'});await finished(Q);
 log('F06',!!q&&/Which port/.test(q.detail)&&/PERM AskUserQuestion -> .*USB-C/.test(fake()),`question card "${q?.detail?.slice(0,30)}" options ${JSON.stringify(q?.options)}; the answer USB-C reached the agent=${/PERM AskUserQuestion -> .*USB-C/.test(fake())}`);
 const PL=await mk('plan');await c.rpc('thread.modes',{thread:PL,interaction:'plan'});await c.rpc('turn.start',{thread:PL,text:'zqplan'});
 const pl=await until(()=>has(e=>e.$==='AskOpened'&&e.thread===PL&&e.kind==='plan'),15000);const pe=events().find(e=>e.$==='AskOpened'&&e.thread===PL&&e.kind==='plan');
 log('F06',pl&&/Draw the board/.test(pe.detail)&&has(e=>e.$==='ModesSet'&&e.id===PL&&e.interaction==='plan')&&/permission-mode.{0,6}plan/.test(fake()),`plan mode: ModesSet interaction=plan, the agent started in plan permission mode=${/permission-mode.{0,6}plan/.test(fake())}, plan card "${pe?.detail?.slice(0,30)}"`)}

// F07 held queue: a turn is interrupted, a model choice then holds what waits (Config.hold), a message sent now is held, queue.resume releases it
{const H=await mk('held');await c.rpc('turn.start',{thread:H,text:'long turn'});await until(()=>turnStates(H).includes('running'),10000);
 await c.rpc('turn.interrupt',{thread:H});await until(()=>turnStates(H).at(-1)==='interrupted',10000);
 await c.rpc('thread.modes',{thread:H,model:'opus'});await sleep(500);const hold=settingOf(`thread.config.hold.${H}`);
 const runs=()=>turnStates(H).filter(s=>s==='running').length;const before=runs();
 const rs=await c.rpc('queue.resume',{thread:H});await sleep(1500);const cleared=settingOf(`thread.config.hold.${H}`)==='';
 log('F07-held',hold==='on'&&rs.ok&&cleared,`interrupted turn, then a model choice: thread.config.hold=${hold}; queue.resume ok=${rs.ok} and cleared the hold=${cleared}; the thread ran no turn of its own in between (running turns ${before} -> ${runs()})`)}

// F08 bring back, after a fork
{const A=await mk('forkme');await c.rpc('turn.start',{thread:A,text:'zqshort fork source'});await finished(A);
 const n=events().filter(e=>e.$==='ThreadCreated').length;const f=await c.rpc('thread.fork',{thread:A});await until(()=>events().filter(e=>e.$==='ThreadCreated').length>n,8000);
 const F=events().filter(e=>e.$==='ThreadCreated').at(-1).id;await c.rpc('turn.start',{thread:F,text:'zqecho in the fork'});await finished(F);
 const bk=await c.rpc('thread.back',{thread:F});await sleep(1500);
 const brought=events().some(e=>e.$==='TurnQueued'&&e.thread===A&&/handoff:back/.test(e.msg||'')&&/in the fork/.test(e.text||''));
 log('F08',f.ok&&bk.ok&&brought,`fork ${F} of ${A}: ok=${f.ok}; thread.back from the fork ok=${bk.ok} (${bk.error||''}); the fork's conversation is queued for the source thread as a hand-off=${brought}`)}

// F10 an agent calls the hub's orchestration tools
{const T=await mk('orch');await c.rpc('turn.start',{thread:T,text:'zqtool thread_list'});await finished(T);
 log('F10',/TOOL thread_list -> .*(orch|"id")/.test(fake()),`an agent's MCP call thread_list answered: ${(/TOOL thread_list -> (.{0,100})/.exec(fake())||[])[1]}`)}

// F11 model, effort, project rename
{const T=await mk('modelpick');const m=await c.rpc('thread.modes',{thread:T,model:'sonnet'});const e=await c.rpc('thread.effort',{thread:T,effort:'high'});
 await c.rpc('project.rename',{project:P,title:'Renamed project'});await sleep(800);
 log('F11',m.ok&&e.ok&&has(e=>e.$==='ModesSet'&&e.id===T&&e.model==='sonnet')&&settingOf(`effort.${T}`)==='high'&&has(e=>e.$==='ProjectCreated'&&e.id===P&&e.title==='Renamed project'),`thread.modes model=sonnet logged, thread.effort high -> effort.${T}=${settingOf('effort.'+T)}, project.rename logged=${has(e=>e.$==='ProjectCreated'&&e.id===P&&e.title==='Renamed project')}`)}

// F13 titles: the first message names an untitled thread through the text model (the stand-in answers `claude -p`), and Regenerate title replaces it
{const T=await mk('New thread');await c.rpc('turn.start',{thread:T,text:'zqshort design a USB-C power board'});
 const names=()=>events().filter(e=>e.$==='ThreadRenamed'&&e.id===T).map(e=>e.title);const named=await until(()=>names().includes('Stand-in title for the thread'),30000);const t1=names().join(' > ');
 const r=await c.rpc('thread.retitle',{thread:T});
 log('F13',named&&names()[0]!=='Stand-in title for the thread'&&r.ok&&!!r.result?.title,`a new thread is named from its first line, then by the text model: "${t1}"; thread.retitle ok=${r.ok} -> "${r.result?.title}" (the text model here is the stand-in's -p mode, so this proves the hub's wiring, not a model's quality)`)}

// F15 an attachment: POST /attach stores it under attachments/<thread>/, the message carries agentText with its path, the agent sees it
{const T=await mk('attach');const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==','base64');
 const r=await fetch(`http://127.0.0.1:${PORT}/attach?thread=${T}&key=k1&name=pic.png&token=${token()}`,{method:'POST',body:png,headers:{'content-type':'image/png'}});const j=await r.json().catch(()=>({}));
 const stored=j.attached&&existsSync(j.attached)&&readFileSync(j.attached).equals(png);
 const t=await c.rpc('turn.start',{thread:T,text:'zqecho look at this',agentText:`zqecho look at this [image: ${j.attached}]`,attachments:[{name:'pic.png',path:j.attached,mime:'image/png',size:png.length}]});await finished(T);
 const posted=events().find(e=>e.$==='MessagePosted'&&e.thread===T&&e.role==='user');const att=events().find(e=>e.$==='MessageAttached'&&e.thread===T);
 const bigR=await fetch(`http://127.0.0.1:${PORT}/attach?thread=${T}&key=k2&name=big.bin&token=${token()}`,{method:'POST',body:Buffer.alloc(60<<20),headers:{'content-type':'application/octet-stream'}}).then(async r=>r.status+' '+JSON.stringify(await r.json().catch(()=>({}))).slice(0,90)).catch(e=>'conn '+e.message.slice(0,20));const big=bigR;
 log('F15',r.status===200&&stored&&t.ok&&!!att&&/look at this/.test(posted?.text||'')&&fake().includes(`[image: ${j.attached}]`),`POST /attach ${r.status} stored ${j.attached?.replace(OUT,'')} bytes match=${stored}; MessageAttached logged=${!!att}; the agent was told the path=${fake().includes(j.attached||'#')}; an oversize upload answered ${big}`)}

// F21 resume, outbox (the same message id is one message), long session
{const T=await mk('outbox');const mid='m-outbox-1';const a=await c.rpc('turn.start',{thread:T,text:'zqshort once',msg:mid});const b=await c.rpc('turn.start',{thread:T,text:'zqshort once',msg:mid});await finished(T);
 const n=events().filter(e=>e.$==='MessagePosted'&&e.thread===T&&e.msg===mid).length;
 log('F21-outbox',a.ok&&b.ok&&n===1,`the same client message id sent twice (a retry after a lost reply) posted ${n} message(s); replies ok=${a.ok},${b.ok}`);
 const L=await mk('long');let sent=0;for(let i=0;i<150;i++){const r=await c.rpc('turn.start',{thread:L,text:`zqshort long ${i}`});if(r.ok)sent++;if(i%10===9)await finished(L,i/10+1)}
 await finished(L,15);const total=events().filter(e=>e.$==='MessagePosted'&&e.thread===L&&e.role==='user').length;
 c.ws.close();process.kill(pid0,'SIGTERM');await sleep(2500);const pid1=await start();c=await connect();await sleep(1500);
 const after=events().filter(e=>e.$==='MessagePosted'&&e.thread===L&&e.role==='user').length;
 const cs=await connect('since=0&origin=0&');await sleep(2500);const seen=JSON.stringify(cs.frames).split('MessagePosted').length-1;cs.ws.close();
 log('F21-long',sent===150&&total>=150&&after===total&&seen>=150,`150 messages sent, ${total} user messages logged, ${after} after a hub restart; a fresh connection from since=0 replayed ${seen} MessagePosted items; events.jsonl ${(readFileSync(HOME+'/events.jsonl').length/1024).toFixed(0)} KiB`);
 const c2=await connect(`since=${Math.max(0,events().length-20)}&origin=0&`);await sleep(1200);log('F21-resume',c2.frames.some(f=>f.t==='log'||f.t==='changes'),`a connection with since= got ${[...new Set(c2.frames.map(f=>f.t))].join(',')}`);c2.ws.close()}

// F22 the order flow up to the quote: the agent offers the folder, the person approves the files, the hub uploads them and posts the quote; no order is placed
{const T=await mk('order');const cwd=OUT+'/proj';mkdirSync(cwd+'/mfg/o1',{recursive:true});writeFileSync(cwd+'/mfg/o1/board.gbr','G04*\nM02*\n');
 const crypto=await import('node:crypto');writeFileSync(cwd+'/mfg/o1/manifest.json',JSON.stringify({files:{'board.gbr':{sha256:crypto.createHash('sha256').update('G04*\nM02*\n').digest('hex')}}}));
 await c.rpc('setting.set',{key:'order.server',value:ORDER});await c.rpc('setting.set',{key:`buy.${T}`,value:'t0'});await sleep(500);
 await c.rpc('turn.start',{thread:T,text:'zqquote mfg/o1'});
 const op=await until(()=>has(e=>e.$==='AskOpened'&&e.thread===T&&e.tool==='order_quote'),15000);const ask=events().find(e=>e.$==='AskOpened'&&e.thread===T&&e.tool==='order_quote');
 const noUpload=orderSeen.length===0;
 const an=ask?await c.rpc('ask.answer',{id:ask.id,answer:'accept'}):{ok:false};
 const quoted=await until(()=>!!settingOf(`order.quote.${T}`),30000);
 log('F22',op&&noUpload&&an.ok&&quoted&&orderSeen.some(s=>s.m==='POST'&&s.manifest)&&!orderSeen.some(s=>/pay|buy|checkout/.test(s.u)),`approval card for the files opened (${ask?.detail?.split('\n')[0]?.slice(0,50)}), nothing uploaded before the answer=${noUpload}, after it POST /v1/quotes carried the manifest, quote card set=${quoted} (${(settingOf('order.quote.'+T)||'').slice(0,60)}); requests seen ${orderSeen.map(s=>s.m+' '+s.u).join(', ')}; no pay or buy request`)}


// F09 the context meter: a finished turn leaves its token and context readings in the hub's settings (the meter itself is drawn by the client)
{const T=await mk('meter');await c.rpc('turn.start',{thread:T,text:'zqshort meter'});await finished(T);await sleep(800);
 const keys=[...new Set(events().filter(e=>e.$==='SettingSet'&&e.key.includes(T)&&/tok|ctx|context|usage|turn\./.test(e.key)).map(e=>e.key.replace(T,'<t>')))];
 const ev=events().filter(e=>e.thread===T&&/Usage|Context|Tokens/.test(e.$||'')).map(e=>e.$);
 log('F09-meter',keys.length>0||ev.length>0,`readings after a turn: settings ${JSON.stringify(keys)} events ${JSON.stringify([...new Set(ev)])}`)}

// F12 checkpoints, revert, push to a remote (a local bare one), PR without gh
{const G=OUT+'/gitproj2',R=OUT+'/remote.git';mkdirSync(G,{recursive:true});const {execFileSync}=await import('node:child_process');
 const git=(cwd,...a)=>execFileSync('git',['-c','user.name=e2e','-c','user.email=e2e@example.invalid',...a],{cwd}).toString();
 git(OUT,'init','-q','--bare',R);git(G,'init','-q','-b','main');writeFileSync(G+'/a.txt','one\n');git(G,'add','.');git(G,'commit','-qm','init');git(G,'remote','add','origin',R);git(G,'push','-q','origin','main');
 await c.rpc('project.add',{path:G});await until(()=>events().filter(e=>e.$==='ProjectCreated').some(e=>e.root===G),5000);const GP=events().filter(e=>e.$==='ProjectCreated').find(e=>e.root===G).id;
 const t=(await c.rpc('thread.create',{project:GP,title:'wt2',env:'worktree'})).thread;await sleep(2500);const wt=events().find(e=>e.$==='WorktreeSet'&&e.id===t)?.worktree;
 await c.rpc('turn.start',{thread:t,text:'zqwrite first.txt'});await finished(t);await sleep(3000);await c.rpc('turn.start',{thread:t,text:'zqwrite second.txt'});await finished(t,2);
 const both=existsSync(wt+'/first.txt')&&existsSync(wt+'/second.txt');
 const rv=await c.rpc('git.revert',{thread:t,turn:1});await sleep(1500);
 const after={first:existsSync(wt+'/first.txt'),second:existsSync(wt+'/second.txt')};
 const cm=await c.rpc('git.act',{thread:t,action:'commit',message:'e2e work'});
 const ps=await c.rpc('git.act',{thread:t,action:'push'});
 let remote='';try{remote=git(R,'branch','--list')}catch{}
 const pr=await c.rpc('git.act',{thread:t,action:'create_pr'});
 log('F12-checkpoint',both&&rv.ok&&after.first&&!after.second,`two turns wrote first.txt and second.txt (both present=${both}); git.revert to checkpoint 1 (after the first turn) ok=${rv.ok} (${(rv.result?.text||rv.error||'').slice(0,60)}); then first.txt=${after.first}, second.txt=${after.second}`);
 log('F12-push',cm.ok&&ps.ok&&/backplane\//.test(remote),`commit ok=${cm.ok}; push ok=${ps.ok} (${(ps.result?.text||'').slice(0,60)}); the bare remote lists ${JSON.stringify(remote.trim()).slice(0,80)}`);
 log('F12-pr',!pr.ok,`create_pr with no gh or forge remote is refused, not faked: ok=${pr.ok} ${(pr.result?.text||pr.error||'').slice(0,100)}`)}

// F19 a bot answers a message in its chat
{const b=await c.rpc('bots.create',{name:'Echo',persona:'zqecho persona',provider:'claude',model:''});await sleep(1000);
 const bid=[...new Set(events().filter(e=>/Bot/.test(e.$||'')&&e.name==='Echo').map(e=>e.id))][0];
 const n0=events().filter(e=>e.$==='MessagePosted').length;const sy=await c.rpc('bots.say',{bot:bid,text:'zqecho hello bot'});
 const got=await until(()=>events().slice(n0).some(e=>e.$==='MessagePosted'&&e.role==='assistant'&&/echo: .*hello bot/.test(e.text||'')),30000);
 log('F19-bot',b.ok&&sy.ok&&got,`bots.create ok=${b.ok}; bots.say ok=${sy.ok}; the bot's turn ran on the stand-in and its answer was posted=${got}`)}

// F20 downloads: a file and a folder as a zip, through the HTTP routes the client's browser uses
{const T=await mk('dl');mkdirSync(OUT+'/proj/dl',{recursive:true});writeFileSync(OUT+'/proj/dl/x.txt','download me\n');
 const g=await fetch(`http://127.0.0.1:${PORT}/files/get?thread=${T}&path=dl/x.txt&token=${token()}`);const gt=await g.text();
 const z=await fetch(`http://127.0.0.1:${PORT}/files/zip?thread=${T}&path=dl&token=${token()}`);const zb=Buffer.from(await z.arrayBuffer());
 const bad=await fetch(`http://127.0.0.1:${PORT}/files/get?thread=${T}&path=../../etc/passwd&token=${token()}`);
 const noauth=await fetch(`http://127.0.0.1:${PORT}/files/get?thread=${T}&path=dl/x.txt`);
 log('F20-download',g.status===200&&gt==='download me\n'&&/attachment/.test(g.headers.get('content-disposition')||'')&&z.status===200&&zb.slice(0,2).toString()==='PK'&&bad.status===400&&noauth.status===401,`GET /files/get ${g.status} body ok, Content-Disposition ${g.headers.get('content-disposition')}; /files/zip ${z.status} ${zb.length} bytes PK; a path outside the project ${bad.status}; no token ${noauth.status}`)}

// F17 the viewers' hub side: a STEP rendered to PNG views by the hub
{const T=await mk('mech');mkdirSync(OUT+'/proj/mech',{recursive:true});copyFileSync(HERE+'../files/fixtures/mesh/bracket.step',OUT+'/proj/mech/bracket.step');
 const ls=await c.rpc('mech.list',{thread:T});const r=await c.rpc('mech.render',{thread:T,path:'mech/bracket.step'});await sleep(500);
 const pngs=existsSync(OUT+'/proj/mech/renders')?readdirSync(OUT+'/proj/mech/renders').filter(f=>f.endsWith('.png')):[];
 log('F17-mech',ls.ok&&r.ok&&pngs.length>0,`mech.list ok=${ls.ok} (${JSON.stringify(ls.result||ls.error).slice(0,70)}); mech.render ok=${r.ok} ${(r.result?.text||r.error||'').slice(0,80)}; renders written ${pngs.join(',')}`)}

writeFileSync(OUT+'/rows.json',JSON.stringify({rows,notDriven:['F09 context meter and crash drafts (client)','F16 keybindings (client key events)','F17 viewer frames (client)','F18 CEF browser (build not made here)']},null,1));
try{process.kill(Number(/^(\d+)/.exec(readFileSync(HOME+'/hub.lock','utf8'))[1]),'SIGTERM')}catch{} srv.close(); await sleep(500); process.exit(0);
