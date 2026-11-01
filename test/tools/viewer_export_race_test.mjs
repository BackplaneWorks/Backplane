// The actual Bend export function is driven by two controllable CLI writers.
// Build test/native/viewer_copper_probe.bend, then pass its emitted JS here.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn} from 'node:child_process';
const probe=resolve(process.argv[2]||'build/viewer_copper_probe.js');
const dir=fs.mkdtempSync(join(tmpdir(),'bp-export-race-')),barrier=join(dir,'barrier'),out=join(dir,'model.glb'),source=join(dir,'board.kicad_pcb'),cli=join(dir,'kicad-cli');
fs.mkdirSync(barrier);fs.writeFileSync(source,'(kicad_pcb)');
fs.writeFileSync(cli,`#!${process.execPath}
const fs=require('node:fs'),path=require('node:path');
if(process.argv.includes('--version')){console.log('10.0.6');process.exit(0);}
const stage=process.argv[process.argv.indexOf('-o')+1],dir=process.env.EXPORT_BARRIER,out=process.env.EXPORT_DEST;
function wait(fn){const start=Date.now();while(!fn()){if(Date.now()-start>10000)throw Error('export barrier timeout');Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,5);}}
function bytes(who){const json=Buffer.from(JSON.stringify({asset:{version:'2.0',generator:who}})),n=Math.ceil(json.length/4)*4,b=Buffer.alloc(20+n,32);b.write('glTF');b.writeUInt32LE(2,4);b.writeUInt32LE(b.length,8);b.writeUInt32LE(n,12);b.writeUInt32LE(0x4e4f534a,16);json.copy(b,20);return b;}
const fd=fs.openSync(stage,'w');
if(process.env.EXPORT_FAIL){fs.writeSync(fd,'partial');fs.closeSync(fd);process.exit(7);}
fs.writeFileSync(path.join(dir,process.pid+'.ready'),'');
wait(()=>fs.readdirSync(dir).filter(n=>n.endsWith('.ready')).length===2);
const first=fs.readdirSync(dir).filter(n=>n.endsWith('.ready')).sort()[0]===process.pid+'.ready';
if(!first)wait(()=>fs.existsSync(out));
const before=first?null:fs.readFileSync(out),b=bytes(first?'first':'second');
fs.writeSync(fd,b,0,20,0);fs.fsyncSync(fd);
fs.writeSync(fd,b,20,b.length-20,20);fs.ftruncateSync(fd,b.length);fs.fsyncSync(fd);
if(!first)fs.writeFileSync(path.join(dir,'isolated.json'),JSON.stringify(fs.readFileSync(out).equals(before)));
fs.closeSync(fd);
`,{mode:0o700});
const env={...process.env,HOME:dir,XDG_CACHE_HOME:join(dir,'cache'),BACKPLANE_HOME:join(dir,'home'),BACKPLANE_KICAD_CLI:cli,EXPORT_BARRIER:barrier,EXPORT_DEST:out};
function run(extra={}) {
 return new Promise((ok,no)=>{
  const p=spawn(process.execPath,[probe,'export',source,out],{env:{...env,...extra},stdio:['ignore','pipe','pipe']});let stdout='',stderr='';
  const timeout=setTimeout(()=>{p.kill();no(Error('export probe timed out'));},20000);
  p.stdout.on('data',b=>stdout+=b);p.stderr.on('data',b=>stderr+=b);p.on('error',no);
  p.on('close',code=>{clearTimeout(timeout);try{assert.equal(code,0,stderr);ok(JSON.parse(stdout.trim()));}catch(e){no(e);}});
 });
}
try {
 const results=await Promise.all([run(),run()]);assert.deepEqual(results.map(r=>r.code),[0,0],JSON.stringify(results));
 assert.equal(JSON.parse(fs.readFileSync(join(barrier,'isolated.json'))),true,'a second writer must not modify the model published by the first');
 const bytes=fs.readFileSync(out);assert.equal(bytes.readUInt32LE(8),bytes.length);assert.equal(JSON.parse(bytes.subarray(20).toString()).asset.generator,'second');
 assert(!fs.readdirSync(dir).some(n=>n.startsWith('model.glb.export-')),'completed exports remove their staging directories');
 const before=fs.readFileSync(out),failed=await run({EXPORT_FAIL:'1'});assert.equal(failed.code,7);assert.deepEqual(fs.readFileSync(out),before,'a failed writer preserves the published model');
 assert(!fs.readdirSync(dir).some(n=>n.startsWith('model.glb.export-')),'failed exports remove their staging directories');
 console.log('ok actual Bend overlapping exports: independent staging, both promotions succeed, published bytes stay unchanged while another writer runs, failure preserves cache and stages are cleaned');
}finally{fs.rmSync(dir,{recursive:true,force:true});}
