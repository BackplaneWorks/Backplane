'use strict';
// Real Bend web client with a private hub. Delay only action replies so the
// pre-ACK bubble and the final canonical entry can be checked independently.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const {Service}=require('../service.cjs');
const root=path.resolve(__dirname,'../..'),web=path.resolve(process.argv[2]||root+'/build/dev/web');
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-thread-entry-')),out=root+'/build/thread-entry-feedback';fs.mkdirSync(out,{recursive:true});
const project=home+'/project',agents=home+'/agents';fs.mkdirSync(project);fs.mkdirSync(agents);
for(const name of['claude','codex','grok','muse','agy','opencode','pi','omp'])fs.writeFileSync(agents+'/'+name,'#!/bin/sh\nexit 127\n',{mode:0o755});
const committedSelector='#timeline .msg.user[data-id]:not(.sending):not(.live):not(.working)';
const events=[{$:'ProjectCreated',id:'fixture',title:'Timeline fixture',root:project,at:1}];
for(const id of['short','long']){
 events.push({$:'ThreadCreated',id,project:'fixture',title:id+' conversation',env:'local',branch:'',worktree:project,provider:'claude',model:'sonnet',at:2});
 for(let i=0;i<(id==='short'?2:70);i++)events.push({$:'MessagePosted',thread:id,msg:id+'-'+i,role:i%2?'assistant':'user',text:id+' message '+i+(id==='long'?'\n\n'+('Measuring history without a gap. '.repeat(i%7+1)):''),at:3+i});
}
if(process.env.ENTRY_AUTOFILL==='1'){
 events.push({$:'ThreadCreated',id:'folded',project:'fixture',title:'folded history autofill',env:'local',branch:'',worktree:project,provider:'claude',model:'sonnet',at:2},
  {$:'MessagePosted',thread:'folded',msg:'folded-start',role:'user',text:'Oldest committed message reached through folded history.',at:3});
 for(let i=0;i<90;i++)events.push({$:'ActivityLogged',thread:'folded',act:'folded-tool-'+i,tone:'tool',kind:'shell',summary:'Collapsed tool '+i,at:4+i});
 events.push({$:'MessagePosted',thread:'folded',msg:'folded-end',role:'assistant',text:'Newest committed message after folded tools.',at:95});
}
for(const key of['restart.continue','mail.on','telemetry.share','order.enabled','order.estimates'])events.push({$:'SettingSet',key,value:'off',at:1});
for(const key of['onboard.done','onboard.uber'])events.push({$:'SettingSet',key,value:'1',at:1});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n',{mode:0o600});
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks'});
const service=new Service({home,binary:process.env.TIMELINE_SERVICE||'/home/h/.local/share/backplane/backplane-serve',web});
let browser,page;const errors=[],results=[];
function geometry(){const t=document.getElementById('timeline'),b=t.getBoundingClientRect(),rows=[...t.querySelectorAll('.timeline-virtual-row')].filter(e=>e.getBoundingClientRect().height>0);const last=rows.at(-1)?.getBoundingClientRect();return{gap:last?b.bottom-last.bottom:null,top:t.scrollTop,height:t.scrollHeight,view:t.clientHeight,keys:rows.map(e=>e.dataset.virtualKey)};}
(async()=>{try{
 const connection=await service.start();browser=await chromium.launch({executablePath:process.env.BACKPLANE_CHROME||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true});
 for(const viewport of[{width:1440,height:1000},{width:390,height:844}]){
  const context=await browser.newContext({viewport});await context.request.post(connection.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}});
  page=await context.newPage();page.on('pageerror',e=>errors.push(String(e)));
  await page.addInitScript(()=>{if(!localStorage.getItem('backplane-selected'))localStorage.setItem('backplane-selected','short');});
  let heldReplies=[],delayActions=false;
  await page.routeWebSocket(/.*/,ws=>{const server=ws.connectToServer();server.onMessage(message=>{if(delayActions)heldReplies.push(()=>ws.send(message));else ws.send(message);});});
  await page.goto(connection.origin);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
  await page.waitForTimeout(150);let g=await page.evaluate(geometry);assert(g.gap>=0&&g.gap<35,JSON.stringify(g));assert(Math.abs(g.height-g.view)<2,'short chat has no empty scroll extent');
  if(viewport.width<760)await page.locator('button.back[data-on-click="select"]').first().click();
  await page.locator('[data-on-click="toggle-settled"]').first().click();await page.locator('[data-on-click="select"][data-value="long"]').first().click();await page.waitForFunction(()=>document.querySelector('main[data-thread="long"]')&&document.documentElement.dataset.backplaneRemoteReady==='true');
  await page.waitForTimeout(150);g=await page.evaluate(geometry);assert(g.gap>=-1&&g.gap<35,JSON.stringify(g));assert(g.top>0,'long chat opens at its end');
  const text='Instant pending '+viewport.width+' 日本語';delayActions=true;await page.locator('#composer').fill(text);const start=Date.now();await page.locator('#composer').press('Enter');
  await page.waitForFunction(text=>[...document.querySelectorAll('#timeline .sending')].some(e=>e.textContent.includes(text)),text,{timeout:2000});
  const pendingMs=Date.now()-start;assert.equal(await page.locator('#composer').inputValue(),'');await page.waitForTimeout(100);assert(heldReplies.length>0,'a hub action ACK is still held');
  await page.locator('#composer').fill('Next unsent draft');assert.equal(await page.evaluate(()=>localStorage.getItem('backplane-draft:long')),text+'\n\nNext unsent draft','reload can recover the captured send until its ACK');await page.screenshot({path:out+'/pending-'+viewport.width+'.png'});
  results.push({viewport,pendingMs,beforeACK:await page.evaluate(geometry)});delayActions=false;for(const reply of heldReplies.splice(0))reply();
  await page.waitForFunction(({text,selector})=>[...document.querySelectorAll(selector)].some(e=>e.textContent.includes(text)),{text,selector:committedSelector},{timeout:15000});
  assert.equal(await page.locator('#composer').inputValue(),'Next unsent draft');assert.equal(await page.evaluate(()=>localStorage.getItem('backplane-draft:long')),'Next unsent draft');assert.equal(await page.locator('#timeline .msg').filter({hasText:text}).count(),1,'one canonical message replaces the provisional bubble');
  await page.screenshot({path:out+'/settled-'+viewport.width+'.png'});
  if(process.env.ENTRY_AUTOFILL==='1'){
   if(viewport.width<760)await page.locator('button.back[data-on-click="select"]').first().click();
   await page.locator('[data-on-click="select"][data-value="folded"]').first().click();
   await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('#timeline .msg.user[data-id="folded-start"]'),null,{timeout:15000});
   const filled=await page.evaluate(geometry);assert(filled.gap>=-1&&filled.gap<35,'bottom alignment survives automatic history fill: '+JSON.stringify(filled));
   results.push({viewport,autofill:filled});await page.screenshot({path:out+'/autofill-'+viewport.width+'.png'});
  }
  await context.close();
 }
 assert.deepEqual(errors,[]);console.log(JSON.stringify({ok:true,results,out}));
}catch(e){console.error(e.stack);if(page&&!page.isClosed()){console.error(await page.locator('body').innerText());await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}process.exitCode=1;}
finally{await browser?.close();service.stop();}})();
