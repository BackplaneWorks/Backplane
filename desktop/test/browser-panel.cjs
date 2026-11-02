'use strict';
// Actual Electron + temp-home service. No product profile or provider runs.
// BACKPLANE_BROWSER_BINARY / WEB / ELECTRON name the exact built candidate.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const binary=path.resolve(process.env.BACKPLANE_BROWSER_BINARY),web=path.resolve(process.env.BACKPLANE_BROWSER_WEB),executablePath=path.resolve(process.env.BACKPLANE_BROWSER_ELECTRON);
const out=path.resolve(process.env.BACKPLANE_BROWSER_OUT||'build/browser-panel');fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-browser-panel-')),project=home+'/project',agents=home+'/agents';fs.mkdirSync(project);fs.mkdirSync(agents);
const at=Math.floor(Date.now()/1000),events=[{$:'ProjectCreated',id:'fixture',title:'Browser review',root:project,at},{$:'ThreadCreated',id:'self',project:'fixture',title:'Browser controls',env:'local',branch:'',worktree:project,provider:'codex',model:'',at}];
for(const key of ['restart.continue','mail.on','telemetry.share'])events.push({$:'SettingSet',key,value:'off',at});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const service=new Service({home,binary,web});
const site=http.createServer((req,res)=>{res.setHeader('Content-Type','text/html');res.setHeader('X-Frame-Options','DENY');res.end('<title>Browser connected</title><h1>Browser connected</h1><label>Local entry <input id="entry"></label>');});
let app,page;const checks=[],errors=[];
async function capture(name){const data=await page.evaluate(()=>window.backplaneDesktop.captureApp());fs.writeFileSync(path.join(out,name+'.png'),Buffer.from(data.split(',')[1],'base64'));}
(async()=>{try{
 await new Promise(resolve=>site.listen(0,'127.0.0.1',resolve));const url=`http://127.0.0.1:${site.address().port}`;
 const connection=await service.start();
 app=await _electron.launch({executablePath,args:process.env.BACKPLANE_BROWSER_HEADLESS==='1'?['--ozone-platform=headless']:[],env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web}});
 page=await app.firstWindow();await page.setViewportSize({width:1440,height:960});page.setDefaultTimeout(10000);page.on('pageerror',e=>errors.push(e.message));
 await page.locator('.sidebar').waitFor();const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();
 await page.locator('.sidebar [data-on-click="select"][data-value="self"]').first().click();
 await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='self'&&document.documentElement.dataset.backplaneRemoteReady==='true');
 await page.locator('.workspace-bar [data-on-click="view-kind"][data-value="browser"]').click();
 await page.locator('[data-desktop-browser-owner="self"]').waitFor();
 await page.waitForFunction(()=>{const s=document.querySelector('.desktop-browser-status');return s&&!s.textContent.includes('Connecting')&&s.textContent.length>0;});
 assert.equal((await page.evaluate(()=>window.backplaneDesktop.browser.state())).phase,'ready');checks.push('connected empty page offers address hint despite retained hidden picker');
 const take=page.locator('.desktop-browser-drive [data-on-click="tk-takeover"]');assert(await take.isVisible());
 assert(await take.evaluate(el=>el.getBoundingClientRect().width<220));checks.push('compact takeover control');
 await capture('empty');
 let id=0;async function tool(name,args={}){const response=await fetch(connection.origin+'/mcp/self',{method:'POST',headers:{authorization:'Bearer '+await service.token(),'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:++id,method:'tools/call',params:{name,arguments:args}})});assert(response.ok);const result=(await response.json()).result;assert.equal(result.isError,false,JSON.stringify(result));return result;}
 await tool('preview_open',{url});
 const shown=()=>app.evaluate(({BrowserWindow},url)=>BrowserWindow.getAllWindows()[0].contentView.children.filter(v=>v.webContents&&!v.webContents.isDestroyed()&&v.webContents.getURL().startsWith(url)&&v.getVisible()).length,url);
 await page.waitForFunction(()=>document.querySelector('.desktop-browser-status')?.textContent==='');assert.equal(await shown(),1);checks.push('preview_open presents actual native page');
 await page.locator('.model-chip').click();await page.locator('.model-menu:visible').waitFor();assert.equal(await shown(),0);await page.keyboard.press('Escape');
 await page.locator('.model-menu:visible').waitFor({state:'hidden'});
 await page.waitForFunction(()=>document.querySelector('.desktop-browser-status')?.textContent==='');assert.equal(await shown(),1);checks.push('visible picker hides guest; dismissing restores it');
 await take.click();await page.locator('.desktop-browser-drive [data-on-click="tk-handback"]').waitFor();checks.push('takeover grants person control');
 await capture('connected');
 await page.locator('.desktop-browser-drive [data-on-click="tk-handback"]').click();await page.locator('.desktop-browser-drive [data-on-click="tk-takeover"]').waitFor();
 await tool('preview_evaluate',{expression:'document.title'});checks.push('handback restores agent access to same page');
 await page.setViewportSize({width:390,height:844});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await capture('mobile');checks.push('390px browser toolbar has no document overflow');
 assert.deepEqual(errors,[]);fs.writeFileSync(path.join(out,'result.json'),JSON.stringify({state:'passed',checks,home,binary,web,errors},null,2));console.log(checks.join('\n'));
 }catch(error){if(page)await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});fs.writeFileSync(path.join(out,'result.json'),JSON.stringify({state:'failed',error:error.stack,checks,errors},null,2));console.error(error);process.exitCode=1;}
 finally{if(app)await app.close();service.stop();await new Promise(resolve=>site.close(resolve));}
})();
