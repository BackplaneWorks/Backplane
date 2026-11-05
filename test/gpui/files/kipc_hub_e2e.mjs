// The Rust hub, its own KiCad transport, a real headless KiCad, and the MCP tools an agent gets: bun kipc_hub_e2e.mjs HUB RUNTIME PORT WORK
// A stand-in `claude` (Python, no model) calls kicad_ipc_status and kicad_ipc from inside a turn. Checks: the answers; no Bun or JS
// anywhere in the hub's process tree; the server stops when the hub stops (--parent-pid), with no signal from the hub. Stopping this
// test's own hub is SIGTERM to the PID it captured, the harness's stopHub; the transport itself sends none. HOME and XDG_* are temp.
import {copyFileSync,chmodSync,existsSync,mkdirSync,mkdtempSync,readFileSync,readlinkSync,rmSync,writeFileSync} from 'node:fs';
import {startHub,stopHub,client,makeStandins,tree,sleep} from '../perf/lib.mjs';
const [bin,rt,port,work]=process.argv.slice(2);
rmSync(work,{recursive:true,force:true});for(const d of ['bin','proj','user/.config/kicad','user/.cache','user/.local/share','user/.local/state'])mkdirSync(`${work}/${d}`,{recursive:true});
const run=mkdtempSync('/tmp/kr-'); // a socket path has 107 bytes; the temp home's is longer
for(const e of ['kicad_pro','kicad_pcb','kicad_sch']){const p=`${work}/proj/ecc83-pp.${e}`;copyFileSync(`${rt}/share/kicad/demos/ecc83/ecc83-pp.${e}`,p);chmodSync(p,0o644)}
makeStandins(`${work}/bin`);
writeFileSync(`${work}/bin/claude`,`#!/usr/bin/env python3
import json,os,sys,urllib.request
a=sys.argv;url=json.loads(a[a.index('--mcp-config')+1])['mcpServers']['backplane']['url'];out=os.environ['KIPC_OUT'];pcb=os.environ['KIPC_PCB']
say=lambda o:(print(json.dumps(o)),sys.stdout.flush())
say({'type':'system','subtype':'init','session_id':'kipc-standin'})
sys.stdin.readline()
n=[0]
def tool(name,args):
    n[0]+=1
    r=urllib.request.Request(url,json.dumps({'jsonrpc':'2.0','id':n[0],'method':'tools/call','params':{'name':name,'arguments':args}}).encode(),{'content-type':'application/json','accept':'application/json'})
    j=json.loads(urllib.request.urlopen(r,timeout=90).read());x=j.get('result',{});t=(x.get('content') or [{}])[0].get('text','')
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
r['tracks']=ipc(C+'GetItems',{'header':{'document':doc},'types':['KOT_PCB_TRACE']})
r['refused']=ipc(C+'GetSelection',{'header':{'document':doc}})
r['badshape']=ipc(C+'GetItems',{'header':5})
open(out+'.tmp','w').write(json.dumps(r));os.rename(out+'.tmp',out)
say({'type':'result','subtype':'success','is_error':False,'result':'done','session_id':'kipc-standin'})
for _ in sys.stdin: pass
`);chmodSync(`${work}/bin/claude`,0o755);
const u=`${work}/user`,out=`${work}/out.json`;
// systemd-run needs the real XDG_RUNTIME_DIR, so the temp one is set inside the scope. With no user bus there, KiCad stays in this hub's capped scope.
const wrapper=`${work}/hub.sh`;writeFileSync(wrapper,`#!/bin/sh\nexport HOME='${u}' XDG_CONFIG_HOME='${u}/.config' XDG_CACHE_HOME='${u}/.cache' XDG_DATA_HOME='${u}/.local/share' XDG_STATE_HOME='${u}/.local/state' KICAD_CONFIG_HOME='${u}/.config/kicad' XDG_RUNTIME_DIR='${run}'\nexec '${bin}' "$@"\n`);chmodSync(wrapper,0o755);
const h=await startHub({bin:wrapper,home:`${work}/home`,port:Number(port),standins:`${work}/bin`,log:`${work}/hub.log`,extraEnv:{BACKPLANE_KICAD_CLI:`${rt}/bin/kicad-cli`,BACKPLANE_KIPC_BIN:'',KIPC_OUT:out,KIPC_PCB:`${work}/proj/ecc83-pp.kicad_pcb`}});
let bad=0;const check=(n,ok,x)=>{console.log(`${ok?'ok  ':'FAIL'} ${n}${ok?'':' '+JSON.stringify(x)?.slice(0,600)}`);if(!ok)bad++};
const c=await client(h,{});
c.send('project.add',{path:`${work}/proj`});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
c.send('thread.create',{project:pc.id,title:'kipc',env:'local'});const th=await c.until(10000,()=>c.seen.find(x=>x.$==='ThreadCreated'));
c.send('turn.start',{thread:th.id,text:'go'});
for(let i=0;i<1200&&!existsSync(out);i++)await sleep(100);
check('the stand-in finished',existsSync(out),readFileSync(`${work}/hub.log`,'utf8').slice(-400));
const r=JSON.parse(readFileSync(out,'utf8'));
console.log('raw GetVersion through the hub:',JSON.stringify(r.version.body));
check('status: server ready',/ready/.test(JSON.stringify(r.status.body.servers??r.status.body)),r.status);
check('GetVersion ends in -bp.4',r.version.body?.message?.version?.full_version?.endsWith('-bp.4'),r.version);
check('OpenDocument',!r.open.error&&!!r.open.body?.message?.document,r.open);
check('GetItems: 59 tracks with @type last',r.tracks.body?.message?.items?.length===59&&Object.keys(r.tracks.body.message.items[0]).at(-1)==='@type',String(r.tracks.body?.message?.items?.length));
check('a command headless KiCad refuses is the hub\'s word',r.refused.error&&!/AS_/.test(JSON.stringify(r.refused.body)),r.refused);
check('a bad shape is protobufjs\'s word',r.badshape.error&&/header: object expected/.test(JSON.stringify(r.badshape.body)),r.badshape);
const exeOf=p=>{try{return readlinkSync(`/proc/${p}/exe`)}catch{return ''}};
const cmd=p=>{try{return readFileSync(`/proc/${p}/cmdline`,'utf8').split('\0').join(' ')}catch{return ''}};
const kids=tree(h.pid).map(p=>({p,exe:exeOf(p),cmd:cmd(p)}));
console.log('hub tree:');for(const k of kids)console.log(`  ${k.p} ${k.exe.split('/').pop()} :: ${k.cmd.slice(-110)}`);
check('no Bun or JS under the hub',!kids.some(k=>/\b(bun|node|deno|kipc)\b/.test(k.exe.split('/').pop()+' '+k.cmd.split(' ')[0])),kids);
const srv=kids.filter(k=>/ api-server /.test(k.cmd));check('one KiCad server under the hub, told the hub\'s pid',srv.length===1&&srv[0].cmd.includes(`--parent-pid ${h.pid}`),kids.map(k=>k.cmd.slice(-160)));
const usage=readFileSync(`${h.home}/events.jsonl`,'utf8').split('\n').filter(l=>l.includes('"TurnUsage"'));check('no real model in the log',!usage.some(l=>/claude-|gpt-/.test(l)),usage.slice(0,2));
stopHub(h);const t0=Date.now();
for(let i=0;i<600&&existsSync(`/proc/${h.pid}`);i++)await sleep(50);const hubMs=Date.now()-t0;check('the hub stopped',!existsSync(`/proc/${h.pid}`));
let gone=false;for(let i=0;i<1200&&!gone;i++){await sleep(50);gone=srv.length>0&&srv.every(k=>!existsSync(`/proc/${k.p}`))}
console.log(`server gone ${Date.now()-t0-hubMs} ms after the hub`);
check('the server ended by itself once the hub went (--parent-pid)',gone,srv.map(k=>k.p));
rmSync(run,{recursive:true,force:true});
console.log(bad?`${bad} FAILED`:'all passed');process.exit(bad?1:0);
