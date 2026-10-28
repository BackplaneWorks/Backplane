// Exact original Bub conversation reproduction; real Electron + isolated hub.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core');
const {Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE);
const out=path.join(stage,'bub-links-'+Date.now());fs.mkdirSync(out);
const ids=['t1792700806-414704','t1792608320-335544','t1792717355-430505'];
const names=['Backplane','Nordic','TestFlight'],bub='t1790292962-8017';
const text='- **[Backplane](backplane://thread/t1792700806-414704):** polish is live. Cache improvements brought warm staging plus desktop/runtime checks to **47.6 seconds locally** or **91.7 seconds through Boat**. These reuse compiler output; cold builds remain **212 seconds**. Default tests pass in **67 seconds**.\n- **[Nordic](backplane://thread/t1792608320-335544):** Luna patches are being reviewed and integrated. Current work fixes DHCP lifecycle races and separates usable BLE pairing commands from unresolved LTK behavior. No new compiled milestone yet.\n- **[TestFlight](backplane://thread/t1792717355-430505):** 0.12.1 is installable internally. Last confirmed external status is awaiting Apple review.';
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-bub-links-'));
const project=path.join(home,'project'),agents=path.join(home,'agents');fs.mkdirSync(project);fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),event=($,fields)=>({$,...fields,at});
const thread=(id,title)=>event('ThreadCreated',{id,title,project:'fixture',env:'local',branch:'',worktree:project,provider:'codex',model:'gpt-6.1-sol'});
const events=[event('ProjectCreated',{id:'fixture',title:'Exact Bub link fixture',root:project}),thread(bub,'Bub Chat'),...ids.map((id,i)=>thread(id,names[i])),
 event('BotSet',{id:'bub',name:'Bub',thread:bub,look:0,persona:''}),event('UberSet',{bot:'bub',uid:'bub-link-fixture'}),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>event('SettingSet',{key,value:'off'})),event('SettingSet',{key:'onboard.done',value:'1'}),
 event('MessagePosted',{thread:bub,msg:'item_4-435337',role:'assistant',text}),event('MessagePosted',{thread:ids[0],msg:'normal-list',role:'assistant',text}),
 event('MessagePosted',{thread:bub,msg:'unsafe',role:'assistant',text:'**[unsafe](javascript:alert(1))** and **[unknown](unknown:payload)** and `**[literal](backplane://thread/t1)**`'})];
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
const info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json')));
const service=new Service({home,binary:path.join(stage,'payload/backplane-serve'),web:path.join(stage,'payload/web'),expectedVersion:info.version});
let app,page;const start=performance.now(),navigation=[];
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:path.join(stage,'payload/backplane-serve'),BACKPLANE_WEB:path.join(stage,'payload/web'),BACKPLANE_SERVICE_VERSION:info.version},args:[]});
 page=await app.firstWindow();
 const ready=async id=>page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',id);
 const chat=async()=>{await page.locator('.bot-row[data-value="bub"]').first().click();const tab=page.locator('[data-on-click="bot-tab"][data-value="chat"]');if(await tab.count())await tab.first().click();await ready(bub);};
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');await chat();
 const body=page.locator('[data-id="item_4-435337"] .body');
 const anchors=body.locator('li strong a.ref');
 if(process.env.BUB_LINKS_EXPECT==='literal'){
  assert.equal(await anchors.count(),0);for(const id of ids)assert((await body.innerText()).includes('backplane://thread/'+id));
  await page.screenshot({path:path.join(out,'before.png')});console.log(JSON.stringify({state:'reproduced',out,version:info.version}));return;
 }
 assert.equal(await anchors.count(),3);assert((await body.locator('strong').allTextContents()).includes('47.6 seconds locally'));
 assert(!(await body.innerText()).includes('backplane://thread/'));
 assert.equal(await page.locator('a[href^="javascript:"],a[href^="unknown:"]').count(),0);
 await page.screenshot({path:path.join(out,'bub-chat.png')});
 for(let i=0;i<ids.length;i++){
  const a=page.locator(`[data-id="item_4-435337"] a[data-ref="backplane://thread/${ids[i]}"]`);
  assert.equal(await a.getAttribute('href'),'backplane://thread/'+ids[i]);assert.equal(await a.innerText(),names[i]);
  await a.click();await ready(ids[i]);navigation.push(ids[i]);
  if(i===0)assert.equal(await page.locator('[data-id="normal-list"] li strong a.ref').count(),3);
  await chat();
 }
 const windows=await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows().map(w=>({visible:w.isVisible(),url:w.webContents.getURL()})));
 assert.equal(windows.filter(w=>w.visible).length,1,JSON.stringify(windows));
 const result={state:'passed',seconds:(performance.now()-start)/1000,out,version:info.version,commit:info.commit,originalMessage:'item_4-435337',renderer:'View.bot Chat → View.thread → Md.render',navigation,windows};
 fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}catch(e){console.error(e);process.exitCode=1;if(page){fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>''));await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});}}
finally{if(app)await app.close();service.stop();}})();
