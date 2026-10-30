// Actual WebGL negatives, separate loopback server/browser, no live app IO.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(pathToFileURL(resolve('/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs')).href);
const vendor=resolve(process.env.PATH_VENDOR_DIR||'build/viewer/vendor'),board=readFileSync(process.argv[2]||'/home/h/.cache/backplane-bend/models/board-130960143.glb');
const json=JSON.stringify({asset:{version:'2.0'},scene:0,scenes:[{nodes:[]}],nodes:[]}),padding=(4-json.length%4)%4,empty=Buffer.alloc(20+json.length+padding,32);
empty.writeUInt32LE(0x46546c67,0);empty.writeUInt32LE(2,4);empty.writeUInt32LE(empty.length,8);empty.writeUInt32LE(empty.length-20,12);empty.writeUInt32LE(0x4e4f534a,16);empty.write(json,20);
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){const p=new URL(req.url).pathname;
 if(p==='/solid-pathtrace.js')return new Response(readFileSync('src/web/solid-pathtrace.js'),{headers:{'Content-Type':'application/javascript'}});
 if(p.startsWith('/viewer/vendor/'))return new Response(readFileSync(resolve(vendor,p.split('/').at(-1))),{headers:{'Content-Type':'application/javascript'}});
 if(p.endsWith('.glb'))return new Response(p==='/empty.glb'?empty:board,{headers:{'Content-Type':'model/gltf-binary'}});
 return new Response('<style>html,body{margin:0}canvas{width:420px;height:300px}</style><canvas></canvas>',{headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome-stable',headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader']});
try{
 const page=await browser.newPage({viewport:{width:420,height:300},deviceScaleFactor:2}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${server.port}/`);
 const result=await page.evaluate(async()=>{
  const {createPathTraceView}=await import('/solid-pathtrace.js'),states=[];
  const v=await createPathTraceView({canvas:document.querySelector('canvas'),onStatus:s=>states.push(s)});
  const load=(url,key)=>v.load({url,key,revision:key,source:'/saved-board.kicad_pcb'});
  const empty=await load('/empty.glb','empty'),emptyDiagnostics=await v.diagnostics();
  // Look away before loading any original triangles; the background itself
  // differs from the historical hardcoded RGB proof under tone mapping.
  const cam={pivot:[1000000,1000000,1000000],r:[1,0,0],u:[0,0,1],f:[0,1,0],sx:0,sy:0,dist:18000,home:18000,fov:.7};
  v.setCamera({cam,projection:'perspective',exposure:1,aperture:'off'});v.setMode('raster');
  const offscreen=await load('/board.glb','offscreen'),offscreenDiagnostics=await v.diagnostics();
  v.setCamera({cam:{...cam,pivot:[9280,-7699,113.5],f:[0,.866,-.5],u:[0,.5,.866]},projection:'perspective',exposure:0,aperture:'off'});
  for(let i=0;i<50&&!v.status().visible;i++)await new Promise(r=>setTimeout(r,100));const visible=await v.diagnostics();
  const fidelity=await v.setMaterialPolicy('source-defaults'),realistic=await v.setMaterialPolicy('realistic');v.setMode('pathtrace',{samples:8});await new Promise(r=>setTimeout(r,250));const blocked=await v.diagnostics();
  // Replacement empty revision must clear the previous visible proof.
  const replacement=await load('/empty.glb','replacement');v.dispose();
  return {empty,emptyDiagnostics,offscreen,offscreenDiagnostics,visible,replacement,fidelity,realistic,blocked,states};
 });
 for(const s of [result.empty,result.offscreen,result.replacement]){assert.equal(s.visible,false,JSON.stringify(s));assert.equal(s.firstFrame,false);assert.equal(s.phase,'raster');}
 assert.equal(result.empty.triangles,0);assert.ok(result.offscreen.triangles>0);
 assert.equal(result.visible.visible,true,JSON.stringify(result));assert.equal(result.visible.firstFrame,true);assert.equal(result.visible.key,'offscreen');
 assert.equal(result.visible.renderWidth,840);assert.equal(result.visible.renderHeight,600);assert.equal(result.visible.upscaled,false);
 for(const s of [result.emptyDiagnostics,result.offscreenDiagnostics,result.visible]){assert.equal(s.traceAllocated,false);assert.equal(s.bvhBuilding,false);assert.equal(s.bvhReady,false);}
 assert.deepEqual(errors,[]);console.log(JSON.stringify({ok:true,empty:result.empty.visible,offscreen:result.offscreen.visible,visible:result.visible.visible,replacement:result.replacement.visible,renderWidth:result.visible.renderWidth,traceAllocated:result.visible.traceAllocated,errors}));
 assert.equal(result.fidelity.estimatedFactorCount,0);assert.equal(result.realistic.estimatedFactorCount,26);assert.equal(result.realistic.materialRoleCounts.substrate,1);
 assert.match(result.blocked.unmaskedRenderer,/SwiftShader|llvmpipe|softpipe|software rasterizer/i);assert.equal(result.blocked.pathTraceAvailable,false);assert.match(result.blocked.pathTraceReason,/software renderer/);assert.equal(result.blocked.phase,'raster');assert.equal(result.blocked.visible,true);assert.equal(result.blocked.samples,0);assert.equal(result.blocked.traceAllocated,false);assert.equal(result.blocked.bvhBuilding,false);assert.equal(result.blocked.bvhReady,false);
 console.log(JSON.stringify({softwareSafetyFallback:true,scope:'source rendering and reversible policy only; historical software tracing responsiveness remains failed',renderer:result.blocked.unmaskedRenderer,pathTraceAvailable:result.blocked.pathTraceAvailable,pathTraceReason:result.blocked.pathTraceReason,allocated:result.blocked.traceAllocated,estimatedFields:result.realistic.estimatedFactorCount,errors}));
}finally{await browser.close();server.stop(true);}
