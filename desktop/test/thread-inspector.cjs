// Real production UI, isolated event store/profile, no real agents or account.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json')));
const baseline=process.env.INSPECTOR_BASELINE==='1',out=process.env.INSPECTOR_OUT;
assert(out&&path.isAbsolute(out));fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-inspector-')),project=path.join(home,'project'),agents=path.join(home,'agents');
fs.mkdirSync(project);fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
// A captured stand-in keeps Running observable until the real Stop handler acts.
fs.writeFileSync(path.join(agents,'codex'),`#!/bin/sh
printf '%s\\n' '{"type":"thread.started","thread_id":"fixture-session"}'
exec sleep 60
`,{mode:0o755});
const at=Math.floor(Date.now()/1000),ev=($,fields)=>({$,...fields,at}),tid='review',child='power-review';
const thread=(id,title)=>ev('ThreadCreated',{id,title,project:'fixture',env:'local',branch:'',worktree:project,provider:'codex',model:'gpt-6.1-sol'});
const events=[ev('ProjectCreated',{id:'fixture',title:'Sensor rev C',root:project}),thread(tid,'Coordinate rev C'),thread(child,'Power budget review'),thread('empty','No delegated work'),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>ev('SettingSet',{key,value:'off'})),ev('SettingSet',{key:'onboard.done',value:'1'}),
 ev('MessagePosted',{thread:tid,msg:'request',role:'user',text:'Review the sensor board. Keep the power and layout checks together here.'}),
 ev('MessagePosted',{thread:tid,msg:'response',role:'assistant',text:'The reviews have returned. Open a review to inspect its conversation, or expand the recent agent steps below.'}),
 ev('MessagePosted',{thread:child,msg:'child-body',role:'assistant',text:'Power budget: 18 months at the stated duty cycle. Local decoupling and regulator dropout checked.'}),
 ev('TaskDelegated',{task:'power',parent:tid,child,depth:1,provider:'claude',model:'sonnet'}),ev('TaskStateSet',{task:'power',state:'done',summary:'Power budget reviewed'}),ev('TaskDelivered',{task:'power'})];
