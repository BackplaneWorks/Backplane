// Focused state regression over the exact production worker class, with a
// deterministic GPU readback stand-in. Actual ray pixels have their own test.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
const queued=[];let rasterFactory=()=>{throw Error('Unexpected batching before source proof');};
const createRasterBatches=args=>rasterFactory(args),applyMaterialPolicy=g=>{g.onPolicy?.();return {};},disposeMaterialPolicy=g=>g.onPolicyDispose?.();
const previews=[];let solderSourceFailure=null,solderZero=false;const selectSolderSource=()=>solderSourceFailure?{view:null,reason:solderSourceFailure}:{view:{},receipt:{meshes:1,triangles:1}};
const buildSolderPreview=()=>{const preview={group:{visible:false},coverage:{joints:solderZero?0:2,components:solderZero?0:1,limits:'Illustrative estimate'},skipped:solderZero?[{reason:'No exact source anchors.'}]:[],disposed:0,dispose(){this.disposed++;}};previews.push(preview);return preview;};
const source=readFileSync('src/web/solid-pathtrace-core.js','utf8').replace(/^import .*;$/gm,'').replace('export async function createPathTraceView','async function createPathTraceView');
const View=runInNewContext(source+'\nOriginalModelView;',{performance:{now:()=>1000},requestAnimationFrame:f=>queued.push(f),setTimeout,clearTimeout,console,DOMException,Uint8Array,Float32Array,Map,Set,Math,Number,JSON,structuredClone,buildSolderPreview,selectSolderSource,createRasterBatches,AbortController,applyMaterialPolicy,disposeMaterialPolicy});
function fake(geometry){
 const v=Object.create(View.prototype),events=[];
 Object.assign(v,{state:{phase:'raster',key:'source-key',revision:'source-rev',visible:false,samples:0,triangles:1},maxPixels:2000000,rasterKey:{visible:false,shadow:{dispose(){}}},areaKey:{visible:true},model:{visible:true},generation:1,ready:true,canvas:{width:2,height:2},onStatus:s=>events.push(s),resizeToCanvas(){},resolveHDR(){return {texture:{}};},drawHDR(){},drawSelection(){},scene:{},camera:{},renderer:{shadowMap:{},render(){},getContext(){return {RGBA:1,UNSIGNED_BYTE:1,NO_ERROR:0,getError:()=>0,readPixels(x,y,w,h,format,type,p){for(let i=0;i<p.length;i+=4)p.set([13,14,19,255],i);if(geometry&&v.model.visible)p[0]=200;}}}}});return {v,events};
}
{
 const {v,events}=fake(true);v.state.triangles=0;v.raster();assert.equal(queued.length,0);assert.equal(events.length,0,'zero triangles cannot reveal a background');
}
{
 const {v,events}=fake(false);v.raster();assert.equal(events.length,0);assert.equal(v.state.visible,false);
 assert.equal(queued.length,1,'a proof that finds no geometry retries on the next frame');
 let n=0;while(queued.length&&n<100){queued.shift()();n++;}
 assert.equal(n,30,'proof retries are bounded');assert.equal(events.length,0,'retries never reveal a frame without geometry');assert.equal(v.state.visible,false);
}
{
 const {v,events}=fake(true);v.raster();assert.equal(events.length,0);assert.equal(v.state.visible,false);queued.shift()();assert.equal(events[0].visible,true);assert.equal(events[0].firstFrame,true);assert.equal(events[0].key,'source-key');assert.equal(events[0].revision,'source-rev');
}
{
 const {v,events}=fake(true);v.raster();v.generation++;queued.shift()();assert.equal(events.length,0,'superseded frame cannot reveal a new source');
}
{
 const {v,events}=fake(true);let filtered=0;Object.assign(v,{mode:'pathtrace',built:true,maxMs:10,started:0,tracer:{samples:2.25},denoise(){filtered++}});v.sample();assert.equal(filtered,1);assert.equal(events[0].phase,'paused');assert.equal(events[0].samples,2.25);assert.equal(events[0].denoised,true);
}
{
 const {v,events}=fake(true);let refreshes=0;Object.assign(v,{supported:false,pathTraceReason:'Known software renderer',refresh(){refreshes++;}});v.setMode('pathtrace');assert.equal(v.mode,'raster');assert.equal(events.at(-1).phase,'raster');assert.equal(events.at(-1).pathTraceAvailable,false);assert.equal(refreshes,1);assert.equal(v.tracer,undefined,'blocked mode never constructs a tracer');
}
{
 const {v}=fake(true);let pending=true,submissions=0,fences=0,deletes=0;const gl={TIMEOUT_EXPIRED:1,WAIT_FAILED:3,SYNC_GPU_COMMANDS_COMPLETE:4,clientWaitSync(){return pending?1:2;},fenceSync(){fences++;return {};},flush(){},deleteSync(){deletes++;}};
 Object.assign(v,{mode:'pathtrace',built:true,maxMs:100000,started:1000,maxSamples:2,tracer:{samples:0,renderSample(){submissions++;this.samples++;}},renderer:{getContext:()=>gl},drawSelection(){}});
 v.sample();assert.equal(submissions,1);assert.equal(fences,1);v.sample();assert.equal(submissions,1,'unsignaled fence forbids another GPU tile');pending=false;v.sample();assert.equal(submissions,2);assert.equal(deletes,1);assert.equal(v.finalizing,'samples');v.stop();assert.equal(deletes,2);assert.equal(v.finalizing,false,'camera/cancel invalidates pending presentation');
}
{
 const {v}=fake(true);let draws=0;Object.assign(v,{mode:'pathtrace',built:true,maxMs:100000,started:1000,maxSamples:4,tracer:{samples:0,renderSample(){this.samples+=.5;}},drawHDR(){draws++;}});
 v.sample();assert.equal(draws,0,'partial tile pass retains the prior visible frame');v.sample();assert.equal(draws,1,'completed pass uses resolved HDR');v.sample();assert.equal(draws,1,'next partial pass cannot overdraw the retained image');v.stop();
}
{
 const {v,events}=fake(true);let draws=0;Object.assign(v,{hdrResolve:{render(){const e=Error('Waiting for current-size target');e.code='HDR_RESOLVE_PENDING';throw e;}},resolveHDR:View.prototype.resolveHDR,tracer:{samples:1,target:{width:1000,height:700}},drawHDR(){draws++;}});
 v.present(false);assert.equal(draws,0);assert.equal(v.state.resolvePending,true);assert.match(events.at(-1).reason,/current-size/);
 Object.assign(v,{mode:'pathtrace',built:true,maxMs:100000,started:1000,finalizing:'samples'});v.sample();assert.equal(v.state.phase,'paused');assert.equal(v.state.denoised,false);
 v.hdrResolve.render=()=>({texture:{}});v.present(false);assert.equal(draws,1);assert.equal(v.state.resolvePending,false);assert.equal(v.state.reason,'');
}
{
 const {v}=fake(true);let disposed=0;const resource=()=>({dispose(){disposed++;}});Object.assign(v,{hdrResolve:resource(),guides:resource(),albedoGuide:resource(),albedoMaterials:new Map([[{},resource()],[{},resource()]]),denoiseQuad:{material:resource(),dispose(){disposed++;}},normalMaterial:resource()});
 v.releaseGuides();assert.equal(disposed,8);assert.equal(v.albedoMaterials,null);assert.equal(v.hdrResolve,null);v.releaseGuides();assert.equal(disposed,8,'repeated policy cleanup cannot retain or redispose material-dependent caches');
}
{
 const {v}=fake(true);let tiles;Object.assign(v,{maxPixels:2000000,canvas:{width:1000,height:700},tracer:{tiles:{set(x,y){tiles=[x,y];}}}});v.updateTraceScale();assert.equal(v.state.traceScale,1.5);assert.equal(v.state.traceWidth,1500);assert.equal(v.state.traceHeight,1050);assert.deepEqual(tiles,[5,5]);assert(v.state.traceWidth*v.state.traceHeight<=v.maxPixels);
 v.maxPixels=700000;v.updateTraceScale();assert.equal(v.state.traceScale,1);assert.equal(v.state.edgeSampling,'native');
}
{
 const {v}=fake(true);let refreshes=0;Object.assign(v,{traceGeneration:0,asset:{source:'/board',revision:'save'},scene:{add(){}},refresh(){refreshes++}});
 const original=v.model,on=v.setSolderPreview(true);assert.equal(on.enabled,true);assert.equal(on.coverage.joints,2);assert.equal(v.state.triangles,1);assert.equal(v.model,original);assert.equal(v.solderPreview.group.visible,true);assert.equal(v.traceGeneration,1);
 v.setSolderPreview(true);assert.equal(previews.length,1,'same enabled state does not rebuild');
 v.setComponents({});assert.equal(previews[0].disposed,1);assert.equal(previews.length,2,'new metadata invalidates old illustrative geometry');
 const off=v.setSolderPreview(false);assert.equal(off.enabled,false);assert.equal(off.coverage.joints,0);assert.equal(previews[1].disposed,1);assert.equal(v.solderPreview,null);assert.equal(v.traceGeneration,3);assert.equal(refreshes,3);
}
{
 const {v,events}=fake(true);Object.assign(v,{traceGeneration:0,asset:{source:'/board',revision:'save'},scene:{add(){}},refresh(){}});v.setSolderPreview(true);const prior=v.solderPreview;solderSourceFailure='Missing original board-pad provenance';assert.throws(()=>v.setSolderPreview(true,{rebuild:true}),/pad provenance/);assert.equal(prior.disposed,1);assert.equal(v.solderPreview,null);assert.equal(v.solderEnabled,false);assert.equal(events.at(-1).illustrativeSolder,false);assert.equal(events.at(-1).solderJoints,0);solderSourceFailure=null;
 solderZero=true;assert.throws(()=>v.setSolderPreview(true),/No exact source anchors/);assert.equal(previews.at(-1).disposed,1);assert.equal(v.solderEnabled,false);solderZero=false;v.ready=false;assert.throws(()=>v.setSolderPreview(true),/not ready/);
}
{
 const {v,events}=fake(true),resolves=[];Object.assign(v,{traceGeneration:0,supported:true,abort:new AbortController(),workerUrl:'/worker',refresh(){},SceneBVHWorker:class{dispose(){}},WebGLPathTracer:class{constructor(){this.tiles={set(){}};this.textureSize={set(){}};}setBVHWorker(){}setSceneAsync(){return new Promise(r=>resolves.push(r));}setCamera(){}dispose(){}}});
 const old=v.ensureTrace();assert.equal(v.ensureTrace(),old,'concurrent requests share one BVH build');v.releaseTrace();const current=v.ensureTrace(),tracer=v.tracer;assert.equal(resolves.length,2);
 resolves[0]();await old;assert.equal(v.tracer,tracer,'superseded build cannot dispose replacement tracer');assert.equal(v.built,false,'old geometry cannot become current');
 resolves[1]();await current;assert.equal(v.built,true);assert.equal(events.filter(e=>e.reason==='Ray acceleration ready').length,1);v.releaseTrace();
}
console.log('ok source-frame proof, paused denoise, optional-solder lifecycle and superseded/shared BVH builds');

