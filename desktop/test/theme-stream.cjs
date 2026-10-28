if(process.env.BP_AGENT_UI_ISOLATED!=='1')throw Error('Run agent UI fixtures through bp-coord/ui-isolation/run.py; never on the user display');
// Sole-integrator isolated acceptance: a reply streaming in tb while the theme
// is toggled and the person goes tb→ta→tb, in both directions, at 1400 and
// 390 px. Never connect to the live user home.
// BACKPLANE_LOCAL_STAGE=<stage> node desktop/test/theme-stream.cjs
// The agent is a deterministic Claude stand-in (theme-agent.cjs: streams like
// stream-agent.cjs, 120 text deltas 50 ms apart, each naming its index and the
// stream's start; THEME-ASK opens a real ask through permission_prompt).
// Every animation frame and class/child mutation after the toggle's own frame is
// sampled: the page's theme (app root, <html>, body background), and on tb the
// reply's newest index and stream, and the Stop control. Separate from
// theme-flash.cjs, whose checks it does not repeat.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs'),{watchViews,chooseTheme}=require('./theme-wire.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
const out=process.env.THEME_STREAM_OUT||path.join(stage,'theme-stream-'+Date.now());fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-theme-stream-')),agents=path.join(home,'agents'),root=path.join(home,'board');fs.mkdirSync(agents);fs.mkdirSync(root);
for(const name of ['codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
fs.writeFileSync(path.join(agents,'claude'),'#!/bin/sh\nexec '+JSON.stringify(process.execPath)+' '+JSON.stringify(path.join(__dirname,'theme-agent.cjs'))+' "$@"\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[],ev=($,fields)=>events.push({$,...fields,at});
ev('ProjectCreated',{id:'board',title:'Board',root});
ev('ThreadCreated',{id:'ta',title:'Thread ta',project:'board',env:'local',branch:'',worktree:root,provider:'claude',model:'sonnet'});
ev('ThreadCreated',{id:'tb',title:'Thread tb',project:'board',env:'local',branch:'',worktree:root,provider:'claude',model:'sonnet'});
for(let i=0;i<20;i++)ev('MessagePosted',{thread:'ta',msg:'ta-m'+i,role:i%2?'assistant':'user',text:'ta message '+i+' 日本語 '+'x'.repeat(120)});
// No ask is seeded: the hub's start closes asks left open in the log (AskClosed
// with an empty answer), which is recovery, not a loss. ta's ask is opened
// after start by the stand-in's own permission_prompt call (theme-agent.cjs).
for(const [key,value] of [['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1'],['ui.theme','dark']])ev('SettingSet',{key,value});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BP_AGENT_TRACE:path.join(home,'agent-trace.jsonl'),SHELL:'/bin/sh'};Object.assign(process.env,env);
const binary=stage+'/payload/backplane-serve',web=stage+'/payload/web',service=new Service({home,binary,web,expectedVersion:info.version});
let app,page,views,askId='',step='startup';const checks=[],rounds=[],trail=[],BODY={dark:'rgb(13, 15, 18)',light:'rgb(250, 250, 248)'};
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread]')?.dataset.thread===id&&document.documentElement.dataset.backplaneCached!=='true',id);}
const settle=()=>page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
async function go(id,narrow){
 if(narrow&&await page.locator('.thread-head .back').first().isVisible())await page.locator('.thread-head .back').first().click();
 await page.locator(`.sidebar [data-on-click="select"][data-value="${id}"]`).first().click();await ready(id);await settle();
}
// one theme change, ONE Close, each on its own reply and connected view (theme-wire.cjs)
async function choose(theme,narrow){await chooseTheme(page,views,trail,theme,{narrow});}
// what one frame shows; the reply is read by its stand-in's own markers, so a
// stamp or footer in the row cannot confuse it
const arm=expect=>page.evaluate(expect=>{
 const r={expect,frames:[],mutations:[],stop:false};window.__themeStream=r;
 const now=kind=>{const a=document.querySelector('.app'),m=document.querySelector('main[data-thread]'),thread=m?.dataset.thread||'';
  const msgs=m?[...m.querySelectorAll('.msg.assistant')]:[],live=m?.querySelector('[data-id="@live:tb"]')||null,row=live||msgs.at(-1)||null,text=row?.textContent||'';
  const idx=[...text.matchAll(/sample-(\d+)\((\d+)\)/g)],stopEl=m?[...m.querySelectorAll('[data-on-click="interrupt"]')]:[];
  return{kind,t:performance.now(),thread,app:a?a.classList.contains('light')?'light':'dark':'',html:document.documentElement.classList.contains('light')?'light':'dark',
   body:getComputedStyle(document.body).backgroundColor,cached:document.documentElement.dataset.backplaneCached||'',ready:document.documentElement.dataset.backplaneRemoteReady||'',
   live:!!live,stream:idx.length?idx[0][2]:'',newest:idx.length?Math.max(...idx.map(x=>+x[1])):-1,
   stopShown:stopEl.length>0,stopEnabled:stopEl.some(b=>!b.disabled&&b.getAttribute('aria-disabled')!=='true')};};
 r.now=now;r.mo=new MutationObserver(()=>r.mutations.push(now('mutation')));
 r.mo.observe(document.getElementById('root'),{subtree:true,attributes:true,attributeFilter:['class'],childList:true,characterData:true});
 const tick=()=>{if(r.stop)return;r.frames.push(now('frame'));requestAnimationFrame(tick);};requestAnimationFrame(tick);
},expect);
const collect=()=>page.evaluate(()=>{const r=window.__themeStream;r.stop=true;r.mo.disconnect();delete window.__themeStream;return{frames:r.frames,mutations:r.mutations};});
const live=()=>page.evaluate(()=>{const m=document.querySelector('main[data-thread]'),row=m?.querySelector('[data-id="@live:tb"]'),idx=[...(row?.textContent||'').matchAll(/sample-(\d+)\((\d+)\)/g)];
 return{thread:m?.dataset.thread,live:!!row,stream:idx.length?idx[0][2]:'',newest:idx.length?Math.max(...idx.map(x=>+x[1])):-1};});
async function round(n,theme,width,draft){
 if(process.env.THEME_STREAM_PREFIX_RECEIPT){const p=JSON.parse(fs.readFileSync(process.env.THEME_STREAM_PREFIX_RECEIPT));assert.equal(p.stage,stage);const kept=p.rounds.find(r=>r.round===n&&r.theme===theme&&r.width===width);if(kept){assert(kept.liveAtReturn&&kept.ask&&kept.draftKept&&['palette','shrank','otherStream','stale','noStop'].every(k=>kept[k].length===0));rounds.push(kept);return;}}
 const narrow=width<=760,name=`${theme}-${width}`;step='stream-'+name;
 // a new reply in tb, seen growing before anything else happens
 await go('tb',narrow);await page.waitForFunction(()=>!document.querySelector('main[data-thread="tb"] [data-id="@live:tb"]'));
 await page.locator('#composer').fill('stream '+n);await page.locator('#composer').press('Enter');
 await page.waitForFunction(()=>{const t=document.querySelector('main[data-thread="tb"] [data-id="@live:tb"]')?.textContent||'';return /sample-[3-9]\(/.test(t)||/sample-\d\d+\(/.test(t);},null,{timeout:20000});
 const before=await live();assert(before.live&&before.stream,'tb is streaming before the toggle');
 // the toggle mid-stream, then tb→ta→tb; sampled from the toggle's own frame on
 step='toggle-'+name;await choose(theme,narrow);if(narrow)await go('tb',true);else await ready('tb');
 await arm(theme);
 step='switch-'+name;await go('ta',narrow);
 const ask=await page.evaluate(id=>[...document.querySelectorAll('main[data-thread="ta"] [data-on-click="answer"]')].filter(b=>b.getAttribute('data-value')?.startsWith(id+'|')&&!b.disabled).length,askId);
 const taDraft=await page.locator('#composer').inputValue();
 await go('tb',narrow);const back=await live();
 const r=await collect();const all=[...r.frames,...r.mutations].sort((a,b)=>a.t-b.t);
 // 1. one palette in every sampled state
 const palette=all.filter(f=>f.app&&(f.app!==theme||f.html!==theme||f.body!==BODY[theme]));
 // 2. the reply in tb only grows, is the same stream, and never comes back older than it was when tb was left.
 //    Hub views only: a retained picture of tb may hold an older reply than the
 //    one last streamed (pictures are kept per view, not per delta); that is
 //    recorded (cachedOlder), not judged here, since the theme does not touch it
 const tb=all.filter(f=>f.thread==='tb'&&f.newest>=0&&f.cached!=='true'),cachedOlder=all.filter(f=>f.thread==='tb'&&f.cached==='true'&&f.newest>=0&&f.newest<before.newest).length;let high=before.newest,shrank=[],other=[];
 for(const f of tb){if(f.stream&&f.stream!==before.stream)other.push(f);else if(f.newest<high)shrank.push({...f,high});else high=f.newest;}
 const leftAt=Math.max(-1,...all.filter(f=>f.thread==='tb'&&f.cached!=='true'&&f.t<(all.find(g=>g.thread==='ta')?.t??Infinity)).map(f=>f.newest));
 const returned=all.filter(f=>f.thread==='tb'&&f.ready==='true'&&f.cached!=='true'&&f.t>(all.find(g=>g.thread==='ta')?.t??Infinity));
 const stale=returned.filter(f=>f.newest>=0&&f.newest<leftAt);
 // 3. Stop is there, and enabled, in every hub view of tb while it streams
 const noStop=all.filter(f=>f.thread==='tb'&&f.live&&f.ready==='true'&&f.cached!=='true'&&!(f.stopShown&&f.stopEnabled));
 const row={round:n,theme,width,frames:r.frames.length,mutations:r.mutations.length,cachedFrames:r.frames.filter(f=>f.cached==='true').length,
  stream:before.stream,before:before.newest,cachedOlder,leftAt,back:back.newest,liveAtReturn:back.live,ask,draftKept:taDraft===draft,
  palette:palette.slice(0,6),shrank:shrank.slice(0,6),otherStream:other.slice(0,3),stale:stale.slice(0,3),noStop:noStop.slice(0,3)};
 rounds.push(row);
 assert.equal(palette.length,0,`${name}: another palette shown: `+JSON.stringify(palette.slice(0,2)));
 assert.equal(other.length,0,`${name}: tb showed another stream`);assert.equal(shrank.length,0,`${name}: the reply shrank: `+JSON.stringify(shrank.slice(0,2)));
 assert.equal(stale.length,0,`${name}: after the hub's view was ready tb showed an older reply than it was left with: `+JSON.stringify(stale.slice(0,2)));
 assert(back.stream===''||back.stream===before.stream,`${name}: tb came back with another stream`);
 assert(back.newest>=leftAt,`${name}: tb came back at sample ${back.newest}, left at ${leftAt}`);
 assert.equal(noStop.length,0,`${name}: Stop missing or disabled while tb streamed: `+JSON.stringify(noStop.slice(0,2)));
 assert(ask>0,`${name}: ta's ask controls missing`);assert.equal(taDraft,draft,`${name}: ta's draft changed`);
 // the round is only meaningful if the reply was still streaming when it started
 assert(r.frames.some(f=>f.thread==='tb'&&f.live),`${name}: the stream ended before the switches: the case was not exercised`);
}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});
 page=await app.firstWindow();page.setDefaultTimeout(15000);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 views=await watchViews(page);
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();
 step='load';await go('tb',false);await go('ta',false);
 // a genuine ask in ta, opened now by the stand-in through the hub's permission_prompt
 step='ask';await page.locator('#composer').fill('THEME-ASK read the board');await page.locator('#composer').press('Enter');
 await page.waitForFunction(()=>[...document.querySelectorAll('main[data-thread="ta"] [data-on-click="answer"]')].some(b=>!b.disabled&&/\|/.test(b.getAttribute('data-value')||'')),null,{timeout:20000});
 askId=await page.evaluate(()=>[...document.querySelectorAll('main[data-thread="ta"] [data-on-click="answer"]')].map(b=>b.getAttribute('data-value')).find(v=>v?.includes('|')).split('|')[0]);
 const opened=fs.readFileSync(home+'/events.jsonl','utf8').trim().split('\n').map(JSON.parse).filter(e=>e.$==='AskOpened'&&e.thread==='ta');
 assert(opened.some(e=>e.id===askId),'ta\'s ask is the one the stand-in opened after start');checks.push('ta has a real ask opened after start ('+askId+')');
 // ta's draft: an IME composition, committed, then typed
 step='draft';const cdp=await page.context().newCDPSession(page);await page.locator('#composer').click();
 await cdp.send('Input.imeSetComposition',{text:'にほん',selectionStart:3,selectionEnd:3});await cdp.send('Input.insertText',{text:'日本'});await page.keyboard.type(' draft');
 const draft=await page.locator('#composer').inputValue();assert(draft.endsWith('日本 draft'),draft);
 let n=0;
 for(const theme of ['light','dark'])await round(++n,theme,1400,draft);
 checks.push('1400 px: mid-stream toggle dark→light and light→dark, tb→ta→tb: one palette, the reply only grew, Stop and the ask stayed, the draft stayed');
 step='narrow';await page.evaluate(()=>{});await app.evaluate(({BrowserWindow})=>{const w=BrowserWindow.getAllWindows()[0];w.setMinimumSize(320,480);w.setSize(390,844);});await page.waitForFunction(()=>innerWidth<=391);
 for(const theme of ['light','dark'])await round(++n,theme,390,draft);
 checks.push('390 px: the same through the shut sidebar');
 assert(rounds.some(x=>x.cachedFrames>0),'no switch showed a retained picture');checks.push('retained pictures were on screen in '+rounds.filter(x=>x.cachedFrames).length+' of '+rounds.length+' rounds');
 step='no-send';assert(!fs.readFileSync(home+'/events.jsonl','utf8').trim().split('\n').map(JSON.parse).some(e=>e.$==='MessagePosted'&&e.thread==='ta'&&e.text?.includes('draft')));checks.push('the ta draft stayed a draft');
 await page.screenshot({path:out+'/after.png'});
 const r={state:'passed',source:info.commit,stage,checks,rounds,trail,wire:views.log,wireOver:views.over(),liveProfileTouched:false,
  limits:['Frames are sampled at requestAnimationFrame and at every class, child and text mutation under #root; a compositor-only frame between them is not observable from the page.',
   'The stand-in streams for about 6 s per reply; a round whose stream ended before the switches fails as not exercised rather than passing.']};
 fs.writeFileSync(out+'/RESULT.json',JSON.stringify(r,null,2)+'\n');console.log(JSON.stringify({state:r.state,source:r.source,checks,rounds:rounds.map(x=>[x.theme,x.width,x.before,x.leftAt,x.back,x.liveAtReturn,x.cachedFrames,x.ask,x.draftKept])}));
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',source:info.commit,stage,home,step,checks,rounds,trail,finding:trail.find(t=>t.finding)?.finding||'',wire:views?.log.slice(-120)||[],error:e.stack},null,2)+'\n');if(page)await page.screenshot({path:out+'/failure.png'}).catch(()=>{});console.error(e.stack);process.exitCode=1;}
finally{if(app)await app.close();await service.stop();}})();
