import {ShadingNormalGuide,qualifyCoatingTraceLayers} from './solid-shading-guide.js';
import {deriveMaskRelief,createMaskReliefScene} from './solid-mask-relief.js';
import {applyMaterialPolicy,disposeMaterialPolicy} from './solid-material-policy.js';
import {buildSolderPreview} from './solid-solder.js';
import {selectSolderSource} from './solid-solder-source.js';
import {HDRResolve} from './solid-hdr-resolve.js';
import {createRasterBatches} from './solid-raster-batches.js';
// Optional original-GLB renderer. Dependencies are pinned local assets; the
// compact Solid wire remains the immediate preview and camera authority.
const LIMIT_BYTES=96*1024*1024,LIMIT_TRIS=1000000;
const abortError=()=>new DOMException('Render source superseded','AbortError');
const raf=globalThis.requestAnimationFrame?.bind(globalThis)||((f)=>setTimeout(()=>f(performance.now()),16));
const caf=globalThis.cancelAnimationFrame?.bind(globalThis)||clearTimeout;
const frame=()=>new Promise(raf);
const page=globalThis.document;
export async function createPathTraceView({canvas,vendorUrl='/viewer/vendor/viewer.js',workerUrl='/viewer/vendor/bvh-worker.js',onStatus=()=>{},maxSamples=32,maxMs=30000,maxPixels=2000000,materialPolicy='cad-studio'}={}) {
  const vendor=await import(vendorUrl);
  return new OriginalModelView({...vendor,canvas,workerUrl,onStatus,maxSamples,maxMs,maxPixels,materialPolicy});
}
class OriginalModelView {
  constructor({THREE,GLTFLoader,WebGLPathTracer,PhysicalCamera,SceneBVHWorker,RectAreaLightUniformsLib,FullScreenQuad,versions,canvas,workerUrl,onStatus,maxSamples,maxMs,maxPixels,materialPolicy}) {
    Object.assign(this,{T:THREE,GLTFLoader,WebGLPathTracer,PhysicalCamera,SceneBVHWorker,canvas,workerUrl,onStatus,versions,maxMs,maxPixels,materialPolicy});
    this.FullScreenQuad=FullScreenQuad;this.maxSamples=Math.max(1,Math.min(256,maxSamples));this.mode='raster';this.interactive=false;this.generation=0;this.traceGeneration=0;this.disposed=false;this.ready=false;this.denoiseEnabled=true;this.solderEnabled=false;
    this.state={phase:'initializing',visible:false,firstFrame:false,samples:0,sourceMeshes:0,materialSource:'original-glb',materialPolicies:['realistic','cad-studio','source-defaults'],versions};
    this.releaseRasterBatches();
    const context=canvas.getContext('webgl2',{antialias:true,alpha:false,powerPreference:'high-performance'});
    if(!context)throw Error('Original-model rendering needs WebGL 2; preview remains available.');
    const debug=context.getExtension('WEBGL_debug_renderer_info');this.gpuIdentity={renderer:context.getParameter(context.RENDERER),vendor:context.getParameter(context.VENDOR),unmaskedRenderer:debug?context.getParameter(debug.UNMASKED_RENDERER_WEBGL):null,unmaskedVendor:debug?context.getParameter(debug.UNMASKED_VENDOR_WEBGL):null};
    this.renderer=new THREE.WebGLRenderer({canvas,context,antialias:true,alpha:false});
    this.renderer.outputColorSpace=THREE.SRGBColorSpace;this.renderer.toneMapping=THREE.ACESFilmicToneMapping;
    this.renderer.setPixelRatio(1);this.renderer.setClearColor(0x202128,1);
    RectAreaLightUniformsLib.init();
    this.floatTargets=!!context.getExtension('EXT_color_buffer_float');
    this.softwareGPU=/SwiftShader|llvmpipe|softpipe|software rasterizer/i.test(this.gpuIdentity.unmaskedRenderer||'');this.supported=this.floatTargets&&!this.softwareGPU;
    this.pathTraceReason=!this.floatTargets?'Float render targets unavailable; Studio remains available':this.softwareGPU?'Path tracing disabled on this software renderer to keep Studio responsive':'';
    Object.assign(this.state,{pathTraceAvailable:this.supported,pathTraceReason:this.pathTraceReason});
    this.camera=new PhysicalCamera(40,1,0.00001,10);this.camera.up.set(0,0,1);this.camera.bokehSize=0;
    this.scene=new THREE.Scene();this.scene.background=new THREE.Color(0x202128);
    this.resize=globalThis.ResizeObserver?new ResizeObserver(()=>{if(this.ready)this.refresh();}):{observe(){},disconnect(){}};this.resize.observe(canvas);
    this.visibility=()=>{if(page?.hidden){this.stop();this.report('paused',{reason:'Page hidden'});}else if(this.ready)this.refresh();};
    page?.addEventListener('visibilitychange',this.visibility);
    this.lost=e=>{e.preventDefault();this.releaseMaskRelief('WebGL context lost');this.releaseRasterBatches('WebGL context lost');this.releaseTrace();this.stop();this.report('unsupported',{reason:'WebGL context lost; preview remains available.'});};canvas.addEventListener('webglcontextlost',this.lost);
    this.report('raster',{reason:this.pathTraceReason||'Original GLB ready to load'});
  }
  report(phase,extra={}) {const previous=this.state.phase;this.state={...this.state,phase,samples:this.tracer?.samples||0,...extra};const now=performance.now();if(extra.firstFrame||previous!==phase||!this.reported||now-this.reported>=100){this.reported=now;this.onStatus({...this.state});}}
  status(){return {...this.state};}
  stop(){if(this.hidden||page?.hidden||this.disposed){this.releaseMaskRelief('Viewer hidden or closed');this.releaseRasterBatches('Viewer hidden or closed');if(this.tracer||this.worker||this.buildPromise)this.releaseTrace();}if(this.raf)caf(this.raf);this.raf=0;if(this.settle)clearTimeout(this.settle);this.settle=0;if(this.gpuFence){this.renderer.getContext().deleteSync(this.gpuFence.sync);this.gpuFence=null;}this.finalizing=false;}
  releaseMaskRelief(reason=''){
    const relief=this.maskRelief,representation=this.maskReliefRepresentation,pending=this.maskReliefPromise;this.traceFallback=null;
    this.maskRelief=null;this.maskReliefRepresentation=null;this.maskReliefPromise=null;
    this.maskReliefEpoch=(this.maskReliefEpoch||0)+1;this.maskReliefAbort?.abort();this.maskReliefAbort=null;
    // Detach handles before releaseTrace→stop can re-enter while hidden.
    if(relief||pending){this.releaseRasterBatches('Mask presentation retired');if(relief&&this.traceMaskOwner===relief)this.releaseTrace();this.releaseGuides();}
    representation?.dispose();relief?.dispose();
    Object.assign(this.state,{maskReliefState:'idle',maskReliefReason:reason,maskReliefBytes:0,maskReliefMaps:0,maskReliefEstimated:false,maskReliefStats:null,coatingTraceTextureLayers:0,coatingTraceTextureBytes:0,tracePresentation:'none',tracePresentationReason:''});this.shadowsDirty=true;
  }
  recordTraceFallback(reason){
    this.traceFallback={reason,generation:this.generation,maskEpoch:this.maskReliefEpoch,proof:this.maskSourceProof};
    Object.assign(this.state,{tracePresentation:'original-source',tracePresentationReason:reason});
  }
  requestMaskRelief(){
    if(!this.ready||this.disposed||this.hidden||page?.hidden||this.mode==='pathtrace'||this.materialPolicy!=='realistic'||this.abort?.signal.aborted||!this.state.visible||!this.state.firstFrame||this.maskRelief||this.maskReliefPromise||this.state.maskReliefState==='fallback')return;
    if(!this.maskSourceProof){Object.assign(this.state,{maskReliefState:'fallback',maskReliefReason:'No verified same-origin PCB digest/orientation proof; original mask retained'});return;}
    const epoch=this.maskReliefEpoch,generation=this.generation,gltf=this.gltf,model=this.model,scene=this.scene,proof=this.maskSourceProof,controller=new AbortController();this.maskReliefAbort=controller;
    const current=()=>!controller.signal.aborted&&!this.abort?.signal.aborted&&!this.disposed&&!this.hidden&&!page?.hidden&&this.ready&&this.mode!=='pathtrace'&&this.materialPolicy==='realistic'&&this.state.visible&&this.state.firstFrame&&epoch===this.maskReliefEpoch&&generation===this.generation&&gltf===this.gltf&&model===this.model&&scene===this.scene&&proof===this.maskSourceProof;
    Object.assign(this.state,{maskReliefState:'building',maskReliefReason:''});
    const build=async()=>{let relief,representation;
      try{
        // Deferred task, never in the first source framebuffer/RAF proof.
        await new Promise(resolve=>setTimeout(resolve,0));if(!current())throw abortError();
        relief=await deriveMaskRelief({THREE:this.T,gltf,proof,boardFromWorld:model.matrixWorld.clone().invert(),signal:controller.signal,isCurrent:current});
        if(!current())throw abortError();representation=await createMaskReliefScene({THREE:this.T,scene,model,relief,signal:controller.signal,isCurrent:current});
        if(!current())throw abortError();this.releaseRasterBatches('Atomic estimated mask presentation');this.releaseTrace();this.releaseGuides();
        this.maskRelief=relief;this.maskReliefRepresentation=representation;
        Object.assign(this.state,{maskReliefState:'ready',maskReliefReason:'Estimated coating normals from measured original outer copper; trace uses only a captured ready representation',maskReliefEstimated:true,maskReliefBytes:relief.stats.ownedBytes,maskReliefMaps:relief.bindings.length,maskReliefStats:relief.stats});this.shadowsDirty=true;this.raster();
      }catch(error){const active=current();if(this.maskRelief===relief&&relief){this.releaseRasterBatches('Mask presentation draw failed');this.maskReliefRepresentation=null;this.maskRelief=null;}representation?.dispose();relief?.dispose();if(active)Object.assign(this.state,{maskReliefState:'fallback',maskReliefReason:String(error.message||error),maskReliefBytes:0,maskReliefMaps:0,maskReliefEstimated:false,maskReliefStats:null});}
      finally{if(this.maskReliefPromise===promise)this.maskReliefPromise=null;if(current()){this.reported=0;this.report(this.state.phase);this.requestRasterBatches();}}
    };
    const promise=build();this.maskReliefPromise=promise;return promise;
  }
  releaseRasterBatches(reason=''){
    // Abort owns synchronous disposal, even while compileAsync is pending.
    // Borrowed source materials/geometry can only be released after this call.
    this.rasterEpoch=(this.rasterEpoch||0)+1;this.rasterBatchAbort?.abort();this.rasterBatchAbort=null;
    this.rasterBatches?.dispose();this.rasterBatches=null;this.rasterBatchPromise=null;this.shadowsDirty=true;
    Object.assign(this.state,{rasterBatchState:'idle',rasterBatchReason:reason,rasterBatchSourceMeshes:0,rasterBatchGroups:0,rasterBatchTransparentMeshes:0,rasterBatchBytes:0,rasterBatchBuildMs:null,rasterBatchEpoch:this.rasterEpoch,rasterBatchMultiDraw:false});
  }
  requestRasterBatches(){
    if(!this.ready||this.disposed||this.hidden||page?.hidden||this.mode==='pathtrace'||!this.state.visible||!this.state.firstFrame||!(this.state.sourceMeshes>=128)||this.maskReliefPromise||this.rasterBatchPromise||this.rasterBatches||this.state.rasterBatchState==='fallback')return;
    const epoch=this.rasterEpoch,generation=this.generation,model=this.maskReliefRepresentation?.model||this.model,gltf=this.gltf,scene=this.maskReliefRepresentation?.scene||this.scene,controller=new AbortController();this.rasterBatchAbort=controller;
    const current=()=>!controller.signal.aborted&&!this.abort?.signal.aborted&&!this.disposed&&!this.hidden&&!page?.hidden&&this.ready&&epoch===this.rasterEpoch&&generation===this.generation&&model===(this.maskReliefRepresentation?.model||this.model)&&gltf===this.gltf&&scene===(this.maskReliefRepresentation?.scene||this.scene);
    Object.assign(this.state,{rasterBatchState:'building',rasterBatchReason:''});this.reported=0;this.report(this.state.phase);
    const started=performance.now();
    const build=async()=>{let representation;
      try{
        representation=await createRasterBatches({THREE:this.T,renderer:this.renderer,scene,model,gltf,signal:controller.signal,isCurrent:current});
        if(!current()){representation.dispose();return;}
        if(!representation.supported){Object.assign(this.state,{rasterBatchState:'fallback',rasterBatchReason:representation.reason,rasterBatchMultiDraw:!!representation.multiDraw});return;}
        await representation.compile(this.renderer,this.camera,{rasterKey:this.maskReliefRepresentation?.lightFor(this.rasterKey)||this.rasterKey,areaKey:this.maskReliefRepresentation?.lightFor(this.areaKey)||this.areaKey});
        if(!current()){representation.dispose();return;}
        this.rasterBatches=representation;const stats=representation.stats;
        Object.assign(this.state,{rasterBatchState:'ready',rasterBatchReason:'',rasterBatchSourceMeshes:stats.sourceMeshes,rasterBatchGroups:stats.batches,rasterBatchTransparentMeshes:stats.transparentFallbacks,rasterBatchBytes:stats.ownedBytes,rasterBatchBuildMs:Math.max(0,performance.now()-started),rasterBatchMultiDraw:true});
        this.shadowsDirty=true;this.raster();
      }catch(error){representation?.dispose();if(this.rasterBatches===representation)this.rasterBatches=null;if(current())Object.assign(this.state,{rasterBatchState:'fallback',rasterBatchReason:String(error.message||error),rasterBatchSourceMeshes:0,rasterBatchGroups:0,rasterBatchTransparentMeshes:0,rasterBatchBytes:0,rasterBatchBuildMs:null});}
      finally{if(current()){this.reported=0;this.report(this.state.phase);}}
    };
    const promise=build();this.rasterBatchPromise=promise;promise.finally(()=>{if(this.rasterBatchPromise===promise)this.rasterBatchPromise=null;});return promise;
  }
  renderRasterScene(){
    // Selection, anchors, material roles, guides and ray acceleration always
    // traverse this.scene. The derived scene is only used for a raster draw.
    if(this.maskReliefRepresentation&&!this.maskReliefRepresentation.sync())this.releaseMaskRelief('Source presentation changed');
    const scene=this.maskReliefRepresentation?.scene||this.scene,model=this.maskReliefRepresentation?.model||this.model,representation=this.rasterBatches;
    if(this.state.visible&&this.state.firstFrame&&representation){
      if(representation.model===model&&representation.sync()){
        this.renderer.render(representation.scene,this.camera);this.state.rasterBatchState='active';return;
      }
      this.releaseRasterBatches('Original source changed; original raster retained');this.state.rasterBatchState='fallback';this.shadowsDirty=true;this.renderer.shadowMap.needsUpdate=true;
    }
    this.renderer.render(scene,this.camera);
  }
  releaseScene(){
    this.releaseMaskRelief();this.maskSourceProof=null;this.releaseRasterBatches();this.stop();this.releaseTrace();this.solderPreview?.dispose();this.solderPreview=null;this.solderEnabled=false;
    if(this.gltf)disposeMaterialPolicy(this.gltf);
    const geometries=new Set(),materials=new Set(),textures=new Set();
    this.scene?.traverse(o=>{if(o.geometry)geometries.add(o.geometry);for(const m of Array.isArray(o.material)?o.material:o.material?[o.material]:[]){materials.add(m);for(const v of Object.values(m))if(v?.isTexture)textures.add(v);}});
    for(const g of geometries)g.dispose();for(const m of materials)m.dispose();for(const t of textures)t.dispose();
    this.rasterKey?.shadow.dispose();this.rasterKey=null;this.areaKey=null;
    this.environment?.dispose();this.environment=null;this.releaseGuides();this.rawQuad?.material.dispose();this.rawQuad?.dispose();this.rawQuad=null;this.scene?.clear();this.gltf=null;this.ready=false;
  }
  releaseGuides(){
    this.rawDenoiseFrameShown=false;this.shadingNormalGuide?.dispose();this.shadingNormalGuide=null;Object.assign(this.state,{shadingGuideBytes:0,shadingGuidePasses:0,shadingGuideMappedSurfaces:0,denoiseGuideReason:''});
    this.hdrResolve?.dispose();this.hdrResolve=null;
    this.state.resolvePending=false;this.state.resolvePendingReason='';
    this.guides?.dispose();this.guides=null;this.albedoGuide?.dispose();this.albedoGuide=null;for(const target of this.filterTargets||[])target.dispose();this.filterTargets=null;for(const m of this.albedoMaterials?.values()||[])m.dispose();this.albedoMaterials=null;this.denoiseQuad?.material.dispose();this.denoiseQuad?.dispose();this.denoiseQuad=null;this.normalMaterial?.dispose();this.normalMaterial=null;
  }
  releaseTrace(){
    const tracer=this.tracer,worker=this.worker;this.traceMaskOwner=null;this.traceScene=null;this.traceFallback=null;Object.assign(this.state,{tracePresentation:'none',tracePresentationReason:''});this.tracer=null;this.worker=null;this.buildPromise=null;this.built=false;this.traceGeneration++;
    this.stop();worker?.dispose();tracer?.dispose();tracer?._lowResPathTracer?.dispose();tracer?._generator?.geometry?.dispose();
  }
  async load({url,key,revision,source,sha256,orientation='gltf-y-up',signal}={}) {
    if(this.disposed)throw Error('Renderer disposed');
    this.sourceAbortCleanup?.();this.sourceAbortCleanup=null;const generation=++this.generation;this.abort?.abort();this.abort=new AbortController();this.abort.signal.addEventListener('abort',()=>{this.releaseMaskRelief('Source load aborted');this.releaseTrace();},{once:true});
    const controller=this.abort,abort=()=>controller.abort(),cleanupAbort=()=>signal?.removeEventListener('abort',abort);signal?.addEventListener('abort',abort,{once:true});this.sourceAbortCleanup=cleanupAbort;if(signal?.aborted)controller.abort();
    this.releaseScene();this.built=false;this.asset={key,revision,source,sha256,orientation};this.state={...this.state,key,revision,source,sha256,orientation,visible:false,firstFrame:false,triangles:0,sourceMeshes:0,samples:0,illustrativeSolder:false,solderJoints:0};this.report('loading');
    const current=()=>!this.disposed&&generation===this.generation&&!this.abort.signal.aborted;
    let timeout;
    try {
      const parsed=new URL(url,location.href);if(parsed.origin!==location.origin)throw Error('Model asset must be served by this authenticated hub.');
      const response=await fetch(parsed.href,{credentials:'same-origin',signal:this.abort.signal});
      if(!response.ok)throw Error(`Original model request failed (${response.status})`);
      const length=Number(response.headers.get('content-length'));if(length>LIMIT_BYTES)throw Error('Original model exceeds 96 MiB; preview remains available.');
      const chunks=[];let total=0;const reader=response.body.getReader();
      while(true){const {value,done}=await reader.read();if(done)break;total+=value.length;if(total>LIMIT_BYTES){await reader.cancel();throw Error('Original model exceeds 96 MiB; preview remains available.');}chunks.push(value);}
      if(!current())throw abortError();const bytes=new Uint8Array(total);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
      let verifiedDigest=null;if(sha256){const digest=await crypto.subtle.digest('SHA-256',bytes),actual=[...new Uint8Array(digest)].map(x=>x.toString(16).padStart(2,'0')).join('');if(actual!==sha256.toLowerCase())throw Error('Original model content hash does not match its saved revision.');if(/^[a-f0-9]{64}$/i.test(sha256))verifiedDigest=actual;}
      // Self-contained GLB only. Never turn a saved texture URI into an
      // arbitrary network request or a filesystem asset read.
      const manager=new this.T.LoadingManager();manager.setURLModifier(resource=>{if(resource.startsWith('blob:')||resource.startsWith('data:'))return resource;throw Error('External GLB resources are unavailable in this viewer.');});
      const loader=new this.GLTFLoader(manager);this.report('loading',{bytes:total,reason:'Parsing original model'});await frame();
      const gltf=await loader.parseAsync(bytes.buffer,'');if(!current())throw abortError();this.gltf=gltf;this.maskSourceProof=verifiedDigest&&key&&revision&&typeof source==='string'&&/\.kicad_pcb$/i.test(source)&&orientation==='gltf-y-up'?Object.freeze({sameOrigin:true,digestVerified:true,sha256:verifiedDigest,key,revision,source,orientation}):null;Object.assign(this.state,applyMaterialPolicy(gltf,this.materialPolicy,this.T));
      let triangles=0,vertices=0,authoredNormals=0,sourceMeshes=0;const materialSet=new Set();
      gltf.scene.traverse(o=>{if(o.isMesh){sourceMeshes++;const p=o.geometry.attributes.position;vertices+=p.count;triangles+=(o.geometry.index?.count||p.count)/3;if(o.geometry.attributes.normal)authoredNormals+=p.count;for(const m of Array.isArray(o.material)?o.material:[o.material])materialSet.add(m);}});
      if(triangles>LIMIT_TRIS)throw Error('Original model exceeds one million triangles; preview remains available.');
      this.scene=new this.T.Scene();this.scene.background=new this.T.Color(0x202128);
      // KiCad's original GLB is X right, Y up, Z board-Y, in metres. This
      // proper rotation is the same world used by Solid's (x,-y,z) wire.
      if(orientation==='gltf-y-up')gltf.scene.rotation.x=Math.PI/2;
      else if(orientation==='step-z-up-mm')gltf.scene.scale.multiplyScalar(0.001);
      else throw Error('Unsupported original-model orientation');
      this.scene.add(gltf.scene);this.model=gltf.scene;this.mapComponents();
      this.environment=this.studio();this.scene.environment=this.environment;
      const box=new this.T.Box3().setFromObject(gltf.scene),center=box.getCenter(new this.T.Vector3()),size=box.getSize(new this.T.Vector3());this.radius=Math.max(size.length()/2,0.0001);
      this.addLights(center,this.radius);this.bounds=box;
      this.addRasterShadows(center,this.radius);
      if(!this.cameraState&&triangles>0)this.fitDefaultCamera(box);
      else if(this.cameraState)this.applyCamera(this.cameraState);
      this.ready=true;this.report('raster',{triangles,vertices,authoredNormals,sourceMeshes,materials:materialSet.size,bytes:total,reason:'Original GLB materials and normals'});this.raster();await frame();
      if(this.mode==='pathtrace')await this.ensureTrace();return this.status();
    } catch(error) {
      clearTimeout(timeout);if(!current()||error.name==='AbortError')return undefined;
      this.worker?.dispose();this.worker=null;this.built=false;this.report(this.ready?'raster':'error',{reason:String(error.message||error)});return this.status();
    } finally {if(!current()){cleanupAbort();if(this.sourceAbortCleanup===cleanupAbort)this.sourceAbortCleanup=null;}}
  }
  ensureTrace(){
    if(this.buildPromise)return this.buildPromise;
    if(!this.ready||!this.supported||this.built||this.disposed||!this.state.triangles)return Promise.resolve(this.status());
    if(this.maskReliefRepresentation&&!this.maskReliefRepresentation.sync())this.releaseMaskRelief('Unsynchronized mask presentation; original trace retained');
    if(this.maskRelief){const layers=qualifyCoatingTraceLayers(this.maskReliefRepresentation.scene);if(!layers.supported){this.releaseMaskRelief(layers.reason);this.recordTraceFallback(layers.reason+'; original trace retained');}else Object.assign(this.state,{coatingTraceTextureLayers:layers.layers,coatingTraceTextureBytes:layers.bytes});}
    const representation=this.maskReliefRepresentation||null,owner=this.maskRelief||null,scene=representation?.scene||this.scene,proof=this.maskSourceProof,maskEpoch=this.maskReliefEpoch;
    const generation=this.generation,traceGeneration=this.traceGeneration,signal=this.abort.signal,current=()=>!this.disposed&&!this.hidden&&!page?.hidden&&generation===this.generation&&traceGeneration===this.traceGeneration&&!signal.aborted&&owner===(this.maskRelief||null)&&representation===(this.maskReliefRepresentation||null)&&(!owner||maskEpoch===this.maskReliefEpoch)&&proof===this.maskSourceProof;
    const fallback=this.traceFallback,currentFallback=fallback&&fallback.generation===generation&&fallback.maskEpoch===maskEpoch&&fallback.proof===proof?fallback.reason:'Original source trace; no current ready eligible coating representation';
    this.traceScene=scene;this.traceMaskOwner=owner;Object.assign(this.state,{tracePresentation:owner?'estimated-mask-relief':'original-source',tracePresentationReason:owner?'Captured ready estimated coating representation; source geometry/picking unchanged':currentFallback});
    const build=async()=>{
      let timeout,onAbort;
      try{
        this.report('building',{progress:0,reason:'Building ray acceleration in a worker'});
        this.worker=new this.SceneBVHWorker(this.workerUrl);this.tracer=new this.WebGLPathTracer(this.renderer);this.tracer.setBVHWorker(this.worker);
        this.tracer.bounces=5;this.tracer.transmissiveBounces=4;this.tracer.renderDelay=0;this.tracer.fadeDuration=0;this.tracer.minSamples=1;this.tracer.dynamicLowRes=false;this.tracer.renderToCanvas=false;this.tracer.textureSize.set(1024,1024);this.tracer.filterGlossyFactor=0.5;
        this.updateTraceScale();
        const built=this.tracer.setSceneAsync(scene,this.camera,{onProgress:progress=>{if(current()&&this.mode==='pathtrace')this.report('building',{progress});}});
        await Promise.race([built,new Promise((_,reject)=>{timeout=setTimeout(()=>reject(Error('Ray acceleration build timed out; original-model raster remains available.')),30000);onAbort=()=>reject(abortError());signal.addEventListener('abort',onAbort,{once:true});})]);
        if(!current())return;this.built=true;this.tracer.setCamera(this.camera);this.report('raster',{progress:1,reason:'Ray acceleration ready'});this.refresh();return this.status();
      }catch(error){
        if(!current())return;this.worker?.dispose();this.worker=null;this.tracer?.dispose();this.tracer=null;this.built=false;this.report('raster',{reason:String(error.message||error)});return this.status();
      }finally{clearTimeout(timeout);signal.removeEventListener('abort',onAbort);}
    };
    const promise=build();this.buildPromise=promise;promise.finally(()=>{if(this.buildPromise===promise)this.buildPromise=null;});return promise;
  }
  fitDefaultCamera(box){
    const T=this.T,center=box.getCenter(new T.Vector3()),direction=new T.Vector3(-0.5,-0.73,0.48).normalize(),forward=direction.clone().negate(),right=new T.Vector3().crossVectors(forward,new T.Vector3(0,0,1)).normalize(),up=new T.Vector3().crossVectors(right,forward).normalize();
    const aspect=Math.max(1,this.canvas.clientWidth)/Math.max(1,this.canvas.clientHeight),tan=Math.tan(this.camera.fov*Math.PI/360);let distance=0;
    for(const x of [box.min.x,box.max.x])for(const y of [box.min.y,box.max.y])for(const z of [box.min.z,box.max.z]){
      const d=new T.Vector3(x,y,z).sub(center),towardEye=d.dot(direction);
      distance=Math.max(distance,towardEye+Math.abs(d.dot(right))/(tan*aspect)*1.1,towardEye+Math.abs(d.dot(up))/tan*1.1);
    }
    this.camera.position.copy(center).addScaledVector(direction,distance);this.camera.up.copy(up);this.camera.lookAt(center);this.camera.aspect=aspect;this.camera.near=this.radius/1000;this.camera.far=distance+this.radius*5;this.camera.updateProjectionMatrix();
  }
  studio(){
    const T=this.T,width=512,height=256,data=new Float32Array(width*height*4);
    // Large neutral softboxes, a narrow rim and dark negative fill. The
    // environment is Z-up like the actual board; no CAD materials are tinted.
    const panels=[{d:new T.Vector3(-0.6,-0.45,0.7).normalize(),spread:.07,power:2.0},{d:new T.Vector3(.65,.1,.75).normalize(),spread:.18,power:.3},{d:new T.Vector3(-.1,.9,.35).normalize(),spread:.018,power:3.0},{d:new T.Vector3(0,0,-1),spread:.12,power:.45}];
    for(let y=0;y<height;y++)for(let x=0;x<width;x++){
      const theta=(x/width-.5)*Math.PI*2,phi=(1-y/height)*Math.PI,d=new T.Vector3(Math.sin(phi)*Math.cos(theta),Math.cos(phi),Math.sin(phi)*Math.sin(theta));
      let v=.015+.025*Math.max(d.z,0);for(const p of panels)v+=p.power*Math.exp(-(1-d.dot(p.d))/p.spread);
      const i=(y*width+x)*4;data[i]=v;data[i+1]=v;data[i+2]=v;data[i+3]=1;
    }
    const texture=new T.DataTexture(data,width,height,T.RGBAFormat,T.FloatType);texture.mapping=T.EquirectangularReflectionMapping;texture.minFilter=T.LinearFilter;texture.magFilter=T.LinearFilter;texture.needsUpdate=true;return texture;
  }
  addLights(center,radius){
    const T=this.T;
    for(const [position,intensity,width,height]of [[[-.9,-.65,1.3],2.0,1.3,1.3],[[1.1,.2,.9],.4,1.2,1.2],[[-.2,1.3,.6],3.0,1.8,.25]]){
      const light=new T.RectAreaLight(0xffffff,intensity,radius*width,radius*height);light.position.copy(center).add(new T.Vector3(...position).multiplyScalar(radius));light.lookAt(center);this.scene.add(light);if(!this.areaKey)this.areaKey=light;
    }
    this.state.lighting='neutral-z-up-softkey-fill-rim-lower-diffuser';
  }
  addRasterShadows(center,radius){
    const T=this.T,key=new T.DirectionalLight(0xffffff,1);key.position.copy(center).add(new T.Vector3(-.9,-.65,1.3).multiplyScalar(radius));key.target.position.copy(center);key.visible=false;key.castShadow=true;
    const camera=key.shadow.camera;Object.assign(camera,{left:-radius*1.1,right:radius*1.1,top:radius*1.1,bottom:-radius*1.1,near:radius*.01,far:radius*5});camera.updateProjectionMatrix();key.shadow.mapSize.set(1024,1024);key.shadow.normalBias=.000005;key.shadow.bias=-.0001;
    this.renderer.shadowMap.enabled=true;this.renderer.shadowMap.type=T.PCFSoftShadowMap;this.renderer.shadowMap.autoUpdate=false;this.renderer.shadowMap.needsUpdate=true;
    this.model.traverse(o=>{if(!o.isMesh)return;const i=this.gltf.parser.associations.get(o)?.meshes,name=this.gltf.parser.json.meshes?.[i]?.name||'',board=/_(soldermask|silkscreen|pad|PCB|copper|via)$/.test(name);o.castShadow=!board;o.receiveShadow=board;});
    this.scene.add(key,key.target);this.rasterKey=key;this.shadowsDirty=true;Object.assign(this.state,{rasterLighting:'directional-key-contact-shadows',shadowMapSize:1024,shadowNormalBiasMM:.005});
  }
  applyCamera(state){
    const cam=state.cam;if(!cam)return;
    const w=Math.max(1,this.canvas.clientWidth),h=Math.max(1,this.canvas.clientHeight),aspect=w/h,half=cam.dist*0.00001*Math.tan(cam.fov/2);
    if(state.projection==='orthographic') {if(!this.camera.isOrthographicCamera)this.camera=new this.T.OrthographicCamera();Object.assign(this.camera,{left:-half*aspect,right:half*aspect,top:half,bottom:-half});}
    else {if(!this.camera.isPerspectiveCamera)this.camera=new this.PhysicalCamera();this.camera.fov=cam.fov*180/Math.PI;this.camera.aspect=aspect;this.camera.filmGauge=24;this.camera.bokehSize=state.aperture&&state.aperture!=='off'?this.camera.getFocalLength()/Number(state.aperture):0;this.camera.focusDistance=(state.focusDistance||cam.dist/100)/1000;}
    const target=cam.pivot.map((v,i)=>(v+cam.r[i]*cam.sx+cam.u[i]*cam.sy)*0.00001),eye=target.map((v,i)=>v-cam.f[i]*cam.dist*0.00001);
    this.camera.position.set(...eye);this.camera.up.set(...cam.u);this.camera.lookAt(new this.T.Vector3(...target));this.camera.near=Math.max(cam.dist*1e-7,(this.radius||0.01)/1000);this.camera.far=cam.dist*1e-5+(this.radius||0.01)*5+Math.hypot(cam.sx,cam.sy)*1e-5;this.camera.updateProjectionMatrix();this.camera.updateMatrixWorld();
    this.renderer.toneMappingExposure=2**(Number(state.exposure)||0);
  }
  setCamera(state){this.cameraState=structuredClone(state);this.applyCamera(state);if(this.tracer&&this.built)this.tracer.setCamera(this.camera);if(this.ready)this.refresh();}
  resizeToCanvas(){
    const w=Math.max(1,this.canvas.clientWidth),h=Math.max(1,this.canvas.clientHeight),dpr=Math.min(globalThis.devicePixelRatio||this.pixelRatio||1,2),css=w*h;
    // Native CSS resolution by default at normal desktop sizes. DPR growth is
    // bounded; an explicit lower budget remains authoritative and observable.
    const scale=Math.min(dpr,Math.sqrt(this.maxPixels/css)),rw=Math.max(1,Math.round(w*scale)),rh=Math.max(1,Math.round(h*scale));
    this.renderer.setSize(rw,rh,false);Object.assign(this.state,{cssWidth:w,cssHeight:h,renderWidth:rw,renderHeight:rh,pixelRatioActual:scale,upscaled:scale<.999,textureSlot:1024});
    this.updateTraceScale();
    if(this.cameraState)this.applyCamera(this.cameraState);else if(this.camera.isPerspectiveCamera){this.camera.aspect=w/h;this.camera.updateProjectionMatrix();}
  }
  updateTraceScale(){
    if(!this.tracer)return;
    const scale=Math.max(1,Math.min(1.5,Math.sqrt(this.maxPixels/(this.canvas.width*this.canvas.height))));this.tracer.renderScale=scale;
    const width=Math.floor(this.canvas.width*scale),height=Math.floor(this.canvas.height*scale),tiles=Math.max(3,Math.min(6,Math.ceil(Math.sqrt(width*height/65536))));this.tracer.tiles.set(tiles,tiles);
    Object.assign(this.state,{traceScale:scale,traceWidth:width,traceHeight:height,traceTiles:tiles,edgeSampling:scale>1?'pixel-area-resolved':'native'});
  }
  raster(){if(!this.ready)return;this.resizeToCanvas();this.rasterKey.visible=true;this.areaKey.visible=false;try{
    // Compare against this renderer's actual background, including its color
    // management. A clear color constant or a selection box cannot prove that
    // source geometry reached the framebuffer.
    let baseline;
    const prove=!this.state.visible&&!this.provingFrame&&this.state.triangles>0&&this.model?.visible;
    const gl=this.renderer.getContext(),w=this.canvas.width,h=this.canvas.height;
    if(prove){baseline=new Uint8Array(w*h*4);this.model.visible=false;try{this.renderRasterScene();gl.readPixels(0,0,w,h,gl.RGBA,gl.UNSIGNED_BYTE,baseline);}finally{this.model.visible=true;}}
    if(this.shadowsDirty||prove)this.renderer.shadowMap.needsUpdate=true;this.renderRasterScene();this.shadowsDirty=false;
    if(baseline){
      const pixels=new Uint8Array(w*h*4);gl.readPixels(0,0,w,h,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
      let geometry=false;for(let i=0;i<pixels.length;i+=4){if(Math.abs(pixels[i]-baseline[i])+Math.abs(pixels[i+1]-baseline[i+1])+Math.abs(pixels[i+2]-baseline[i+2])>12){geometry=true;break;}}
      if(geometry&&gl.getError()===gl.NO_ERROR){const generation=this.generation;this.provingFrame=true;raf(()=>{this.provingFrame=false;if(this.ready&&!this.disposed&&generation===this.generation){this.report(this.state.phase,{visible:true,firstFrame:true});this.requestMaskRelief();this.requestRasterBatches();}});}
    }
    this.drawSelection();}finally{this.rasterKey.visible=false;this.areaKey.visible=true;}
    this.requestMaskRelief();this.requestRasterBatches();
  }
  setInteractive(on){this.interactive=!!on;this.stop();if(this.ready){this.raster();this.report('raster',{reason:on?'Camera moving':'Camera settled',denoised:false});}if(!on)this.refresh();}
  setMode(mode,{samples=this.maxSamples}={}){this.mode=mode==='pathtrace'?'pathtrace':'raster';this.maxSamples=Math.max(1,Math.min(256,Number(samples)||32));if(this.mode==='pathtrace'){const ready=this.maskRelief&&this.maskReliefRepresentation?.sourceModel===this.model&&this.maskReliefRepresentation.sync();if(!ready){this.releaseMaskRelief('No current ready coating representation; original trace retained');this.recordTraceFallback('No current ready coating representation; original trace retained');}else this.traceFallback=null;}if(this.mode==='pathtrace'&&!this.supported){this.mode='raster';this.refresh();this.reported=0;this.report('raster',{reason:this.pathTraceReason,pathTraceAvailable:false,pathTraceReason:this.pathTraceReason});return this.status();}if(this.mode==='pathtrace'&&(this.rasterBatchPromise||this.rasterBatches))this.releaseRasterBatches('Path tracing uses captured unbatched geometry');if(this.mode==='raster')this.report('raster',{reason:'Original GLB studio raster'});this.refresh();return this.status();}
  refresh(){
    this.stop();if(!this.ready||this.disposed||this.hidden||page?.hidden)return;this.state.denoised=false;this.raster();
    if(this.mode!=='pathtrace'||this.interactive)return;
    if(!this.built){this.ensureTrace();return;}
    this.tracer.updateCamera();this.started=performance.now();this.settle=setTimeout(()=>this.sample(),120);
  }
  sample(){
    if(this.disposed||this.interactive||this.hidden||page?.hidden||this.mode!=='pathtrace'||!this.built)return;
    const gl=this.renderer.getContext();
    if(this.gpuFence){
      const fence=this.gpuFence,result=gl.clientWaitSync(fence.sync,0,0);
      if(result===gl.TIMEOUT_EXPIRED){if(performance.now()-fence.started>10000){this.stop();this.report('paused',{reason:'GPU tile completion timed out; refinement stopped',denoised:false});return;}this.raf=raf(()=>this.sample());return;}
      gl.deleteSync(fence.sync);this.gpuFence=null;if(result===gl.WAIT_FAILED){this.stop();this.report('paused',{reason:'GPU synchronization unavailable; refinement stopped',denoised:false});return;}
      const ms=performance.now()-fence.started;Object.assign(this.state,{gpuTileMs:ms,maxGpuTileMs:Math.max(this.state.maxGpuTileMs||0,ms)});
    }
    if(this.finalizing==='present'){this.finalizing=false;this.report('converged',{reason:this.state.denoiseGuideReason||'',denoised:this.finalDenoised!==false});return;}
    if(this.finalizing==='samples'){const filtered=this.denoise()!==false;if(!filtered&&this.state.resolvePending){this.finalizing=false;this.report('paused',{reason:this.state.resolvePendingReason,denoised:false});return;}this.finalDenoised=filtered;this.drawSelection();this.finalizing='present';this.fenceTile();this.raf=raf(()=>this.sample());return;}
    if(performance.now()-this.started>this.maxMs){let denoised=this.tracer.samples>=1;if(denoised)denoised=this.denoise()!==false;this.drawSelection();this.report('paused',{reason:this.state.denoiseGuideReason||(this.state.resolvePending?this.state.resolvePendingReason:'Time budget reached; partial path-traced image retained'),denoised});return;}
    try{this.tracer.renderSample();if(this.tracer.samples>=1&&Number.isInteger(this.tracer.samples)){const resolved=this.resolveHDR();if(resolved){this.drawHDR(resolved.texture);this.drawSelection();}}this.report('pathtrace',{reason:this.state.resolvePending?this.state.resolvePendingReason:'',denoised:false});if(this.tracer.samples>=this.maxSamples)this.finalizing='samples';this.fenceTile();}
    catch(error){this.raster();this.report('error',{reason:`Path tracing unavailable: ${error.message}`});return;}
    this.raf=raf(()=>this.sample());
  }
  fenceTile(){const gl=this.renderer.getContext();if(!gl.fenceSync)return;const sync=gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE,0);if(!sync)throw Error('GPU synchronization unavailable');gl.flush();this.gpuFence={sync,started:performance.now()};}
  resolveHDR(){
    this.hdrResolve ||= new HDRResolve(this.T,this.FullScreenQuad);
    try{const target=this.hdrResolve.render(this.renderer,this.tracer.target,this.canvas.width,this.canvas.height);if(this.state.reason===this.state.resolvePendingReason)this.state.reason='';this.state.resolvePending=false;this.state.resolvePendingReason='';return target;}
    catch(error){if(error.code!=='HDR_RESOLVE_PENDING')throw error;this.state.resolvePending=true;this.state.resolvePendingReason=error.message;return null;}
  }
  drawHDR(texture){
    if(!this.rawQuad){const material=new this.T.ShaderMaterial({depthTest:false,depthWrite:false,uniforms:{map:{value:null}},vertexShader:'varying vec2 vUv;void main(){vUv=uv;gl_Position=vec4(position.xy,0.0,1.0);}',fragmentShader:`varying vec2 vUv;uniform sampler2D map;void main(){gl_FragColor=texture2D(map,vUv);
#include <tonemapping_fragment>
#include <colorspace_fragment>
}`});this.rawQuad=new this.FullScreenQuad(material);}
    this.rawQuad.material.uniforms.map.value=texture;this.rawQuad.render(this.renderer);
  }
  denoise(){
    this.rawDenoiseFrameShown=false;
    const T=this.T,target=this.tracer.target,w=target.width,h=target.height,scene=this.traceScene||this.scene;
    const resolved=this.resolveHDR();if(!resolved)return false;
    if(!this.guides||this.guides.width!==w||this.guides.height!==h){this.shadingNormalGuide?.dispose();this.shadingNormalGuide=null;this.guides?.dispose();this.guides=new T.WebGLRenderTarget(w,h,{minFilter:T.NearestFilter,magFilter:T.NearestFilter});this.guides.depthTexture=new T.DepthTexture(w,h,T.UnsignedIntType);}
    if(!this.albedoGuide||this.albedoGuide.width!==w||this.albedoGuide.height!==h){this.albedoGuide?.dispose();this.albedoGuide=new T.WebGLRenderTarget(w,h,{minFilter:T.NearestFilter,magFilter:T.NearestFilter});}
    this.normalMaterial ||= new T.MeshNormalMaterial({side:T.DoubleSide});
    const previous=this.renderer.getRenderTarget(),override=scene.overrideMaterial,originals=[];
    try{
      scene.overrideMaterial=this.normalMaterial;this.renderer.setRenderTarget(this.guides);this.renderer.render(scene,this.camera);scene.overrideMaterial=override;
      this.albedoMaterials ||= new Map();
      scene.traverse(o=>{if(!o.isMesh)return;originals.push([o,o.material]);const basic=m=>{let b=this.albedoMaterials.get(m);if(!b){b=new T.MeshBasicMaterial({color:m.color,map:m.map,opacity:m.opacity,alphaTest:m.alphaTest,transparent:m.transparent,side:m.side,vertexColors:m.vertexColors,toneMapped:false});this.albedoMaterials.set(m,b);}return b;};o.material=Array.isArray(o.material)?o.material.map(basic):basic(o.material);});
      this.renderer.setRenderTarget(this.albedoGuide);this.renderer.render(scene,this.camera);
    }finally{for(const [o,m]of originals)o.material=m;scene.overrideMaterial=override;this.renderer.setRenderTarget(previous);}
    this.shadingNormalGuide ||= new ShadingNormalGuide(T);
    const owner=this.traceMaskOwner,traceGeneration=this.traceGeneration,guide=this.shadingNormalGuide.render({renderer:this.renderer,scene,camera:this.camera,width:w,height:h,eligibleBindings:owner?.bindings||[],isCurrent:()=>!this.disposed&&traceGeneration===this.traceGeneration&&scene===(this.traceScene||this.scene)&&owner===this.traceMaskOwner});
    if(guide.supported&&this.state.reason===this.state.denoiseGuideReason)this.state.reason='';
    Object.assign(this.state,{shadingGuideBytes:guide.bytes,shadingGuidePasses:guide.passes,shadingGuideMappedSurfaces:guide.mapped,denoiseGuideReason:guide.supported?'':guide.reason});
    if(!guide.supported){this.drawHDR(resolved.texture);this.rawDenoiseFrameShown=true;this.state.denoised=false;this.state.denoisePasses=0;return false;}
    const mixed=this.hdrResolve.protectMixed(this.renderer,this.guides.texture,this.guides.depthTexture,this.albedoGuide.texture,this.camera.projectionMatrixInverse);
    if(!this.denoiseQuad){
      const material=new T.ShaderMaterial({depthTest:false,depthWrite:false,toneMapped:false,uniforms:{map:{value:null},normalMap:{value:null},shadingMap:{value:null},shadingEnabled:{value:0},depthMap:{value:null},albedoMap:{value:null},mixedMap:{value:null},size:{value:new T.Vector2()},outputSize:{value:new T.Vector2()},inverseProjection:{value:new T.Matrix4()},stride:{value:1}},
       vertexShader:'varying vec2 vUv;void main(){vUv=uv;gl_Position=vec4(position.xy,0.0,1.0);}',
       fragmentShader:`varying vec2 vUv;uniform sampler2D map;uniform sampler2D normalMap;uniform sampler2D shadingMap;uniform float shadingEnabled;uniform sampler2D depthMap;uniform sampler2D albedoMap;uniform sampler2D mixedMap;uniform vec2 size;uniform vec2 outputSize;uniform mat4 inverseProjection;uniform float stride;
       vec2 guideUV(vec2 p){return (floor(p*size)+0.5)/size;}
       vec3 shadingAt(vec2 p){vec4 s=texture2D(shadingMap,p);return shadingEnabled>0.5&&s.a>0.5?normalize(s.xyz*2.0-1.0):normalize(texture2D(normalMap,p).xyz*2.0-1.0);}
       vec3 viewAt(vec2 p){vec4 v=inverseProjection*vec4(p*2.0-1.0,texture2D(depthMap,p).r*2.0-1.0,1.0);return v.xyz/v.w;}
       void main(){vec2 uv=guideUV(vUv);vec4 center=texture2D(map,vUv);if(texture2D(mixedMap,vUv).r>0.5){gl_FragColor=center;return;}vec3 geometricNormal=normalize(texture2D(normalMap,uv).xyz*2.0-1.0),normal=shadingAt(uv),point=viewAt(uv);vec3 albedo=texture2D(albedoMap,uv).rgb;vec4 sum=vec4(center.rgb*center.a,center.a);float weight=1.0;
       for(int x=-3;x<=3;x++)for(int y=-3;y<=3;y++){if(x==0&&y==0)continue;vec2 p=vUv+vec2(float(x),float(y))*stride/outputSize;if(any(lessThan(p,vec2(0.0)))||any(greaterThanEqual(p,vec2(1.0)))||texture2D(mixedMap,p).r>0.5)continue;vec2 q=guideUV(p);vec4 tap=texture2D(map,p);vec3 n=shadingAt(q);float planeError=abs(dot(viewAt(q)-point,geometricNormal));vec3 da=texture2D(albedoMap,q).rgb-albedo;float w=exp(-float(x*x+y*y)/4.5)*pow(max(dot(n,normal),0.0),48.0)*exp(-planeError/max(length(point)*0.00001,0.000002))*exp(-dot(da,da)/0.003);sum+=vec4(tap.rgb*tap.a,tap.a)*w;weight+=w;}
       vec4 filtered=mix(vec4(center.rgb*center.a,center.a),sum/weight,0.95);gl_FragColor=vec4(filtered.a>0.0?filtered.rgb/filtered.a:vec3(0.0),filtered.a);
       }`});
      this.denoiseQuad=new this.FullScreenQuad(material);
    }
    const cw=this.canvas.width,ch=this.canvas.height;
    if(!this.filterTargets||this.filterTargets[0].width!==cw||this.filterTargets[0].height!==ch){for(const t of this.filterTargets||[])t.dispose();this.filterTargets=[0,1].map(()=>new T.WebGLRenderTarget(cw,ch,{type:T.HalfFloatType,minFilter:T.LinearFilter,magFilter:T.LinearFilter,depthBuffer:false}));}
    const u=this.denoiseQuad.material.uniforms;u.map.value=resolved.texture;u.normalMap.value=this.guides.texture;u.shadingMap.value=guide.texture||this.guides.texture;u.shadingEnabled.value=guide.mapped?1:0;u.depthMap.value=this.guides.depthTexture;u.albedoMap.value=this.albedoGuide.texture;u.mixedMap.value=mixed.texture;u.size.value.set(w,h);u.outputSize.value.set(cw,ch);u.inverseProjection.value.copy(this.camera.projectionMatrixInverse);
    try{for(let pass=0;pass<3;pass++){u.stride.value=2**pass;this.renderer.setRenderTarget(this.filterTargets[pass%2]);this.denoiseQuad.render(this.renderer);u.map.value=this.filterTargets[pass%2].texture;}}finally{this.renderer.setRenderTarget(previous);}
    this.drawHDR(this.filterTargets[0].texture);
    this.state.denoisePasses=3;
    return true;
  }
  present(denoised=true){
    this.denoiseEnabled=!!denoised;if(!this.tracer||this.tracer.samples<1)return this.status();
    let shown,filtered=false;if(denoised){filtered=this.denoise()!==false;shown=filtered||this.rawDenoiseFrameShown===true;}else {const resolved=this.resolveHDR();shown=!!resolved;if(resolved)this.drawHDR(resolved.texture);}
    if(shown)this.drawSelection();this.reported=0;this.report(this.state.phase,{...(this.state.denoiseGuideReason?{reason:this.state.denoiseGuideReason}:this.state.resolvePending?{reason:this.state.resolvePendingReason}:{}),denoised:filtered});return this.status();
  }
  setMaterialPolicy(policy){this.materialPolicy=['realistic','source-defaults'].includes(policy)?policy:'cad-studio';if(this.gltf){this.releaseMaskRelief('Material policy changed');this.releaseRasterBatches();this.releaseTrace();this.releaseGuides();this.shadowsDirty=true;Object.assign(this.state,applyMaterialPolicy(this.gltf,this.materialPolicy,this.T));this.reported=0;this.report('raster',{samples:0});this.refresh();}return this.status();}
  setComponents(data){this.components=data;this.mapComponents();if(this.solderEnabled)this.setSolderPreview(true,{rebuild:true});return {mapped:this.componentNodes?.size||0};}
  setSolderPreview(enabled,{rebuild=false}={}){
    if(enabled&&!this.ready)throw Error('Original source model is not ready for illustrative solder.');
    const on=!!enabled&&this.ready;
    if(on!==this.solderEnabled||rebuild){
      this.releaseMaskRelief('Illustrative solder representation changed');this.releaseRasterBatches();
      this.solderPreview?.dispose();this.solderPreview=null;this.solderEnabled=false;
      // Changing illustrative geometry invalidates both acceleration and the
      // accumulated image. An in-flight older build owns a different epoch.
      this.releaseTrace();this.releaseGuides();this.state.samples=0;
      if(on){
        try{
          const selected=selectSolderSource({gltf:this.gltf,components:this.components,asset:this.asset});
          if(!selected.view)throw Error(selected.reason);
          const preview=buildSolderPreview({THREE:this.T,model:selected.view,components:this.components,asset:this.asset,maxJoints:128});
          if(!preview.coverage.joints){const reason=preview.skipped[0]?.reason||'No actual pad and terminal anchors qualify for illustrative solder.';preview.dispose();throw Error(reason);}
          preview.coverage.sourceCandidates=selected.receipt;this.solderPreview=preview;this.solderEnabled=true;this.scene.add(preview.group);preview.group.visible=true;
        }catch(error){
          this.solderPreview?.dispose();this.solderPreview=null;this.solderEnabled=false;this.reported=0;
          this.report('raster',{illustrativeSolder:false,solderJoints:0,reason:String(error.message||error)});this.refresh();throw error;
        }
      }
      this.reported=0;this.report('raster',{illustrativeSolder:on,solderJoints:this.solderPreview?.coverage.joints||0,reason:on?'Illustrative solder: estimated toe shapes':'Original source geometry'});this.refresh();
    }
    return {enabled:this.solderEnabled,source:this.asset?.source,revision:this.asset?.revision,coverage:this.solderPreview?.coverage||{joints:0,components:0,limits:'Illustrative estimated toe shapes; no measured joint geometry.'},skipped:this.solderPreview?.skipped||[]};
  }
  mapComponents(){
    this.componentMeshes=new Map();this.componentNodes=new Map();
    if(!this.model||!this.components||!this.asset?.revision||(this.components.sourceRevision||this.components.revision)!==this.asset.revision||this.components.source!==this.asset.source)return;
    const refs=new Map();for(const c of this.components.components||[]){const list=refs.get(c.reference)||[];list.push(c);refs.set(c.reference,list);}
    const candidates=new Map();this.model.traverse(o=>{if(o.name&&!o.name.startsWith('=>')&&refs.get(o.name)?.length===1){const list=candidates.get(o.name)||[];list.push(o);candidates.set(o.name,list);}});
    for(const [ref,nodes]of candidates){if(nodes.length!==1)continue;const id=refs.get(ref)[0].id;this.componentNodes.set(id,nodes[0]);nodes[0].traverse(o=>{if(o.isMesh)this.componentMeshes.set(o,id);});}
  }
  componentBounds(id){
    const node=this.componentNodes?.get(id);if(!node)return null;
    const b=new this.T.Box3().setFromObject(node);if(b.isEmpty())return null;
    // Return saved-board axes/mm, not Three's rotated metres.
    return {id,min:[b.min.x*1000,-b.max.y*1000,b.min.z*1000],max:[b.max.x*1000,-b.min.y*1000,b.max.z*1000],source:this.asset.source,revision:this.asset.revision};
  }
  selectComponent(id){this.selected=id||null;if(this.ready){if(this.mode==='pathtrace'&&this.built&&this.tracer.samples>0)this.denoise();else this.raster();this.drawSelection();}return {id:this.selected,mapped:this.componentNodes?.has(id)||false};}
  pick(x,y){
    if(!this.ready||!this.componentMeshes?.size)return null;
    const ray=new this.T.Raycaster();ray.setFromCamera(new this.T.Vector2(x/Math.max(1,this.canvas.clientWidth)*2-1,1-y/Math.max(1,this.canvas.clientHeight)*2),this.camera);
    // Intersect every real mesh first: board/package occlusion cannot select
    // a hidden component behind the closest rendered surface.
    const hit=ray.intersectObject(this.model,true)[0];if(!hit)return null;return this.componentMeshes.get(hit.object)||null;
  }
  drawSelection(){
    const node=this.componentNodes?.get(this.selected);if(!node)return;
    const box=new this.T.Box3().setFromObject(node),helper=new this.T.Box3Helper(box,0xa9d365),overlay=new this.T.Scene();helper.material.depthTest=false;helper.material.transparent=true;helper.material.opacity=0.85;overlay.add(helper);
    const clear=this.renderer.autoClear;this.renderer.autoClear=false;this.renderer.render(overlay,this.camera);this.renderer.autoClear=clear;helper.geometry.dispose();helper.material.dispose();
  }
  diagnostics(){return {...this.status(),...this.gpuIdentity,renderCalls:this.renderer.info.render.calls,worker:true,bvhReady:!!this.built,bvhBuilding:!!this.buildPromise,traceAllocated:!!this.tracer,traceTargetWidth:this.tracer?.target?.width??null,traceTargetHeight:this.tracer?.target?.height??null,guideWidth:this.guides?.width??null,guideHeight:this.guides?.height??null,hdrResolve:this.hdrResolve?'pixel-area':null,resolveInputWidth:this.hdrResolve?.inputWidth??null,resolveInputHeight:this.hdrResolve?.inputHeight??null,resolveWidth:this.hdrResolve?.width??null,resolveHeight:this.hdrResolve?.height??null,mixedGuideProtection:!!this.hdrResolve?.mixedReady,resolveMaxTexelsPerPixel:this.hdrResolve?.maxTexelsPerPixel??null,resolveBytes:this.hdrResolve?.bytes||0,guideBytes:(this.shadingNormalGuide?.bytes||0)+(this.guides?this.guides.width*this.guides.height*16:0)+(this.filterTargets?.reduce((n,t)=>n+t.width*t.height*8,0)||0)+(this.hdrResolve?.bytes||0),normalVertices:this.state.authoredNormals};}
  resizeCanvas(width,height,pixelRatio){if(width!==this.canvas.clientWidth||height!==this.canvas.clientHeight||pixelRatio!==this.pixelRatio)this.releaseRasterBatches('Canvas size changed');this.canvas.clientWidth=width;this.canvas.clientHeight=height;this.pixelRatio=pixelRatio;if(this.ready)this.refresh();}
  dispose(){if(this.disposed)return;this.disposed=true;this.generation++;this.abort?.abort();this.sourceAbortCleanup?.();this.sourceAbortCleanup=null;this.releaseScene();this.resize.disconnect();page?.removeEventListener('visibilitychange',this.visibility);this.canvas.removeEventListener('webglcontextlost',this.lost);this.renderer.dispose();}
}
