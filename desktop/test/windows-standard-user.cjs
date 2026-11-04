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
  report.packageExe={path:executable,sha256:createHash('sha256').update(fs.readFileSync(executable)).digest('hex')};
  const {_electron}=require('playwright-core');
  const env={...process.env};
  for(const k of Object.keys(env))if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL/.test(k)||['BACKPLANE_SERVICE','BACKPLANE_WEB','BACKPLANE_SERVICE_VERSION','ELECTRON_RUN_AS_NODE','NODE_OPTIONS'].includes(k))delete env[k];
  const home=path.join(out,'Fresh home 日本語');assert(!fs.existsSync(home));
  const agents=path.join(out,'blocked-agents');fs.mkdirSync(agents);
  Object.assign(env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',HOME:process.env.USERPROFILE});
  let app,page;
  try{
    app=await _electron.launch({executablePath:executable,args:[],env,chromiumSandbox:true,timeout:60000});
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
  }finally{if(app)await app.close();}
  // An unavailable packaged service must remain a visible terminal error.
  app=null;
  try{
    const failedEnv={...env,BACKPLANE_HOME:path.join(out,'Failure home'),BACKPLANE_SERVICE:path.join(out,'missing-service.exe')};
    app=await _electron.launch({executablePath:executable,args:[],env:failedEnv,chromiumSandbox:true,timeout:60000});
    page=await app.firstWindow();await page.locator('#retry').waitFor({state:'visible',timeout:15000});
    const message=await page.locator('#status').innerText();assert.match(message,/ENOENT/);
    await page.waitForTimeout(7000);assert.equal(await page.locator('#status').innerText(),message);assert(await page.locator('#retry').isVisible());
    await page.screenshot({path:path.join(out,'startup-error.png')});report.checks.push('packaged missing-service error stays visible with Retry after seven seconds');
  }finally{if(app)await app.close();}
  report.status='PASS';report.scope='Exact packaged desktop startup, first-run service/store, authenticated main UI and terminal failure UI as standard user';
}
main().catch(error=>{report.status='FAIL';report.error=error.stack;console.error(error.stack);process.exitCode=1;}).finally(save);
