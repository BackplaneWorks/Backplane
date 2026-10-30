// Isolated real original-GLB raster/path-tracing evidence. No live app/CAD IO.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs')).href);
const glb=process.argv[2]||'/home/h/.cache/backplane-bend/models/board-130960143.glb',out=resolve(process.env.PATH_TRACE_OUT||'build/viewer/pathtrace');mkdirSync(out,{recursive:true});
const vendorDir=process.env.PATH_VENDOR_DIR||'build/viewer/vendor',width=Number(process.env.PATH_WIDTH||900),height=Number(process.env.PATH_HEIGHT||650);
const bytes=readFileSync(glb),sha=v=>createHash('sha256').update(v).digest('hex');
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
 const p=new URL(req.url).pathname;
 if(p==='/solid-pathtrace.js')return new Response(readFileSync('src/web/solid-pathtrace.js'),{headers:{'Content-Type':'application/javascript'}});
 if(p==='/viewer/vendor/viewer.js'||p==='/viewer/vendor/bvh-worker.js'||p==='/viewer/vendor/render-worker.js')return new Response(readFileSync(resolve(vendorDir,p.split('/').at(-1))),{headers:{'Content-Type':'application/javascript'}});
 if(p==='/board.glb')return new Response(bytes,{headers:{'Content-Type':'model/gltf-binary','Content-Length':String(bytes.length)}});
 return new Response('<style>html,body{margin:0;background:#202128}canvas{width:'+width+'px;height:'+height+'px;display:block}</style><canvas></canvas>',{headers:{'Content-Type':'text/html'}});
}});
const hardware=process.env.PATH_GPU==='hardware';
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome-stable',headless:true,args:hardware?['--enable-gpu','--use-angle=vulkan','--disable-vulkan-surface']:['--use-angle=swiftshader','--enable-unsafe-swiftshader'],env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:''}});
try{
 const page=await browser.newPage({viewport:{width,height}}),errors=[],logs=[];
 page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>logs.push(m.text()));
 await page.goto(`http://127.0.0.1:${server.port}/`);
 const loaded=await page.evaluate(async({samples,pixels,component,maxMs})=>{
  const {createPathTraceView}=await import('/solid-pathtrace.js');window.states=[];
  window.v=await createPathTraceView({canvas:document.querySelector('canvas'),onStatus:s=>states.push({...s,at:performance.now()}),maxSamples:samples,maxMs:Number(maxMs),maxPixels:pixels});
  let prior=performance.now();window.delays=[];const input=setInterval(()=>{const now=performance.now();delays.push(Math.max(0,now-prior-8));prior=now},8);
  const t=performance.now();const result=await v.load({url:'/board.glb',key:'board',revision:'read-only-fixture',source:'/read-only-board.kicad_pcb'});clearInterval(input);
  {
   const pivot=component?[11900,-5300,142]:[9280,-7699,113.5],r=150,yaw=-0.6,pitch=0.5;let dist=component?450:0;
   const f=[-Math.sin(yaw)*Math.cos(pitch),Math.cos(yaw)*Math.cos(pitch),-Math.sin(pitch)],right=[Math.cos(yaw),Math.sin(yaw),0],u=[-f[1]*0+right[1]*f[2],-right[0]*f[2],right[0]*f[1]-right[1]*f[0]];
   if(!component){const dot=(a,b)=>a.reduce((s,v,i)=>s+v*b[i],0),aspect=document.querySelector('canvas').clientWidth/document.querySelector('canvas').clientHeight,tan=Math.tan(.7/2);for(const x of [5000,13560])for(const y of [-10398,-5000])for(const z of [-8,235]){const d=[x-pivot[0],y-pivot[1],z-pivot[2]],toward=-dot(d,f);dist=Math.max(dist,toward+Math.abs(dot(d,right))/(tan*aspect)*1.08,toward+Math.abs(dot(d,u))/tan*1.08);}}
   v.setCamera({cam:{pivot,r:right,u,f,sx:0,sy:0,dist,home:dist,fov:0.7},projection:'perspective',exposure:0,aperture:'off'});
   await new Promise(r=>setTimeout(r,150));
  }
  return {result,loadMs:performance.now()-t,maxInputDelay:Math.max(...delays),states:states.slice()};
 },{samples:Number(process.env.PATH_SAMPLES||8),pixels:Number(process.env.PATH_PIXELS||2000000),component:process.env.PATH_COMPONENT||null,maxMs:Number(process.env.PATH_MAX_MS||180000)});
 assert.notEqual(loaded.result.phase,'error',JSON.stringify({loaded,logs}));
 assert.equal(loaded.result.visible,true,'original canvas stays hidden until actual first-frame proof');
 assert.equal(loaded.result.materialSource,'original-glb-with-estimated-missing-factors');assert.ok(loaded.result.estimatedFactorCount>0);
 const studio=await page.evaluate(()=>v.diagnostics());
 if('traceAllocated' in studio){assert.equal(studio.traceAllocated,false,'Studio must not allocate path tracing');assert.equal(studio.bvhBuilding,false);assert.equal(studio.bvhReady,false);}
 await page.locator('canvas').screenshot({path:`out/raster.png`.replace('out',out)});
 if(hardware){assert.match(studio.unmaskedRenderer||'',/NVIDIA.*RTX 5080/i);assert.doesNotMatch(studio.unmaskedRenderer,/SwiftShader|llvmpipe|softpipe|software rasterizer/i);}
 if(studio.pathTraceAvailable===false){
  await page.evaluate(()=>v.setMode('pathtrace'));await page.waitForTimeout(200);const fallback=await page.evaluate(()=>v.diagnostics());
  assert.equal(fallback.phase,'raster');assert.equal(fallback.samples,0);assert.equal(fallback.traceAllocated,false);assert.equal(fallback.bvhReady,false);assert.equal(fallback.visible,true);assert.deepEqual(errors,[]);
  writeFileSync(`${out}/receipt.json`,JSON.stringify({scope:'Safe raster fallback only; does not qualify software tracing',glb,glb_sha256:sha(bytes),studio,fallback,errors},null,2));await page.evaluate(()=>v.dispose());console.log(JSON.stringify({out,fallback:true,pathTraceReason:fallback.pathTraceReason,traceAllocated:false,errors}));
 }else{
 await page.evaluate(samples=>v.setMode('pathtrace',{samples}),Number(process.env.PATH_SAMPLES||8));
 await page.waitForFunction(()=>['converged','error','paused','unsupported'].includes(v.status().phase),{},{timeout:Number(process.env.PATH_MAX_MS||180000)+20000});
 const status=await page.evaluate(()=>v.status());
 const supportsRaw=await page.evaluate(()=>typeof v.present==='function');
 if(supportsRaw){await page.evaluate(()=>v.present(false));await page.locator('canvas').screenshot({path:`${out}/raw.png`});await page.screenshot({path:`${out}/raw-crop.png`,clip:{x:Math.round(width*.25),y:Math.round(height*.2),width:Math.round(width*.5),height:Math.round(height*.6)}});await page.evaluate(()=>v.present(true));}
 const quality=await page.evaluate(()=>v.diagnostics());
 await page.locator('canvas').screenshot({path:`${out}/pathtrace.png`});
 await page.screenshot({path:`${out}/denoised-crop.png`,clip:{x:Math.round(width*.25),y:Math.round(height*.2),width:Math.round(width*.5),height:Math.round(height*.6)}});
 const checks=await page.evaluate(async()=>{
  const current=v.status();v.setInteractive(true);await new Promise(r=>setTimeout(r,100));const moving=v.status().phase;v.setMode('raster');
  const source='/read-only-board.kicad_pcb',component={id:'fixture-cap-uuid',reference:'C5'};
  await v.setComponents({source,sourceRevision:'wrong',components:[component]});const mismatch=await v.componentBounds(component.id);
  await v.setComponents({source,sourceRevision:'read-only-fixture',components:[component,{...component,id:'ambiguous'}]});const duplicate=await v.componentBounds(component.id);
  await v.setComponents({source,sourceRevision:'read-only-fixture',components:[component]});const bounds=await v.componentBounds(component.id);
  const pivot=[11900,-5300,142],f=[-0.495520388354,0.724300143352,-0.479425538604],r=[0.82533561491,-0.564642473395,0],u=[0.270704021927,0.395686971707,0.87758256189];
  v.setCamera({cam:{pivot,r,u,f,sx:0,sy:0,dist:600,home:600,fov:0.7},projection:'perspective',exposure:0,aperture:'off'});
  const picked=await v.pick(document.querySelector('canvas').clientWidth/2,document.querySelector('canvas').clientHeight/2);v.selectComponent(component.id);
  const before=await v.diagnostics();await new Promise(r=>setTimeout(r,150));const after=await v.diagnostics();const idle=before.renderCalls===after.renderCalls;
  v.dispose();return {samples:current.samples,moving,idle,disposed:true,diagnostics:after,body:{mismatch,duplicate,bounds,picked}};
 });
 const receipt={glb,glb_sha256:sha(bytes),source_sha256:sha(readFileSync('src/web/solid-pathtrace.js')),vendor:JSON.parse(readFileSync(resolve(vendorDir,'manifest.json'))),loaded,studio,status,quality,viewport:{width,height},checks,errors,logs,browser:await browser.version(),raster_sha256:sha(readFileSync(`${out}/raster.png`)),pathtrace_sha256:sha(readFileSync(`${out}/pathtrace.png`))};
 writeFileSync(`${out}/receipt.json`,JSON.stringify(receipt,null,2)+'\n');
 assert.ok(status.samples>0,JSON.stringify({status,logs}));assert.equal(status.phase,'converged',JSON.stringify({status,logs}));assert.equal(checks.moving,'raster');assert.ok(checks.idle);assert.equal(checks.body.mismatch,null);assert.equal(checks.body.duplicate,null);assert.equal(checks.body.picked,'fixture-cap-uuid');assert.ok(Math.abs(checks.body.bounds.min[0]-118)<0.01);assert.deepEqual(errors,[]);assert.notEqual(receipt.raster_sha256,receipt.pathtrace_sha256);
 console.log(JSON.stringify({out,status,checks,loadMs:loaded.loadMs,maxInputDelay:loaded.maxInputDelay,errors}));
 }
}finally{await browser.close();server.stop(true);}
