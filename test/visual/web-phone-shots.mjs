// Reference shots of the web client for the iOS logo/cats comparison, with the rectangles of the logo and
// every cat (CSS px x DPR = shot px). usage: bun web-phone-shots.mjs HUB_URL TOKEN OUTDIR SCHEME [phone|tablet]
// (set ui.theme on the hub first: test/set-setting.mjs). Writes web-<profile>-<scheme>-{list,bots,bot}.png and .json
// env PW (path of playwright-core's index.mjs) and CHROME (a chromium binary) default to the desktop's
const {chromium}=await import(process.env.PW||'/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs');
import {writeFileSync} from 'node:fs';
const [hub,token,out,scheme='dark',profile='phone']=process.argv.slice(2);
const vp=profile==='tablet'?{width:1024,height:1366,deviceScaleFactor:2}:{width:393,height:852,deviceScaleFactor:3};
const browser=await chromium.launch({executablePath:process.env.CHROME||'/usr/bin/google-chrome',args:['--no-sandbox','--disable-gpu']});
const ctx=await browser.newContext({viewport:{width:vp.width,height:vp.height},deviceScaleFactor:vp.deviceScaleFactor,isMobile:profile==='phone',hasTouch:true,colorScheme:scheme});
let page=await ctx.newPage();
await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
const skip=page.getByText('Skip',{exact:true});if(await skip.count())await skip.first().click();
await page.waitForTimeout(2500);
async function shot(name){
  await page.screenshot({path:`${out}/web-${profile}-${scheme}-${name}.png`});
  const items=await page.evaluate((k)=>{
    const r=(e,n,kind)=>{const b=e.getBoundingClientRect();return {name:n,kind,rect:[b.x*k,b.y*k,b.width*k,b.height*k]}};
    const o=[];
    document.querySelectorAll('.brand-mark').forEach((e,i)=>{const b=e.getBoundingClientRect();if(b.width>0&&b.x>=0)o.push(r(e,'brand','logo'))});
    document.querySelectorAll('img.cat').forEach((e,i)=>{const b=e.getBoundingClientRect();if(b.width>0&&b.y>=0)o.push(r(e,'cat:'+(e.dataset.cat||'')+':'+Math.round(b.width),'cat'))});
    return o;},vp.deviceScaleFactor);
  writeFileSync(`${out}/web-${profile}-${scheme}-${name}.json`,JSON.stringify(items,null,1));
}
await shot('list');
const bots=page.getByText('Bots',{exact:true});if(await bots.count()){await bots.first().click();await page.waitForTimeout(1500);}
await shot('bots');
const bub=page.getByText('Bub',{exact:true});if(await bub.count()){await bub.first().click();await page.waitForTimeout(2000);}
await shot('bot');
// a room's posts (each carries its author's cat) and a bot's look picker (Settings page, every look as its idle cat)
await page.close();page=await (await browser.newContext({viewport:{width:vp.width,height:vp.height},deviceScaleFactor:vp.deviceScaleFactor,isMobile:profile==='phone',hasTouch:true,colorScheme:scheme})).newPage();await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
{const b2=page.getByText('Bots',{exact:true});if(await b2.count()){await b2.first().click();await page.waitForTimeout(800);}
 const room=page.getByText('crew',{exact:true});if(await room.count()){await room.first().click();await page.waitForTimeout(2500);}
 await shot('room');}
await page.close();page=await (await browser.newContext({viewport:{width:vp.width,height:vp.height},deviceScaleFactor:vp.deviceScaleFactor,isMobile:profile==='phone',hasTouch:true,colorScheme:scheme})).newPage();await page.goto(`${hub}/#token=${token}`);await page.waitForTimeout(6000);
{const b3=page.getByText('Bub',{exact:true});if(await b3.count()){await b3.first().click();await page.waitForTimeout(2000);}
 const clickText=(t)=>page.evaluate((t)=>{const c=[...document.querySelectorAll('button,summary,a,li,span,div')].filter(e=>e.children.length===0&&e.textContent.trim()===t&&e.getClientRects().length>0);const e=c[c.length-1];if(e)e.click();return !!e;},t);
 await clickText('Pages');await page.waitForTimeout(800);
 await clickText('Settings');await page.waitForTimeout(2200);
 await shot('look');}
// the logo alone at 48 css px (the iOS pair screen draws it that size, with no connection square): the same
// --brand-image the header uses, so dark gets apple-touch-icon.png and light favicon-light.svg
await page.evaluate(()=>{const d=document.createElement('div');d.id='logo48';d.style.cssText='position:fixed;left:40px;top:40px;width:48px;height:48px;z-index:99999;background:var(--brand-image) center/contain no-repeat';document.body.appendChild(d);});
await page.waitForTimeout(500);
await page.screenshot({path:`${out}/web-${profile}-${scheme}-logo48.png`});
const lb=await page.evaluate((k)=>{const b=document.getElementById('logo48').getBoundingClientRect();return [b.x*k,b.y*k,b.width*k,b.height*k]},vp.deviceScaleFactor);
writeFileSync(`${out}/web-${profile}-${scheme}-logo48.json`,JSON.stringify([{name:'logo48',kind:'logo',rect:lb}],null,1));
await browser.close();
