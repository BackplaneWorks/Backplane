// Resolve the selected saved board inside a disposable project. A canonical
// file may be configured outside it; an explicit different source may not.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
const dir=fs.mkdtempSync(join(tmpdir(),'bp-component-sources-')),cwd=join(dir,'project'),boards=join(cwd,'boards');
fs.mkdirSync(boards,{recursive:true});
const alpha=join(boards,'alpha.kicad_pcb'),beta=join(boards,'beta.kicad_pcb'),sch=join(boards,'beta.kicad_sch'),external=join(dir,'configured.kicad_pcb'),externalSch=join(dir,'configured.kicad_sch');
for(const p of [alpha,beta,sch,external,externalSch])fs.writeFileSync(p,'saved');
const effects=new Map(),code=fs.readFileSync('src/server/effects/components_cache.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
new Function('require','io_eff',code)(require,(name,fn)=>effects.set(name,fn));
const resolve=(path,canonical=alpha,schematic='')=>JSON.parse(effects.get('ComponentsFx.sources')(cwd,path,canonical,schematic));
try {
 assert.deepEqual(resolve(''),{source:alpha,schematic:''});
 assert.deepEqual(resolve(beta),{source:beta,schematic:sch});
 assert.deepEqual(resolve('boards/beta.kicad_pcb'),{source:beta,schematic:sch});
 assert.deepEqual(resolve(alpha,alpha,sch),{source:alpha,schematic:sch},'configured schematic remains authoritative for canonical board');
 assert.deepEqual(resolve('',external,externalSch),{source:external,schematic:externalSch});
 assert.deepEqual(resolve(external,external,externalSch),{source:external,schematic:externalSch});
 for(const path of [external,'../configured.kicad_pcb',join(cwd,'missing.kicad_pcb'),sch])assert(resolve(path).error,path);
 fs.symlinkSync(external,join(boards,'escaped.kicad_pcb'));assert(resolve('boards/escaped.kicad_pcb').error);
 fs.symlinkSync(beta,join(boards,'alias.kicad_pcb'));assert.equal(resolve('boards/alias.kicad_pcb').source,join(boards,'alias.kicad_pcb'));
 fs.unlinkSync(sch);assert.deepEqual(resolve(beta),{source:beta,schematic:''},'no paired sheet never falls back to alpha schematic');
 fs.symlinkSync(externalSch,sch);assert(resolve(beta).error,'a sheet symlink cannot leave the selected project');
 console.log('ok selected component sources: canonical config retained, board/sheet pairing, missing sheets and project boundaries');
}finally{fs.rmSync(dir,{recursive:true,force:true});}
