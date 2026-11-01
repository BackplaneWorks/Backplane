import {chromium} from '<playwright-core>/index.mjs';
import fs from 'node:fs';
import path from 'node:path';
const out=path.resolve(import.meta.dirname,'..');
const name=process.env.README_B_RUN || 'B-publish';
if(!name.startsWith('B-') || name.includes('/')) throw Error('Invalid run name');
const run='<capture-root>/run/'+name;
const browser=await chromium.launch({executablePath:'<ms-playwright>/chromium-1243/chrome-linux64/chrome',headless:true,args:['--no-sandbox','--disable-dev-shm-usage'],env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:''}});
try {
 const ctx=await browser.newContext({viewport:{width:1000,height:720},deviceScaleFactor:2,colorScheme:'dark'});
 const page=await ctx.newPage();
 page.on('pageerror',e=>console.log('PAGE ERROR',e.message));
 await page.goto('http://127.0.0.1:47820/#token='+fs.readFileSync(run+'/home/token','utf8').trim());
 await page.waitForTimeout(1600);
 const act=async(a,v)=>{await page.locator(`[data-on-click="${a}"]${v===undefined?'':`[data-value="${v}"]`}:visible`).first().click();await page.waitForTimeout(400);};
 const snap=async(name)=>{
  await page.waitForTimeout(400);
  await page.screenshot({path:path.join(out,'raw','current-'+name+'.png')});
  const info=await page.evaluate(()=>({text:document.body.innerText,controls:[...document.querySelectorAll('[data-on-click],button')].map(e=>({tag:e.tagName,text:e.innerText?.slice(0,100),action:e.dataset.onClick,value:e.dataset.value})),main:[...document.querySelectorAll('main,.main,.workspace,.content')].map(e=>({tag:e.tagName,class:e.className,rect:e.getBoundingClientRect().toJSON()}))}));
  fs.writeFileSync(path.join(out,'source','current-'+name+'-dom.json'),JSON.stringify(info,null,2));
  console.log(name,JSON.stringify(info,null,2));
 };
 await act('uber');
 await act('bot-tab','mail');
 await act('bot-tab','status');
 await snap('overview');
 if(process.argv[2]==='all') {
  await page.setViewportSize({width:1076,height:720}); await act('select','release'); await snap('thread');
  const sub=page.locator('[data-on-click*="sub"]');
  console.log('SUBS',await sub.count());
  if(await sub.count()) {await sub.first().click(); await page.waitForTimeout(600);}
  const done=page.locator('[data-on-click="fold"][data-value="1subs.done:release"]:visible');
  if(await done.count()) await done.click();
  await snap('delegation');
  await page.setViewportSize({width:1000,height:720}); await act('bot','tofu'); await snap('tofu');
  const tabs=await page.locator('[data-on-click="bot-tab"]').evaluateAll(es=>es.map(e=>({value:e.dataset.value,text:e.innerText})));
  console.log('BOT TABS',JSON.stringify(tabs));
  fs.writeFileSync(path.join(out,'source','current-bot-tabs.json'),JSON.stringify(tabs,null,2));
  if(tabs.some(t=>t.value==='mail')) {await page.setViewportSize({width:1600,height:850}); await act('bot-tab','mail'); await act('mail-open','demo-mail-2'); await page.waitForTimeout(600); await snap('mailroom'); await page.setViewportSize({width:1000,height:720});}
  await act('bot-tab','hooks'); await snap('webhooks');
  await act('bot','pixel'); await act('bot-tab','space'); await snap('space');
  await page.setViewportSize({width:480,height:720});
  const resp=await page.goto('http://127.0.0.1:47820/bots/pixel/space.html');
  console.log('SPACE STATUS',resp.status());
  await snap('space-webapp');
 }
} finally {await browser.close();}
