// Exact production WebGL in an isolated loopback browser. Optional read-only
// GLB path exercises a populated real board and its actual component model.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {glbFixture} from './solid_glb_fixture.mjs';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH||'tools/browser/node_modules/playwright-core/index.mjs')).href);
const out=process.env.SOLID_RENDER_OUT||'build/solid-materials';mkdirSync(out,{recursive:true});
const source=readFileSync('src/web/solid.js','utf8');
const before=process.env.SOLID_BEFORE_SOURCE?readFileSync(process.env.SOLID_BEFORE_SOURCE,'utf8'):null;
const fixture=JSON.parse(readFileSync('test/fixtures/solid-handedness/wire.json','utf8'));
const fixtures=[['board',process.env.SOLID_GLB?glbFixture(process.env.SOLID_GLB):{n:fixture.n,mesh:new Uint8Array(Buffer.from(fixture.mesh_base64,'base64')),box3:fixture.wire_bounds}]];
if(process.env.SOLID_GLB)fixtures.push(['component',glbFixture(process.env.SOLID_GLB,process.env.SOLID_COMPONENT||'C5')]);
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
  const p=new URL(req.url).pathname;
  const js={'/solid.js':source,'/before.js':before,'/app.bend':'export default {solid_waiting:()=>"Waiting",solid_stats:()=>"Ready"};'};
  if(js[p])return new Response(js[p]+(p.endsWith('solid.js')||p==='/before.js'?'\nexport {view as testView};':''),{headers:{'Content-Type':'application/javascript'}});
  if(p==='/solid-dof.js'||p==='/solid-materials.js'||p==='/solid-assets.js'||p==='/solid-framing.js')return new Response(readFileSync(`src/web${p}`),{headers:{'Content-Type':'application/javascript'}});
  return new Response('<style>html,body{margin:0;background:#202128}canvas{width:1000px;height:700px;display:block}</style><div><canvas data-key="2|board"></canvas><span class="viewer-note"></span></div>',{headers:{'Content-Type':'text/html'}});
}});
const executablePath=process.env.CHROME_BIN||['/usr/bin/google-chrome-stable','/usr/bin/chromium','/usr/bin/chromium-browser'].find(existsSync);
const browser=await chromium.launch({executablePath,headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const sha=x=>createHash('sha256').update(x).digest('hex'),receipt={source_sha256:sha(source),materials_sha256:sha(readFileSync('src/web/solid-materials.js')),glb_sha256:process.env.SOLID_GLB?sha(readFileSync(process.env.SOLID_GLB)):null,browser:await browser.version(),images:[],checks:{}};
try{
  for(const variant of before?['before','after']:['after'])for(const [name,wire] of fixtures){
    const page=await browser.newPage({viewport:{width:1000,height:700},deviceScaleFactor:1});
    const errors=[];page.on('pageerror',e=>errors.push(String(e)));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.evaluate(async({variant,wire})=>{
      window.S=await import(variant==='before'?'/before.js':'/solid.js');window.S.mount(document.querySelector('canvas'));
      await window.S.got({key:'2|board',n:wire.n,mesh:new Uint8Array(wire.mesh),box3:wire.box3});
    },{variant,wire:{...wire,mesh:[...wire.mesh]}});
    await page.waitForFunction(()=>window.S.testView.count>0&&(!window.S.assetState||window.S.assetState().status==='ready'));
    await page.evaluate(()=>{window.S.testView.fit();window.S.testView.draw()});
    const path=`${out}/${variant}-${name}.png`;await page.locator('canvas').screenshot({path});
    const state=await page.evaluate(()=>{const v=window.S.testView;return {count:v.count,box:v.box,glError:v.gl.getError()}});
    assert.equal(state.count,wire.n*3);assert.equal(state.glError,0);assert.deepEqual(errors,[]);
    receipt.images.push({name,variant,path,sha256:sha(readFileSync(path)),wire_sha256:sha(wire.mesh),...state});
    if(variant==='after'&&name==='board'){
      receipt.checks=await page.evaluate(async()=>{
        const S=window.S,v=S.testView;
        const image=()=>{v.draw();const gl=v.gl,data=new Uint8Array(v.canvas.width*v.canvas.height*4);gl.readPixels(0,0,v.canvas.width,v.canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,data);return data;};
        const mean=a=>{let s=0;for(let i=0;i<a.length;i+=4)s+=a[i]+a[i+1]+a[i+2];return s/a.length;};
        const different=(a,b)=>a.some((v,i)=>v!==b[i]);
        const point=[(v.box[0]+v.box[3])/200,-(v.box[1]+v.box[4])/200,v.box[2]/100];
        const p=S.projectPoint(point),back=S.rayToBoard(p.x,p.y,point[2]);
        const projection=back.every((x,i)=>Math.abs(x-point[i])<1e-7);
        S.setSelection({id:'saved-footprint',ref:'C5'});
        const original=image(),count=v.count,box=v.box.slice();
        S.cameraAction('exposure',1);const brighter=image();
        S.cameraAction('exposure',0);const restored=image();
        S.setThermal({enabled:true,temperature:240,liquid:0.9});const thermal=image();
        S.setThermal({enabled:false});const cooled=image();
        let dof = false, focusLock = false, orthoSharp = false, dofError = null;
        if (Object.hasOwn(S.cameraState(),'aperture')) {
          S.cameraAction('focus',v.cam.dist/100*0.55);S.cameraAction('aperture',2.8);const blurred=image();
          dof=different(original,blurred);dofError=v.gl.getError();
          const locked=S.cameraState().focusDistance;v.cam.zoom(1.2,0,0);v.later();focusLock=S.cameraState().focusDistance===locked;
          v.cam.zoom(1/1.2,0,0);S.cameraAction('aperture','off');S.cameraAction('focus-lock');
          S.cameraAction('projection','orthographic');const plain=image();S.cameraAction('aperture',2.8);const noLens=image();orthoSharp=!different(plain,noLens);
          S.cameraAction('aperture','off');S.cameraAction('projection','perspective');
        }
        const exposure=mean(brighter)>mean(original),reversible=!different(original,restored)&&!different(original,cooled),tint=different(original,thermal);
        const before=v.cam.dist*Math.tan(v.cam.fov/2);S.cameraAction('focal',250);const after=v.cam.dist*Math.tan(v.cam.fov/2);
        const scale=Math.abs(before-after)<1e-7;S.cameraAction('projection','orthographic');S.cameraAction('bottom');
        const ortho=image();S.cameraAction('top');const top=image();
        const orientation=different(ortho,top),focal=Math.abs(S.cameraState().focal-250)<1e-6;
        S.cameraAction('exposure',Infinity);const finite=S.cameraState().exposure===0;
        S.cameraAction('exposure',0.5);const saved=S.cameraState();const cam=JSON.stringify(v.cam),canvas=v.canvas;
        S.mount(null);S.mount(canvas);await new Promise(requestAnimationFrame);const reopened=JSON.stringify(S.testView.cam)===cam&&S.cameraState().exposure===saved.exposure;
        canvas.dataset.scope='other-thread';S.mount(canvas);S.cameraAction('exposure',-1);canvas.dataset.scope='';S.mount(canvas);
        const scoped=S.cameraState().exposure===saved.exposure;
        const oldPose=JSON.stringify(S.testView.cam);canvas.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
        const keyboard=JSON.stringify(S.testView.cam)!==oldPose;
        const geometry=count===S.testView.count&&JSON.stringify(box)===JSON.stringify(S.testView.box);
        let draws=0;const draw=S.testView.draw.bind(S.testView);S.testView.draw=()=>{draws++;draw()};
        await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);draws=0;await new Promise(r=>setTimeout(r,80));
        return {dof,focusLock,orthoSharp,dofError,exposure,reversible,tint,scale,orientation,focal,finite,reopened,geometry,scoped,projection,keyboard,selection:S.cameraState().selection?.ref==='C5',idle:draws===0,glError:S.testView.gl.getError(),provenance:S.cameraState().materialSource};
      });
      for(const [key,value]of Object.entries(receipt.checks))assert.equal(value,key==='provenance'?'color-estimate':key==='glError'||key==='dofError'?0:true,key);
    }
    if(variant==='after'&&name==='board') {
      await page.evaluate(()=>{window.S.cameraAction('iso');window.S.cameraAction('projection','perspective');window.S.cameraAction('focal',33);window.S.cameraAction('exposure',0);window.S.cameraAction('fit');window.S.cameraAction('focus',window.S.testView.cam.dist/100*0.85);window.S.cameraAction('aperture',5.6);window.S.testView.draw();});
      await page.locator('canvas').screenshot({path:`${out}/after-board-dof.png`});
      receipt.gpu=await page.evaluate(()=>{
        const S=window.S,v=S.testView,run=()=>{const times=[];for(let i=0;i<3;i++){const t=performance.now();v.draw();const pixels=new Uint8Array(v.canvas.width*v.canvas.height*4);v.gl.readPixels(0,0,v.canvas.width,v.canvas.height,v.gl.RGBA,v.gl.UNSIGNED_BYTE,pixels);times.push(performance.now()-t)}return times;};
        const dof=run();S.cameraAction('aperture','off');const sharp=run();return {sharpMs:sharp,dofMs:dof,framebufferBytes:v.dof.width*v.dof.height*10,width:v.dof.width,height:v.dof.height,renderer:v.gl.getParameter(v.gl.RENDERER)};
      });
    }
    await page.close();
  }
  // Two real depth planes with identical checker edges: focus one plane,
  // measure the other softening. This catches a decorative uniform blur.
  {
    const page=await browser.newPage({viewport:{width:1000,height:700},deviceScaleFactor:1});
    await page.goto(`http://127.0.0.1:${server.port}/`);
    receipt.depth=await page.evaluate(async()=>{
      const S=await import('/solid.js'),canvas=document.querySelector('canvas');S.mount(canvas);
      const bytes=[];let prev=[0,0,0],color=0,n=0;
      const put=v=>{do{const b=v%128;v=Math.floor(v/128);bytes.push(b|(v?128:0))}while(v)};
      const tri=(c,ps)=>{put((color^c)>>>0);color=c;n++;for(const p of ps)for(let i=0;i<3;i++){const d=p[i]-prev[i];put(d<0?-d*2-1:d*2);prev[i]=p[i]}};
      for(const [x0,z]of [[-2000,0],[100,1000]])for(let x=0;x<10;x++)for(let y=0;y<10;y++){
        const a=x0+x*190,b=-1000+y*200,c=(x+y)%2?0xffffff:0x050505;
        tri(c,[[a,b,z],[a+190,b,z],[a,b+200,z]]);tri(c,[[a+190,b,z],[a+190,b+200,z],[a,b+200,z]]);
      }
      await S.got({key:'2|board',n,mesh:new Uint8Array(bytes),box3:[-2000,-1000,0,2000,1000,1000]});
      S.cameraAction('top');S.cameraAction('fit');S.cameraAction('top');
      const v=S.testView,read=()=>{v.draw();const data=new Uint8Array(1000*700*4);v.gl.readPixels(0,0,1000,700,v.gl.RGBA,v.gl.UNSIGNED_BYTE,data);return data};
      const off=read();S.cameraAction('focus',(v.cam.dist+500)/100);S.cameraAction('aperture',5.6);const focused=read();
      const delta=(lo,hi)=>{let total=0;for(let y=240;y<460;y++)for(let x=lo;x<hi;x++){const i=(y*1000+x)*4;total+=Math.abs(off[i]-focused[i])+Math.abs(off[i+1]-focused[i+1])+Math.abs(off[i+2]-focused[i+2])}return total;};
      return {focusedDelta:delta(240,440),otherDelta:delta(560,760),error:v.gl.getError(),focus:S.cameraState().focusDistance};
    });
    assert.equal(receipt.depth.error,0);assert.ok(receipt.depth.otherDelta>receipt.depth.focusedDelta*2,JSON.stringify(receipt.depth));
    await page.locator('canvas').screenshot({path:`${out}/depth-planes.png`});await page.close();
  }
  {
    const page=await browser.newPage();const logs=[];page.on('console',m=>logs.push(m.text()));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    const fallback=await page.evaluate(async()=>{
      const original=WebGLRenderingContext.prototype.getExtension;
      WebGLRenderingContext.prototype.getExtension=function(name){return name==='WEBGL_depth_texture'?null:original.call(this,name)};
      const S=await import('/solid.js');S.mount(document.querySelector('canvas'));
      return !Object.hasOwn(S.cameraState(),'aperture');
    });
    assert.ok(fallback);assert.ok(logs.some(s=>s.includes('depth textures unavailable')));receipt.fallback=true;await page.close();
  }
  if(before)for(const [name]of fixtures)assert.notEqual(receipt.images.find(x=>x.name===name&&x.variant==='before').sha256,receipt.images.find(x=>x.name===name&&x.variant==='after').sha256);
  writeFileSync(`${out}/receipt.json`,JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt.checks));console.log(`ok actual studio WebGL and camera regressions; evidence ${out}`);
}finally{await browser.close();server.stop(true);}
