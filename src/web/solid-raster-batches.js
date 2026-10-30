// Optional raster representation. Owns derived buffers/instance textures and stage clones;
// never owns original model geometry, materials, environment or source nodes.
const limits={nodes:60000,depth:128,meshes:32768,groups:64,vertices:1000000,indices:3000000,bytes:64*1024*1024,geometryVertices:65535,geometryIndices:200000};
function hierarchyVisible(node){for(let p=node;p;p=p.parent)if(!p.visible)return false;return true;}
function sameElements(a,b){return a.length===b.length&&a.every((v,i)=>v===b[i]);}
export function classifySourceMesh(T,o){
 if(o.type!=='Mesh'||o.isSkinnedMesh||o.isInstancedMesh||o.isBatchedMesh||o.geometry?.isInstancedBufferGeometry)return 'special-mesh';
 const g=o.geometry,m=o.material;if(Array.isArray(m)||!m?.isMeshStandardMaterial)return 'unknown-material';
 if(['onBeforeRender','onAfterRender','onBeforeShadow','onAfterShadow'].some(k=>o[k]!==T.Object3D.prototype[k])||o.customDepthMaterial||o.customDistanceMaterial)return 'custom-hooks';
 if(m.onBeforeCompile!==T.Material.prototype.onBeforeCompile||m.onBeforeRender!==T.Material.prototype.onBeforeRender||m.customProgramCacheKey!==T.Material.prototype.customProgramCacheKey||Object.keys(m.defines||{}).some(k=>!['STANDARD','PHYSICAL'].includes(k))||Object.keys(m.userData?.gltfExtensions||{}).length)return 'custom-material';
 if(m.alphaTest!==0||m.alphaHash||m.alphaToCoverage||(m.transmission||0)>0||m.blending!==T.NormalBlending||m.polygonOffset||m.stencilWrite||m.clippingPlanes?.length)return 'unsupported-material-feature';
 if(g.morphAttributes&&Object.keys(g.morphAttributes).length||o.morphTargetInfluences||g.groups.length||g.drawRange.start!==0||g.drawRange.count!==Infinity)return 'custom-geometry';
 if(!g.attributes.position||!g.attributes.normal||Object.keys(g.attributes).some(k=>!['position','normal','tangent','uv','uv1','uv2','uv3','color'].includes(k)))return 'unknown-attributes';
 if(Object.values(g.attributes).some(a=>!a.array||a.isInterleavedBufferAttribute||a.isGLBufferAttribute||a.isInstancedBufferAttribute||a.gpuType!==T.FloatType||a.usage!==T.StaticDrawUsage))return 'unsupported-attribute-storage';
 if(!Number.isInteger(g.attributes.position.count)||g.attributes.position.count<1||g.attributes.position.count>limits.geometryVertices||(g.index?.count||0)>limits.geometryIndices)return 'single-geometry-budget';
 const e=o.matrixWorld.elements;if(e.some(n=>!Number.isFinite(n))||o.matrixWorld.determinant()<=0)return 'singular-or-reflected-transform';
 const a=new T.Vector3().setFromMatrixColumn(o.matrixWorld,0),b=new T.Vector3().setFromMatrixColumn(o.matrixWorld,1),c=new T.Vector3().setFromMatrixColumn(o.matrixWorld,2);if(Math.abs(a.dot(b))>a.length()*b.length()*1e-7||Math.abs(a.dot(c))>a.length()*c.length()*1e-7||Math.abs(b.dot(c))>b.length()*c.length()*1e-7)return 'shear-transform';
 if(m.transparent)return 'transparent-fallback';
 if(m.opacity!==1||!m.depthWrite||!m.depthTest)return 'unsupported-opaque-state';
 return null;
}
function layout(o){return JSON.stringify([o.material.uuid,!!o.geometry.index,Object.entries(o.geometry.attributes).sort(([a],[b])=>a.localeCompare(b)).map(([k,a])=>[k,a.itemSize,a.normalized,a.array.constructor.name]),o.layers.mask,o.renderOrder,o.castShadow,o.receiveShadow,o.frustumCulled]);}
export async function createRasterBatches({THREE:T,renderer,scene,model,gltf,signal,isCurrent=()=>true,yieldControl,onProgress=()=>{}}){
 let multiDraw=false;try{multiDraw=!!renderer.getContext().getExtension('WEBGL_multi_draw');}catch{}
 // Capability is verified before any derived scene, geometry or texture exists.
 if(!multiDraw)return {supported:false,multiDraw:false,reason:'Multi-draw unavailable; original raster retained.',scene,dispose(){},sync(){return false;}};
 let channel=null,waiting=[],chunkStart=performance.now(),dispose=()=>closeYield();
 const abortError=()=>new DOMException('Raster representation superseded','AbortError');
 const current=()=>!signal?.aborted&&isCurrent();
 const closeYield=()=>{channel?.port1.close();channel?.port2.close();channel=null;for(const item of waiting.splice(0))item.reject(abortError());};
 const taskYield=()=>{if(yieldControl)return yieldControl();if(!channel){channel=new MessageChannel();channel.port1.onmessage=()=>waiting.shift()?.resolve();}return new Promise((resolve,reject)=>{waiting.push({resolve,reject});channel.port2.postMessage(0);});};
 const onAbort=()=>dispose();signal?.addEventListener('abort',onAbort,{once:true});
 const start=performance.now(),stats={limits,sourceMeshes:0,sourceTriangles:0,batches:0,transparentFallbacks:0,uniqueGeometries:0,vertices:0,indices:0,estimatedBytes:0,ownedBytes:0,maxGeometryVertices:0,maxGeometryIndices:0,buildMs:0,maxChunkMs:0,yields:0};
 const fail=reason=>{signal?.removeEventListener('abort',onAbort);closeYield();return {supported:false,multiDraw,reason,stats,scene,dispose(){},sync(){return false;}};};
 const pause=async()=>{if(!current())throw abortError();stats.maxChunkMs=Math.max(stats.maxChunkMs,performance.now()-chunkStart);stats.yields++;await taskYield();if(!current())throw abortError();chunkStart=performance.now();};
 if(!current())return fail('Raster representation superseded.');
 if(gltf?.animations?.length||!T.BatchedMesh)return fail('Animated source or BatchedMesh unavailable; retain original scene.');
 const groups=new Map(),transparent=[],stack=[[model,0]];let visited=0,estimate=0;
 try{while(stack.length){const [o,depth]=stack.pop();if(depth>limits.depth||++visited>limits.nodes||stack.length+o.children.length>limits.nodes)return fail('Source traversal/depth budget exceeded.');for(let i=o.children.length-1;i>=0;i--)stack.push([o.children[i],depth+1]);if(visited%256===0)await pause();
  if(!o.isMesh){if((!o.isGroup&&!o.isScene&&o.type!=='Object3D')||o.renderOrder!==0)return fail('Unsupported original hierarchy feature: '+o.type+' '+o.name+' renderOrder='+o.renderOrder);continue;}
  if(++stats.sourceMeshes>limits.meshes)return fail('Source mesh budget exceeded.');stats.sourceTriangles+=(o.geometry.index?.count??o.geometry.attributes.position.count)/3;
  const reason=classifySourceMesh(T,o);if(reason==='transparent-fallback'){transparent.push(o);continue;}if(reason)return fail(reason+'; retain original scene.');
  const key=layout(o);if(!groups.has(key)){if(groups.size>=limits.groups)return fail('Compatible group budget exceeded.');groups.set(key,{meshes:[],geometries:new Map(),vertices:0,indices:0});}
  const group=groups.get(key);group.meshes.push(o);if(!group.geometries.has(o.geometry)){const g=o.geometry;group.geometries.set(g,null);group.vertices+=g.attributes.position.count;group.indices+=g.index?.count||0;stats.vertices+=g.attributes.position.count;stats.indices+=g.index?.count||0;stats.uniqueGeometries++;stats.maxGeometryVertices=Math.max(stats.maxGeometryVertices,g.attributes.position.count);stats.maxGeometryIndices=Math.max(stats.maxGeometryIndices,g.index?.count||0);for(const a of Object.values(g.attributes))estimate+=a.array.byteLength;estimate+=(g.index?.count||0)*4;}
  if(stats.vertices>limits.vertices||stats.indices>limits.indices)return fail('Owned geometry capacity budget exceeded.');
 }
 }catch(error){return fail(String(error.message||error));}
 stats.estimatedBytes=estimate+stats.sourceMeshes*256+groups.size*4096;if(stats.estimatedBytes>limits.bytes)return fail('Owned representation byte budget exceeded.');
 // Only known stage light types and their target nodes can be represented.
 const originalLights=scene.children.filter(o=>o.isLight),targets=new Set(originalLights.map(l=>l.target).filter(Boolean));
 for(const child of scene.children)if(child!==model&&!originalLights.includes(child)&&!targets.has(child)&&!(child.isGroup&&child.userData.illustrative))return fail('Unknown non-model scene child.');
 if(originalLights.some(l=>!l.isDirectionalLight&&!l.isRectAreaLight&&!l.isAmbientLight&&!l.isHemisphereLight))return fail('Unsupported stage light.');
 const derived=new T.Scene(),sourceGroup=new T.Group(),batches=[],bindings=[],geometryRecords=[],fallback=[],lights=[],extras=[];derived.add(sourceGroup);let disposed=false;
 dispose=()=>{if(disposed)return;disposed=true;signal?.removeEventListener('abort',onAbort);closeYield();for(const b of batches)b.dispose();for(const [,light]of lights)light.shadow?.dispose();derived.clear();};
 const abort=reason=>{dispose();return fail(reason);};
 try{
  for(const group of groups.values()){
   const example=group.meshes[0],batch=new T.BatchedMesh(group.meshes.length,group.vertices,group.indices,example.material);batch.name='Compatible opaque raster batch';batch.castShadow=example.castShadow;batch.receiveShadow=example.receiveShadow;batch.layers.mask=example.layers.mask;batch.renderOrder=example.renderOrder;batch.frustumCulled=false;batch.perObjectFrustumCulled=example.frustumCulled;batch.sortObjects=false;batches.push(batch);sourceGroup.add(batch);
   let chunk=performance.now(),n=0;for(const geometry of group.geometries.keys()){const geometryId=batch.addGeometry(geometry);group.geometries.set(geometry,geometryId);geometryRecords.push({source:geometry,batch,geometryId});if(++n%128===0){stats.maxChunkMs=Math.max(stats.maxChunkMs,performance.now()-chunk);onProgress({...stats});await pause();chunk=performance.now();}}
   for(const source of group.meshes){const instance=batch.addInstance(group.geometries.get(source.geometry));batch.setMatrixAt(instance,source.matrixWorld);batch.setVisibleAt(instance,hierarchyVisible(source));bindings.push({source,batch,instance,matrix:source.matrixWorld.elements.slice(),visible:hierarchyVisible(source),material:source.material});if(++n%256===0){stats.maxChunkMs=Math.max(stats.maxChunkMs,performance.now()-chunk);await pause();chunk=performance.now();}}
   stats.maxChunkMs=Math.max(stats.maxChunkMs,performance.now()-chunk);
   for(const a of Object.values(batch.geometry.attributes))stats.ownedBytes+=a.array.byteLength;stats.ownedBytes+=batch.geometry.index?.array.byteLength||0;for(const t of [batch._matricesTexture,batch._indirectTexture,batch._colorsTexture])stats.ownedBytes+=t?.image.data.byteLength||0;if(stats.ownedBytes>limits.bytes)return abort('Actual owned buffer budget exceeded.');
  }
  const cloneMesh=source=>{const clone=new T.Mesh(source.geometry,source.material);clone.name=source.name;clone.matrixAutoUpdate=false;clone.matrix.copy(source.matrixWorld);clone.visible=hierarchyVisible(source);clone.layers.mask=source.layers.mask;clone.renderOrder=source.renderOrder;clone.castShadow=source.castShadow;clone.receiveShadow=source.receiveShadow;clone.frustumCulled=source.frustumCulled;return clone;};
  for(const source of transparent){const clone=cloneMesh(source);sourceGroup.add(clone);fallback.push([source,clone,source.material]);}
  for(const source of originalLights){const clone=source.clone();clone.matrixAutoUpdate=false;clone.matrix.copy(source.matrixWorld);derived.add(clone);if(clone.target){clone.target.matrixAutoUpdate=false;clone.target.matrix.copy(source.target.matrixWorld);derived.add(clone.target);}lights.push([source,clone]);}
  for(const child of scene.children.filter(o=>o.isGroup&&o.userData.illustrative)){const group=new T.Group();derived.add(group);const items=[];child.traverse(o=>{if(!o.isMesh)return;if(classifySourceMesh(T,o))throw Error('Unsupported illustrative fallback geometry.');const clone=cloneMesh(o);group.add(clone);items.push([o,clone,o.material]);});extras.push([child,group,items]);}
  stats.batches=batches.length;stats.transparentFallbacks=fallback.length;stats.buildMs=performance.now()-start;
  const sync=()=>{
   if(disposed||scene.overrideMaterial)return false;sourceGroup.visible=model.visible;
   for(const b of bindings){if(b.source.material!==b.material||!sameElements(b.source.matrixWorld.elements,b.matrix))return false;const visible=hierarchyVisible(b.source);if(visible!==b.visible){b.batch.setVisibleAt(b.instance,visible);b.visible=visible;}}
   for(const [source,clone,material]of fallback){if(source.material!==material)return false;clone.matrix.copy(source.matrixWorld);clone.visible=hierarchyVisible(source);}
   for(const [source,clone]of lights){clone.visible=source.visible;clone.matrix.copy(source.matrixWorld);clone.color.copy(source.color);clone.intensity=source.intensity;if(clone.target)clone.target.matrix.copy(source.target.matrixWorld);}
   for(const [source,group,items]of extras){group.visible=source.visible;for(const [o,clone,material]of items){if(o.material!==material)return false;clone.matrix.copy(o.matrixWorld);clone.visible=hierarchyVisible(o);}}
   derived.background=scene.background;derived.environment=scene.environment;derived.fog=scene.fog;derived.backgroundIntensity=scene.backgroundIntensity;derived.backgroundBlurriness=scene.backgroundBlurriness;derived.environmentIntensity=scene.environmentIntensity;derived.backgroundRotation.copy(scene.backgroundRotation);derived.environmentRotation.copy(scene.environmentRotation);return true;
  };
  if(!current()||!sync())return abort('Source changed during asynchronous representation build.');
  closeYield();
  const compile=async(renderer,camera,{rasterKey,areaKey}={})=>{for(const [source,light]of lights){if(source===rasterKey)light.visible=true;if(source===areaKey)light.visible=false;}await renderer.compileAsync(derived,camera);};
  return {supported:true,scene:derived,sourceScene:scene,model,stats,batches,bindings,geometryRecords,sourceGroup,fallback,sync,compile,dispose};
 }catch(error){return abort(String(error.message||error));}
}
