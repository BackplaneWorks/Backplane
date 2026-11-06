'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),http=require('node:http');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
// A patch hands the rows over (defer) and the host's refresh plans them once
// with its own command; a host that never refreshes still gets the plan.
test('a deferred update plans once, after the patch, with the render’s command',async t=>{
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/timeline-scroller.js'));
 const server=http.createServer((req,res)=>{res.setHeader('content-type',req.url.endsWith('.js')?'text/javascript':'text/html');res.end(req.url.endsWith('.js')?source:'<div id="timeline" style="height:300px;width:400px;overflow:auto"></div>');});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));t.after(()=>new Promise(r=>server.close(r)));
 const browser=await chromium.launch({headless:true,...(process.env.BACKPLANE_CHROME?{executablePath:process.env.BACKPLANE_CHROME}:{})});t.after(()=>browser.close());
 const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);
 const out=await page.evaluate(async()=>{
  const {TimelineScroller}=await import('/timeline-scroller.js');const plans=[];
  const plan=input=>{plans.push(input.mode);const items=input.keys.map((key,i)=>({key,index:i,start:i*40}));return {visible:items,items,total:items.length*40,top:input.mode==='bottom'?Math.max(0,items.length*40-300):0,view:input.view,atEnd:true,valid:[0,0]};};
  const el=document.getElementById('timeline');
  const list=new TimelineScroller(el,{plan,create:v=>{const d=document.createElement('div');d.textContent=v;d.style.height='40px';return d;},patch:(n,v)=>{n.textContent=v;return n;},key:v=>v});
  list.update(['a','b'],{defer:true});list.refresh();
  const first=plans.slice();plans.length=0;
  list.update(['a','b','c','d','e','f','g','h','i','j'],{defer:true});
  const during=plans.length;
  list.refresh({mode:'bottom'});
  const afterRefresh=plans.slice();plans.length=0;
  list.update(['a','b','c'],{defer:true});await Promise.resolve();await new Promise(r=>setTimeout(r,0));
  return {first,during,afterRefresh,fallback:plans.slice(),rows:el.querySelectorAll('.timeline-virtual-row').length,top:el.scrollTop};
 });
 assert.equal(out.during,0,'nothing planned while the patch runs');
 assert.equal(out.afterRefresh[0],'bottom','the render’s command plans the new rows');
 assert.ok(out.afterRefresh.length<=2,'one plan (and at most its measure): '+out.afterRefresh);
 assert.ok(out.fallback.length>=1,'no refresh: planned before the frame anyway');
 assert.equal(out.rows,3);
});
