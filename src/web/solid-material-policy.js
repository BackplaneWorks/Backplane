import {materialEstimate} from './solid-materials.js';
const presentations=new WeakMap();
const roleOf=name=>/_soldermask$/.test(name)?'soldermask':/_silkscreen$/.test(name)?'silkscreen':/_pad$/.test(name)?'pad':/_(copper|via)$/.test(name)?'copper':/_PCB$/.test(name)?'substrate':'unclassified';
// Presentation replacements are reversible and never become original GLB
// materials. Role evidence comes from original mesh definitions, not RGB.
export function disposeMaterialPolicy(gltf){
  const previous=presentations.get(gltf);if(!previous)return;
  for(const [mesh,material]of previous.meshes)mesh.material=material;
  for(const material of previous.materials){gltf.parser?.associations.delete(material);material.dispose();}
  presentations.delete(gltf);
}
// Explicit glTF PBR factors always win. CAD exporters often omit both factors;
// the glTF defaults then make every component a fully rough metal. The studio
// policy fills only fields proven absent in the original material definition.
export function applyMaterialPolicy(gltf,policy='cad-studio',T){
  disposeMaterialPolicy(gltf);
  const mode=['source-defaults','realistic'].includes(policy)?policy:'cad-studio',materials=new Set(),roles=new Map();
  gltf.scene.traverse(o=>{if(o.isMesh){const index=gltf.parser?.associations.get(o)?.meshes,role=roleOf(gltf.parser?.json.meshes?.[index]?.name||'');for(const m of Array.isArray(o.material)?o.material:[o.material]){materials.add(m);if(!roles.has(m))roles.set(m,new Set());roles.get(m).add(role);}}});
  let missingFactorCount=0,estimatedFactorCount=0,unmappedMaterialCount=0;
  for(const material of materials){
    const index=gltf.parser?.associations.get(material)?.materials,definition=gltf.parser?.json.materials?.[index];
    if(!definition){unmappedMaterialCount++;continue;}
    if(!material.isMeshStandardMaterial&&!material.isMeshPhysicalMaterial)continue;
    const pbr=definition.pbrMetallicRoughness||{},estimate=materialEstimate(material.color.r,material.color.g,material.color.b);
    for(const [field,key,value]of [['metalness','metallicFactor',estimate.metalness],['roughness','roughnessFactor',estimate.roughness]]){
      if(Object.hasOwn(pbr,key)){material[field]=pbr[key];continue;}
      missingFactorCount++;material[field]=mode==='source-defaults'?1:value;if(mode!=='source-defaults')estimatedFactorCount++;
    }
    material.needsUpdate=true;
  }
  const materialRoleCounts={},replacements=new Map(),materialWarnings=[];let explicitPresentationOverrideCount=0,presentationOverrideCount=0;
  for(const [original,set]of roles){
    const role=set.size===1?[...set][0]:'ambiguous';materialRoleCounts[role]=(materialRoleCounts[role]||0)+1;
    if(role==='unclassified'||role==='ambiguous')continue;
    // A dark coating hides the laminate: KiCad's 83% alpha over its olive
    // substrate turns a black mask olive. Studio draws only dark masks
    // opaque; lighter masks keep the authored translucency over copper.
    const studio=mode==='cad-studio'&&role==='soldermask'&&original.transparent&&original.opacity<1&&Math.max(original.color.r,original.color.g,original.color.b)<.12;
    if(mode!=='realistic'&&!studio)continue;
    if(studio){const material=original.clone();for(const [key,value]of [['opacity',1],['transparent',false],['depthWrite',true]])if(material[key]!==value){presentationOverrideCount++;if(key!=='depthWrite')explicitPresentationOverrideCount++;material[key]=value;}
      material.userData={...original.userData,presentationRole:role,presentationSource:'estimated-opaque-dark-coating'};material.needsUpdate=true;replacements.set(original,material);const association=gltf.parser?.associations.get(original);if(association)gltf.parser.associations.set(material,{...association});continue;}
    if(!T?.MeshPhysicalMaterial){materialWarnings.push('Physical material support unavailable; original board factors retained.');continue;}
    const material=new T.MeshPhysicalMaterial();T.MeshStandardMaterial.prototype.copy.call(material,original);material.defines={...material.defines,PHYSICAL:''};
    const settings=role==='soldermask'?{metalness:0,roughness:.24,opacity:1,transparent:false,depthWrite:true,clearcoat:1,clearcoatRoughness:.10,ior:1.5,transmission:0}:
      role==='silkscreen'?{metalness:0,roughness:.82,opacity:1,transparent:false,depthWrite:true}:
      role==='substrate'?{metalness:0,roughness:.9}:
      {metalness:1,roughness:.28};
    const index=gltf.parser?.associations.get(original)?.materials,definition=gltf.parser?.json.materials?.[index],pbr=definition?.pbrMetallicRoughness||{};
    for(const [key,value]of Object.entries(settings)){if(original[key]!==value){presentationOverrideCount++;if(key==='metalness'&&Object.hasOwn(pbr,'metallicFactor')||key==='roughness'&&Object.hasOwn(pbr,'roughnessFactor')||key==='opacity'&&pbr.baseColorFactor?.length===4||key==='transparent'&&Object.hasOwn(definition||{},'alphaMode'))explicitPresentationOverrideCount++;}material[key]=value;}
    material.name=original.name;material.userData={...original.userData,presentationRole:role,presentationSource:'estimated-physical-board-role'};material.needsUpdate=true;
    replacements.set(original,material);const association=gltf.parser?.associations.get(original);if(association)gltf.parser.associations.set(material,{...association});
  }
  if(replacements.size){const meshes=[];gltf.scene.traverse(o=>{if(!o.isMesh)return;meshes.push([o,o.material]);o.material=Array.isArray(o.material)?o.material.map(m=>replacements.get(m)||m):replacements.get(o.material)||o.material;});presentations.set(gltf,{meshes,materials:[...replacements.values()]});}
  if(mode==='cad-studio'&&replacements.size)materialWarnings.push('Dark soldermask drawn opaque: its authored translucency would show the substrate colour through it.');
  if(mode==='realistic')materialWarnings.push('Board coating and finish are presentation estimates; unclassified component materials retain explicit factors and estimates only for missing fields.');
  return {materialPolicy:mode,materialPolicies:['realistic','cad-studio','source-defaults'],missingFactorCount,estimatedFactorCount,unmappedMaterialCount,materialRoleCounts,presentationOverrideCount,explicitPresentationOverrideCount,materialWarnings,materialSource:mode==='realistic'?'original-glb-with-estimated-physical-board-materials':replacements.size?'original-glb-with-opaque-dark-coating':estimatedFactorCount?'original-glb-with-estimated-missing-factors':'original-glb'};
}
