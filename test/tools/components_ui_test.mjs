// Actual parser output and production inspector/PCB selection in an isolated
// browser. Renderer projection inverses are covered by the solid renderer tests.
import assert from 'node:assert/strict';
import {readFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {data} from './components_test.mjs';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs')));
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
 const path=new URL(req.url).pathname;
 if(path==='/app.bend')return new Response('export default {};',{headers:{'Content-Type':'application/javascript'}});
 if(['/components.js','/plot2d.js','/solid.js'].includes(path))return new Response(readFileSync('src/web'+path),{headers:{'Content-Type':'application/javascript'}});
 return new Response(`<style>*{box-sizing:border-box}body{margin:0;background:#121619;color:#e0e4de;font:14px sans-serif}main{display:grid;grid-template-columns:1fr 340px;height:100vh}#stages{display:grid;grid-template-rows:1fr 1fr;min-width:0}.viewer-stage{position:relative;min-height:0}canvas{width:100%;height:100%;display:block;touch-action:none}aside{overflow:auto;padding:10px}input{width:100%}table{width:100%;border-collapse:collapse;word-break:break-word}td,th{padding:4px;text-align:left}tr[aria-selected=true]{outline:2px solid #d2f05a}.component-list{max-height:180px;overflow:auto}dd{margin:0;overflow-wrap:anywhere}p{overflow-wrap:anywhere}svg{color:#e0e4de}.component-head{display:flex;justify-content:space-between}.component-limit{color:#b6bead}@media(max-width:600px){main{grid-template-columns:1fr;grid-template-rows:50vh 50vh}}</style><main><div id="stages"><div class="viewer-stage"><canvas id="plot" data-key="0|root|" data-fade="0"></canvas></div><div class="viewer-stage"><canvas id="solid"></canvas></div></div><aside id="inspector"></aside></main>`,{headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_BIN||'/usr/bin/google-chrome-stable'});
try {
 const page=await browser.newPage({viewport:{width:1000,height:800}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));page.setDefaultTimeout(5000);
 await page.goto(`http://127.0.0.1:${server.port}/`);
 await page.evaluate(async data=>{
   window.C=await import('/components.js');window.P=await import('/plot2d.js');window.store=new C.ComponentStore('0|root|',data);
   window.cleanInspector=C.mountInspector(document.querySelector('#inspector'),store);
   P.got({key:'0|root|',box:[10000,20000,50000,60000],cs:[]});P.mount(document.querySelector('#plot'));
   P.setComponents(store.key,data,id=>store.select(id));store.subscribe(c=>P.selectComponent(c?.id||'',store.key));
   // An affine projection mock only exercises the overlay/controller contract;
   // it is explicitly not a numerical test of the production WebGL camera.
   window.cleanSolid=C.attachSolid(document.querySelector('#solid'),store,{projectPoint:([x,y,z])=>({x:x*10,y:y*5,depth:100-z,visible:true}),rayToBoard:(x,y,z)=>[x/10,y/5,z]});
 },data);
 await page.getByRole('button',{name:'R1',exact:true}).click();
 assert.equal(await page.evaluate(()=>store.current().id),'resistor');
 await page.getByRole('img',{name:'R1 saved footprint, numbered pads'}).waitFor();await page.getByRole('img',{name:'R1 source schematic symbol'}).waitFor();
 assert.match(await page.locator('#inspector').innerText(),/SOURCE-MPN/);assert.equal(await page.locator('tr[aria-selected=true]').getAttribute('data-component'),'resistor');
 await page.locator('.component-selection-overlay polygon').first().waitFor();
 await page.getByRole('button',{name:'J2',exact:true}).click();assert.match(await page.locator('#inspector').innerText(),/Symbol unavailable/);assert.equal(await page.locator('#inspector not-html').count(),0,'MPN must be text, never markup');
 // Actual PCB canvas inverse transform: fit 40x40mm box in canvas with .9 margin.
 const p=await page.locator('#plot').boundingBox();const scale=.9*Math.min(p.width/40000,p.height/40000),px=p.x+p.width/2+(20000-30000)*scale,py=p.y+p.height/2+(28000-40000)*scale;
 await page.mouse.click(px,py);assert.equal(await page.evaluate(()=>store.current()?.id),'resistor','PCB copper pad click selects actual source UUID');
 await page.mouse.click(px,py+2000*scale);assert.equal(await page.evaluate(()=>store.current()),null,'empty footprint gap clears instead of bounding-box selecting');
 // Exact contract point on a saved top-side pad, not a rendered model triangle.
 const s=await page.locator('#solid').boundingBox();await page.mouse.click(s.x+200,s.y+140);assert.equal(await page.evaluate(()=>store.current()?.id),'resistor');
 await page.getByRole('button',{name:'Clear',exact:true}).click();assert.equal(await page.evaluate(()=>store.current()),null);
 await page.getByRole('searchbox',{name:'Find saved component'}).fill('USB-C');assert.equal(await page.locator('.component-bom tbody tr').count(),1);assert.equal(await page.getByRole('searchbox').inputValue(),'USB-C');
 await page.getByRole('searchbox').fill('');await page.getByRole('button',{name:'R1',exact:true}).click();
 mkdirSync('/tmp/backplane-reference-audit/component-ui',{recursive:true});await page.screenshot({path:'/tmp/backplane-reference-audit/component-ui/desktop.png'});
 await page.setViewportSize({width:390,height:844});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.screenshot({path:'/tmp/backplane-reference-audit/component-ui/mobile.png'});
 await page.evaluate(()=>store.setData(store.key,{...store.data,sourceRevision:'changed'}));assert.equal(await page.evaluate(()=>store.current()),null);
 await page.evaluate(()=>{cleanInspector();cleanSolid();});assert.equal(await page.locator('.component-selection-overlay').count(),0);
 assert.deepEqual(errors,[]);console.log('ok production saved-component UI: BOM/2D-pad/3D-footprint-location shared UUID selection, actual symbol and numbered footprint, XSS-safe fields, filter, revision invalidation, responsive width and cleanup');
} finally {await browser.close();server.stop(true);}
