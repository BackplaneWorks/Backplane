// Real mobile web UI against a private home and a supplied matching package.
// No live home, browser profile, CAD saves, agent turns or installation.
// node test/tools/mobile_web_viewers.mjs --bin PACKAGE/backplane-serve --web PACKAGE/web --out DIR
// Supply --project CAD_ROOT (.backplane.json), --mech PART.step (with its four
// adjacent renders) and --kicad-cli CLI. The package's step helper is used unless
// --step-helper names an explicitly supplied copy. Fixture inputs stay private.
// --diagnostic-feedback replaces ONLY the bundled feedback functions and CSS
// with current source for a narrow before/after probe; never a build receipt.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url), {Service}=require('../../desktop/service.cjs');
const args=process.argv.slice(2), opt=(n,d='')=>{const i=args.indexOf('--'+n);return i<0?d:args[i+1];};
const binary=path.resolve(opt('bin')), web=path.resolve(opt('web')), out=path.resolve(opt('out'));
assert.ok(opt('bin')&&opt('web')&&opt('out'),'supply --bin, --web, --out');
assert.ok(opt('project')&&opt('mech')&&(opt('kicad-cli')||process.env.BACKPLANE_KICAD_CLI),'supply --project, --mech and --kicad-cli (or BACKPLANE_KICAD_CLI)');
const stepHelper=path.resolve(opt('step-helper',path.join(path.dirname(binary),'backplane-step2glb')));
const kicadCli=path.resolve(opt('kicad-cli',process.env.BACKPLANE_KICAD_CLI));
assert.ok(fs.existsSync(stepHelper),'supply the package step helper with --step-helper');
assert.ok(fs.existsSync(kicadCli),'KiCad CLI does not exist');
assert.ok(!binary.startsWith(os.homedir()+'/.local/share/backplane/'),'copy the package; do not run installed files');
const diagnostic=args.includes('--diagnostic-feedback'), baseline=args.includes('--baseline');
assert.ok(!diagnostic||!baseline,'choose baseline or diagnostic override');
const pw=await import(opt('playwright','/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs'));
const engine=opt('engine','chromium');assert.ok(['chromium','webkit'].includes(engine));
const expectedRender=opt('expect-render','original');assert.ok(['original','preview'].includes(expectedRender));
const width=Number(opt('width','390')),height=Number(opt('height','844')),deviceScaleFactor=Number(opt('scale','1')),settleMs=Number(opt('settle-ms','0'));
assert.ok(Number.isInteger(width)&&width>=320&&width<=760&&Number.isInteger(height)&&height>=400&&height<=1400);
assert.ok(deviceScaleFactor>=1&&deviceScaleFactor<=3&&Number.isInteger(settleMs)&&settleMs>=0&&settleMs<=30000);
const {PNG}=require(opt('pngjs','/home/h/.bun/install/cache/pngjs@7.0.0@@@1'));
fs.mkdirSync(out,{recursive:true});const home=fs.mkdtempSync(path.join(out,'home-')), project=path.join(home,'project'),agents=path.join(home,'agents');
for(const dir of [project,agents,path.join(project,'mech'),path.join(project,'mech/renders')])fs.mkdirSync(dir,{recursive:true});
const cad=path.resolve(opt('project')), cfg=JSON.parse(fs.readFileSync(path.join(cad,'.backplane.json'))),pro=path.resolve(cad,cfg.project);
const source=[pro,pro.replace(/\.kicad_pro$/,'.kicad_pcb'),pro.replace(/\.kicad_pro$/,'.kicad_sch')];
const sha=p=>createHash('sha256').update(fs.readFileSync(p)).digest('hex'), before=source.map(p=>({path:p,sha256:sha(p)}));
fs.writeFileSync(path.join(project,'.backplane.json'),JSON.stringify({project:pro}));
fs.writeFileSync(path.join(project,'README.md'),'# Mobile viewer fixture\n\nNonempty file preview, with an unsent draft preserved.\n');
const mech=path.resolve(opt('mech'));
fs.copyFileSync(mech,path.join(project,'mech/gauge.step'));
for(const view of ['iso','front','top','right'])fs.copyFileSync(mech.replace(/\/([^/]+)\.step$/,`/renders/$1.${view}.png`),path.join(project,`mech/renders/gauge.${view}.png`));
const at=Math.floor(Date.now()/1000),ev=($,rest)=>({$,at,...rest});
fs.writeFileSync(path.join(home,'events.jsonl'),[
 ev('ProjectCreated',{id:'p1',title:'Mobile web viewer acceptance',root:project}),
 ...['t1','t2'].map(id=>ev('ThreadCreated',{id,project:'p1',title:'Viewer '+id,env:'local',branch:'',worktree:'',provider:'codex',model:''})),
 ...['onboard.done','onboard.uber'].map(key=>ev('SettingSet',{key,value:'1'})),
 ...['restart.continue','telemetry.enabled','mail.on'].map(key=>ev('SettingSet',{key,value:'off'}))
].map(JSON.stringify).join('\n')+'\n');
for(const n of ['claude','codex','grok','muse','opencode','pi','agy'])fs.writeFileSync(path.join(agents,n),'#!/bin/sh\nexit 127\n',{mode:0o755});
fs.symlinkSync(stepHelper,path.join(agents,'backplane-step2glb'));
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_KICAD_CLI:kicadCli,PATH:agents+':'+process.env.PATH});
const artifacts=Object.fromEntries(fs.readdirSync(web).filter(n=>n==='index.html'||/^chunk-.*\.(js|css)$/.test(n)).map(n=>[n,sha(path.join(web,n))]));
const service=new Service({home,binary,web}),facts=[],errors=[],http=[];let browser,page,version;
const record=(name,data={})=>{facts.push({name,...data});console.log('ok '+name);};
try{
 const conn=await service.start();
 version=conn.version;
 browser=await pw[engine].launch(engine==='chromium'?{headless:true,executablePath:opt('browser','/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome'),args:['--no-sandbox','--disable-dev-shm-usage','--use-angle=swiftshader','--enable-unsafe-swiftshader']}:{headless:true,executablePath:opt('browser','/home/h/.cache/ms-playwright/webkit-2359/pw_run.sh')});
 const context=await browser.newContext({viewport:{width,height},deviceScaleFactor,isMobile:true,hasTouch:true});
 await context.request.post(conn.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}});
 page=await context.newPage();page.setDefaultTimeout(30000);
 page.on('pageerror',e=>errors.push(e.message));page.on('response',r=>{if(new URL(r.url()).origin===conn.origin&&r.status()>=400)http.push({path:new URL(r.url()).pathname,status:r.status()});});
 if(diagnostic){
  // This explicitly bounded source substitution avoids an unrelated full Bend
  // build during diagnosis. Final acceptance must run without this option.
  await page.route('**/chunk-*.js',async route=>{
   const response=await route.fetch();let body=await response.text();
   const choice=/function ([\w$]+)\([^)]*\)\{return [^{}]+?"rv-page"[^{}]+?"bom"\}/.exec(body);
   if(choice&&body.includes('.workspace-bar,.viewer-tabs')){
    const start=body.indexOf('function ',choice.index+choice[0].length),end=body.indexOf('function ',start+9),part=body.slice(start,end),name=/^function ([\w$]+)/.exec(part)?.[1];
    assert.ok(part.includes('.workspace-bar,.viewer-tabs')&&name,'feedback function boundary changed');
    const src=fs.readFileSync('src/web/viewer-feedback.js','utf8').replace('export function viewerChoice',`function ${choice[1]}`).replace('export function installViewerFeedback',`function ${name}`);
    body=body.slice(0,choice.index)+src+'\n'+body.slice(end);
   }
   await route.fulfill({response,body,headers:{...response.headers(),'content-encoding':'identity'}});
  });
  await page.route('**/chunk-*.css',route=>route.fulfill({body:fs.readFileSync('src/web/style.css'),contentType:'text/css'}));
  record('diagnostic source override only; not a compiled artifact receipt');
 }
 await page.addInitScript(()=>{
  localStorage.setItem('backplane-selected','t1');
  const send=WebSocket.prototype.send;
  WebSocket.prototype.send=function(data){if(window.__holdViewerSend)(window.__heldViewerSends||=[]).push(()=>send.call(this,data));else send.call(this,data);};
 });
 await page.goto(conn.origin);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('.top-views summary'));
 const origin=page.url(),draft='Unsent mobile draft · 🛠';await page.locator('#composer').fill(draft);
 await page.locator('.top-views summary').tap();
 await page.evaluate(()=>{window.__holdViewerSend=true;});
 await page.locator('.top-views [data-on-click="view-kind"][data-value="board"]').tap();
 const pending=await page.evaluate(()=>({menuOpen:document.querySelector('.top-views details')?.open,viewer:!!document.querySelector('#viewer'),visibleStatus:[...document.querySelectorAll('[role=status],.workspace-feedback')].filter(e=>e.checkVisibility({visibilityProperty:true})).map(e=>e.textContent),busy:document.querySelector('.top-views summary')?.getAttribute('aria-busy')}));
 await page.screenshot({path:path.join(out,'pending.png')});
 if(baseline){assert.equal(pending.viewer,false);assert.ok(!pending.visibleStatus.some(x=>x.includes('Loading')));record('baseline reproduces silent mobile viewer tap while actual request is delayed',pending);}
 else{assert.equal(pending.menuOpen,false);assert.ok(pending.visibleStatus.some(x=>x.includes('Loading')),'mobile pending selection has no visible loading status');assert.equal(pending.busy,'true');record('delayed mobile touch acknowledged visibly and accessibly',pending);}
 await page.evaluate(()=>{window.__holdViewerSend=false;for(const send of window.__heldViewerSends||[])send();window.__heldViewerSends=[];});
 async function geometry(selector,label){
  // Optional observation after a real delay distinguishes a mounted pane
  // from content that remains visible after asynchronous work settles.
  if(settleMs)await new Promise(resolve=>setTimeout(resolve,settleMs));
  await page.waitForFunction(s=>{const c=document.querySelector(s);if(!c)return false;const g=c.getContext('2d'),p=g.getImageData(0,0,c.width,c.height).data;let color=0;for(let i=0;i<p.length;i+=4)if(Math.max(p[i],p[i+1],p[i+2])-Math.min(p[i],p[i+1],p[i+2])>40)color++;return color>600;},selector);
  const rect=await page.locator(selector).boundingBox();assert.ok(rect.width>100&&rect.height>100&&rect.x>=0&&rect.y>=0&&rect.x+rect.width<=width+1&&rect.y+rect.height<=height+1);
  await page.screenshot({path:path.join(out,label+'.png')});record(label+' real nonempty 2D geometry rendered inside mobile viewport',{rect});
 }
 async function choose(action,value){const parent=await page.locator('#viewer').count()?'#viewer':'.top-views';await page.locator(parent+' .workspace-menu > summary').tap();await page.locator(parent+` .workspace-menu [data-on-click="${action}"][data-value="${value}"]`).tap();}
 async function model(label){
  await page.waitForFunction(expected=>{const text=document.querySelector('#viewer')?.innerText||'';return /triangles/.test(text)&&(expected==='original'?text.includes('original model'):text.includes('rendering needs WebGL 2; preview remains available'));},expectedRender);
  if(settleMs)await new Promise(resolve=>setTimeout(resolve,settleMs));
  const canvas=expectedRender==='original'?page.locator('.solid-pathtrace'):page.locator('#solid');
  const box=await canvas.boundingBox();assert.ok(box.width>100&&box.height>100);
  const png=PNG.sync.read(await canvas.screenshot());let edges=0;
  // A smooth empty background has no sharp edges. Restrict this to the
  // middle of the actual canvas, excluding status/toolbar text.
  for(let y=Math.floor(png.height*.3);y<png.height*.8;y++)for(let x=Math.floor(png.width*.1);x<png.width*.9;x++){
   const i=(y*png.width+x)*4;if(Math.max(...[0,1,2].map(c=>Math.abs(png.data[i+c]-png.data[i+c-4])))>24)edges++;
  }
  assert.ok(edges>100,`${label}: no visible model geometry in the actual canvas (${edges} edges)`);
  await page.screenshot({path:path.join(out,label+'.png')});record(label+' real '+expectedRender+' model geometry rendered',{box,edges});
 }
 await geometry('#plot','board');
 if(!baseline)assert.equal(await page.locator('.viewer-choice-status').textContent(),'');
 await choose('view-kind','schematic');await geometry('#plot','schematic');
 await choose('view-kind','3d');
 await model('3d');
 if(args.includes('--context-cycle')){
  await page.locator('.solid-render-mode').selectOption('fast');
  await page.waitForFunction(()=>/triangles/.test(document.querySelector('.viewer-stat')?.textContent||'')&&!document.querySelector('.viewer-note')?.textContent);
  const context=await page.evaluate(async()=>{
   const canvas=document.querySelector('#solid'),gl=canvas.getContext('webgl'),ext=gl.getExtension('WEBGL_lose_context');
   if(!ext)throw Error('No graphics context loss extension');
   const prevented=await new Promise(resolve=>{canvas.addEventListener('webglcontextlost',e=>queueMicrotask(()=>resolve(e.defaultPrevented)),{once:true});ext.loseContext();});
   if(!prevented)throw Error('Graphics loss permanently retires this viewer');
   const restored=new Promise(resolve=>canvas.addEventListener('webglcontextrestored',resolve,{once:true}));
   await new Promise(r=>setTimeout(r,50));ext.restoreContext();await restored;
   await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
   return {prevented,isLost:gl.isContextLost()};
  });
  assert.equal(context.isLost,false);
  const png=PNG.sync.read(await page.locator('#solid').screenshot());let edges=0;
  for(let y=Math.floor(png.height*.3);y<png.height*.8;y++)for(let x=Math.floor(png.width*.1);x<png.width*.9;x++){const i=(y*png.width+x)*4;if(Math.max(...[0,1,2].map(c=>Math.abs(png.data[i+c]-png.data[i+c-4])))>24)edges++;}
  assert.ok(edges>100,'restored preview is blank');record('actual packaged preview recovers visible geometry after graphics loss',{...context,edges});
 }
 await choose('view-kind','split');await geometry('#plot-sch','split');
 await choose('mech','');
 // Mech opens the selected STEP in 3D; Renders takes the part's four views.
 await page.waitForSelector('[data-on-click="mech-renders"]');
 await page.waitForFunction(()=>document.querySelector('#solid')?.dataset.key.includes('gauge.step'));
 await model('mech-3d');
 await page.locator('[data-on-click="mech-renders"]').tap();
 await page.waitForSelector('.mech-grid img');
 const imgs=page.locator('.mech-grid img');assert.equal(await imgs.count(),4);await page.waitForFunction(()=>[...document.querySelectorAll('.mech-grid img')].every(i=>i.complete&&i.naturalWidth>0));
 await page.screenshot({path:path.join(out,'mech.png')});record('Mech real STEP part and all four rendered views visible');
 await choose('fm','');await page.waitForSelector('[data-on-click="fm-open"]');
 await page.locator('[data-on-click="fm-open"]').filter({hasText:'README.md'}).tap();await page.waitForFunction(()=>document.querySelector('#viewer')?.innerText.includes('Nonempty file preview'));
 record('Files opens real Markdown content');
 for(const [action,value,kind,expected] of [['diff','','diff','Not a git repository'],['rv-page','bom','review','BOM'],['view-kind','browser','browser',"hasn't opened"]]){await choose(action,value);await page.waitForFunction(({kind,expected})=>document.querySelector('#viewer')?.classList.contains(kind)&&document.querySelector('#viewer')?.innerText.includes(expected),{kind,expected});record(kind+' destination mounts its expected state');}
 await page.locator('#viewer .viewer-tabs [data-on-click="panel"]').tap();await page.waitForSelector('#viewer',{state:'detached'});assert.equal(await page.locator('#composer').inputValue(),draft);assert.equal(page.url(),origin);record('mobile close preserves draft and origin');
 await page.reload();await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');assert.equal(await page.locator('#composer').inputValue(),draft);record('reload preserves unsent draft');
 await page.setViewportSize({width:1440,height:900});await page.locator('.workspace-rail [data-on-click="view-kind"][data-value="board"]').click();await page.waitForSelector('#plot');
 const desktop=await page.locator('#viewer').boundingBox();assert.ok(desktop.x>500&&desktop.width>200);assert.equal(await page.locator('#composer').inputValue(),draft);record('desktop rail and side-by-side navigation preserved',{desktop});
 const screenshot=await page.locator('#plot').screenshot();const png=PNG.sync.read(screenshot);assert.ok(png.width>100&&png.height>100);
 await page.locator('[data-on-click="select"][data-value="t2"]').first().click();
 await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='t2');
 await page.locator('#composer').fill('Second thread private draft');
 assert.equal(await page.locator('#viewer').count(),0,'a new thread inherited the previous viewer');
 await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
 await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='t1'&&document.querySelector('#plot'));
 assert.equal(await page.locator('#composer').inputValue(),draft);record('thread switch restores only its own viewer and draft');
 assert.deepEqual(http,[],'same-origin viewer asset/transport HTTP errors');
 const significant=errors.filter(x=>!x.startsWith('ResizeObserver loop'));assert.deepEqual(significant,[],'browser runtime errors');
 assert.deepEqual(source.map(p=>({path:p,sha256:sha(p)})),before,'canonical CAD changed');
 fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify({scope:diagnostic?'diagnostic-source-override':'matching-package-mobile-web',baseline,engine,expectedRender,viewport:{width,height,deviceScaleFactor},settleMs,browserVersion:await browser.version(),userAgent:await page.evaluate(()=>navigator.userAgent),version,binary,serviceSHA256:sha(binary),stepHelper,stepHelperSHA256:sha(stepHelper),kicadCli,web,artifacts,source:before,facts,errors,http},null,2)+'\n');
}catch(error){if(page){await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});fs.writeFileSync(path.join(out,'failure.json'),JSON.stringify({error:String(error.stack||error),facts,errors,http},null,2));}throw error;}
finally{await browser?.close();await service.stop();}
