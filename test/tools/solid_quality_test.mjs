// Fresh saved-model QFN/mask optical and edge evidence, isolated browser only.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const {chromium}=await import('/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs');
const out=resolve(process.env.QUALITY_OUT||'build/viewer/qfn-quality'),vendor=resolve(process.env.QUALITY_VENDOR||'build/viewer/vendor-qfn-after');mkdirSync(out,{recursive:true});
const model=process.env.QUALITY_GLB||'/tmp/backplane-solder-evidence/saved-smu.glb',metadata=process.env.QUALITY_METADATA||'/tmp/backplane-solder-evidence/saved-components.json',bytes=readFileSync(model),data=JSON.parse(readFileSync(metadata)),sha=b=>createHash('sha256').update(b).digest('hex');
const width=Number(process.env.QUALITY_WIDTH||1000),height=Number(process.env.QUALITY_HEIGHT||700),samples=Number(process.env.QUALITY_SAMPLES||16),budget=Number(process.env.QUALITY_MS||300000),policy=process.env.QUALITY_POLICY||'realistic',view=process.env.QUALITY_VIEW||'qfn';
const gpuMode=process.env.PATH_GPU||'software';assert(['software','hardware'].includes(gpuMode),'PATH_GPU must explicitly select software or hardware');
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){const p=new URL(req.url).pathname;
 if(p==='/solid-pathtrace.js')return new Response(readFileSync('src/web/solid-pathtrace.js'),{headers:{'Content-Type':'application/javascript'}});
 if(p.startsWith('/viewer/vendor/'))return new Response(readFileSync(resolve(vendor,p.split('/').at(-1))),{headers:{'Content-Type':'application/javascript'}});
 if(p==='/board.glb')return new Response(bytes,{headers:{'Content-Type':'model/gltf-binary'}});
 return new Response(`<style>html,body{margin:0;background:#202128}canvas{width:${width}px;height:${height}px;display:block}</style><canvas></canvas>`,{headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome-stable',headless:true,args:gpuMode==='hardware'?['--enable-gpu','--use-angle=vulkan','--disable-vulkan-surface']:['--use-angle=swiftshader','--enable-unsafe-swiftshader'],env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:''}});
try{
 const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1}),errors=[];page.on('pageerror',e=>errors.push(String(e)));await page.goto(`http://127.0.0.1:${server.port}`);
 const initial=await page.evaluate(async({data,samples,budget,policy,view})=>{
  const {createPathTraceView}=await import('/solid-pathtrace.js');window.states=[];window.v=await createPathTraceView({canvas:document.querySelector('canvas'),materialPolicy:policy,maxPixels:2000000,maxMs:budget,maxSamples:samples,onStatus:s=>states.push({...s,at:performance.now()})});
  const pivot=view==='qfn'?[11500,-7700,76]:[9280,-7699,113.5],yaw=-.6,pitch=.5,f=[-Math.sin(yaw)*Math.cos(pitch),Math.cos(yaw)*Math.cos(pitch),-Math.sin(pitch)],r=[Math.cos(yaw),Math.sin(yaw),0],u=[r[1]*f[2],-r[0]*f[2],r[0]*f[1]-r[1]*f[0]];
  let dist=view==='qfn'?1000:0;
  if(view!=='qfn'){const dot=(a,b)=>a.reduce((s,v,i)=>s+v*b[i],0),aspect=document.querySelector('canvas').clientWidth/document.querySelector('canvas').clientHeight,tan=Math.tan(.7/2);for(const x of [5000,13560])for(const y of [-10398,-5000])for(const z of [-8,235]){const d=[x-pivot[0],y-pivot[1],z-pivot[2]],eye=-dot(d,f);dist=Math.max(dist,eye+Math.abs(dot(d,r))/(tan*aspect)*1.08,eye+Math.abs(dot(d,u))/tan*1.08);}}
  const camera={cam:{pivot,r,u,f,sx:0,sy:0,dist,home:dist,fov:.7},projection:'perspective',exposure:0,aperture:'off'};v.setCamera(camera);v.setMode('raster');
  const t=performance.now();await v.load({url:'/board.glb',key:'fresh-smu',source:data.source,revision:data.sourceRevision});await v.setComponents(data);const bounds=await v.componentBounds('a249b1bf-7fe7-4abb-a884-9e32cc3761c3');return {loadMs:performance.now()-t,bounds,camera,diagnostics:await v.diagnostics()};
 },{data,samples,budget,policy,view});
 assert.equal(initial.diagnostics.traceAllocated,false);assert.equal(initial.diagnostics.upscaled,false);assert.equal(initial.diagnostics.materialPolicy,policy);assert.equal(initial.bounds.source,data.source);assert.ok(Math.abs(initial.bounds.min[2]-.075)<1e-5,'source QFN must remain at its actually exported intersecting height');
 if(gpuMode==='hardware'){assert.match(initial.diagnostics.unmaskedRenderer||'',/NVIDIA.*RTX 5080/i,'actual render-worker context must expose the hardware identity');assert.doesNotMatch(initial.diagnostics.unmaskedRenderer,/SwiftShader|llvmpipe|software/i);}
 await page.locator('canvas').screenshot({path:out+'/raster.png'});
 if(policy==='realistic'){assert.equal(initial.diagnostics.estimatedFactorCount,26);assert.equal(initial.diagnostics.materialRoleCounts.substrate,1);assert.equal(initial.diagnostics.materialRoleCounts.soldermask,2);assert(initial.diagnostics.materialPolicies.includes('realistic'));}
 assert.notEqual(initial.diagnostics.pathTraceAvailable,false,initial.diagnostics.pathTraceReason||'Use the source-only software fallback test; tracing is unavailable.');
 const started=performance.now();await page.evaluate(samples=>{
  const probe=window.probe={active:true,rafCount:0,maxRAFGap:0,maxAckMs:0,acks:0},start=performance.now();let last=start,pending=false;
  function tick(now){probe.rafCount++;probe.maxRAFGap=Math.max(probe.maxRAFGap,now-last);last=now;if(probe.active)requestAnimationFrame(tick);}requestAnimationFrame(tick);
  probe.timer=setInterval(async()=>{if(pending)return;pending=true;const t=performance.now();try{await v.diagnostics();probe.maxAckMs=Math.max(probe.maxAckMs,performance.now()-t);probe.acks++;}finally{pending=false;}},250);v.setMode('pathtrace',{samples});
 },samples);
 let duringCaptureMs=null,duringCaptureError=null;
 if(process.env.QUALITY_DURING_CAPTURE){await page.waitForFunction(()=>v.status().samples>0,{},{timeout:60000});const t=performance.now();try{await page.screenshot({path:out+'/during-refinement.png',timeout:15000});duringCaptureMs=performance.now()-t;}catch(e){duringCaptureError=e.message;}}
 await page.waitForFunction(()=>['converged','paused','error','unsupported'].includes(v.status().phase),{},{timeout:budget+40000});const elapsedMs=performance.now()-started,status=await page.evaluate(()=>v.diagnostics()),responsiveness=await page.evaluate(()=>{probe.active=false;clearInterval(probe.timer);return {rafCount:probe.rafCount,maxRAFGap:probe.maxRAFGap,maxAckMs:probe.maxAckMs,acks:probe.acks};});
 assert.ok(status.samples>=1,JSON.stringify(status));assert.equal(status.phase,'converged',JSON.stringify(status));
 await page.evaluate(()=>v.present(false));await page.locator('canvas').screenshot({path:out+'/raw.png'});await page.screenshot({path:out+'/raw-crop.png',clip:{x:Math.round(width*.25),y:Math.round(height*.25),width:Math.round(width*.5),height:Math.round(height*.5)}});
 await page.evaluate(()=>v.present(true));await page.locator('canvas').screenshot({path:out+'/denoised.png'});await page.screenshot({path:out+'/denoised-crop.png',clip:{x:Math.round(width*.25),y:Math.round(height*.25),width:Math.round(width*.5),height:Math.round(height*.5)}});
 const final=await page.evaluate(()=>v.diagnostics());await page.evaluate(()=>v.dispose());assert.deepEqual(errors,[]);assert.equal(sha(readFileSync(model)),sha(bytes));
 const receipt={model,glbSha256:sha(bytes),metadataSha256:sha(readFileSync(metadata)),vendor:JSON.parse(readFileSync(vendor+'/manifest.json')),gpuMode,policy,view,viewport:{width,height},samples,budget,elapsedMs,responsiveness,duringCaptureMs,duringCaptureError,initial,status,final,errors};writeFileSync(out+'/receipt.json',JSON.stringify(receipt,null,2));console.log(JSON.stringify({out,gpuMode,policy,view,elapsedMs,loadMs:initial.loadMs,responsiveness,duringCaptureMs,duringCaptureError,status,errors}));
}finally{await browser.close();server.stop(true);}
