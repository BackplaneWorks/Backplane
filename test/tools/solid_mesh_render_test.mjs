// Headless only: exercise the production decoder, camera and WebGL draw path.
// Requires tools/browser's playwright-core and CHROME_BIN (or installed Chromium).
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH||'tools/browser/node_modules/playwright-core/index.mjs')).href);
const out=process.env.SOLID_RENDER_OUT||'build/solid-handedness';
mkdirSync(out,{recursive:true});
const source=process.env.SOLID_RENDER_SOURCE||'src/web/solid.js';
const solid=readFileSync(source,'utf8');
const fixture=JSON.parse(readFileSync('test/fixtures/solid-handedness/wire.json','utf8'));
const wire={n:fixture.n,mesh:[...Buffer.from(fixture.mesh_base64,'base64')]};
assert.equal(createHash('sha256').update(readFileSync(fixture.source)).digest('hex'),fixture.source_sha256);
assert.equal(createHash('sha256').update(Buffer.from(wire.mesh)).digest('hex'),fixture.mesh_sha256);
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
  const p=new URL(req.url).pathname;
  // Only expose camera configuration to the test. The renderer body is exact
  // production source; App only supplies the localized progress strings.
  if(p==='/solid.js')return new Response(solid+'\nexport {view as testView};',{headers:{'Content-Type':'application/javascript'}});
  if(p==='/solid-assets.js')return new Response(readFileSync('src/web/solid-assets.js','utf8'),{headers:{'Content-Type':'application/javascript'}});
  if(p==='/solid-framing.js')return new Response(readFileSync('src/web/solid-framing.js','utf8'),{headers:{'Content-Type':'application/javascript'}});
  if(p==='/solid-dof.js')return new Response(readFileSync('src/web/solid-dof.js','utf8'),{headers:{'Content-Type':'application/javascript'}});
  if(p==='/solid-materials.js')return new Response(readFileSync('src/web/solid-materials.js','utf8'),{headers:{'Content-Type':'application/javascript'}});
  if(p==='/app.bend')return new Response('export default {solid_waiting:()=>"",solid_stats:()=>""};',{headers:{'Content-Type':'application/javascript'}});
  if(p==='/top.png'||p==='/bottom.png')return new Response(readFileSync(`test/fixtures/solid-handedness${p}`),{headers:{'Content-Type':'image/png'}});
  return new Response('<style>html,body{margin:0;background:#202128}canvas{width:1000px;height:700px;display:block}</style><canvas id="model" data-key="2|fixture"></canvas>',{headers:{'Content-Type':'text/html'}});
}});
const executablePath=process.env.CHROME_BIN||['/usr/bin/google-chrome-stable','/usr/bin/chromium','/usr/bin/chromium-browser'].find(existsSync);
assert.ok(executablePath,'set CHROME_BIN to Chromium');
const browser=await chromium.launch({executablePath,headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader']});
try{
  const page=await browser.newPage({viewport:{width:1000,height:700},deviceScaleFactor:1});
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.evaluate(async wire=>{
    window.S=await import('/solid.js');window.S.mount(document.querySelector('canvas'));
    window.S.got({key:'2|fixture',n:wire.n,mesh:new Uint8Array(wire.mesh)});
  },wire);
  await page.waitForFunction(()=>window.S.testView.count>0);
  const scores={};
  for(const bottom of [false,true]){
    const name=bottom?'bottom':'top';
    scores[name]=await page.evaluate(async({bottom,name})=>{
      const image=await createImageBitmap(await(await fetch(`/${name}.png`)).blob());
      const ref=new OffscreenCanvas(1000,700),r=ref.getContext('2d');r.drawImage(image,0,0);
      const expected=r.getImageData(0,0,1000,700).data;
      const v=window.S.testView;v.fit();v.cam.aim(bottom?Math.PI:0,bottom?-Math.PI/2:Math.PI/2);v.draw();
      const copy=new OffscreenCanvas(1000,700),c=copy.getContext('2d');c.drawImage(v.canvas,0,0);
      const actual=c.getImageData(0,0,1000,700).data;
      const white=(a,i)=>a[i]>220&&a[i+1]>220&&a[i+2]>220&&a[i+3]>128;
      let union=0,both=0;
      for(let i=0;i<actual.length;i+=4){const a=white(actual,i),b=white(expected,i);if(a||b)union++;if(a&&b)both++;}
      return {iou:both/union,union};
    },{bottom,name});
    await page.locator('canvas').screenshot({path:`${out}/${name}.png`});
    assert.ok(scores[name].union>500,`${name} has no silk pixels`);
    assert.ok(scores[name].iou>0.95,`${name} silk orientation/registration differs: ${JSON.stringify(scores[name])}`);
    console.log(`ok actual WebGL ${name} silk pixels match reviewed KiCad-oriented fixture (IoU ${scores[name].iou})`);
  }
  assert.deepEqual(errors,[]);
  writeFileSync(`${out}/receipt.json`,JSON.stringify({source,solid_sha256:createHash('sha256').update(solid).digest('hex'),wire_sha256:fixture.mesh_sha256,browser:await browser.version(),scores},null,2)+'\n');
}finally{await browser.close();server.stop(true);}
