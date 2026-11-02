// Actual Bend tour DOM and production CSS; loopback-free isolated Chromium.
import assert from 'node:assert/strict';
import {readFileSync, existsSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const fixture = JSON.parse(lib.fixture());
const pw = process.env.PLAYWRIGHT_CORE_PATH || ['tools/browser/node_modules/playwright-core/index.mjs', '/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs'].find(existsSync);
assert.ok(pw, 'playwright-core is required for viewport acceptance');
const {chromium} = await import(pathToFileURL(resolve(pw)).href);
const executablePath = process.env.CHROME_BIN || ['/usr/bin/google-chrome-stable', '/usr/bin/chromium'].find(existsSync);
assert.ok(executablePath, 'Chromium is required for viewport acceptance');
const css = readFileSync('src/web/style.css', 'utf8');
const out = 'build/telemetry-optout/viewport'; mkdirSync(out, {recursive:true});
const browser = await chromium.launch({executablePath, headless:true});
const facts=[];
try {
  for (const [width,height,scale] of [[320,568,100],[390,844,100],[320,568,200],[390,844,200]]) {
    const page = await browser.newPage({viewport:{width,height}});
    await page.setContent('<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>'+css+'</style>');
    await page.evaluate(({fixture,scale})=>{
      document.documentElement.style.setProperty('--text-scale',String(scale));
      function node(n) {
        if(n.type==='text') return document.createTextNode(n.text);
        const el=document.createElement(n.tag);
        for(const a of n.attrs||[]) {
          if(a.name) el.setAttribute(a.name,a.value);
          else if(a.event) {el.dataset.action=a.action;el.addEventListener(a.event,()=>window.actions.push([a.action,el.dataset.value||'']));}
        }
        for(const child of n.kids||[]) el.append(node(child));
        return el;
      }
      window.actions=[];document.body.append(node(fixture));
    },{fixture,scale});
    const controls=await page.evaluate(()=>{
      const off=document.querySelector('.telemetry-off'),on=document.querySelector('.telemetry-tour .primary'),copy=document.querySelector('.telemetry-tour .tour-body');
      const hit=el=>{if(!el)return false;const r=el.getBoundingClientRect();return r.width>0&&r.height>=44&&r.left>=0&&r.top>=0&&r.right<=innerWidth&&r.bottom<=innerHeight&&document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)===el;};
      const initialHeight=copy.clientHeight;
      copy.scrollTop=copy.scrollHeight;
      return {off:hit(off),on:hit(on),disclosureHeight:initialHeight,disclosureScrolls:copy.scrollHeight>copy.clientHeight,disclosureEndReachable:copy.scrollTop+copy.clientHeight>=copy.scrollHeight-1,offText:off?.textContent,onText:on?.textContent};
    });
    assert.ok(controls.off&&controls.on,`${width}x${height}@${scale}%: both Off and Continue visible and hittable: ${JSON.stringify(controls)}`);
    assert.ok(controls.disclosureHeight>0&&controls.disclosureEndReachable,'complete disclosure retains positive height and its final text is reachable');
    const focus=[];
    for(let i=0;i<8;i++){await page.keyboard.press('Tab');focus.push(await page.evaluate(()=>document.activeElement.textContent));if(focus.at(-1)?.startsWith('Continue'))break;}
    assert.ok(focus.indexOf('Turn off all telemetry')>=0&&focus.indexOf('Turn off all telemetry')<focus.findIndex(x=>x.startsWith('Continue')), 'keyboard reaches Off before default Continue');
    await page.locator('.telemetry-off').click();
    assert.deepEqual(await page.evaluate(()=>window.actions.at(-1)),['share','off']);
    await page.locator('.telemetry-tour .primary').click();
    assert.deepEqual(await page.evaluate(()=>window.actions.at(-1)),['ob-next','']);
    await page.locator('.telemetry-later').click();
    assert.deepEqual(await page.evaluate(()=>window.actions.at(-1)),['ob-skip','']);
    await page.screenshot({path:`${out}/${width}-${height}-${scale}.png`});
    facts.push({width,height,scale,...controls,focus});console.log(`ok telemetry popup ${width}x${height} ${scale}% visible Off/default/keyboard/dispatch`);
    await page.close();
  }
  writeFileSync(`${out}/receipt.json`,JSON.stringify({browser:await browser.version(),cssSHA256:createHash('sha256').update(css).digest('hex'),fixtureSHA256:createHash('sha256').update(JSON.stringify(fixture)).digest('hex'),facts},null,2)+'\n');
} finally {await browser.close();}
