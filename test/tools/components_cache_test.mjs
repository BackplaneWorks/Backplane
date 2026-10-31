// The component page cache's real effects on a temporary project and cache:
// a page comes back only for the exact project, offset and both revisions,
// while both files still have those stamps.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const dir=fs.mkdtempSync(join(tmpdir(),'bp-component-pages-')),project=join(dir,'project'),other=join(dir,'other'),cache=join(dir,'cache');
fs.mkdirSync(project);fs.mkdirSync(other);process.env.XDG_CACHE_HOME=cache;
const effects=new Map();
const code=fs.readFileSync('src/server/effects/components_cache.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
new Function('require','io_eff',code)(require,(name,fn)=>effects.set(name,fn));
const put=(...a)=>effects.get('ComponentsFx.put')(...a),get=(...a)=>effects.get('ComponentsFx.get')(...a);
const revision=p=>{const s=fs.statSync(p,{bigint:true});return[s.dev,s.ino,s.size,s.mtimeNs/1000000000n,s.mtimeNs%1000000000n,s.ctimeNs/1000000000n,s.ctimeNs%1000000000n].join(':');};
const pcb=join(project,'b.kicad_pcb'),sch=join(project,'b.kicad_sch');fs.writeFileSync(pcb,'(kicad_pcb)');fs.writeFileSync(sch,'(kicad_sch)');
try {
 const pr=revision(pcb),sr=revision(sch),text='{"components":{"components":[],"page":{"offset":20}}}';
 assert.equal(put(project,pcb,sch,pr,sr,'20',text),'');
 assert.equal(get(project,'20',pr,sr),text,'the exact page comes back');
 assert.equal(get(project,'43',pr,sr),'','another offset misses');
 assert.equal(get(other,'20',pr,sr),'','another project misses');
 assert.equal(get(project,'20','',sr),'','no PCB revision misses');
 assert.equal(get(project,'20',pr,'1:2:3:4:5:6:7'),'','another schematic revision misses');
 assert.equal(fs.statSync(join(cache,'backplane-bend','component-pages')).mode&0o777,0o700,'private cache');
 fs.utimesSync(sch,new Date(),new Date(Date.now()+5000));assert.equal(get(project,'20',pr,sr),'','a changed schematic misses');
 fs.utimesSync(pcb,new Date(),new Date(Date.now()+9000));assert.equal(get(project,'20',pr,revision(sch)),'','a changed board misses');
 // a project without a schematic keeps an empty schematic revision
 const p2=revision(pcb);assert.equal(put(project,pcb,'',p2,'','5',text),'');assert.equal(get(project,'5',p2,''),text);
 assert.equal(put(project,pcb,sch,'',sr,'7',text),'');assert.equal(get(project,'7','',sr),'','nothing is kept without a PCB revision');
 console.log('ok component page cache: exact project/offset/revisions, stamps re-checked, private, no-schematic case');
} finally {fs.rmSync(dir,{recursive:true,force:true});}
