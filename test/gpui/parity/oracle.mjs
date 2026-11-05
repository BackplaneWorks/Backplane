// Differential oracle: events.jsonl -> Bend State.replay vs the Rust fold -> equal canonical digests.
//   bun oracle.mjs --log events.jsonl --bend "/path/digest" [--rust "cmd args"] [--chunk 4194304]
// Fixture or hub-generated logs only (the store copy never leaves its machine).
import {readFileSync,writeFileSync,mkdtempSync,rmSync,createReadStream} from 'node:fs';
import {createInterface} from 'node:readline';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>0?process.argv[i+1]:d};

export function canonical(d){
  const sortBy=(xs,k)=>[...xs].sort((a,b)=>a[k]<b[k]?-1:a[k]>b[k]?1:0);
  const o={projects:sortBy(d.projects||[],'id'),threads:sortBy(d.threads||[],'id'),settings:sortBy(d.settings||[],'key'),seq:d.seq};
  const go=v=>Array.isArray(v)?v.map(go):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,go(v[k])])):v;
  return JSON.stringify(go(o));
}
export const digest=d=>createHash('sha256').update(canonical(d)).digest('hex');
// stream the log (a store copy is 1.5 GB: never read it whole); at most max lines
export async function chunks(logPath,size,dir,max=Infinity){
  const files=[];let cur=[],n=0,total=0;
  const flush=()=>{if(!cur.length)return;const f=`${dir}/chunk-${files.length}.jsonl`;writeFileSync(f,cur.join('\n')+'\n');files.push(f);cur=[];n=0};
  const rl=createInterface({input:createReadStream(logPath),crlfDelay:Infinity});
  for await(const l of rl){if(!l)continue;if(total>=max)break;cur.push(l);total++;n+=l.length+1;if(n>=size)flush()}
  rl.close();flush();return files;
}
const run=(cmd,files)=>{const [bin,...a]=cmd.split(/\s+/);return JSON.parse(execFileSync(bin,[...a,...files],{maxBuffer:1<<28,env:process.env}).toString())};
if(import.meta.main){
  const log=arg('log'),bend=arg('bend'),rust=arg('rust'),size=Number(arg('chunk',4194304));
  const max=Number(arg('lines','Infinity'));const dir=mkdtempSync(tmpdir()+'/oracle-');const files=await chunks(log,size,dir,max);
  const b=run(bend,files);const hb=digest(b);
  console.log(JSON.stringify({log:log.replace(/.*\//,'…/'),lines:max,chunks:files.length,bend:{sha256:hb,projects:b.projects.length,threads:b.threads.length,settings:b.settings.length,seq:b.seq}}));
  let rc=0;
  if(rust){const r=run(rust,files);const hr=digest(r);console.log(JSON.stringify({rust:{sha256:hr,seq:r.seq},equal:hb===hr}));
    if(hb!==hr){rc=1;const cb=JSON.parse(canonical(b)),cr=JSON.parse(canonical(r));
      for(const k of ['projects','threads','settings','seq']){if(JSON.stringify(cb[k])!==JSON.stringify(cr[k]))console.log('  differs in',k,JSON.stringify(cb[k]).length,'vs',JSON.stringify(cr[k]).length)}}}
  else{console.log('rust side not given: Bend digest only');rc=2}
  rmSync(dir,{recursive:true,force:true});process.exit(rc);
}
