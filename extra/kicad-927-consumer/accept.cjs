'use strict';
// PREPARED, NOT EXECUTED. Artifact consumer only: no compiler, bundler,
// package modification, source overlay, RPC injection or release publication.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {execFileSync} = require('node:child_process');
const {DatabaseSync} = require('node:sqlite');
const configPath = path.resolve(process.argv[2] || '');
const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const out = path.resolve(cfg.out);
fs.mkdirSync(out, {recursive:true});
const result = {schema:1, issue:927, status:'INCOMPLETE', cases:[], cleanup:[], published:false};
const save = () => fs.writeFileSync(path.join(out,'result.json'), JSON.stringify(result,null,2)+'\n');
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const json = file => JSON.parse(fs.readFileSync(file,'utf8'));
const normal = value => value.replaceAll('\\','/').replace(/\/$/,'').toLowerCase();
const points = value => Array.from(value, c => c.codePointAt(0));
const wait = ms => new Promise(resolve => setTimeout(resolve,ms));
async function until(label, fn, ms=120000) {
  const end=Date.now()+ms; let last;
  while(Date.now()<end) { try { const value=await fn(); if(value)return value; } catch(e){if(e.code==='HEALTH_JSON_INVALID')throw e;last=e.message;} await wait(250); }
  throw Error(label+' timed out'+(last?': '+last:''));
}
function setting(home) {
  const db=new DatabaseSync(path.join(home,'store.db'),{readOnly:true});
  try { return db.prepare('SELECT v FROM setting WHERE k=?').get('kicad.path')?.v ?? ''; }
  finally { db.close(); }
}
function health(home) {
  const raw=fs.readFileSync(path.join(home,'kicad-health.json'),'utf8');
  try {return JSON.parse(raw);}
  catch(cause) {const e=Error('Invalid production health JSON: '+cause.message);e.code='HEALTH_JSON_INVALID';throw e;}
}
function cleanEnvironment() {
  const env={...process.env};
  for(const key of Object.keys(env)) {
    if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL/i.test(key) ||
       /^BACKPLANE_(KICAD|SERVICE|WEB|JS_LOOP|HOME|PEERS)/i.test(key) ||
       /^(NODE_OPTIONS|ELECTRON_RUN_AS_NODE|PATH)$/i.test(key)) delete env[key];
  }
  // The packaged service must add its own runtime/bin. Keep OS commands,
  // never an ambient KiCad/provider executable or a different Cygwin.
  env.PATH=[path.join(process.env.SystemRoot,'System32'),process.env.SystemRoot].join(';');
  Object.assign(env,{BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',
    BACKPLANE_AGENTS:path.join(out,'blocked-agents'),LANG:'en_US.UTF-8'});
  fs.mkdirSync(env.BACKPLANE_AGENTS,{recursive:true});
  return env;
}
const apps=[];
async function openCase(name, pathRoot) {
  const home=path.join(out,'home-'+name+' 日本');
  assert(!fs.existsSync(home),'Acceptance home must be new');
  const env=cleanEnvironment();env.BACKPLANE_HOME=home;
  if(pathRoot) env.PATH=path.join(pathRoot,'bin')+';'+env.PATH;
  const {_electron}=require(path.resolve(cfg.playwrightModule));
  const app=await _electron.launch({executablePath:cfg.executable,args:[],env,chromiumSandbox:true,timeout:90000});
  apps.push(app);
  const entry={name,home,pid:app.process().pid,phase:'launching'};result.cases.push(entry);save();
  const page=await app.firstWindow();
  try { await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:120000}); }
  catch(e) {
    entry.url=page.url();entry.visibleText=await page.locator('body').innerText().catch(()=>'<unavailable>');save();
    await page.screenshot({path:path.join(out,name+'-startup-failed.png')}).catch(()=>{});throw e;
  }
  const url=new URL(page.url());assert.equal(url.hostname,'127.0.0.1');
  Object.assign(entry,{origin:url.origin,phase:'ready'});save();
  // Dismiss the fresh home's onboarding through its real visible control.
  // It holds input while open; seeding a setting would bypass this boundary.
  const skip=page.locator('.tour [data-on-click="ob-skip"]');
  if(await skip.isVisible()) {
    await skip.click();await page.locator('.tour').waitFor({state:'hidden',timeout:30000});
    entry.tourSkipped=true;save();
  }
  return {app,page,home,entry};
}
async function cadSettings(page) {
  const skip=page.locator('.tour [data-on-click="ob-skip"]');
  if(await skip.isVisible()) {await skip.click();await page.locator('.tour').waitFor({state:'hidden',timeout:30000});}
  if(!await page.locator('#settings-content').isVisible()) {
    await page.locator('[data-on-click="flag"][data-value="settings"]').first().click();
  }
  if(await page.locator('#settings-content').getAttribute('data-settings-page')!=='cad') {
    await page.locator('[data-on-click="settings-page"][data-value="cad"]').first().click();
  }
  await page.locator('#kicad-path').waitFor({state:'visible',timeout:30000});
}
async function typePath(context, typed, expectedStatus='ok') {
  const {page,home,entry}=context;
  await cadSettings(page);
  const field=page.locator('#kicad-path');
  await field.fill('');
  // Real key/text events, including each literal backslash; no dispatch shortcut.
  if(typed) await field.pressSequentially(typed,{delay:20});
  const dom=await field.inputValue();assert.equal(dom,typed,'DOM input corruption');
  entry.typed=typed;entry.dom=dom;entry.domCodePoints=points(dom);save();
  await page.screenshot({path:path.join(out,entry.name+'-typed.png')});
  await field.press('Enter');
  await until('literal persisted setting',()=>setting(home)===typed);
  entry.saved=setting(home);entry.savedCodePoints=points(entry.saved);save();
  const h=await until('checked manual path',()=>{
    const h=health(home);return h.setting===typed && h.status===expectedStatus ? h : false;
  });
  assert.equal(h.setting,typed,'Resolver input differs from submitted setting');
  entry.saved=setting(home);entry.savedCodePoints=points(entry.saved);entry.health=h;save();
  await page.screenshot({path:path.join(out,entry.name+'-checked.png')});
  return h;
}
async function detected(context, expectedRoot, expectedSource) {
  const h=await until('native detection',()=>{const v=health(context.home);return v.status==='ok'?v:false;});
  assert.equal(normal(h.cli),normal(path.join(expectedRoot,'bin','kicad-cli.exe')));
  assert.equal(h.source,expectedSource);
  assert.equal(setting(context.home),'','Detection must have no explicit setting');
  context.entry.health=h;save();
}
async function check(name,fn) {
  try { await fn(); }
  catch(e) {
    (result.failures??=[]).push({name,error:e.stack});
    for(const entry of result.cases) {try{entry.lastHealth=health(entry.home);}catch{}}
    save();
  }
}
async function main() {
  assert.equal(process.platform,'win32','Real Windows required');
  assert.equal(process.env.GITHUB_ACTIONS,'true','Hosted acceptance only');
  assert.equal(process.env.GITHUB_RUN_ID,cfg.hostedRun.runId,'Actual hosted-run identity mismatch');
  result.hostedRun=cfg.hostedRun;
  assert(cfg.buildProvenance && typeof cfg.buildProvenance.compilerMode==='string' && cfg.buildProvenance.compilerMode.length>0,'Actual compiler mode required in test evidence');
  assert(typeof cfg.buildProvenance.producerWorkId==='string' && cfg.buildProvenance.producerWorkId.length>0,'Actual producer work identity required');
  result.buildProvenance=cfg.buildProvenance;
  result.releaseBuildApproved=false;
  assert(/^[a-f0-9]{40}$/.test(cfg.sourceCommit));assert(/^[a-f0-9]{40}$/.test(cfg.sourceTree));
  assert(/^[a-f0-9]{64}$/.test(cfg.packageSHA256));
  assert.equal(sha(cfg.packagePath),cfg.packageSHA256,'Authoritative Windows ZIP mismatch');
  const shell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
  result.identity=JSON.parse(execFileSync(shell,['-NoProfile','-NonInteractive','-Command',
    '$i=[Security.Principal.WindowsIdentity]::GetCurrent(); $p=[Security.Principal.WindowsPrincipal]::new($i); @{name=$i.Name;sid=$i.User.Value;admin=$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)} | ConvertTo-Json -Compress'],{encoding:'utf8'}));
  assert.equal(result.identity.admin,false,'All assertions and application processes must be standard-user');
  assert.equal(cfg.standardUserSid,result.identity.sid,'Wrong disposable account');
  const resources=json(path.join(out,'resources-start.json'));
  assert(resources.jobMemoryLimitBytes>0 && resources.jobMemoryLimitBytes<=8589934592 && resources.member && (resources.limitFlags&512) && !(resources.limitFlags&0x1800),'Kernel job cap / no-breakaway gate missing');
  assert(Array.isArray(resources.pageFiles) && resources.pagingCounters,'Actual pagefile and paging measurements required');
  assert(resources.pageFiles.length===0 || cfg.resourcePolicy?.allowExistingPagefile===true,'Existing pagefile exception requires explicit authorization');
  result.resourcePolicy=cfg.resourcePolicy;
  result.resourcesStart=resources;
  const build=json(path.join(path.dirname(cfg.executable),'resources','DESKTOP-BUILD.json'));
  assert.equal(build.sourceCommit,cfg.sourceCommit,'Package source commit mismatch');
  assert.equal(build.sourceTree,cfg.sourceTree,'Package source tree mismatch');
  assert.equal(build.serviceVersion,cfg.version,'Package version mismatch');
  assert.equal(build.testOnly,true);assert.equal(build.releaseQualified,false);
  assert.equal(build.buildProvenance.releaseBuildApproved,false);
  assert.deepEqual(build.buildProvenance,cfg.buildProvenance,'Compiler provenance mismatch');
  result.package={sourceCommit:build.sourceCommit,sourceTree:build.sourceTree,version:build.serviceVersion,zipSHA256:cfg.packageSHA256,executableSHA256:sha(cfg.executable)};
  const serviceDir=path.join(path.dirname(cfg.executable),'resources','backplane-service');
  const serviceBuild=json(path.join(serviceDir,'BUILD.json'));
  assert.deepEqual(serviceBuild.buildProvenance,cfg.buildProvenance);
  const serviceSHA=sha(path.join(serviceDir,'backplane-serve.exe'));
  assert.equal(serviceSHA,'11f53c9619d095e0ccd520c5616743155ee240b9637021d4d5a0bea4707aadd1');
  assert.equal(serviceSHA,serviceBuild.binary_sha256);result.package.serviceSHA256=serviceSHA;
  assert(normal(cfg.programFilesRoot).startsWith(normal(process.env.ProgramFiles)+'/kicad/'),'Program Files fixture must be under actual Program Files');
  for(const root of [cfg.programFilesRoot,cfg.pathRoot,...cfg.manualRoots]) {
    assert.equal(sha(path.join(root,'bin/kicad-cli.exe')),cfg.runtimeCliSHA256,'Fixture must contain exact real vendor runtime');
  }
  save();
  await check('program-files',async()=>{
    const first=await openCase('program-files');await detected(first,cfg.programFilesRoot,'system');
  });
  await check('native-path',async()=>{
    const second=await openCase('native-path',cfg.pathRoot);await detected(second,cfg.pathRoot,'path');
  });
  // Assert the exact reported spelling is included; also exercise literal \n/\t and spaces.
  assert(cfg.manualRoots.some(p=>p==='C:\\backplane_kicad\\'));
  assert(cfg.manualRoots.some(p=>p.includes('\\new')&&p.includes('\\test')&&p.includes(' ')));
  for(let i=0;i<cfg.manualRoots.length;i++) {
    await check('manual-'+i,async()=>{
    const context=await openCase('manual-'+i);
    const typed=cfg.manualRoots[i];
    try {
    const h=await typePath(context,typed);
    assert.equal(normal(h.cli),normal(path.join(typed,'bin/kicad-cli.exe')));
    assert.equal(h.source,'setting');
    } finally {
    if(context.entry.saved===typed) {
    await context.page.reload();
    await context.page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:90000});
    await cadSettings(context.page);
    assert.equal(await context.page.locator('#kicad-path').inputValue(),typed,'Saved path changed after a real page reload');
    context.entry.reloadPreserved=true;save();
    }
    }
    });
  }
  await check('real-install',async()=>{
  const install=await openCase('real-install');
  // Force a genuine missing runtime notice without changing Program Files or
  // substituting a download. After install, clear the explicit missing setting.
  const missing=path.join(out,'absent-runtime')+'\\';
  assert(!fs.existsSync(missing));
  try {await typePath(install,missing,'missing');}
  catch(e) {if(e.code!=='HEALTH_JSON_INVALID')throw e;(result.failures??=[]).push({name:'install-health-json',error:e.message});save();}
  const button=install.page.locator('[data-on-click="kicad-install"], [data-on-click="cad-install"][data-value="kicad"]').first();
  await button.waitFor({state:'visible',timeout:30000});
  install.entry.installStartedAt=new Date().toISOString();save();await button.click();
  const installed=path.join(install.home,'kicad',cfg.runtimeVersion,'bin','kicad-cli.exe');
  await until('real in-app download / verify / extract / promote',()=>fs.existsSync(installed),1200000);
  assert.equal(sha(installed),cfg.runtimeCliSHA256,'Installed native CLI differs from the pinned runtime');
  install.entry.installedCli=installed;install.entry.installedCliSHA256=sha(installed);
  try {
    const h=await typePath(install,'');
    assert.equal(h.source,'installed');assert.equal(normal(h.cli),normal(installed));
    install.entry.installedResolverVerified=true;
  } catch(e) {if(e.code!=='HEALTH_JSON_INVALID')throw e;(result.failures??=[]).push({name:'installed-health-json',error:e.message});save();}
  assert.equal(json(path.join(install.home,'kicad',cfg.runtimeVersion,'manifest.json')).version,cfg.runtimeVersion);
  install.entry.version=execFileSync(installed,['version'],{encoding:'utf8',timeout:30000}).trim();
  assert(install.entry.version.includes(cfg.runtimeVersion.split('-bp.')[0]),'Installed real CLI version mismatch');
  install.entry.installCompletedAt=new Date().toISOString();
  install.entry.realInstallBytesAndCliVerified=true;save();
  await install.page.screenshot({path:path.join(out,'real-install-complete.png')});
  });
  assert.equal(result.failures?.length||0,0,'One or more native acceptance cases failed; see failures and partial evidence');
  result.status='PASS';result.scope='Exact packaged source; native Program Files/PATH discovery, literal UI typing/persistence/reload, real in-app install and CLI launch as standard user';save();
}
main().catch(e=>{
  result.status='FAIL';result.error=e.stack;process.exitCode=1;
  for(const entry of result.cases) {try{entry.lastHealth=health(entry.home);}catch{}}
}).finally(async()=>{
  for(const app of apps.reverse()) {
    const child=app.process(), row={pid:child.pid};
    try { await Promise.race([app.close(),new Promise((_,reject)=>setTimeout(()=>reject(Error('close timeout')),5000))]); }
    catch(e) {row.error=e.message; if(child.exitCode===null&&child.signalCode===null)child.kill('SIGTERM');}
    try { await until('captured UI exit',()=>child.exitCode!==null||child.signalCode!==null,5000); }
    catch(e) {row.exitError=e.message;result.status='FAIL';process.exitCode=1;}
    row.uiExited=child.exitCode!==null||child.signalCode!==null;result.cleanup.push(row);
  }
  save();
  // The SID-scoped bootstrap must retire resident hubs and verify cleanup.
  process.exit(process.exitCode||0);
});
