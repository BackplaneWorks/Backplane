if(process.env.BP_AGENT_UI_ISOLATED!=='1')throw Error('Run agent UI fixtures through bp-coord/ui-isolation/run.py; never on the user display');
// Sole-integrator isolated acceptance: no thread ever paints the other theme.
// Never connect to the live user home. BACKPLANE_LOCAL_STAGE=<stage> node desktop/test/theme-flash.cjs
// Every animation frame from the click that switches threads until the hub's
// own view of the target is ready (and a few after) is sampled for the theme
// the app root, the page and the main pane actually have; a MutationObserver
// also records every app-root class the DOM ever held between frames.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs'),{watchViews,chooseTheme}=require('./theme-wire.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
const out=process.env.THEME_FLASH_OUT||path.join(stage,'theme-flash-'+Date.now());fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-theme-flash-')),agents=path.join(home,'agents'),root=path.join(home,'board');fs.mkdirSync(agents);fs.mkdirSync(root);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[],ev=($,fields)=>events.push({$,...fields,at});
const threads=['ta','tb','tc'];
ev('ProjectCreated',{id:'board',title:'Board',root});
for(const id of threads){ev('ThreadCreated',{id,title:'Thread '+id,project:'board',env:'local',branch:'',worktree:root,provider:'codex',model:''});
 for(let i=0;i<30;i++)ev('MessagePosted',{thread:id,msg:id+'-m'+i,role:i%2?'assistant':'user',text:id+' message '+i+' 日本語 '+'x'.repeat(120)});}