// Exact production class lifecycle: deferred raster work never establishes
// source visibility, never promotes another epoch, and releases borrowed refs
// before policy, illustration or original-source resources are disposed.
function batchFixture({compilePending=false}={}){
 const {v,events}=fake(true),records=[];
 Object.assign(v,{T:{},gltf:{},mode:'raster'});v.releaseRasterBatches();v.state.sourceMeshes=128;
 rasterFactory=args=>{
  let releaseCompile;const record={supported:true,disposed:0,compiled:false,scene:{derived:true},model:args.model,stats:{sourceMeshes:128,batches:1,transparentFallbacks:0,ownedBytes:256},sync(){return !this.disposed;},dispose(){if(!this.disposed)this.disposed++;},compile(){this.compiled=true;return compilePending?new Promise(r=>releaseCompile=r):Promise.resolve();},finish(){releaseCompile?.();}};
  args.signal.addEventListener('abort',()=>record.dispose(),{once:true});records.push(record);return Promise.resolve(record);
 };
 return {v,events,records};
}
{
 queued.length=0;const {v,records}=batchFixture();v.raster();assert.equal(records.length,0,'unproved source may never start batching');assert.equal(v.state.visible,false);
 queued.shift()();assert.equal(v.state.firstFrame,true);assert.equal(records.length,1);assert.equal(v.state.rasterBatchState,'building');const promise=v.rasterBatchPromise;await promise;
 assert.equal(v.state.rasterBatchState,'active');assert.equal(v.state.rasterBatchSourceMeshes,128);assert.equal(v.state.sourceMeshes,128);assert.equal(v.state.triangles,1);assert.equal(v.state.rasterBatchGroups,1);assert.equal(v.state.rasterBatchBytes,256);assert.equal(v.state.rasterBatchBuildMs,0);assert.equal(v.state.rasterBatchMultiDraw,true);
 const originalScene=v.scene;let drawn;v.renderer.render=scene=>drawn=scene;v.renderRasterScene();assert.equal(drawn,records[0].scene);assert.equal(v.scene,originalScene);
 v.state.visible=false;v.renderRasterScene();assert.equal(drawn,records[0].scene,'a retained exact batch is what the proof draws');v.releaseRasterBatches();assert.equal(records[0].disposed,1);
}
{
 const {v,records}=batchFixture();assert.equal(v.state.visible,false);const promise=v.requestRasterBatches(true);assert.equal(records.length,1,'load may batch a large source before its first proof');await promise;
 assert.equal(v.state.rasterBatchState,'active');let drawn;v.renderer.render=scene=>drawn=scene;v.renderRasterScene();assert.equal(drawn,records[0].scene);v.releaseRasterBatches();
}
for(const change of ['policy','solder','resize','hidden','load','dispose','pathtrace']){
 const {v,records}=batchFixture({compilePending:true});Object.assign(v.state,{visible:true,firstFrame:true});const pending=v.requestRasterBatches();await Promise.resolve();const prior=records[0];assert.equal(prior.compiled,true);const epoch=v.rasterEpoch;
 // Keep the transition independent from a new raster build; assert actual
 // lifecycle method ordering against the still-pending borrowed representation.
 v.refresh=()=>{};
 const assertReleased=()=>assert.equal(prior.disposed,1,change+' must release derived references first');
 if(change==='policy'){v.gltf.onPolicy=assertReleased;v.setMaterialPolicy('realistic');}
 if(change==='solder'){v.solderEnabled=true;v.solderPreview={dispose:assertReleased};v.setSolderPreview(false);}
 if(change==='resize')v.resizeCanvas(3,2,2);
 if(change==='hidden'){v.hidden=true;v.stop();}
 if(change==='load'||change==='dispose'){
  v.gltf.onPolicyDispose=assertReleased;v.scene={traverse(){assertReleased();},clear(){}};
  if(change==='dispose'){v.resize={disconnect(){}};v.canvas.removeEventListener=()=>{};v.renderer.dispose=assertReleased;v.dispose();}
  else {await v.load({url:'/next',key:'next-source',revision:'next-revision',source:'/next'});assert.equal(v.state.key,'next-source');assert.equal(v.state.visible,false);assert.equal(v.state.sourceMeshes,0);v.model={visible:true};v.gltf={};}
 }
 if(change==='pathtrace'){v.supported=true;v.setMode('pathtrace');}
 assertReleased();assert(v.rasterEpoch>epoch);assert.equal(v.state.rasterBatchBytes,0);assert.equal(v.state.rasterBatchBuildMs,null);assert.equal(v.state.rasterBatchGroups,0);assert.equal(v.state.rasterBatchSourceMeshes,0);assert.equal(v.state.rasterBatchMultiDraw,false);
 prior.finish();await pending;assert.equal(v.rasterBatches,null,'stale compile cannot become the new source');assert.equal(prior.disposed,1);
 if(change==='dispose'){assert.equal(v.requestRasterBatches(),undefined,'a closed Fast/original worker cannot reopen itself');continue;}
 // A reopened/current source owns a fresh epoch; old completion stays inert.
 v.abort=new AbortController();v.state.sourceMeshes=128;v.hidden=false;v.disposed=false;v.ready=true;v.mode='raster';v.state.visible=true;v.state.firstFrame=true;v.raster=()=>{v.renderRasterScene();};
 const next=v.requestRasterBatches();await Promise.resolve();assert.equal(records.length,2);records[1].finish();await next;assert.equal(v.state.rasterBatchState,'active');assert.equal(v.rasterBatches,records[1]);assert.equal(prior.disposed,1);v.releaseRasterBatches();
}
{
 const {v}=batchFixture();v.state.visible=true;v.state.firstFrame=true;let builds=0;rasterFactory=async()=>{builds++;return {supported:false,multiDraw:false,reason:'Multi-draw unavailable',dispose(){}};};
 await v.requestRasterBatches();v.requestRasterBatches();assert.equal(builds,1,'unsupported source does not rebuild on every camera move');assert.equal(v.state.rasterBatchState,'fallback');assert.equal(v.state.rasterBatchBytes,0);assert.equal(v.state.rasterBatchBuildMs,null);assert.equal(v.state.visible,true);assert.equal(v.state.rasterBatchMultiDraw,false);
 v.releaseRasterBatches();await v.requestRasterBatches();assert.equal(builds,2,'a new epoch may retry capability/fidelity checks');
}
console.log('ok deferred original proof, batch diagnostics, policy/solder/resize/hide/load/dispose/pathtrace invalidation and reopen');
{
 const {v,records}=batchFixture({compilePending:true});v.state.visible=true;v.state.firstFrame=true;
 const old=v.requestRasterBatches();await Promise.resolve();const oldEpoch=v.rasterEpoch;v.releaseRasterBatches();assert.equal(records[0].disposed,1);
 const current=v.requestRasterBatches();await Promise.resolve();records[1].finish();await current;assert.equal(v.state.rasterBatchState,'active');const epoch=v.state.rasterBatchEpoch;assert(epoch>oldEpoch);
 records[0].finish();await old;assert.equal(v.state.rasterBatchEpoch,epoch);assert.equal(v.state.rasterBatchState,'active');assert.equal(v.rasterBatches,records[1]);assert.equal(records[1].disposed,0);v.releaseRasterBatches();
}
console.log('ok older compile resolves after replacement is already active without changing its ownership or diagnostics');
