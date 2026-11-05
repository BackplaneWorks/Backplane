// Web reference at the emulator's size (411x914 css px @2.625 = 1079x2399): bun android-web-ref.mjs HUB_URL TOKEN OUTDIR SCHEME
import {writeFileSync} from 'node:fs';
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
const [hub,token,out,scheme]=process.argv.slice(2);
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
const ctx=await browser.newContext({viewport:{width:411,height:914},deviceScaleFactor:2.625,isMobile:true,hasTouch:true,colorScheme:scheme});
const page=await ctx.newPage();const boxes={};const grab=async k=>{boxes[k]=await page.evaluate(()=>{const r=e=>{const b=e.getBoundingClientRect();return [Math.round(b.x*2.625),Math.round(b.y*2.625),Math.round(b.width*2.625),Math.round(b.height*2.625)]};return [...document.querySelectorAll('.brand-mark,img.cat')].map(e=>({kind:e.matches('.brand-mark')?'logo':'cat',name:e.closest('.bot-row,.bot-head,.post')?.querySelector('.bot-name,.bot-title,.post-from span')?.textContent||'',where:e.closest('.bot-head')?'header':e.closest('.bot-row')?'row':'other',rect:r(e)}))})};
await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
await page.waitForTimeout(2500);
await page.screenshot({path:`${out}/web-${scheme}-1-threads.png`});await grab('threads');
const bots=page.getByText('Bots',{exact:true});if(await bots.count()){await bots.first().click();await page.waitForTimeout(1500);}
await page.screenshot({path:`${out}/web-${scheme}-2-bots.png`});await grab('bots');
const b=page.getByText('Mochi',{exact:true});if(await b.count()){await b.first().click();await page.waitForTimeout(2500);}
await page.screenshot({path:`${out}/web-${scheme}-3-bot.png`});await grab('bot');writeFileSync(`${out}/web-${scheme}-boxes.json`,JSON.stringify(boxes,null,1));
await browser.close();
