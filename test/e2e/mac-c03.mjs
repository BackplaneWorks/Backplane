// C03 on a machine with FreeCAD and no window: Settings > CAD's FreeCAD path (field fpath, button freecad-path, setting freecad.path) is the one the hub launches.
// usage: node test/e2e/mac-c03.mjs HUB_BIN OUT_DIR [PORT] [REAL_FREECADCMD]   -> one line per check, OUT_DIR/c03.json
// A wrapper stands in for freecadcmd: it logs each call, then runs the real one, so the log shows which program the hub started to mesh a STEP (BACKPLANE_STEP2GLB=off: FreeCAD is the only mesher).
import {mkdirSync,writeFileSync,readFileSync,existsSync,rmSync,chmodSync,copyFileSync,readdirSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {viewClient} from '../perf/view.mjs';
import {encode,decode} from '../wire.mjs';
const [bin,out,port0,realArg]=process.argv.slice(2);const port=Number(port0||3840);process.env.NO_SCOPE='1';
const real=realArg||process.env.HOME+'/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd';
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/bin',{recursive:true});mkdirSync(out+'/fc',{recursive:true});mkdirSync(out+'/proj',{recursive:true});makeStandins(out+'/bin');
const calls=out+'/fc/calls.log';const wrap=out+'/fc/freecadcmd';
writeFileSync(wrap,`#!/bin/sh\necho "$@" >> ${calls}\nexec ${real} "$@"\n`);chmodSync(wrap,0o755);
copyFileSync(new URL('../files/fixtures/mesh/bracket.step',import.meta.url).pathname,out+'/proj/bracket.step');
const home=out+'/home';const rows=[];const rec=(id,ok,ev)=>{rows.push({id,ok,ev});console.log(`${ok?'ok  ':'FAIL'} ${id} ${ev}`)};
const hub=await startHub({bin,home,port,standins:out+'/bin',log:out+'/hub.log',extraEnv:{BACKPLANE_STEP2GLB:'off',PATH:out+'/bin:/usr/bin:/bin',XDG_CACHE_HOME:out+'/cache'}});
const v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();
const act=async(action,value='')=>{const r=await v.req('view.action',{action,value},30000);for(const c of r?.x?.result?.cmds||[])if(c.type==='send'){try{v.ws.send(encode(JSON.parse(c.text)))}catch{}await sleep(200)}return r};
await act('flag','settings');
const cadRow=async()=>{const r=await act('settings-page','cad');const rows=r?.x?.result?.shown?.settings?.rows||[];return rows.find(x=>/freecad/i.test(x.label||''))||rows.find(x=>/freecad/i.test(JSON.stringify(x)))||{rowsSeen:rows.map(x=>x.label)}};
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';const replies=new Map();
ws.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}if(o.t==='reply')replies.set(o.id,o)};
await new Promise(r=>ws.onopen=r);await sleep(1500);
const rpc=async(m,p,ms=180000)=>{const n=Math.floor(Math.random()*1e9);ws.send(encode({id:n,m,p}));for(let k=0;k<ms/100&&!replies.has(n);k++)await sleep(100);return replies.get(n)||{ok:false,error:'no reply'}};
const pr=await rpc('project.add',{path:out+'/proj'});const T=pr.thread;await sleep(1500);
const nCalls=()=>existsSync(calls)?readFileSync(calls,'utf8').split('\n').filter(Boolean).length:0;
const renders=()=>{try{return readdirSync(out+'/proj/renders').filter(f=>f.startsWith('bracket'))}catch{return []}};
let seq=0;const render=async()=>{rmSync(out+'/proj/renders',{recursive:true,force:true});const before=nCalls();const nm=`bracket${++seq}.step`;writeFileSync(out+'/proj/'+nm,readFileSync(out+'/proj/bracket.step','utf8')+`\n/* ${seq} ${Date.now()} */\n`);const m=await rpc('mech.render',{thread:T,path:nm});await sleep(500);return {ok:m.ok!==false&&renders().length>0,files:renders().length,calls:nCalls()-before,raw:JSON.stringify(m).slice(0,120)}};
const json=o=>JSON.stringify(o||{});
// 0. no setting: the hub finds FreeCAD by its own search (the app bundle), never the wrapper
let row=await cadRow();const r0=await render();
rec('C03 default',r0.ok&&r0.calls===0&&!/not found/i.test(json(row)),`no freecad.path: row "${json(row).slice(0,160)}"; mech.render wrote ${r0.files} files, wrapper calls ${r0.calls}`);
// 1. Settings > CAD: type the path in the field, press Set
await act('bfield','fpath\u001f'+wrap);await act('freecad-path');await sleep(1200);
const stored=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(l=>l.includes('"freecad.path"')).map(l=>JSON.parse(l).value).pop();
row=await cadRow();const r1=await render();
rec('C03 path set',stored===wrap&&json(row).includes(wrap)&&r1.ok&&r1.calls>0,`stored ${stored}; row note shows the path: ${json(row).includes(wrap)} ("${json(row).slice(0,200)}"); mech.render wrote ${r1.files} files and the hub started the wrapper ${r1.calls} time(s)`);
// 2. a path that does not run: the page says so and the hub falls back to its search
await act('bfield','fpath\u001f/nonexistent/freecadcmd');await act('freecad-path');await sleep(1200);
row=await cadRow();const r2=await render();
rec('C03 bad path',(row?.note||'').includes('/FreeCAD.app/')&&!json(row?.note).includes('nonexistent')&&r2.ok&&r2.calls===0,`the setting does not run, so the note shows the program the search found instead ("not found at that path" is for when nothing is found): "${row?.note}"; mech.render still wrote ${r2.files} files via the normal search, wrapper calls ${r2.calls}`);
// 3. persisted across a restart
await act('bfield','fpath\u001f'+wrap);await act('freecad-path');await sleep(1000);
try{v.close()}catch{};ws.close();stopHub(hub);await sleep(1500);
const hub2=await startHub({bin,home,port,standins:out+'/bin',log:out+'/hub.log',extraEnv:{BACKPLANE_STEP2GLB:'off',PATH:out+'/bin:/usr/bin:/bin',XDG_CACHE_HOME:out+'/cache'}});
const t2=readFileSync(home+'/token','utf8').trim();const ws2=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${t2}`);ws2.binaryType='arraybuffer';const rep2=new Map();
ws2.onmessage=e=>{let o;try{o=typeof e.data==='string'?JSON.parse(e.data):decode(e.data)}catch{return}if(o.t==='reply')rep2.set(o.id,o)};await new Promise(r=>ws2.onopen=r);await sleep(2500);
rmSync(out+'/proj/renders',{recursive:true,force:true});const b4=nCalls();const n=1;writeFileSync(out+'/proj/bracket9.step',readFileSync(out+'/proj/bracket.step','utf8')+`\n/* 9 ${Date.now()} */\n`);ws2.send(encode({id:n,m:'mech.render',p:{thread:T,path:'bracket9.step'}}));for(let k=0;k<1800&&!rep2.has(n);k++)await sleep(100);await sleep(500);
rec('C03 after restart',nCalls()>b4&&renders().length>0,`after a hub restart mech.render started the wrapper ${nCalls()-b4} time(s), ${renders().length} files`);
ws2.close();stopHub(hub2);writeFileSync(out+'/c03.json',JSON.stringify(rows,null,1));process.exit(rows.every(r=>r.ok)?0:1);
