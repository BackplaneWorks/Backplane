// Linux native child identity is captured from the launched Electron process's
// own kernel child list, never from hub.lock. Windows requires its owner gate.
const {_electron}=require('playwright-core');const fs=require('node:fs');const os=require('node:os');const path=require('node:path');const assert=require('node:assert/strict');
if(process.platform!=='linux')throw new Error('This explicit Linux persistence gate does not qualify Windows shutdown');
const root=path.resolve(__dirname,'../..'),home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-electron-persist-'));
const artifact=process.env.BACKPLANE_SERVICE,web=process.env.BACKPLANE_WEB||path.join(root,'build/electron-web');
const fake=path.join(home,'agents');fs.mkdirSync(fake);for(const name of ['claude','codex','grok','muse','agy'])fs.writeFileSync(path.join(fake,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
fs.writeFileSync(path.join(home,'events.jsonl'),[
 {$:'ProjectCreated',id:'p',title:'Persistence fixture',root:home,at:1},
 {$:'ThreadCreated',id:'persist-thread',project:'p',title:'Background work',env:'local',branch:'',worktree:home,provider:'claude',model:'sonnet',at:1},
 {$:'SettingSet',key:'restart.continue',value:'off',at:1},{$:'SettingSet',key:'onboard.done',value:'1',at:1}
].map(JSON.stringify).join('\n')+'\n');
let app,nativePid,page;
const phase=name=>console.log("persistence: "+name);
const out=path.join(root,'build/electron-evidence');fs.mkdirSync(out,{recursive:true});
const quote=value=>"'"+value.replaceAll("'","'\\''")+"'";
(async()=>{
 try{
  phase("launch UI");
  app=await _electron.launch({executablePath:require('electron'),args:[path.join(root,'desktop')],env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:artifact,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:'0.11.2-dev.mail-45328b78',BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_GOOGLE_BAKE:'0',BACKPLANE_LOCK_DIR:path.join(home,'locks'),DISPLAY:process.env.DISPLAY||':0'},timeout:15000});
  page=await app.firstWindow({timeout:10000});
  page.on('pageerror',error=>console.log('persistence renderer error: '+error.message));
  // Capture the native child independently of UI readiness so failed startup
  // still has an exact, fixture-scoped cleanup target.
  phase('capture spawned service');
  const parent=app.process().pid;
  const deadline=Date.now()+5000;
  while(!nativePid&&Date.now()<deadline){
    const ids=fs.readFileSync(`/proc/${parent}/task/${parent}/children`,'utf8').trim().split(/\s+/);
    for(const id of ids){try{const exe=fs.readlinkSync(`/proc/${id}/exe`),args=fs.readFileSync(`/proc/${id}/cmdline`,'utf8').split('\0');if(exe===fs.realpathSync(artifact)&&args.includes(home))nativePid=+id;}catch{}}
    if(!nativePid)await new Promise(resolve=>setTimeout(resolve,50));
  }
  assert(nativePid,'captured spawned native service PID from parent kernel child list');
  phase('wait for canonical UI');
  await page.waitForFunction(()=>document.documentElement.dataset.backplaneReady==='true',null,{timeout:15000});
  const origin=new URL(page.url()).origin,done=path.join(home,'background-finished.txt');
  const command=quote(process.execPath)+' -e '+quote(`setTimeout(()=>require('fs').writeFileSync(${JSON.stringify(done)},'finished after UI close'),2500)`);
  const token=fs.readFileSync(path.join(home,'token'),'utf8').trim();
  phase('launch service-owned work');
  const response=await fetch(origin+'/mcp/persist-thread',{signal:AbortSignal.timeout(5000),method:'POST',headers:{'content-type':'application/json',accept:'application/json',authorization:`Bearer ${token}`},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'work_run',arguments:{command,note:'private persistence fixture',timeoutSec:30}}})});
  const reply=await response.json();assert(response.ok&&!reply.result?.isError,JSON.stringify(reply));
  phase('close owning window');
  const uiProcess=app.process();
  const nativeExit=new Promise(resolve=>uiProcess.once('exit',(code,signal)=>resolve({code,signal})));
  await page.close();
  const uiExit=await Promise.race([nativeExit,new Promise((_,reject)=>setTimeout(()=>reject(new Error('UI native process exit timed out; exitCode='+uiProcess.exitCode)),5000))]);
  assert.equal(uiExit.code,0,'ordinary window close exits UI process cleanly');
  app=null;
  await new Promise((resolve,reject)=>{if(fs.existsSync(done))return resolve();const watcher=fs.watch(home,()=>{if(fs.existsSync(done)){clearTimeout(timer);watcher.close();resolve();}});const timer=setTimeout(()=>{watcher.close();reject(new Error('service-owned work did not finish after UI close'));},10000);});
  phase('verify service remains available');
  assert((await fetch(origin+'/hello',{signal:AbortSignal.timeout(2000)})).ok,'service remains available after owning UI exits');
  const receipt={status:'passed',home,nativePid,serviceSurvivedOwningUiExit:true,uiExit,backgroundWorkAfterClose:fs.readFileSync(done,'utf8'),shutdownScope:'explicit cleanup of captured Linux native PID only; no Windows claim'};
  fs.writeFileSync(path.join(out,'persistence.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
 }finally{
  if(app){await Promise.race([app.close().catch(()=>{}),new Promise(resolve=>setTimeout(resolve,3000))]);if(app.process().exitCode===null)app.process().kill('SIGTERM');}
  if(nativePid){try{const exe=fs.readlinkSync(`/proc/${nativePid}/exe`),args=fs.readFileSync(`/proc/${nativePid}/cmdline`,'utf8').split('\0');if(exe===fs.realpathSync(artifact)&&args.includes(home))process.kill(nativePid,'SIGTERM');}catch{}}
 }
})().catch(error=>{console.error(error);fs.writeFileSync(path.join(out,'persistence-failure.json'),JSON.stringify({error:error.stack,home,nativePid},null,2));process.exitCode=1;});
