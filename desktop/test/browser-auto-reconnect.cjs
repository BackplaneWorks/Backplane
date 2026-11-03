'use strict';
// Actual isolated Electron/private Browser plus a hub stopped/restarted in place.
// Never launches a provider or touches the installed application home.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),http=require('node:http'),assert=require('node:assert/strict'),{createHash}=require('node:crypto');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const root=path.resolve(__dirname,'../..'),out=path.resolve(process.env.BROWSER_AUTO_OUT||'build/ui-removals/native');fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-browser-auto-')),agents=home+'/agents',project=home+'/project';fs.mkdirSync(agents);fs.mkdirSync(project);
const binary=path.resolve('build/ui-removals/backplane-serve'),web=path.resolve('build/ui-removals/web');
const at=Math.floor(Date.now()/1000),events=[{$:'ProjectCreated',id:'p',title:'Browser reconnect fixture',root:project,at},{$:'ThreadCreated',id:'self',project:'p',title:'Automatic browser recovery',env:'local',branch:'',worktree:project,provider:'codex',model:'',at}];
for(const [key,value]of [['onboard.done','1'],['restart.continue','off'],['mail.on','off'],['telemetry.share','off']])events.push({$:'SettingSet',key,value,at});fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};Object.assign(process.env,env);
const service=new Service({home,binary,web,expectedVersion:'0.13.1-dev.ui-removals'});
const site=http.createServer((req,res)=>{res.setHeader('Content-Type','text/html');res.end('<title>Retained browser URL</title><h1>Retained browser URL</h1><input id="retained" value="fixture">');});
let app,page,step='start';const checks=[],wireTrace=[];
async function stopOwned(){const child=service.child;if(!child||child.exitCode!==null){service.stop();return;}const exited=new Promise(resolve=>child.once('exit',resolve));service.stop();await exited;}
function stageRuntime(){
 const runtime=process.env.BROWSER_AUTO_RUNTIME||'/home/h/.local/share/backplane/desktop/runtime',staged=home+'/runtime';fs.mkdirSync(staged);
 for(const file of fs.readdirSync(runtime)){if(file==='resources')continue;const source=path.join(runtime,file),dest=path.join(staged,file==='electron'?'backplane-desktop':file);if(['electron','backplane-desktop'].includes(file))fs.copyFileSync(source,dest,fs.constants.COPYFILE_FICLONE);else fs.symlinkSync(source,dest);}
 const resources=staged+'/resources',appRoot=resources+'/app';fs.mkdirSync(appRoot,{recursive:true});
 for(const file of fs.readdirSync(runtime+'/resources'))if(!['app','DESKTOP-BUILD.json'].includes(file))fs.symlinkSync(runtime+'/resources/'+file,resources+'/'+file);
 for(const file of require('../prepare-components.cjs').appFiles(root+'/desktop')){const dest=appRoot+'/'+file;fs.mkdirSync(path.dirname(dest),{recursive:true});fs.copyFileSync(root+'/desktop/'+file,dest);}
 fs.mkdirSync(appRoot+'/node_modules');fs.symlinkSync(root+'/desktop/node_modules/ws',appRoot+'/node_modules/ws');
 const sha=f=>createHash('sha256').update(fs.readFileSync(root+'/desktop/'+f)).digest('hex');
 fs.writeFileSync(resources+'/DESKTOP-BUILD.json',JSON.stringify({serviceVersion:'0.13.1-dev.ui-removals',lifecyclePolicy:{mode:'resident-service'},desktopBrowser:{protocol:1,rendererBridge:true,guestManagerSha256:sha('browser-guest.cjs'),preloadSha256:sha('browser-recording-preload.cjs')}}));
 return staged+'/backplane-desktop';
}
async function guest(url){return app.evaluate(async({webContents},url)=>{
 const find=()=>webContents.getAllWebContents().find(w=>!w.isDestroyed()&&w.getURL()===url);
 const current=find();if(current)return current.id;
 return new Promise((resolve,reject)=>{const tick=setInterval(()=>{const w=find();if(w){clearInterval(tick);clearTimeout(timer);resolve(w.id);}},50);const timer=setTimeout(()=>{clearInterval(tick);reject(Error('Native guest URL restoration deadline'));},30000);});
},url);}
async function capture(name){const data=await page.evaluate(()=>window.backplaneDesktop.captureApp());fs.writeFileSync(out+'/'+name+'.png',Buffer.from(data.split(',')[1],'base64'));}
(async()=>{try{
 await new Promise(resolve=>site.listen(0,'127.0.0.1',resolve));const url=`http://127.0.0.1:${site.address().port}/retained`;
 const connection=await service.start();assert.equal(connection.version,'0.13.1-dev.ui-removals');
 app=await _electron.launch({executablePath:stageRuntime(),args:['--ozone-platform=headless','--disable-gpu'],env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web}});
 page=await app.firstWindow();
 const Wire=await import('../../src/web/wire.js');const cb=fs.readFileSync(root+'/src/core/cbor.bend','utf8');const dict=name=>JSON.parse(cb.split('def Cbor.'+name+'() -> List<&2, String>:')[1].split('\n\n')[0].trim());const keys=dict('keys'),words=dict('words');
 page.on('websocket',ws=>{for(const direction of ['framesent','framereceived'])ws.on(direction,frame=>{try{const value=typeof frame.payload==='string'?JSON.parse(frame.payload):Wire.plain(Wire.cbor(new Uint8Array(frame.payload),keys,words));if(value?.m==='bots.browser'||value?.t==='reply'&&value.id>=2**44&&value.id<2**45)wireTrace.push({direction,value});}catch{}});});
 page.setDefaultTimeout(20000);await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].setContentBounds({x:0,y:0,width:1440,height:960}));
 step='select';await page.locator('.sidebar').waitFor();const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.isVisible())await skip.click();await page.locator('.sidebar [data-on-click="select"][data-value="self"]').first().click();await page.waitForFunction(()=>document.querySelector('main[data-thread]')?.dataset.thread==='self'&&document.documentElement.dataset.backplaneRemoteReady==='true');
 await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase==='ready');
 assert.equal(await page.locator('[data-desktop-browser-reconnect]').count(),0);assert.equal(await page.evaluate(()=>window.backplaneDesktop.browser.reconnect),undefined);
 step='open-browser';await page.locator('.workspace-bar [data-on-click="view-kind"][data-value="browser"]:visible').first().click();await page.locator('[data-desktop-browser-owner="self"]').waitFor();
 const result=await fetch(connection.origin+'/mcp/self',{method:'POST',headers:{authorization:'Bearer '+await service.token(),'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'preview_open',arguments:{url}}})}).then(r=>r.json());assert.equal(result.result?.isError,false,JSON.stringify(result));
 await guest(url);checks.push('actual native guest URL opened');
 await page.locator('#composer').fill('Retained unsent draft');await capture('connected');
 step='hub-stop';await stopOwned();await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase!=='ready');await capture('disconnected');
 step='hub-restart';const replacement=await service.start();assert.equal(replacement.origin,connection.origin);await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase==='ready',null,{timeout:45000});checks.push('private channel reconnected automatically after hub restart');
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');await guest(url);assert.equal(await page.locator('#composer').inputValue(),'Retained unsent draft');checks.push('active URL, selected thread and unsent draft restored without a click');await capture('reconnected');
 step='renderer-reload';const reconnectedGuest=await guest(url);await page.reload();await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase==='ready');assert.equal(await guest(url),reconnectedGuest);checks.push('renderer reload keeps the same native guest');
 const logged=fs.readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(JSON.parse);assert.equal(logged.filter(x=>x.$==='MessagePosted').length,0);checks.push('no provider turn or user message replayed');
 fs.writeFileSync(out+'/result.json',JSON.stringify({result:'PASS',checks,isolatedHome:home,scope:'Actual headless Electron guest URL, hub restart and renderer reload; native view compositing is not asserted in headless mode; no installed-user profile or provider'},null,2)+'\n');console.log('ok '+checks.join('; '));
}catch(error){fs.writeFileSync(out+'/failure.json',JSON.stringify({step,checks,wireTrace,error:error.stack,status:await page?.locator('.desktop-browser-status').textContent().catch(()=>''),controller:await page?.evaluate(()=>window.backplaneDesktop?.browser?.state()).catch(()=>null)},null,2)+'\n');if(page){await page.screenshot({path:out+'/failure.png'}).catch(()=>{});fs.writeFileSync(out+'/failure.html',await page.content().catch(()=>''));}console.error(error.stack);process.exitCode=1;}
finally{await app?.close();await stopOwned();await new Promise(resolve=>site.close(resolve));}})();
