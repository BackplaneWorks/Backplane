'use strict';
// Mixed-version integration: an explicitly supplied older packaged shell,
// current renderer module, real private Browser channel and an isolated hub.
// Never launches a provider or changes the installed shell/profile.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require(process.env.BACKPLANE_PLAYWRIGHT_MODULE||'playwright-core');
const {Service}=require('../service.cjs');
const legacy=path.resolve(process.env.BACKPLANE_LEGACY_RUNTIME),binary=path.resolve(process.env.BACKPLANE_BROWSER_BINARY);
const root=path.resolve(__dirname,'../..'),out=path.resolve(process.env.BACKPLANE_BROWSER_OUT||'build/browser-legacy-recovery');
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-browser-legacy-')),web=home+'/web',project=home+'/project',agents=home+'/agents';
for(const dir of [out,web,project,agents])fs.mkdirSync(dir,{recursive:true});
fs.copyFileSync(process.env.BACKPLANE_BROWSER_RENDERER||root+'/src/web/desktop-browser.js',web+'/desktop-browser.js');
fs.writeFileSync(web+'/index.html',`<!doctype html><meta charset="utf-8"><title>Mixed-version Browser fixture</title>
<textarea id="draft">Retained unsent fixture draft</textarea>
<div data-desktop-browser-owner="self" style="height:300px"><span class="desktop-browser-status">Connecting Browser…</span></div>
<script type="module" src="./fixture.js"></script>`);
fs.writeFileSync(web+'/fixture.js',`import {installDesktopBrowser} from './desktop-browser.js';
window.fixtureStates=[];window.backplaneDesktop.browser.onState(value=>window.fixtureStates.push({at:Date.now(),...value}));
window.fixtureBrowser=installDesktopBrowser({bridge:window.backplaneDesktop.browser});`);
const at=Math.floor(Date.now()/1000),events=[
 {$:'ProjectCreated',id:'p',title:'Isolated browser compatibility test',root:project,at},
 {$:'ThreadCreated',id:'self',project:'p',title:'Legacy recovery',env:'local',branch:'',worktree:project,provider:'codex',model:'',at}
];
for(const [key,value]of [['onboard.done','1'],['restart.continue','off'],['mail.on','off'],['telemetry.share','off']])events.push({$:'SettingSet',key,value,at});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n');
const env={...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_DESKTOP_BROWSER:'1'};
Object.assign(process.env,env);
const service=new Service({home,binary,web});
let app,page,step='start';const checks=[],errors=[];
async function stopOwned(){
 const child=service.child;if(!child||child.exitCode!==null){service.stop();return;}
 const exited=new Promise(resolve=>child.once('exit',resolve));service.stop();await exited;
}
function stageLegacy(version){
 const staged=home+'/runtime';fs.mkdirSync(staged);
 for(const file of fs.readdirSync(legacy)){
  if(file==='resources')continue;
  const src=path.join(legacy,file),dest=path.join(staged,file);
  if(file==='backplane-desktop'||file==='electron')fs.copyFileSync(src,dest,fs.constants.COPYFILE_FICLONE);
  else fs.symlinkSync(src,dest);
 }
 const resources=staged+'/resources';fs.mkdirSync(resources);
 for(const file of fs.readdirSync(legacy+'/resources')){
  if(file==='DESKTOP-BUILD.json')continue;
  // Use the older shell's exact sources, preload and dependencies.
  fs.symlinkSync(legacy+'/resources/'+file,resources+'/'+file);
 }
 const build=JSON.parse(fs.readFileSync(legacy+'/resources/DESKTOP-BUILD.json'));
 // Startup pin follows the isolated test hub. The older Browser controller
 // and its manual reconnect interface are deliberately left intact.
 build.serviceVersion=version;fs.writeFileSync(resources+'/DESKTOP-BUILD.json',JSON.stringify(build));
 return staged+'/'+(fs.existsSync(staged+'/backplane-desktop')?'backplane-desktop':'electron');
}
(async()=>{try{
 const connection=await service.start();
 app=await _electron.launch({executablePath:stageLegacy(connection.version),args:['--ozone-platform=headless','--disable-gpu'],env:{...env,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web}});
 page=await app.firstWindow();page.setDefaultTimeout(10000);page.on('pageerror',error=>errors.push(error.message));
 await page.waitForURL(connection.origin+'/');
 step='initial-ready';await page.waitForFunction(async()=>typeof window.backplaneDesktop?.browser?.reconnect==='function'&&(await window.backplaneDesktop.browser.state()).phase==='ready');
 await page.waitForFunction(()=>!!window.fixtureBrowser);checks.push('actual older packaged shell exposes its manual reconnect bridge');
 const presented=()=>page.waitForFunction(()=>/open a page|enter an address/i.test(document.querySelector('.desktop-browser-status')?.textContent||''));
 await presented();
 await app.evaluate(({app})=>{
  const load=process.getBuiltinModule('module').createRequire(app.getAppPath()+'/main.cjs'),proto=load('ws').prototype;
  const send=proto.send,emit=proto.emit;globalThis.fixtureFrames=[];
  // Observe operation names and bounded refusals only. Forward every frame
  // unchanged; credentials, connection identities and page contents stay out.
  const record=(socket,direction,data)=>{try{
   if(new URL(socket.url).pathname!=='/desktop/browser/v1')return;
   const m=JSON.parse(data.toString()),p=m.payload||{};
   globalThis.fixtureFrames.push({at:Date.now(),direction,op:m.op,error:typeof p.error==='string'?p.error.slice(0,240):undefined});
   if(globalThis.fixtureFrames.length>100)globalThis.fixtureFrames.shift();
  }catch{}};
  proto.send=function(data,...args){record(this,'send',data);return send.call(this,data,...args);};
  proto.emit=function(name,...args){if(name==='message')record(this,'receive',args[0]);return emit.call(this,name,...args);};
 });
 const before=await page.evaluate(()=>window.backplaneDesktop.version());
 step='stop-hub';await stopOwned();await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase==='error');
 step='restart-hub';const replacement=await service.start();assert.equal(replacement.origin,connection.origin);
 await page.waitForFunction(async()=>(await window.backplaneDesktop.browser.state()).phase==='ready',null,{timeout:Number(process.env.BACKPLANE_BROWSER_RECOVERY_TIMEOUT||30000)});
 await presented();
 checks.push('new renderer automatically reconnects the old shell after a real hub restart');
 assert.equal((await page.evaluate(()=>window.backplaneDesktop.version())).launchedAtMs,before.launchedAtMs);
 assert.equal(await page.locator('#draft').inputValue(),'Retained unsent fixture draft');
 checks.push('same desktop process and renderer retain the unsent draft');
 const response=await fetch(connection.origin+'/mcp/self',{method:'POST',headers:{authorization:'Bearer '+await service.token(),'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'preview_status',arguments:{}}})}).then(r=>r.json());
 assert.equal(response.result?.isError,false,JSON.stringify(response));checks.push('preview_status works through the recovered real private channel');
 const logged=fs.readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(JSON.parse);
 assert.equal(logged.filter(e=>e.$==='MessagePosted').length,0);checks.push('no provider turn or user message replayed');
 fs.writeFileSync(out+'/result.json',JSON.stringify({result:'PASS',checks,home,legacyRuntime:legacy,serviceVersion:connection.version,scope:'Actual older Electron shell and real hub/private channel with a minimal current-module renderer fixture; no production timeline UI or native compositing assertion'},null,2)+'\n');
 console.log(checks.join('\n'));
}catch(error){
 const states=await page?.evaluate(async()=>({events:window.fixtureStates,state:await window.backplaneDesktop?.browser?.state()})).catch(()=>null);
 const frames=await app?.evaluate(()=>globalThis.fixtureFrames).catch(()=>null);
 fs.writeFileSync(out+'/result.json',JSON.stringify({result:'FAIL',step,checks,home,errors,url:page?.url(),states,frames,error:error.stack},null,2)+'\n');console.error(error.stack);process.exitCode=1;
}finally{await app?.close();await stopOwned();}})();
