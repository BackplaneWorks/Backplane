// Isolated real hub/effect/RPC/stand-in provider qualification. Never GitHub.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, chmodSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ReportWriter } from '../../src/web/report-writer.js';
const [binary='build/reporting-hub',wire='build/reporting-wire']=process.argv.slice(2);
for(const f of readdirSync(wire).filter(x=>x.endsWith('.js'))) (0,eval)(readFileSync(join(wire,f),'utf8'));
const W=(globalThis as any).Wire;
const root=mkdtempSync(join(tmpdir(),'bp-reporting-e2e-')),home=join(root,'home'),user=join(root,'user'),fake=join(root,'bin');
mkdirSync(fake);mkdirSync(user);
const expected='# Title\n\nReconnect state stays red.\n\n## Evidence\nThe user restarted a test hub and saw a red dot after the conversation loaded.\n\nOS: Linux\nApp version: 1.2.3\n';
writeFileSync(join(fake,'codex'),'#!/bin/sh\nif [ "${1:-}" = "--version" ]; then echo "codex-cli 1.0.0"; exit; fi\nprintf \'%s\\n\' '+[JSON.stringify({type:'thread.started',thread_id:'fixture-session'}),JSON.stringify({type:'item.completed',item:{id:'answer',type:'agent_message',text:expected}}),JSON.stringify({type:'turn.completed',usage:{input_tokens:1,output_tokens:1}})].map(x=>"'"+x.replace(/'/g,"'\\''")+"'").join(' ')+'\n');chmodSync(join(fake,'codex'),0o755);
for(const name of ['claude','grok','muse','opencode']){writeFileSync(join(fake,name),'#!/bin/sh\nexit 127\n');chmodSync(join(fake,name),0o755);}
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}),port=listener.port;listener.stop(true);
const proc=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--no-tailscale'],{env:{...process.env,HOME:user,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_PEERS:'',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9'},stdout:'ignore',stderr:'pipe'});
const origin='http://127.0.0.1:'+port;let ws:WebSocket|undefined;
const nap=(ms:number)=>new Promise(r=>setTimeout(r,ms));
try{
 let started=false;for(let n=0;n<150;n++){try{if((await fetch(origin+'/hello')).ok){started=true;break;}}catch{}await nap(100);}assert(started,'test hub starts');
 const get=async()=>{const r=await fetch(origin+'/debug/diagnostics.json');assert.equal(r.headers.get('cache-control'),'no-store');assert(r.ok);return r.json() as Promise<any>;};
 const first=await get();assert.equal(first.status.cap_bytes,1048576);assert.equal(first.status.enabled,true);assert(first.status.allocated_bytes<=first.status.cap_bytes);
 ws=new WebSocket(origin.replace('http:','ws:')+'/ws');ws.binaryType='arraybuffer';
 const pending=new Map<number,{resolve:(v:any)=>void,reject:(e:Error)=>void,timer:ReturnType<typeof setTimeout>}>();let next=0;const changes:any[]=[];
 ws.onmessage=e=>{const value=JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));for(const c of value.items||[])changes.push(c);if(value.t==='reply'){const p=pending.get(Number(value.id));if(p){pending.delete(Number(value.id));clearTimeout(p.timer);value.ok===false?p.reject(Error(value.error)):p.resolve(value.result??value);}}};
 await new Promise<void>((r,j)=>{ws!.onopen=()=>r();ws!.onerror=()=>j(Error('test websocket failed'));});
 const rpc={request(m:string,p:any){const id=++next;return new Promise<any>((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(Error('RPC timeout '+m));},15000);pending.set(id,{resolve,reject,timer});ws!.send(W.encode(JSON.stringify({id,m,p})));});}};
 mkdirSync(join(user,'reports'),{recursive:true});
 const createdProject=await rpc.request('project.add',{path:join(user,'reports')});
 const projectState=await rpc.request('improvements.context',{});
 const reportProject=projectState.projects.find((p:any)=>p.root===join(user,'reports'))?.id;assert(reportProject);
 await rpc.request('setting.set',{key:'agent.provider',value:'codex'});
 await rpc.request('setting.set',{key:'agent.model',value:'gpt-6.1-sol'});
 const draft:any={text:'Fixture behavior only',writerProject:reportProject},writer=new ReportWriter({rpc,save:async()=>{},upload:async()=>{throw Error('No screenshot elected');}});
 assert.equal(await writer.start(draft,'Write only the supplied report. No external issue. No source changes.'),true);
 let report:any;for(let i=0;i<120;i++){report=await rpc.request('improvements.report',{thread:draft.writer.thread});if(report.done)break;await nap(100);}
 assert.equal(report.done,true);assert.equal(report.body,expected);assert.equal(await writer.load(draft),expected);
 const create=await rpc.request('thread.create',{project:reportProject,title:'ignored retry',creationKey:draft.writer.key});assert.equal(create.thread,draft.writer.thread);
 const status=await rpc.request('diagnostics.control',{action:'off'});assert.equal(status.enabled,false);assert.equal(status.ok,true);
 const off=await get();assert.equal(off.status.enabled,false);
 const clear=await rpc.request('diagnostics.control',{action:'clear'});assert.equal(clear.ok,true);assert.deepEqual((await get()).events,[]);
 await rpc.request('diagnostics.control',{action:'on'});await rpc.request('improvements.context',{sourceThread:draft.writer.thread});
 const exported=await get();assert(exported.events.some((e:any)=>e.name==='hub.rpc'));assert(!JSON.stringify(exported).includes('Fixture behavior'));assert(!JSON.stringify(exported).includes(root));assert(!JSON.stringify(exported).includes('fixture-session'));
 let bytes=0,allocated=0;for(const f of readdirSync(join(home,'diagnostics'))){const s=statSync(join(home,'diagnostics',f));bytes+=s.size;allocated+=s.blocks*512;}assert(bytes<=1048576&&allocated<=1048576);
 assert.equal(changes.filter(x=>x.$==='ThreadCreated'&&x.id===draft.writer.thread).length,1);
 console.log('ok real hub startup, safe export and measured disk cap; normal selected-project report agent result; keyed retry; durable controls; no private text in diagnostics');
}finally{ws?.close();proc.kill();await proc.exited;const errors=await new Response(proc.stderr).text();if(proc.exitCode&&process.env.REPORTING_DEBUG)console.error(errors.slice(-10000));rmSync(root,{recursive:true,force:true});}