for(const [key,value] of [['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1'],['ui.theme','dark']])ev('SettingSet',{key,value});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',SHELL:'/bin/sh'};Object.assign(process.env,env);
const binary=stage+'/payload/backplane-serve',web=stage+'/payload/web',service=new Service({home,binary,web,expectedVersion:info.version});
let app,page,views,step='startup';const checks=[],rounds=[],trail=[];
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread]')?.dataset.thread===id&&document.documentElement.dataset.backplaneCached!=='true',id);}
// the recorder: started before the click, so the click's own frame is in it
const arm=expect=>page.evaluate(expect=>{
 const r={expect,frames:[],mutations:[],stop:false};window.__theme=r;
 const now=()=>{const a=document.querySelector('.app'),m=document.querySelector('main');
  return{t:performance.now(),app:a?a.classList.contains('light')?'light':'dark':'',html:document.documentElement.classList.contains('light')?'light':'dark',
   bg:m?getComputedStyle(m).backgroundColor:'',body:getComputedStyle(document.body).backgroundColor,thread:m?.dataset.thread||'',cached:document.documentElement.dataset.backplaneCached||'',ready:document.documentElement.dataset.backplaneRemoteReady||''};};
 r.mo=new MutationObserver(()=>r.mutations.push(now()));r.mo.observe(document.getElementById('root'),{subtree:true,attributes:true,attributeFilter:['class'],childList:true});
 const tick=()=>{if(r.stop)return;r.frames.push(now());requestAnimationFrame(tick);};requestAnimationFrame(tick);
},expect);
const collect=()=>page.evaluate(()=>{const r=window.__theme;r.stop=true;r.mo.disconnect();delete window.__theme;return{expect:r.expect,frames:r.frames,mutations:r.mutations};});
// the canonical colours of each theme's main pane and page (style.css)
let colours={};
async function learn(theme){colours[theme]=await page.evaluate(()=>({bg:getComputedStyle(document.querySelector('main')).backgroundColor,body:getComputedStyle(document.body).backgroundColor}));}
// one theme change, ONE Close, each on its own reply and connected view (theme-wire.cjs)
async function choose(theme){await chooseTheme(page,views,trail,theme);}
async function go(id,narrow){
 // at 390 px the sidebar is shut while a thread shows: the header's back button opens it
 if(narrow&&await page.locator('.thread-head .back').first().isVisible())await page.locator('.thread-head .back').first().click();
 await page.locator(`.sidebar [data-on-click="select"][data-value="${id}"]`).first().click();
 await ready(id);
 await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
}
async function round(theme,width,order){
 const narrow=width<=760;
 if(process.env.THEME_COMPLETED_RECEIPT){const kept=JSON.parse(fs.readFileSync(process.env.THEME_COMPLETED_RECEIPT));assert.equal(kept.stage,stage);const prior=kept.rounds.filter(r=>r.theme===theme&&r.width===width);if(prior.length){assert.deepEqual(prior.map(r=>r.thread),order);assert(prior.every(r=>!r.bad.length&&r.cachedFrames>0));rounds.push(...prior);return;}}
 for(const id of order){
  await arm(theme);await go(id,narrow);const r=await collect();
  const bad=[...r.frames.map(f=>({...f,kind:'frame'})),...r.mutations.map(f=>({...f,kind:'mutation'}))]
   .filter(f=>f.app&&(f.app!==theme||f.html!==theme||f.body!==colours[theme].body||f.bg&&f.thread&&f.bg!==colours[theme].bg));
  rounds.push({theme,width,thread:id,frames:r.frames.length,mutations:r.mutations.length,cachedFrames:r.frames.filter(f=>f.cached==='true').length,bad:bad.slice(0,8)});
  assert.equal(bad.length,0,`${id} at ${width}px painted another theme than ${theme}: `+JSON.stringify(bad.slice(0,3)));
 }
}
async function size(w,h){await app.evaluate(({BrowserWindow},[w,h])=>{const win=BrowserWindow.getAllWindows()[0];win.setMinimumSize(320,480);win.setSize(w,h);},[w,h]);await page.waitForFunction(w=>innerWidth<=w+1,w);}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});
 const nativeLog=fs.createWriteStream(out+'/native-process.log');app.process().stderr?.pipe(nativeLog);app.process().once('exit',(code,signal)=>fs.writeFileSync(out+'/native-exit.json',JSON.stringify({at:Date.now(),code,signal,step})+'\n'));
 await app.evaluate(({app})=>{process.on('uncaughtExceptionMonitor',e=>console.error('THEME_UNCAUGHT',e?.stack||String(e)));app.on('render-process-gone',(_e,_wc,d)=>console.error('THEME_RENDER_GONE',JSON.stringify(d)));});
 page=await app.firstWindow();page.setDefaultTimeout(15000);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 views=await watchViews(page);
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();
 step='load-dark';for(const id of threads)await go(id,false);await learn('dark');
 step='draft';await go('ta',false);
 // an IME composition left open, then committed, then a typed tail: the draft must survive every switch
 const cdp=await page.context().newCDPSession(page);await page.locator('#composer').click();
 await cdp.send('Input.imeSetComposition',{text:'にほん',selectionStart:3,selectionEnd:3});await cdp.send('Input.insertText',{text:'日本'});await page.keyboard.type(' draft');
 const draft=await page.locator('#composer').inputValue();assert(draft.endsWith('日本 draft'),draft);
 step='light';await choose('light');await go('ta',false);await learn('light');assert.notEqual(colours.light.body,colours.dark.body);
 for(const [theme,next] of [['light','dark'],['dark','light']]){
  step='switch-'+theme+'-1400';
  if(theme==='light'&&process.env.THEME_WIDE_LIGHT_RECEIPT){const kept=JSON.parse(fs.readFileSync(process.env.THEME_WIDE_LIGHT_RECEIPT));assert.equal(kept.stage,stage);assert.equal(kept.rounds.length,5);assert(kept.rounds.every(r=>r.theme==='light'&&r.width===1400&&!r.bad.length&&r.cachedFrames>0));rounds.push(...kept.rounds);}
  else await round(theme,1400,['tb','tc','ta','tc','tb']);
  await go('ta',false);assert.equal(await page.locator('#composer').inputValue(),draft,'draft kept after '+theme+' switches');
  step='toggle-'+next;await choose(next);
 }
 checks.push('wide: every frame of 10 loaded-thread switches after each toggle showed only the chosen theme, light→dark and dark→light');
 step='narrow';await size(390,844);await go('ta',true);
 for(const [theme,next] of [['light','dark'],['dark','light']]){
  step='switch-'+theme+'-390';await round(theme,390,['tb','tc','ta']);
  await go('ta',true);assert.equal(await page.locator('#composer').inputValue(),draft,'draft kept at 390 px');
  step='toggle-'+next+'-390';await page.locator('.thread-head .back').first().click();await choose(next);await go('ta',true);
 }
 checks.push('390 px: the same for switches through the shut sidebar');
 // the retained pictures this guards were really on screen during the switches
 assert(rounds.reduce((n,x)=>n+x.cachedFrames,0)>0,'no switch showed a retained picture: the case was not exercised');checks.push('retained pictures were shown in '+rounds.filter(x=>x.cachedFrames).length+' of '+rounds.length+' switches');
 step='no-send';assert(!fs.readFileSync(home+'/events.jsonl','utf8').trim().split('\n').map(JSON.parse).some(e=>e.$==='MessagePosted'&&e.text?.includes('draft')));checks.push('the draft stayed a draft');
 await page.screenshot({path:out+'/after.png'});
 const r={state:'passed',source:info.commit,stage,checks,colours,rounds,trail,wire:views.log,wireOver:views.over(),liveProfileTouched:false,
  limits:['Frames are sampled at requestAnimationFrame and every class mutation; a compositor-only frame between them is not observable from the page.']};
 fs.writeFileSync(out+'/RESULT.json',JSON.stringify(r,null,2)+'\n');console.log(JSON.stringify({state:r.state,source:r.source,checks,rounds:rounds.map(x=>[x.theme,x.width,x.thread,x.frames,x.cachedFrames,x.bad.length])}));
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',source:info.commit,stage,home,step,checks,colours,rounds,trail,finding:trail.find(t=>t.finding)?.finding||'',wire:views?.log.slice(-120)||[],error:e.stack},null,2)+'\n');if(page)await page.screenshot({path:out+'/failure.png'}).catch(()=>{});console.error(e.stack);process.exitCode=1;}
finally{if(app)await app.close();await service.stop();}})();
