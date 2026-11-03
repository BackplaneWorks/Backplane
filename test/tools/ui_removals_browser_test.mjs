// Actual production Bend page at desktop/390 px, with synthetic quota and
// connection states. This is a UI regression fixture, never live account data.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const out=path.resolve(process.env.UI_REMOVALS_OUT||'build/ui-removals');
const lib={};for(const version of ['before','after'])lib[version]=(await import(pathToFileURL(path.join(out,`page-${version}.mjs`)).href)).default;
const escape=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
function html(n){if(n.type==='text')return escape(n.text);const as=n.attrs.map(x=>x.event?`data-on-${escape(x.event)}="${escape(x.action)}"`:`${escape(x.name)}="${escape(x.value)}"`);return `<${n.tag} ${as.join(' ')}>${n.tag==='textarea'?escape(n.attrs.find(a=>a.name==='value')?.value||''):n.kids.map(html).join('')}</${n.tag}>`;}
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
 const u=new URL(req.url),version=u.searchParams.get('version')==='before'?'before':'after',scenario=u.searchParams.get('scenario')||'allowance';
 if(u.pathname==='/style.css')return new Response(fs.readFileSync(version==='before'?path.join(out,'style-before.css'):'src/web/style.css'),{headers:{'content-type':'text/css'}});
 if(u.pathname==='/composer-size.js')return new Response(fs.readFileSync('src/web/composer-size.js'),{headers:{'content-type':'text/javascript'}});
 if(u.pathname==='/desktop-browser.js')return new Response(fs.readFileSync(version==='before'?path.join(out,'desktop-browser-before.js'):'src/web/desktop-browser.js'),{headers:{'content-type':'text/javascript'}});
 const tree=JSON.parse(lib[version].page(scenario));
 return new Response(`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css?version=${version}"><body><div id="root">${html(tree)}</div><script type="module">
 import {sizeComposer,sizeComposerControls} from '/composer-size.js';
 sizeComposerControls();const field=document.querySelector('#composer');if(field)sizeComposer(field);
 import {installDesktopBrowser} from '/desktop-browser.js?version=${version}';
 window.fixtureController=installDesktopBrowser({bridge:{state:async()=>({phase:'error',message:'Browser unavailable · reconnecting automatically…'}),onState:()=>()=>{},hide:()=>{},present:async()=>({shown:false,phase:'error',message:'Browser unavailable · reconnecting automatically…'})}});
 window.fixtureReady=true;
 </script></body></html>`,{headers:{'content-type':'text/html'}});
}});
const {chromium}=await import(pathToFileURL(path.resolve(process.env.PLAYWRIGHT_CORE_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs')).href);
const browser=await chromium.launch({executablePath:process.env.CHROME_BIN||'/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--disable-dev-shm-usage']});
const evidence=[],errors=[];let page;
try{
 page=await browser.newPage();page.on('pageerror',e=>errors.push(e.message));
 for(const version of ['before','after'])for(const width of [1440,390])for(const scenario of ['allowance','unknown','browser']){
  await page.setViewportSize({width,height:width===1440?960:844});
  await page.goto(`http://127.0.0.1:${server.port}/?version=${version}&scenario=${scenario}`);
  await page.waitForFunction(()=>window.fixtureReady);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const text=await page.locator('body').innerText();
  const chip=page.locator('#allowance-chip');
  if(scenario==='unknown')assert.equal(await chip.count(),version==='before'?1:0);
  else assert.equal((await chip.locator('.allowance-current').textContent()).trim(),version==='before'?'Limits stale':'39% left · shared');
  if(version==='after'){
   assert.equal(await page.locator('[data-desktop-browser-reconnect]').count(),0);
   assert(!/Limits unknown|Limits stale|presentation has expired/.test(text));
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'no horizontal page overflow');
   const bar=await page.locator('.composer-foot').evaluate(el=>({height:el.getBoundingClientRect().height,button:parseFloat(getComputedStyle(el).getPropertyValue('--composer-button'))}));
   assert(bar.height<=bar.button+1,'one composer control row');
  }else assert.equal(await page.locator('[data-desktop-browser-global-reconnect]').isVisible(),true);
  if(width===390&&scenario!=='browser'){
   const summary=page.locator('.composer-more > summary');if(await summary.isVisible())await summary.click();
   if(scenario!=='unknown'&&await chip.isVisible())assert(await chip.evaluate(el=>el.scrollWidth<=el.clientWidth),'shared-pool phone label fits');
  }
  const file=`${version}-${width}-${scenario}.png`;await page.screenshot({path:path.join(out,file)});evidence.push({version,width,scenario,file});
 }
 assert.deepEqual(errors,[]);fs.writeFileSync(path.join(out,'screenshots.json'),JSON.stringify({result:'PASS',scope:'Production Bend page/stylesheet and desktop renderer with synthetic allowance and disconnected state; not an installed/live-account screenshot',evidence,errors},null,2)+'\n');
 console.log('ok before/after desktop and 390px: no reconnect control, last known limits, empty hidden chip, one composer row and readable phone label');
}catch(error){if(page){await page.screenshot({path:path.join(out,'screenshot-failure.png')});fs.writeFileSync(path.join(out,'screenshot-failure.html'),await page.content());}throw error;}finally{await browser.close();server.stop(true);}
