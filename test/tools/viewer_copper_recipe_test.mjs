// Actual narrow Bend IO probe, not a JS export/cache reimplementation:
// bend test/native/viewer_copper_probe.bend -o /tmp/viewer-copper-probe.js
// bun test/tools/viewer_copper_recipe_test.mjs /tmp/viewer-copper-probe.js
// Optional real saved CAD gate: COPPER_BOARD=... COPPER_KICAD_CLI=...
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync,mkdirSync,existsSync,statSync,utimesSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve,join,basename} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const probe=resolve(process.argv[2]||'/tmp/viewer-copper-probe.js'),dir=mkdtempSync(join(tmpdir(),'bp-copper-recipe-')),cache=join(dir,'cache');mkdirSync(cache);
const env={...process.env,XDG_CACHE_HOME:cache};
const hash=b=>createHash('sha256').update(b).digest('hex');
function run(mode,path,extra='') {
  const r=spawnSync(process.execPath,[probe,mode,path,String(extra)],{env,encoding:'utf8',timeout:15000,maxBuffer:2*1024*1024});
  assert.equal(r.status,0,r.stderr||r.error?.message);return JSON.parse(r.stdout.trim());
}
const fixture=join(dir,'board.kicad_pcb');writeFileSync(fixture,'first');
const flags=run('flags',fixture,join(dir,'export.part.glb')),base=['--include-pads','--include-soldermask','--include-silkscreen'];
for(const [key,values]of Object.entries(flags)) {
  assert.deepEqual(values.slice(0,3),['pcb','export','glb']);
  for(const f of base)assert(values.includes(f));
  for(const f of ['--include-tracks','--include-zones','--cut-vias-in-body'])assert.equal(values.includes(f),key.startsWith('full'),key+f);
  assert(!values.includes('--include-inner-copper'));assert(!values.includes('--fill-all-vias'));
  assert.equal(values.includes('--no-components'),key.endsWith('Bare'));
  assert.equal(values.includes('--subst-models'),key.endsWith('Parts'));
  assert.deepEqual(values.slice(-4),['-f','-o',join(dir,'export.part.glb'),fixture]);
}
const names=run('paths',fixture);
assert.equal(names.compactParts,names.recipeCompact);assert.equal(names.compactParts,names.solidCompact);assert.equal(names.fullParts,names.solidFull);
assert.equal(new Set([names.compactParts,names.compactBare,names.fullParts,names.fullBare]).size,4);
assert.match(basename(names.fullParts),/^board-full-outer-v1-parts-/);assert.match(basename(names.fullBare),/^board-full-outer-v1-bare-/);
assert.match(basename(names.compactParts),/^board-\d+\./);
writeFileSync(names.compactParts,'old pads-only published asset');assert(!existsSync(names.fullParts),'full recipe cannot hit an existing compact asset');
const old=statSync(fixture);writeFileSync(fixture,'other');utimesSync(fixture,old.atimeMs/1000,old.mtimeMs/1000);
assert.equal(statSync(fixture).size,old.size);assert.equal(Math.floor(statSync(fixture).mtimeMs/1000),Math.floor(old.mtimeMs/1000));
const changed=run('paths',fixture);assert.notEqual(changed.fullParts,names.fullParts,'exact revision invalidates equal-size/same-mtime-second saves');
const originalSources=run('sources',fixture);assert.equal(originalSources.glbCompact,fixture);assert.equal(originalSources.glbFull,fixture);assert.equal(originalSources.stepCompact,originalSources.stepFull);assert.match(basename(originalSources.stepFull),/^step3-/);
const published=join(dir,'published.glb'),stage=published+'.part.glb';writeFileSync(published,'published');writeFileSync(stage,'failed staging bytes');
assert.equal(run('promote',published,7).code,7);assert.equal(readFileSync(published,'utf8'),'published');assert(existsSync(stage));
writeFileSync(stage,'successful staging bytes');assert.equal(run('promote',published,0).code,0);assert.equal(readFileSync(published,'utf8'),'successful staging bytes');assert(!existsSync(stage));
assert.equal(run('promote',published,0).code,3,'successful CLI status cannot hide failed/missing-stage rename');assert.equal(readFileSync(published,'utf8'),'successful staging bytes');
const blocked=join(dir,'nonempty-target');mkdirSync(blocked);writeFileSync(join(blocked,'keep'),'keep');writeFileSync(blocked+'.part.glb','new');assert.equal(run('promote',blocked,0).code,3);assert(existsSync(blocked+'.part.glb'));assert.equal(readFileSync(join(blocked,'keep'),'utf8'),'keep');

