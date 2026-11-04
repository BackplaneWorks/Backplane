'use strict';
// Invoked only by the disposable hosted Windows runner's created local user.
// The packaged app starts its own hub from an EMPTY home; no service prestart,
// Electron replacement, source overlay, admin token or --no-sandbox.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {execFileSync,spawnSync}=require('node:child_process');
const {createHash}=require('node:crypto');
const out=path.resolve(process.argv[2]),executable=process.argv[3],version=process.argv[4];
fs.mkdirSync(out,{recursive:true});
const report={status:'INCOMPLETE',platform:process.platform,checks:[],sourceCommit:process.env.GITHUB_SHA};
const save=()=>fs.writeFileSync(path.join(out,'result.json'),JSON.stringify(report,null,2)+'\n');
const phase=name=>{report.phase=name;save();};
async function bounded(promise,label,ms){
  let timer;
  try{return await Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Timed out: '+label)),ms);})]);}
  finally{clearTimeout(timer);}
}
const alive=child=>child.exitCode===null&&child.signalCode===null;
async function closeOwnedUi({app,child,label}){
  const result={label,pid:child.pid,gracefulClose:false,forcedTermination:false};
  try{await bounded(app.close(),label+' Playwright close',5000);result.gracefulClose=true;}
  catch(error){result.adapterCloseDiagnostic=error.message;}
  if(alive(child)){
    // Cleanup of this test's captured UI handle only. Graceful shutdown is
    // recorded, not a startup/error-UI qualification claim.
    result.forcedTermination=true;child.kill('SIGTERM');
    if(alive(child))await bounded(new Promise(resolve=>child.once('exit',resolve)),label+' captured UI exit',5000);
  }
  result.closed=!alive(child);result.exitCode=child.exitCode;result.signal=child.signalCode;
  assert(result.closed,'Captured test UI still alive');return result;
}
const shell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
async function main(){
  assert.equal(process.platform,'win32');
  report.identity=JSON.parse(execFileSync(shell,['-NoProfile','-NonInteractive','-Command',
    "$i=[Security.Principal.WindowsIdentity]::GetCurrent(); $p=[Security.Principal.WindowsPrincipal]::new($i); $d=Get-ItemPropertyValue 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock' AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue; $profile=Get-ItemPropertyValue ('HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList\\'+$i.User.Value) ProfileImagePath; @{name=$i.Name;sid=$i.User.Value;profile=$profile;admin=$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);developerMode=($d -eq 1)} | ConvertTo-Json -Compress"
  ],{encoding:'utf8'}));
  assert.equal(report.identity.admin,false,'Smoke must run with a standard user token');
  assert.equal(report.identity.developerMode,false,'Developer Mode must be OFF');
  // CreateProcessWithLogonW loads the new profile but inherits the caller's
  // environment. Resolve this account's actual registered profile and give
  // it a writable TEMP, instead of reaching into the runner admin's home.
  assert(path.isAbsolute(report.identity.profile));
  Object.assign(process.env,{USERPROFILE:report.identity.profile,HOME:report.identity.profile,
    APPDATA:path.join(report.identity.profile,'AppData','Roaming'),LOCALAPPDATA:path.join(report.identity.profile,'AppData','Local'),
    TEMP:path.join(out,'temp'),TMP:path.join(out,'temp')});
  fs.mkdirSync(process.env.TEMP,{recursive:true});
  report.privileges=execFileSync('whoami.exe',['/priv'],{encoding:'utf8'});
  assert(!report.privileges.includes('SeCreateSymbolicLinkPrivilege'),'User must not hold symlink privilege');
  const source=path.join(out,'symlink-source');fs.writeFileSync(source,'negative control');
  let denied;
  try{fs.symlinkSync(source,path.join(out,'forbidden-symlink'),'file');}catch(error){denied=error.code;}
  assert(['EPERM','EACCES'].includes(denied),'Negative control: real file symlink must fail');
  report.symlinkNegativeControl=denied;report.checks.push('standard account, Developer Mode OFF, no symlink privilege, actual symlink denied');save();
  const units=spawnSync(process.execPath,['--test',...['store-link','service-startup-error','startup-recovery','load-recovery','windows-service-launch','service-recovery'].map(n=>path.join(__dirname,n+'.test.cjs'))],{encoding:'utf8',timeout:60000});
  fs.writeFileSync(path.join(out,'unit-tests.txt'),(units.stdout||'')+(units.stderr||''));
  assert.equal(units.status,0,'Standard-user regression tests failed');report.checks.push('production store effect and desktop error/recovery regressions');save();
  if(!executable){report.status='PASS';report.scope='Standard-user production-effect and desktop startup tests; packaged launch requires package input';return;}
  assert(path.isAbsolute(executable));assert(version);
  report.package=JSON.parse(fs.readFileSync(path.join(out,'package.json'),'utf8'));
  const build=JSON.parse(fs.readFileSync(path.join(path.dirname(executable),'resources','DESKTOP-BUILD.json'),'utf8'));
  assert.equal(build.serviceVersion,version);
  report.productSourceCommit=build.sourceCommit;report.productSourceTree=build.sourceTree;
  report.packageExe={path:executable,sha256:createHash('sha256').update(fs.readFileSync(executable)).digest('hex')};
  const {_electron}=require('playwright-core');
  const env={...process.env};
  for(const k of Object.keys(env))if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL/.test(k)||['BACKPLANE_SERVICE','BACKPLANE_WEB','BACKPLANE_SERVICE_VERSION','ELECTRON_RUN_AS_NODE','NODE_OPTIONS'].includes(k))delete env[k];
  const home=path.join(out,'Fresh home 日本語');assert(!fs.existsSync(home));
  const agents=path.join(out,'blocked-agents');fs.mkdirSync(agents);
  Object.assign(env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',HOME:process.env.USERPROFILE});
  let app,page;const owned=[];
  report.cleanup=[];
  try{
    phase('cold-launch');
    app=await _electron.launch({executablePath:executable,args:[],env,chromiumSandbox:true,timeout:60000});
    owned.push({app,child:app.process(),label:'cold-start UI'});
    page=await app.firstWindow();
    await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:90000});
    const url=new URL(page.url());assert.equal(url.hostname,'127.0.0.1');assert.equal(url.protocol,'http:');
    report.versions=await page.evaluate(()=>window.backplaneDesktop.version());assert.equal(report.versions.service,version);
    const {Service}=require('../service.cjs');
    const service=new Service({home,expectedVersion:version});const connected=await service.discover();assert.equal(connected.origin,url.origin);
    const db=path.join(home,'store.db');assert(fs.statSync(db).size>0,'Cold startup must build store.db');
    const cookies=await app.evaluate(async({BrowserWindow})=>{
      const w=BrowserWindow.getAllWindows()[0],p=w.webContents.getLastWebPreferences();
      const cookies=await w.webContents.session.cookies.get({url:w.webContents.getURL(),name:'bp_token'});
      return{count:cookies.length,httpOnly:cookies[0]?.httpOnly,prefs:{sandbox:p.sandbox,nodeIntegration:p.nodeIntegration,contextIsolation:p.contextIsolation}};
    });
    assert.equal(cookies.count,1);assert.equal(cookies.httpOnly,true);assert.deepEqual(cookies.prefs,{sandbox:true,nodeIntegration:false,contextIsolation:true});
    report.window={origin:url.origin,...cookies};await page.screenshot({path:path.join(out,'main-ui.png')});
    report.checks.push('exact packaged EXE starts its own hub from empty Unicode home, builds store, authenticates, reaches sandboxed main UI');save();
    // Exercise the error UI before teardown. A resident service's handles or
    // a hung adapter close must not strand the second, independent home.
    phase('failure-launch');
    const failedEnv={...env,BACKPLANE_HOME:path.join(out,'Failure home'),BACKPLANE_SERVICE:path.join(out,'missing-service.exe')};
    app=await _electron.launch({executablePath:executable,args:[],env:failedEnv,chromiumSandbox:true,timeout:60000});
    owned.push({app,child:app.process(),label:'failure UI'});
    page=await app.firstWindow();await page.locator('#retry').waitFor({state:'visible',timeout:15000});
    const message=await page.locator('#status').innerText();assert.match(message,/ENOENT/);
    assert.equal((await page.evaluate(()=>backplaneDesktop.connectionState())).phase,'error');
    phase('failure-visible');
    await page.waitForTimeout(7000);assert.equal(await page.locator('#status').innerText(),message);assert(await page.locator('#retry').isVisible());
    assert.equal((await page.evaluate(()=>backplaneDesktop.connectionState())).phase,'error');
    await page.screenshot({path:path.join(out,'startup-error.png')});
    report.failureUi={phase:'error',nativeError:'ENOENT',stableForMs:7000,retryVisible:true};
    report.checks.push('packaged missing-service error stays visible with Retry after seven seconds');save();
    await page.locator('#retry').click();
    await page.locator('#retry').waitFor({state:'visible',timeout:15000});
    await page.waitForFunction(()=>backplaneDesktop.connectionState()?.phase==='error',null,{timeout:15000});
    assert.match(await page.locator('#status').innerText(),/ENOENT/);assert(await page.locator('#retry').isVisible());
    report.failureUi.explicitRetryReturnsVisibleError=true;save();
  }finally{
    phase('test-owned-ui-cleanup');
    for(const entry of owned.reverse()){
      const result=await closeOwnedUi(entry);report.cleanup.push(result);save();
    }
  }
  report.status='PASS';report.phase='done';report.scope='Exact packaged desktop cold startup, own service/store, authenticated sandboxed main UI, persistent native failure UI and explicit Retry as standard user. Test-owned UI cleanup bounded; graceful product shutdown is not asserted.';
}
main().catch(error=>{report.status='FAIL';report.error=error.stack;console.error(error.stack);process.exitCode=1;}).finally(()=>{
  save();
  // The elevated wrapper retires the disposable account's resident hub and
  // remaining adapter processes. Never wait on inherited transport handles
  // after both captured UI processes have already exited and evidence saved.
  process.exit(process.exitCode||0);
});
