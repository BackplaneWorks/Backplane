import {chromium} from '<playwright-core>/index.mjs';
import fs from 'node:fs';
const R='<capture-root>';
const browser=await chromium.connectOverCDP('http://127.0.0.1:47811');
const page=browser.contexts()[0].pages().find(p=>p.url().includes('47810'))||browser.contexts()[0].pages()[0];
const mode=process.argv[2]||'inspect';
const session=await page.context().newCDPSession(page);
await session.send('Emulation.setDeviceMetricsOverride',{width:1600,height:1000,deviceScaleFactor:2,mobile:false});
await page.waitForTimeout(250);
try {
 if(mode==='inspect') console.log(JSON.stringify(await page.evaluate(()=>({url:location.href,text:document.body.innerText,viewport:[innerWidth,innerHeight,devicePixelRatio],controls:[...document.querySelectorAll('[data-on-click],button,canvas,textarea')].map(e=>({tag:e.tagName,text:e.textContent.slice(0,70),action:e.dataset.onClick,value:e.dataset.value,key:e.getAttribute('data-key'),id:e.id,rect:(()=>{const r=e.getBoundingClientRect();return [r.x,r.y,r.width,r.height]})()}))})),null,2));
 if(mode==='run') await (await import(process.argv[3])).default(page);
 if(mode==='eval') console.log(await page.evaluate(fs.readFileSync(process.argv[3],'utf8')));
 if(mode==='click') await page.locator(process.argv[3]).click();
 if(mode==='shot') await page.screenshot({path:process.argv[3]});
}finally{await browser.close();}
