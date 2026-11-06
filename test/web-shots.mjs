// Reference screenshots of the web UI at 1440x900 (usage: bun web-shots.mjs HUB_URL TOKEN OUTDIR)
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
const [hub,token,out]=process.argv.slice(2);
for (const scheme of ['dark','light']) {
  const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
  const page=await (await browser.newContext({viewport:{width:1440,height:900},colorScheme:scheme})).newPage();
  await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(3000);
  const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
  await page.waitForTimeout(500);await page.screenshot({path:`${out}/web-${scheme}-1-sidebar.png`});
  await page.getByText('load 1',{exact:true}).first().click();await page.waitForTimeout(2500);
  await page.screenshot({path:`${out}/web-${scheme}-2-thread.png`});
  const box=page.getByPlaceholder(/Ask anything/).first();await box.click();await page.keyboard.type('/');await page.waitForTimeout(1500);
  await page.screenshot({path:`${out}/web-${scheme}-3-popup.png`});
  await browser.close();
}
