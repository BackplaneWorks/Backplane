// Retained synthetic 5k-conversation/50k-header log. No mail.send reseed,
// body IO or copied user data. Validates late replay-count initialization,
// exact wire order/cutoff while small commits arrive, and live delta gaps.
// bun test/tools/mail_catchup_e2e.ts BINARY WIREDIR OUT
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { standinScript } from "./perf_standin";
const [binArg, wireArg, outArg] = process.argv.slice(2);
if (!binArg || !wireArg || !outArg) throw new Error("BINARY WIREDIR OUT required");
const bin = resolve(binArg), wire = resolve(wireArg), out = resolve(outArg);
for (const f of readdirSync(wire).filter(x => x.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
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

const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
const until = async (f: () => boolean, why: string, ms = 60000) => {
  const end = performance.now() + ms;
  while (!f()) { if (performance.now() >= end) throw new Error(why); await sleep(5); }
};
const canon = (v: any): string => v && typeof v === "object" ? Array.isArray(v) ? "[" + v.map(canon).join(",") + "]" : "{" + Object.keys(v).sort().map(k => JSON.stringify(k) + ":" + canon(v[k])).join(",") + "}" : JSON.stringify(v);
const sha = (v: string | Uint8Array) => new Bun.CryptoHasher("sha256").update(v).digest("hex");
const stat = (xs: number[]) => { const a = [...xs].sort((x, y) => x-y); return { n:a.length, p50:a[Math.floor((a.length-1)*.5)] ?? null, p99:a[Math.floor((a.length-1)*.99)] ?? null, max:a.at(-1) ?? null }; };
const root = mkdtempSync(join(tmpdir(), "bp-mail-catchup-")), home = join(root, "home"), fake = join(root, "agents"), proj = join(root, "project");
for (const d of [home, fake, proj]) mkdirSync(d);
writeFileSync(join(fake, "claude"), standinScript); chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "agy", "tailscale", "notify-send", "gh", "git", "xdg-open", "gio"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const retained: any[] = [
  {$:"SettingSet",key:"restart.continue",value:"off",at:1},
  {$:"SettingSet",key:"agent.provider",value:"claude",at:2},
  {$:"SettingSet",key:"onboard.done",value:"1",at:3},
  {$:"UnknownFutureChange", fixture:"retained-unknown",at:4},
  {$:"DeviceDropped",token:"synthetic-private-token",at:5},
];
for (let c=0;c<5000;c++) retained.push({$:"ConvSet",id:`retained-c${c}`,subject:`Retained ${c}`,kind:"pair",parts:[`bot:fixture${c%6}`,"thread:fixture"],project:"",tags:[],at:10+c});
for (let m=0;m<50000;m++) retained.push({$:"MailPosted",id:`retained-m${m}`,conv:`retained-c${m%5000}`,from:"person",to:`bot:fixture${m%6}`,kind:"thread",ref:"",want:"fyi",head:`Retained preview ${m}`,size:20,at:10000+m});
const disk = retained.map(JSON.stringify).join("\n") + "\n{\n";
writeFileSync(join(home,"events.jsonl"),disk);
const expected = retained.map(x => x.$ === "DeviceDropped" ? {$:x.$,at:x.at} : x);
const listen = Bun.listen({hostname:"127.0.0.1",port:0,socket:{data(){}}}); const port=listen.port; listen.stop(true);
if (port<3799) throw new Error("test port below3799");
const trace=join(root,"trace.json");
const proc=Bun.spawn([bin,"--home",home,"--port",String(port),"--host","127.0.0.1","--no-tailscale","--foreground"], {
  env:{...process.env,HOME:root,BACKPLANE_AGENTS:fake,PATH:`${fake}:${process.env.PATH}`,DISPLAY:"",WAYLAND_DISPLAY:"",BACKPLANE_NO_UPDATE:"1",BACKPLANE_KEYRING_LOCKED:"0",BACKPLANE_PEERS:"",BACKPLANE_LOCK_DIR:join(root,"locks"),BACKPLANE_DEPLOY_LOCKS:join(root,"deploy-locks"),BACKPLANE_TRACE:trace,FAKE_LOG:join(root,"agents.log"),FAKE_SECS:"60",FAKE_PER:"1",FAKE_CYCLE:"100000"},
  stdout:Bun.file(join(root,"stdout")),stderr:Bun.file(join(root,"stderr")),
});
const base=`http://127.0.0.1:${port}`, sockets:WebSocket[]=[], threads:string[]=[];
const report:any={root,port,binary:bin,binary_sha256:sha(readFileSync(bin)),benchmark_sha256:sha(readFileSync(import.meta.path)),fixture:{conversations:5000,headers:50000,wire_lines:expected.length,torn_lines:1,disk_sha256:sha(disk),expected_wire_sha256:sha(expected.map(canon).join("\n"))},checks:{},error:null};
let rid=0, measuredStart=0, measuredEnd=0;
type Client={ws:WebSocket,seen:any[],deltas:Map<string,number[]>,frames:any[],opened:number,bootEnd:number,rpc:(m:string,p:object)=>Promise<any>};
const clients:Client[]=[];
async function client(suffix=""):Promise<Client>{
  const ws=new WebSocket(base.replace("http:","ws:")+"/ws"+suffix); ws.binaryType="arraybuffer"; sockets.push(ws);
  const pending=new Map<number,{resolve:(v:any)=>void,reject:(e:Error)=>void,timer:Timer}>();
  const c:Client={ws,seen:[],deltas:new Map(),frames:[],opened:performance.now(),bootEnd:0,rpc:async(m,p)=>{
    const id=++rid; const answer=new Promise<any>((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(m+" timeout"));},30000);pending.set(id,{resolve,reject,timer});});
    ws.send(W.encode(JSON.stringify({id,m,p})));const r=await answer;if(!r.ok||r.error)throw new Error(JSON.stringify(r));return r.result;
  }};
  ws.onmessage=e=>{
    try {
      const t=performance.now(),o=decodeCbor(new Uint8Array(e.data as ArrayBuffer));
      if(o.t==="reply"){const p=pending.get(Number(o.id));if(p){clearTimeout(p.timer);pending.delete(Number(o.id));p.resolve(o);}}
      if(o.t==="log"||(o.t==="changes"&&o.boot!==undefined))c.frames.push({t,kind:o.t,boot:o.boot,more:o.more,since:o.since,n:o.items?.length??0,received:c.seen.length});
      if(o.t==="log") (c as any).origin=o.origin;
      if(o.t==="log"||o.t==="changes")for(const x of o.items??[])c.seen.push(typeof x==="string"?JSON.parse(x):x);
      if((o.t==="log"&&!o.more)||(o.t==="changes"&&o.boot===false))c.bootEnd ||= t;
      if(o.t==="delta"&&o.text){const a=c.deltas.get(o.thread)??[];a.push(t);c.deltas.set(o.thread,a);}
    }catch(e){report.error=String(e);}
  };
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});clients.push(c);return c;
}
const perf=async()=>await(await fetch(base+"/debug/perf.json",{signal:AbortSignal.timeout(10000)})).json();
try{
  const start=performance.now();for(;;){try{if((await fetch(base+"/hello",{signal:AbortSignal.timeout(500)})).ok)break;}catch{}if(proc.exitCode!==null||performance.now()-start>60000)throw new Error("startup timeout");await sleep(25);}
  const control=await client();await until(()=>!!control.bootEnd,"initial retained join cutoff");
  report.checks.initial_order=control.seen.slice(0,expected.length).every((x,i)=>canon(x)===canon(expected[i]))&&control.seen.length>=expected.length;
  report.checks.private_sanitized=!JSON.stringify(control.seen).includes("synthetic-private-token");
  await control.rpc("project.add",{path:proj});await until(()=>control.seen.some(x=>x.$==="ProjectCreated"&&x.root===proj),"project");
  const project=control.seen.find(x=>x.$==="ProjectCreated"&&x.root===proj).id;
  for(let i=0;i<4;i++){await control.rpc("thread.create",{project,title:`retained-stream-${i}`,env:"local",provider:"claude"});await until(()=>control.seen.some(x=>x.$==="ThreadCreated"&&x.title===`retained-stream-${i}`),"thread");threads.push(control.seen.find(x=>x.$==="ThreadCreated"&&x.title===`retained-stream-${i}`).id);}
  for(const thread of threads)await control.rpc("turn.start",{thread,text:"Stream during retained catch-up",msg:`start-${thread}`});
  await until(()=>threads.every(t=>(control.deltas.get(t)?.length??0)>=3),"four stand-in streams");
  report.before=await perf();measuredStart=performance.now();
  const observer=await client();await until(()=>observer.frames.length>0,"observer first log");
  for(let wave=0;wave<256;wave++){
    await Promise.all(Array.from({length:4},(_,j)=>control.rpc("setting.set",{key:"retained.commit",value:String(wave*4+j),})));await sleep(5);
  }
  await control.rpc("setting.set",{key:"retained.done",value:"yes"});
  await until(()=>!!observer.bootEnd&&observer.seen.some(x=>x.$==="SettingSet"&&x.key==="retained.done"),"observer catches final durable marker");
  await until(()=>threads.every(t=>(observer.deltas.get(t)?.length??0)>=5),"observer receives live deltas");
  measuredEnd=performance.now();report.after=await perf();
  report.checks.observer_order=observer.seen.slice(0,expected.length).every((x,i)=>canon(x)===canon(expected[i]))&&observer.seen.length>=expected.length;
  const a=control.seen.filter(x=>x.$==="SettingSet"&&x.key==="retained.commit").map(x=>x.value),b=observer.seen.filter(x=>x.$==="SettingSet"&&x.key==="retained.commit").map(x=>x.value);
  report.checks.fresh_exact_once=a.length===1024&&b.length===1024&&JSON.stringify(a)===JSON.stringify(b)&&new Set(b).size===1024;
  report.checks.boot_cutoff=observer.frames.filter(x=>x.boot===false).length===1;
  report.observer={boot_ms:observer.bootEnd-observer.opened,frames:observer.frames,post_retained_boot_frames:observer.frames.filter(x=>x.boot===true&&x.received>=expected.length).length};
  // Reconnect from the exact received wire count and origin. No retained
  // header may reappear; the empty cutoff/live snapshots remain allowed.
  const at=observer.seen.length,origin=(observer as any).origin;
  const resumed=await client(`?since=${at}&origin=${encodeURIComponent(origin)}`);await until(()=>!!resumed.bootEnd,"resume cutoff");
  report.checks.resume_no_retained_replay=!resumed.seen.some(x=>x.$==="MailPosted"||x.$==="ConvSet");
  report.streams=Object.fromEntries([["live",control],["joining",observer]].map(([name,c]:any)=>[name,Object.fromEntries(threads.map(t=>{const ts=[measuredStart,...(c.deltas.get(t)??[]).filter((x:number)=>x>=measuredStart&&x<=measuredEnd),measuredEnd];return[t,{n:ts.length-2,longest_gap_ms:Math.max(...ts.slice(1).map((x:number,i:number)=>x-ts[i]))}];}))]));
  report.checks.live_gap=Object.values(report.streams.live).every((x:any)=>x.n>=3&&x.longest_gap_ms<=1000);
  report.checks.joining_gap=Object.values(report.streams.joining).every((x:any)=>x.n>=3&&x.longest_gap_ms<=1000);
  report.wall_ms=measuredEnd-measuredStart;
}catch(e){report.error=String(e);}
finally{
  if(clients[0]?.ws.readyState===WebSocket.OPEN)for(const thread of threads){try{await clients[0].rpc("turn.interrupt",{thread});}catch{break;}}
  for(const ws of sockets)ws.close();if(proc.exitCode===null)proc.kill("SIGTERM");await proc.exited;
  const spans:Record<string,number[]>={};
  try{for(const line of readFileSync(trace,"utf8").split("\n")){let x:any;try{x=JSON.parse(line.replace(/,$/,""));}catch{continue;}if(x.ph==="X"&&(x.name==="hub.join_more"||x.name==="hub.agent_line")){(spans[x.name]??=[]).push(x.dur/1000);}}}catch{}
  report.trace_ms=Object.fromEntries(Object.entries(spans).map(([k,v])=>[k,stat(v)]));
  writeFileSync(out,JSON.stringify(report,null,2)+"\n");console.log(JSON.stringify({out,checks:report.checks,error:report.error,trace_ms:report.trace_ms,streams:report.streams,observer_boot_ms:report.observer?.boot_ms}));
}
process.exit(report.error||Object.entries(report.checks).some(([k,v])=>!k.endsWith("_gap")&&v!==true)?1:0);
