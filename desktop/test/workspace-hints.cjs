// One isolated changed-path acceptance. Never attach to the person's profile.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
const out=process.env.WORKSPACE_HINTS_OUT||path.join(stage,'workspace-hints-'+Date.now());fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-workspace-hints-')),agents=path.join(home,'agents');fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),events=[],ev=($,f)=>events.push({$,...f,at});
for(const id of ['hint-a','hint-b']) { const root=path.join(home,id);fs.mkdirSync(root);ev('ProjectCreated',{id,title:id,root}); }
for(const [id,project] of [['parent','hint-a'],['child','hint-a'],['settled','hint-a'],['other','hint-b']])ev('ThreadCreated',{id,title:id==='child'?'Needle child':id,project,env:'local',branch:'',worktree:path.join(home,project),provider:'codex',model:''});
ev('ThreadParentSet',{id:'child',parent:'parent'});ev('ThreadSettled',{id:'settled',manual:true});
for(const [key,value] of [['thread.tree.parent','1'],['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1']])ev('SettingSet',{key,value});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const binary=stage+'/payload/backplane-serve',web=stage+'/payload/web',service=new Service({home,binary,web,expectedVersion:info.version});
const finalOnly=process.env.WORKSPACE_HINTS_FINAL==='1',resume=finalOnly||process.env.WORKSPACE_HINTS_RESUME==='1';
let app,page,sinkId,step='startup';const checks=[],focusBoundary={};
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&(!id||document.querySelector('main[data-thread]')?.dataset.thread===id),id);}
async function blur(){await page.evaluate(()=>document.activeElement?.blur?.());}
async function projection(){return page.locator('.sidebar').evaluate(el=>({projects:JSON.parse(el.dataset.shortcutProjects),threads:JSON.parse(el.dataset.shortcutThreads),mode:el.dataset.shortcutHints||''}));}
async function labels(kind){return page.locator(`.sidebar [data-shortcut-kind="${kind}"]`).evaluateAll((xs,kind)=>xs.map(el=>({id:kind==='threads'?el.closest('[data-on-click="select"]').dataset.value:(el.closest('.project-head')?.dataset.value||el.closest('[data-shortcut-project]')?.dataset.shortcutProject),digit:el.dataset.shortcutDigit,opacity:getComputedStyle(el,'::after').opacity})),kind);}
async function verify(kind){const p=await projection(),rows=await labels(kind);assert.equal(p.mode,kind);assert(rows.length>0,kind+' hint labels missing');for(const row of rows){assert.equal(row.digit,String(p[kind].indexOf(row.id)+1));assert.equal(row.opacity,'1');}return p;}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});page=await app.firstWindow();page.setDefaultTimeout(12000);await ready();
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();
 await page.locator('.sidebar [data-on-click="select"][data-value="parent"]').first().click();await ready('parent');
 await page.waitForFunction(()=>!!document.querySelector('.sidebar')?.dataset.shortcutThreads);
 const composer=page.locator('#composer');
 if(!resume){
 step='typing-guard';await composer.fill('unsent hint draft');await page.keyboard.down('Control');assert.equal((await projection()).mode,'threads');await page.keyboard.up('Control');assert.equal(await composer.inputValue(),'unsent hint draft');await page.keyboard.press('Escape');checks.push('Ctrl lights the thread digits from the composer and the draft stays');
 step='alt-projects';const before=await page.locator('.sidebar .project-head .name').evaluateAll(xs=>xs.map(el=>{const b=el.getBoundingClientRect();return {x:b.x,y:b.y,w:b.width,h:b.height};}));
 await page.keyboard.down('Alt');const p=await verify('projects');assert(p.projects.includes('hint-b'));const after=await page.locator('.sidebar .project-head .name').evaluateAll(xs=>xs.map(el=>{const b=el.getBoundingClientRect();return {x:b.x,y:b.y,w:b.width,h:b.height};}));assert.deepEqual(after,before);
 await page.screenshot({path:path.join(out,'alt-project-hints.png')});await page.keyboard.press(String(p.projects.indexOf('hint-b')+1));await page.keyboard.up('Alt');await ready('other');assert.equal((await projection()).mode,'');checks.push('Alt badges use exact project targets, no layout jump, and digit opens its project');
 step='ctrl-child';await page.locator('.sidebar [data-on-click="select"][data-value="parent"]').first().click();await ready('parent');await blur();await page.keyboard.down('Control');const t=await verify('threads');assert(t.threads.includes('child'));assert(!t.threads.includes('settled'));
 await page.screenshot({path:path.join(out,'ctrl-thread-hints.png')});await page.keyboard.press(String(t.threads.indexOf('child')+1));await page.keyboard.up('Control');await ready('child');checks.push('Ctrl badge on expanded child opens that exact child; folded settled thread omitted');
 step='collapse-shelf-refresh';await page.locator('.sidebar [data-on-click="thread-tree"][data-value="parent"]').click();await page.waitForFunction(()=>!JSON.parse(document.querySelector('.sidebar').dataset.shortcutThreads).includes('child'));
 await page.locator('.sidebar [data-on-click="toggle-settled"][data-value="hint-a"]').click();await page.waitForFunction(()=>JSON.parse(document.querySelector('.sidebar').dataset.shortcutThreads).includes('settled'));
 await blur();await page.keyboard.down('Control');const folded=await verify('threads');assert(!folded.threads.includes('child'));assert(folded.threads.includes('settled'));await page.keyboard.up('Control');checks.push('collapse and opened settled shelf refresh exact hint targets without independent numbering');
 } else {
  // Recreate only the suffix's starting context; do not repeat its four
  // retained prefix assertions, native digit activations or screenshots.
  step='suffix-setup';await page.locator('.sidebar [data-on-click="select"][data-value="child"]').first().click();await ready('child');
  if(!finalOnly){
   await page.locator('.sidebar [data-on-click="thread-tree"][data-value="parent"]').click();
   await page.waitForFunction(()=>!JSON.parse(document.querySelector('.sidebar').dataset.shortcutThreads).includes('child'));
   await page.locator('.sidebar [data-on-click="toggle-settled"][data-value="hint-a"]').click();
   await page.waitForFunction(()=>JSON.parse(document.querySelector('.sidebar').dataset.shortcutThreads).includes('settled'));
  }
 }
 // Drafts belong to threads. The prefix typed on parent, then selected
 // child; guard the current child's own draft through the unfinished suffix.
 const suffixDraft=await composer.inputValue();
 if(!finalOnly){
 step='search-projection';await page.locator('.sidebar input').first().fill('hint-a');
 await page.waitForFunction(()=>JSON.stringify(JSON.parse(document.querySelector('.sidebar').dataset.shortcutProjects))==='["hint-a"]');
 await blur();await page.keyboard.down('Control');const searched=await verify('threads');assert(!searched.threads.includes('child'));assert(searched.threads.includes('settled'));await page.keyboard.up('Control');assert.equal((await projection()).mode,'');checks.push('project-name search filters exact targets, retains explicit child collapse and opened shelf; keyup clears hints');
 }
 const native=await app.browserWindow(page);
 if(process.env.WORKSPACE_HINTS_PTY_ONLY!=='1'){
 step='focus-boundary';await page.locator('.sidebar input').first().fill('');await page.waitForFunction(()=>JSON.parse(document.querySelector('.sidebar').dataset.shortcutProjects).includes('hint-b'));await blur();await page.keyboard.down('Control');await verify('threads');
 const mainId=await native.evaluate(win=>win.id);
 const domFocus=()=>page.evaluate(()=>({focused:document.hasFocus(),visibility:document.visibilityState,hints:document.querySelector('.sidebar')?.dataset.shortcutHints||''}));
 focusBoundary.before={nativeFocused:await native.evaluate(win=>win.isFocused()),dom:await domFocus()};
 assert.equal(focusBoundary.before.nativeFocused,true,'Fixture main window must be focused before transfer');assert.equal(focusBoundary.before.dom.focused,true);
 // BrowserWindow.blur() alone need not transfer focus on an isolated display.
 // Own a neutral sink, prove actual focus loss, then evaluate hint cleanup.
 sinkId=await app.evaluate(async({BrowserWindow})=>{const sink=new BrowserWindow({show:false,width:180,height:120,webPreferences:{nodeIntegration:false,contextIsolation:true,sandbox:true}});await sink.loadURL('about:blank');sink.show();sink.focus();return sink.id;});
 focusBoundary.afterTransfer={nativeFocused:await native.evaluate(win=>win.isFocused()),sinkFocused:await app.evaluate(({BrowserWindow},id)=>BrowserWindow.fromId(id)?.isFocused(),sinkId),dom:await domFocus()};
 await page.waitForFunction(()=>!document.hasFocus(),null,{timeout:2500});
 assert.equal(await native.evaluate(win=>win.isFocused()),false,'Fixture failed native focus-loss precondition');
 step='focus-clear';await page.waitForFunction(()=>!document.querySelector('.sidebar').dataset.shortcutHints,null,{timeout:2500});focusBoundary.cleared=await domFocus();await page.keyboard.up('Control');
 await app.evaluate(({BrowserWindow},ids)=>{BrowserWindow.fromId(ids.sink)?.destroy();BrowserWindow.fromId(ids.main)?.focus();},{sink:sinkId,main:mainId});sinkId=null;await page.bringToFront();await page.waitForFunction(()=>document.hasFocus());checks.push('observed native and DOM focus loss clears held hints before key release');
 } else { await blur(); await page.keyboard.up('Control'); }
 if(process.env.WORKSPACE_HINTS_GEOMETRY_ONLY!=='1'){
 step='pty-guard';await composer.focus();await page.keyboard.press('`');await page.locator('#term').waitFor();await page.locator('#term').focus();await page.keyboard.down('Control');assert.equal((await projection()).mode,'');await page.keyboard.up('Control');await page.keyboard.press('Control+j');await page.locator('#term').waitFor({state:'detached'});assert.equal(await composer.inputValue(),suffixDraft);checks.push('PTY owns control input, hints hidden, existing bare-backtick/CtrlJ and draft intact');
 }
 step='390px-geometry';await native.evaluate(win=>win.setSize(390,760));await page.locator('.thread-head .back').first().click();await page.locator('.sidebar').waitFor({state:'visible'});await blur();
 const compactBoxes=()=>page.locator('.sidebar .project-head .name').evaluateAll(xs=>xs.map(el=>{const b=el.getBoundingClientRect();return {x:b.x,y:b.y,w:b.width,h:b.height};}));
 const compactBefore=await compactBoxes();await page.keyboard.down('Alt');await verify('projects');assert.deepEqual(await compactBoxes(),compactBefore);
 const bounds=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth,sidebar:document.querySelector('.sidebar').getBoundingClientRect().right,labels:[...document.querySelectorAll('.sidebar [data-shortcut-kind="projects"]')].map(el=>el.getBoundingClientRect().right)}));assert(bounds.width<=390);assert(bounds.scroll<=bounds.width+1);assert(bounds.labels.every(right=>right<=bounds.sidebar+1));await page.screenshot({path:path.join(out,'390-project-hints.png')});await page.keyboard.up('Alt');checks.push('390px project hints stay in-row without overflow or hold-induced movement');
 const r={state:'passed',stage,source:info.commit,resumeMode:resume,finalOnly,focusBoundary,checks,retainedPrefix:process.env.WORKSPACE_HINTS_PREFIX||null,retainedSearch:process.env.WORKSPACE_HINTS_SEARCH||null,liveProfileTouched:false};fs.writeFileSync(out+'/RESULT.json',JSON.stringify(r,null,2)+'\n');console.log(JSON.stringify(r));
}catch(e){fs.writeFileSync(out+'/FAIL.json',JSON.stringify({state:'failed',stage,source:info.commit,home,resumeMode:resume,finalOnly,focusBoundary,step,checks,error:e.stack},null,2)+'\n');if(page)await page.screenshot({path:out+'/failure.png'}).catch(()=>{});console.error(e.stack);process.exitCode=1;}finally{if(sinkId&&app)await app.evaluate(({BrowserWindow},id)=>BrowserWindow.fromId(id)?.destroy(),sinkId).catch(()=>{});if(app)await app.close();await service.stop();}})();
