// Actual connection/device DOM and app CSS, with non-secret fixture responses.
// Screenshots never require pairing or changing a live hub.
import {readFileSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve, join} from 'node:path';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const out = resolve(process.argv[2] || 'build/pairing-theme');
const screenshots = join(out, 'screenshots');mkdirSync(screenshots, {recursive:true});
const qr = JSON.parse(readFileSync(join(out,'fixture-qr.json'),'utf8'));
for (const kind of ['before', 'after']) {
  const source = kind === 'before' ? join(out,'before/src/web') : resolve('src/web');
  const entry = join(out,kind,'fixture.ts');mkdirSync(join(out,kind),{recursive:true});
  writeFileSync(entry, `import {installPairingUI} from ${JSON.stringify(join(source,'pairing-ui.js'))};
    const params=new URLSearchParams(location.search);
    document.documentElement.classList.toggle('light',params.get('theme')==='light');
    window.fixtureCalls=[];window.fixturePaired=false;
    const fetch=async(path,options={})=>{window.fixtureCalls.push({path,options});
      return {ok:true,json:async()=>path==='/auth/devices'?{devices:[{id:'fixture-device',name:'Linux / Desktop',lastSeen:1793664000000}]}:
      path==='/auth/invitations'?{code:'ABCD2345',link:'https://hub.example/pair#code=ABCD2345',routes:['https://hub.example'],expiresAt:Date.now()+600000,qr:${JSON.stringify(qr)}}:{ok:true}}};
    window.fixture=installPairingUI({document,location,fetch,onPaired:()=>{window.fixturePaired=true}});
    if(params.get('view')==='devices') window.fixture.devices();else window.fixture.show();`);
  const result = await Bun.build({entrypoints:[entry],target:'browser'});
  if(!result.success) throw Error(String(result.logs));
  writeFileSync(join(out,kind,'fixture.js'),await result.outputs[0].text());
}
const server = Bun.serve({hostname:'127.0.0.1',port:0,fetch(request) {
  const u=new URL(request.url), kind=u.pathname.startsWith('/before')?'before':'after';
  const source=kind==='before'?join(out,'before/src/web'):resolve('src/web');
  if(u.pathname.endsWith('.css')) return new Response(readFileSync(join(source,u.pathname.endsWith('pairing.css')?'pairing.css':'style.css')), {headers:{'Content-Type':'text/css'}});
  if(u.pathname.endsWith('fixture.js')) return new Response(readFileSync(join(out,kind,'fixture.js')), {headers:{'Content-Type':'text/javascript'}});
  return new Response('<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/'+kind+'/style.css"><link rel="stylesheet" href="/'+kind+'/pairing.css"><div id="root"></div><script type="module" src="/'+kind+'/fixture.js"></script>', {headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox']});
const receipts=[];
try {
 for(const width of [390,1280]) for(const theme of ['light','dark']) for(const view of ['connect','devices']) for(const kind of ['before','after']) {
  const context=await browser.newContext({viewport:{width,height:900},userAgent:width===390?'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Version/18.0 Mobile/15E148 Safari/604.1':'Mozilla/5.0 (X11; Linux x86_64) Chrome/142.0 Safari/537.36'});
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`http://127.0.0.1:${server.port}/${kind}/?theme=${theme}&view=${view}`);
  await page.getByRole('heading',{name:view==='connect'?'Connect to your Backplane hub':'Devices',exact:true}).waitFor();
  if(view==='devices') {await page.getByRole('button',{name:'Add a device',exact:true}).click();await page.getByLabel('One-time pairing link').waitFor();}
  const name=`${kind}-${view}-${width}-${theme}.png`;await page.screenshot({path:join(screenshots,name)});
  if(kind==='after') {
   assert.equal(await page.locator('.pairing-brand, details').count(),0,'no separate branding or collapsed method');
   const colors=await page.evaluate(()=>{
    const panel=document.querySelector('.pairing-screen'),root=getComputedStyle(document.documentElement),style=getComputedStyle(panel);
    const color=(token)=>{const probe=document.createElement('span');probe.style.color='var('+token+')';document.body.append(probe);const c=getComputedStyle(probe).color;probe.remove();return c};
    const b=document.querySelector('.pairing-card button.primary'),bs=getComputedStyle(b);
    return {background:style.backgroundColor,bg:color('--bg'),text:style.color,tokenText:color('--text'),button:bs.backgroundColor,accent:color('--accent'),border:bs.borderColor,onAccent:color('--on-accent'),buttonText:bs.color};
   });
   assert.equal(colors.background,colors.bg);assert.equal(colors.text,colors.tokenText);assert.equal(colors.button,colors.accent);assert.equal(colors.border,colors.accent);assert.equal(colors.buttonText,colors.onAccent);
   assert.ok(await page.locator('.pairing-screen').evaluate(n=>n.scrollWidth<=innerWidth),'no horizontal overflow');
   if(view==='connect') {
    const tabs=page.getByRole('tab');assert.equal(await tabs.count(),3);
    for(let i=0;i<3;i++)assert.equal(await tabs.nth(i).isVisible(),true);
    assert.equal(await page.getByLabel('Device name',{exact:true}).inputValue(),width===390?'iPhone / Safari':'Linux / Chrome');
    await tabs.nth(0).focus();await page.keyboard.press('End');assert.equal(await tabs.nth(2).getAttribute('aria-selected'),'true');
    assert.equal(await page.getByLabel('8-character code',{exact:true}).isVisible(),true);
    await page.screenshot({path:join(screenshots,`after-code-${width}-${theme}.png`)});
    await tabs.nth(1).click();
    const chooserEvent=page.waitForEvent('filechooser');await page.getByRole('button',{name:'Choose QR image',exact:true}).click();await chooserEvent;
    assert.equal(await page.locator('input[type=file]').isVisible(),false,'native file control stays hidden');
    await page.screenshot({path:join(screenshots,`after-qr-${width}-${theme}.png`)});
    await tabs.nth(2).click();await page.getByLabel('8-character code',{exact:true}).fill('ABCD2345');await page.getByLabel('Device name',{exact:true}).fill('My laptop');
    await page.getByRole('button',{name:'Connect with code',exact:true}).click();
    await page.waitForFunction(()=>window.fixturePaired);
    const nameSent=await page.evaluate(()=>window.fixtureCalls.find(c=>c.path==='/auth/enroll').options.headers['X-Backplane-Name']);assert.equal(nameSent,'My laptop');
   }
  }
  assert.deepEqual(errors,[]);receipts.push({kind,view,width,theme,screenshot:name,passed:true});await context.close();
 }
 writeFileSync(join(out,'screenshot-results.json'),JSON.stringify(receipts,null,2)+'\n');
 console.log('PASS themed connection + Add a device: before/after at 390/1280 in light/dark; keyboard, image chooser, code controls and edited name verified');
} finally {await browser.close();server.stop(true);}
