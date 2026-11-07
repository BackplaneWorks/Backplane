// Effects of Settings controls observable without a window (hub files, processes, HTTP): files.max.mb, person.tz, telemetry.share. Same rig as mac-hub-rows.mjs.
// usage (on the machine): node test/e2e/mac-effects.mjs HUB_BIN OUT_DIR [PORT]   -> one line per check on stdout, OUT_DIR/effects.json
// A check passes only from events in the hub's own log (events.jsonl) or the HTTP answer; nothing here stands in for the client.
import {spawn,execFileSync} from 'node:child_process';
import net from 'node:net';
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,existsSync,openSync,readdirSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
import {standinScript} from '../tools/perf_standin.ts';
const [BIN,OUT,PORT_ARG]=process.argv.slice(2); const PORT=Number(PORT_ARG||3801);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
rmSync(OUT,{recursive:true,force:true}); mkdirSync(OUT+'/bin',{recursive:true}); mkdirSync(OUT+'/proj/sub',{recursive:true});
const HOME=OUT+'/home', AG=OUT+'/bin';
writeFileSync(AG+'/claude',standinScript.replace('echo "START','echo "$*" >> "$FAKE_LOG.argv"; echo "START')); for(const n of['codex','grok','muse','opencode']) writeFileSync(`${AG}/${n}`,'#!/bin/sh\nexit 1\n');
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
const out_rows=[]; const row=(id,ok,ev)=>{out_rows.push({id,ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
const pid=await start(600); const token=readFileSync(HOME+'/token','utf8').trim();
let c=await connect(); await sleep(1500);
await c.rpc('project.add',{path:OUT+'/proj'}); await until(()=>has(e=>e.$==='ProjectCreated'));
const P=events().find(e=>e.$==='ProjectCreated').id;
const T=(await c.rpc('thread.create',{project:P,title:'effects',env:'local'})).thread;
const set=async(k,v)=>{await c.rpc('setting.set',{key:k,value:v});await sleep(300)};
// files.max.mb: POST /attach with a body over / under the limit
const up=(mb,name)=>new Promise(res=>{ // like the client: Expect: 100-continue, so a refusal comes before the body is sent
  const body=Buffer.alloc(Math.round(mb*1048576),97); const sock=net.connect(PORT,'127.0.0.1'); let buf='',sent=false;
  const done=()=>{const m=/^HTTP\/1\.1 (\d+)/.exec(buf);const k=buf.indexOf('\r\n\r\n');res({status:m?Number(m[1]):0,body:k>0?buf.slice(k+4,k+164):buf.slice(0,160)});sock.destroy()};
  sock.on('data',d=>{buf+=d; if(/^HTTP\/1\.1 100/.test(buf)&&!sent){sent=true;buf='';sock.write(body);return} if(buf.includes('\r\n\r\n')&&!/^HTTP\/1\.1 100/.test(buf)){setTimeout(done,150)}});
  sock.on('error',()=>res({status:0,body:'socket error '+buf.slice(0,80)})); setTimeout(done,20000);
  sock.write(`POST /attach?thread=${T}&key=k${name}&name=${name}.bin HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer ${token}\r\nContent-Type: application/octet-stream\r\nContent-Length: ${body.length}\r\nExpect: 100-continue\r\n\r\n`)});
const onDisk=n=>{try{return execFileSync('find',[HOME+'/attachments','-name',`*${n}.bin*`]).toString().trim().length>0}catch{return false}};
await set('files.max.mb','1');
const over=await up(2,'over'), under=await up(0.5,'under');
row('S02 files.max.mb=1',over.status===413&&under.status===200&&onDisk('under')&&!onDisk('over'),`2 MB upload -> ${over.status} ${over.body}; 0.5 MB -> ${under.status}, stored ${onDisk('under')}, over-limit stored ${onDisk('over')}`);
await set('files.max.mb','3');
const again=await up(2,'again');
row('S02 files.max.mb=3',again.status===200&&onDisk('again'),`same 2 MB upload after raising the limit to 3 -> ${again.status}, stored ${onDisk('again')}`);
// person.tz: the hub's own clock tool, called the way an agent calls it (MCP URL from the stand-in agent's argv)
const bot=(await c.rpc('bots.create',{name:'ada'})).bot; await until(()=>has(e=>e.$==='BotSet'&&e.id===bot)); const BT=events().find(e=>e.$==='BotSet'&&e.id===bot).thread;
await c.rpc('turn.start',{thread:BT,text:'hello zqshort'}); await until(()=>existsSync(OUT+'/fake.log.argv'),8000);
const url=/"url":"(http:\/\/127\.0\.0\.1:\d+\/mcp\/[^"]+)"/.exec(readFileSync(OUT+'/fake.log.argv','utf8'))?.[1];
const tool=async(name,args={})=>{const r=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name,arguments:args}})});const j=await r.json();const t=j?.result?.content?.[0]?.text;try{return JSON.parse(t)}catch{return {raw:t,j}}};
if(!url) row('S11 person.tz',false,'no MCP url in the stand-in agent argv');
else{
  await c.rpc('bots.routine',{bot,routine:'ro1',name:'morning',schedule:'daily 09:30',prompt:'p'}); await sleep(500);
  const zones=['Asia/Tokyo','America/New_York','Europe/Helsinki']; const res=[];
  for(const z of zones){await c.rpc('setting.set',{key:'person.tz',value:z}); await sleep(400); const r=await tool('person_timezone'); const want=new Intl.DateTimeFormat('en-GB',{timeZone:z,hour:'2-digit',hour12:false}).format(new Date());
    if(!r.zone)console.log("RAW",JSON.stringify(r).slice(0,400));const rl=await tool('routine_list'); if(process.env.RAW)console.log('ROUTINES',JSON.stringify(rl).slice(0,500)); const got=String(r.iso||r.localTime||'').match(/T(\d\d):/)?.[1]||String(r.localTime||'').match(/(\d\d):\d\d/)?.[1];
    res.push({z,next:(rl.find?.(x=>x.id==='ro1')||{}).next,zone:r.zone,source:r.source,offset:r.offset,iso:r.iso,want,got,ok:r.zone===z&&r.source==='set'&&got===want.padStart(2,'0')&&String((rl.find?.(x=>x.id==='ro1')||{}).next).includes('T09:30')})}
  row('S11 person.tz',res.every(x=>x.ok),res.map(x=>`${x.z}: zone=${x.zone} source=${x.source} offset=${x.offset} iso=${x.iso}, a daily 09:30 routine next fires ${x.next} (hour ${x.got}, expected ${x.want})`).join(' | '));
}
// telemetry.share: whether the hub opens any non-loopback connection or sends anything, in all three states (setting.set refuses On on purpose, settings/telemetry.rs `reserved`:
// the Settings On button sends telemetry.choice with the token of what the page showed, so the probe does the same)
const conns=()=>{try{return execFileSync('lsof',['-nP','-a','-p',String(pid),'-i'],{stdio:['ignore','pipe','ignore']}).toString().split('\n').filter(l=>/->/.test(l)&&!/->(127\.0\.0\.1|\[::1\]|localhost)/.test(l))}catch{return []}};
const tmState=()=>{const g=k=>{let v='';for(const e of events())if(e.$==='SettingSet'&&e.key===k)v=e.value;return v};return JSON.stringify([g('telemetry.share'),g('telemetry.asked'),g('telemetry.usage.asked'),g('telemetry.usage.generation'),g('telemetry.choice.revision')])};
const share=()=>{let v='';for(const e of events())if(e.$==='SettingSet'&&e.key==='telemetry.share')v=e.value;return v};
const seen={},stored={};
const watch=async n=>{const found=new Set(); for(let i=0;i<20;i++){conns().forEach(l=>found.add(l.trim().split(/\s+/).slice(-2).join(' ')));await sleep(500)} seen[n]=[...found]};
await c.rpc('setting.set',{key:'telemetry.share',value:'off'}); await sleep(400); stored.off=share(); await watch('off');
const refused=await c.rpc('setting.set',{key:'telemetry.share',value:'on'}); await sleep(300); const stillOff=share()==='off';
const on=await c.rpc('telemetry.choice',{value:'on',expected:tmState(),decision:'',popup:false}); await sleep(400); stored.on=share(); await watch('on');
await c.rpc('setting.set',{key:'telemetry.share',value:'off'}); await sleep(300);
const logTxt=readFileSync(OUT+'/hub.log','utf8');
const quiet=Object.values(seen).every(a=>a.length===0)&&!/telemetry.*(send|post|upload)/i.test(logTxt);
row('S14 telemetry.share',quiet&&stored.off==='off'&&stillOff&&stored.on==='on',`states walked: Off via setting.set (stored ${stored.off}); On via setting.set refused (still ${share()===''?'unset':'off'}: ${stillOff}); On via telemetry.choice with the page's token (ok=${on.ok}, stored ${stored.on}); non-loopback connections of the hub over 10 s each: off ${seen.off.length}, on ${seen.on.length} (${Object.values(seen).flat().join('; ')||'none'}). This build has no sender or endpoint (settings/telemetry.rs may_send is false), so On and Off differ only in the stored choice`);
// agents page (S03): agent.provider and agent.model are what a new thread starts on when the request names neither
{ const made=async(p)=>{const r=await c.rpc('thread.create',{project:P,title:'default-'+p,env:'local',provider:null,model:null}); await sleep(300); return events().find(e=>e.$==='ThreadCreated'&&e.id===r.thread)};
  const before=await made('before'); await set('agent.provider','codex'); await set('agent.model','gpt-5.6-sol'); const after=await made('after');
  await set('agent.provider','claude'); await set('agent.model','opus'); const back=await made('back');
  row('S03 agent.provider, agent.model',before?.provider==='claude'&&after?.provider==='codex'&&after?.model==='gpt-5.6-sol'&&back?.provider==='claude'&&back?.model==='opus',`new thread by default: ${before?.provider}/${before?.model||''}; after Set codex gpt-5.6-sol: ${after?.provider}/${after?.model}; after Set claude opus: ${back?.provider}/${back?.model}`)}
