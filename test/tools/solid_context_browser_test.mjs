// Real saved GLB -> production preview decoder/shaders, including browser GPU
// context loss/restoration. App is stubbed only for two localized status words.
// No hub, live browser profile, user session or CAD save.
// node test/tools/solid_context_browser_test.mjs --glb FILE --out DIR
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {glbFixture} from './solid_glb_fixture.mjs';
const args=process.argv.slice(2),opt=(n,d='')=>{const i=args.indexOf('--'+n);return i<0?d:args[i+1];};
const glb=path.resolve(opt('glb')),out=path.resolve(opt('out')),source=path.resolve(opt('source','src/web/solid.js')),broken=args.includes('--expect-broken');
assert.ok(opt('glb')&&opt('out'),'supply --glb and --out');fs.mkdirSync(out,{recursive:true});
const fixture=glbFixture(glb),sha=b=>createHash('sha256').update(b).digest('hex');
const solid=fs.readFileSync(source,'utf8')+'\nexport {view as testView, assets as testAssets};';
const files=['solid-assets.js','solid-materials.js','solid-dof.js','solid-framing.js','mesh-worker.js','solid-pictures.js'];
const server=http.createServer((req,res)=>{
 const name=new URL(req.url,'http://fixture').pathname.slice(1);
 if(name==='solid.js'){res.setHeader('Content-Type','text/javascript');res.end(solid);}
 else if(files.includes(name)){res.setHeader('Content-Type','text/javascript');res.end(fs.readFileSync(path.join(path.dirname(source),name)));}
 else if(name==='app.bend'){res.setHeader('Content-Type','text/javascript');res.end('export default {solid_waiting:()=>"Preparing model",solid_stats:()=>"Ready"};');}
 else if(name==='board.mesh')res.end(fixture.mesh);
 else{res.setHeader('Content-Type','text/html');res.end('<style>html,body{margin:0;height:100%;background:#202128}canvas{width:100%;height:100%;display:block;touch-action:none}.viewer-note{position:absolute;top:0;color:white}</style><canvas id="solid" data-key="2|real-board" data-scope="test-thread"></canvas><div class="viewer-note" role="status"></div><div class="viewer-stat"></div>');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const {chromium}=await import(opt('playwright','/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs'));
const browser=await chromium.launch({headless:true,executablePath:opt('browser','/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome'),args:['--no-sandbox','--disable-dev-shm-usage','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const facts=[];
try{
 for(const [name,viewport] of [['mobile',{width:390,height:844}],['desktop',{width:1000,height:700}]]){
  const page=await browser.newPage({viewport,hasTouch:name==='mobile'}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));await page.goto('http://127.0.0.1:'+server.address().port);
  await page.evaluate(async({n,box3})=>{
   const S=window.S=await import('/solid.js'),canvas=document.querySelector('canvas');S.mount(canvas);
   const v=S.testView,draw=v.draw.bind(v);window.frames=[];
   v.draw=()=>{draw();const gl=v.gl;if(!gl||gl.isContextLost())return;const px=new Uint8Array(canvas.width*canvas.height*4);gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,px);let visible=0;for(let i=3;i<px.length;i+=4)if(px[i])visible++;window.frames.push(visible);};
   await S.got({key:'2|real-board',n,box3,mesh:new Uint8Array(await(await fetch('/board.mesh')).arrayBuffer())});
  },{n:fixture.n,box3:fixture.box3});
  await page.waitForFunction(()=>window.frames.some(n=>n>50));
  await page.locator('canvas').screenshot({path:path.join(out,name+'-before.png')});
  const result=await page.evaluate(async broken=>{
   const S=window.S,v=S.testView,canvas=document.querySelector('canvas'),gl=v.gl,ext=gl.getExtension('WEBGL_lose_context');
   if(!ext)throw Error('Browser does not expose the context loss extension');
   v.cam.spin(37,-19);v.cam.pan(12,8,v.cam.unit(canvas.clientHeight));S.cameraAction('projection','orthographic');S.cameraAction('exposure',1.2);
   await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
   const pose=JSON.stringify(v.snapshot()),model=S.testAssets.get(v.key).model;
   const lost=await new Promise(resolve=>{canvas.addEventListener('webglcontextlost',e=>queueMicrotask(()=>resolve({prevented:e.defaultPrevented,isLost:gl.isContextLost()})),{once:true});ext.loseContext();});
   const framesBefore=window.frames.length;let restored=false;
   canvas.addEventListener('webglcontextrestored',()=>{restored=true},{once:true});
   // Restoration is requested after the loss event has finished dispatching.
   await new Promise(r=>setTimeout(r,50));ext.restoreContext();
   const end=performance.now()+3000;
   while(performance.now()<end&&!restored)await new Promise(r=>setTimeout(r,20));
   if(!broken&&!restored)throw Error('Context never restored: '+JSON.stringify({lost,glError:gl.getError(),note:document.querySelector('.viewer-note')?.textContent}));
   if(restored){await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);}
   const after=window.frames.slice(framesBefore),result={lost,restored,isLost:gl.isContextLost(),naturalFrames:after,geometryReused:S.testAssets.get(v.key).model===model,cameraPreserved:JSON.stringify(v.snapshot())===pose};
   if(broken){if(lost.prevented||restored||!result.isLost)throw Error('Old unrecoverable context failure did not reproduce');return result;}
   if(!lost.prevented||result.isLost||!after.some(n=>n>50)||!result.geometryReused||!result.cameraPreserved)throw Error('Recovery failed: '+JSON.stringify(result));
   // Disposal while graphics are lost must retire both restoration listeners
   // and scheduled paints; the released canvas cannot become live again.
   await new Promise(resolve=>{canvas.addEventListener('webglcontextlost',resolve,{once:true});ext.loseContext();});
   S.mount(null);const framesAtDispose=window.frames.length;await new Promise(r=>setTimeout(r,50));ext.restoreContext();
   await new Promise(r=>setTimeout(r,100));
   result.disposedQuiet=v.disposed&&v.gl===null&&window.frames.length===framesAtDispose;
   if(!result.disposedQuiet)throw Error('Disposed context became active again');
   return result;
  },broken);
  assert.deepEqual(errors,[]);facts.push({name,viewport,...result});console.log('ok '+name+' '+JSON.stringify(result));await page.close();
 }
 fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify({scope:'isolated production preview graphics context lifecycle; no actual user-session cause claim',broken,source,sourceSHA256:sha(fs.readFileSync(source)),glb,glbSHA256:sha(fs.readFileSync(glb)),triangles:fixture.n,browser:await browser.version(),facts},null,2)+'\n');
}finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