function snapshot(path) {const s=statSync(path,{bigint:true});return {path,bytes:Number(s.size),sha256:hash(readFileSync(path)),revision:`${s.dev}:${s.ino}:${s.size}:${s.mtimeNs}:${s.ctimeNs}`};}
function glb(path) {
  const bytes=readFileSync(path);assert.equal(bytes.toString('ascii',0,4),'glTF');assert.equal(bytes.readUInt32LE(4),2);assert.equal(bytes.readUInt32LE(8),bytes.length);
  const n=bytes.readUInt32LE(12),j=JSON.parse(bytes.subarray(20,20+n)),bin=bytes.subarray(28+n),width={SCALAR:1,VEC2:2,VEC3:3,VEC4:4},size={5121:1,5123:2,5125:4,5126:4};
  function accessor(i) {const a=j.accessors[i],v=j.bufferViews[a.bufferView],w=width[a.type],s=size[a.componentType],stride=v.byteStride||w*s,off=(v.byteOffset||0)+(a.byteOffset||0);return {a,v,w,s,stride,off};}
  function get(i,k) {const a=accessor(i),r=[];for(let c=0;c<a.w;c++){const o=a.off+k*a.stride+c*a.s;r.push(a.a.componentType===5126?bin.readFloatLE(o):a.a.componentType===5125?bin.readUInt32LE(o):a.a.componentType===5123?bin.readUInt16LE(o):bin.readUInt8(o));}return r;}
  let triangles=0,vertices=0,normals=0,primitives=0;const meshes=[];
  for(let i=0;i<j.meshes.length;i++) {
    const m=j.meshes[i],digest=createHash('sha256');let mt=0;
    for(const p of m.primitives) {
      const count=j.accessors[p.attributes.POSITION].count;vertices+=count;normals+=j.accessors[p.attributes.NORMAL]?.count||0;
      const t=(p.indices===undefined?count:j.accessors[p.indices].count)/3;triangles+=t;mt+=t;primitives++;
      for(const ai of [...Object.values(p.attributes),p.indices].filter(x=>x!==undefined)) {
        const a=accessor(ai);digest.update(JSON.stringify([a.a.count,a.a.componentType,a.a.type]));
        if(a.stride===a.w*a.s)digest.update(bin.subarray(a.off,a.off+a.a.count*a.w*a.s));
        else for(let k=0;k<a.a.count;k++)digest.update(bin.subarray(a.off+k*a.stride,a.off+k*a.stride+a.w*a.s));
      }
    }
    meshes.push({name:m.name||'',triangles:mt,sha256:digest.digest('hex')});
  }
  return {path,bytes:bytes.length,sha256:hash(bytes),triangles,vertices,normals,meshCount:j.meshes.length,primitiveMeshes:primitives,materials:j.materials.length,images:j.images?.length||0,textures:j.textures?.length||0,meshes,j,get};
}
function vias(text) {
  const out=[];let start=-1,depth=0,quoted=false,escaped=false;
  for(let i=0;i<text.length;i++) {
    const c=text[i];if(quoted){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c==='"')quoted=false;continue;}if(c==='"'){quoted=true;continue;}
    if(c==='('){if(depth===1&&text.slice(i,i+4)==='(via'&&/\s/.test(text[i+4]))start=i;depth++;}
    if(c===')'){depth--;if(depth===1&&start>=0){const s=text.slice(start,i+1),a=s.match(/\(at\s+([^\s()]+)\s+([^\s()]+)/),d=s.match(/\(drill\s+([^\s()]+)/);if(a&&d)out.push({at:[Number(a[1]),Number(a[2])],drill:Number(d[1])});start=-1;}}
  }return out;
}
function bodyCentreHits(g,points) {
  const ids=g.j.meshes.map((m,i)=>[m.name,i]).filter(([name])=>name?.endsWith('_PCB')).map(([,i])=>i);assert.equal(ids.length,1,'one flat saved board substrate');
  for(const node of g.j.nodes.filter(n=>ids.includes(n.mesh)))assert(!node.matrix&&!node.translation&&!node.rotation&&!node.scale,'probe requires identity board-mesh placement');
  let top=-Infinity,bottom=Infinity;const horizontal=[];
  for(const p of g.j.meshes[ids[0]].primitives)for(let k=0,n=p.indices===undefined?g.j.accessors[p.attributes.POSITION].count:g.j.accessors[p.indices].count;k<n;k+=3) {
    const t=[];for(let a=0;a<3;a++){const v=g.get(p.attributes.POSITION,p.indices===undefined?k+a:g.get(p.indices,k+a)[0]);t.push([v[0]*1000,v[2]*1000,v[1]*1000]);top=Math.max(top,v[1]*1000);bottom=Math.min(bottom,v[1]*1000);}
    if(Math.max(...t.map(p=>p[2]))-Math.min(...t.map(p=>p[2]))<1e-6)horizontal.push(t);
  }
  function hit(p,[a,b,c]) {const den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1]);if(Math.abs(den)<1e-12)return false;const u=((b[1]-c[1])*(p[0]-c[0])+(c[0]-b[0])*(p[1]-c[1]))/den,v=((c[1]-a[1])*(p[0]-c[0])+(a[0]-c[0])*(p[1]-c[1]))/den;return u>=-1e-6&&v>=-1e-6&&u+v<=1+1e-6;}
  const ts=horizontal.filter(t=>Math.abs(t[0][2]-top)<1e-5),bs=horizontal.filter(t=>Math.abs(t[0][2]-bottom)<1e-5);return {topZMm:top,bottomZMm:bottom,solidCentres:points.filter(p=>ts.some(t=>hit(p.at,t))).length,solidBottomCentres:points.filter(p=>bs.some(t=>hit(p.at,t))).length,total:points.length};
}
if(process.env.COPPER_BOARD) {
  const board=resolve(process.env.COPPER_BOARD),sch=process.env.COPPER_SCHEMATIC||board.replace(/\.kicad_pcb$/,'.kicad_sch'),cli=process.env.COPPER_KICAD_CLI;assert(cli,'Explicit selected COPPER_KICAD_CLI required.');
  const inputs=[board,...(existsSync(sch)?[sch]:[])],before=inputs.map(snapshot),actualNames=run('paths',board),outputs=[];
  for(const [kind,destination]of [['compactParts',actualNames.compactParts],['fullParts',actualNames.fullParts]]) {
    const args=run('flags',board,destination+'.part.glb')[kind],t=performance.now(),r=spawnSync(cli,args,{encoding:'utf8',timeout:40000,maxBuffer:8*1024*1024}),elapsedMs=performance.now()-t;
    writeFileSync(join(dir,kind+'.log'),(r.stdout||'')+(r.stderr||''));assert.equal(r.status,0,r.stderr||r.error?.message);assert.equal(run('promote',destination,0).code,0);
    assert.deepEqual(inputs.map(snapshot),before,'saved CAD must remain unchanged after each export');
    const g=glb(destination);assert.equal(g.normals,g.vertices,'preserve actual source normals');outputs.push({...g,j:undefined,get:undefined,elapsedMs,command:[cli,...args],viaBodyCheck:bodyCentreHits(g,vias(readFileSync(board,'utf8')))});
  }
  const [compact,full]=outputs;assert(full.meshes.some(m=>m.name.endsWith('_copper')));assert(full.meshes.some(m=>m.name.endsWith('_via')));assert.equal(full.viaBodyCheck.solidCentres,0,'full detail must cut actual via centres through substrate');assert.equal(full.viaBodyCheck.solidBottomCentres,0,'through-via centres must also be open at the substrate bottom');assert(compact.viaBodyCheck.solidCentres>0);
  const sourceMeshes=compact.meshes.filter(m=>!m.name.endsWith('_PCB')),available=[...full.meshes];
  for(const m of sourceMeshes){const i=available.findIndex(n=>n.name===m.name&&n.sha256===m.sha256);assert(i>=0,'existing component/pad/mask/silk geometry changed: '+m.name);available.splice(i,1);}
  const receipt={scope:'Actual narrow Bend exporter/cache/promotion functions and selected CLI, saved canonical CAD read-only; not full-app or renderer acceptance.',probeSha256:hash(readFileSync(probe)),plotSha256:hash(readFileSync('src/server/plot.bend')),before,after:inputs.map(snapshot),names:actualNames,outputs,existingMeshGeometryHashesPreserved:sourceMeshes.length,densityAlert:'Unbatched full GLB has over 22000 primitive meshes/draws; this change does not qualify interactive rendering speed or illustrative-solder limits.'};
  const receiptPath=process.env.COPPER_RECEIPT||join(dir,'actual-receipt.json');writeFileSync(receiptPath,JSON.stringify(receipt,null,2));console.log(JSON.stringify({receipt:receiptPath,sourceMeshesPreserved:sourceMeshes.length,outputs:outputs.map(({path,bytes,triangles,vertices,normals,primitiveMeshes,elapsedMs,viaBodyCheck})=>({path,bytes,triangles,vertices,normals,primitiveMeshes,elapsedMs,viaBodyCheck}))}));
}
console.log('ok actual Bend copper recipes: compact compatibility, four recipe identities, exact revisions, STEP/GLB passthrough, no failed-stage promotion and rename failure handling; temp '+dir);
