// Narrow completion of180488's resume assertion. Reuses its native binary
// and retained synthetic log; no native build or1024-append workload.
// bun test/tools/mail_catchup_resume.ts BINARY WIREDIR PRIOR_JSON OUT
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { standinScript } from "./perf_standin";
const [binArg, wireArg, priorArg, outArg]=process.argv.slice(2);
const bin=resolve(binArg),wire=resolve(wireArg),out=resolve(outArg),prior=JSON.parse(readFileSync(priorArg,"utf8"));
if(!String(prior.root).startsWith("/tmp/bp-mail-catchup-"))throw new Error("only retained synthetic fixture allowed");
for(const f of readdirSync(wire).filter(x=>x.endsWith(".js")))(0,eval)(readFileSync(join(wire,f),"utf8"));
const W=(globalThis as any).Wire;
const cborSource = readFileSync(resolve("src/core/cbor.bend"), "utf8");
const dict = (name: string): string[] => JSON.parse(new RegExp(`def Cbor\\.${name}\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])\\n`).exec(cborSource)![1]);
const keys = dict("keys"), words = dict("words"), td = new TextDecoder();
function decodeCbor(bytes: Uint8Array): any {
  let p = 0;
  const arg = (n: number) => {
    if (n < 24) return n;
    if (n === 24) return bytes[p++];
    if (n === 25) { const v = bytes[p] * 256 + bytes[p + 1]; p += 2; return v; }
    if (n === 26) { const v = bytes[p] * 16777216 + bytes[p + 1] * 65536 + bytes[p + 2] * 256 + bytes[p + 3]; p += 4; return v; }
    throw new Error("unsupported CBOR argument");
  };
  const item = (key = false): any => {
    const head = bytes[p++], major = head >> 5, ai = head & 31;
    if (major === 7) return ai === 21 ? true : ai === 20 ? false : null;
    if (major === 6) { const tag = arg(ai), v = item(); return tag === 6 ? words[v] : Number(v); }
    const n = arg(ai);
    if (major === 0) return key ? keys[n] : n;
    if (major === 1) return -1 - n;
    if (major === 2 || major === 3) { const s = td.decode(bytes.subarray(p, p + n)); p += n; return s; }
    if (major === 4) return Array.from({ length: n }, () => item());
    if (major !== 5) throw new Error("unsupported CBOR major");
    const o: any = {}; for (let i = 0; i < n; i++) { const k = item(true); o[k] = item(); } return o;
  };
  const value = item(); if (p !== bytes.length) throw new Error("CBOR trailing bytes"); return value;
}


