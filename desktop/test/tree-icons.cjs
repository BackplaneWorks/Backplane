// Actual isolated Electron/production service; synthetic profile and no providers.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
const out=process.env.TREE_ICONS_OUT;assert(out);fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-tree-icons-')),agents=path.join(home,'agents'),cwd=path.join(home,'board');fs.mkdirSync(agents);fs.mkdirSync(cwd);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[];
for(let p=0;p<12;p++)events.push({$:'ProjectCreated',id:'p'+p,title:'Synthetic project '+p,root:cwd,at:at+p});
for(let i=0;i<120;i++)events.push({$:'ThreadCreated',id:i===0?'self':'t'+i,project:'p'+i%12,title:'Synthetic thread '+i,env:'local',branch:'',worktree:cwd,provider:'codex',model:'',at:at+i});
events.push({$:'ThreadParentSet',id:'t12',parent:'self',at:at+121},{$:'ThreadParentSet',id:'t24',parent:'t12',at:at+122},{$:'SettingSet',key:'thread.tree.self',value:'0',at:at+123});
for(const key of ['restart.continue','mail.on','telemetry.share','onboard.done'])events.push({$:'SettingSet',key,value:key==='onboard.done'?'1':'off',at});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const binary=stage+'/payload/backplane-serve',web=stage+'/payload/web',service=new Service({home,binary,web,expectedVersion:info.version});let app,page,step='startup';const timings=[],wire=[],checks=[],requests=new Map();
async function ready(){await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main')?.dataset.thread==='self'&&!document.querySelector('#settings-content')&&document.querySelector('main')?.getAttribute('aria-busy')!=='true');}
async function timed(name,click,wait){const t=await page.evaluate(()=>performance.now());await click();await wait();const end=await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>r(performance.now()))));timings.push({name,clickToRAFMs:end-t});}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});page=await app.firstWindow();page.setDefaultTimeout(10000);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();await page.locator('.sidebar [data-on-click="select"][data-value="self"]').first().click();await ready();
 step='immutable-tree-glyphs';
 const parent=page.locator('.sidebar .thread-expander[data-value="self"]');
 await page.locator('#composer').fill('Synthetic 日本語 draft');
 async function state(open){await page.waitForFunction(open=>{const b=document.querySelector('.thread-expander[data-value="self"]');return b?.getAttribute('aria-expanded')===String(open)&&!!b.querySelector('svg.ico path')},open);}
 await state(false);
 const initial=await parent.locator('svg').evaluate(el=>({width:el.getAttribute('width'),height:el.getAttribute('height'),path:el.querySelector('path').getAttribute('d'),hidden:el.getAttribute('aria-hidden')}));
 assert.deepEqual({width:initial.width,height:initial.height,hidden:initial.hidden},{width:'14',height:'14',hidden:'true'});
 const shared=await page.evaluate(()=>{const xs=[...document.querySelectorAll('.thread-expander[aria-expanded="false"] svg')];return {count:xs.length,matching:xs.length>1&&!!xs[0].__v&&xs.every(x=>x.outerHTML===xs[0].outerHTML)}});assert(shared.count>1&&shared.matching,'fixed closed glyphs must have identical actual SVG markup');checks.push('all repeated closed glyphs render identical original14px SVG markup');
 await parent.click();await state(true);await page.locator('.thread-tree-row [data-on-click="select"][data-value="t12"]').first().waitFor({state:'visible'});
 const opened=await parent.locator('svg path').getAttribute('d');assert.notEqual(opened,initial.path);assert.equal(await page.locator('#composer').inputValue(),'Synthetic 日本語 draft');checks.push('ordinary server action refreshes hierarchy, accessible state and glyph without changing draft');
 await parent.press('Space');await state(false);assert.equal(await parent.locator('svg path').getAttribute('d'),initial.path);await page.locator('.thread-tree-row [data-on-click="select"][data-value="t12"]').waitFor({state:'hidden'});checks.push('keyboard collapse reuses closed picture while children disappear');
 await parent.press('Enter');await state(true);assert.equal(await parent.locator('svg path').getAttribute('d'),opened);checks.push('keyboard reopen displays fresh server children and original open glyph');
 await timed('settings-open',()=>page.locator('.sidebar [data-on-click="flag"][data-value="settings"]').click(),()=>page.locator('#settings-content').waitFor({state:'visible'}));
 await timed('settings-appearance',()=>page.locator('[data-on-click="settings-page"][data-value="appearance"]').first().click(),()=>page.waitForFunction(()=>document.querySelector('#settings-content')?.dataset.settingsPage==='appearance'));
 await timed('settings-close',()=>page.locator('.settings-view [data-on-click="flag"][data-value="settings"]').click(),ready);
 await state(true);assert.equal(await page.locator('#composer').inputValue(),'Synthetic 日本語 draft');checks.push('Settings/category/Close preserve fresh expanded state and exact selected draft');
 await page.screenshot({path:out+'/desktop.png'});
 assert(!fs.readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(JSON.parse).some(e=>e.$==='MessagePosted'));checks.push('no messages or providers; isolated synthetic profile only');
 const receipt={state:'passed',source:info.commit,stage,checks,timings,wire,liveProfileTouched:false,scope:'Only immutable glyph production memo boundary, ordinary mouse/Space/Enter refresh and one Settings sequence on120threads/12projects. Existing model/Terminal/autocomplete cases retained; synthetic timings are not a live speedup benchmark.'};fs.writeFileSync(out+'/RESULT.json',JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt));
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',source:info.commit,stage,step,checks,timings,wire,error:e.stack},null,2)+'\n');console.error(e.stack);process.exitCode=1;}finally{if(app)await app.close();await service.stop();}})();
