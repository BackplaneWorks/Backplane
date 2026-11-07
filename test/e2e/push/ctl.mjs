// bun ctl.mjs ROOT HUB_BIN PORT: a throwaway hub for the phone push checks and a small control port (PORT+1) the simulator's test drives. Stand-in agents only: every call below is
// a real turn, and the stand-in makes the real MCP tool call, so a push takes the path a real one takes. MODE=sink writes pushes to ROOT/sink (run.sh hands them to `simctl push`);
// MODE=apns sends them through APNs sandbox with the key the person's hub was configured with, by path. Only ever point a phone's token at this hub from a simulator.
import {chmodSync,existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep} from '../../perf/lib.mjs';
const [ROOT,BIN,PORT_='3860']=process.argv.slice(2), PORT=Number(PORT_), MODE=process.env.MODE||'sink';
mkdirSync(ROOT+'/proj',{recursive:true});mkdirSync(ROOT+'/sink',{recursive:true});makeStandins(ROOT+'/bin');
// the stand-in that makes real MCP calls (`mcpcall:`), in the place of the load one
writeFileSync(ROOT+'/bin/claude',`#!/bin/sh\nexec ${process.execPath} ${import.meta.dir}/../../tools/claude_standin.ts "$@"\n`);chmodSync(ROOT+'/bin/claude',0o755);
const FAKE=ROOT+'/fake.log';
const env={BP_STANDIN_MODE:'stream',BP_STREAM_N:'1',BP_STREAM_MS:'10',FAKE_LOG:FAKE,BACKPLANE_NO_UPDATE:'1',...(MODE==='sink'?{BACKPLANE_PUSH_SINK:ROOT+'/sink'}:{})};
const h=await startHub({bin:BIN,home:ROOT+'/home',port:PORT,standins:ROOT+'/bin',extraEnv:env,extraArgs:[],log:ROOT+'/hub.log'});
const c=await client(h);
const call=async(m,p)=>(await c.rpc(m,p,15000))?.o;
const events=()=>readFileSync(ROOT+'/home/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
const until=async(ms,f)=>{const end=Date.now()+ms;for(;;){const v=f();if(v)return v;if(Date.now()>end)return f();await sleep(100)}};
const set=(k,v)=>call('setting.set',{key:k,value:v});
await set('onboard.done','1');await set('notify.quiet','off');await set('push.burst.s','0');await set('person.tz','America/New_York');await set('tz.rule.America/New_York','EST5EDT,M3.2.0,M11.1.0');
if(MODE==='apns'){ // the three settings the person's hub had; values are read here and never printed
  const last={};for(const l of readFileSync(process.env.HOME+'/.backplane-bend/events.jsonl','utf8').split('\n')){if(!l.includes('"push.apns.'))continue;try{const e=JSON.parse(l);if(e.$==='SettingSet')last[e.key]=e.value}catch{}}
  for(const k of ['push.apns.key','push.apns.keyid','push.apns.team'])await set(k,last[k]);
}
const pr=await call('project.add',{path:ROOT+'/proj'});
const project=(await c.until(8000,()=>c.seen.find(x=>x.$==='ProjectCreated')))?.id||pr?.project;
const w=await call('thread.create',{project,title:'Worker thread',env:'local'});const worker=w.thread;
const bub=(await call('bots.create',{name:'Bub',persona:'stand-in',provider:'claude',model:'',uber:true})).bot;
const mochi=(await call('bots.create',{name:'Mochi',persona:'stand-in',provider:'claude',model:''})).bot;
await until(8000,()=>events().some(e=>e.$==='UberSet'));
const botThread=id=>events().filter(e=>e.$==='BotSet'&&e.id===id&&e.thread).at(-1)?.thread;
const mcpLines=()=>existsSync(FAKE)?readFileSync(FAKE,'utf8').split('\n').filter(l=>l.startsWith('MCPCALL')):[];
const tool=async(say,name,args)=>{const n=mcpLines().length;await say(`mcpcall:${JSON.stringify({name,arguments:args})}`);const l=await until(20000,()=>mcpLines().length>n);return mcpLines().at(-1)?.slice(0,400)||(l?'':'no call was made')};
const toBot=b=>t=>call('bots.say',{bot:b,text:t});
const toWorker=t=>c.send('turn.start',{thread:worker,text:t});
const state=()=>{const e=events();const n=x=>e.filter(v=>v.$===x).length;return {bub,mochi,worker,bubThread:botThread(bub),devices:e.filter(v=>v.$==='DeviceRegistered').length,envs:[...new Set(e.filter(v=>v.$==='DeviceRegistered').map(v=>v.env+'/'+v.bundle))],
  ordered:n('PushOrdered'),sent:n('PushSent'),cleared:n('PushCleared'),lastSent:e.filter(v=>v.$==='PushSent').map(({kind,key,thread,ask,title,body,badge,sent})=>({kind,key,thread,ask,title,body,badge,sent})).at(-1)||null,lastCleared:e.filter(v=>v.$==='PushCleared').at(-1)||null,
  sink:readdirSync(ROOT+'/sink').length}};
const J=o=>new Response(JSON.stringify(o),{headers:{'content-type':'application/json'}});
Bun.serve({port:PORT+1,hostname:'127.0.0.1',idleTimeout:60,async fetch(r){
  const u=new URL(r.url),q=Object.fromEntries(u.searchParams),a={reason:q.reason||'needs_you',text:q.text||''};if(q.thread)a.thread=q.thread;if(q.ask)a.ask=q.ask;
  switch(u.pathname){
    case '/state':return J(state());
    case '/uber':return J({said:await tool(toBot(bub),'notify_person',a),state:state()});
    case '/remind':return J({said:await tool(toBot(bub),'remind_person',{text:q.text,at:q.at||'in 1m'}),state:state()});
    case '/other':return J({said:await tool(toBot(mochi),'notify_person',a),state:state()});
    case '/worker':return J({said:await tool(toWorker,'notify_person',a),state:state()});
    case '/workerdone':{const n=events().filter(e=>e.$==='TurnChanged'&&e.thread===worker&&e.state==='completed').length;toWorker('finish this');await until(15000,()=>events().filter(e=>e.$==='TurnChanged'&&e.thread===worker&&e.state==='completed').length>n);return J(state())}
    case '/ask':{toWorker('approve: please');const o=await until(15000,()=>events().filter(e=>e.$==='AskOpened'&&e.thread===worker).at(-1));return J({ask:o?.id||'',thread:worker,state:state()})}
    case '/answer':return J({r:await call('ask.answer',{id:q.ask,answer:q.answer||'accept'}),state:state()});
    case '/setting':await set(q.k,q.v??'');return J(state());
    case '/seen':return J({r:await call('push.seen',{thread:q.thread}),state:state()});
    case '/mute':{const t=events().filter(e=>e.$==='DeviceRegistered').at(-1)?.token;return J({r:await call('device.prefs',{token:t,mute:q.on==='1'}),state:state()})}
    case '/stop':setTimeout(async()=>{try{c.ws.close()}catch{}stopHub(h);process.exit(0)},200);return J({stopping:true});
    default:return new Response('no',{status:404});
  }}});
console.log(`ready hub ${PORT} ctl ${PORT+1} mode ${MODE} token-file ${ROOT}/home/token`);
