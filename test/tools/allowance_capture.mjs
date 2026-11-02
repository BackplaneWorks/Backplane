// Actual Chromium render of the production Bend page/composer and actions.
// Inputs are explicit synthetic quotas, never represented as live account use.
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const sha=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const out=resolve(process.env.ALLOWANCE_VISUAL_OUT||'build/allowance-recovery/visual');mkdirSync(out,{recursive:true});
process.env.ALLOWANCE_PREVIEW_PORT='4043';
await import('./allowance_preview_v2.mjs');
const {chromium}=await import(pathToFileURL(process.env.PLAYWRIGHT_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs').href);
const browser=await chromium.launch({executablePath:process.env.CHROME_BIN||'/usr/bin/google-chrome',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1440,height:1000}});const evidence=[];
try {
 await page.goto('http://127.0.0.1:4043');
 for (const width of [1440,390,320]) {
  await page.setViewportSize({width,height:width===1440?1000:844});
  await page.evaluate(()=>window.allowanceFixture('base'));
  await page.locator('#allowance-chip').waitFor();
  assert.equal(await page.locator('#allowance-chip').innerText(),'39% left · shared');
  const bar=await page.locator('.composer-model').evaluate(el=>({group:el.getBoundingClientRect().toJSON(),buttons:[...el.querySelectorAll('button')].map(b=>({text:b.innerText,box:b.getBoundingClientRect().toJSON()}))}));
  assert.equal(bar.buttons.length,3);assert(bar.buttons.every(b=>b.box.x>=0&&b.box.right<=width+1));
  assert(bar.buttons[0].box.width>=120);assert(bar.buttons[2].box.width>0);
  await page.screenshot({path:resolve(out,`${width}-closed.png`)});
  await Promise.all([page.waitForResponse(r=>r.url().endsWith('/act')&&JSON.parse(r.request().postData()||'{}').action==='draft'),page.locator('#composer').fill('Draft 日本語 👩🏽‍💻')]);
  await page.locator('#allowance-chip').click();
  await page.locator('#allowance-details').waitFor();
  assert.equal(await page.locator('#composer').inputValue(),'Draft 日本語 👩🏽‍💻');
  assert((await page.locator('#allowance-details').innerText()).includes('allocation per model not reported'));
  assert((await page.locator('#allowance-details').innerText()).includes('72% left'));
  const pop=await page.locator('#allowance-details').evaluate(el=>({box:el.getBoundingClientRect().toJSON(),overflow:el.scrollHeight>el.clientHeight}));
  assert(pop.box.x>=0&&pop.box.right<=width+1&&pop.box.y>=0&&pop.box.bottom<(width===1440?1000:844));
  await page.screenshot({path:resolve(out,`${width}-details.png`)});
  await page.locator('#allowance-details').evaluate(el=>el.scrollTop=el.scrollHeight);
  const check=await page.locator('[data-on-click="allowance-read"]').boundingBox();assert(check.y>=pop.box.y&&check.y+check.height<=pop.box.bottom);
  await page.keyboard.press('Escape');await page.locator('#allowance-details').waitFor({state:'detached'});
  assert.equal(await page.evaluate(()=>document.activeElement.id),'allowance-chip');
  assert.equal(await page.locator('#composer').inputValue(),'Draft 日本語 👩🏽‍💻');
  const heights=[];for(const scenario of ['stale','unknown','reset']) {
   await page.evaluate(s=>window.allowanceFixture(s),scenario);
   const label=await page.locator('#allowance-chip').innerText();assert.equal(label,scenario==='unknown'?'Limits unknown':'Limits stale');
   heights.push(await page.locator('.composer-model').evaluate(el=>el.getBoundingClientRect().height));
   await page.locator('#allowance-chip').click();await page.locator('#allowance-details').waitFor();
   await page.screenshot({path:resolve(out,`${width}-${scenario}.png`)});
  }
  assert(heights.every(h=>Math.abs(h-heights[0])<1));assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
  evidence.push({width,bar,pop,stableStateHeights:heights});
 }
 const receipt={result:'PASS',scope:'Chromium rendered production Bend page/composer with synthetic snapshots and real Bend UI actions; not installed app or live provider data',fixtureSha256:sha(process.argv[2]),styleSha256:sha('src/web/style.css'),keyboardSha256:sha('src/web/workspace-keys.js'),captureScriptSha256:sha('test/tools/allowance_capture.mjs'),evidence};
 writeFileSync(resolve(out,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');
 console.log('ok production composer rendered at 1440,390,320 widths; details, shared scope, drafts, Escape/focus, stale/unknown/reset and stable height');
} finally {await browser.close();}
process.exit(0);
