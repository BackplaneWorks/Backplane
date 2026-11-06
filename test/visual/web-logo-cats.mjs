// The web's logo and cats at 1440x900, for the visual check.
//   bun web-logo-cats.mjs HUB_URL TOKEN OUTDIR SCHEME SCALE
// Writes OUTDIR/web-<scheme>-<scale>x.png and .rects.json ({name: [x,y,w,h]} in px of the PNG).
// The hub's prebuilt web bundle predates #922 (light-mode logo), so the page's stylesheet and the logo
// files are served from src/web (route interception), i.e. today's app. The hub sets the theme (ui.theme).
import {chromium} from '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs';
import {readFileSync,writeFileSync} from 'node:fs';
const [hub,token,out,scheme,scaleArg]=process.argv.slice(2);const scale=Number(scaleArg||1);
const SRC=new URL('../../src/web/',import.meta.url).pathname;
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
const ctx=await browser.newContext({viewport:{width:1440,height:900},deviceScaleFactor:scale,colorScheme:scheme,serviceWorkers:'block'});
const page=await ctx.newPage();
await page.route(/chunk-[a-z0-9]+\.css$/,r=>r.fulfill({contentType:'text/css',body:readFileSync(SRC+'style.css')}));
for(const f of ['favicon-light.svg','favicon.svg','apple-touch-icon.png'])
  await page.route('**/'+f,r=>r.fulfill({contentType:f.endsWith('svg')?'image/svg+xml':'image/png',body:readFileSync(SRC+f)}));
await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(3500);
const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
await page.getByText('load 1',{exact:true}).first().click();await page.waitForTimeout(2500);
const rects={};
const logo=await page.locator('.topbar .brand-mark').first().boundingBox();if(logo)rects.logo=[logo.x,logo.y,logo.width,logo.height].map(v=>Math.round(v*scale));
const names=await page.locator('.bot-row').all();
for(const row of names){const n=(await row.locator('.bot-name').innerText()).trim();const b=await row.locator('img.cat').first().boundingBox();if(b)rects['cat:'+n]=[b.x,b.y,b.width,b.height].map(v=>Math.round(v*scale));}
await page.screenshot({path:`${out}/web-${scheme}-${scale}x.png`});
writeFileSync(`${out}/web-${scheme}-${scale}x.rects.json`,JSON.stringify(rects));
console.log(scheme,scale,JSON.stringify(rects));
await browser.close();
