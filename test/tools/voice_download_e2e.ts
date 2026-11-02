// A real isolated hub and a deterministic helper; no model or network download.
// Verifies progress reaches the requesting connection before the helper exits,
// survives split stdout reads, preserves an error followed by status, and retries.
// bun test/tools/voice_download_e2e.ts BINARY WIREDIR
import assert from 'node:assert/strict';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const [binary, wire] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), 'bp-download-e2e-'));
const home = join(root, 'home'), fake = join(root, 'fake');
mkdirSync(home); mkdirSync(fake);
const bin = join(root, 'backplane-serve'); copyFileSync(resolve(binary), bin); chmodSync(bin, 0o755);
writeFileSync(join(root, 'backplane-voice'), `#!${process.execPath}
import {existsSync,writeFileSync,mkdirSync} from 'node:fs';
const a=process.argv.slice(2), cmd=a[0], dir=a[a.indexOf('--local-dir')+1], model=a[a.indexOf('--model')+1] || 'parakeet-v2';
const emit=o=>process.stdout.write(JSON.stringify(o)+'\\n');
const status=()=>({event:'local',supported:true,platform:'test',runtime:existsSync(dir+'/ready'),vad:existsSync(dir+'/ready'),models:[{id:'parakeet-v2',label:'Parakeet v2',size:482468385,installed:existsSync(dir+'/ready')},{id:'parakeet-v3',label:'Parakeet v3',size:487170055,installed:false}]});
if(cmd==='devices')emit({event:'devices',default:'',devices:[]});
else if(cmd==='local-install'){
 process.stdout.write('{"event":"pro'); await Bun.sleep(50);
 process.stdout.write('gress","what":"Parakeet v2","done":241000000,"total":482000000}\\n');
 await Bun.sleep(500);
 if(model==='parakeet-v3')emit({event:'error',error:'test network offline'});
 else if(existsSync(dir+'/crash'))process.exit(9);
 else {emit({event:'progress',phase:'unpack',what:'Parakeet v2',done:0,total:0});await Bun.sleep(200);mkdirSync(dir,{recursive:true});writeFileSync(dir+'/ready','ok');}
 emit(status());
}else emit(status());
`); chmodSync(join(root, 'backplane-voice'), 0o755);
const s = Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port=s.port;s.stop(true);
const hub=Bun.spawn([bin,'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--foreground'],{
 env:{...process.env,BACKPLANE_AGENTS:fake,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9'},stdout:'ignore',stderr:Bun.file(join(root,'hub.log'))});
async function until(f:()=>any, ms=15000){const end=Date.now()+ms;while(Date.now()<end){const v=f();if(v)return v;await Bun.sleep(25);}throw new Error('timeout: '+readFileSync(join(root,'hub.log'),'utf8').slice(-1500));}
let ws:WebSocket|undefined;
try{
 await until(()=>existsSync(join(home,'token')));
 const token=readFileSync(join(home,'token'),'utf8').trim();
 const seen:any[]=[]; ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';
 ws.onmessage=e=>seen.push(JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))));
 await new Promise<void>((r,j)=>{ws!.onopen=()=>r();ws!.onerror=j;});
 let id=0;
 const send=(p:any)=>{const n=++id;ws!.send(W.encode(JSON.stringify({id:n,m:'bots.voice',p})));return n;};
 const reply=(n:number)=>until(()=>seen.find(o=>o.t==='reply'&&String(o.id)===String(n)));
 const st=await reply(send({op:'status'}));assert.equal(st.local.models[0].size,482468385);
 let start=seen.length;
 const n=send({op:'local-install',id:'parakeet-v2'});
 await reply(n);
 const progress=await until(()=>seen.slice(start).find(o=>o.ev==='local-progress'));
 assert.equal(progress.done,241000000);
 assert.equal(seen.slice(start).some(o=>o.ev==='local-result'),false,'progress must arrive while download is running');
 // A status probe races the install; it must remain an independent reply.
 assert.equal((await reply(send({op:'status'}))).local.models[0].installed,false);
 const done=await until(()=>seen.slice(start).find(o=>o.ev==='local-result'));
 assert.equal(done.error,''); assert.equal(done.local.models[0].installed,true);
 assert.ok(seen.slice(start).some(o=>o.ev==='local-progress'&&o.phase==='unpack'));
 start=seen.length;send({op:'local-install',id:'parakeet-v3'});
 const fail=await until(()=>seen.slice(start).find(o=>o.ev==='local-result'));
 assert.equal(fail.error,'test network offline','status after error cannot erase the failure');
 start=seen.length;send({op:'local-install',id:'parakeet-v2'});
 assert.equal((await until(()=>seen.slice(start).find(o=>o.ev==='local-result'))).error,'','retry finishes');
 writeFileSync(join(home,'voice/local/crash'),'1');start=seen.length;send({op:'local-install',id:'parakeet-v2'});
 assert.match((await until(()=>seen.slice(start).find(o=>o.ev==='local-result'))).error,/stopped before finishing/);
 console.log('PASS: live progress before exit, split lines, status race, unpack, failure followed by status, retry, helper crash');
}finally{ws?.close();hub.kill();await hub.exited;rmSync(root,{recursive:true,force:true});}
