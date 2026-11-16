// What a Mac gets from a macOS Backplane KiCad release before KiCad is in the pane (bp.12.5): node mac-kicad-runtime.mjs HUB RUNTIME PORT WORK
// RUNTIME is the unpacked release folder (bin/kicad-cli, bin/pcbnew, KiCad.app). On a logged-in macOS desktop (the e2e-macos runner):
//   - an agent's kicad_ipc through the hub's own transport and a headless server of this runtime (status, version, open, a commit, a save);
//   - Open in KiCad starts the editor as its own window (the launcher beside the CLI), and while it is open an agent's save through the
//     headless server tells the agent (detached_kicad) and the person (a warning line in the thread) that this window does not show it.
// EXPECT_VERSION (e.g. 10.99.0-bp.12.5) is checked against GetVersion. The KiCad window this test opens is closed at the end.
import {copyFileSync,chmodSync,existsSync,mkdirSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {startHub,stopHub,client,makeStandins,sleep} from '../perf/lib.mjs';
const [bin,rt,port,work]=process.argv.slice(2);const expect=process.env.EXPECT_VERSION||'';process.env.NO_SCOPE='1'; // no systemd-run on macOS
rmSync(work,{recursive:true,force:true});for(const d of ['bin','proj','user/Library/Preferences'])mkdirSync(`${work}/${d}`,{recursive:true});
// the ecc83 board from the repo's fixtures (the macOS package carries no demos), as the embed suites seed it
const fx=new URL('../files/fixtures/plot/',import.meta.url).pathname;
for(const e of ['kicad_pcb','kicad_sch']){const p=`${work}/proj/ecc83-pp.${e}`;copyFileSync(`${fx}ecc83-pp.${e}`,p);chmodSync(p,0o644)}
writeFileSync(`${work}/proj/ecc83-pp.kicad_pro`,'{}\n'); // the fixtures have no project file: a minimal one, as seed_project.py writes
const pcb=`${work}/proj/ecc83-pp.kicad_pcb`;
makeStandins(`${work}/bin`);
writeFileSync(`${work}/bin/claude`,`#!/usr/bin/env python3
import json,os,sys,urllib.request,uuid
a=sys.argv;url=json.loads(a[a.index('--mcp-config')+1])['mcpServers']['backplane']['url'];out=os.environ['KIPC_OUT'];pcb=os.environ['KIPC_PCB']
say=lambda o:(print(json.dumps(o)),sys.stdout.flush())
say({'type':'system','subtype':'init','session_id':'kipc-standin'})
sys.stdin.readline()
n=[0]
def tool(name,args):
    n[0]+=1
    r=urllib.request.Request(url,json.dumps({'jsonrpc':'2.0','id':n[0],'method':'tools/call','params':{'name':name,'arguments':args}}).encode(),{'content-type':'application/json','accept':'application/json'})
    j=json.loads(urllib.request.urlopen(r,timeout=120).read());x=j.get('result',{});t=(x.get('content') or [{}])[0].get('text','')
    try: b=json.loads(t)
    except Exception: b=t
    return {'error':bool(x.get('isError')),'body':b}
C='kiapi.common.commands.'
ipc=lambda ty,m={}:tool('kicad_ipc',{'path':pcb,'type':ty,'message':m})
r={}
r['status']=tool('kicad_ipc_status',{'path':pcb})
r['version']=ipc(C+'GetVersion')
r['open']=ipc(C+'OpenDocument',{'type':'DOCTYPE_PCB','path':pcb})
doc=r['open']['body'].get('message',{}).get('document') if isinstance(r['open']['body'],dict) else None
h={'document':doc}
r['begin']=ipc(C+'BeginCommit',{'header':h})
tid=(r['begin']['body'].get('message',{}) if isinstance(r['begin']['body'],dict) else {}).get('id')
r['create']=ipc(C+'CreateItems',{'header':h,'items':[{'@type':'type.googleapis.com/kiapi.board.types.Track','id':{'value':str(uuid.uuid4())},'start':{'x_nm':100000000,'y_nm':100000000},'end':{'x_nm':110000000,'y_nm':100000000},'width':{'value_nm':250000},'layer':'BL_F_Cu'}]})
r['end']=ipc(C+'EndCommit',{'header':h,'id':tid,'action':'CMA_COMMIT','message':'mac e2e'})
r['save']=ipc(C+'SaveDocument',{'document':doc})
open(out+'.tmp','w').write(json.dumps(r));os.rename(out+'.tmp',out)
say({'type':'result','subtype':'success','is_error':False,'result':'done','session_id':'kipc-standin'})
for _ in sys.stdin: pass
`);chmodSync(`${work}/bin/claude`,0o755);
const u=`${work}/user`,out=`${work}/out.json`;
const wrapper=`${work}/hub.sh`;writeFileSync(wrapper,`#!/bin/sh\nexport HOME='${u}'\nexec '${bin}' "$@"\n`);chmodSync(wrapper,0o755);
const h=await startHub({bin:wrapper,home:`${work}/home`,port:Number(port),standins:`${work}/bin`,log:`${work}/hub.log`,extraEnv:{BACKPLANE_KICAD_CLI:`${rt}/bin/kicad-cli`,KIPC_OUT:out,KIPC_PCB:pcb}});
let bad=0;const check=(n,ok,x)=>{console.log(`${ok?'ok  ':'FAIL'} ${n}${ok?'':' '+JSON.stringify(x)?.slice(0,600)}`);if(!ok)bad++};
const c=await client(h,{});
c.send('project.add',{path:`${work}/proj`});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
c.send('thread.create',{project:pc.id,title:'kicad on a mac',env:'local'});const th=await c.until(10000,()=>c.seen.find(x=>x.$==='ThreadCreated'));
// KiCad's windows, from the window list (no permission needed): owner pid and name of every on-screen window
const wins=()=>{try{return JSON.parse(execFileSync('python3',['-c',"import json,Quartz;print(json.dumps([[w.get('kCGWindowOwnerPID'),w.get('kCGWindowOwnerName',''),w.get('kCGWindowLayer',0)] for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly,Quartz.kCGNullWindowID)]))"],{encoding:'utf8'}))}catch(e){return []}};
const before=new Set(wins().map(w=>w[0]));
// Open in KiCad: the board shown in the thread's viewer, as the viewer bar's button sends it
const o=await c.rpc('kicad.open',{thread:th.id,key:`0|${work}/proj|${pcb}`},30000);
check('Open in KiCad answers',o?.o?.ok===true,o?.o);
let kicadWin=null;for(let i=0;i<120&&!kicadWin;i++){await sleep(500);kicadWin=wins().find(w=>!before.has(w[0])&&/pcbnew|kicad/i.test(w[1])&&w[2]===0)}
check('Open in KiCad shows the board editor in a window of its own',!!kicadWin,wins().slice(0,12));
try{execFileSync('screencapture',['-x',`${work}/open-in-kicad.png`])}catch{}
c.send('turn.start',{thread:th.id,text:'go'});
for(let i=0;i<1800&&!existsSync(out);i++)await sleep(100);
check('the stand-in agent finished',existsSync(out),readFileSync(`${work}/hub.log`,'utf8').slice(-600));
const r=existsSync(out)?JSON.parse(readFileSync(out,'utf8')):{};
const full=r.version?.body?.message?.version?.full_version||'';console.log('GetVersion through the hub:',full);
check('status: the headless server is ready',/ready/.test(JSON.stringify(r.status?.body?.servers??r.status?.body)),r.status);
check(`GetVersion is ${expect||'a Backplane KiCad'}`,expect?full.endsWith(expect.slice(expect.lastIndexOf('-'))):/bp\./.test(full),r.version); // '10.99.0-unknown-bp.6' on older runtimes
check('OpenDocument',!r.open?.error&&!!r.open?.body?.message?.document,r.open);
check('a commit (BeginCommit, CreateItems, EndCommit)',!r.begin?.error&&!r.create?.error&&!r.end?.error,[r.begin,r.create,r.end]);
check('the save writes the board',!r.save?.error,r.save);
const warn=r.save?.body?.detached_kicad||r.end?.body?.detached_kicad||'';
check('the agent is told the KiCad window does not show its change',/own window/.test(warn)&&/replaces the agent's edit/.test(warn),r.save?.body);
const events=readFileSync(`${h.home}/events.jsonl`,'utf8').split('\n').filter(l=>l.includes('"ActivityLogged"')&&l.includes('kicad-detached'));
check('the person is told in the thread',events.some(l=>/own window/.test(l)&&/"warn"/.test(l)),events.slice(-2));
const tracks=readFileSync(pcb,'utf8').match(/\(segment/g)?.length||0;check('the saved board has the agent\'s track on disk',tracks>0,tracks);
if(kicadWin){try{process.kill(kicadWin[0],'SIGTERM')}catch{}}
stopHub(h);
const running=()=>{try{return execFileSync('pgrep',['-f',`${rt}/`],{encoding:'utf8'}).trim()}catch{return ''}};
let left=running();for(let i=0;i<40&&left;i++){await sleep(250);left=running()} // --parent-pid is polled: give it 10 s
check('nothing of this runtime is left running (the headless server ended with the hub)',!left,left);
console.log(bad?`${bad} FAILED`:'all passed');process.exit(bad?1:0);
