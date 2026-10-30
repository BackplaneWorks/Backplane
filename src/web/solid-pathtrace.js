// Main-thread controller only. Model parsing, Three/WebGL shader compilation,
// geometry preparation and ray tracing all run in the dedicated render worker.
export async function createPathTraceView({canvas,vendorUrl='/viewer/vendor/viewer.js',workerUrl='/viewer/vendor/bvh-worker.js',renderWorkerUrl='/viewer/vendor/render-worker.js',onStatus=()=>{},onSelect=()=>{},maxSamples=32,maxMs=30000,maxPixels=2000000,materialPolicy='cad-studio'}={}) {
  if(!canvas?.transferControlToOffscreen||typeof Worker==='undefined')throw Error('Threaded original-model rendering is unavailable; preview remains available.');
  for(const path of [vendorUrl,workerUrl,renderWorkerUrl]){if(new URL(path,location.href).origin!==location.origin)throw Error('Renderer assets must be served by this hub.');}
  const worker=new Worker(renderWorkerUrl,{type:'module',name:'backplane-original-model-render'}),pending=new Map();let serial=0,disposed=false,status={phase:'initializing',samples:0},generation=0,cameraQueued=false,latestCamera,lastCameraToken;
  const send=(method,args={},transfer=[])=>{if(disposed)return Promise.reject(Error('Renderer disposed'));const id=++serial;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(Error(`Renderer ${method} timed out`));},120000);pending.set(id,{resolve,reject,timer});worker.postMessage({id,method,args},transfer);});};
  const notify=(method,args)=>{if(!disposed)worker.postMessage({method,args});};
  const flushCamera=()=>{if(!cameraQueued)return;cameraQueued=false;notify('camera',latestCamera);};
  worker.onmessage=({data})=>{if(disposed)return;if(data.status){status=data.status;onStatus({...status});return;}const p=pending.get(data.id);if(!p)return;clearTimeout(p.timer);pending.delete(data.id);data.error?p.reject(Error(data.error)):p.resolve(data.result);};
  worker.onerror=e=>{status={...status,phase:'error',reason:e.message||'Original-model worker failed'};onStatus({...status});for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error(status.reason));}pending.clear();};
  const absolute=u=>new URL(u,location.href).href;
  const offscreen=canvas.transferControlToOffscreen();
  const initial=send('init',{canvas:offscreen,vendorUrl:absolute(vendorUrl),workerUrl:absolute(workerUrl),maxSamples,maxMs,maxPixels,materialPolicy,width:Math.max(1,canvas.clientWidth),height:Math.max(1,canvas.clientHeight),pixelRatio:devicePixelRatio||1},[offscreen]);
  const resize=new ResizeObserver(()=>notify('resize',{width:Math.max(1,canvas.clientWidth),height:Math.max(1,canvas.clientHeight),pixelRatio:devicePixelRatio||1}));resize.observe(canvas);
  const visibility=()=>notify('visibility',{hidden:document.hidden});document.addEventListener('visibilitychange',visibility);
  let down=null;
  const pointerDown=e=>{down={x:e.clientX,y:e.clientY};};
  const pointerUp=e=>{if(!down||Math.hypot(e.clientX-down.x,e.clientY-down.y)>4){down=null;return;}down=null;const b=canvas.getBoundingClientRect();void send('pick',{x:e.clientX-b.left,y:e.clientY-b.top}).then(id=>{if(id!=null)onSelect(id);}).catch(()=>{});};
  canvas.addEventListener('pointerdown',pointerDown);canvas.addEventListener('pointerup',pointerUp);
  const controller={
    async load(asset){const gen=++generation,signal=asset.signal;if(signal?.aborted)return;const abort=()=>notify('cancel',{generation:gen});signal?.addEventListener('abort',abort,{once:true});try{return await send('load',{...asset,signal:undefined,url:absolute(asset.url),generation:gen});}finally{signal?.removeEventListener('abort',abort);}},
    setCamera(state){const token=JSON.stringify([state.cam,state.projection,state.exposure,state.aperture,state.focusDistance]);if(token===lastCameraToken)return;lastCameraToken=token;latestCamera=state;if(cameraQueued)return;cameraQueued=true;requestAnimationFrame(flushCamera);},
    setInteractive(on){flushCamera();notify('interactive',{on:!!on});},
    setMode(mode,options={}){flushCamera();notify('mode',{mode,options});},
    setComponents(data,callback){if(callback)onSelect=callback;return send('components',data);},
    selectComponent(id){notify('selection',{id});},
    pick(x,y){flushCamera();return send('pick',{x,y});},
    componentBounds(id){return send('bounds',{id});},
    setMaterialPolicy(policy){return send('material-policy',{policy});},
    setSolderPreview(enabled){flushCamera();return send('solder-preview',{enabled:!!enabled});},
    present(denoised=true){return send('present',{denoised});},
    status(){return {...status};},
    diagnostics(){return send('status',{});},
    dispose(){if(disposed)return;notify('dispose',{});disposed=true;generation++;resize.disconnect();document.removeEventListener('visibilitychange',visibility);canvas.removeEventListener('pointerdown',pointerDown);canvas.removeEventListener('pointerup',pointerUp);for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error('Renderer disposed'));}pending.clear();worker.terminate();},
  };
  try{await initial;return controller;}catch(error){controller.dispose();throw error;}
}
