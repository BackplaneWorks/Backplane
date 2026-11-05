// Reference shots of the web client at phone size, for the iOS logo/cats comparison.
// usage: bun web-phone-shots.mjs HUB_URL TOKEN OUTDIR  -> web-phone-{dark,light}-{list,bots,bot}.png (393x852 @3x)
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
const [hub,token,out]=process.argv.slice(2);
for (const scheme of ['dark','light']) {
  const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
  const ctx=await browser.newContext({viewport:{width:393,height:852},deviceScaleFactor:3,isMobile:true,hasTouch:true,colorScheme:scheme});
  const page=await ctx.newPage();
  await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(3500);
  const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
  await page.waitForTimeout(800);
  await page.screenshot({path:`${out}/web-phone-${scheme}-list.png`});
  const bots=page.getByText('Bots',{exact:true});if(await bots.count()){await bots.first().click();await page.waitForTimeout(1500);}
  await page.screenshot({path:`${out}/web-phone-${scheme}-bots.png`});
  const bub=page.getByText('Bub',{exact:true});if(await bub.count()){await bub.first().click();await page.waitForTimeout(2000);}
  await page.screenshot({path:`${out}/web-phone-${scheme}-bot.png`});
  await browser.close();
}
