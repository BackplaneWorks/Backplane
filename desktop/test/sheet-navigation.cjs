// Integrator-owned acceptance: copied KiCad demo, isolated service and profile.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../..');
const {_electron}=require('playwright-core'),{Service}=require(root+'/desktop/service.cjs');
const stage=path.resolve(process.argv[2]),out=path.resolve(process.argv[3]),info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json'));
fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(os.tmpdir()+'/bp-sheet-nav-'),project=home+'/multichannel',agents=home+'/agents';
fs.cpSync('/usr/share/kicad/demos/multichannel',project,{recursive:true});fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(agents+'/'+name,'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),ev=($,fields)=>({$,...fields,at});
const events=[ev('ProjectCreated',{id:'sheets',title:'Sheet navigation fixture',root:project}),
 ...['sheet-a','sheet-b'].map(id=>ev('ThreadCreated',{id,title:id,project:'sheets',env:'local',branch:'',worktree:project,provider:'codex',model:''})),
 ...[['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1']].map(([key,value])=>ev('SettingSet',{key,value}))];
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};
Object.assign(process.env,env);
const shellQuote=s=>"'"+s.replaceAll("'", "'\"'\"'")+"'";
const wrapper=home+'/fixture-service';fs.writeFileSync(wrapper,'#!/bin/sh\nexec '+shellQuote(stage+'/payload/backplane-serve')+' "$@" >>'+shellQuote(out+'/service-stdio.log')+' 2>&1\n',{mode:0o755});
const service=new Service({home,binary:wrapper,web:stage+'/payload/web',expectedVersion:info.version});
const resume=process.env.SHEET_NAV_SUFFIX==='1';const retained=resume?JSON.parse(fs.readFileSync(process.env.SHEET_NAV_PREFIX_RECEIPT)):null;
if(retained){assert.equal(retained.source,info.commit);assert(retained.checks.includes('returning to a manually panned sheet preserves its camera'));}
let app,page,step='bootstrap';const checks=[],observations={};
const result=state=>({state,source:info.commit,stage,step,checks,observations,home,liveProfileTouched:false,phoneRuntime:'not run',retainedPrefix:retained?{receipt:process.env.SHEET_NAV_PREFIX_RECEIPT,checks:retained.checks}:null});
async function ready(id='sheet-a'){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main')?.dataset.thread===id&&document.querySelector('main')?.getAttribute('aria-busy')!=='true',id);}
async function current(value,id='sheet-a'){await page.waitForFunction(value=>document.querySelector('.sch-tree .sch-name[aria-current="page"]')?.dataset.value===value,value);await ready(id);}
async function scene(){await page.waitForFunction(()=>{const c=document.querySelector('#plot'),s=window.__sheetScene;return c&&s?.key===c.dataset.key&&s.strokes>0&&c.clientWidth>100&&c.clientHeight>80;});}
async function action(name){await page.locator(`[data-on-click="${name}"]`).click();await ready();}
async function matrix(){return page.evaluate(()=>({...window.__sheetScene}));}
async function geometry(){return page.evaluate(()=>{const box=el=>{const r=el.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height};};return{viewport:[innerWidth,innerHeight],tree:box(document.querySelector('.sch-tree')),buttons:[...document.querySelectorAll('.sch-btn,.sch-fold:not(.none),.sch-name,.sch-crumb')].map(el=>({box:box(el),disabled:el.disabled,action:el.dataset.onClick})),viewer:box(document.querySelector('#viewer')),documentWidth:document.documentElement.scrollWidth};});}
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',env:{...env,BACKPLANE_SERVICE:stage+'/payload/backplane-serve',BACKPLANE_WEB:stage+'/payload/web',BACKPLANE_SERVICE_VERSION:info.version}});
 page=await app.firstWindow();page.setDefaultTimeout(12000);await page.setViewportSize({width:1280,height:900});
 await page.addInitScript(()=>{window.__sheetScene=null;const p=CanvasRenderingContext2D.prototype,transform=p.setTransform,stroke=p.stroke;
  p.setTransform=function(...a){const r=transform.apply(this,a);if(this.canvas.id==='plot'&&a.length===6&&a[0]!==1)this.__sheetTransform=a.slice();return r;};
  p.stroke=function(...a){if(this.canvas.id==='plot'&&this.__sheetTransform){const t=this.__sheetTransform,key=this.canvas.dataset.key,old=window.__sheetScene;window.__sheetScene={key,scale:t[0]/devicePixelRatio,ox:t[4]/devicePixelRatio,oy:t[5]/devicePixelRatio,strokes:old?.key===key?old.strokes+1:1};}return stroke.apply(this,a);};
 });
 // Reload only this fresh isolated renderer so the passive canvas observer precedes its first scene.
 await page.reload();await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();await page.waitForFunction(()=>!document.querySelector('.tour-back'));
 await page.locator('.sidebar [data-on-click="select"][data-value="sheet-a"]').first().click();await ready();
 step='open-schematic';await page.locator('[data-on-click="view-kind"][data-value="schematic"]').first().click();
 await page.locator('.sch-tree .sch-name').first().waitFor();await scene();
 const rows=await page.locator('.sch-tree .sch-name').evaluateAll(es=>es.map(e=>({value:e.dataset.value,text:e.textContent})));
 assert.equal(rows.length,5);assert.equal(new Set(rows.map(r=>r.value.split('\t')[2])).size,5);
 const top=rows[0],children=rows.slice(1);observations.rows=rows;assert.equal(await page.locator('[data-on-click="sch-up"]').isDisabled(),true);assert.equal(await page.locator('[data-on-click="sch-root"]').isDisabled(),true);
 let hit;
 if(!resume){
 checks.push('actual copied demo renders root and four unique repeated-file instances with disabled root boundaries');
 step='repeated-branches';for(const child of children){await page.locator('.sch-tree .sch-name').filter({hasText:child.text}).click();await current(child.value);await scene();assert.equal(await page.locator('.sch-tree .sch-name[aria-current="page"]').count(),1);assert((await page.locator('.sch-pos').innerText()).match(/Sheet [2-5] of 5/));}
 checks.push('each repeated child selects alone, renders real geometry and shows its KiCad page');
 step='up-history';const last=children.at(-1);await action('sch-up');await current(top.value);await action('sch-back');await current(last.value);await action('sch-fwd');await current(top.value);
 await page.locator('.sch-tree .sch-name').filter({hasText:children[0].text}).click();await current(children[0].value);await action('sch-root');await current(top.value);checks.push('Up/Top and Back/Forward move between exact instances');
 step='canvas-child';await scene();hit=await page.evaluate(()=>{const c=document.querySelector('#plot'),r=c.getBoundingClientRect(),s=window.__sheetScene,f=c.dataset.sheets.split('\n')[0].split('\t'),n=f[3].split(/\s+/).map(Number);return{value:f.slice(0,3).join('\t'),x:r.x+s.ox+(n[0]+n[2])*500*s.scale,y:r.y+s.oy+(n[1]+n[3])*500*s.scale};});
 assert(Number.isFinite(hit.x)&&Number.isFinite(hit.y));await page.mouse.click(hit.x,hit.y);await current(hit.value);await scene();checks.push('trusted pointer click on the actual root sheet symbol enters its exact instance');
 step='pan-return';const before=await matrix(),c=await page.locator('#plot').boundingBox();await page.mouse.move(c.x+c.width/2,c.y+c.height/2);await page.mouse.down();await page.mouse.move(c.x+c.width/2+25,c.y+c.height/2+15,{steps:4});await page.mouse.up();
 await page.waitForFunction(old=>window.__sheetScene.ox!==old.ox||window.__sheetScene.oy!==old.oy,before);const moved=await matrix();
 await action('sch-up');await current(top.value);await action('sch-back');await current(hit.value);await scene();const returned=await matrix();for(const k of ['scale','ox','oy'])assert(Math.abs(returned[k]-moved[k])<0.01,`return camera ${k}`);checks.push('returning to a manually panned sheet preserves its camera');
 }else{step='suffix-prerequisite';hit={value:children[0].value};await page.locator('.sch-tree .sch-name').filter({hasText:children[0].text}).click();await current(hit.value);await scene();}
 step='thread-isolation';await page.locator('.sidebar [data-on-click="select"][data-value="sheet-b"]').first().click();await ready('sheet-b');
 observations.freshSibling={viewerCount:await page.locator('#viewer').count(),hierarchyRows:await page.locator('.sch-tree .sch-name').count()};
 await page.locator('[data-on-click="view-kind"][data-value="schematic"]').first().click();await page.locator('.sch-tree .sch-name').first().waitFor();await current(top.value,'sheet-b');
 await page.locator('.sch-tree .sch-name').filter({hasText:children[1].text}).click();await current(children[1].value,'sheet-b');
 await page.locator('.sidebar [data-on-click="select"][data-value="sheet-a"]').first().click();await ready();await current(hit.value);checks.push('two threads keep distinct selected instances when returning');
 step='keyboard-and-fold';await action('sch-root');await current(top.value);const fold=page.locator('.sch-tree .sch-fold').first();await fold.focus();await page.keyboard.press('Space');await page.waitForFunction(()=>document.querySelectorAll('.sch-tree .sch-name').length===1);assert(await fold.evaluate(el=>el===document.activeElement));await page.keyboard.press('Enter');await page.waitForFunction(()=>document.querySelectorAll('.sch-tree .sch-name').length===5);checks.push('Space/Enter fold and expand without losing focus or selection');
 observations.desktop=await geometry();await page.screenshot({path:out+'/desktop.png'});
 step='390-geometry';await page.setViewportSize({width:390,height:844});await page.emulateMedia({reducedMotion:'reduce'});await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));observations.mobile=await geometry();assert(observations.mobile.tree.height<=844*.28+2);assert(observations.mobile.documentWidth<=390);
 for(const b of observations.mobile.buttons)assert(b.box.x>=-1&&b.box.x+b.box.width<=391,'hierarchy control within viewport');await page.screenshot({path:out+'/390.png'});checks.push('390px controls and scroll-bounded tree stay within the viewport');
 const persisted=fs.readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(JSON.parse);assert(!persisted.some(e=>e.$==='MessagePosted'));checks.push('navigation never posts a message or starts a real provider');
 fs.writeFileSync(out+'/RESULT.json',JSON.stringify(result('passed'),null,2)+'\n');console.log(JSON.stringify(result('passed')));
 }catch(e){const r={...result('failed'),error:e.stack};fs.writeFileSync(out+'/RESULT.json',JSON.stringify(r,null,2)+'\n');if(page){fs.writeFileSync(out+'/failure.html',await page.content().catch(()=>''));await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}console.error(e.stack);process.exitCode=1;
 }finally{if(app)await app.close();await service.stop();}})();
