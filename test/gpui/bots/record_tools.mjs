// Record the JS hub's bot MCP tools: the tools/list a bot's agent gets and the result of each call, with the events they leave (the oracle for bots/tools.rs).
// usage: bun test/gpui/bots/record_tools.mjs OUTDIR     (JS_HUB=build/gpui/backplane-serve PORT=3915; a temp home; stand-in agents only)
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync,copyFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
const out=process.argv[2];
const work=out+'/work';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec bun ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve',home,port:Number(process.env.PORT||3915),standins:work+'/bin',extraEnv:{FAKE_LOG:work+'/fake.log',BP_STANDIN_MODE:'plain',TZ:'Etc/UTC'}});
console.log(execFileSync('test/gpui/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(3000);const c=await client(h);
const R=async(m,p)=>(await c.rpc(m,p))?.o;
const ada=(await R('bots.create',{name:'ada',persona:'dry'})).bot;await sleep(300);
const bob=(await R('bots.create',{name:'bob'})).bot;await sleep(300);
await R('bots.say',{bot:ada,text:'hello'});await sleep(4000);
const mcp=readFileSync(work+'/fake.log','utf8').split('\n').filter(l=>l.startsWith('MCP ')).at(-1).split(' ')[2];
let id=0;const rpc=async(method,params)=>JSON.parse(await (await fetch(mcp,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:++id,method,params})})).text());
const calls=[];const T=async(name,args)=>{const r=await rpc('tools/call',{name,arguments:args});calls.push({name,args,result:r.result??r.error});await sleep(400);return r};
const list=await rpc('tools/list',{});
await T('bot_whoami',{});await T('bot_list',{});
await T('memory_save',{key:'Pref One',kind:'pref',text:'likes tea',tags:'drink'});await sleep(1100);
await T('memory_save',{key:'fact.two',kind:'weird',text:'two'});
await T('memory_recall',{query:'tea'});await T('memory_recall',{query:'zzz'});await T('memory_list',{});await T('memory_list',{kind:'pref'});await T('memory_forget',{key:'pref-one'});await T('memory_list',{});
await T('space_set',{spec:'{"title":"Hi","blocks":[{"type":"text","text":"x"}]}'});await T('space_set',{spec:'nope'});await T('space_get',{});
await T('space_page',{html:'<p>x</p>'});await T('space_get',{});await T('space_page',{html:''});
await T('room_create',{name:'lab',members:'bob'});await T('room_list',{});
const rooms=calls.at(-1).result;
await T('bot_send',{to:'bob',text:'hi bob',topic:'greet'});await T('bot_send',{to:'bob',text:'a private word',direct:true});await T('bot_send',{to:'nobody',text:'x'});await sleep(1500);
await T('routine_set',{id:'rt1',name:'later',at:'in 3d',prompt:'p'});await T('routine_set',{name:'daily',schedule:'daily 09:30',prompt:'q'});await T('routine_set',{name:'bad',schedule:'whenever',prompt:'q'});await T('routine_list',{});await T('routine_delete',{id:'rt1'});await T('routine_delete',{id:'nope'});
await T('project_list',{});await T('remind_person',{at:'in 20m',text:'water the plants'});await T('remind_person',{at:'banana',text:'x'});await T('person_timezone',{});await T('person_timezone',{zone:'Europe/Helsinki'});await T('person_timezone',{zone:'Not/AZone'});await T('voice_dictionary_add',{term:'Oulu'});
await T('no_such_tool',{});
// rooms by id after the fact
const roomIds=JSON.parse(rooms.content?.[0]?.text||'[]');
if(Array.isArray(roomIds)&&roomIds[0]){const rid=roomIds[0].id||roomIds[0].room;await T('room_post',{room:rid,text:'hello room'});await T('room_post',{room:'nope',text:'x'});await T('room_read',{room:rid});await T('room_read',{room:'nope'});await T('room_read',{room:'bob'})}
await sleep(1500);stopHub(h);await sleep(1500);
copyFileSync(home+'/events.jsonl',out+'/events-tools.jsonl');
writeFileSync(out+'/tools-bot.json',JSON.stringify(list,null,1));writeFileSync(out+'/calls-bot.json',JSON.stringify(calls,null,1));
console.log('wrote',out,calls.length,'calls');process.exit(0);
