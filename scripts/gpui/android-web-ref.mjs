// Web reference at the emulator's size (411x914 css px @2.625 = 1079x2399): bun android-web-ref.mjs HUB_URL TOKEN OUTDIR SCHEME
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
const [hub,token,out,scheme]=process.argv.slice(2);
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
const ctx=await browser.newContext({viewport:{width:411,height:914},deviceScaleFactor:2.625,isMobile:true,hasTouch:true,colorScheme:scheme});
const page=await ctx.newPage();
await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
await page.waitForTimeout(2500);
await page.screenshot({path:`${out}/web-${scheme}-1-threads.png`});
const bots=page.getByText('Bots',{exact:true});if(await bots.count()){await bots.first().click();await page.waitForTimeout(1500);}
await page.screenshot({path:`${out}/web-${scheme}-2-bots.png`});
const b=page.getByText('Mochi',{exact:true});if(await b.count()){await b.first().click();await page.waitForTimeout(2500);}
await page.screenshot({path:`${out}/web-${scheme}-3-bot.png`});
await browser.close();