for(const [call,title,end]of [['layout','Layout and connector review','done'],['failed','Thermal review','failed']]){
 for(const [kind,summary]of [['start:subagent',title],['Read','boards/sensor/sensor.kicad_pcb'],['say','Checked the antenna keepout and connector orientation. A long report/path/without/breaks/'.repeat(3)],['end',end]])events.push(ev('ActivityLogged',{thread:'@sub:'+tid,act:call,tone:call,kind,summary}));
}
events.push(...require('./thread-hierarchy.cjs').hierarchyEvents({at,home,project,threadId:tid}));
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
// This newly generated temp-home Browser fixture is not a copied user log.
// Copy mode intentionally drops B.Teach before TeachReq, so it cannot qualify
// takeover/handback. Only this explicit Browser-only suffix uses normal mode;
// stand-in agents, loopback/no-tailnet service, mail/telemetry off still apply.
const controlledBrowserOnly=process.env.INSPECTOR_HELPERS_ONLY==='1'&&process.env.BATCH_BROWSER==='1'&&process.env.BATCH_HIERARCHY==='0'&&process.env.BROWSER_CONTINUE_TAKEOVER==='1';
Object.assign(process.env,{BACKPLANE_AGENTS:agents,BACKPLANE_COPY:controlledBrowserOnly?'0':'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks'),BACKPLANE_DESKTOP_BROWSER:'1'});
if(controlledBrowserOnly&&process.env.BROWSER_CONTINUE_HISTORY==='1')process.env.BACKPLANE_LOG_FILE=path.join(out,'browser-service.log');
const binary=path.join(stage,'payload/backplane-serve'),web=path.join(stage,'payload/web'),service=new Service({home,binary,web,expectedVersion:info.version});
const continueChild=process.env.INSPECTOR_CONTINUE_CHILD==='1';
const retained=continueChild?{
 source:'f5eef0da17a48291da6827fa61e750ba622534ad',
 evidence:'/home/h/Documents/i2cjak/bp-coord/local-fast-ui/ui-batch/accept-corrections-20261023-022631/inspector',
 scope:'Layout expansion, dock location, workspace controls and Escape/Enter focus checks completed before the child-history assertion'
}:null;
if(retained){assert.equal(info.commit,retained.source);assert(fs.existsSync(path.join(retained.evidence,'FAIL.json')));assert(fs.existsSync(path.join(retained.evidence,'desktop.png')));}
let app,page;const errors=[],timings=[],checks=[];
(async()=>{try{
 const connection=await service.start();app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),args:[],env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});page=await app.firstWindow();page.on('pageerror',e=>errors.push(e.message));
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count()){await skip.click();await page.locator('.tour-back').waitFor({state:'hidden'});}
 const select=async id=>{await page.locator(`.sidebar [data-on-click="select"][data-value="${id}"]`).first().click();await page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',id);};
 await select(tid);await page.setViewportSize({width:1440,height:960});
 // Helper-only continuation reuses this exact stage and Electron fixture.
 // It deliberately does not turn skipped inspector checks into a PASS.
 if(process.env.INSPECTOR_HELPERS_ONLY==='1'){
  assert(!baseline,'helper acceptance requires the combined candidate');
  assert(process.env.BATCH_BROWSER==='1'||process.env.BATCH_HIERARCHY==='1','choose at least one pending helper');
  const helpers={};
  let helperFailed=false;
  if(process.env.BATCH_BROWSER==='1'){try{helpers.browser=await require('./shared-browser.cjs').runSharedBrowserAcceptance({app,page,origin:connection.origin,token:await service.token(),threadId:tid,otherThreadId:'empty',out:path.join(out,'shared-browser')});}catch(e){helperFailed=true;helpers.browser={state:'failed',error:e.message};}}
  if(process.env.BATCH_HIERARCHY==='1'){try{const close=page.locator('#viewer [data-on-click=\"panel\"],.workspace-bar .viewer-close');if(await close.count())await close.first().click();helpers.hierarchy=await require('./thread-hierarchy.cjs').runHierarchyAcceptance({app,page,service,home,project,threadId:tid,child,out:path.join(out,'hierarchy')});}catch(e){helperFailed=true;helpers.hierarchy={state:'failed',error:e.message};}}
  assert.equal(errors.length,0,errors.join('\n'));
  fs.writeFileSync(path.join(out,'HELPERS-RESULT.json'),JSON.stringify({state:helperFailed?'failed':'passed',scope:'selected runtime helpers only',inspector:'NOT RUN',source:info.commit,version:info.version,stage,home,fixtureMode:{controlledBrowserOnly,copy:process.env.BACKPLANE_COPY},helpers,errors},null,2));
  console.log(JSON.stringify({out,scope:'selected runtime helpers only',inspector:'NOT RUN',helpers}));if(helperFailed)process.exitCode=1;return;
 }
 const head=page.locator('.subs-head');if((await head.getAttribute('data-on-click'))==='subs-open')await head.click();await page.locator('.sub-row').first().waitFor();
 const finished=page.locator('.sub-row[data-value*="subs.done:"]');if(await finished.count())await finished.click();
 if(!continueChild){await page.locator('.sub-row').filter({hasText:'Layout and connector review'}).click();await page.locator('.sub-step').first().waitFor();
 await page.screenshot({path:path.join(out,'desktop.png')});}
 if(!baseline){
  if(!continueChild){assert.equal(await page.locator('.thread-view > section.subs').count(),1);assert.equal(await page.locator('.timeline .subs').count(),0);checks.push('inspector is docked outside timeline');
  assert.equal(await page.locator('.thread-head [data-on-click="interrupt"]').count(),0);assert.equal(await page.locator('.workspace-bar [data-on-click="term-toggle"][data-value="@term-size"]').count(),1);checks.push('flat workspace and terminal contract');
  await page.locator('.sub-row').filter({hasText:'Layout and connector review'}).focus();await page.keyboard.press('Escape');await page.waitForFunction(()=>document.querySelector('.subs-head')?.getAttribute('aria-expanded')==='false');assert(await head.evaluate(el=>el===document.activeElement));await page.keyboard.press('Enter');await page.locator('.sub-row').first().waitFor();checks.push('Escape collapse restores focus; Enter reopens');}
  await page.locator('.sub-row[data-on-click="select"]').filter({hasText:'Power budget review'}).click();// Selection updates before the selected view/body arrives; wait for its
  // real connected presentation and the expected child message, then assert.
  await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='power-review'&&document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main')?.getAttribute('aria-busy')==='false');
  await page.locator('main[data-thread="power-review"] [data-id="child-body"] .body').waitFor();
  assert((await page.locator('main').innerText()).includes('18 months'));await page.locator('button.parent').click();await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='review'&&document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main')?.getAttribute('aria-busy')==='false');checks.push('child conversation and parent navigation');
  await page.locator('.thread-controls [data-on-click="delete"]').click();await page.locator('.thread-controls [data-on-click="delete-no"]').waitFor();await page.locator('.thread-controls [data-on-click="delete-no"]').click();await page.locator('.thread-controls [data-on-click="delete-no"]').waitFor({state:'hidden'});checks.push('delete confirmation retained without deleting');
  const composer=page.locator('textarea[data-paste="attach"]');await composer.fill('Workbar Stop acceptance');await composer.press('Enter');await page.locator('.thread-workbar [data-on-click="interrupt"]').waitFor();
  assert.equal(await page.locator('.thread-head [data-on-click="interrupt"]').count(),0);await page.screenshot({path:path.join(out,'workbar-running-desktop.png')});await page.locator('.thread-workbar [data-on-click="interrupt"]').click();await page.locator('.thread-workbar [data-on-click="interrupt"]').waitFor({state:'hidden'});checks.push('actual Running Stop in workbar interrupts captured stand-in');

 }
 const viewerButton=baseline?page.locator('.thread-head [data-on-click="panel"]'):page.locator('.workspace-bar [data-on-click="view-kind"][data-value="board"]');
 const began=performance.now();await viewerButton.click();await page.locator('#viewer').waitFor();timings.push({action:'open-board',readyMs:performance.now()-began});
 await page.evaluate(()=>{window.__viewerMeasures=[];document.addEventListener('click',e=>{const el=e.target.closest('[data-on-click="view-kind"]');if(!el)return;const at=performance.now(),value=el.dataset.value;const sample=()=>{const current=document.querySelector('.viewer-tabs .tab.on,.workspace-bar .tab.on'),pending=document.querySelector('[data-viewer-pending]');if(current?.dataset.value===value||pending){window.__viewerMeasures.push({value,feedbackMs:performance.now()-at,pending:!!pending});return;}if(performance.now()-at<5000)requestAnimationFrame(sample);};requestAnimationFrame(sample);},true);});
 const nav=baseline?'.viewer-tabs':'.workspace-bar';
 const schematic=page.locator(`${nav} [data-on-click="view-kind"][data-value="schematic"]`);await schematic.click();await page.waitForFunction(()=>document.querySelector('#viewer')?.classList.contains('schematic'));timings.push(...await page.evaluate(()=>window.__viewerMeasures));
 await page.screenshot({path:path.join(out,'viewer-desktop.png')});
 await page.locator('#viewer [data-on-click="panel"],.workspace-bar .viewer-close').first().click();await page.locator('#viewer').waitFor({state:'hidden'});
 await page.setViewportSize({width:390,height:844});await page.screenshot({path:path.join(out,'mobile390.png')});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no390px document overflow');
 assert(await page.locator('.composer').evaluate(el=>{const r=el.getBoundingClientRect();return r.bottom<=innerHeight&&r.top>=0;}),'composer remains reachable');checks.push('390px no overflow/composer visible');
 if(!baseline){await page.locator('.workspace-bar [data-on-click="view-kind"][data-value="board"]').click();await page.locator('#viewer').waitFor();await page.screenshot({path:path.join(out,'viewer-mobile390.png')});await page.locator('#viewer [data-on-click="panel"]').click();await page.locator('#viewer').waitFor({state:'hidden'});checks.push('mobile viewer full-page and back');}
 if(process.env.BATCH_HIERARCHY==='1')await require('./thread-hierarchy.cjs').runHierarchyAcceptance({app,page,service,home,project,threadId:tid,child,out:path.join(out,'hierarchy')});
 if(process.env.BATCH_BROWSER==='1'){await page.setViewportSize({width:1440,height:960});await require('./shared-browser.cjs').runSharedBrowserAcceptance({app,page,origin:connection.origin,token:await service.token(),threadId:tid,otherThreadId:'empty',out:path.join(out,'shared-browser')});}
 assert.equal(errors.length,0,errors.join('\n'));fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify({state:'passed',baseline,retainedEvidence:retained,source:info.commit,version:info.version,stage,out,home,checks,timings,errors},null,2));console.log(JSON.stringify({out,checks,timings}));
 }catch(error){process.exitCode=1;console.error(error);if(page){await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>''));}fs.writeFileSync(path.join(out,'FAIL.json'),JSON.stringify({error:error.stack,errors},null,2));}
 finally{if(app)await app.close();service.stop();}
})();
