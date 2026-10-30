// Pure lifecycle checks by default. --gpu checks the production GLSL against
// an independent area-integral reference; callers serialize GPU work.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {HDRResolve} from '../../src/web/solid-hdr-resolve.js';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
{
 let targets=0,disposedTargets=0,disposedMaterials=0,disposedQuads=0,throwRender=false;
 class Vector{set(){return this;}copy(){return this;}}
 const T={Vector2:Vector,Matrix4:Vector,ShaderMaterial:class{constructor(values){Object.assign(this,values);}dispose(){disposedMaterials++;}},WebGLRenderTarget:class{constructor(width,height){Object.assign(this,{width,height,texture:{}});targets++;}dispose(){disposedTargets++;}}};
 class Quad{constructor(material){this.material=material;}render(){if(throwRender)throw Error('GPU fixture error');}dispose(){disposedQuads++;}}
 const pass=new HDRResolve(T,Quad),previous={},renderer={target:previous,getRenderTarget(){return this.target;},setRenderTarget(t){this.target=t;}},source={width:3,height:3,texture:{}};
 const first=pass.render(renderer,source,2,2);assert.equal(renderer.target,previous);assert.equal(pass.render(renderer,source,2,2),first);assert.equal(targets,2);
 pass.render(renderer,source,3,2);assert.equal(disposedTargets,2);assert.equal(pass.bytes,120);assert.equal(pass.maxTexelsPerPixel,9);const retained=pass.target;assert.throws(()=>pass.render(renderer,{width:2000000,height:1,texture:{}},1,1),e=>e.code==='HDR_RESOLVE_PENDING');assert.equal(pass.target,retained);assert.equal(renderer.target,previous);assert.equal(targets,4);throwRender=true;assert.throws(()=>pass.render(renderer,source,3,2),/GPU fixture/);assert.equal(renderer.target,previous,'failed resolve restores render target');
 for(const n of [0,NaN,Infinity,-1,1.5])assert.throws(()=>pass.render(renderer,source,n,2),/output dimensions/);assert.throws(()=>pass.render(renderer,{width:0,height:1},1,1),/input dimensions/);
 pass.dispose();pass.dispose();assert.equal(disposedTargets,4);assert.equal(disposedMaterials,2);assert.equal(disposedQuads,2);assert.equal(pass.bytes,0);
}
if(!process.argv.includes('--gpu')){console.log('HDR resolve lifecycle, reuse, exception restore and disposal PASS; GPU gate requires --gpu');process.exit(0);}
const cases=[],add=(name,w,h,ow,oh,pixel)=>{const data=Float32Array.from(Array.from({length:w*h},(_,i)=>pixel(i%w,Math.floor(i/w))).flat());cases.push({name,w,h,ow,oh,data:Array.from(data)});};
add('identity',3,2,3,2,(x,y)=>[x*.2+2,y*.3,.7,1]);
add('constant-odd-anisotropic',7,5,5,3,()=>[2,.5,9,.4]);
add('checker-1.5',3,3,2,2,(x,y)=>[(x+y)%2?4:0,0,0,1]);
add('bright-sparse',7,5,5,3,(x,y)=>x===3&&y===2?[600,2000,.2,1]:[0,0,0,1]);
add('transparent-border',3,1,2,1,(x)=>x===2?[0,0,1,1]:[10000,5000,0,0]);
add('diagonal',9,7,6,5,(x,y)=>x>y*1.1?[1,.25,0,1]:[0,0,0,1]);
add('upsample-retained-target',3,2,5,3,(x,y)=>[x,y,.1,1]);
add('ratio-limit',4,4,2,2,(x,y)=>[x,y,x*y,1]);
function reference(c){const out=[];for(let y=0;y<c.oh;y++)for(let x=0;x<c.ow;x++){let alpha=0,red=0,green=0,blue=0,area=0;for(let sy=0;sy<c.h;sy++)for(let sx=0;sx<c.w;sx++){const ix=Math.max(0,Math.min((x+1)*c.w/c.ow,sx+1)-Math.max(x*c.w/c.ow,sx)),iy=Math.max(0,Math.min((y+1)*c.h/c.oh,sy+1)-Math.max(y*c.h/c.oh,sy)),w=ix*iy,i=(sy*c.w+sx)*4,a=c.data[i+3];area+=w;alpha+=w*a;red+=w*a*c.data[i];green+=w*a*c.data[i+1];blue+=w*a*c.data[i+2];}out.push(alpha?red/alpha:0,alpha?green/alpha:0,alpha?blue/alpha:0,alpha/area);}return out;}
const vendor=resolve(process.env.HDR_VENDOR||root+'/build/viewer/vendor-rng-reviewed'),out=resolve(process.env.HDR_OUT||root+'/build/viewer/hdr-resolve-evidence');mkdirSync(out,{recursive:true});
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){const p=new URL(req.url).pathname;if(p==='/vendor.js')return new Response(readFileSync(vendor+'/viewer.js'),{headers:{'Content-Type':'application/javascript'}});if(p==='/resolve.js')return new Response(readFileSync(root+'/src/web/solid-hdr-resolve.js'),{headers:{'Content-Type':'application/javascript'}});return new Response('<canvas width="32" height="32"></canvas>',{headers:{'Content-Type':'text/html'}});}});
const {chromium}=await import('/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs'),hardware=process.env.PATH_GPU==='hardware',browser=await chromium.launch({executablePath:'/usr/bin/google-chrome-stable',headless:true,args:hardware?['--enable-gpu','--use-angle=vulkan','--disable-vulkan-surface']:['--use-angle=swiftshader','--enable-unsafe-swiftshader'],env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:''}});
try{
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(String(e)));const logs=[];page.on('console',m=>{if(m.type()==='error')logs.push(m.text());});await page.goto(`http://127.0.0.1:${server.port}`);
 const result=await page.evaluate(async cases=>{
  const {THREE:T,FullScreenQuad}=await import('/vendor.js'),{HDRResolve}=await import('/resolve.js'),renderer=new T.WebGLRenderer({canvas:document.querySelector('canvas'),antialias:false}),gl=renderer.getContext(),ext=gl.getExtension('WEBGL_debug_renderer_info'),identity=ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):null;renderer.toneMapping=T.ACESFilmicToneMapping;renderer.toneMappingExposure=2.7;renderer.outputColorSpace=T.SRGBColorSpace;
  const pass=new HDRResolve(T,FullScreenQuad),results=[];for(const c of cases){const texture=new T.DataTexture(Float32Array.from(c.data),c.w,c.h,T.RGBAFormat,T.FloatType);texture.needsUpdate=true;const target=pass.render(renderer,{width:c.w,height:c.h,texture},c.ow,c.oh),pixels=new Float32Array(c.ow*c.oh*4);renderer.readRenderTargetPixels(target,0,0,c.ow,c.oh,pixels);results.push({name:c.name,pixels:Array.from(pixels),error:gl.getError()});texture.dispose();}
  const guides=[];for(const kind of ['constant','albedo','depth','normal','background']){const make=fn=>{const a=[];for(let x=0;x<3;x++)a.push(...fn(x));const t=new T.DataTexture(Float32Array.from(a),3,1,T.RGBAFormat,T.FloatType);t.needsUpdate=true;return t;},normal=make(x=>kind==='normal'&&x===1?[1,.5,.5,1]:[.5,.5,1,1]),albedo=make(x=>kind==='albedo'&&x===1?[1,1,1,1]:[.1,.1,.1,1]),depth=make(x=>[kind==='background'&&x<2?1:kind==='depth'&&x===1?.8:.5,0,0,1]),color=make(()=>[1,1,1,1]);pass.render(renderer,{width:3,height:1,texture:color},2,1);const t=pass.protectMixed(renderer,normal,depth,albedo,new T.Matrix4()),pixels=new Uint8Array(8);renderer.readRenderTargetPixels(t,0,0,2,1,pixels);guides.push({kind,mixed:[pixels[0],pixels[4]],error:gl.getError()});for(const t of [normal,albedo,depth,color])t.dispose();}
  const retained=pass.target,previous=renderer.getRenderTarget();let pendingCode;try{pass.render(renderer,{width:2000000,height:1,texture:{}},1,1);}catch(e){pendingCode=e.code;}const stalePreserved=pass.target===retained&&renderer.getRenderTarget()===previous,maxTexelsPerPixel=pass.maxTexelsPerPixel;pass.dispose();const texturesAfterDispose=renderer.info.memory.textures;renderer.dispose();return {identity,results,guides,texturesAfterDispose,pendingCode,stalePreserved,maxTexelsPerPixel};
 },cases);
 if(hardware)assert.match(result.identity||'',/NVIDIA.*RTX 5080/i);assert.equal(result.texturesAfterDispose,0);assert.equal(result.pendingCode,'HDR_RESOLVE_PENDING');assert(result.stalePreserved);assert.equal(result.maxTexelsPerPixel,9);const metrics=[];
 for(let n=0;n<cases.length;n++){const c=cases[n],gpu=result.results[n],cpu=reference(c);assert.equal(gpu.error,0);let maxError=0,nearestError=0;for(let i=0;i<cpu.length;i++){const error=Math.abs(gpu.pixels[i]-cpu[i]);maxError=Math.max(maxError,error);assert(error<Math.max(3e-5,Math.abs(cpu[i])*3e-6),`${c.name}[${i}] GPU${gpu.pixels[i]} CPU${cpu[i]}`);const p=Math.floor(i/4),x=p%c.ow,y=Math.floor(p/c.ow),index=(Math.min(c.h-1,Math.floor((y+.5)*c.h/c.oh))*c.w+Math.min(c.w-1,Math.floor((x+.5)*c.w/c.ow)))*4+i%4;nearestError+=Math.abs(c.data[index]-cpu[i]);}metrics.push({name:c.name,maxError,nearestMAE:nearestError/cpu.length});}
 assert(metrics.find(x=>x.name==='diagonal').nearestMAE>.01);assert.deepEqual(result.guides.map(x=>[x.kind,x.mixed]),[['constant',[0,0]],['albedo',[255,255]],['depth',[255,255]],['normal',[255,255]],['background',[0,255]]]);assert(result.guides.every(x=>x.error===0));assert.deepEqual(errors,[]);assert.deepEqual(logs,[]);
 const receipt={scope:'Actual production area/mixed-guide shaders vs independent CPU pixel integral; no path tracer allocation',...result,metrics,errors,logs};writeFileSync(out+'/analytic-receipt.json',JSON.stringify(receipt,null,2));console.log(JSON.stringify({out,identity:result.identity,metrics,guides:result.guides,texturesAfterDispose:result.texturesAfterDispose,errors}));
}finally{await browser.close();server.stop(true);}