// network page (S08): update.channel is the channel the hub's update service reports and checks
{ const chan=async()=>{const r=await c.rpc('update.status',{});return r?.result?.channel??r?.channel};
  await set('update.channel','beta'); const b=await chan(); await set('update.channel','stable'); const st=await chan();
  row('S08 update.channel',b==='beta'&&st==='stable',`update.status reports ${b} after Beta and ${st} after Stable`)}
// ordering page (S06): order.enabled changes what the next agent is told about ordering (its system prompt, in the stand-in agent's argv)
{ const argv=()=>readFileSync(OUT+'/fake.log.argv','utf8');
  const told=async(state)=>{ await set('order.enabled',state); const n=argv().length; const th=(await c.rpc('thread.create',{project:P,title:'order-'+state,env:'local'})).thread;
    await c.rpc('turn.start',{thread:th,text:'hello zqshort'}); await until(()=>argv().length>n+200,10000); await sleep(300); return argv().slice(n) };
  const on=await told('on'), off=await told('off'); const phrase='Ordering from Backplane is off';
  row('S06 order.enabled',!on.includes(phrase)&&off.includes(phrase),`next turn's prompt says "${phrase}": with Ordering On ${on.includes(phrase)}, with Off ${off.includes(phrase)}`) }
c.ws.close(); try{process.kill(pid,'SIGTERM')}catch{}
writeFileSync(OUT+'/effects.json',JSON.stringify(out_rows,null,1));
process.exit(out_rows.every(r=>r.ok)?0:1);
