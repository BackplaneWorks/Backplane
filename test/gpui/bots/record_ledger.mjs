// Record the JS hub's ledger tools (told_*, task_board_*), notify_person, project_create and plane_read: the tools/list of a bot's and a plain thread's agent, and
// each call's answer with the log lines before and after it (the oracle for bots/ledger.rs and bots/tools.rs; bots::tests::js_ledger_tools replays it).
// usage: bun test/gpui/bots/record_ledger.mjs OUTDIR     (JS_HUB=build/gpui/backplane-serve-current PORT=3921; a temp home and HOME; stand-in agents only)
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,copyFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
const out=process.argv[2];
const work=out+'/work';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});mkdirSync(work+'/fakehome',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec bun ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve-current',home,port:Number(process.env.PORT||3921),standins:work+'/bin',extraEnv:{FAKE_LOG:work+'/fake.log',BP_STREAM_N:'2',BP_STREAM_MS:'5',TZ:'Etc/UTC',HOME:work+'/fakehome'}});
console.log(execFileSync('test/gpui/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(3000);const c=await client(h);
const R=async(m,p)=>(await c.rpc(m,p))?.o;
const lines=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).length;
const urls=()=>readFileSync(work+'/fake.log','utf8').split('\n').filter(l=>l.startsWith('MCP ')).map(l=>l.split(' ')[2]);
const urlOf=th=>urls().filter(u=>u.includes(th)).at(-1);
let id=0;const rpc=async(url,method,params)=>JSON.parse(await (await fetch(url,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:++id,method,params})})).text());
const calls=[];
const T=async(who,th,name,args)=>{const pre=lines(),t=Math.floor(Date.now()/1000);const r=await rpc(urlOf(th),'tools/call',{name,arguments:args});await sleep(350);const call={who,thread:th,name,args,t,pre,post:lines(),result:r.result??r.error};calls.push(call);return call};
const ask=()=>lines();
const ada=(await R('bots.create',{name:'ada',persona:'dry'})).bot;await sleep(300);
const bob=(await R('bots.create',{name:'bob'})).bot;await sleep(300);
const threadOf=b=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l)).find(e=>e.$==='BotSet'&&e.id===b).thread;
const TA=threadOf(ada),TP='t1700000001-2',TL='t1700000002-3';
await R('bots.say',{bot:ada,text:'mode:stream hello'});await sleep(3000);
await R('turn.start',{thread:TP,text:'mode:stream hello'});await sleep(3000);
const A=(n,a={})=>T('bot',TA,n,a),P=(n,a={})=>T('plain',TP,n,a);
const listBot=await rpc(urlOf(TA),'tools/list',{}),listPlain=await rpc(urlOf(TP),'tools/list',{});
const idOf=c=>JSON.parse(c.result.content[0].text).id;
// told
await A('told_check',{});await A('told_mark',{text:'  '});
await A('told_mark',{key:'  Lunch: Dana!!  ',text:'Dana lunch in NYC Wed 10/28',channel:'digest'});await sleep(1100);
await A('told_mark',{text:'The new board revision shipped to the fab today',channel:'weird'});await sleep(1100);
await P('told_mark',{text:'ok',channel:''});
await A('told_check',{key:'lunch-dana'});await A('told_check',{key:' LUNCH: dana '});await A('told_check',{text:'dana lunch nyc wed'});await A('told_check',{text:'nothing in common at all'});await A('told_check',{text:'board revision',days:3});await P('told_check',{key:'lunch-dana',days:99});
// the board
await A('task_board_list',{});await A('task_board_add',{});await A('task_board_add',{title:'x'.repeat(301)});await A('task_board_add',{title:'y',owner:'nobody'});await A('task_board_add',{title:'y',project:'nope'});await A('task_board_add',{title:'y',due:'12/1'});await A('task_board_add',{title:'y',notes:'n'.repeat(4001)});
const i1=idOf(await A('task_board_add',{title:'Fix it',status:'DOING',owner:'me',due:'2026-12-01',group:'hw',links:'a b,c',notes:'first'}));await sleep(1100);
const i2=idOf(await A('task_board_add',{title:'Order parts',owner:'bob',project:'fixture',status:'nonsense'}));await sleep(1100);
const i3=idOf(await P('task_board_add',{title:'From a plain thread',owner:'me',project:'p1700000000-1',links:['u1',' u2 ','']}));await sleep(1100);
await A('task_board_list',{});await A('task_board_list',{all:true});await A('task_board_list',{status:'doing,todo'});await A('task_board_list',{status:'done'});await A('task_board_list',{owner:'me'});await A('task_board_list',{owner:'person'});await A('task_board_list',{owner:'bob'});await A('task_board_list',{owner:'nobody'});await A('task_board_list',{project:'fixture'});await A('task_board_list',{project:'nope'});await A('task_board_list',{group:'hw'});await P('task_board_list',{owner:'me'});
await A('task_board_update',{});await A('task_board_update',{id:'nope'});await A('task_board_update',{id:i1,status:'blocked',notes_append:'second'});await A('task_board_update',{id:i1,due:'',owner:'person',group:'sw'});await A('task_board_update',{id:i1,due:'bad'});await A('task_board_update',{id:i2,title:'Order more',links:'["a","b"]',project:''});
await A('task_board_list',{all:true});
await A('task_board_remove',{});await A('task_board_remove',{id:'nope'});await A('task_board_remove',{id:i3});await A('task_board_list',{all:true});
const md='# hw\n- [ ] one\n  a note\n\tanother\n- [x] two (due 2026-11-30)\n* [~] three due:2026-12-24\n1. [!] four\n+ [-] five\n\nplain line\n## fixture\n- [ ] in the project\n- [ ] one\n# hw\n- [ ] one\n- [q] not a box\n';
await A('task_board_import',{});await A('task_board_import',{markdown:md});await A('task_board_import',{markdown:md});await A('task_board_import',{markdown:'- [ ] mine\n',owner:'me',project:'fixture'});await A('task_board_import',{markdown:'- [ ] x\n',owner:'nobody'});await A('task_board_import',{markdown:'- [ ] x\n',project:'nope'});await A('task_board_list',{all:true});
// notify_person: a turn that is over, then one that runs
await A('notify_person',{text:'',reason:'done'});await A('notify_person',{text:'x',reason:'chat'});await A('notify_person',{text:'done late',reason:'done'});await A('notify_person',{text:'digest late',reason:'digest'});
await A('notify_person',{text:'need you\tnow\nsecond line',reason:'needs_you'});await A('notify_person',{text:'again',reason:'needs_you'});
await R('bots.say',{bot:ada,text:'hold:7000 do the thing'});await sleep(1500);
await A('notify_person',{text:'the thing is done',reason:'done'});await A('notify_person',{text:'done twice',reason:'done'});await A('notify_person',{text:'need again',reason:'needs_you'});await sleep(7000);
await R('setting.set',{key:'notify.needs',value:'off'});await R('bots.say',{bot:ada,text:'hold:4000 quiet one'});await sleep(1500);await A('notify_person',{text:'unheard',reason:'needs_you'});await sleep(4500);await R('setting.set',{key:'notify.needs',value:'on'});
await R('bots.routine',{bot:ada,routine:'ro1',name:'digest',schedule:'daily 09:30',prompt:'hold:4000 make the digest'});await R('bots.routine_run',{routine:'ro1'});await sleep(1500);
await A('notify_person',{text:'the digest',reason:'digest'});await A('notify_person',{text:'digest twice',reason:'reminder'});await A('plane_read',{});await sleep(4500);
await P('notify_person',{text:'plain',reason:'needs_you'});
// plane_read: the routine, a room with news, running, a question, an approval, a failure
await R('bots.room',{room:'r1',name:'lab',members:`${ada}\n${bob}`});await R('bots.post',{room:'r1',text:'news',msg:'p1'});await sleep(800);await A('room_post',{room:'lab',text:'from ada'});await sleep(2500);
await A('plane_read',{});await A('plane_read',{idle:true});await A('plane_read',{machine:'nowhere'});
await R('turn.start',{thread:TP,text:'hold:6000 run on'});await sleep(1500);await A('plane_read',{});await sleep(6000);
const made=await R('thread.create',{project:'p1700000000-1',title:'Boom',env:'local',provider:'claude'});const TF=made.thread??made.id;await sleep(500);
await R('turn.start',{thread:TF,text:'mode:exit-now boom'});await sleep(3000);await A('plane_read',{});
await P('work_run',{command:'sleep 40',note:'slow'});await P('wake_on',{kind:'thread',target:TL,on:'end'});await P('lock_acquire',{name:'merge',note:'landing'});await sleep(1500);await A('plane_read',{});
await R('turn.start',{thread:TL,text:'mode:stream ask: which one'});await sleep(2500);await A('plane_read',{});await A('plane_read',{idle:true});
// project_create: asked first; declined, accepted, for the session, a folder that is a project, a bad folder
const pc=async(a,answer)=>{const pre=lines(),t=Math.floor(Date.now()/1000);const p=rpc(urlOf(TA),'tools/call',{name:'project_create',arguments:a});await sleep(900);const mid=lines();
 const e=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l)).filter(x=>x.$==='AskOpened'&&x.tool==='project_create').at(-1);
 const plane=answer?await A('plane_read',{}):null;if(answer)await R('ask.answer',{id:e.id,answer});const r=await p;await sleep(500);
 calls.push({who:'bot',thread:TA,name:'project_create',args:a,answer,t,pre,mid,post:lines(),ask:e,result:r.result??r.error});};
const f1=work+'/made/one/deep',f2=work+'/made/two';
await pc({path:f1,name:'  One  '},'decline');await pc({path:f1,name:'  One  '},'accept');await pc({path:f1},'accept');await pc({path:f2},'session');await pc({path:work+'/made/three',name:'Three'},'');await pc({path:'~/tilde'},'');await pc({path:'bare'},'');
writeFileSync(work+'/afile','x');await pc({path:work+'/afile'},'');await pc({path:''},'');
await P('project_create',{path:work+'/made/four'});
await A('project_list',{});
await R('bots.uber',{bot:ada});await sleep(1500);await A('plane_read',{});
await sleep(1500);stopHub(h);await sleep(1500);
copyFileSync(home+'/events.jsonl',out+'/js-ledger.jsonl');
writeFileSync(out+'/js-ledger-calls.json',JSON.stringify({bot:ada,bob,thread:TA,work,listBot:listBot.result?.tools?.map(t=>t.name),listPlain:listPlain.result?.tools?.map(t=>t.name),calls},null,0));
writeFileSync(out+'/js-ledger-tools.json',JSON.stringify(listBot.result?.tools??[],null,1));
console.log('wrote',out,calls.length,'calls');process.exit(0);
