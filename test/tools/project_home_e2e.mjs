// Real rendered controls, through a fresh owned hub and stand-in agents.
// No existing hub address, home, invitation, token or stdin input is accepted.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import * as Frame from '../../src/web/wire.js';
import {configuration,startFixture,assertOwnedResource,assertOwnedURL} from './instant_ui_fixture.mjs';
const config=configuration();
if (!process.stdin.isTTY && fs.readFileSync(0,'utf8').trim()) throw Error('No external pairing input');
const require=createRequire(import.meta.url),{chromium}=require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const output=path.resolve(process.env.PROJECT_HOME_OUT || 'build/project-home/e2e');
fs.mkdirSync(output,{recursive:true});
const put=(root,name,data)=>{const dest=path.join(root,name);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,data);};
function triangle(){
 const positions=Buffer.alloc(36);[0,0,0,10,0,0,0,10,0].forEach((v,i)=>positions.writeFloatLE(v,i*4));
 const data={asset:{version:'2.0'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives:[{attributes:{POSITION:0},material:0}]}],
  materials:[{pbrMetallicRoughness:{baseColorFactor:[.5,.5,.5,1],metallicFactor:0,roughnessFactor:1},doubleSided:true}],
  buffers:[{byteLength:36}],bufferViews:[{buffer:0,byteOffset:0,byteLength:36,target:34962}],accessors:[{bufferView:0,componentType:5126,count:3,type:'VEC3',min:[0,0,0],max:[10,10,0]}]};
 const raw=Buffer.from(JSON.stringify(data)),json=Buffer.alloc(Math.ceil(raw.length/4)*4,32);raw.copy(json);
 const head=Buffer.alloc(20);head.writeUInt32LE(0x46546c67);head.writeUInt32LE(2,4);head.writeUInt32LE(20+json.length+8+positions.length,8);head.writeUInt32LE(json.length,12);head.writeUInt32LE(0x4e4f534a,16);
 const bin=Buffer.alloc(8);bin.writeUInt32LE(positions.length);bin.writeUInt32LE(0x004e4942,4);return Buffer.concat([head,json,bin,positions]);
}
// Diagnostics retain sizes, routing and command methods, never message bodies.
const dictionaries=fs.readFileSync('src/core/cbor.bend','utf8');
const dict=name=>{const block=dictionaries.split('def Cbor.'+name+'()')[1].split('\ndef ')[0];return JSON.parse(block.slice(block.indexOf('['),block.lastIndexOf(']')+1));};
const keys=dict('keys'),words=dict('words');
function wireObject(data){return typeof data==='string'?JSON.parse(data):Frame.plain(Frame.cbor(new Uint8Array(data),keys,words));}
let boardRoot;
const fixture=await startFixture({...config,prepare({root,project,rows,at}){
 boardRoot=project;rows[0].title='Fixture board';
 put(project,'.backplane/intent.md','# Goal\nSynthetic board for UI qualification.\n\n# Parts\n\n# Notes\nPrivate test only.\n');
 put(project,'.backplane.json',JSON.stringify({project:'boards/ui/ui.kicad_pro',drc:'build/reports/drc.json',erc:'build/reports/erc.json'}));
 put(project,'boards/ui/ui.kicad_pro','{}');
 put(project,'boards/ui/ui.kicad_pcb',fs.readFileSync('test/fixtures/pcb-bottom-silk/asymmetric.kicad_pcb'));
 put(project,'boards/ui/ui.kicad_sch',fs.readFileSync('test/fixtures/fork-viewers/aliases.kicad_sch'));
 put(project,'boards/ui/ui.fab.json',JSON.stringify({kind:'pcb',name:'Fixture board',source:'ui.kicad_pcb',qty:5,options:{layers:2},features:{width_mm:60,height_mm:40,min_drill_mm:.3}}));
 put(project,'build/reports/drc.json',JSON.stringify({source:'ui.kicad_pcb',date:'2026-11-03T00:00:00',violations:[{severity:'error'},{severity:'warning'},{severity:'warning'}],unconnected_items:[{items:[]}]}));
 put(project,'build/reports/erc.json',JSON.stringify({source:'ui.kicad_sch',date:'2026-11-03T00:00:00',sheets:[{violations:[{severity:'warning'}]}]}));
 put(project,'README.md','# Fixture README\n'+'Bounded file continuation. 日本語\n'.repeat(2000));
 put(project,'mech/fixture.glb',triangle());
 put(project,'mech/renders/fixture.iso.png',Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aB0cAAAAASUVORK5CYII=','base64'));
 for(const [id,title,folder]of [['fixture-spec','Fixture specification','spec'],['fixture-software','Fixture software','software']]){
  const dir=path.join(root,folder);put(dir,'.backplane/intent.md',`# Goal\n${title}.\n\n# Parts\n\n# Notes\n`);
  put(dir,folder==='spec'?'docs/SPEC.md':'README.md',folder==='spec'?'# Specification\nPlanning stage. No board has been created.':'# Software\nA software-only test project.');
  rows.push({$:'ProjectCreated',id,title,root:dir,at},{$:'ThreadCreated',id:id+'-thread',project:id,title:'A synthetic project thread',env:'local',branch:'',worktree:dir,provider:'claude',model:'fixture',at});
 }
 rows.push({$:'SettingSet',key:'order.estimates',value:'off',at});
}});
const report={isolated:true,fixtureRoot:fixture.root,checks:[],latency:[],errors:[],screens:[],wire:[]};
let browser,context,device,page,delay=0;const sendIds=new Set();
function wireRecord(data,from){try{const o=wireObject(data);if(from==='client'&&o.m==='view.action'&&o.p?.action==='send'){sendIds.add(o.id);const v=o.p.session;report.wire.push({from,id:o.id,action:o.p.action,target:o.p.target,selected:v?.sel,bytes:Buffer.byteLength(v?.drafts?.[v?.sel]||''),bound:v?.drafts?.['@draft.bound']||''});}else if(from==='client'&&o.m==='project.start')report.wire.push({from,id:o.id,rpc:o.m,project:o.p.project,bytes:Buffer.byteLength(o.p.text||'')});else if(from==='server'&&sendIds.has(o.id)){const r=o.result||{};report.wire.push({from,id:o.id,ok:o.ok,error:o.error,nextTarget:r.nextTarget,cmds:(r.cmds||[]).map(c=>{let p={};try{p=JSON.parse(c.text||'{}')}catch{}return {type:c.type,rpc:p.m,bytes:typeof p.p?.text==='string'?Buffer.byteLength(p.p.text):0}})});}}catch{}}
const check=(name,condition)=>{assert(condition,name);report.checks.push(name);};
try{
 browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox','--disable-dev-shm-usage']});
 context=await browser.newContext({viewport:{width:1280,height:1000},serviceWorkers:'block'});
 await context.addInitScript(()=>{globalThis.__homeStatus=[];globalThis.__homeClicks=[];document.addEventListener('click',event=>{const node=event.target.closest?.('[data-on-click]');if(node?.dataset.onClick==='send')globalThis.__homeClicks.push({action:node.dataset.onClick,selected:document.querySelector('main')?.getAttribute('data-project-home')});},true);new MutationObserver(()=>{for(const node of document.querySelectorAll('#view-connection-text,.home-error')){const text=node.textContent;if(text&&globalThis.__homeStatus.at(-1)!==text)globalThis.__homeStatus.push(text);}}).observe(document,{subtree:true,childList:true,characterData:true});});
 await context.route('**/*',route=>{try{assertOwnedResource(route.request().url(),fixture.origin);return route.continue()}catch{report.errors.push('Blocked external resource');return route.abort('blockedbyclient')}});
 await context.routeWebSocket('**/*',socket=>{
  assertOwnedURL(socket.url(),fixture.origin,true);const server=socket.connectToServer();
  socket.onMessage(data=>{wireRecord(data,'client');server.send(data)});
  server.onMessage(data=>{wireRecord(data,'server');const deliver=()=>{try{socket.send(data)}catch{}};if(delay)setTimeout(deliver,delay);else deliver();});
 });
 const name='TEST project-home '+Date.now(),code=await fixture.invitation();
 const enrolled=await context.request.post(fixture.origin+'/auth/enroll',{headers:{Authorization:'Bearer '+code,'X-Backplane-Name':name},maxRedirects:0});assert(enrolled.ok());
 device=(await (await context.request.get(fixture.origin+'/auth/devices')).json()).devices.find(d=>d.name===name)?.id;assert(device);
 page=await context.newPage();page.on('pageerror',e=>{if(!e.message.startsWith('ResizeObserver loop'))report.errors.push(e.message)});
 await page.goto(fixture.origin);await page.waitForSelector('[data-on-click="proj-home"]',{timeout:90000});
 const ready=()=>page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const click=(action,value)=>page.locator(`[data-on-click="${action}"]${value===undefined?'':`[data-value="${value}"]`}`).filter({visible:true}).first().click();
 async function open(id){if(!await page.locator(`[data-on-click="proj-home"][data-value="${id}"]`).first().isVisible())await page.locator('main .home-header .back').click();await click('proj-home',id);await page.waitForSelector(`main[data-project-home="${id}"]`);await ready();}
 const initialThreads=fixture.evidence().threads.length;
 await open('fixture-project');await page.getByRole('heading',{name:'Fab readiness',exact:true}).waitFor();
 await page.getByText('1 errors · 2 warnings · 1 unconnected',{exact:true}).waitFor();
 await page.locator('main .home-live-viewer .viewer').waitFor();check('saved board mounts lazily inside the project home',true);
 await page.waitForFunction(()=>{const c=document.querySelector('main .home-live-viewer canvas.plot2d');if(!c?.width||!c.height)return false;const ctx=c.getContext('2d');if(!ctx)return false;const d=ctx.getImageData(0,0,c.width,c.height).data;for(let i=0;i<d.length;i+=16){const r=d[i],g=d[i+1],b=d[i+2];if(r>110&&r>g*1.35&&r>b*1.35||g>100&&g>r*1.35&&g>b*1.35||r>130&&g>110&&b<75)return true;}return false;},null,{timeout:45000,polling:250});
 check('saved board geometry is rendered rather than a loading placeholder',true);
 check('opening a project creates no conversation',fixture.evidence().threads.length===initialThreads);
 check('board/schematic/DRC/ERC/mech/readiness controls are present',await page.locator('main [data-on-click="home-model"]').count()>=1&&await page.locator('main [data-on-click="home-bom"]').count()===1);
 const composer=page.locator('#composer');await composer.fill('Keep this draft through every name edit.');
 async function rename(kind,id,title,save=true){
  await page.evaluate(({kind,id})=>{const el=[...document.querySelectorAll('main [data-on-click="home-rename"]')].find(n=>{const v=JSON.parse(n.dataset.value);return v.kind===kind&&v.id===id});if(!el)throw Error('Missing inline name control '+kind);el.click();},{kind,id});
  const input=page.getByRole('textbox',{name:'Display name',exact:true});await input.waitFor();await input.fill(title);await input.press(save?'Enter':'Escape');await input.waitFor({state:'detached'});
 }
 await rename('project','fixture-project','Renamed board project');
 await page.waitForFunction(()=>document.querySelector('main h1')?.textContent==='Renamed board project');
 check('project rename updates the canonical sidebar',await page.locator('[data-on-click="proj-home"][data-value="fixture-project"]').first().textContent()==='Renamed board project');
 await rename('project','fixture-project','Discard project name',false);
 check('Escape leaves the saved project name',await page.locator('main h1').innerText()==='Renamed board project');
 const threadId=fs.readFileSync(fixture.log,'utf8').trim().split('\n').map(JSON.parse).find(e=>e.$==='ThreadCreated'&&e.project==='fixture-project')?.id;assert(threadId);
 await rename('thread',threadId,'Renamed discussion');
 await page.waitForFunction(()=>[...document.querySelectorAll('main .home-thread')].some(n=>n.textContent.includes('Renamed discussion')));
 check('thread rename uses the shared sidebar path',await page.locator(`.sidebar [data-on-click="select"][data-value="${threadId}"]`).first().innerText().then(t=>t.includes('Renamed discussion')));
 await rename('artifact',path.join(boardRoot,'boards/ui/ui.kicad_pcb'),'Main logic board');
 await page.getByRole('button',{name:/Main logic board/}).first().waitFor();
 await rename('artifact',path.join(boardRoot,'mech/fixture.glb'),'Enclosure');
 await page.getByRole('button',{name:/Enclosure/}).first().waitFor();
 check('display renames retain the saved CAD and model paths',fs.existsSync(path.join(boardRoot,'boards/ui/ui.kicad_pcb'))&&fs.existsSync(path.join(boardRoot,'mech/fixture.glb')));
 await page.locator('main [title="Change project icon"]').click();
 const icons=page.locator('main .look .glyphs button');await icons.first().waitFor();check('home opens the existing fourteen-choice icon picker',await icons.count()===14);
 await page.locator('main .look [data-on-click="look-set"][title="cpu"]').click();
 await page.waitForFunction(()=>document.querySelector('main .home-project-icon svg')?.getAttribute('data-icon')==='cpu');
 check('home icon choice synchronizes the sidebar badge',await page.locator('[data-on-click="look"][data-value="fixture-project"] svg[data-icon="cpu"]').count()>=1);
 await page.locator('main .look [data-on-click="look"]').click();
 check('renames and icon edits preserve the exact unsent draft',await composer.inputValue()==='Keep this draft through every name edit.');

 await click('home-intent');const editor=page.locator('textarea[data-bfield^="home-intent:"]');await editor.fill('# Goal\nEdited through the real home control.\n\n# Parts\n\n# Notes\n');await click('home-save');
 await page.waitForFunction(()=>!document.querySelector('textarea[data-bfield^="home-intent:"]'));
 check('Save writes the project intent',fs.readFileSync(path.join(boardRoot,'.backplane/intent.md'),'utf8').includes('Edited through the real home control.'));
 await click('home-intent');await editor.fill('Discard this local edit');await click('home-cancel');
 check('Cancel preserves the saved intent',!fs.readFileSync(path.join(boardRoot,'.backplane/intent.md'),'utf8').includes('Discard this'));
 const readme=path.join(boardRoot,'README.md');await click('home-document',readme);await page.locator('.home-document-body').waitFor();
 const first=await page.locator('.home-document-body').innerText();check('document has a bounded first section',Buffer.byteLength(first)<=8192);
 await click('home-document-more');await page.waitForFunction(first=>document.querySelector('.home-document-body')?.textContent!==first,first);
 check('document continuation reads another section',await page.locator('.home-document-body').innerText()!==first);await click('home-document-close');
 await click('home-price');await page.getByRole('heading',{name:'Price estimate',exact:true}).waitFor();check('estimate control produces a price result',true);
 await click('home-bom');await page.locator('main .viewer .review-pane').first().waitFor({timeout:30000});await page.locator('main .review-body[data-review-ready="1"]').waitFor({timeout:45000});check('BOM opens populated existing review viewer',true);check('review heading has no unsupported artifact rename control',await page.locator('main .home-live-viewer > h2 [data-on-click="home-rename"]').count()===0);await click('panel');
 for(const [id,title]of [['fixture-spec','Specification'],['fixture-software','README']]){
  await open(id);await page.getByRole('heading',{name:title,exact:true,level:2}).waitFor();
  check(id+' has no CAD controls',await page.locator('main [data-on-click="home-view"],main [data-on-click="home-model"],main [data-on-click="home-bom"]').count()===0);
 }
 // A delayed view reply proves first paint comes from the transition cache.
 await open('fixture-project');await page.getByRole('heading',{name:'Fab readiness',exact:true}).waitFor();
 await open('fixture-software');await page.getByRole('heading',{name:'README',exact:true}).waitFor();delay=800;
 const elapsed=await page.evaluate(async()=>{
  const button=[...document.querySelectorAll('[data-on-click="proj-home"]')].find(n=>n.dataset.value==='fixture-project');
  const start=performance.now();button.click();while(performance.now()-start<5000){await new Promise(requestAnimationFrame);if(document.querySelector('main[data-project-home="fixture-project"]'))return performance.now()-start;}return null;
 });report.latency.push({cached:true,firstContentMs:elapsed,replyDelayMs:delay});check('cached project opens within 100 ms while reply is delayed',elapsed!==null&&elapsed<=100);await ready();delay=0;
 for(const width of [1280,390])for(const theme of ['dark','light']){
  await page.setViewportSize({width,height:1000});
  // Exercise the actual theme CSS against the actual emitted DOM; theme state
  // is otherwise owned by the hub, so no persisted preference is changed.
  await page.evaluate(theme=>document.querySelector('.app').classList.toggle('light',theme==='light'),theme);
  const geometry=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth,row:document.querySelector('.composer-foot').getBoundingClientRect().height}));
  check(`layout ${width} ${theme}: no overflow and one control row`,geometry.scroll<=width&&geometry.row===(width===390?36:28));
  const file=path.join(output,`app-${width}-${theme}.png`);await page.screenshot({path:file});report.screens.push(file);
 }
 await page.setViewportSize({width:390,height:1000});await open('fixture-spec');await ready();
 const firstMessage='One exact project-home question\n'+'Preserve the entire message 日本語. '.repeat(2400);
 const documentId=await page.evaluate(()=>globalThis.__homeDocumentId=crypto.randomUUID());
 await page.locator('#composer').fill(firstMessage);await click('send');
 await page.waitForFunction(()=>document.querySelector('main[data-thread]')?.getAttribute('data-thread')?.startsWith('home-'),null,{timeout:45000});
 const events=fs.readFileSync(fixture.log,'utf8').trim().split('\n').map(JSON.parse);
 const created=events.filter(e=>e.$==='ThreadCreated'&&e.project==='fixture-spec'&&e.id.startsWith('home-'));
 check('home send creates one thread in its project',created.length===1);
 check('home send keeps the current document without native form submission',await page.evaluate(()=>globalThis.__homeDocumentId)===documentId);
 check('home send posts its exact first message once',events.filter(e=>e.$==='MessagePosted'&&e.thread===created[0].id&&e.text===firstMessage).length===1);
 check('browser has no page errors or external requests',report.errors.length===0);
 console.log('ok '+report.checks.length+' isolated project-home checks');
}catch(e){report.errors.push(e.stack||String(e));process.exitCode=1;if(page)try{await page.screenshot({path:path.join(output,'failure.png')});report.failureText=(await page.locator('body').innerText()).slice(0,3000);report.statusHistory=await page.evaluate(()=>globalThis.__homeStatus);report.sendClicks=await page.evaluate(()=>globalThis.__homeClicks)}catch{}}
finally{
 try{if(context&&device)report.revoked=(await context.request.post(fixture.origin+'/auth/revoke',{headers:{'X-Backplane-Device':device},maxRedirects:0})).ok()}catch{}
 try{await browser?.close()}finally{await fixture.stop();fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));}
 if(!report.revoked)process.exitCode=1;
 console.log('isolated report '+path.join(output,'report.json'));
}
