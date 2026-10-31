import assert from 'node:assert/strict';
import {applyMaterialPolicy} from '../../src/web/solid-material-policy.js';
const materials=[
 {isMeshStandardMaterial:true,color:{r:.119,g:.059,b:.038},metalness:1,roughness:1,opacity:1},
 {isMeshStandardMaterial:true,color:{r:.645,g:.638,b:.572},metalness:1,roughness:.4,opacity:.83},
 {isMeshStandardMaterial:true,color:{r:.08,g:.2,b:.14},metalness:0,roughness:.6,opacity:.83},
];
const json={materials:[{pbrMetallicRoughness:{baseColorFactor:[.119,.059,.038,1]}},{pbrMetallicRoughness:{metallicFactor:1,roughnessFactor:.4}},{pbrMetallicRoughness:{metallicFactor:0,roughnessFactor:.6}}]};
const gltf={scene:{traverse:f=>f({isMesh:true,material:materials})},parser:{json,associations:new Map(materials.map((m,i)=>[m,{materials:i}]))}};
const colors=JSON.stringify(materials.map(m=>m.color));const summary=applyMaterialPolicy(gltf);
assert.equal(summary.missingFactorCount,2);assert.equal(summary.estimatedFactorCount,2);assert.equal(summary.materialSource,'original-glb-with-estimated-missing-factors');
assert.equal(materials[0].metalness,0);assert.equal(materials[0].roughness,.72);assert.equal(materials[1].metalness,1);assert.equal(materials[1].roughness,.4);assert.equal(materials[2].metalness,0);assert.equal(materials[2].roughness,.6);assert.equal(materials[1].opacity,.83);assert.equal(JSON.stringify(materials.map(m=>m.color)),colors);
assert.equal(applyMaterialPolicy(gltf,'source-defaults').estimatedFactorCount,0);assert.equal(materials[0].metalness,1);assert.equal(materials[0].roughness,1);assert.equal(materials[1].roughness,.4);
assert.equal(applyMaterialPolicy(gltf,'realistic').estimatedFactorCount,2,'realistic mode counts component estimates as well');
console.log('ok missing-factor-only estimates; explicit PBR/color/alpha untouched; source-default fidelity reversible');

const T=await import('../../tools/viewer/node_modules/three/build/three.module.js');
const mask=new T.MeshStandardMaterial({color:new T.Color(.0345,.0345,.0345),metalness:0,roughness:.6,opacity:.83,transparent:true,depthWrite:false});
const object=new T.Mesh(new T.PlaneGeometry(),mask),scene=new T.Scene();scene.add(object);
const exact={scene,parser:{json:{meshes:[{name:'saved_board_soldermask'}],materials:[{alphaMode:'BLEND',pbrMetallicRoughness:{metallicFactor:0,roughnessFactor:.6,baseColorFactor:[.0345,.0345,.0345,.83]}}]},associations:new Map([[object,{meshes:0}],[mask,{materials:0}]])}};
const geometry=object.geometry,beforeColor=mask.color.toArray(),realistic=applyMaterialPolicy(exact,'realistic',T);
assert.equal(realistic.materialRoleCounts.soldermask,1);assert.equal(realistic.explicitPresentationOverrideCount,3);assert.equal(object.material.isMeshPhysicalMaterial,true);assert.equal(object.material.opacity,1);assert.equal(object.material.transparent,false);assert.equal(object.material.clearcoat,1);assert.equal(object.material.clearcoatRoughness,.1);assert.deepEqual(object.material.color.toArray(),beforeColor);assert.equal(object.geometry,geometry);assert.equal(mask.opacity,.83,'original alpha remains untouched');assert.equal(mask.roughness,.6);
let disposed=0;object.material.addEventListener('dispose',()=>disposed++);applyMaterialPolicy(exact,'source-defaults',T);assert.equal(object.material,mask);assert.equal(mask.transparent,true);assert.equal(mask.opacity,.83);assert.equal(disposed,1,'presentation resources released on fidelity switch');
// The exact source mesh role is required; a name elsewhere or a gray/green
// material alone cannot establish coating provenance.
exact.parser.json.meshes[0].name='soldermask_guess';assert.equal(applyMaterialPolicy(exact,'realistic',T).explicitPresentationOverrideCount,0);assert.equal(object.material,mask);
exact.parser.json.meshes[0].name='saved_board_soldermask';const shared=new T.Mesh(geometry,mask);scene.add(shared);exact.parser.associations.set(shared,{meshes:1});exact.parser.json.meshes.push({name:'vendor_part'});assert.equal(applyMaterialPolicy(exact,'realistic',T).materialRoleCounts.ambiguous,1);assert.equal(object.material,mask,'shared unclassified material cannot inherit a board-only role');
applyMaterialPolicy(exact,'source-defaults',T);scene.remove(shared);exact.parser.associations.delete(shared);exact.parser.json.meshes.pop();
const studio=applyMaterialPolicy(exact,'cad-studio',T);assert.notEqual(object.material,mask);assert.equal(object.material.opacity,1);assert.equal(object.material.transparent,false);assert.equal(object.material.depthWrite,true);assert.deepEqual(object.material.color.toArray(),beforeColor);assert.equal(studio.explicitPresentationOverrideCount,2);assert.equal(studio.materialSource,'original-glb-with-opaque-dark-coating');assert.equal(mask.opacity,.83);assert.equal(mask.transparent,true);
applyMaterialPolicy(exact,'source-defaults',T);assert.equal(object.material,mask,'studio coating reversible');
mask.color.setRGB(.08,.2,.14);assert.equal(applyMaterialPolicy(exact,'cad-studio',T).presentationOverrideCount,0);assert.equal(object.material,mask,'a light (green) mask keeps its authored translucency');mask.color.setRGB(.0345,.0345,.0345);
console.log('ok exact original mesh roles, physical coat estimates, untouched source color/geometry/alpha, reversible disposal, ambiguous-role rejection');
