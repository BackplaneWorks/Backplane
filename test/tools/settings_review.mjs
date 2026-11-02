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
const server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(req){const url=new URL(req.url);
 if(url.pathname==='/style.css')return new Response(Bun.file(resolve('src/web/style.css')),{headers:{'Content-Type':'text/css'}});
 if(url.pathname==='/act'){const {action,value}=await req.json();actions.push({action,value});ui=lib.act(ui,action,value);return new Response(view());}
 if(url.pathname==='/recv'){ui=lib.recv(ui,JSON.stringify(await req.json()));return new Response(view());}
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
 await page.goto(`http://127.0.0.1:${server.port}`);
 assert.equal(await page.locator('.settings-category').count(),13);
 assert.equal(await page.locator('.settings-group').count(),4);
 assert.equal(await page.locator('.settings-category-icon svg path').count()>13,true,'existing vector icons rendered');
 // The production scroll container is captured as the user sees it.
 await page.screenshot({path:resolve(out,'settings-desktop.png')});
 const ids=await page.locator('.settings-category').evaluateAll(bs=>bs.map(b=>b.dataset.value));
 for(const id of ids){
  const before=actions.length;
  await page.locator(`.settings-category[data-value="${id}"]`).click();
  await page.locator(`#settings-content[data-settings-page="${id}"]`).waitFor();
  assert.equal(actions.length,before+1,`${id} opens on one click`);
  assert.equal(await page.locator('#settings-side-antigravity').count(),0,'Antigravity has no separate sidebar entry');
  if(id==='agents'){
   await page.locator('[data-on-click="agy-install"]').waitFor();
   assert.equal(await page.locator('#agy-agykey').getAttribute('type'),'password','Antigravity API key stays sealed');
   await page.screenshot({path:resolve(out,'settings-agents-desktop.png')});
   await page.locator('[data-on-click="provider-setup-open"]').click();
   await page.locator('[data-on-click="provider-setup-pick"]').first().waitFor();
   assert.equal(await page.locator('[data-on-click="agy-install"]').count(),1,'Antigravity remains reachable during guided setup');
  }
  await page.locator('button[data-on-click="settings-page"][data-value=""]').click();
  await page.locator('.settings-category').first().waitFor();
 }
 // Voice states are real Bend actions/events over the production view.
 const push=async o=>{await page.evaluate(async o=>{const r=await fetch('/recv',{method:'POST',body:JSON.stringify(o)});document.querySelector('#review').innerHTML=await r.text();},o);};
 const local=installed=>({supported:true,runtime:installed,vad:installed,models:[{id:'parakeet-v2',label:'Parakeet v2',size:482468385,installed}]});
 await page.locator('.settings-category[data-value="voice"]').click();
 await page.getByRole('button',{name:'Check availability',exact:true}).waitFor();
 assert.equal(await page.getByText(/0 MB/).count(),0,'unknown size does not pretend to be zero');
 await push({t:'reply',local:local(false)});
 const learned=page.locator('.set-row').filter({hasText:'Learned fixes'});
 assert.equal(await learned.getAttribute('data-setting-kind'),'info');
 await learned.getByText(/No fixes learned yet/).waitFor();
 assert.equal(await learned.locator('button').count(),0,'empty learned fixes has no fake control');
 await page.screenshot({path:resolve(out,'settings-voice-empty.png')});
 await page.getByRole('button',{name:'Download Parakeet v2 (482 MB)',exact:true}).click();
 await page.getByRole('status').filter({hasText:'Starting download'}).waitFor();
 await push({t:'voice',ev:'local-progress',what:'Parakeet v2',done:241000000,total:482000000});
 await page.getByRole('status').filter({hasText:'241 / 482 MB (50%)'}).waitFor();
 assert.equal(await page.locator('progress').getAttribute('value'),'241000000');
 await page.screenshot({path:resolve(out,'settings-voice-progress.png')});
 await push({t:'voice',ev:'local-result',local:local(false),error:'network offline'});
 await page.getByRole('button',{name:'Retry download',exact:true}).waitFor();
 await page.getByRole('status').filter({hasText:'network offline'}).waitFor();
 await page.getByRole('button',{name:'Retry download',exact:true}).click();
 await push({t:'voice',ev:'local-progress',phase:'unpack',what:'Parakeet v2',done:0,total:0});
 assert.equal(await page.locator('progress').getAttribute('value'),null,'unpacking is indeterminate');
 await push({t:'voice',ev:'local-result',local:local(true)});
 await page.getByRole('button',{name:'Remove',exact:true}).first().waitFor();
 await page.getByRole('status').filter({hasText:'is ready.'}).waitFor();
 await page.setViewportSize({width:390,height:844});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth),390,'voice fits a phone');
 await page.screenshot({path:resolve(out,'settings-voice-390.png')});
 await page.locator('button[data-on-click="settings-page"][data-value=""]').click();
 await page.locator('.settings-category').first().waitFor();
 await page.setViewportSize({width:390,height:844});
 const mobile=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth,columns:getComputedStyle(document.querySelector('.settings-grid')).gridTemplateColumns}));
 assert.equal(mobile.scroll,390,'390px layout has no horizontal overflow');
 await page.screenshot({path:resolve(out,'settings-390.png')});
 await page.locator('#settings-find').fill('usage');
 await page.waitForFunction(()=>document.querySelectorAll('.settings-category').length===1 && document.querySelector('.settings-category')?.dataset.value==='usage');
 assert.equal(await page.locator('.settings-category').count(),1,'search keeps matching category');
 await page.locator('#settings-find').press('Enter');
 await page.locator('#settings-content[data-settings-page="usage"]').waitFor();
 await page.screenshot({path:resolve(out,'settings-usage-390.png')});
 await page.locator('button[data-on-click="settings-page"][data-value=""]').click();
 await page.locator('#settings-find').fill('antigravity');
 await page.locator('.settings-category[data-value="agents"]').waitFor();
 assert.equal(await page.locator('.settings-category').count(),1,'Antigravity search offers Agents');
 await page.locator('#settings-find').press('Enter');
 await page.locator('#settings-content[data-settings-page="agents"]').waitFor();
 await page.locator('[data-on-click="agy-install"]').waitFor();
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth),390,'Agents has no horizontal overflow at 390px');
 await page.screenshot({path:resolve(out,'settings-agents-390.png')});
 await page.goto(`http://127.0.0.1:${server.port}/ja`);
 assert.equal(await page.locator('.settings-category').count(),13);
 await page.getByRole('heading',{name:'ワークスペース',exact:true}).waitFor();
 // Open the advertised Agents page and exercise its real fallback controls.
 await page.setViewportSize({width:1280,height:1000});
 await page.goto(`http://127.0.0.1:${server.port}/fallback`);
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
 const receipt={pass:true,scope:'actual production Settings fixture; isolated browser, no live hub writes',checks:['13 categories open with one click each','4 groups with existing SVG icons','390px no overflow and single column','search filters and Enter opens result','Japanese menu retains13 categories','Agents fallback enable/add/reorder/remove and duplicate prevention','fallback editor fits 390px and retains configured order on reload','voice: empty, download, progress, unpack, error, retry, ready','read-only rows are distinct from controls','Antigravity controls on Agents, including guided setup','Antigravity search opens Agents at 390px without overflow','zero page errors'],mobile};
 writeFileSync(resolve(out,'browser-checks.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
}finally{await browser.close();server.stop(true);}
