// Continuous traversal through generated rich/tool-heavy history. No live home/UI.
// node test/tools/continuous_history_runtime.cjs SAFE_STAGE WEB OUT [wheel|keyboard|touch]
'use strict';
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_CORE || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const { Service } = require('../../desktop/service.cjs');
const { cbor, plain } = require('../../src/web/wire.js');
const [stage, web, out, input = 'wheel'] = process.argv.slice(2);
assert([stage, web, out].every(p => p && path.isAbsolute(p)));assert(['wheel','keyboard','touch'].includes(input));
fs.mkdirSync(out, { recursive: true });
const source = fs.readFileSync(path.join(__dirname, '../../src/core/cbor.bend'), 'utf8');
const dict = name => JSON.parse(new RegExp('def Cbor\\.' + name + '\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])').exec(source)[1]);
const keys = dict('keys'), words = dict('words');
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-continuous-')), project = path.join(home, 'project'), agents = path.join(home, 'agents');
fs.mkdirSync(project);fs.mkdirSync(agents);
for (const name of ['claude','codex','grok','muse','agy','opencode','pi','omp']) fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
let at = 1793000000, events = [];
const event = ($, data) => events.push({ $, ...data, at: at++ });
event('ProjectCreated', { id:'fixture', title:'Continuous history fixture', root:project });
event('ThreadCreated', { id:'rich', project:'fixture', title:'Rich and tool-heavy', env:'local', branch:'', worktree:project, provider:'codex', model:'' });
for (let i = 0; i < 768; i++) {
  const id = 'row-' + String(i).padStart(6, '0');
  if (i % 24 >= 6 || (i >= 460 && i < 640)) event('ActivityLogged', { thread:'rich', act:id, tone:'tool', kind:'shell', summary:('Generated inspection ' + i + '\n').repeat(12) });
  else event('MessagePosted', { thread:'rich', msg:id, role:i % 2 ? 'user' : 'assistant', text:i % 2 ? ('Question ' + i + '. ').repeat(10) : ('Answer ' + i + '.\n\n| Part | Check |\n|---|---|\n| fixture | pass |\n\n- Detail\n\n```js\nconst n = 42;\n```\n\n').repeat(3) });
}
for (const key of ['restart.continue','mail.on','telemetry.share']) event('SettingSet',{key,value:'off'});
event('SettingSet',{key:'onboard.done',value:'1'});
const text = events.map(JSON.stringify).join('\n') + '\n';fs.writeFileSync(path.join(home,'events.jsonl'),text);events = null;
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks'});
const result = { input, stage, web, home, fixtureEntries:768, fixtureBytes:Buffer.byteLength(text), requests:[], replies:[], samples:[], errors:[], limits:['Generated isolated history only; no claim about selected actual conversation.','Linux headless Chromium; physical inertia, native clients and native IME not qualified.','CDP heap is measured separately from serialized fixture/metadata bytes.'] };
let service, browser, page;
function sample() {
 const tl = document.getElementById('timeline'), box = tl.getBoundingClientRect();
 const rows = [...tl.querySelectorAll('[data-id],button.fold')].filter(e=>e.getBoundingClientRect().height>0);
 const visible=rows.filter(e=>{const r=e.getBoundingClientRect();return r.bottom>box.top+1&&r.top<box.bottom;});
 const id=e=>e.dataset.id||'fold:'+e.dataset.value.slice(1), anchor=visible[0];
 const folds=[...tl.querySelectorAll('button.fold')].map(e=>({head:e.dataset.value.slice(1),value:e.dataset.value,ids:e.dataset.foldIds?JSON.parse(e.dataset.foldIds):null}));
 return {folds,top:tl.scrollTop,height:tl.scrollHeight,viewport:tl.clientHeight,rows:rows.map(id),visible:visible.map(id),nodes:tl.querySelectorAll('*').length,anchor:anchor&&{id:id(anchor),offset:anchor.getBoundingClientRect().top-box.top},draft:document.getElementById('composer')?.value,pages:__bpPerf.trace.filter(x=>x.phase==='thin-view').length,sends:__bpPerf.trace.filter(x=>x.phase==='send'&&x.method==='view.page').length};
}
(async()=>{try{
 service=new Service({home,binary:path.join(stage,'payload/backplane-serve'),web,expectedVersion:JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json'))).version});const connection=await service.start();
 browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',args:['--no-sandbox','--disable-dev-shm-usage','--enable-precise-memory-info']});
 const context=await browser.newContext({viewport:{width:1280,height:800},hasTouch:input==='touch'});
 await context.request.post(connection.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}});
 page=await context.newPage();page.setDefaultTimeout(10000);page.on('pageerror',e=>result.errors.push(String(e)));
 page.on('websocket',ws=>{
  ws.on('framesent',({payload})=>{try{const o=plain(cbor(new Uint8Array(payload),keys,words));if(o.m==='view.page')result.requests.push({id:o.id,cursor:o.p.cursor,anchor:o.p.anchor});}catch{}});
  ws.on('framereceived',({payload})=>{try{const o=plain(cbor(new Uint8Array(payload),keys,words));if(o.result?.window)result.replies.push({id:o.id,window:o.result.window});}catch{}});
 });
 await page.addInitScript(()=>{
  localStorage.setItem('backplane-selected','rich');window.__bpPerf={render:[],act:[],part:[],live:[],trace:[]};window.__historyAnchors=[];
  const raf=window.requestAnimationFrame.bind(window);
  const snapshot=()=>{const t=document.getElementById('timeline');if(!t)return null;const top=t.getBoundingClientRect().top;const rows=[...t.querySelectorAll('[data-id],button.fold')];const a=rows.find(e=>e.getBoundingClientRect().height>0&&e.getBoundingClientRect().bottom>top+1);return a?{id:a.dataset.id||'fold:'+a.dataset.value.slice(1),off:a.getBoundingClientRect().top,ids:a.dataset.foldIds?JSON.parse(a.dataset.foldIds):[a.dataset.id||a.dataset.value.slice(1)],rows:rows.map(e=>e.dataset.id||'fold:'+e.dataset.value.slice(1)).join('|')}:null;};
  window.requestAnimationFrame=fn=>raf(time=>{const before=snapshot();const value=fn(time),after=snapshot();if(before&&after&&before.rows!==after.rows){const t=document.getElementById('timeline');let el=[...t.querySelectorAll('[data-id],button.fold')].find(e=>(e.dataset.id||'fold:'+e.dataset.value.slice(1))===before.id);if(!el)el=[...t.querySelectorAll('button.fold[data-fold-ids]')].find(e=>JSON.parse(e.dataset.foldIds).some(id=>before.ids.includes(id)));__historyAnchors.push({id:before.id,after:el&&(el.dataset.id||'fold:'+el.dataset.value.slice(1)),delta:el?el.getBoundingClientRect().top-before.off:null});}return value;});
 });
 await page.goto(connection.origin);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();await page.waitForFunction(()=>!document.querySelector('.app.tour-on'));
 const draft='Unsent fixture 日本語 👩‍💻';await page.locator('#composer').fill(draft);await page.waitForTimeout(200);
 await page.evaluate(()=>{window.fixtureComposer=document.getElementById('composer');fixtureComposer.setSelectionRange(3,8);fixtureComposer.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));});
 const cdp=await context.newCDPSession(page);await cdp.send('HeapProfiler.collectGarbage');result.initialHeap=await cdp.send('Runtime.getHeapUsage');result.initialDom=await cdp.send('Memory.getDOMCounters');
 result.initial=await page.evaluate(sample);
 await page.locator('#timeline').evaluate(t=>t.setAttribute('tabindex','0'));await page.locator('#timeline').focus();
 const box=await page.locator('#timeline').boundingBox(),x=box.x+box.width/2;await page.mouse.move(x,box.y+box.height/2);
 let noProgress=0, previous=result.initial, step;
 const earliest=()=>Math.min(...(result.replies.at(-1)?.window.ids||['9999']).map(Number));
 for(step=0;step<600;step++){
  // No scrollTop assignment: every page boundary is reached by trusted input.
  if(input==='wheel')await page.mouse.wheel(0,-520);
  else if(input==='keyboard')await page.keyboard.press('PageUp');
  else {const y=box.y+Math.min(120,box.height/3);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});for(let j=1;j<=5;j++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x,y:y+j*40}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});}
  await page.waitForTimeout(input==='keyboard'?110:70);
  const s=await page.evaluate(sample);result.samples.push({step,...s});
  assert.equal(new Set(s.rows).size,s.rows.length,'duplicate rendered rows');
  const ordered=s.rows.map(id=>id.replace(/^fold:/,''));assert(ordered.every((id,n)=>!n||id>ordered[n-1]),'out of order rendered rows');
  const tool = n => n % 24 >= 6 || n >= 460 && n < 640;
  const run = id => {let n=Number(id.slice(9));while(n>0&&tool(n-1))n--;return n;};
  const folds=s.rows.filter(id=>id.startsWith('fold:')).map(run);
  if (new Set(folds).size !== folds.length) {
   const buttons=await page.locator('#timeline button.fold').evaluateAll(xs=>xs.map(e=>({head:e.dataset.value.slice(1),label:e.textContent,value:e.dataset.value})));
   const repeated=buttons.filter((b,i)=>buttons.some((a,j)=>j<i&&run('fold:'+a.head)===run('fold:'+b.head))).at(-1);
   const sharedWith=buttons.find(b=>b!==repeated&&run('fold:'+b.head)===run('fold:'+repeated.head));
   const n=Number(repeated.head.slice(4)), end=(()=>{let i=n;while(tool(i))i++;return i;})();
   const windows=result.replies.map(r=>r.window.ids.map(n=>'row-'+String(Number(n)-1).padStart(6,'0')));
   const represented=head=>windows.filter(ids=>ids.includes(head)).map(ids=>ids.filter(id=>{const i=Number(id.slice(4));return i>=Number(head.slice(4))&&i<end;})).sort((a,b)=>b.length-a.length)[0]||[];
   const oldIds=represented(repeated.head), currentIds=represented(sharedWith.head);
   result.duplicate={buttons,sharedEntryIds:oldIds.filter(id=>currentIds.includes(id)),before:s};
   await page.locator('#timeline button.fold[data-value="1'+repeated.head+'"]').click();await page.waitForFunction(n=>__bpPerf.trace.filter(x=>x.phase==='thin-view').length>n,s.pages);await page.waitForTimeout(40);
   result.duplicate.afterClick=await page.evaluate(sample);
   result.duplicate.clicked=await page.locator('#timeline button.fold').evaluateAll((xs,head)=>xs.filter(e=>e.dataset.value.slice(1)===head).map(e=>({head,label:e.textContent,value:e.dataset.value})),repeated.head);
   assert.fail('overlapping page boundaries duplicated tool entries; retained fold cannot expand');
  }
  assert.equal(s.draft,draft,'draft changed');
  if(s.sends===previous.sends&&s.top===previous.top&&JSON.stringify(s.rows)===JSON.stringify(previous.rows))noProgress++;else noProgress=0;
  previous=s;
  if(earliest()<=385)break;
  assert(noProgress<24,'continuous '+input+' stalled before 24 page requests');
 }
 assert(earliest()<=385,'continuous input did not traverse rich/tool-heavy history');
 result.deep=await page.evaluate(sample);
 // Return through the same issued pages using normal downward input.
 for(let n=0;n<600;n++){
  if(input==='wheel')await page.mouse.wheel(0,520);else if(input==='keyboard')await page.keyboard.press('PageDown');else{const y=box.y+Math.min(320,box.height-40);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});for(let j=1;j<=5;j++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x,y:y-j*40}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});}
  await page.waitForTimeout(input==='keyboard'?110:70);const s=await page.evaluate(sample);result.samples.push({direction:'newer',step:n,...s});
  assert.equal(new Set(s.rows).size,s.rows.length,'duplicate rendered row during reversal');assert.equal(s.draft,draft);
  if(result.replies.at(-1)?.window.ids[0]==='768'&&s.height-s.top-s.viewport<=2)break;
  assert(n<599,'continuous reversal did not return to Current');
 }
 result.final=await page.evaluate(sample);result.composerKept=await page.evaluate(()=>fixtureComposer===document.getElementById('composer')&&fixtureComposer.selectionStart===3&&fixtureComposer.selectionEnd===8);assert(result.composerKept);
 await cdp.send('HeapProfiler.collectGarbage');result.finalHeap=await cdp.send('Runtime.getHeapUsage');result.finalDom=await cdp.send('Memory.getDOMCounters');result.anchors=await page.evaluate(()=>__historyAnchors);
 if(process.env.REQUIRE_COVERAGE==='1')assert(result.anchors.every(a=>a.delta!==null&&Math.abs(a.delta)<=1),'page transition moved/lost reading anchor');
 result.perf=await page.evaluate(()=>__bpPerf);assert.equal(result.errors.length,0);
 if(process.env.REQUIRE_COVERAGE==='1') {
  // Separate control scenario, after the continuous roundtrip. The click may
  // scroll its button into view; it never substitutes for traversal input.
  let retained;
  for(let n=0;n<100&&!retained;n++){
   await page.mouse.wheel(0,-520);await page.waitForTimeout(90);
   // One trusted gesture can complete several asynchronous adjacent pages.
   // Choose the control after those replies settle, so the click refers to
   // the retained page actually on screen rather than an intermediate one.
   await page.waitForTimeout(1300);await page.waitForFunction(()=>!document.getElementById('history-status'));
   const s=await page.evaluate(sample),canonical=new Set(result.replies.at(-1).window.ids.map(n=>'row-'+String(Number(n)-1).padStart(6,'0')));
   retained=s.folds.find(f=>!canonical.has(f.head)&&f.value.startsWith('1')&&f.ids?.length);
   if(retained)result.retainedSelection={sample:s,requests:result.requests.length,window:result.replies.at(-1).window,head:retained.head};
  }
  assert(retained,'no retained control was exercised');
  await page.evaluate(fn=>{const snapshot=(0,eval)('('+fn+')');document.addEventListener('click',e=>{if(e.target.closest?.('#timeline button.fold'))window.fixtureClickAnchor=snapshot();},{capture:true,once:true});},sample.toString());
  const requestsBefore=result.requests.length;
  await page.locator('#timeline button.fold[data-value="1'+retained.head+'"]').click();
  await page.waitForFunction(id=>document.querySelector('#timeline button.fold[data-value="0'+id+'"]'),retained.head);await page.waitForTimeout(40);
  const before=await page.evaluate(()=>fixtureClickAnchor),opened=await page.evaluate(sample),calls=opened.rows.filter(id=>retained.ids.includes(id));
  assert(result.requests.length>requestsBefore,'retained fold bypassed adjacent page authority');assert.deepEqual(calls,retained.ids,'retained fold opened wrong calls or lost unique calls');
  await page.locator('#timeline button.fold[data-value="0'+retained.head+'"]').click();await page.waitForFunction(id=>document.querySelector('#timeline button.fold[data-value="1'+id+'"]'),retained.head);
  const closed=await page.evaluate(sample);assert(!closed.rows.some(id=>retained.ids.includes(id)),'retained fold failed to close');
  result.retained={head:retained.head,expected:retained.ids,before,opened,closed};
 }
 result.state='passed';
}catch(e){result.state='failed';result.error=e.stack;process.exitCode=1;if(page)try{result.failure=await page.evaluate(sample);result.anchors=await page.evaluate(()=>__historyAnchors);result.perf=await page.evaluate(()=>__bpPerf);}catch{}}
finally{if(browser)await browser.close();if(service)await service.stop();fs.writeFileSync(path.join(out,input+'.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({state:result.state,input,pages:result.requests.length,samples:result.samples.length,error:result.error}));}
})();
