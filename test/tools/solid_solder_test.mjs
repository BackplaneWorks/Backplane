// Focused geometry qualification. No app build or CAD writes.
// THREE_MODULE=/path/to/three.module.js node test/tools/solid_solder_test.mjs
// Optional actual saved-board gate: SOLDER_GLB=... SOLDER_COMPONENTS=...
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {resolve,dirname} from 'node:path';
import {createHash} from 'node:crypto';
import {buildSolderPreview} from '../../src/web/solid-solder.js';
const modulePath=resolve(process.env.THREE_MODULE||'tools/viewer/node_modules/three/build/three.module.js');
const T=await import(pathToFileURL(modulePath));
const pad=(number,x)=>({number,type:'smd',shape:'rect',board:[x,0],local:[x,0],size:[1,1.2],angle:0});
const c={id:'source-uuid',reference:'C1',layer:'F.Cu',at:[0,0,0],pads:[pad('1',-1.05),pad('2',1.05)]};
const data={schema:1,source:'/saved/board.kicad_pcb',sourceRevision:'exact-save',thicknessMm:1.6,components:[c]};
const asset={source:data.source,revision:data.sourceRevision};
function mesh(points,parent) {
  const g=new T.BufferGeometry();g.setAttribute('position',new T.Float32BufferAttribute(points.flat().map((x,i)=>x*.001*(i%3===1?-1:1)),3));
  const m=new T.Mesh(g,new T.MeshBasicMaterial({color:0x333333}));parent.add(m);return m;
}
function fixture({terminal=true,pads=true,reference='C1',side=1,angle=0,partitioned=true}={}) {
  const model=new T.Group(),body=new T.Group();body.name=reference;model.add(body);
  if(pads)for(const p of c.pads) {
    const x=p.board[0],a=[x-.5,-.6,1.64],b=[x+.5,-.6,1.64],d=[x-.5,.6,1.64],e=[x+.5,.6,1.64];mesh([a,b,d,b,e,d],model);
  }
  const terminalMaterial=new T.MeshBasicMaterial({color:0x333333});
  if(terminal)for(const x of [-1,1]) {
    const a=[x,-.4,1.68],b=[x,.4,1.68],d=[x,-.4,2.3],e=[x,.4,2.3];const m=mesh([a,b,d,b,e,d],body);m.material=terminalMaterial;
  }
  const centre=mesh([[-.6,-.4,2.3],[.6,-.4,2.3],[0,.4,2.3]],body);if(!partitioned)centre.material=terminalMaterial;
  if(side===-1){for(const o of [model]){o.scale.z=-1;o.position.z=.0016;}}
  model.rotation.z=angle;return model;
}
function run(model=fixture(),components=data,extra={}) {return buildSolderPreview({THREE:T,model,components,asset,...extra});}
let model=fixture(),original=model.toJSON(),r=run(model);
assert.equal(r.coverage.joints,2,JSON.stringify(r.skipped));assert.equal(r.group.visible,false);assert.equal(r.coverage.components,1);
assert.deepEqual(model.toJSON(),original,'source meshes/materials/transforms unchanged');
assert(r.group.children.every(m=>m.userData.illustrative&&m.name.startsWith('Illustrative solder')));
for(const a of r.coverage.anchors){assert.equal(a.terminalTriangles,2);assert.equal(a.padTriangles,2);assert(Math.abs(a.padSurfaceZMm-1.64)<.001);}
for(const m of r.group.children)assert([...m.geometry.attributes.position.array].every(Number.isFinite));
const scene=new T.Scene();scene.add(r.group);r.dispose();r.dispose();assert.equal(r.group.parent,null);assert.equal(r.group.children.length,0);assert.equal(model.children.length,3);
const rejects=[
  [fixture(),{...data,sourceRevision:'stale'},'revision'],
  [fixture(),{...data,source:'/other'},'revision'],
  [fixture(),{...data,sourceRevision:''},'revision'],
  [fixture(),{...data,components:[c,c]},'UUID'],
  [fixture({reference:'renamed'}),data,'ancestor'],
  [fixture({terminal:false}),data,'terminal'],
  [fixture({pads:false}),data,'pad face'],
  [fixture({partitioned:false}),data,'terminal'],
  [fixture(),{...data,components:[{...c,pads:c.pads.map(p=>({...p,type:'thru_hole'}))}]},'SMD'],
  [fixture(),{...data,components:[{...c,pads:c.pads.map(p=>({...p,shape:'custom'}))}]},'SMD'],
  [fixture(),{...data,components:[{...c,pads:c.pads.map(p=>({...p,number:'1'}))}]},'SMD'],
  [fixture(),{...data,components:[{...c,pads:c.pads.map(p=>({...p,board:[p.board[0]+.2,p.board[1]]}))}]},'pad face'],
];
for(const [model,components,reason]of rejects){r=run(model,components);assert.equal(r.coverage.joints,0,reason);assert(r.skipped.some(s=>s.reason.toLowerCase().includes(reason.toLowerCase())),JSON.stringify(r.skipped));r.dispose();}
model=fixture();const duplicate=new T.Group();duplicate.name='C1';model.add(duplicate);assert.equal(run(model).coverage.joints,0,'duplicate original ancestor');
model=fixture();model.children[0].children[0].geometry.attributes.position.setZ(0,NaN);assert.equal(run(model).coverage.joints,0,'nonfinite terminal must not be repaired by guess');
assert.equal(run(fixture(),data,{maxJoints:1}).coverage.joints,0,'strict bounded budget');
model=fixture({side:-1});r=run(model,{...data,components:[{...c,layer:'B.Cu'}]});assert.equal(r.coverage.joints,2,JSON.stringify(r.skipped));assert(r.coverage.anchors.every(a=>a.contactHeightMm<0));r.dispose();
// A true saved pad/component rotation must agree with transformed real meshes.
const a=.63,rot=([x,y])=>[Math.cos(a)*x+Math.sin(a)*y,-Math.sin(a)*x+Math.cos(a)*y];
r=run(fixture({angle:a}),{...data,components:[{...c,pads:c.pads.map(p=>({...p,board:rot(p.board),angle:-a*180/Math.PI}))}]});assert.equal(r.coverage.joints,2,JSON.stringify(r.skipped));r.dispose();

