// Full sidebar metadata and sent-image rendering on the actual Electron client.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json')));
const out=path.join(stage,'catalog-images-'+Date.now());fs.mkdirSync(out);
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-catalog-images-')),agents=path.join(home,'agents');fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),ev=($,fields)=>({$,...fields,at});
const events=[];
for(let i=0;i<43;i++){
 const root=path.join(home,'p'+i);fs.mkdirSync(root);events.push(ev('ProjectCreated',{id:'p'+i,title:'Catalog project '+String(i).padStart(2,'0'),root}));
 if(i<42)events.push(ev('ThreadCreated',{id:'t'+i,title:'Catalog thread '+i,project:'p'+i,env:'local',branch:'',worktree:root,provider:'codex',model:'gpt-6.1-sol'}));
}
events.push(ev('ProjectRemoved',{id:'p41'}),ev('ThreadArchived',{id:'t40',archived:true}));
// Crowd recent24 with just two projects, as in the original missing-catalog bug.
for(let i=0;i<30;i++)events.push(ev('ThreadCreated',{id:'recent'+i,title:'Recent '+i,project:'p'+(i%2),env:'local',branch:'',worktree:path.join(home,'p'+(i%2)),provider:'codex',model:'gpt-6.1-sol'}));
const attachments=path.join(home,'attachments','t0');fs.mkdirSync(attachments,{recursive:true});
for(const name of ['one.png','two.png'])fs.copyFileSync(path.resolve(__dirname,'../../assets/icon/backplane-64.png'),path.join(attachments,name));
fs.writeFileSync(path.join(attachments,'notes.txt'),'non-image fixture');
const text='Sent screenshots should appear here.\n\n'+['one.png','two.png','notes.txt'].map(n=>'[attachment: '+path.join(attachments,n)+']').join('\n');
events.push(ev('MessagePosted',{thread:'t0',msg:'sent-images',role:'user',text}),ev('MessagePosted',{thread:'t30',msg:'private-other-body',role:'assistant',text:'OTHER_BODY_MUST_NOT_APPEAR_IN_T0'}),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>ev('SettingSet',{key,value:'off'})),ev('SettingSet',{key:'onboard.done',value:'1'}));
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
const service=new Service({home,binary:path.join(stage,'payload/backplane-serve'),web:path.join(stage,'payload/web'),expectedVersion:info.version});
let app,page;const start=performance.now();
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),args:[],env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:path.join(stage,'payload/backplane-serve'),BACKPLANE_WEB:path.join(stage,'payload/web'),BACKPLANE_SERVICE_VERSION:info.version}});page=await app.firstWindow();
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count()){await skip.click();await page.locator('.tour-back').waitFor({state:'hidden'});}
 const older=page.locator('[data-on-click="side-older"]');if(await older.count())await older.click();
 await page.waitForFunction(()=>document.querySelectorAll('.sidebar section.project[data-drag-to^="p"]').length===42);
 assert.equal(await page.locator('.project-head[data-value="p41"]').count(),0);
 assert.equal(await page.locator('[data-on-click="select"][data-value="t40"]').count(),0,'archived thread stays folded');
 assert.equal(await page.locator('.project-head[data-value="p42"]').count(),1,'empty project retained');
 const select=async id=>{await page.locator(`[data-on-click="select"][data-value="${id}"]`).first().click();await page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',id);};
 await select('t0');
 const msg=page.locator('[data-id="sent-images"]');await msg.locator('.shot').first().scrollIntoViewIfNeeded();
 await page.waitForFunction(()=>{const imgs=[...document.querySelectorAll('[data-id="sent-images"] .shot img')];return imgs.length===2&&imgs.every(i=>i.complete&&i.naturalWidth===64);});
 assert((await msg.innerText()).includes('notes.txt'));assert.equal(await msg.locator('.shot').count(),2);
 assert(!(await page.locator('main').innerText()).includes('OTHER_BODY_MUST_NOT_APPEAR_IN_T0'));
 await page.screenshot({path:path.join(out,'sent-images-desktop.png')});
 await msg.locator('.shot').first().click();await page.waitForFunction(()=>document.querySelector('.lightbox .lb-img')?.naturalWidth===64);await page.screenshot({path:path.join(out,'image-lightbox.png')});await page.keyboard.press('Escape');
 await select('t30');await select('t0');
 await page.setViewportSize({width:390,height:844});await msg.locator('.shot').first().scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,'sent-images-mobile.png')});
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no mobile horizontal overflow');
 await page.locator('.thread-head .back').click();
 await page.waitForFunction(()=>!document.querySelector('.app').classList.contains('picked'));
 const logo=await page.locator('.brand-mark').boundingBox(),title=await page.locator('.brand .name').boundingBox();
 assert(logo&&title&&logo.width===24&&logo.x<title.x,'mobile sidebar keeps one24px logo before title');
 assert.equal(await page.locator('.brand-mark').count(),1);
 await page.locator('input#proj-find').fill('Catalog project 30');
 await page.waitForFunction(()=>document.querySelectorAll('.sidebar section.project').length===1);
 await page.screenshot({path:path.join(out,'mobile-logo-search.png')});
 const result={state:'passed',seconds:(performance.now()-start)/1000,version:info.version,projects:42,images:2,lightbox:true,archivedHidden:true,removedHidden:true,emptyProject:true,out};fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}catch(e){console.error(e);process.exitCode=1;if(page){fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>''));await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});}}finally{if(app)await app.close();service.stop();}})();
