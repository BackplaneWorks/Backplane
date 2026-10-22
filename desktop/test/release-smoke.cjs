'use strict';
// Focused final-target smoke derived from smoke.cjs and the accepted thin probe.
// Executes supplied stage bytes on this native host; never sends chat or mail.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const assert=require('node:assert/strict'),crypto=require('node:crypto');
const {spawn,execFileSync}=require('node:child_process');
const sha=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const read=p=>JSON.parse(fs.readFileSync(p,'utf8'));
const write=(p,v)=>fs.writeFileSync(p,JSON.stringify(v,null,2)+'\n');
const ref=p=>({path:p,sha256:sha(p),bytes:fs.statSync(p).size});
function hostTarget(){return `${process.platform==='win32'?'windows':process.platform}-${process.arch}`;}
function journalCheck(before,after){
  assert(after.subarray(0,before.length).equals(before),'Seed journal prefix changed');
  const added=after.subarray(before.length).toString('utf8').split('\n').filter(Boolean).map(JSON.parse);
  assert(!added.some(e=>['MailPosted','RoomPosted','MessagePosted','RunSet','BotWoke','TurnQueued'].includes(e.$)),'Smoke must not send or wake an agent');
  const all=after.toString('utf8').split('\n').filter(Boolean).map(JSON.parse);
  for(const key of ['mail.on','telemetry.share'])assert(all.filter(e=>e.$==='SettingSet'&&e.key===key).every(e=>e.value==='off'),key+' enabled');
  return {prefixExact:true,addedKinds:[...new Set(added.map(e=>e.$))],noSendOrAgentTurn:true,mailOff:true,telemetryOff:true};
}
function linuxSandbox(pid,mainPid){
  const status=fs.readFileSync(`/proc/${pid}/status`,'utf8');
  const value=name=>Number(new RegExp(`^${name}:\\s+(\\d+)`,'m').exec(status)?.[1]);
  const namespaces=Object.fromEntries(['user','pid','mnt'].map(name=>[name,{renderer:fs.readlinkSync(`/proc/${pid}/ns/${name}`),main:fs.readlinkSync(`/proc/${mainPid}/ns/${name}`)}]));
  const facts={rendererPid:pid,seccomp:value('Seccomp'),noNewPrivs:value('NoNewPrivs'),namespaces};
  assert.equal(facts.seccomp,2,'Renderer lacks kernel seccomp filter');
  assert.equal(facts.noNewPrivs,1,'Renderer lacks no-new-privileges');
  assert(Object.values(namespaces).some(n=>n.renderer!==n.main),'No distinct Chromium sandbox namespace');
  return facts;
}
function nativeIdentity(child){
  assert(child?.pid&&child.exitCode===null,'Missing captured native child');
  if(process.platform==='linux')return {pid:child.pid,birth:fs.readFileSync(`/proc/${child.pid}/stat`,'utf8').split(')').slice(1).join(')').trim().split(/\s+/)[19],executable:fs.readlinkSync(`/proc/${child.pid}/exe`)};
  if(process.platform==='darwin')return {pid:child.pid,birth:execFileSync('/bin/ps',['-p',String(child.pid),'-o','lstart='],{encoding:'utf8',timeout:5000}).trim(),executable:execFileSync('/bin/ps',['-p',String(child.pid),'-o','comm='],{encoding:'utf8',timeout:5000}).trim()};
  // This is Node's captured native child PID, never the POSIX hub.lock field.
  const shell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
  const text=execFileSync(shell,['-NoProfile','-NonInteractive','-Command',`$p=Get-CimInstance Win32_Process -Filter 'ProcessId = ${child.pid}'; if(!$p){exit 2}; @{pid=$p.ProcessId;birth=$p.CreationDate.ToUniversalTime().ToString('o');executable=$p.ExecutablePath}|ConvertTo-Json -Compress`],{encoding:'utf8',timeout:10000});
  return JSON.parse(text);
}
async function bounded(p,label,ms=15000){let timer;try{return await Promise.race([p,new Promise((_,reject)=>timer=setTimeout(()=>reject(Error('Timed out: '+label)),ms))]);}finally{clearTimeout(timer);}}
async function exitOf(child,label){if(child.exitCode!==null||child.signalCode!==null)return child.exitCode;return bounded(new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);}),label,10000);}
// Inspect the actual window's named Electron session; Playwright's default CDP
// context is not the app's persist:backplane-ui cookie store. Never return values.
async function windowAuthWitness({BrowserWindow,session},origin){
  const windows=BrowserWindow.getAllWindows().filter(w=>{try{return new URL(w.webContents.getURL()).origin===origin;}catch{return false;}});
  if(windows.length!==1)return {matchingWindows:windows.length,uiPartition:false,cookieCount:0,httpOnly:false};
  const ses=windows[0].webContents.session,cookies=await ses.cookies.get({url:origin,name:'bp_token'});
  return {matchingWindows:1,uiPartition:ses===session.fromPartition('persist:backplane-ui'),cookieCount:cookies.length,httpOnly:cookies.length===1&&cookies[0].httpOnly===true};
}
function childAlive(child){return !!child&&child.exitCode===null&&child.signalCode===null;}
async function cleanupOwnedUi(app,child){
  let error;
  try{await bounded(app.close(),'owned UI cleanup',5000);if(childAlive(child))await exitOf(child,'owned UI exit');}
  catch(e){error=e.message;if(childAlive(child)){child.kill('SIGTERM');try{await exitOf(child,'captured UI fallback exit');}catch(f){error+='; '+f.message;}}}
  return {closed:!childAlive(child),error};
}
function argumentsFor(argv){
  if(argv.length===1&&argv[0]==='--help')return null;
  assert.equal(argv.length,4,'Use --stage ABS_STAGE --out NEW_ABS_EVIDENCE');
  const a={};for(let i=0;i<4;i+=2){assert(['--stage','--out'].includes(argv[i])&&!a[argv[i]],'Unknown/duplicate option');a[argv[i]]=argv[i+1];}
  assert(path.isAbsolute(a['--stage'])&&path.isAbsolute(a['--out']),'Absolute paths required');
  assert(!fs.existsSync(a['--out']),'Evidence directory must be new');return {stage:a['--stage'],out:a['--out']};
}
async function main(args){
  const options=argumentsFor(args);if(!options){console.log('node desktop/test/release-smoke.cjs --stage ABS_TARGET_STAGE --out NEW_ABS_EVIDENCE\nRequires pinned playwright-core 1.64.1 and a native desktop session. Linux requires working unprivileged Chromium sandbox. No product compile, mail/chat Send or provider turn.');return;}
  const {stage,out}=options,producer=require('../produce-release.cjs'),{_electron}=require('playwright-core');
  const s=read(path.join(stage,'STAGE.json'));assert.equal(s.target,hostTarget(),'A matching native host is required; no emulation substitution');
  assert.equal(s.release.version,'0.12.1');assert.equal(s.electronVersion,'44.9.2');
  assert.deepEqual(s.lifecyclePolicy,{mode:'resident-service',managedStop:false,managedUpdate:false});
  const inspected=producer.inspectStage(stage);assert.equal(inspected.payloadManifestSha256,s.payloadManifestSha256,'Stage payload changed');
  const layout=producer.TARGETS[s.target],resources=path.join(stage,layout.resources),appRoot=path.join(resources,'app');
  const binary=path.join(resources,'backplane-service',process.platform==='win32'?'backplane-serve.exe':'backplane-serve');
  const executable=path.join(stage,s.entrypoint),{Service}=require(path.join(appRoot,'service.cjs'));
  fs.mkdirSync(out);const home=fs.mkdtempSync(path.join(os.tmpdir(),'Backplane smoke 日本語 ')),project=path.join(home,'project 日本語'),agents=path.join(home,'agents');
  fs.mkdirSync(project);fs.mkdirSync(agents); // Empty stand-in directory blocks every real provider CLI.
  const marker='FINAL_CURRENT_日本語_👩‍💻_é',at=Math.floor(Date.now()/1000),ev=($,v)=>({$,...v,at});
  const events=[ev('ProjectCreated',{id:'release-smoke',title:'Release smoke 日本語',root:project}),
    ev('ThreadCreated',{id:'release-thread',title:'Current release content',project:'release-smoke',env:'local',branch:'',worktree:project,provider:'codex',model:''}),
    ...['mail.on','restart.continue','telemetry.share'].map(key=>ev('SettingSet',{key,value:'off'})),
    ev('SettingSet',{key:'telemetry.asked',value:'1'}),ev('SettingSet',{key:'onboard.done',value:'1'}),ev('SettingSet',{key:'ui.lang',value:'en'}),
    ev('MessagePosted',{thread:'release-thread',msg:'release-current',role:'assistant',text:marker})];
  const prefix=Buffer.from(events.map(JSON.stringify).join('\n')+'\n');fs.writeFileSync(path.join(home,'events.jsonl'),prefix);
  const env={...process.env};for(const k of Object.keys(env))if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL|AUTH_COOKIE/.test(k))delete env[k];
  for(const k of ['BACKPLANE_SERVICE','BACKPLANE_WEB','BACKPLANE_SERVICE_VERSION','NODE_OPTIONS','ELECTRON_RUN_AS_NODE','BACKPLANE_TELEMETRY_URL','BACKPLANE_TELEMETRY_KID','BACKPLANE_TELEMETRY_KEY'])delete env[k];
  Object.assign(env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
  // Exact shipped Service owns the native child handle. UI then discovers and authenticates to it.
  const originalEnv=process.env;process.env=env;
  const service=new Service({home,binary,web:path.join(resources,'backplane-service/web'),expectedVersion:'0.12.1',residentService:true});
  let app,uiChild,page,second,error,phase='prepare',serviceIdentity,sandbox,uiPid,origin;
  const facts={checks:[],errors:[],reconnectMode:'automatic',deferredByUser:['telemetry-privacy','telemetry-consent','telemetry-ingestion'],scope:'Minimal actual matching-host stable stage startup/discovery/auth, renderer OS sandbox, current content, automatic reconnect, single instance and resident close. Telemetry validation DEFERRED by user, not PASS. No Send/provider/scale/390/native-phone/IME/Browser parity or telemetry-upload claim.'};
  const save=()=>write(path.join(out,'result.json'),{...facts,status:error||facts.cleanupError||facts.cleanup?.portClosed===false?'FAIL':phase==='done'?'PASS':'INCOMPLETE',phase,error:error?.message,home,target:s.target,version:'0.12.1',commit:s.release.commit,tree:s.release.tree,stage:inspected,fixture:ref(__filename),serviceIdentity,sandbox,uiPid});
  const step=name=>{phase=name;save();console.log('phase '+name);};
  const record=name=>{facts.checks.push(name);save();};
  async function ready(){await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:30000});const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.isVisible())await skip.click();}
  async function current(){await page.locator('[data-on-click="select"][data-value="release-thread"]').first().click();await page.waitForFunction(text=>document.querySelector('#timeline')?.textContent.includes(text),marker);}
  try{
    step('genuine-service-startup');const connection=await bounded(service.start(),'service canonical/auth readiness',50000);origin=connection.origin;
    serviceIdentity=nativeIdentity(service.child);write(path.join(out,'service.json'),{version:connection.version,port:new URL(origin).port,identity:serviceIdentity,binary:ref(binary),nativePathWithSpacesAndUnicode:home});
    record('captured native child starts exact stable service using Unicode/space home');
    const token=await service.token(),bad=token==='0'.repeat(32)?'1'.repeat(32):'0'.repeat(32);
    assert.equal((await fetch(origin+'/auth/pair',{method:'POST',headers:{Authorization:'Bearer '+bad},signal:AbortSignal.timeout(3000)})).status,401);
    assert.equal((await fetch(origin+'/auth/pair',{method:'POST',headers:{Authorization:'Bearer '+token,Origin:'https://foreign.invalid'},signal:AbortSignal.timeout(3000)})).status,401);
    record('wrong credential and foreign-origin pairing refused');
    step('packaged-window');app=await _electron.launch({executablePath:executable,args:[],chromiumSandbox:true,env,timeout:30000});uiChild=app.process();uiPid=uiChild.pid;
    await app.context().addInitScript(()=>{const sockets=new Set(),c=globalThis.__releaseSockets={opened:0,closed:0,messages:0};const WS=WebSocket;globalThis.WebSocket=class extends WS{constructor(...args){super(...args);sockets.add(this);this.addEventListener('open',()=>c.opened++);this.addEventListener('message',()=>c.messages++);this.addEventListener('close',()=>{c.closed++;sockets.delete(this);});}};globalThis.__releaseDisconnect=()=>{for(const s of sockets)s.close();};});
    page=await app.firstWindow();page.setDefaultTimeout(15000);page.on('pageerror',e=>facts.errors.push(e.message));await ready();
    // A very fast initial page may predate addInitScript. A normal reload installs
    // the same socket observer used by the retained accepted probe, before JS.
    await page.reload({waitUntil:'domcontentloaded'});await ready();await current();
    assert.deepEqual(await page.evaluate(()=>({node:typeof require,process:typeof process})),{node:'undefined',process:'undefined'});
    const versions=await page.evaluate(()=>backplaneDesktop.version());assert.equal(versions.service,'0.12.1');assert.equal(versions.electron,'44.9.2');
    facts.authSession=await app.evaluate(windowAuthWitness,origin);save();
    assert.equal(facts.authSession.matchingWindows,1,'Expected one authenticated app window');assert.equal(facts.authSession.uiPartition,true,'Wrong app session partition');
    assert.equal(facts.authSession.cookieCount,1,'Authenticated app session cookie missing/ambiguous');assert.equal(facts.authSession.httpOnly,true,'Authenticated app session cookie must be HttpOnly');assert(!await page.evaluate(()=>document.cookie.includes('bp_token=')));
    await page.screenshot({path:path.join(out,'current.png')});record('actual Unicode current DOM, exact versions, authenticated isolated preload');
    step('actual-renderer-sandbox');const runtime=await app.evaluate(({app,BrowserWindow})=>{const w=BrowserWindow.getAllWindows()[0],wc=w.webContents,p=wc.getLastWebPreferences(),pid=wc.getOSProcessId(),m=app.getAppMetrics().find(m=>m.pid===pid);return {platform:process.platform,arch:process.arch,mainPid:process.pid,rendererPid:pid,argv:process.argv,prefs:{sandbox:p.sandbox,nodeIntegration:p.nodeIntegration,contextIsolation:p.contextIsolation},metric:m?{pid:m.pid,type:m.type,sandboxed:m.sandboxed,integrityLevel:m.integrityLevel}:null};});
    assert(!runtime.argv.some(a=>['--no-sandbox','--disable-seccomp-filter-sandbox','--disable-namespace-sandbox','--disable-gpu-sandbox'].includes(a)));
    assert.deepEqual(runtime.prefs,{sandbox:true,nodeIntegration:false,contextIsolation:true});
    if(process.platform==='linux')sandbox={...runtime,kernel:linuxSandbox(runtime.rendererPid,runtime.mainPid)};else{assert.equal(runtime.metric?.sandboxed,true,'Actual renderer is not sandboxed on OS level');sandbox=runtime;}
    write(path.join(out,'sandbox.json'),sandbox);record('actual renderer OS sandbox');
    step('single-instance');second=spawn(executable,[],{env,stdio:'ignore',windowsHide:true});const secondPid=second.pid;assert.equal(await exitOf(second,'second-instance exit'),0);
    const windows=await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows().length);assert.equal(windows,1);assert.equal(uiChild.pid,uiPid);assert.equal(nativeIdentity(service.child).birth,serviceIdentity.birth);write(path.join(out,'single-instance.json'),{secondPid,secondExit:second.exitCode,windowCount:windows,originalUiPid:uiPid});record('second launcher joins single resident UI/service');
    step('websocket-reconnect');const opened=await page.evaluate(()=>{const n={opened:__releaseSockets.opened,messages:__releaseSockets.messages};__releaseDisconnect();return n;});
    // the page reconnects by itself (src/web/reconnect.js): no Retry click
    await page.waitForFunction(()=>__releaseSockets.closed>0,null,{timeout:5000});
    await page.waitForFunction(n=>__releaseSockets.opened>n.opened&&__releaseSockets.messages>n.messages&&document.documentElement.dataset.backplaneRemoteReady==='true',opened,{timeout:20000});await current();
    await page.reload({waitUntil:'domcontentloaded'});await ready();await current();record('real socket reconnect and reload restore current content');
    const menu=await app.evaluate(({Menu})=>{const labels=[];const walk=items=>{for(const i of items){labels.push(i.label||'');if(i.submenu)walk(i.submenu.items);}};walk(Menu.getApplicationMenu()?.items||[]);return labels;});assert(!menu.some(v=>/stop service|restart service|update service/i.test(v)));record('no unsupported managed-stop/update menu advertised');
    step('window-close-resident');const closed=page.waitForEvent('close');await app.evaluate(({BrowserWindow})=>{setTimeout(()=>BrowserWindow.getAllWindows()[0].close(),25);return true;});await bounded(closed,'window close',5000);await app.close();app=null;
    assert.equal(nativeIdentity(service.child).birth,serviceIdentity.birth);assert(await service.discover(),'Closing UI terminated service');record('window close leaves exact captured backend resident');
    facts.journal=journalCheck(prefix,fs.readFileSync(path.join(home,'events.jsonl')));assert.equal(facts.errors.length,0,facts.errors.join('\n'));step('done');
  }catch(e){error=e;process.exitCode=1;if(page&&!page.isClosed())await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});console.error(e.message);}
  finally{
    if(childAlive(second))second.kill();if(app){const uiCleanup=await cleanupOwnedUi(app,uiChild);if(uiCleanup.error){facts.cleanupError=uiCleanup.error;process.exitCode=1;}}
    // Test cleanup only: retained native ChildProcess handle, never lock-file PID.
    const child=service.child;if(child&&child.exitCode===null){child.kill('SIGTERM');await exitOf(child,'owned test-service cleanup').catch(e=>{facts.cleanupError=e.message;process.exitCode=1;});}
    facts.cleanup={nativeChildPid:child?.pid,nativeChildExit:child?.exitCode,signal:child?.signalCode,uiClosed:!childAlive(uiChild)};
    if(child&&child.exitCode===null&&child.signalCode===null){facts.cleanupError='Captured test service is still alive';process.exitCode=1;}
    if(origin){try{await fetch(origin+'/hello',{signal:AbortSignal.timeout(1500)});facts.cleanup.portClosed=false;process.exitCode=1;}catch{facts.cleanup.portClosed=true;}}
    process.env=originalEnv;save();
  }
}
module.exports={hostTarget,journalCheck,linuxSandbox,argumentsFor,windowAuthWitness,childAlive,cleanupOwnedUi};
if(require.main===module)main(process.argv.slice(2)).catch(e=>{console.error(e.message);process.exitCode=1;});
