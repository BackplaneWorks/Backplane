// C03, what the hub tells an agent about FreeCAD: a stand-in agent that writes out the prompt, arguments and environment it was started with, then runs the FreeCAD path it was told.
// usage: node test/e2e/mac-c03-agent.mjs HUB_BIN OUT_DIR [PORT] [REAL_FREECADCMD]   -> one line per check, OUT_DIR/c03-agent.json
import {mkdirSync,writeFileSync,readFileSync,existsSync,rmSync,chmodSync,readdirSync,renameSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {encode,decode} from '../wire.mjs';
const [bin,out,port0,realArg]=process.argv.slice(2);const port=Number(port0||3850);process.env.NO_SCOPE='1';
const real=realArg||process.env.HOME+'/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd';
const haveFC=existsSync(real);
rmSync(out,{recursive:true,force:true});for(const d of ['bin','fc','proj','dumps'])mkdirSync(`${out}/${d}`,{recursive:true});makeStandins(out+'/bin');
renameSync(out+'/bin/claude',out+'/bin/claude.real');
writeFileSync(out+'/bin/claude',`#!/bin/sh
D=${out}/dumps; printf '%s\\n' "$@" > $D/$$.args; env | sort > $D/$$.env
p=$(printf '%s\\n' "$@" | grep -o '[^ "\`'"'"'()]*[Ff]ree[Cc][Aa][Dd][Cc]md' | head -1); echo "$p" > $D/$$.fcpath
[ -n "$p" ] && "$p" --version > $D/$$.version 2>&1
exec sh ${out}/bin/claude.real "$@"
`);chmodSync(out+'/bin/claude',0o755);
const wrap=out+'/fc/freecadcmd';writeFileSync(wrap,`#!/bin/sh\necho "$@" >> ${out}/fc/calls.log\nexec ${real} "$@"\n`);chmodSync(wrap,0o755);
const home=out+'/home';const rows=[];const rec=(id,ok,ev)=>{rows.push({id,ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
const hub=await startHub({bin,home,port,standins:out+'/bin',log:out+'/hub.log',extraEnv:{PATH:out+'/bin:/usr/bin:/bin',XDG_CACHE_HOME:out+'/cache'}});
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';const replies=new Map();
ws.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}if(o.t==='reply')replies.set(o.id,o)};
await new Promise(r=>ws.onopen=r);await sleep(1500);
const rpc=async(m,p,ms=30000)=>{const n=Math.floor(Math.random()*1e9);ws.send(encode({id:n,m,p}));for(let k=0;k<ms/100&&!replies.has(n);k++)await sleep(100);return replies.get(n)||{ok:false,error:'no reply'}};
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(Boolean);
await rpc('project.add',{path:out+'/proj'});for(let i=0;i<50&&!events().some(e=>e.$==='ProjectCreated');i++)await sleep(100);
const P=events().find(e=>e.$==='ProjectCreated').id;
const dumps=()=>readdirSync(out+'/dumps').filter(f=>f.endsWith('.fcpath'));
const turn=async name=>{const before=new Set(dumps());const T=(await rpc('thread.create',{project:P,title:name,env:'local'})).thread;await rpc('turn.start',{thread:T,text:'hello zqshort'});
  let f='';for(let i=0;i<150&&!f;i++){await sleep(200);f=dumps().find(x=>!before.has(x))||'';if(f&&!existsSync(out+'/dumps/'+f.replace('.fcpath','.version')))await sleep(1500)}
  if(!f)return null;const b=out+'/dumps/'+f.replace('.fcpath','');const rd=x=>existsSync(b+x)?readFileSync(b+x,'utf8'):'';
  return {told:rd('.fcpath').trim(),args:rd('.args'),env:rd('.env'),version:rd('.version').trim()}};
const app=real;
// A: nothing configured -> the normal search result
let a=await turn('A default');
rec('C03 agent default',!!a&&a.told===app&&/^FreeCAD /.test(a.version.split('\n').pop()||a.version),a?`prompt names ${a.told}; the stand-in ran it: "${a.version.split('\n').pop()}"`:'no agent started');
// B: Settings freecad.path set -> the configured path, and the agent runs that
await rpc('setting.set',{key:'freecad.path',value:wrap});await sleep(1500);
let b=await turn('B set');const calls=existsSync(out+'/fc/calls.log')?readFileSync(out+'/fc/calls.log','utf8'):'';
rec('C03 agent path set',!!b&&b.told===wrap&&/--version/.test(calls)&&/FreeCAD/.test(b.version),b?`prompt names ${b.told}; the stand-in ran it through the wrapper (logged "${calls.trim()}") and got "${b.version.split('\n').pop()}"`:'no agent started');
// C: a path that does not run -> the normal search result, not the broken path
await rpc('setting.set',{key:'freecad.path',value:'/nonexistent/freecadcmd'});await sleep(1500);
let c=await turn('C bad');
rec('C03 agent bad path',!!c&&c.told===app&&!c.args.includes('nonexistent')&&/FreeCAD/.test(c.version),c?`prompt names ${c.told} (the broken path appears in it: ${c.args.includes('nonexistent')}); the stand-in ran it: "${c.version.split('\n').pop()}"`:'no agent started');
ws.close();stopHub(hub);writeFileSync(out+'/c03-agent.json',JSON.stringify(rows,null,1));process.exit(rows.every(r=>r.ok)?0:1);
