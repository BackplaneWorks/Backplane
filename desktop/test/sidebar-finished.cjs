// Isolated rendering only. Never attaches to the person's app or hub.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const css=fs.readFileSync(path.resolve(__dirname,'../../src/web/style.css'),'utf8'),out=path.resolve(__dirname,'../../build/sidebar-finished');
const row=(id,state)=>`<div class="thread" role="button" data-fixture="${id}"><span role="img" class="dot ${state}" aria-label="${state}"></span><span class="title">A long thread title that must stay reachable</span><span class="ago">12h</span></div>`;
(async()=>{const browser=await chromium.launch({headless:true,executablePath:process.env.E2E_CHROME||'/usr/bin/google-chrome'});try{
 fs.mkdirSync(out,{recursive:true});const page=await browser.newPage();await page.route('**/*',route=>route.abort());let checks=0;
 for(const width of [1280,375])for(const locale of ['en','ja'])for(const scale of [100,200]){
  await page.setViewportSize({width,height:600});await page.setContent(`<html lang="${locale}" data-theme="dark" style="--text-scale:${scale}"><head><style>${css}</style></head><body><aside class="sidebar" style="width:${Math.min(264,width)}px">${row('finished','finished')}${row('active','active')}${row('waiting','waiting')}${row('settled','settled')}</aside></body></html>`);
  const result=await page.evaluate(()=>[...document.querySelectorAll('.thread')].map(el=>{const dot=el.querySelector('.dot'),s=getComputedStyle(dot),box=el.getBoundingClientRect(),side=el.closest('.sidebar').getBoundingClientRect();return{id:el.dataset.fixture,before:getComputedStyle(dot,'::before').content,after:getComputedStyle(dot,'::after').content,animation:s.animationName,background:s.backgroundColor,radius:s.borderRadius,width:dot.getBoundingClientRect().width,overflows:el.scrollWidth>el.clientWidth||box.right>side.right+1}}));
  // Finished is the plain green dot: no mark or word beside it, no pulse.
  const done=result.find(x=>x.id==='finished');assert.equal(done.radius,'50%');assert.notEqual(done.background,'rgba(0, 0, 0, 0)');assert(done.width<16,JSON.stringify(done));assert.equal(done.animation,'none');
  for(const r of result){assert(['none','normal','""'].includes(r.after)&&['none','normal','""'].includes(r.before),r.id+' has text beside its dot');assert(!r.overflows,JSON.stringify({width,locale,scale,r}))}
  assert.notEqual(result.find(x=>x.id==='active').animation,'none');assert.notEqual(result.find(x=>x.id==='waiting').animation,'none');
  if(locale==='en'&&scale===100)await page.screenshot({path:path.join(out,`finished-${width}.png`)});
  // A status change restyles the same dot without rebuilding a row.
  await page.locator('[data-fixture=finished] .dot').evaluate(el=>{el.className='dot active'});
  assert.notEqual(await page.locator('[data-fixture=finished] .dot').evaluate(el=>getComputedStyle(el).animationName),'none');checks++;
 }
 console.log(`ok ${checks} desktop/mobile/locale/text-scale layouts and completion-to-active transitions`);
 fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify({state:'PASS',layouts:checks,scope:'isolated source CSS; the dot attributes are checked separately by Bend regression; no live UI'},null,2)+'\n');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
