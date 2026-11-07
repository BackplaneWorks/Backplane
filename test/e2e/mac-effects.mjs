// Effects of Settings controls observable without a window (hub files, processes, HTTP): files.max.mb, person.tz, telemetry.share. Same rig as mac-hub-rows.mjs.
// usage (on the machine): node test/e2e/mac-effects.mjs HUB_BIN OUT_DIR [PORT]   -> one line per check on stdout, OUT_DIR/effects.json
// A check passes only from events in the hub's own log (events.jsonl) or the HTTP answer; nothing here stands in for the client.
import {spawn,execFileSync} from 'node:child_process';
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
const token=readFileSync(HOME+'/token','utf8').trim();
const out_rows=[]; const row=(id,ok,ev)=>{out_rows.push({id,ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
const pid=await start(600);
let c=await connect(); await sleep(1500);
await c.rpc('project.add',{path:OUT+'/proj'}); await until(()=>has(e=>e.$==='ProjectCreated'));
const P=events().find(e=>e.$==='ProjectCreated').id;
const T=(await c.rpc('thread.create',{project:P,title:'effects',env:'local'})).thread;
const set=async(k,v)=>{await c.rpc('setting.set',{key:k,value:v});await sleep(300)};
// files.max.mb: POST /attach with a body over / under the limit
const up=async(mb,name)=>{const body=Buffer.alloc(Math.round(mb*1048576),97);
  const r=await fetch(`http://127.0.0.1:${PORT}/attach?thread=${T}&key=k${name}&name=${name}.bin`,{method:'POST',headers:{Authorization:'Bearer '+token,'Content-Type':'application/octet-stream'},body});
  return {status:r.status,body:(await r.text()).slice(0,160)}};
await set('files.max.mb','1');
const over=await up(2,'over'), under=await up(0.5,'under');
const onDisk=n=>{try{return execFileSync('find',[HOME+'/attachments','-name',`*${n}.bin*`]).toString().trim().length>0}catch{return false}};
row('S02 files.max.mb=1',over.status===413&&under.status===200&&onDisk('under')&&!onDisk('over'),`2 MB upload -> ${over.status} ${over.body}; 0.5 MB -> ${under.status}, stored ${onDisk('under')}, over-limit stored ${onDisk('over')}`);
await set('files.max.mb','3');
const again=await up(2,'again');
row('S02 files.max.mb=3',again.status===200&&onDisk('again'),`same 2 MB upload after raising the limit to 3 -> ${again.status}, stored ${onDisk('again')}`);
// person.tz: the hub's own clock tool, called the way an agent calls it (MCP URL from the stand-in agent's argv)
await c.rpc('turn.start',{thread:T,text:'hello zqshort'}); await until(()=>existsSync(OUT+'/fake.log.argv'),8000);
const url=/"url":"(http:\/\/127\.0\.0\.1:\d+\/mcp\/[^"]+)"/.exec(readFileSync(OUT+'/fake.log.argv','utf8'))?.[1];
const tool=async(name,args={})=>{const r=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name,arguments:args}})});const j=await r.json();const t=j?.result?.content?.[0]?.text;try{return JSON.parse(t)}catch{return {raw:t,j}}};
if(!url) row('S11 person.tz',false,'no MCP url in the stand-in agent argv');
else{
  const zones=['Asia/Tokyo','America/New_York','Europe/Helsinki']; const res=[];
  for(const z of zones){await set('person.tz',z); const r=await tool('person_timezone'); const want=new Intl.DateTimeFormat('en-GB',{timeZone:z,hour:'2-digit',hour12:false}).format(new Date());
    const got=String(r.iso||r.localTime||'').match(/T(\d\d):/)?.[1]||String(r.localTime||'').match(/(\d\d):\d\d/)?.[1];
    res.push({z,zone:r.zone,source:r.source,offset:r.offset,iso:r.iso,want,got,ok:r.zone===z&&r.source==='set'&&got===want.padStart(2,'0')})}
  row('S11 person.tz',res.every(x=>x.ok),res.map(x=>`${x.z}: zone=${x.zone} source=${x.source} offset=${x.offset} iso=${x.iso} (hour ${x.got}, expected ${x.want})`).join(' | '));
}
// telemetry.share: whether the hub opens any non-loopback connection or sends anything, with it on and off
const conns=()=>{try{return execFileSync('lsof',['-nP','-a','-p',String(pid),'-i'],{stdio:['ignore','pipe','ignore']}).toString().split('\n').filter(l=>/->/.test(l)&&!/->(127\.0\.0\.1|\[::1\]|localhost)/.test(l))}catch{return []}};
const seen={};
for(const v of ['off','on','']){ await set('telemetry.share',v); const found=new Set(); for(let i=0;i<20;i++){conns().forEach(l=>found.add(l.trim().split(/\s+/).slice(-2).join(' ')));await sleep(500)} seen[v||'unset']=[...found]; }
const stored=()=>events().filter(e=>e.$==='SettingSet'&&e.key==='telemetry.share').map(e=>e.value);
const logTxt=readFileSync(OUT+'/hub.log','utf8');
row('S14 telemetry.share',Object.values(seen).every(a=>a.length===0)&&!/telemetry.*(send|post|upload)/i.test(logTxt),`non-loopback connections of the hub over 10 s each: off ${seen.off.length}, on ${seen.on.length}, unset ${seen.unset.length} (${Object.values(seen).flat().join('; ')||'none'}); the build has no sender or endpoint (settings/telemetry.rs may_send is false), so on and off differ only in the stored choice (${stored().join(',')})`);
c.ws.close(); try{process.kill(pid,'SIGTERM')}catch{}
writeFileSync(OUT+'/effects.json',JSON.stringify(out_rows,null,1));
process.exit(out_rows.every(r=>r.ok)?0:1);
