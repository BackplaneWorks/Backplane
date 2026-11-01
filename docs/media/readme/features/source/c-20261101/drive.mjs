import {chromium} from '<playwright-core>/index.mjs';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const R='<capture-root>';
const source=path.dirname(new URL(import.meta.url).pathname);
const browser=await chromium.connectOverCDP('http://127.0.0.1:47831');
const mode=process.argv[2]||'inspect';
try {
 let context=browser.contexts().find(c=>c.pages().some(p=>p.url().startsWith('http://127.0.0.1:47830/')));
 if(!context) context=await browser.newContext({viewport:{width:1100,height:800},deviceScaleFactor:2,colorScheme:'dark'});
 let page=context.pages().find(p=>p.url().startsWith('http://127.0.0.1:47830/'));
 if(!page){
   page=await context.newPage();
   const token=fs.readFileSync(R+'/run/C-features/home/token','utf8').trim();
   await page.goto('http://127.0.0.1:47830/#token='+token);
   await page.waitForTimeout(1700);
 }
 if(mode==='inspect') console.log(await page.evaluate(()=>({text:document.body.innerText,actions:[...document.querySelectorAll('[data-on-click],textarea,input')].map(e=>({tag:e.tagName,text:e.textContent?.slice(0,110),action:e.dataset.onClick,value:e.dataset.value,key:e.getAttribute('data-key'),name:e.name,placeholder:e.placeholder})),dpr:devicePixelRatio})));
 if(mode==='eval') console.log(await page.evaluate(fs.readFileSync(process.argv[3],'utf8')));
 if(mode==='click') await page.locator(process.argv[3]).click();
 if(mode==='shot') await page.screenshot({path:process.argv[3]});
 if(mode==='run') await (await import(process.argv[3])).default(page,{R,source,context});
} finally {await browser.close();}
