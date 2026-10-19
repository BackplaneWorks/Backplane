import { chromium } from '/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs';
import fs from 'node:fs';
import path from 'node:path';
const out=path.resolve(import.meta.dirname,'..');
const [port,home]=process.argv.slice(2);
const browser=await chromium.launch({executablePath:'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,args:['--no-sandbox']});
try{
 const ctx=await browser.newContext({viewport:{width:1000,height:720},deviceScaleFactor:2});
 const page=await ctx.newPage();
 page.on('pageerror',e=>console.log('PAGE ERROR',e.message));
 await page.goto(`http://127.0.0.1:${port}/`);
 await page.waitForTimeout(2000);
 const snap=async(name)=>{
  await page.waitForTimeout(800);
  await page.screenshot({path:path.join(out,'raw',name+'.png')});
  console.log('\n=== '+name+' ===\n'+await page.locator('body').innerText());
  fs.writeFileSync(path.join(out,'source',name+'-dom.html'),await page.content());
  console.log(await page.locator('[data-on-click]').evaluateAll(es=>es.map(e=>({tag:e.tagName,text:e.innerText.slice(0,80),action:e.dataset.onClick,value:e.dataset.value}))));
 };
 const act=async(a,v)=>{await page.locator(`[data-on-click="${a}"]${v===undefined?'':`[data-value="${v}"]`}`).first().click();};
 await snap('web-initial');
 await act('uber'); await snap('uberbot-overview');
 await act('select','release'); await snap('delegated-thread');
 const sub=page.locator('[data-on-click*="sub"]');
 if(await sub.count()){await sub.first().click();await page.waitForTimeout(700);const done=page.locator('[data-on-click="fold"][data-value="1subs.done:release"]');if(await done.count()){await done.click();await page.waitForTimeout(500);}await snap('subagent-panel');await page.setViewportSize({width:480,height:720});await snap('subagents-mobile');await page.setViewportSize({width:1000,height:720});}
 await act('bot','tofu'); await snap('catbot-chat');
 const tabs=await page.locator('[data-on-click="bot-tab"]').evaluateAll(es=>es.map(e=>e.dataset.value));
 console.log('tabs:',tabs);
 if(tabs.includes('mail')){await act('bot-tab','mail');await snap('catbot-mailbox');}
 await act('bot-tab','hooks');await snap('catbot-webhooks');
 await act('bot','pixel');await act('bot-tab','space');await snap('catbot-space');await page.setViewportSize({width:480,height:720});await snap('space-mobile');await page.setViewportSize({width:1000,height:720});
 const response=await page.goto(`http://127.0.0.1:${port}/bots/pixel/space.html`);
 console.log('SPACE PAGE STATUS',response.status());
 await snap('space-webapp');await page.setViewportSize({width:480,height:720});await snap('space-webapp-mobile');
}finally{await browser.close();}