const sha=(v:string|Uint8Array)=>new Bun.CryptoHasher("sha256").update(v).digest("hex");
if(sha(readFileSync(bin))!==prior.binary_sha256)throw new Error("saved binary SHA mismatch");
const root=mkdtempSync(join(tmpdir(),"bp-mail-resume-")),home=join(root,"home"),fake=join(root,"agents");
mkdirSync(home);mkdirSync(fake);
copyFileSync(join(prior.root,"home","events.jsonl"),join(home,"events.jsonl"));
const retainedSha=sha(readFileSync(join(home,"events.jsonl")));
writeFileSync(join(fake,"claude"),standinScript);chmodSync(join(fake,"claude"),0o755);
for(const n of ["codex","grok","muse","agy","tailscale","notify-send","gh","git","xdg-open","gio"]){writeFileSync(join(fake,n),"#!/bin/sh\nexit 1\n");chmodSync(join(fake,n),0o755);}
const listen=Bun.listen({hostname:"127.0.0.1",port:0,socket:{data(){}}}),port=listen.port;listen.stop(true);if(port<3799)throw new Error("test port");
const proc=Bun.spawn([bin,"--home",home,"--port",String(port),"--host","127.0.0.1","--no-tailscale","--foreground"],{env:{...process.env,HOME:root,BACKPLANE_AGENTS:fake,PATH:`${fake}:${process.env.PATH}`,DISPLAY:"",WAYLAND_DISPLAY:"",BACKPLANE_NO_UPDATE:"1",BACKPLANE_KEYRING_LOCKED:"0",BACKPLANE_PEERS:"",BACKPLANE_LOCK_DIR:join(root,"locks"),BACKPLANE_DEPLOY_LOCKS:join(root,"deploy-locks"),FAKE_LOG:join(root,"agents.log"),FAKE_SECS:"30",FAKE_PER:"1",FAKE_CYCLE:"100000"},stdout:Bun.file(join(root,"stdout")),stderr:Bun.file(join(root,"stderr"))});
const sleep=(n:number)=>new Promise(r=>setTimeout(r,n));
const until=async(f:()=>boolean,why:string)=>{const end=performance.now()+30000;while(!f()){if(performance.now()>end)throw new Error(why);await sleep(5);}};
let rid=0;const sockets:WebSocket[]=[],threads:string[]=[];let control:any;
const report:any={root,binary_sha256:prior.binary_sha256,retained_source_root:prior.root,retained_disk_sha256:retainedSha,prior_test_sha256:prior.benchmark_sha256,test_sha256:sha(readFileSync(import.meta.path)),checks:{},error:null};
async function client(query=""){
 const ws=new WebSocket(`ws://127.0.0.1:${port}/ws${query}`);ws.binaryType="arraybuffer";sockets.push(ws);
 const c:any={ws,seen:[],wire_count:0,live_rows:0,origin:"",log:null,boot:false,deltas:new Map()};
 const pending=new Map<number,{resolve:(v:any)=>void,reject:(e:Error)=>void,timer:Timer}>();
 c.rpc=async(m:string,p:object)=>{const id=++rid;const answer=new Promise<any>((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error(m+" timeout")),15000);pending.set(id,{resolve,reject,timer});});ws.send(W.encode(JSON.stringify({id,m,p})));const r=await answer;if(!r.ok||r.error)throw new Error(JSON.stringify(r));return r.result;};
 ws.onmessage=e=>{try{const o=decodeCbor(new Uint8Array(e.data as ArrayBuffer));
  if(o.t==="log"){c.origin=o.origin;c.log={since:o.since,n:o.items?.length??0,more:o.more};}
  if(o.t==="log"||o.t==="changes"){for(const x of o.items??[]){c.seen.push(typeof x==="string"?JSON.parse(x):x);c.wire_count++;}}
  if(o.t==="live")c.live_rows+=o.items?.length??0;
  if((o.t==="log"&&!o.more)||(o.t==="changes"&&o.boot===false))c.boot=true;
  if(o.t==="delta"&&o.text)c.deltas.set(o.thread,(c.deltas.get(o.thread)??0)+1);
  if(o.t==="reply"){const p=pending.get(Number(o.id));if(p){clearTimeout(p.timer);pending.delete(Number(o.id));p.resolve(o);}}
 }catch(e){report.error=String(e);}};
 await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});return c;
}
try{
 const start=performance.now();for(;;){try{if((await fetch(`http://127.0.0.1:${port}/hello`,{signal:AbortSignal.timeout(500)})).ok)break;}catch{}if(proc.exitCode!==null||performance.now()-start>45000)throw new Error("startup");await sleep(25);}
 control=await client();await until(()=>control.boot,"control boot");
 for(let i=0;i<4;i++){const t=control.seen.find((x:any)=>x.$==="ThreadCreated"&&x.title===`retained-stream-${i}`)?.id;if(!t)throw new Error("missing retained fixture thread");threads.push(t);await control.rpc("turn.start",{thread:t,text:"Narrow resume assertion",msg:`resume-${i}-${Date.now()}`});}
 await until(()=>threads.every(t=>(control.deltas.get(t)??0)>=3),"four stand-in streams");
 const observer=await client();await until(()=>observer.boot&&observer.live_rows===4,"observer boot/live snapshot");
 const since=observer.wire_count,origin=observer.origin;observer.ws.close();
 // Five known commits give a nonempty suffix. Read the actual synthetic
 // disk log independently: valid JSON contributes one wire line, private
 // registrations/drops are sanitized, and the fixture's torn object is out.
 const wireOnDisk=()=>readFileSync(join(home,"events.jsonl"),"utf8").split("\n").flatMap(line=>{try{const x=JSON.parse(line);return[x.$==="DeviceDropped"||x.$==="DeviceRegistered"?{$:x.$,at:x.at??0}:x];}catch{return[];}});
 const canonical=(v:any):string=>v&&typeof v==="object"?Array.isArray(v)?"["+v.map(canonical).join(",")+"]":"{"+Object.keys(v).sort().map(k=>JSON.stringify(k)+":"+canonical(v[k])).join(",")+"}":JSON.stringify(v);
 for(let i=0;i<5;i++)await control.rpc("setting.set",{key:"retained.resume-suffix",value:String(i)});
 const before=wireOnDisk(),expected=before.slice(since);
 const resumed=await client(`?since=${since}&origin=${encodeURIComponent(origin)}`);await until(()=>resumed.boot&&resumed.live_rows===4,"resumed cutoff/live snapshot");
 report.ledger={wire_cursor:since,actual_wire_count_before_resume:before.length,expected_suffix_count:expected.length,nondurable_live_snapshot_rows_excluded:observer.live_rows,incorrect_old_cursor:since+observer.live_rows,server_resume_log:resumed.log,resumed_wire_items:resumed.wire_count,resumed_live_snapshot_rows:resumed.live_rows,expected_suffix_sha256:sha(canonical(expected)),received_suffix_sha256:sha(canonical(resumed.seen))};
 report.checks.cursor_bounded_by_actual_wire=since<=before.length;
 report.checks.durable_suffix_order_exact=canonical(resumed.seen)===canonical(expected)&&expected.length>=5;
 report.checks.durable_suffix_exact_once=JSON.stringify(resumed.seen.filter((x:any)=>x.$==="SettingSet"&&x.key==="retained.resume-suffix").map((x:any)=>x.value))===JSON.stringify(["0","1","2","3","4"]);
 report.checks.suffix_ends_at_actual_cutoff=since+resumed.wire_count===before.length;
 report.checks.cursor_excludes_live=observer.live_rows===4&&since===observer.seen.length;
 report.checks.resume_since_equal=resumed.log.since===since;
 report.checks.resume_no_retained_replay=!resumed.seen.some((x:any)=>x.$==="MailPosted"||x.$==="ConvSet");
 report.checks.resumed_live_snapshot=resumed.live_rows===4;
}catch(e){report.error=String(e);}
finally{
 if(control?.ws.readyState===WebSocket.OPEN)for(const thread of threads){try{await control.rpc("turn.interrupt",{thread});}catch{break;}}
 for(const ws of sockets)ws.close();if(proc.exitCode===null)proc.kill("SIGTERM");await proc.exited;
 writeFileSync(out,JSON.stringify(report,null,2)+"\n");console.log(JSON.stringify(report));
}
process.exit(report.error||Object.values(report.checks).some(v=>v!==true)?1:0);
