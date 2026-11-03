// Run on a different tailnet machine. Credential arrives only on stdin;
// the caller owns/revokes its disposable TEST device in a finally block.
import assert from 'node:assert/strict';
import {mkdirSync, writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const log=x=>console.error(new Date().toISOString()+' '+x);
const input=await new Response(Bun.stdin.stream()).json();
const {origin,token,paths,out,entry,bot='Bub'}=input;
mkdirSync(out,{recursive:true});
const {chromium}=await import(pathToFileURL(process.env.PLAYWRIGHT_MODULE || '/home/h/.bun/install/cache/playwright-core@1.63.0@@@1/index.mjs'));
log('Launch headless mobile Chromium');
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_BIN || '/home/h/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome',args:['--no-sandbox','--disable-dev-shm-usage']});
process.on('SIGTERM',()=>{browser.close().finally(()=>process.exit(143));});
let page;
const receipt={origin,viewport:390,device:'disposable TEST',checks:[],images:[]};
try {
 const context=await browser.newContext({viewport:{width:390,height:844},isMobile:true,hasTouch:true,deviceScaleFactor:1,userAgent:'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'});
 await context.addCookies([{name:'bp_token',value:token,url:origin,httpOnly:true,secure:origin.startsWith('https:'),sameSite:'Strict'}]);
 page=await context.newPage();
 log('Open paired Bub conversation');
 await page.goto(origin+'/#bot='+encodeURIComponent(bot));
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:60000});
 // In a live conversation history is paged; explicitly jump to the message
 // through its existing history controls when it is not on the newest page.
 if(entry && !(await page.locator('[data-id="'+entry+'"]').count())) {
  // Call the same delegated DOM action used by a history search result.
  await page.evaluate(id=>{const button=document.createElement('button');button.setAttribute('data-on-click','hist-jump');button.setAttribute('data-value',id);document.body.append(button);button.click();button.remove();},entry);
 }
 for(const path of paths) {
  log('Decode '+path.split('/').at(-1));
  // Path is encoded by the shared Bend URL encoder; decode query values
  // instead of embedding an arbitrary filesystem name into a CSS selector.
  await page.waitForFunction(p=>[...document.querySelectorAll('img[data-message-image]')].some(img=>new URL(img.src).searchParams.get('path')===p),path,{timeout:45000});
  const index=await page.evaluate(p=>[...document.querySelectorAll('img[data-message-image]')].findIndex(img=>new URL(img.src).searchParams.get('path')===p),path);
  const img=page.locator('img[data-message-image]').nth(index);
  await img.scrollIntoViewIfNeeded();
  await img.evaluate(img=>img.decode());
  const result=await img.evaluate(img=>({path:new URL(img.src).searchParams.get('path'),route:new URL(img.src).pathname,width:img.naturalWidth,height:img.naturalHeight,lazy:img.loading,placeholder:img.parentElement.classList.contains('image-unavailable')}));
  assert.equal(result.route,'/message-image');assert(result.width>0&&result.height>0);assert.equal(result.lazy,'lazy');assert.equal(result.placeholder,false);
  receipt.images.push(result);
  await page.screenshot({path:out+'/'+path.split('/').at(-1)});
 }
 receipt.checks.push('Four referenced PNGs decoded in the 390px conversation through message-image');
 log('Authentication and authorization negative controls');
 // Cookie, native query token and native Bearer use the same admission gate.
 const route=await page.evaluate(p=>[...document.querySelectorAll('img[data-message-image]')].find(img=>new URL(img.src).searchParams.get('path')===p).getAttribute('src'),paths[0]);
 const url=new URL(route,origin);
 assert.equal((await context.request.get(url.href)).status(),200);
 const native=await browser.newContext();
 assert.equal((await native.request.get(url.href)).status(),401);
 assert.equal((await native.request.get(url.href,{headers:{Authorization:'Bearer '+token}})).status(),200);
 url.searchParams.set('token',token);assert.equal((await native.request.get(url.href)).status(),200);url.searchParams.delete('token');
 receipt.checks.push('Cookie, Bearer and native query credentials accepted; unauthenticated image denied');
 url.searchParams.set('path','/etc/passwd');assert.equal((await context.request.get(url.href)).status(),404);
 url.searchParams.set('path',paths[0]);url.searchParams.set('entry','unreferenced');assert.equal((await context.request.get(url.href)).status(),404);
 url.searchParams.set('path','/tmp/../etc/passwd.png');assert.equal((await context.request.get(url.href)).status(),404);
 receipt.checks.push('Arbitrary paths, traversal and unreferenced entry denied');
 log('Unavailable placeholder negative control');
 await page.evaluate(()=>{const original=document.querySelector('img[data-message-image]');const wrapper=original.parentElement.cloneNode(true),img=wrapper.querySelector('img');img.src='/message-image?entry=missing&path=/tmp/missing.png';wrapper.id='image-negative-control';original.parentElement.after(wrapper);});
 await page.locator('#image-negative-control').scrollIntoViewIfNeeded();
 await page.waitForFunction(()=>document.getElementById('image-negative-control')?.classList.contains('image-unavailable'));
 assert.equal(await page.locator('#image-negative-control .message-image-unavailable').innerText(),'Image unavailable');
 await page.screenshot({path:out+'/unavailable.png'});
 receipt.checks.push('Missing-image negative control has text placeholder and no broken icon');
 receipt.state='passed';
} catch(e) {receipt.state='failed';receipt.error=String(e.stack).replace(/[a-f0-9]{32}/g,'[redacted]');if(page)await page.screenshot({path:out+'/failure.png'}).catch(()=>{});process.exitCode=1;}
finally {await browser.close();writeFileSync(out+'/receipt.json',JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt));}
