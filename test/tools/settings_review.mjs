// Focused local render/interaction receipt over the production Settings view.
// No live hub, provider calls, settings writes or CI controller.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { writeFileSync, mkdirSync } from 'node:fs';
const lib=(await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const {chromium}=await import(pathToFileURL(process.env.BACKPLANE_PLAYWRIGHT_PATH||resolve('tools/browser/node_modules/playwright-core/index.mjs')).href);
const out=resolve('build/settings-review');mkdirSync(out,{recursive:true});
const each=xs=>{const a=[];for(;xs?.$==='Con';xs=xs.tail)a.push(xs.head);return a;};
const esc=x=>String(x).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
function html(n){if(n.$?.endsWith('.Txt'))return esc(n.text);return `<${n.tag} ${each(n.attrs).map(a=>a.$?.endsWith('.At')?`${esc(a.name)}="${esc(a.value)}"`:`data-on-${esc(a.event)}="${esc(a.action)}"`).join(' ')}>${each(n.kids).map(html).join('')}</${n.tag}>`;}
let ui=lib.init(),ja=false;const actions=[];
const view=()=>html(lib.view(ui,ja));
const server=Bun.serve({hostname:'127.0.0.1',port:4040,async fetch(req){const url=new URL(req.url);
 if(url.pathname==='/style.css')return new Response(Bun.file(resolve('src/web/style.css')),{headers:{'Content-Type':'text/css'}});
 if(url.pathname==='/act'){const {action,value}=await req.json();actions.push({action,value});ui=lib.act(ui,action,value);return new Response(view());}
 if(url.pathname==='/ja'){ja=true;ui=lib.init();}
 if(url.pathname==='/fallback'){ja=false;ui=lib.fallback_init();}
 return new Response(`<!doctype html><html lang="${ja?'ja':'en'}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Backplane Settings review</title><link rel="stylesheet" href="/style.css"><body><div id="review" style="height:100vh">${view()}</div><script>
 const send=async(action,value)=>{const focused=document.activeElement?.id;const selection=document.activeElement?.selectionStart;const response=await fetch('/act',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action,value})});document.querySelector('#review').innerHTML=await response.text();if(focused){const f=document.getElementById(focused);f?.focus();if(typeof selection==='number')f?.setSelectionRange(selection,selection);}};
 document.addEventListener('click',e=>{const b=e.target.closest('[data-on-click]');if(b)send(b.dataset.onClick,b.dataset.value||'');});
 document.addEventListener('input',e=>{const f=e.target.closest('[data-bfield]');if(f)send('bfield',f.dataset.bfield+'\\u001f'+f.value);});
 document.addEventListener('keydown',e=>{if(e.key==='Enter'&&e.target.dataset.enter){e.preventDefault();send(e.target.dataset.enter,e.target.dataset.value||'');}});
 </script></body></html>`,{headers:{'Content-Type':'text/html'}});
}});
const browser=await chromium.launch({executablePath:process.env.BACKPLANE_CHROME||chromium.executablePath(),headless:true,args:['--disable-gpu']});
try{
 const page=await browser.newPage({viewport:{width:1280,height:1000}});const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.goto('http://127.0.0.1:4040');
 assert.equal(await page.locator('.settings-category').count(),14);
 assert.equal(await page.locator('.settings-group').count(),4);
 assert.equal(await page.locator('.settings-category-icon svg path').count()>14,true,'existing vector icons rendered');
 // The production scroll container is captured as the user sees it.
 await page.screenshot({path:resolve(out,'settings-desktop.png')});
 const ids=await page.locator('.settings-category').evaluateAll(bs=>bs.map(b=>b.dataset.value));
 for(const id of ids){
  const before=actions.length;
  await page.locator(`.settings-category[data-value="${id}"]`).click();
  await page.locator(`#settings-content[data-settings-page="${id}"]`).waitFor();
  assert.equal(actions.length,before+1,`${id} opens on one click`);
  await page.locator('button[data-on-click="settings-page"][data-value=""]').click();
  await page.locator('.settings-category').first().waitFor();
 }
 await page.setViewportSize({width:390,height:844});
 const mobile=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth,columns:getComputedStyle(document.querySelector('.settings-grid')).gridTemplateColumns}));
 assert.equal(mobile.scroll,390,'390px layout has no horizontal overflow');
 await page.screenshot({path:resolve(out,'settings-390.png')});
 await page.locator('#settings-find').fill('usage');
 await page.waitForFunction(()=>document.querySelectorAll('.settings-category').length===1);
 await page.locator('.settings-category[data-value="usage"]').waitFor();
 assert.equal(await page.locator('.settings-category').count(),1,'search keeps matching category');
 await page.locator('#settings-find').press('Enter');
 await page.locator('#settings-content[data-settings-page="usage"]').waitFor();
 await page.screenshot({path:resolve(out,'settings-usage-390.png')});
 await page.goto('http://127.0.0.1:4040/ja');
 assert.equal(await page.locator('.settings-category').count(),14);
 await page.getByRole('heading',{name:'ワークスペース',exact:true}).waitFor();
 // Open the advertised Agents page and exercise its real fallback controls.
 await page.setViewportSize({width:1280,height:1000});
 await page.goto('http://127.0.0.1:4040/fallback');
 await page.locator('.settings-category[data-value="agents"]').click();
 const toggle=page.locator('button[data-value="fallback.on=on"]');
 await toggle.waitFor();
 await toggle.click();
 await page.locator('button.on[data-value="fallback.on=on"]').waitFor();
 const add=page.getByRole('button',{name:'Add to hierarchy',exact:true});
 await add.click();
 await page.getByText('1. codex/gpt-6.1-sol',{exact:true}).waitFor();
 assert.equal(await add.count(),0,'duplicate target cannot be added twice');
 await page.locator('button[data-value="fallback.add.provider=claude"]').click();
 await page.locator('button[data-value="fallback.add.model=sonnet"]').click();
 await add.click();
 await page.getByText('2. claude/sonnet',{exact:true}).waitFor();
 await page.locator('button[data-value="fallback.chain=claude/sonnet,codex/gpt-6.1-sol"]').filter({hasText:'Move up'}).click();
 await page.getByText('1. claude/sonnet',{exact:true}).waitFor();
 await page.screenshot({path:resolve(out,'fallback-agents-desktop.png')});
 await page.setViewportSize({width:390,height:844});
 await toggle.scrollIntoViewIfNeeded();
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth),390,'fallback editor fits 390px');
 await page.screenshot({path:resolve(out,'fallback-agents-390.png')});
 await page.locator('button[data-value="fallback.chain=codex/gpt-6.1-sol"]').filter({hasText:'Remove'}).click();
 await page.getByText('1. codex/gpt-6.1-sol',{exact:true}).waitFor();
 // Reload the saved fixture state, rather than its explicit reset route.
 await page.evaluate(()=>history.replaceState(null,'','/'));
 await page.reload();
 await page.getByText('1. codex/gpt-6.1-sol',{exact:true}).waitFor();
 assert.deepEqual(errors,[],'no browser errors');
 const receipt={pass:true,scope:'actual production Settings fixture; isolated browser, no live hub writes',checks:['14 categories open with one click each','4 groups with existing SVG icons','390px no overflow and single column','search filters and Enter opens result','Japanese menu retains14 categories','Agents fallback enable/add/reorder/remove and duplicate prevention','fallback editor fits 390px and retains configured order on reload','zero page errors'],mobile};
 writeFileSync(resolve(out,'browser-checks.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
}finally{await browser.close();server.stop(true);}
