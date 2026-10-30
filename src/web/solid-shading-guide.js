// Generated coating shading normals only. Owns its color/depth target and at
// most two guide proxies/materials; borrows source geometry and normal maps.
export const shadingGuideLimits=Object.freeze({nodes:60000,depth:128,meshes:2,pixels:2000000,bytes:16*1024*1024});
const same=(a,b)=>a.length===b.length&&a.every((x,i)=>x===b[i]);
const hierarchyVisible=o=>{for(let p=o;p;p=p.parent)if(!p.visible)return false;return true;};
// Match pinned pathtracer0.0.24 source+colorSpace texture deduplication.
// This cap qualifies the optional coating trace only, never drops source maps.
export function qualifyCoatingTraceLayers(scene){
 const stack=[[scene,0]],materials=new Set(),layers=new Set();let nodes=0;
 while(stack.length){const[o,depth]=stack.pop();if(++nodes>60000||depth>128||stack.length+o.children.length>60000)return{supported:false,reason:'Coating trace texture inventory hierarchy budget exceeded'};for(const child of o.children)stack.push([child,depth+1]);if(o.isMesh)for(const m of Array.isArray(o.material)?o.material:[o.material])materials.add(m);}
 for(const m of materials)for(const value of Object.values(m))if(value?.isTexture){if(!value.source?.uuid)return{supported:false,reason:'Unknown coating trace texture source'};layers.add(value.source.uuid+':'+value.colorSpace);if(layers.size>8)return{supported:false,reason:'Coating trace color array exceeds32MiB at existing1024 slots'};}
 return{supported:true,layers:Math.max(1,layers.size),bytes:Math.max(1,layers.size)*1024*1024*4};
}
export class ShadingNormalGuide {
 constructor(T){this.T=T;this.target=null;this.ownedDepth=null;this.bindings=[];this.materials=[];this.bytes=0;this.disposed=false;this.passes=0;this.signature=null;}
 clearBindings(){for(const m of this.materials)m.dispose();this.materials=[];this.bindings=[];this.scene?.clear();this.signature=null;}
 releaseTarget(){const target=this.target,depth=this.ownedDepth;this.target=null;this.ownedDepth=null;this.bytes=0;if(target){target.depthTexture=null;target.dispose();}depth?.dispose();}
 dispose(){if(this.disposed)return;this.disposed=true;this.clearBindings();this.releaseTarget();this.depthMaterial?.dispose();this.depthMaterial=null;this.scene=null;}
 fallback(reason){this.clearBindings();this.releaseTarget();this.passes=0;return{supported:false,reason,bytes:0,passes:0,mapped:0};}
 render({renderer,scene,camera,width,height,eligibleBindings=[],isCurrent=()=>true}={}){
  const T=this.T,limits=shadingGuideLimits,pixels=width*height;
  if(this.disposed)return{supported:false,reason:'Shading guide disposed',bytes:0,passes:0,mapped:0};
  if(!isCurrent())return this.fallback('Shading guide source/presentation superseded');
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||!Number.isSafeInteger(pixels)||pixels>limits.pixels||pixels*8>limits.bytes)return this.fallback('Shading guide exceeds actual-target pixel/16MiB color+depth budget; raw HDR retained');
  if(scene.overrideMaterial)return this.fallback('Unknown scene override prevents a faithful shading guide; raw HDR retained');
  const eligible=new Map(eligibleBindings.map(b=>[b.material,b.texture])),mapped=[],stack=[[scene,0]];let nodes=0,depthUnsupported='';
  while(stack.length){const[o,depth]=stack.pop();if(++nodes>limits.nodes||depth>limits.depth||stack.length+o.children.length>limits.nodes)return this.fallback('Shading guide hierarchy budget exceeded; raw HDR retained');for(let i=o.children.length-1;i>=0;i--)stack.push([o.children[i],depth+1]);if(!o.isMesh)continue;
   if(o.type!=='Mesh'||o.isSkinnedMesh||o.isInstancedMesh||o.isBatchedMesh||Object.keys(o.geometry.morphAttributes||{}).length||o.customDepthMaterial||o.customDistanceMaterial||['onBeforeRender','onAfterRender','onBeforeShadow','onAfterShadow'].some(k=>o[k]!==T.Object3D.prototype[k]))depthUnsupported='Unknown depth-pass mesh feature';
   for(const m of Array.isArray(o.material)?o.material:[o.material]){
    if(m.transparent||m.opacity!==1||m.alphaTest||m.alphaHash||m.alphaToCoverage||(m.transmission||0)>0||!m.depthTest||!m.depthWrite||m.allowOverride!==true||m.side!==T.DoubleSide||m.wireframe||m.depthFunc!==T.LessEqualDepth||m.clippingPlanes?.length||m.polygonOffset||m.stencilWrite||m.onBeforeCompile!==T.Material.prototype.onBeforeCompile)depthUnsupported='Unknown or translucent depth-pass material';
    if(m.bumpMap||m.displacementMap)return this.fallback('Authored bump/displacement has no qualified shading guide; raw HDR retained');
    if(!m.normalMap&&!m.clearcoatNormalMap)continue;
    if(Array.isArray(o.material)||eligible.get(m)!==m.normalMap||m.clearcoatNormalMap!==m.normalMap||m.normalMapType!==T.TangentSpaceNormalMap||!m.clearcoatNormalScale||!m.normalScale?.equals(m.clearcoatNormalScale))return this.fallback('Authored, divergent or unknown normal lobes have no qualified shading guide; raw HDR retained');
    if(o.type!=='Mesh'||o.isSkinnedMesh||o.isInstancedMesh||o.isBatchedMesh||Object.keys(o.geometry.morphAttributes||{}).length||o.geometry.groups.length||o.geometry.drawRange.start!==0||o.geometry.drawRange.count!==Infinity||m.transparent||m.opacity!==1||m.alphaTest||m.alphaHash||m.alphaToCoverage||m.wireframe||m.clippingPlanes?.length||m.polygonOffset||m.stencilWrite||m.onBeforeCompile!==T.Material.prototype.onBeforeCompile||['onBeforeRender','onAfterRender','onBeforeShadow','onAfterShadow'].some(k=>o[k]!==T.Object3D.prototype[k]))return this.fallback('Unsupported mapped surface feature; raw HDR retained');
    const channel=m.normalMap.channel,uv=channel===0?'uv':'uv'+channel;if(!Number.isInteger(channel)||channel<0||channel>3||!o.geometry.attributes[uv]||!o.geometry.attributes.normal)return this.fallback('Mapped surface lacks a qualified UV/normal channel; raw HDR retained');
    mapped.push({source:o,material:m});if(mapped.length>limits.meshes)return this.fallback('Generated mask guide exceeds two mapped surfaces; raw HDR retained');
   }
  }
  if(!mapped.length){this.clearBindings();this.releaseTarget();this.passes=0;return{supported:true,texture:null,bytes:0,passes:0,mapped:0};}
  if(depthUnsupported)return this.fallback(depthUnsupported+'; raw HDR retained');
  const signature=mapped.map(({source:o,material:m})=>JSON.stringify([o.uuid,o.geometry.uuid,m.uuid,m.version,m.normalMap.uuid,m.normalMap.version,m.normalMap.channel,m.normalMapType,m.normalScale.toArray(),m.side,m.flatShading]));
  try{
   if(!this.signature||!same(signature,this.signature)){
    this.clearBindings();this.scene ||= new T.Scene();
    for(const{source,material:m}of mapped){const guide=new T.MeshNormalMaterial({normalMap:m.normalMap,normalMapType:m.normalMapType,normalScale:m.normalScale.clone(),side:m.side,flatShading:m.flatShading,depthTest:true,depthWrite:false,depthFunc:T.LessEqualDepth,blending:T.NoBlending,toneMapped:false});this.materials.push(guide);const proxy=new T.Mesh(source.geometry,guide);proxy.matrixAutoUpdate=false;proxy.layers.mask=source.layers.mask;proxy.renderOrder=source.renderOrder;proxy.frustumCulled=source.frustumCulled;this.scene.add(proxy);this.bindings.push({source,proxy,geometry:source.geometry,material:m});}this.signature=signature;
   }
   if(!this.target||this.target.width!==width||this.target.height!==height){this.releaseTarget();this.target=new T.WebGLRenderTarget(width,height,{minFilter:T.NearestFilter,magFilter:T.NearestFilter,depthBuffer:true,stencilBuffer:false,samples:0});this.target.texture.colorSpace=T.NoColorSpace;this.ownedDepth=new T.DepthTexture(width,height,T.UnsignedIntType);this.target.depthTexture=this.ownedDepth;this.bytes=pixels*8;}
   this.depthMaterial ||= new T.MeshDepthMaterial({side:T.DoubleSide,colorWrite:false,depthTest:true,depthWrite:true,depthFunc:T.LessEqualDepth,toneMapped:false});
   for(const b of this.bindings){if(b.source.geometry!==b.geometry||b.source.material!==b.material||!isCurrent())return this.fallback('Shading guide source binding changed; raw HDR retained');b.proxy.matrix.copy(b.source.matrixWorld);b.proxy.visible=hierarchyVisible(b.source)&&b.material.visible===true;}
   const saved={target:renderer.getRenderTarget(),override:scene.overrideMaterial,background:scene.background,autoClear:renderer.autoClear,color:renderer.getClearColor(new T.Color()),alpha:renderer.getClearAlpha(),clearColor:renderer.autoClearColor,clearDepth:renderer.autoClearDepth,clearStencil:renderer.autoClearStencil};
   try{
    // This target owns its depth. Clear alpha0 once, populate occlusion from
    // the full captured scene, then shade only the generated mapped surfaces.
    renderer.setClearColor(0x000000,0);renderer.autoClear=true;renderer.autoClearColor=true;renderer.autoClearDepth=true;renderer.autoClearStencil=false;scene.background=null;scene.overrideMaterial=this.depthMaterial;renderer.setRenderTarget(this.target);renderer.render(scene,camera);
    scene.overrideMaterial=saved.override;scene.background=saved.background;renderer.autoClear=false;renderer.render(this.scene,camera);this.passes=2;
   }finally{scene.overrideMaterial=saved.override;scene.background=saved.background;renderer.autoClear=saved.autoClear;renderer.autoClearColor=saved.clearColor;renderer.autoClearDepth=saved.clearDepth;renderer.autoClearStencil=saved.clearStencil;renderer.setClearColor(saved.color,saved.alpha);renderer.setRenderTarget(saved.target);}
   return{supported:true,texture:this.target.texture,bytes:this.bytes,passes:2,mapped:mapped.length};
  }catch(error){return this.fallback('Shading guide unavailable: '+String(error.message||error)+'; raw HDR retained');}
 }
}
