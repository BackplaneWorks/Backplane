// Web reference at the emulator's size (411x914 css px @2.625 = 1079x2399): bun android-web-ref.mjs HUB_URL TOKEN OUTDIR SCHEME
// shots: 1-threads 2-bots 3-bot 4-mention 5-room 6-looks, and web-<scheme>-boxes.json (device px) of every logo and cat on them.
import {writeFileSync,readFileSync} from 'node:fs';
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
const [hub,token,out,scheme]=process.argv.slice(2);
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
const ctx=await browser.newContext({viewport:{width:411,height:914},deviceScaleFactor:2.625,isMobile:true,hasTouch:true,colorScheme:scheme});
const page=await ctx.newPage();const boxes={};
const grab=async k=>{boxes[k]=await page.evaluate(()=>{const r=e=>{const b=e.getBoundingClientRect();return [Math.round(b.x*2.625),Math.round(b.y*2.625),Math.round(b.width*2.625),Math.round(b.height*2.625)]};return [...document.querySelectorAll('.brand-mark,img.cat')].map(e=>({kind:e.matches('.brand-mark')?'logo':'cat',name:e.closest('.look-pick')?.dataset.value??(e.closest('.bot-row,.bot-head,.post')?.querySelector('.bot-name,.bot-title,.post-from span')?.textContent||''),where:e.closest('.bot-head')?'header':e.closest('.bot-row')?'row':e.closest('.post')?'post':e.closest('.look-pick')?'look':'other',rect:r(e)}))})};
const shot=async n=>{await page.screenshot({path:`${out}/web-${scheme}-${n}.png`});};
const tryClick=async l=>{try{await l.click({timeout:5000});return true}catch(e){console.log('click failed',String(e).split('\n')[0]);return false}};
// the hub's web bundle (build/gpui/web, 3 Nov) predates commit 80cc30e499; this is that commit's style.css rule, verbatim: html.light/.app.light --brand-image: url(favicon-light.svg)
const light=readFileSync(new URL('src/web/favicon-light.svg',import.meta.url));
await page.addInitScript(`document.addEventListener('DOMContentLoaded',()=>{const s=document.createElement('style');s.textContent='html.light .brand-mark,.app.light .brand-mark{background-image:url(data:image/svg+xml;base64,${light.toString('base64')}) !important}';document.head.appendChild(s)})`);
await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
await page.waitForTimeout(2500);
await shot('1-threads');await grab('threads');
await tryClick(page.getByText('Bots',{exact:true}).first());await page.waitForTimeout(1500);
await shot('2-bots');await grab('bots');
// a room and its posts
if(await tryClick(page.locator('.room-row').first())){await page.waitForTimeout(2500);await shot('5-room');await grab('room');await tryClick(page.locator('button.back').first());await page.waitForTimeout(1500);}
// a bot: header, the mention popup, the Settings page with the look picker
await tryClick(page.getByText('Mochi',{exact:true}).first());await page.waitForTimeout(2500);
await shot('3-bot');await grab('bot');
const box=page.getByPlaceholder(/Ask/).first();if(await box.count()){await box.click();await page.keyboard.type('@');await page.waitForTimeout(1500);await shot('4-mention');await box.fill('');await page.keyboard.press('Escape');await page.waitForTimeout(500);}
if(await tryClick(page.getByText('Pages',{exact:true}).first())){await page.waitForTimeout(500);await tryClick(page.locator('button').filter({hasText:/^Settings$/}).first());await page.waitForTimeout(1800);}
const lk=page.locator('.looks').first();if(await lk.count())await lk.scrollIntoViewIfNeeded();await page.waitForTimeout(800);
await shot('6-looks');await grab('looks');
writeFileSync(`${out}/web-${scheme}-boxes.json`,JSON.stringify(boxes,null,1));
await browser.close();
