// Render the production Bend composer tree with the shipped stylesheet.
// Generate after.json: bend test/tools/composer_bar_fixture.bend > build/composer-bar/after.json
// Optional before.json + BEFORE_CSS capture the reported-build comparison.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium, webkit} = await import(pathToFileURL(path.resolve(process.env.PLAYWRIGHT_CORE_PATH || 'tools/browser/node_modules/playwright-core/index.mjs')));
const out = path.resolve(process.env.COMPOSER_BAR_OUT || 'build/composer-bar');
const styles = fs.readFileSync('src/web/style.css', 'utf8');
const tree = fs.readFileSync(path.join(out, 'after.json'), 'utf8').trim();
const before = fs.existsSync(path.join(out, 'before.json')) && process.env.BEFORE_CSS ? fs.readFileSync(path.join(out, 'before.json'), 'utf8').trim() : null;
const server = Bun.serve({hostname:'127.0.0.1', port:0, fetch(req) {
  const url = new URL(req.url);
  if (url.pathname === '/style.css') return new Response(url.searchParams.has('before') ? fs.readFileSync(process.env.BEFORE_CSS) : styles, {headers:{'Content-Type':'text/css'}});
  if (url.pathname.endsWith('.js')) return new Response(fs.readFileSync('src/web/'+url.pathname.slice(1)), {headers:{'Content-Type':'application/javascript'}});
  return new Response(`<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><link rel="stylesheet" href="/style.css${url.search}"></head><body><div id="root"></div><script type="module">
import {installWorkspaceMenus} from '/workspace-menus.js';
import {sizeComposer,sizeComposerControls} from '/composer-size.js';
const fixture=${url.searchParams.has('before') ? before : tree};const tree=fixture.composer||fixture;
function node(n,svg=false){if(n.type==='text')return document.createTextNode(n.text);svg=svg||n.tag==='svg';const e=svg?document.createElementNS('http://www.w3.org/2000/svg',n.tag):document.createElement(n.tag);for(const a of n.attrs){if(a.event)e.setAttribute('data-on-'+a.event,a.action);else if(a.name==='value')e.value=a.value;else e.setAttribute(a.name,a.value)}for(const k of n.kids)e.append(node(k,svg));return e}
// Same grid and thread containers as View.thread; no live user state.
const app=document.createElement('div');app.className='app picked';app.style.cssText='height:100dvh;grid-template-columns:minmax(0,1fr)';
app.innerHTML='<main class="thread-view workspace-thread" data-thread="t"><div class="thread-content"><div class="timeline"><article><h3>Composer regression</h3><p>The draft and controls stay reachable beside long model and allowance labels.</p></article></div></div></main>';
app.querySelector('main').style.gridColumn='1';app.querySelector('.thread-content').append(node(tree));document.getElementById('root').append(app);
installWorkspaceMenus(document);window.actions=[];
document.addEventListener('click',e=>{const c=e.target.closest('[data-on-click]');if(c)window.actions.push(c.dataset.onClick)});
const field=document.getElementById('composer');field.addEventListener('input',()=>sizeComposer(field));sizeComposer(field);window.sizeComposer=sizeComposer;window.sizeComposerControls=sizeComposerControls;
sizeComposerControls();addEventListener('resize',()=>sizeComposerControls());window.addReply=()=>{if(fixture.reply)for(const child of fixture.reply.kids)document.querySelector('.composer').prepend(node(child));else{const b=document.createElement('div');b.className='reply-chip';b.innerHTML='<span class="label">Replying to:</span><a class="rto">A long reply banner repeated: A long reply banner repeated</a><button class="reply-drop">×</button>';document.querySelector('.composer').prepend(b)}};
window.addStar=()=>{if(fixture.star)for(const child of fixture.star.kids)app.append(node(child));else{const star=document.createElement('section');star.className='star-nudge';star.innerHTML='<p class="star-nudge-text">If Backplane helps, give it a star on GitHub.</p><a class="star-nudge-open" href="#">Star on GitHub</a><button class="star-nudge-dismiss" aria-label="Dismiss star suggestion">×</button>';app.append(star)}};
window.ready=true;</script></body></html>`, {headers:{'Content-Type':'text/html; charset=utf-8'}});
}});
const executablePath=process.env.CHROME_BIN || '/usr/bin/google-chrome-stable';
const browser=process.env.COMPOSER_ENGINE==='webkit' ? await webkit.launch({headless:true}) : await chromium.launch({executablePath,headless:true});
const facts=[];
try {
 const page=await browser.newPage({viewport:{width:390,height:844},hasTouch:true});
 page.on('pageerror',error=>{throw error});
 for(const {width,pane} of [...[320,360,390,430,768,1024,1440].map(width=>({width,pane:width})),...[320,390,700].map(pane=>({width:1440,pane}))]) {
  await page.setViewportSize({width,height:844});
  if(before && [320,390].includes(width) && pane===width) {
   await page.goto(`http://127.0.0.1:${server.port}/?before`);await page.waitForFunction(()=>window.ready);
   await page.evaluate(()=>{window.addReply();window.addStar();document.documentElement.classList.add("light")});await page.screenshot({path:path.join(out,`before-${width}.png`)});
  }
  for(const long of [false,true]) for(const reply of [false,true]) for(const account of [false,true]) {
   await page.goto(`http://127.0.0.1:${server.port}/`);await page.waitForFunction(()=>window.ready);
   await page.evaluate(({long,reply,account,pane})=>{
    document.querySelector(".app").style.gridTemplateColumns=pane+"px minmax(0,1fr)";window.sizeComposerControls?.();
    if(long){document.querySelector('.mp-sel').firstChild.textContent='claude / extremely-long-model-name-'.repeat(20);document.querySelector('.allowance-current').textContent='91% left · shared pool · resets next Thursday '.repeat(20);document.querySelector('.acct-v')?.append(' very long account'.repeat(20));}
    if(!account)document.querySelector('.account-chip')?.remove();
    if(reply)window.addReply();
    window.addStar();document.documentElement.classList.add('light');
   },{long,reply,account,pane});
   await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   const measure=()=>page.evaluate(()=>{
    const bar=document.querySelector('.composer-foot'),send=bar.querySelector('.foot-end > button:last-child'),star=document.querySelector('.star-nudge'),field=document.getElementById('composer');
    const r=bar.getBoundingClientRect(),s=send.getBoundingClientRect();
    const controls=[...bar.querySelectorAll('button,summary,label.attach')].filter(e=>e.getClientRects().length && getComputedStyle(e).visibility!=='hidden');
    return {viewport:innerWidth,height:r.height,button:s.height,barWidth:r.width,wrap:getComputedStyle(bar).flexWrap,overflow:document.documentElement.scrollWidth>innerWidth,
     layout:[...document.querySelectorAll('.app,main,.thread-content,.composer')].map(e=>({class:e.className,rect:e.getBoundingClientRect().toJSON(),display:getComputedStyle(e).display,grid:getComputedStyle(e).gridTemplateColumns})),overlap:r.bottom>star.getBoundingClientRect().top+1,controls:controls.map(e=>({label:e.title||e.getAttribute('aria-label')||e.textContent,top:e.getBoundingClientRect().top,bottom:e.getBoundingClientRect().bottom,left:e.getBoundingClientRect().left,right:e.getBoundingClientRect().right})),value:field.value,fieldHeight:field.getBoundingClientRect().height};
   });
   const m=await measure();const hint=await page.locator('#composer').getAttribute('placeholder');if(pane<=700)assert(hint.length<16,'narrow placeholder must be short '+JSON.stringify({width,pane,hint}));if(m.overflow)await page.screenshot({path:path.join(out,'failure.png')});
   assert(m.height<=m.button+0.5,JSON.stringify({width,long,reply,account,...m}));assert.equal(m.wrap,'nowrap');assert(!m.overflow,JSON.stringify({width,long,reply,account,...m}));assert(!m.overlap,JSON.stringify({width,long,reply,account,...m}));
   for(const c of m.controls){assert(c.left>=-0.5&&c.right<=width+0.5,JSON.stringify({width,c}));assert(c.top>=m.controls.at(-1).top-1&&c.bottom<=m.controls.at(-1).bottom+1,JSON.stringify({width,c,m}));}
   const summary=page.locator('.composer-more > summary');
   if(await summary.isVisible()) {
    await summary.click();
    for(const selector of ['.allowance-chip','.attach.draw','.tok-chip',...(account?['.account-chip']:[])])assert(await page.locator(selector).isVisible(),`${selector} unreachable at ${width}`);
    const opened=await measure();assert.equal(opened.height,m.height,'overflow menu changes bar height');
    await page.locator('.allowance-chip').click();assert.equal(await page.locator('.composer-more').getAttribute('open'),null);assert.equal(await page.evaluate(()=>window.actions.at(-1)),'allowance-open');
    await summary.click();await summary.press('Escape');assert.equal(await page.locator('.composer-more').getAttribute('open'),null);
   } else {
    for(const selector of ['.allowance-chip','.attach.draw','.tok-chip',...(account?['.account-chip']:[])])assert(await page.locator(selector).isVisible(),`${selector} missing on desktop ${width}`);
   }
   if(!long&&reply&&account&&pane===width&&[320,390,1440].includes(width)) await page.screenshot({path:path.join(out,`after-${width}.png`)});
   const field=page.locator('#composer');
   await field.fill('Real input wraps and grows. '.repeat(50));await field.dispatchEvent('input');
   const typed=await measure();assert(typed.fieldHeight>m.fieldHeight,'textarea does not grow');assert.equal(typed.height,m.height);assert.equal(typed.value,'Real input wraps and grows. '.repeat(50));
   await page.setViewportSize({width,height:430});await field.dispatchEvent('input');const keyboard=await measure();assert(keyboard.height<=keyboard.button+0.5);assert(!keyboard.overlap,'keyboard-height layout overlaps star nudge '+JSON.stringify({width,pane,long,reply,account,...keyboard}));await page.setViewportSize({width,height:844});
   facts.push({width,pane,long,reply,account,height:m.height,button:m.button});
  }
 }
 fs.writeFileSync(path.join(out,'browser-receipt.json'),JSON.stringify({browser:await browser.version(),activeStarNudge:true,facts},null,2)+'\n');
 console.log(`PASS ${facts.length} rendered composer states; active star nudge, overflow access, real input growth, keyboard-height resize`);
} finally {await browser.close();server.stop();}
