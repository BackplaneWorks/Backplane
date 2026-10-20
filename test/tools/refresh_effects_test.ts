// Exercise the actual JS filesystem effects on synthetic files only.
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const source=fs.readFileSync("src/server/effects/host.js","utf8").replace(/CID\(([^)]+)\)/g, '"$1"');
const effects=new Map<string,Function>();
let metadata: any = null;
new Function("require","io_eff","CID","io_done","io_fail","io_tup",source)(
  (n:string)=>n==="fs"?{statSync:(p:string,o:unknown)=>metadata??fs.statSync(p,o as any)}:require(n),(id:string,f:Function)=>effects.set(id,f),(n:unknown)=>n,
  (value:unknown)=>({ok:true,value}),(error:unknown)=>({ok:false,error}),
  (a:unknown,b:unknown)=>[a,b]);
const d=fs.mkdtempSync(join(tmpdir(),"bp-refresh-")), p=join(d,"s.kicad_sch");
const revision=()=>effects.get("FS.revision")!(p);
let bad=0;
const ok=(v:boolean,s:string)=>{console.log(`${v?'ok':'FAIL'} ${s}`);if(!v)bad++};
try {
  const sec=Math.floor(Date.now()/1000);
  fs.writeFileSync(p,"REV000");fs.utimesSync(p,sec,sec+.1);
  const a=revision();fs.readFileSync(p);const read=revision();
  ok(a.ok&&read.ok&&a.value[1]===read.value[1],"read does not change revision (no atime)");
  fs.writeFileSync(p,"REV001");fs.utimesSync(p,sec,sec+.2);const b=revision();
  ok(a.value[0]===b.value[0]&&a.value[1]!==b.value[1],"same-size rapid edit changes exact revision");
  const st=fs.statSync(p);fs.writeFileSync(p+".tmp","REV002");fs.utimesSync(p+".tmp",st.atime,st.mtime);fs.renameSync(p+".tmp",p);
  const c=revision();ok(c.ok&&c.value[1]!==b.value[1],"atomic replacement preserving mtime changes identity");
  // A coarse filesystem may return identical ctime for immediate writes.
  // Supply exact metadata here to test that even a one-nanosecond ctime
  // change survives the effect; real rapid edits are tested by both actors.
  metadata={...fs.statSync(p,{bigint:true}),ctimeNs:1792440000000000001n};
  const before=revision();metadata={...metadata,ctimeNs:1792440000000000002n};
  const e=revision();
  ok(e.ok&&e.value[1]!==before.value[1],"preserved mtime one-nanosecond ctime change survives");
  metadata=null;
  fs.unlinkSync(p);ok(!revision().ok,"missing file fails rather than fabricating revision");
} finally {fs.rmSync(d,{recursive:true,force:true});}
process.exit(bad?1:0);
