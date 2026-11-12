// A headless schematic save through the hub keeps instance paths where KiCad's editor wants them: bun kipc_sch_toplevel_e2e.mjs HUB RUNTIME PORT WORK
// KiCad 10.99 roots every symbol and sheet instance path at the project's top-level sheet UUID (.kicad_pro schematic.top_level_sheets),
// which differs from the root file's own UUID once the editor has saved the project. Before Backplane KiCad bp.12 the headless server
// (`kicad-cli api-server`, the hub's kserve when no editor is open) kept the file's UUID for a single top-level sheet, so an agent's
// SaveDocument rewrote every path to it and the person's next open showed "An error was found ... automatically fixed".
// A stand-in `claude` (Python, no model) opens the schematic with kicad_ipc and saves it; the check reads the files. HOME and XDG_* are temp.
import {chmodSync,cpSync,existsSync,mkdirSync,mkdtempSync,readFileSync,readdirSync,rmSync,writeFileSync} from 'node:fs';
import {startHub,stopHub,client,makeStandins,sleep} from '../perf/lib.mjs';
const [bin,rt,port,work]=process.argv.slice(2);
if(existsSync(work)){const w=d=>{try{chmodSync(d,0o755)}catch{};for(const e of readdirSync(d,{withFileTypes:true}))if(e.isDirectory())w(`${d}/${e.name}`)};w(work)}rmSync(work,{recursive:true,force:true});for(const d of ['bin','user/.config/kicad','user/.cache','user/.local/share','user/.local/state'])mkdirSync(`${work}/${d}`,{recursive:true});
const run=mkdtempSync('/tmp/ks-');
const proj=`${work}/proj`;cpSync(`${rt}/share/kicad/demos/complex_hierarchy`,proj,{recursive:true});
const writable=d=>{chmodSync(d,0o755);for(const e of readdirSync(d,{withFileTypes:true})){const p=`${d}/${e.name}`;if(e.isDirectory())writable(p);else chmodSync(p,0o644)}};writable(proj);
// The project names its one top-level sheet by a UUID unlike the root file's, and the sheets' paths start there (what the editor saves).
const ROOT='6a7b3c1e-1111-4222-8333-944455556666',sch=`${proj}/complex_hierarchy.kicad_sch`;
const fileUuid=/\(uuid "([^"]+)"\)/.exec(readFileSync(sch,'utf8'))[1];
const schs=()=>readdirSync(proj).filter(f=>f.endsWith('.kicad_sch')).map(f=>`${proj}/${f}`);
for(const f of schs())writeFileSync(f,readFileSync(f,'utf8').split(`(path "/${fileUuid}`).join(`(path "/${ROOT}`));
const pro=JSON.parse(readFileSync(`${proj}/complex_hierarchy.kicad_pro`,'utf8'));
(pro.schematic??={}).top_level_sheets=[{filename:'complex_hierarchy.kicad_sch',name:'complex_hierarchy',uuid:ROOT}];
writeFileSync(`${proj}/complex_hierarchy.kicad_pro`,JSON.stringify(pro,null,2));
const roots=()=>{const n={};for(const f of schs())for(const m of readFileSync(f,'utf8').matchAll(/\(path "\/([0-9a-f-]{36})/g))n[m[1]]=(n[m[1]]??0)+1;return n};
const before=roots();
makeStandins(`${work}/bin`);
writeFileSync(`${work}/bin/claude`,`#!/usr/bin/env python3
import json,os,sys,urllib.request
a=sys.argv;url=json.loads(a[a.index('--mcp-config')+1])['mcpServers']['backplane']['url'];out=os.environ['KIPC_OUT'];sch=os.environ['KIPC_SCH']
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
ipc=lambda ty,m={}:tool('kicad_ipc',{'path':sch,'type':ty,'message':m})
import re
r={}
r['open']=ipc(C+'OpenDocument',{'type':'DOCTYPE_SCHEMATIC','path':sch})
m=re.search(r'lock_acquire (\\S+)',json.dumps(r['open']['body']))
if r['open']['error'] and m:
    r['lock']=tool('lock_acquire',{'name':m.group(1).rstrip(',')})
    r['open']=ipc(C+'OpenDocument',{'type':'DOCTYPE_SCHEMATIC','path':sch})
doc=r['open']['body'].get('message',{}).get('document') if isinstance(r['open']['body'],dict) else None
r['save']=ipc(C+'SaveDocument',{'document':doc})
open(out+'.tmp','w').write(json.dumps(r));os.rename(out+'.tmp',out)
say({'type':'result','subtype':'success','is_error':False,'result':'done','session_id':'kipc-standin'})
for _ in sys.stdin: pass
`);chmodSync(`${work}/bin/claude`,0o755);
const u=`${work}/user`,out=`${work}/out.json`;
const wrapper=`${work}/hub.sh`;writeFileSync(wrapper,`#!/bin/sh\nexport HOME='${u}' XDG_CONFIG_HOME='${u}/.config' XDG_CACHE_HOME='${u}/.cache' XDG_DATA_HOME='${u}/.local/share' XDG_STATE_HOME='${u}/.local/state' KICAD_CONFIG_HOME='${u}/.config/kicad' XDG_RUNTIME_DIR='${run}'\nexec '${bin}' "$@"\n`);chmodSync(wrapper,0o755);
const h=await startHub({bin:wrapper,home:`${work}/home`,port:Number(port),standins:`${work}/bin`,log:`${work}/hub.log`,extraEnv:{BACKPLANE_KICAD_CLI:`${rt}/bin/kicad-cli`,BACKPLANE_KIPC_BIN:'',KIPC_OUT:out,KIPC_SCH:sch}});
let bad=0;const check=(n,ok,x)=>{console.log(`${ok?'ok  ':'FAIL'} ${n}${ok?'':' '+JSON.stringify(x)?.slice(0,600)}`);if(!ok)bad++};
check('the fixture roots every path at the project\'s sheet UUID',Object.keys(before).join()===ROOT,before);
const c=await client(h,{});
c.send('project.add',{path:proj});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
c.send('thread.create',{project:pc.id,title:'kipc-sch',env:'local'});const th=await c.until(10000,()=>c.seen.find(x=>x.$==='ThreadCreated'));
c.send('turn.start',{thread:th.id,text:'go'});
for(let i=0;i<1800&&!existsSync(out);i++)await sleep(100);
check('the stand-in finished',existsSync(out),readFileSync(`${work}/hub.log`,'utf8').slice(-400));
const r=existsSync(out)?JSON.parse(readFileSync(out,'utf8')):{open:{},save:{}};
check('OpenDocument (headless)',!r.open.error&&!!r.open.body?.message?.document,r.open);
check('SaveDocument (headless)',!r.save.error,r.save);
const after=roots();
check('every instance path still starts at the project\'s top-level sheet UUID',Object.keys(after).join()===ROOT&&after[ROOT]===before[ROOT],{before,after,fileUuid});
await stopHub(h);rmSync(run,{recursive:true,force:true});
console.log(bad?`${bad} failure(s)`:'all ok');process.exit(bad?1:0);
