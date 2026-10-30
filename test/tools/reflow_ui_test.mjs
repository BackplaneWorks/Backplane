// The production UI plus the actual emitted Bend numeric planner, isolated
// headless browser. Camera hooks are observed here; renderer has its own tests.
import assert from 'node:assert/strict';
import {readFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs')));
const out='build/viewer/reflow-ui';mkdirSync(out,{recursive:true});
const types={js:'application/javascript',css:'text/css',mjs:'application/javascript'};
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
 const path=new URL(req.url).pathname;
 if(path==='/planner.mjs')return new Response(readFileSync('build/viewer/reflow.mjs'),{headers:{'Content-Type':types.js}});
 if(path==='/web/app.bend')return new Response("import R from '/planner.mjs'; export default {reflow_defaults:R.defaults,reflow_alloys:R.alloys,reflow_plan:R.plan};",{headers:{'Content-Type':types.js}});
 if(path==='/web/reflow.js'||path==='/web/viewer-tools.js'||path==='/web/style.css'||path==='/web/components.js'||path==='/web/viewer-quality.js')return new Response(readFileSync('src'+path),{headers:{'Content-Type':types[path.split('.').at(-1)]}});
 return new Response('<link rel="stylesheet" href="/web/style.css"><style>body{height:100vh}.solid-workbench{height:100vh}.viewer-stage{min-height:200px}</style><div class="solid-workbench" id="solid-workbench" data-scope="isolated" data-key="2|real|board"><div class="solid-toolbar"></div><div class="solid-layout"><div class="viewer-stage"></div><aside class="solid-inspector" id="solid-inspector"></aside></div></div>',{headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_BIN||'/usr/bin/google-chrome-stable'});
try{
 const page=await browser.newPage({viewport:{width:1100,height:800},reducedMotion:'reduce'});page.setDefaultTimeout(5000);const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${server.port}/`);
 await page.evaluate(async()=>{window.calls=[];window.Tools=await import('/web/viewer-tools.js');window.solid={cameraAction:(...a)=>calls.push(a),cameraState:()=>({projection:'perspective',focal:50,exposure:0}),setThermal:o=>window.thermal=o};Tools.mount(document.querySelector('#solid-workbench'),solid);});
 await page.getByRole('button',{name:'Top',exact:true}).click();assert.deepEqual(await page.evaluate(()=>calls.at(-1)),['top',undefined]);
 await page.getByRole('button',{name:'Reflow',exact:true}).click();
 await page.getByText('Sn 96.5 · Ag 3 · Cu 0.5 wt%',{exact:false}).waitFor();
 const slider=page.getByRole('slider',{name:'Reflow replay time in seconds'});
 await slider.fill('240');assert.match(await page.locator('.reflow-readings').innerText(),/100.0%/);
 await page.getByLabel('Illustrative heat overlay').check();assert.equal(await page.evaluate(()=>thermal.enabled),true);assert.equal(await page.evaluate(()=>thermal.liquid),1);
 await page.getByText('Edit profile points',{exact:true}).click();
 await page.getByRole('spinbutton',{name:'Point 2 Seconds',exact:true}).fill('0');await page.getByRole('button',{name:'Apply',exact:true}).click();assert.match(await page.getByRole('alert').innerText(),/strictly increasing/);
 assert.match(await page.locator('.reflow-controls output').innerText(),/240.0/); // invalid edits do not corrupt active preview
 await page.getByRole('button',{name:'Reset',exact:true}).click();
 await page.getByRole('combobox',{name:'Solder alloy',exact:true}).selectOption('Sn42Bi58');assert.match(await page.locator('.solid-inspector').innerText(),/138 °C/);
 await slider.fill('330');await page.getByRole('button',{name:'Play',exact:true}).click();await page.waitForTimeout(160);await page.getByRole('button',{name:'Pause',exact:true}).click();assert((await slider.inputValue())<330);
 const before=await slider.inputValue();await page.waitForTimeout(150);assert.equal(await slider.inputValue(),before);
 await page.getByRole('button',{name:'Reflow',exact:true}).click();assert.equal(await page.evaluate(()=>thermal.enabled),false);
 await page.getByRole('button',{name:'Reflow',exact:true}).click();assert.equal(await page.getByRole('combobox',{name:'Solder alloy'}).inputValue(),'Sn42Bi58');
 await page.screenshot({path:`${out}/desktop.png`});
 await page.setViewportSize({width:390,height:844});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await page.screenshot({path:`${out}/mobile.png`});
 await page.getByText('Edit profile points',{exact:true}).click();
 const download=page.waitForEvent('download');await page.getByRole('button',{name:'Export JSON'}).click();assert.equal((await download).suggestedFilename(),'reflow-preview.json');
 await page.getByRole('button',{name:'Play',exact:true}).click();await page.evaluate(()=>Tools.mount(null,solid));assert.equal(await page.evaluate(()=>thermal.enabled),false);
 assert.deepEqual(errors,[]);console.log('ok production reflow UI: linked graph/scrub/alloy/edit validation/play/pause/reset/export, retained session, mobile width, reduced-motion, teardown');
}finally{await browser.close();server.stop(true);}
