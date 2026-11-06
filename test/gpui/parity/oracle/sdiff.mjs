// How two digests differ, content-safe: setting KEY PATTERNS (digits and ids folded) with counts, and thread FIELD names. Never values.
// usage: bun sdiff.mjs LOG LINES BEND_BIN RUST_BIN
import {execFileSync} from 'node:child_process';import {canonical,without,SKIP,chunks} from '../oracle.mjs';
import {mkdtempSync,rmSync} from 'node:fs';import {tmpdir} from 'node:os';
const [log,lines,b,r]=process.argv.slice(2);
const dir=mkdtempSync(tmpdir()+'/sdiff-');const files=await chunks(log,4194304,dir,Number(lines));
const run=(bin,extra=[])=>JSON.parse(execFileSync(bin,[...extra,...files],{maxBuffer:1<<30}).toString());
const A=JSON.parse(canonical(without(run(b),SKIP))),B=JSON.parse(canonical(without(run(r,['--json']),SKIP)));
const pat=k=>k.replace(/[pt]\d{10}-\d+/g,'<id>').replace(/\d+/g,'#');
const ma=new Map(A.settings.map(x=>[x.key,x.value])),mb=new Map(B.settings.map(x=>[x.key,x.value]));
const kinds=new Map();
for(const k of new Set([...ma.keys(),...mb.keys()])){const a=ma.get(k),c=mb.get(k);if(a===c)continue;const t=a===undefined?'only in rust':c===undefined?'only in bend':'value differs';const key=pat(k)+'  ['+t+']';kinds.set(key,(kinds.get(key)||0)+1)}
console.log('SETTINGS differing, by key pattern:');for(const [k,n] of [...kinds].sort((x,y)=>y[1]-x[1]).slice(0,30))console.log(String(n).padStart(6),k);
const ta=new Map(A.threads.map(x=>[x.id,x])),tb=new Map(B.threads.map(x=>[x.id,x]));const fields=new Map();
for(const id of new Set([...ta.keys(),...tb.keys()])){const x=ta.get(id),y=tb.get(id);if(!x||!y){fields.set('thread only in '+(x?'bend':'rust'),(fields.get('thread only in '+(x?'bend':'rust'))||0)+1);continue}
  for(const f of Object.keys(x))if(JSON.stringify(x[f])!==JSON.stringify(y[f]))fields.set(f,(fields.get(f)||0)+1)}
console.log('THREADS differing, by field:');for(const [f,n] of fields)console.log(String(n).padStart(6),f);
console.log('projects',JSON.stringify(A.projects)===JSON.stringify(B.projects)?'equal':'DIFFER','seq',A.seq,B.seq);
rmSync(dir,{recursive:true,force:true});
