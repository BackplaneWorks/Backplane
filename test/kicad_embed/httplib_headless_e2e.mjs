// Issue #1192: a symbol from a KiCad HTTP library placed through the headless KiCad (kicad-cli api-server), driven as an agent drives it.
//   bun httplib_headless_e2e.mjs HUB RUNTIME PORT WORK [--expect-hollow]
// The HTTP part is placed first, on a server that has loaded no library yet (as an agent's first call does).
// A REST stub (lib/httplib_stub.py) serves one part whose symbolIdStr is Device:R; the global symbol table (a temp KICAD_CONFIG_HOME) holds
// that HTTP library (My_Parts) and Device. A stand-in `claude` calls kicad_edit inside a turn on a temp hub (no editor open, so the
// headless KiCad serves every call) and the answers are checked:
//   whole     the HTTP part placed by kicad_edit add has its pins in `created`, and its raw definition has pins and graphics
//   control   Device:R placed the same way has them (the runtime and the hub are fine without the HTTP library)
//   served    the answer says the headless KiCad served it (served_by), on a refusal too
//   relib     set raw definition.id is refused (a placed symbol's library symbol is not changed by set), not answered as done
//   rawadd    a raw SchematicSymbolInstance add with only definition.id has the part's other fields (MPN); without the fork fix it is refused as empty
//   adddel    add then delete by ref in one call deletes it
// With --expect-hollow (a runtime without the fork fix) `whole` must fail and the hub must say so (flag `hollow`), the rest as above.
// HOME and XDG_* are temp. Only processes this script started are stopped (the hub by its PID, the stub by its PID).
import {spawn} from 'node:child_process';
import {chmodSync,existsSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync,copyFileSync} from 'node:fs';
import {startHub,stopHub,client,makeStandins,sleep} from '../perf/lib.mjs';
const [bin,rt,port,work]=process.argv.slice(2);const expectHollow=process.argv.includes('--expect-hollow');
const here=new URL('.',import.meta.url).pathname;
rmSync(work,{recursive:true,force:true});for(const d of ['bin','proj','user/.config/kicad','user/.cache','user/.local/share','user/.local/state'])mkdirSync(`${work}/${d}`,{recursive:true});
const run=mkdtempSync('/tmp/kh-'); // a socket path has 107 bytes; the temp home's is longer
// the REST stub
const pf=`${work}/stub.port`;const stub=spawn('python3',['-I',`${here}/lib/httplib_stub.py`,pf],{stdio:'ignore'});
for(let i=0;i<100&&!existsSync(pf);i++)await sleep(50);const sport=readFileSync(pf,'utf8').trim();
// the HTTP library and the global symbol table
const lib=`${work}/My_Parts.kicad_httplib`;
writeFileSync(lib,JSON.stringify({meta:{version:1.0},name:'My_Parts',description:'test parts',source:{type:'REST_API',api_version:'v1',root_url:`http://127.0.0.1:${sport}`,token:'test',timeout_parts_seconds:60,timeout_categories_seconds:600}},null,1));
const full=process.argv.includes('--full-table'); // the runtime's whole default table (every KiCad library) plus My_Parts, as a person's is
const table=full?readFileSync(`${rt}/share/kicad/template/sym-lib-table`,'utf8').replaceAll('${KICAD10_SYMBOL_DIR}',`${rt}/share/kicad/symbols`).replace(/\)\s*$/,`  (lib (name "My_Parts")(type "HTTP")(uri "${lib}")(options "")(descr ""))\n)\n`):`(sym_lib_table\n  (version 7)\n  (lib (name "Device")(type "KiCad")(uri "${rt}/share/kicad/symbols/Device.kicad_symdir")(options "")(descr ""))\n  (lib (name "power")(type "KiCad")(uri "${rt}/share/kicad/symbols/power.kicad_symdir")(options "")(descr ""))\n  (lib (name "My_Parts")(type "HTTP")(uri "${lib}")(options "")(descr ""))\n)\n`;
const kch=`${work}/user/.config/kicad`;for(const v of ['10.0','10.99','11.0'])mkdirSync(`${kch}/${v}`,{recursive:true}),writeFileSync(`${kch}/${v}/sym-lib-table`,table),writeFileSync(`${kch}/${v}/fp-lib-table`,'(fp_lib_table\n  (version 7)\n)\n');
// the project: an empty schematic
const sch=`${work}/proj/t.kicad_sch`;
writeFileSync(sch,`(kicad_sch (version 20250114) (generator "eeschema") (generator_version "9.0") (uuid "6a3c0b1e-0000-4000-8000-000000001192") (paper "A4") (lib_symbols) (sheet_instances (path "/" (page "1"))))\n`);
writeFileSync(`${work}/proj/t.kicad_pro`,JSON.stringify({meta:{filename:'t.kicad_pro',version:3}}));
makeStandins(`${work}/bin`);
const P='My_Parts:470R 1206 1% 250mW 200V';
writeFileSync(`${work}/bin/claude`,`#!/usr/bin/env python3
import json,os,sys,urllib.request
a=sys.argv;url=json.loads(a[a.index('--mcp-config')+1])['mcpServers']['backplane']['url'];out=os.environ['KH_OUT'];sch=os.environ['KH_SCH'];P=${JSON.stringify(P)}
say=lambda o:(print(json.dumps(o)),sys.stdout.flush())
say({'type':'system','subtype':'init','session_id':'kh-standin'})
sys.stdin.readline()
n=[0]
def tool(name,args):
    n[0]+=1
    r=urllib.request.Request(url,json.dumps({'jsonrpc':'2.0','id':n[0],'method':'tools/call','params':{'name':name,'arguments':args}}).encode(),{'content-type':'application/json','accept':'application/json'})
    j=json.loads(urllib.request.urlopen(r,timeout=240).read());x=j.get('result',{});t=(x.get('content') or [{}])[0].get('text','')
    try: b=json.loads(t)
    except Exception: b=t
    return {'error':bool(x.get('isError')),'body':b}
ed=lambda ops:tool('kicad_edit',{'path':sch,'ops':ops})
r={}
r['whole']=ed([{'add':[{'symbol':{'lib':P,'ref':'R998','at':[30,100]}}]},{'find':{'kind':'symbol','ref':'R998','raw':True}}])
r['control']=ed([{'add':[{'symbol':{'lib':'Device:R','ref':'R997','at':[60,100]}}]},{'find':{'kind':'symbol','ref':'R997','raw':True}}])
r['again']=ed([{'add':[{'symbol':{'lib':P,'ref':'R994','at':[150,100]}}]},{'find':{'kind':'symbol','ref':'R994','raw':True}}])
r['relib']=ed([{'set':['R997'],'raw':{'definition':{'id':{'library_nickname':'My_Parts','entry_name':'470R 1206 1% 250mW 200V'}}}},{'find':{'kind':'symbol','ref':'R997'}}])
r['rawadd']=ed([{'add':[{'@type':'type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance','definition':{'id':{'library_nickname':'My_Parts','entry_name':'470R 1206 1% 250mW 200V'}},'position':{'x_nm':'90000000','y_nm':'100000000'},'reference_field':{'name':'Reference','text':{'text':'R996'}}}]},{'find':{'kind':'symbol','ref':'R996','fields':True}}])
r['adddel']=ed([{'add':[{'symbol':{'lib':'Device:R','ref':'R995','at':[120,100]}}]},{'delete':{'kind':'symbol','ref':'R995'}}])
r['after']=ed([{'find':{'kind':'symbol','fields':True}}])
open(out+'.tmp','w').write(json.dumps(r));os.rename(out+'.tmp',out)
say({'type':'result','subtype':'success','is_error':False,'result':'done','session_id':'kh-standin'})
for _ in sys.stdin: pass
`);chmodSync(`${work}/bin/claude`,0o755);
const u=`${work}/user`,out=`${work}/out.json`;
const wrapper=`${work}/hub.sh`;writeFileSync(wrapper,`#!/bin/sh\nexport HOME='${u}' XDG_CONFIG_HOME='${u}/.config' XDG_CACHE_HOME='${u}/.cache' XDG_DATA_HOME='${u}/.local/share' XDG_STATE_HOME='${u}/.local/state' KICAD_CONFIG_HOME='${kch}' XDG_RUNTIME_DIR='${run}' WXTRACE=KICAD_HTTP_LIB\nexec '${bin}' "$@"\n`);chmodSync(wrapper,0o755);
const h=await startHub({bin:wrapper,home:`${work}/home`,port:Number(port),standins:`${work}/bin`,log:`${work}/hub.log`,extraEnv:{BACKPLANE_KICAD_CLI:`${rt}/bin/kicad-cli`,BACKPLANE_KIPC_BIN:'',KH_OUT:out,KH_SCH:sch}});
let bad=0;const check=(n,ok,x)=>{console.log(`${ok?'ok  ':'FAIL'} ${n}${ok?'':' '+JSON.stringify(x)?.slice(0,900)}`);if(!ok)bad++};
try{
const c=await client(h,{});
c.send('project.add',{path:`${work}/proj`});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
c.send('thread.create',{project:pc.id,title:'httplib',env:'local'});const th=await c.until(10000,()=>c.seen.find(x=>x.$==='ThreadCreated'&&x.title==='httplib'));
await c.until(10000,()=>c.seen.find(x=>x.$==='ProviderSet'&&x.id===th.id));const ts=await c.rpc('turn.start',{thread:th.id,text:'go',msg:'kh1'});console.log('turn.start',JSON.stringify(ts?.o).slice(0,400));
for(let i=0;i<3000&&!existsSync(out);i++)await sleep(100);
check('the stand-in finished',existsSync(out),readFileSync(`${work}/hub.log`,'utf8').slice(-600));
const r=JSON.parse(readFileSync(out,'utf8'));writeFileSync(`${work}/answers.json`,JSON.stringify(r,null,1));
const res=k=>r[k]?.body?.results??[];const raw=k=>res(k)[1]?.rows?.[0];
const items=d=>(d?.definition?.items??[]).map(i=>(i.item?.['@type']??i['@type']??'').split('.').pop());
const whole=x=>items(x).includes('SchematicPin')&&items(x).some(t=>/Graphic|Shape/.test(t));
check('control: Device:R comes whole',!r.control.error&&whole(raw('control'))&&!!res('control')[0]?.created?.[0]?.pins,r.control);
check('served: the answer says the headless KiCad served it',r.whole.body?.served_by==='headless'&&r.control.body?.served_by==='headless',[r.whole.body?.served_by,r.control.body?.served_by]);
if(expectHollow){
 check('whole (without the fork fix): the empty template is refused, nothing placed',r.whole.error&&/template KiCad placed is empty/.test(r.whole.body?.error)&&/source symbol itself/.test(r.whole.body?.error),r.whole);
}else check('whole: the HTTP part comes with its pins and graphics',!r.whole.error&&whole(raw('whole'))&&!!res('whole')[0]?.created?.[0]?.pins,r.whole);
console.log(`info again: the HTTP part placed after Device:R comes ${whole(raw('again'))?'whole':'hollow'}`);
check('relib: set raw definition is refused, not answered as done',r.relib.error&&/not changed by set/.test(r.relib.body?.error),r.relib);
const ra=res('rawadd')[1]?.rows?.[0];
if(expectHollow)check('rawadd (without the fork fix): refused as empty',r.rawadd.error&&/is empty/.test(r.rawadd.body?.error),r.rawadd);
else check('rawadd: a raw add by definition.id carries the part\'s fields',!r.rawadd.error&&JSON.stringify(ra??{}).includes('RC1206FR-07470RL'),r.rawadd);
check('adddel: add then delete by ref in one call',!r.adddel.error&&res('adddel')[1]?.deleted===1,r.adddel);
const left=(r.after.body?.results?.[0]?.rows??[]).map(x=>x.ref);check('after: R995 is gone',!left.includes('R995'),left);
}finally{await stopHub(h);try{process.kill(stub.pid)}catch{}rmSync(run,{recursive:true,force:true})}
console.log(bad?`FAILED ${bad}`:'PASS');process.exit(bad?1:0);
