// Only the newly added provider Settings/picker rows, using synthetic CLIs.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
assert.equal(process.env.BP_AGENT_UI_ISOLATED,'1','Use the private-display controller');
const stage=process.env.BACKPLANE_LOCAL_STAGE,out=process.env.PI_OMP_UI_OUT,meta=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-piomp-ui-')),project=home+'/project',agents=home+'/agents';fs.mkdirSync(project);fs.mkdirSync(agents);
fs.writeFileSync(agents+'/pi',`#!/bin/sh
case "$1" in
--version) echo '1.4.0-fixture';;
--list-models) printf 'provider model context maxout thinking images\nstandin fake-pi 128k 16k no no\n';;
*) exit 127;;
esac
`,{mode:0o755});
fs.writeFileSync(agents+'/omp',`#!/bin/sh
case "$1" in
--version) echo '18.12.6-fixture';;
models) printf '%s\n' '{"models":[{"selector":"standin/fake-omp","name":"Fake OMP","thinking":["low","high"]}]}';;
*) exit 127;;
esac
`,{mode:0o755});
for(const cli of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(agents+'/'+cli,'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[{$:'ProjectCreated',id:'p',title:'Provider fixture',root:project,at},...['pi','omp'].map(provider=>({$:'ThreadCreated',id:provider,project:'p',title:provider,provider,model:'',env:'local',branch:'',worktree:project,at})),...['restart.continue','mail.on','telemetry.share','onboard.done'].map(key=>({$:'SettingSet',key,value:key==='onboard.done'?'1':'off',at}))];fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const service=new Service({home,binary:stage+'/payload/backplane-serve',web:stage+'/payload/web',expectedVersion:meta.version});let app,page,step='startup';const checks=[];
const rows=()=>fs.readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(JSON.parse);
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread]')?.dataset.thread===id&&document.querySelector('main')?.getAttribute('aria-busy')!=='true',id);}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:stage+'/payload/backplane-serve',BACKPLANE_WEB:stage+'/payload/web',BACKPLANE_SERVICE_VERSION:meta.version}});page=await app.firstWindow();page.setDefaultTimeout(10000);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();await page.locator('.tour-back').waitFor({state:'hidden'});
 for(const provider of ['pi','omp']){
  step=provider+'-settings';await page.locator('.sidebar [data-on-click="select"][data-value="'+provider+'"]').first().click();await ready(provider);
  await page.locator('#composer').fill('Unsent synthetic '+provider+' draft');
  await page.locator('.sidebar [data-on-click="flag"][data-value="settings"]').click();await page.locator('#settings-content').waitFor({state:'visible'});
  if(await page.locator('#settings-content').getAttribute('data-settings-page')!=='agents')await page.locator('[data-on-click="settings-page"][data-value="agents"]').first().click();
  await page.waitForFunction(()=>document.querySelector('#settings-content')?.dataset.settingsPage==='agents');
  await page.locator('[data-on-click="setting"][data-value="agent.provider='+provider+'"]').click();
  const model='standin/fake-'+provider;await page.locator('#settings-content [data-on-click="setting"][data-value="agent.model='+model+'"]').waitFor({state:'visible'});
  await page.locator('#settings-content [data-on-click="setting"][data-value="agent.model='+model+'"]').click();
  await page.waitForFunction(({provider,model})=>document.querySelector('[data-value="agent.provider='+provider+'"]')?.classList.contains('on')&&document.querySelector('[data-value="agent.model='+model+'"]')?.classList.contains('on'),{provider,model});
  assert(rows().some(e=>e.$==='SettingSet'&&e.key==='agent.provider'&&e.value===provider));assert(rows().some(e=>e.$==='SettingSet'&&e.key==='agent.model'&&e.value===model));if(provider!=='pi')checks.push(provider+' actual CLI catalog/default Settings selection persists');
  await page.locator('.settings-view [data-on-click="flag"][data-value="settings"]').click();await ready(provider);
  step=provider+'-picker';await page.locator('.model-chip[data-on-click="flag"][data-value="models"]').click();await page.locator('.model-menu').waitFor({state:'visible'});
  const choice=page.locator('.model-menu [data-on-click="mp-model"][data-value="'+model+'"]');await choice.waitFor({state:'visible'});
  if(provider==='omp')assert((await page.locator('.model-menu').textContent()).includes('Fake OMP'));
  await choice.click();await ready(provider);await page.waitForFunction(model=>document.querySelector('.model-layer .mp-models .row.on')?.dataset.value===model,model);assert(await page.locator('.model-menu').isVisible(),'the picker stays open after a choice');
  if(provider==='pi')assert.equal(await page.locator('.model-layer [data-on-click="mp-effort"]').count(),0);else assert(await page.locator('.model-layer [data-on-click="mp-effort"][data-value="high"]').count());
  await page.locator('.shade[data-value="models"]').click();await page.locator('.model-menu').waitFor({state:'hidden'});
  assert.equal(await page.locator('#composer').inputValue(),'Unsent synthetic '+provider+' draft');assert(rows().some(e=>e.$==='ModesSet'&&e.id===provider&&e.model===model));checks.push(provider+' composer model selection persists with draft; catalog label/thinking shape correct');
 }
 assert.equal(rows().filter(e=>e.$==='TurnChanged'&&e.state==='running').length,0);checks.push('No provider turn starts; temporary defaults do not affect user settings');
 fs.writeFileSync(out+'/RESULT.json',JSON.stringify({state:'passed',source:meta.commit,stage,checks,retained:['ui-20261023-164432 Pi Settings/default persistence PASS; setup only repeated to reach picker suffix'],scope:'Only new Pi/OMP Settings and picker catalog/selection/draft through actual package. Private temporary profile, synthetic CLI catalogs; no live provider or user window.'},null,2)+'\n');
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',source:meta.commit,stage,step,checks,error:e.stack},null,2)+'\n');if(page){await page.screenshot({path:out+'/failure.png'}).catch(()=>{});fs.writeFileSync(out+'/failure.html',await page.content().catch(()=>''));}console.error(e.stack);process.exitCode=1;}finally{if(app)await app.close();await service.stop();}})();
