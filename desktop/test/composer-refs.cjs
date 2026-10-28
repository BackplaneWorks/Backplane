// Sole-integrator isolated acceptance. Never connect to the live user home.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
const out=process.env.COMPOSER_REFS_OUT||path.join(stage,'composer-refs-'+Date.now());fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-composer-refs-')),agents=path.join(home,'agents'),root=path.join(home,'board');fs.mkdirSync(agents);fs.mkdirSync(root);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[],ev=($,fields)=>events.push({$,...fields,at});
ev('ProjectCreated',{id:'board',title:'Board',root});
for(const [id,title] of [['self','Here'],['t100-7','Fix perf'],['bot-thread','Bot chat']])ev('ThreadCreated',{id,title,project:'board',env:'local',branch:'',worktree:root,provider:'codex',model:''});
ev('BotSet',{id:'miso',name:'Miso',thread:'bot-thread',look:0,persona:'Parts helper'});ev('RoomSet',{id:'room',name:'Hardware',members:'miso'});
for(const [key,value] of [['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1']])ev('SettingSet',{key,value});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const binary=stage+'/payload/backplane-serve',web=stage+'/payload/web',service=new Service({home,binary,web,expectedVersion:info.version});
let app,page,step='startup';const checks=[];
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread]')?.dataset.thread===id,id);}
async function offer(value,sigil){await page.locator('#composer').fill(value);await page.waitForFunction(({value,sigil})=>document.querySelector('#composer')?.value===value&&[...document.querySelectorAll('.composer .skill')].some(el=>el.dataset.value?.startsWith(sigil)),{value,sigil});return page.locator('.composer .skill').first().getAttribute('data-value');}
async function valueIs(value){await page.waitForFunction(value=>document.querySelector('#composer')?.value===value,value);}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});page=await app.firstWindow();page.setDefaultTimeout(12000);
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();
 await page.locator('.sidebar [data-on-click="select"][data-value="self"]').first().click();await ready('self');const composer=page.locator('#composer');
 for(const [sigil,query,method] of [['@','@mi','Tab'],['>','>fi','Enter'],['%','%bo','click'],['#','#ha','Tab']]){
  step='offer-'+sigil;const prefix='日本語 prefix ',word=await offer(prefix+query,sigil);assert(word.startsWith(sigil));
  if(method==='click')await page.locator('.composer .skill').first().click();else await composer.press(method);
  await valueIs(prefix+word+' ');assert.equal(await composer.evaluate(el=>document.activeElement===el),true);checks.push(sigil+' existing target and '+method+' completion preserve prefix and focus');
 }
 step='escape-reopen';await offer('ask @mi','@');await composer.press('Escape');await page.waitForFunction(()=>document.querySelector('#composer')?.getAttribute('data-esc')===''&&!document.querySelector('.composer .skill'));
 assert.equal(await composer.inputValue(),'ask @mi');await offer('ask @mis','@');checks.push('Escape dismisses without altering draft; actual edit reopens references');
 step='caret-ime';await composer.fill('日本語 #ha');await composer.evaluate(el=>el.setSelectionRange(2,2));await page.waitForFunction(()=>document.querySelector('.composer .skill')?.dataset.value?.startsWith('#'));
 assert.deepEqual(await composer.evaluate(el=>[el.selectionStart,el.selectionEnd,document.activeElement===el]),[2,2,true]);
 const ime=await composer.evaluate(el=>['Tab','Enter','Escape'].map(key=>{const e=new KeyboardEvent('keydown',{key,isComposing:true,bubbles:true,cancelable:true});el.dispatchEvent(e);return e.defaultPrevented;}));assert.deepEqual(ime,[false,false,false]);assert.equal(await composer.inputValue(),'日本語 #ha');checks.push('refresh preserves focused caret; IME keys do not complete, dismiss or send');
 step='thread-isolation';await page.locator('.sidebar [data-on-click="select"][data-value="t100-7"]').first().click();await ready('t100-7');await valueIs('');assert.equal(await page.locator('.composer .skill').count(),0);
 await page.locator('.sidebar [data-on-click="select"][data-value="self"]').first().click();await ready('self');await valueIs('日本語 #ha');await page.waitForFunction(()=>!!document.querySelector('.composer .skill'));checks.push('other thread stays empty; returning restores this draft and its suggestions');
 step='no-send';const persisted=fs.readFileSync(home+'/events.jsonl','utf8').trim().split('\n').map(JSON.parse);assert(!persisted.some(e=>e.$==='MessagePosted'&&e.thread==='self'));checks.push('autocomplete never posts a message or starts the selected agent');
 await page.screenshot({path:out+'/composer-refs.png'});const result={state:'passed',source:info.commit,stage,checks,liveProfileTouched:false};fs.writeFileSync(out+'/RESULT.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',source:info.commit,stage,home,step,checks,error:e.stack},null,2)+'\n');if(page)await page.screenshot({path:out+'/failure.png'}).catch(()=>{});console.error(e.stack);process.exitCode=1;}finally{if(app)await app.close();await service.stop();}})();
