'use strict';
// One private long-history smoke of the actual Bend-generated client.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || '../../desktop/node_modules/playwright-core');
const { Service } = require('../../desktop/service.cjs');
const [stage, entries, out] = process.argv.slice(2);
assert([stage, entries, out].every(p => p && path.isAbsolute(p)), 'absolute stage, copied entries and output required');
fs.mkdirSync(out, { recursive: true, mode: 0o700 });
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-bend-timeline-'));
const project = path.join(home, 'project'), agents = path.join(home, 'agents');
fs.mkdirSync(project);fs.mkdirSync(agents);
for (const name of ['claude','codex','grok','muse','agy','opencode','pi','omp']) fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const copied = fs.readFileSync(entries,'utf8').trim().split('\n').map(JSON.parse).map(e => ({...e,thread:'history-copy'}));
const events = [{$:'ProjectCreated',id:'fixture',title:'Private timeline smoke',root:project,at:1},
  {$:'ThreadCreated',id:'history-copy',project:'fixture',title:'Largest copied conversation',env:'local',branch:'',worktree:project,provider:'codex',model:'',at:2},
  ...copied,...['restart.continue','mail.on','telemetry.share'].map(key=>({$:'SettingSet',key,value:'off',at:1})),
  {$:'SettingSet',key:'onboard.done',value:'1',at:1},{$:'SettingSet',key:'onboard.uber',value:'1',at:1},{$:'SettingSet',key:'order.enabled',value:'off',at:1},{$:'SettingSet',key:'order.estimates',value:'off',at:1}];
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n',{mode:0o600});
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks'});
const payload = fs.existsSync(path.join(stage,'payload')) ? path.join(stage,'payload') : stage;
const version = JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json'),'utf8')).version;
const result = {state:'running',stage,entries,home,copiedEntries:copied.length,bytes:fs.statSync(entries).size,errors:[],pages:[],limits:['One headless Chromium smoke at desktop dimensions, not a physical-display or native-phone qualification.','Actual copied content with isolated project/thread wrappers and blocked providers; no live history changed.']};
let service, browser, page;
function probe() {
  localStorage.setItem('backplane-selected','history-copy');
  window.__bpPerf={render:[],act:[],part:[],live:[],trace:[]};
  window.timelineSmoke={frames:[],samples:[],active:false};
  let prior;
  function frame(now){ if(prior && timelineSmoke.active) timelineSmoke.frames.push(now-prior);prior=now;requestAnimationFrame(frame); }
  requestAnimationFrame(frame);
}
function sample() {
  const tl=document.getElementById('timeline'),box=tl.getBoundingClientRect();
  const rows=[...tl.querySelectorAll('.timeline-virtual-row')];
  const visible=rows.filter(e=>{const r=e.getBoundingClientRect();return r.height>0&&!e.dataset.virtualKey?.startsWith('control:')&&r.bottom>box.top+1&&r.top<box.bottom;});
  const anchor=visible[0];
  return {top:tl.scrollTop,height:tl.scrollHeight,view:tl.clientHeight,mounted:rows.length,
    anchor:anchor?{key:anchor.dataset.virtualKey,offset:anchor.getBoundingClientRect().top-box.top}:null,
    draft:document.getElementById('composer')?.value,
    pages:__bpPerf.trace.filter(e=>e.phase==='send'&&e.method==='view.page').length,
    replies:__bpPerf.trace.filter(e=>e.phase==='thin-view').length,
    maxDocumentHeight:document.documentElement.scrollHeight,viewportHeight:innerHeight};
}
async function settle(){await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));}
async function quiet(){
 await page.waitForFunction(()=>{
  const sends=__bpPerf.trace.filter(e=>e.phase==='send'&&e.method?.startsWith('view.'));
  return !document.getElementById('history-status')&&(!sends.length||performance.now()-sends.at(-1).ms>200);
 },null,{timeout:15000});await settle();
}
(async()=>{
 try {
  service=new Service({home,binary:path.join(payload,'backplane-serve'),web:path.join(payload,'web'),expectedVersion:version});
  const connection=await service.start();
  browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',args:['--no-sandbox','--disable-dev-shm-usage']});
  const context=await browser.newContext({viewport:{width:1440,height:900}});
  await context.request.post(connection.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}});
  page=await context.newPage();page.setDefaultTimeout(20000);page.on('pageerror',e=>result.errors.push(String(e)));
  await page.addInitScript(probe);await page.goto(connection.origin);
  await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
  for(let n=0;n<4&&await page.locator('[data-on-click="ob-skip"]').count();n++){await page.locator('[data-on-click="ob-skip"]').click();await settle();}
  await page.waitForFunction(()=>!document.querySelector('.app.tour-on'));await quiet();
  assert(await page.locator('.timeline-virtual-track').count(),'Bend virtual list mounted');
  await page.locator('#composer').fill('Unsent Bend timeline 日本語 👩‍💻');await quiet();
  await page.evaluate(()=>{window.keptComposer=document.getElementById('composer');keptComposer.setSelectionRange(3,8);timelineSmoke.active=true;});
  result.start=await page.evaluate(sample);
  const box=await page.locator('#timeline').boundingBox();await page.mouse.move(box.x+box.width/2,box.y+40);
  for(let n=0;n<3;n++){
   await quiet();await page.mouse.wheel(0,1);await settle();await page.evaluate(()=>document.getElementById('timeline').scrollTop=0);await settle();await quiet();
   const before=await page.evaluate(sample);await page.mouse.wheel(0,-1);
   await page.waitForFunction(count=>__bpPerf.trace.filter(e=>e.phase==='thin-view').length>count,before.replies);
   await quiet();
   const after=await page.evaluate(sample);
   const preserved=await page.locator('#timeline').evaluate((tl,anchor)=>{const el=[...tl.querySelectorAll('.timeline-virtual-row')].find(e=>e.dataset.virtualKey===anchor?.key);return el?el.getBoundingClientRect().top-tl.getBoundingClientRect().top:null},before.anchor);
   result.pages.push({before,after,anchorDrift:preserved===null?null:preserved-before.anchor.offset});
  }
  // Exercise both directions with real trusted wheel input.
  for(const delta of [-1600,900,-700,1800]){await page.mouse.wheel(0,delta);await settle();}
  await quiet();
  result.end=await page.evaluate(sample);result.effects=await page.evaluate(()=>({composerKept:keptComposer===document.getElementById('composer'),selectionKept:keptComposer.selectionStart===3&&keptComposer.selectionEnd===8,frames:timelineSmoke.frames,perf:__bpPerf}));
  assert.equal(result.end.draft,result.start.draft);assert(result.effects.composerKept);assert(result.effects.selectionKept);
  assert.equal(result.errors.length,0,result.errors.join('\n'));
  assert(result.end.maxDocumentHeight<=result.end.viewportHeight,'no page overflow');
  assert(result.pages.every(p=>p.anchorDrift!==null&&Math.abs(p.anchorDrift)<=0.5),'prepend anchor drift >0.5 CSS px');
  await page.screenshot({path:path.join(out,'desktop.png')});
  result.state='passed';
 } catch(e){result.state='failed';result.error=e.stack;process.exitCode=1;if(page){try{result.failure=await page.evaluate(sample);await page.screenshot({path:path.join(out,'failure.png')});}catch{}}}
 finally{if(browser)await browser.close();if(service)await service.stop();fs.writeFileSync(path.join(out,'SMOKE.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({state:result.state,error:result.error,pages:result.pages.map(p=>({drift:p.anchorDrift,mounted:p.after.mounted})),out}));}
})();
