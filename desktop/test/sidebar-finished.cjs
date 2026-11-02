// Isolated rendering only. Never attaches to the person's app or hub.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const css=fs.readFileSync(path.resolve(__dirname,'../../src/web/style.css'),'utf8'),out=path.resolve(__dirname,'../../build/sidebar-finished');
const row=(id,state,label)=>`<div class="thread" role="button" data-fixture="${id}"><span role="img" class="dot ${state}" aria-label="${state}" data-status-label="${label}"></span><span class="title">A long thread title that must stay reachable</span><span class="ago">12h</span></div>`;
(async()=>{const browser=await chromium.launch({headless:true,executablePath:process.env.E2E_CHROME||'/usr/bin/google-chrome'});try{
 fs.mkdirSync(out,{recursive:true});const page=await browser.newPage();await page.route('**/*',route=>route.abort());let checks=0;
 for(const width of [1280,375])for(const [locale,word]of [['en','Finished'],['ja','完了']])for(const scale of [100,200]){
  await page.setViewportSize({width,height:600});await page.setContent(`<html lang="${locale}" data-theme="dark" style="--text-scale:${scale}"><head><style>${css}</style></head><body><aside class="sidebar" style="width:${Math.min(264,width)}px">${row('finished','finished',word)}${row('active','active','')}${row('waiting','waiting','')}${row('settled','settled','')}</aside></body></html>`);
  const result=await page.evaluate(()=>[...document.querySelectorAll('.thread')].map(el=>{const dot=el.querySelector('.dot'),s=getComputedStyle(dot),box=el.getBoundingClientRect(),side=el.closest('.sidebar').getBoundingClientRect();return{id:el.dataset.fixture,before:getComputedStyle(dot,'::before').content,after:getComputedStyle(dot,'::after').content,animation:s.animationName,background:s.backgroundColor,overflows:el.scrollWidth>el.clientWidth||box.right>side.right+1}}));
  const done=result.find(x=>x.id==='finished');assert.equal(done.after,JSON.stringify(word));assert.equal(done.before,'"✓"');assert.equal(done.animation,'none');assert(!done.overflows,JSON.stringify({width,locale,scale,done}));
  for(const r of result.filter(x=>x.id!=='finished')){assert(['none','normal','""'].includes(r.after),r.id+' inherited Finished label');assert(!r.overflows,r.id+' overflow')}
  assert.notEqual(result.find(x=>x.id==='active').animation,'none');assert.notEqual(result.find(x=>x.id==='waiting').animation,'none');
  if(locale==='en'&&scale===100)await page.screenshot({path:path.join(out,`finished-${width}.png`)});
  // A status change must remove completion text without rebuilding a row.
  await page.locator('[data-fixture=finished] .dot').evaluate(el=>{el.className='dot active';el.setAttribute('data-status-label','')});
  assert.equal(await page.locator('[data-fixture=finished] .dot').evaluate(el=>getComputedStyle(el,'::after').content),'none');checks++;
 }
 console.log(`ok ${checks} desktop/mobile/locale/text-scale layouts and completion-to-active transitions`);
 fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify({state:'PASS',layouts:checks,scope:'isolated source CSS; generated localized attribute checked separately by Bend regression; no live UI'},null,2)+'\n');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
