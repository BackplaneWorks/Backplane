'use strict';
// Real Chromium layout/input around the two clamped geometries captured by
// continuous_history_runtime. No service, bundle, user home or live window.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_CORE||'/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const source=fs.readFileSync(process.env.HISTORY_HOST_SOURCE||path.resolve(__dirname,'../../src/web/host.js'),'utf8');
const history=source.slice(source.indexOf('let historyBase ='),source.indexOf('const thinMailDraft ='));
const gestures=source.slice(source.indexOf('function earlierCheck('),source.indexOf('document.addEventListener("click",',source.indexOf('function earlierCheck(')));
async function fixture(browser){
 const page=await browser.newPage({viewport:{width:1000,height:800}});
 await page.setContent('<style>body{margin:0}#timeline{height:605px;width:720px;box-sizing:border-box;overflow-y:auto;overflow-anchor:none;padding:16px 16px 8px;display:flex;flex-direction:column;gap:10px}#timeline>*{flex-shrink:0;box-sizing:border-box;margin:0}</style><div id="timeline"></div>');
 await page.evaluate(code=>{
  window.each=function*(xs){for(;xs?.$==='Con';xs=xs.tail)yield xs.head;};window.is=(n,t)=>n?.$===t;
  window.App={lang:()=> 'en',sel:()=> 'fixture',hold_at_end:n=>n<=2n};window.ui=null;window.cacheIdentity=x=>x;
  window.focusedNode=null;window.frontendNode={};window.viewMode='view-v1';window.earlierAsked=false;
  window.state={requests:0,newest:false,waiting:false,deltas:[],asked:[]};window.thin={atNewest:()=>state.newest,pageError:null,newer:()=>{
   if(state.waiting||state.newest)return false;state.asked.push(snapshot().anchor);state.requests++;state.waiting=true;
   setTimeout(()=>{state.waiting=false;state.newest=state.requests===8;const head=464+state.requests*16;
    show([mark(),fold(head,32),...(state.newest?[message(head+32,551),message(head+33,50),message(head+34,551),message(head+35,50)]:[])],'newer');},15);return true;
  }};window.dispatch=(a)=>a==='earlier'?false:true;
  (0,eval)(code.history+'\n'+code.gestures+`\n
  window.list=xs=>xs.reduceRight((tail,head)=>({$:"Con",head,tail}),{$:"Nil"});
  window.entry=(id,height,folded,ids)=>({$:"El",tag:folded?"button":"div",key:id,attrs:list([
   {$:"At",name:"style",value:"height:"+height+"px;min-height:"+height+"px"},
   ...(folded?[{$:"At",name:"class",value:"fold"},{$:"At",name:"data-value",value:"1"+id},{$:"At",name:"data-fold-ids",value:JSON.stringify(ids)},{$:"On",event:"click",action:"fold"}]:[{$:"At",name:"data-id",value:id}])]),kids:list([])});
  window.fold=(start,n)=>entry("row-"+start,24,true,Array.from({length:n},(_,i)=>"row-"+(start+i)));
  window.message=(id,height)=>entry("row-"+id,height,false);
  window.mark=()=>({$:"El",tag:"i",key:"earlier",attrs:list([{$:"At",name:"data-earlier",value:"fixture"},{$:"At",name:"style",value:"height:1px;min-height:1px"}]),kids:list([])});
  window.dom=node=>{const e=document.createElement(node.tag);for(const a of each(node.attrs))if(a.$==="At")e.setAttribute(a.name,a.value);return e;};
  window.publish=()=>{if(typeof historyAccepted==="function")historyAccepted();if(typeof historyPresented==="function")historyPresented();};
  window.show=(rows,direction)=>{
   if(typeof historyAccepted==="function")historyAccepted();
   const tl=document.getElementById("timeline");if(held)held=anchorOf(tl)||held;
   const before=held&&{...held};historyPage=direction?{direction}:null;
   const projected=historyProject({$:"El",tag:"div",key:"timeline",attrs:list([{$:"At",name:"id",value:"timeline"}]),kids:list(rows)});
   tl.replaceChildren(...[...each(projected.kids)].map(dom));if(typeof historyPresented==="function")historyPresented();holdFix(tl);holdWatch(tl);earlierAsked=laterAsked=false;
   if(before&&held){const e=anchorElement(tl,held.id);state.deltas.push({before:before.id,after:held.id,delta:e?e.getBoundingClientRect().top-before.off:null});}
   requestAnimationFrame(()=>laterCheck(tl));
  };
  window.snapshot=()=>{const tl=document.getElementById("timeline");return {requests:state.requests,newest:state.newest,presented:typeof historyPresentedNewest==="undefined"?state.newest:historyPresentedNewest,accepted:typeof historyAcceptedNewest==="undefined"?state.newest:historyAcceptedNewest,top:tl.scrollTop,height:tl.scrollHeight,view:tl.clientHeight,anchor:anchorOf(tl),held:held&&{...held,actual:anchorElement(tl,held.id)?.getBoundingClientRect().top},canNewer:historyCanPage(tl,"newer"),runways:tl.querySelectorAll("[data-history-runway]").length,asked:state.asked,deltas:state.deltas};};
  window.anchorCase=()=>{show([mark(),fold(464,32)]);const tl=document.getElementById("timeline");tl.scrollTop=0;held=anchorOf(tl);const before={...held};show([mark(),fold(448,8),message(456,551),message(457,50),message(458,551),message(459,50),fold(460,20)],"older");return {before,...snapshot()};};
  // These helpers share the extracted host block's lexical bindings. Assigning
  // held/historyBase from a later page.evaluate would create unrelated globals.
  window.replaceTimeline=()=>{state.newest=false;publish();holdFix(document.getElementById("timeline"));window.oldTimeline=document.getElementById("timeline");
   const fresh=document.createElement("div");fresh.id="timeline";fresh.appendChild(dom(fold(464,32)));oldTimeline.replaceWith(fresh);
   historyBase=[fold(464,32)];held=anchorOf(fresh);holdFix(fresh);return snapshot();};
  window.prepareHistoricalTimeline=()=>{frontendNode={};state.newest=false;publish();historyBase=[fold(464,32)];const tl=document.getElementById("timeline");held=anchorOf(tl);holdFix(tl);};
  window.currentSetup=()=>{historyBase=null;historySpare=[fold(704,16)];historySpare.direction="newer";held=null;state.newest=false;state.waiting=true;
   show([mark(),message(720,1000),message(721,1000),fold(726,18),message(744,1000),message(747,66.5),message(748,546.5),message(749,55),fold(750,2)]);
   const tl=document.getElementById("timeline");tl.scrollTop=3159;fixedAt=-1;held=anchorOf(tl);heldSize=holdSize(tl);state.deltas=[];return snapshot();};
  window.currentCommit=()=>{state.newest=true;show([mark(),fold(736,8),message(744,1000),message(747,66.5),message(748,546.5),message(749,55),fold(750,18)],"newer");return snapshot();};
  window.reverseCase=()=>{historyBase=null;historySpare=[fold(448,8),message(456,551),message(457,50),message(458,551),message(459,50)];historySpare.direction="newer";held=null;
   show([mark(),fold(464,32)]);const tl=document.getElementById("timeline");tl.scrollTop=730;fixedAt=-1;held=anchorOf(tl);heldSize=holdSize(tl);state.deltas=[];return snapshot();};
  `);
 },{history,gestures});return page;
}
test('history runway repairs clamped fold anchors and continuous newer traversal',{timeout:20000},async t=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',args:['--no-sandbox','--disable-dev-shm-usage']});t.after(()=>browser.close());
 if(process.env.HISTORY_NEW_BASELINE_ONLY!=='1') await t.test('coalescing the held final fold preserves its exact offset',async()=>{
  const page=await fixture(browser);try{const r=await page.evaluate(()=>anchorCase());console.log(JSON.stringify({case:'coalesced-anchor',...r}));assert.equal(r.deltas.at(-1).delta,0);assert.equal(r.held.id,'fold:row-460');assert.equal(r.held.actual,r.before.off);assert.equal(r.runways,1);}finally{await page.close();}
 });
 if(process.env.HISTORY_NEW_BASELINE_ONLY!=='1') await t.test('ordinary downward wheels enter the short canonical page, cross eight windows and remove runway at Current',async()=>{
  const page=await fixture(browser);try{const before=await page.evaluate(()=>reverseCase());assert.equal(before.anchor.id,'row-458');await page.mouse.move(350,300);
   for(let i=0;i<24;i++){await page.mouse.wheel(0,520);await page.waitForTimeout(45);if(await page.evaluate(()=>state.newest))break;}
   const r=await page.evaluate(()=>snapshot());console.log(JSON.stringify({case:'continuous-reverse',before,...r}));assert.equal(r.requests,8);assert.equal(r.newest,true);assert.equal(r.runways,0);assert.deepEqual(r.asked.map(a=>a?.id),Array.from({length:8},(_,i)=>'fold:row-'+(464+i*16)),'canonical real fold must grant every request');assert.equal(r.deltas.length,8);assert(r.deltas.every(a=>a.delta!==null&&Math.abs(a.delta)<=1),'reading anchor moved during newer traversal');
  }finally{await page.close();}
 });
 if(process.env.HISTORY_NEW_BASELINE_ONLY!=='1') await t.test('runway is bounded, inert, stable on resize/shrink, and cleared with view ownership',async()=>{
  const page=await fixture(browser);try{
   await page.evaluate(()=>{anchorCase();state.mutations=0;window.runwayMutations=new MutationObserver(ms=>state.mutations+=ms.length);runwayMutations.observe(document.getElementById('timeline'),{attributes:true,childList:true,subtree:true});});
   const frame=()=>page.evaluate(()=>new Promise(requestAnimationFrame));
   for(const height of [420,740,80,605]){
    await page.evaluate(height=>document.getElementById('timeline').style.height=height+'px',height);await frame();await frame();await page.waitForTimeout(25);
    const r=await page.evaluate(()=>snapshot());assert(Math.abs(r.held.actual-r.held.off)<=1,'resize clamped held fold');
    const attrs=await page.locator('[data-history-runway]').evaluate(e=>({height:e.getBoundingClientRect().height,view:e.parentNode.clientHeight,inert:e.hasAttribute('inert'),aria:e.getAttribute('aria-hidden'),row:e.matches('[data-id],button.fold')}));
    assert(attrs.height>=0&&attrs.height<=attrs.view);assert(attrs.inert);assert.equal(attrs.aria,'true');assert.equal(attrs.row,false);
   }
   await page.evaluate(()=>{state.mutations=0;for(let i=0;i<40;i++)holdFix(document.getElementById('timeline'));});await frame();await frame();
   assert.equal(await page.evaluate(()=>state.mutations),0,'repeated observer repair wrote layout again');assert.equal(await page.evaluate(()=>state.requests),0,'layout repair fabricated a paging gesture');
   await page.evaluate(()=>{state.newest=true;publish();holdFix(document.getElementById('timeline'));});assert.equal(await page.locator('[data-history-runway]').count(),0);
   const replacement=await page.evaluate(()=>replaceTimeline());assert.equal(replacement.held.id,'fold:row-464');assert.equal(replacement.runways,1);
   assert.equal(await page.evaluate(()=>oldTimeline.querySelectorAll('[data-history-runway]').length),0);assert.equal(await page.locator('[data-history-runway]').count(),1);
   await page.evaluate(()=>{frontendNode=null;historyClear();});await frame();await frame();assert.equal(await page.locator('[data-history-runway]').count(),0);
   await page.evaluate(()=>{prepareHistoricalTimeline();viewMode='compatibility';holdFix(document.getElementById('timeline'));});assert.equal(await page.locator('[data-history-runway]').count(),0);
   await page.evaluate(()=>{viewMode='view-v1';holdFix(document.getElementById('timeline'));holdFix(null);});assert.equal(await page.locator('[data-history-runway]').count(),0);
  }finally{await page.close();}
 });

 await t.test('rich-page reader motion cannot acquire the captured 195px artificial tail before Current',async()=>{
  const page=await fixture(browser);try{
   const request=await page.evaluate(()=>currentSetup());assert.equal(request.anchor.id,'row-747');assert.equal(request.anchor.off,-34);
   await page.mouse.move(350,300);await page.mouse.wheel(0,286);await page.waitForTimeout(120);
   const before=await page.evaluate(()=>snapshot());const after=await page.evaluate(()=>currentCommit());
   console.log(JSON.stringify({case:'current-195',request,before,after}));assert.equal(after.deltas.at(-1).before,'row-748');assert.equal(after.deltas.at(-1).after,'row-748');assert(Math.abs(after.deltas.at(-1).delta)<=1,'Current cleanup clamped the reader');assert.equal(after.runways,0);assert.equal(after.height,3821);assert.equal(after.top,3216);
  }finally{await page.close();}
 });
 await t.test('pending Current and accepted-but-unpainted Current cannot retire historical geometry',async()=>{
  const page=await fixture(browser);try{
   await page.evaluate(()=>{anchorCase();state.newest=true;holdFix(document.getElementById('timeline'));});
   const pending=await page.evaluate(()=>snapshot());assert(Math.abs(pending.held.actual-pending.held.off)<=1,'pending cursor retired presented runway');assert.equal(pending.presented,false);assert.equal(pending.runways,1);
   await page.evaluate(()=>{if(typeof historyAccepted==='function')historyAccepted();document.getElementById('timeline').style.height='420px';});await page.waitForTimeout(50);
   const accepted=await page.evaluate(()=>snapshot());assert.equal(accepted.accepted,true);assert.equal(accepted.presented,false);assert(Math.abs(accepted.held.actual-accepted.held.off)<=1,'ResizeObserver used accepted route before DOM presentation');assert.equal(accepted.requests,0);
   await page.evaluate(()=>show([mark(),fold(448,8),message(456,551),message(457,50),message(458,551),message(459,50),fold(460,19),message(479,900)],'newer'));
   const painted=await page.evaluate(()=>snapshot());assert.equal(painted.presented,true);assert.equal(painted.runways,0);assert(Math.abs(painted.held.actual-painted.held.off)<=1);
  }finally{await page.close();}
 });

});