if(process.env.SOLDER_GLB&&process.env.SOLDER_COMPONENTS) {
  const loaderPath=resolve(dirname(modulePath),'../examples/jsm/loaders/GLTFLoader.js');
  const {GLTFLoader}=await import(pathToFileURL(loaderPath));
  const bytes=readFileSync(process.env.SOLDER_GLB),before=createHash('sha256').update(bytes).digest('hex');
  const gltf=await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),'');
  gltf.scene.rotation.x=Math.PI/2;gltf.scene.updateMatrixWorld(true);
  const components=JSON.parse(readFileSync(process.env.SOLDER_COMPONENTS)),asset={source:components.source,revision:components.sourceRevision};
  const started=performance.now(),preview=buildSolderPreview({THREE:T,model:gltf.scene,components,asset,maxJoints:128}),elapsedMs=performance.now()-started;
  assert(preview.coverage.anchors.some(a=>a.reference==='C5'),'actual C5 must have two source-geometry anchors: '+JSON.stringify(preview.skipped.find(s=>s.reference==='C5')));
  assert.equal(createHash('sha256').update(readFileSync(process.env.SOLDER_GLB)).digest('hex'),before);
  const receipt={source:components.source,revision:components.sourceRevision,glb:process.env.SOLDER_GLB,glbSha256:before,elapsedMs,coverage:preview.coverage,skipped:preview.skipped,defaultVisible:preview.group.visible};
  if(process.env.SOLDER_RECEIPT)writeFileSync(process.env.SOLDER_RECEIPT,JSON.stringify(receipt,null,2));
  console.log(JSON.stringify({actualSavedBoard:true,elapsedMs,joints:preview.coverage.joints,components:preview.coverage.components,skipped:preview.skipped.length,C5:preview.coverage.anchors.filter(a=>a.reference==='C5')}));preview.dispose();
}
console.log('ok illustrative solder: revision/UUID guards, actual pad-face and terminal-edge geometry, same-color materials, absent/ambiguous/unsupported geometry, rotations, both sides, budget and reversible disposal');
