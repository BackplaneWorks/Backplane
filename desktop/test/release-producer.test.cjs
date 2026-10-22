'use strict';
// Synthetic offline artifact models only: no executable, signer, VM or service runs.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto'),{execFileSync}=require('node:child_process');
const producer=require('../produce-release.cjs');
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
function put(p,value,mode=0o644){fs.mkdirSync(path.dirname(p),{recursive:true});fs.writeFileSync(p,value);fs.chmodSync(p,mode);return p;}
const write=(p,o)=>put(p,JSON.stringify(o,null,2)+'\n');
const ref=p=>({path:p,sha256:hash(p),bytes:fs.statSync(p).size});
function binary(target){const b=Buffer.alloc(512);if(target.startsWith('linux')){b.write('7f454c46','hex');b[4]=2;b[5]=1;b.writeUInt16LE(target==='linux-x64'?62:183,18);}else if(target==='darwin-arm64'){b.writeUInt32LE(0xfeedfacf);b.writeUInt32LE(0x100000c,4);}else{b.write('MZ');b.writeUInt32LE(128,60);b.write('PE\0\0',128);b.writeUInt16LE(0x8664,132);}return b;}
function fixture(t,target='linux-x64',resident=false,releaseSource={}){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-producer-offline-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const release={version:'0.12.1',tag:'v0.12.1',channel:'release',commit:releaseSource.commit||'a'.repeat(40),tree:releaseSource.tree||'b'.repeat(40)};
 const identity=kind=>({schema:1,kind,status:'VERIFIED',target,sourceCommit:release.commit,sourceTree:release.tree,version:release.version,channel:'release'});
 const notice=(dir,name)=>{put(path.join(dir,'LICENSE.'+name),'SYNTHETIC OFFLINE LICENSE '+name);return{name,version:'fixture',sourceDisposition:'Synthetic test only; no redistribution or platform qualification',licenses:[{path:'LICENSE.'+name,sha256:hash(path.join(dir,'LICENSE.'+name))}]};};
 const shell=path.join(root,'shell'),electron=path.join(root,'electron'),service=path.join(root,'service');
 put(path.join(shell,'package.json'),JSON.stringify({name:'backplane-desktop',version:'0.12.1',main:'main.cjs'}));put(path.join(shell,'main.cjs'),'// offline data, never executed\n');
 const shellReceipt={...identity('backplane-electron-shell'),electronVersion:'44.9.2',bundleIdentifier:'dev.backplane.fixture',features:{defaultPackagedHome:true,contextIsolation:true,sandbox:true,nodeIntegration:false,managedStop:false,managedUpdate:false,windowsManagedControl:resident?'UNSUPPORTED':'fixture-control-v1',windowsNativePathMapping:true},packages:[notice(shell,'shell')]};shellReceipt.members=producer.inventory(shell,target);
 const exe=target==='darwin-arm64'?'Electron.app/Contents/MacOS/Electron':target==='windows-x64'?'electron.exe':'electron';put(path.join(electron,exe),binary(target),0o755);
 if(target==='darwin-arm64'){
  put(path.join(electron,'Electron.app/Contents/Info.plist'),'<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Electron</string></dict></plist>');
  put(path.join(electron,'Electron.app/Contents/Frameworks/Fixture.framework/Versions/A/data'),'fixture data');fs.symlinkSync('A',path.join(electron,'Electron.app/Contents/Frameworks/Fixture.framework/Versions/Current'));
 }
 const electronReceipt={...identity('backplane-electron-runtime'),electronVersion:'44.9.2',signablePaths:[exe],packages:[notice(electron,'electron')]};electronReceipt.runtimeManifest='RUNTIME.json';electronReceipt.closureManifest='CLOSURE.json';
 write(path.join(electron,'RUNTIME.json'),{schema:1,target,inventory:[{path:exe,package:'Electron',version:'44.9.2',architecture:target,sha256:hash(path.join(electron,exe)),bytes:512}],packages:electronReceipt.packages});
 write(path.join(electron,'CLOSURE.json'),{schema:1,target,measured:true,status:'PASS',edges:[],unresolvedMandatory:[]});electronReceipt.runtimeManifestSha256=hash(path.join(electron,'RUNTIME.json'));electronReceipt.closureSha256=hash(path.join(electron,'CLOSURE.json'));electronReceipt.members=producer.inventory(electron,target);
 const serviceExe=target==='windows-x64'?'backplane-serve.exe':'backplane-serve';put(path.join(service,serviceExe),binary(target),0o755);
 const packageNotice=notice(service,'Backplane');put(path.join(service,'web/index.html'),'<main>synthetic stable fixture</main>');put(path.join(service,'web/index.html.gz'),require('node:zlib').gzipSync(fs.readFileSync(path.join(service,'web/index.html'))));
 const web={schema:1,kind:'matching-generated-web',sourceCommit:release.commit,sourceTree:release.tree,entrypoint:'src/web/index.html',members:{}};
 for(const n of fs.readdirSync(path.join(service,'web')))web.members[n]={sha256:hash(path.join(service,'web',n)),bytes:fs.statSync(path.join(service,'web',n)).size};write(path.join(service,'WEB.json'),web);
 const runtimeNotice=notice(service,'runtime');const runtimeMember=serviceExe;
 const runtime={schema:1,target,inventory:[{path:runtimeMember,package:'fixture',version:'fixture',architecture:target,sha256:hash(path.join(service,runtimeMember)),bytes:fs.statSync(path.join(service,runtimeMember)).size}],packages:[runtimeNotice]};write(path.join(service,'RUNTIME.json'),runtime);
 const closure={schema:1,target,measured:true,status:'PASS',edges:[{from:serviceExe,system:true,kind:'normal',classification:'approved-system',name:'synthetic-system'}],unresolvedMandatory:[]};write(path.join(service,'CLOSURE.json'),closure);
 const compileInputs={version:'0.12.1',versionStamp:'0.12.1',telemetryChannel:'release',google:{requested:true,requiredInputsPresent:true},telemetry:{requested:true,requiredInputsPresent:true,endpoint:'https://backplane.works/api/v1/events'}};
 const buildReceipt={versionStamp:'0.12.1',telemetryChannel:'release',compileInputs,status:'VERIFIED_GENUINE_SYNTHETIC_FIXTURE_ONLY',target,entry:'src/server/main.bend',version:'0.12.1',source_commit:release.commit,source_tree:release.tree,serviceRuntime:'bun',service:serviceExe,binary_sha256:hash(path.join(service,serviceExe))};const buildPath=write(path.join(root,'genuine.json'),buildReceipt);fs.copyFileSync(buildPath,path.join(service,'BUILD.json'));
 let serviceSpec,serviceReceipt;
 {
  serviceReceipt={...identity('backplane-bend-service'),genuineService:true,entrypoint:'src/server/main.bend',roles:{service:serviceExe,web:'web'},buildReceipt:ref(buildPath),runtimeManifest:'RUNTIME.json',runtimeManifestSha256:hash(path.join(service,'RUNTIME.json')),closureManifest:'CLOSURE.json',closureSha256:hash(path.join(service,'CLOSURE.json')),packages:[packageNotice]};serviceReceipt.members=producer.inventory(service,target);const p=write(path.join(root,'service.json'),serviceReceipt);serviceSpec={payload:{directory:service},receipt:ref(p)};
 }
 const shellRef=ref(write(path.join(root,'shell.json'),shellReceipt)),electronRef=ref(write(path.join(root,'electron.json'),electronReceipt));
 const pins={shellReceiptSha256:shellRef.sha256,electronReceiptSha256:electronRef.sha256,webManifestSha256:hash(path.join(service,'WEB.json')),serviceReceiptSha256:serviceSpec.receipt.sha256};
 const freeze={schema:1,kind:'backplane-release-freeze',status:'FROZEN',...release,compileInputs,sourceDateEpoch:1700000000,signingPolicy:{[target]:'unsigned-portable-allowed'},components:{[target]:pins},...(resident?{lifecyclePolicy:{[target]:{mode:'resident-service',managedStop:false,managedUpdate:false}}}:{})};
 release.freezeReceipt=ref(write(path.join(root,'freeze.json'),freeze));
 const request={schema:1,kind:'backplane-electron-release-inputs',target,release,shell:{payload:{directory:shell},receipt:shellRef},electron:{payload:{directory:electron},receipt:electronRef},service:serviceSpec};
 const requestPath=write(path.join(root,'request.json'),request);
 return{root,target,request,requestPath,shell,electron,service,serviceReceipt,shellReceipt,electronReceipt,freeze,refresh(){request.release.freezeReceipt=ref(write(path.join(root,'freeze.json'),freeze));write(requestPath,request);}};
}
function evidence(f,dir,changes={}){
 const inspected=producer.inspectStage(dir),common={schema:1,status:'PASS',target:f.target,version:'0.12.1',commit:f.request.release.commit,tree:f.request.release.tree,payloadManifestSha256:inspected.payloadManifestSha256,freezeReceiptSha256:f.request.release.freezeReceipt.sha256};
 const receipts={signing:{...common,kind:'desktop-signing',signed:false},sandbox:{...common,kind:'desktop-sandbox',osEnforced:true,flags:[]},lifecycle:{...common,kind:'desktop-lifecycle',nativeChildIdentity:true,authBound:true,singleInstance:true,reconnect:true,windowClose:'resident-service',shutdown:f.freeze.lifecyclePolicy?'unsupported':'owned-native-child',startup:true,closePreservesService:true,managedStopAdvertised:false,managedUpdateAdvertised:false,nativeToPosixPaths:true},runtimeAcceptance:{...common,kind:'desktop-runtime-acceptance',scope:'desktop-release'}};
 const a={schema:1,kind:'backplane-desktop-target-acceptance',target:f.target,stageReceiptSha256:inspected.stageReceiptSha256,payloadManifestSha256:inspected.payloadManifestSha256};
 for(const[k,v]of Object.entries(receipts))a[k]=ref(write(path.join(f.root,'proof-'+k+'.json'),{...v,...changes[k]}));return write(path.join(f.root,'acceptance.json'),a);
}
for(const target of Object.keys(producer.TARGETS))test('offline '+target+' stage/archive layout, receipts and deterministic archive',t=>{
 // Windows ships only with the resident policy: the JS service has no managed control there.
 const f=fixture(t,target,target==='windows-x64'),dir=path.join(f.root,'stage');const staged=producer.stage(f.requestPath,dir);assert.equal(staged.status,'STAGED_UNQUALIFIED');assert.equal(staged.releaseQualified,false);assert(!fs.existsSync(path.join(dir,'DESKTOP.json')));
 const acceptance=evidence(f,dir),one=producer.archive(dir,acceptance,path.join(f.root,'dist1')),two=producer.archive(dir,acceptance,path.join(f.root,'dist2'));assert.equal(one.sha256,two.sha256);assert.equal(one.published,false);assert.equal(path.basename(one.archive),'backplane-v0.12.1-'+target+producer.TARGETS[target].suffix);
 const packaged=path.dirname(one.desktopReceipt),desktop=JSON.parse(fs.readFileSync(one.desktopReceipt));assert.equal(desktop.schema,1);assert.equal(desktop.kind,'backplane-electron-desktop-package');assert.equal(desktop.electronVersion,'44.9.2');assert.equal(desktop.serviceVersion,'0.12.1');
 assert(fs.existsSync(path.join(packaged,producer.TARGETS[target].executable)));const m=JSON.parse(fs.readFileSync(path.join(packaged,'MANIFEST.json')));assert.equal(m.files['DESKTOP.json'].sha256,hash(one.desktopReceipt));assert.equal(desktop.members['NOTICES.json'].sha256,hash(path.join(packaged,'NOTICES.json')));
 const serviceDir=path.join(packaged,producer.TARGETS[target].resources,'backplane-service');assert(fs.existsSync(path.join(serviceDir,target==='windows-x64'?'backplane-serve.exe':'backplane-serve')));
 for(const n of ['BUILD.json','WEB.json','RUNTIME.json','CLOSURE.json'])assert(fs.readFileSync(path.join(serviceDir,n)).equals(fs.readFileSync(path.join(f.service,n))));
});
test('stable release, target, source, Electron and payload mismatches fail before stage creation',t=>{
 for(const mutation of [f=>f.request.release.version='0.11.2-dev.fixture',f=>f.request.target='darwin-x64',f=>f.request.release.tree='c'.repeat(40),f=>{f.electronReceipt.electronVersion='43.0.0';f.request.electron.receipt=ref(write(path.join(f.root,'electron.json'),f.electronReceipt));f.freeze.components[f.target].electronReceiptSha256=f.request.electron.receipt.sha256;},f=>put(path.join(f.shell,'unexpected'),'extra')]){
  const f=fixture(t);mutation(f);f.refresh();const dest=path.join(f.root,'stage');assert.throws(()=>producer.stage(f.requestPath,dest));assert(!fs.existsSync(dest));
 }
});
test('missing/pending/wrong-target signing sandbox lifecycle or runtime evidence never creates a public archive',t=>{
 const f=fixture(t),dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);
 for(const change of [{signing:{status:'PENDING'}},{sandbox:{target:'linux-arm64'}},{sandbox:{flags:['--no-sandbox']}},{lifecycle:{nativeChildIdentity:false}},{runtimeAcceptance:{scope:'partial-linux-fixture'}}]){
  const file=evidence(f,dir,change),dest=path.join(f.root,'reject-'+crypto.randomUUID());assert.throws(()=>producer.archive(dir,file,dest));assert(!fs.existsSync(dest));
 }
 const a=JSON.parse(fs.readFileSync(evidence(f,dir)));delete a.signing;const file=write(path.join(f.root,'missing.json'),a);assert.throws(()=>producer.archive(dir,file,path.join(f.root,'missing-out')));
});
test('unresolved closure, missing notices and malicious payload links are refused',t=>{
 const f=fixture(t);write(path.join(f.service,'CLOSURE.json'),{target:f.target,measured:true,status:'PASS',edges:[],unresolvedMandatory:['libmissing.so']});
 f.serviceReceipt.closureSha256=hash(path.join(f.service,'CLOSURE.json'));f.serviceReceipt.members=producer.inventory(f.service,f.target);f.request.service.receipt=ref(write(path.join(f.root,'service.json'),f.serviceReceipt));f.freeze.components[f.target].serviceReceiptSha256=f.request.service.receipt.sha256;f.refresh();assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')),/closure/);
 const g=fixture(t);fs.symlinkSync('/etc/passwd',path.join(g.shell,'escape'));assert.throws(()=>producer.stage(g.requestPath,path.join(g.root,'stage')),/link/);
 const h=fixture(t);fs.unlinkSync(path.join(h.shell,'LICENSE.shell'));assert.throws(()=>producer.stage(h.requestPath,path.join(h.root,'stage')));
});
test('arbitrary post-stage application mutations cannot be excused by a signing receipt',t=>{
 const f=fixture(t),dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);put(path.join(dir,'runtime/resources/app/main.cjs'),'changed program');const file=evidence(f,dir,{signing:{signed:true}});assert.throws(()=>producer.archive(dir,file,path.join(f.root,'dist')),/Unexpected post-stage/);
});
test('request, component and acceptance references relocate across runner directories',t=>{
 const f=fixture(t);f.request.shell.payload.directory='shell';f.request.shell.receipt.path='shell.json';f.request.electron.payload.directory='electron';f.request.electron.receipt.path='electron.json';f.request.service.payload.directory='service';f.request.service.receipt.path='service.json';f.serviceReceipt.buildReceipt.path='genuine.json';
 f.request.service.receipt=ref(write(path.join(f.root,'service.json'),f.serviceReceipt));f.request.service.receipt.path='service.json';f.freeze.components[f.target].serviceReceiptSha256=f.request.service.receipt.sha256;f.refresh();f.request.release.freezeReceipt.path='freeze.json';write(f.requestPath,f.request);
 const relocated=fs.mkdtempSync(path.join(os.tmpdir(),'bp-producer-relocated-'));t.after(()=>fs.rmSync(relocated,{recursive:true,force:true}));fs.cpSync(f.root,relocated,{recursive:true});
 const dir=path.join(relocated,'stage');producer.stage(path.join(relocated,'request.json'),dir);const moved={...f,root:relocated};const file=evidence(moved,dir);const a=JSON.parse(fs.readFileSync(file));for(const key of ['signing','sandbox','lifecycle','runtimeAcceptance'])a[key].path=path.basename(a[key].path);write(file,a);
 assert.equal(producer.archive(dir,file,path.join(relocated,'dist')).target,'linux-x64');
});
test('traversal, case-colliding archive members, fonts and wrong architecture fail closed',t=>{
 for(const name of ['../escape','backplane-service/A','backplane-service/a']){
  const f=fixture(t),zip=path.join(f.root,'bad.zip');const names=name==='../escape'?[name]:['backplane-service/A','backplane-service/a'];execFileSync('python3',['-c','import sys,json,zipfile\nwith zipfile.ZipFile(sys.argv[1],"w") as z:\n for n in json.loads(sys.argv[2]): z.writestr(n,"bad")',zip,JSON.stringify(names)]);f.request.service.payload={archive:ref(zip),root:'backplane-service'};f.refresh();assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'out')));assert(!fs.existsSync(path.join(f.root,'escape')));
 }
 const f=fixture(t);put(path.join(f.shell,'font.ttf'),'font');assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'out')),/font/i);
 const g=fixture(t);put(path.join(g.electron,'electron'),binary('linux-arm64'),0o755);const runtime=JSON.parse(fs.readFileSync(path.join(g.electron,'RUNTIME.json')));runtime.inventory[0].sha256=hash(path.join(g.electron,'electron'));write(path.join(g.electron,'RUNTIME.json'),runtime);g.electronReceipt.runtimeManifestSha256=hash(path.join(g.electron,'RUNTIME.json'));g.electronReceipt.members=producer.inventory(g.electron,g.target);g.request.electron.receipt=ref(write(path.join(g.root,'electron.json'),g.electronReceipt));g.freeze.components[g.target].electronReceiptSha256=g.request.electron.receipt.sha256;g.refresh();assert.throws(()=>producer.stage(g.requestPath,path.join(g.root,'out')),/ELF/);
});

test('stable configuration and gzip witnesses are mandatory, without rewriting raw BUILD',t=>{
 const f=fixture(t);delete f.freeze.compileInputs;f.refresh();assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')),/configuration/);
 const g=fixture(t);fs.unlinkSync(path.join(g.service,'BUILD.json'));assert.throws(()=>producer.stage(g.requestPath,path.join(g.root,'stage')));
 const h=fixture(t);fs.unlinkSync(path.join(h.service,'web/index.html.gz'));const web=JSON.parse(fs.readFileSync(path.join(h.service,'WEB.json')));delete web.members['index.html.gz'];write(path.join(h.service,'WEB.json'),web);assert.throws(()=>producer.generatedWeb(path.join(h.service,'web'),web,h.request.release),/gzip/);
});

test('callable desktop CLI stages and inspects explicit input, and refuses unknown options',t=>{
 const f=fixture(t),cli=path.resolve(__dirname,'../produce-release.cjs'),dir=path.join(f.root,'cli-stage');
 assert.deepEqual(JSON.parse(execFileSync(process.execPath,[cli,'describe'])).targets,Object.keys(producer.TARGETS));
 const stage=JSON.parse(execFileSync(process.execPath,[cli,'stage','--request',f.requestPath,'--output',dir]));assert.equal(stage.status,'STAGED_UNQUALIFIED');assert.equal(JSON.parse(execFileSync(process.execPath,[cli,'inspect','--stage',dir])).payloadManifestSha256,stage.payloadManifestSha256);
 const acceptance=evidence(f,dir),archived=JSON.parse(execFileSync(process.execPath,[cli,'archive','--stage',dir,'--acceptance',acceptance,'--output',path.join(f.root,'cli-dist')]));assert.equal(archived.published,false);
 assert.throws(()=>execFileSync(process.execPath,[cli,'stage','--request',f.requestPath,'--target','linux-x64'],{stdio:'pipe'}));
});
test('Electron dependency closure is required; signed executable delta is narrowly allowed',t=>{
 const f=fixture(t);fs.unlinkSync(path.join(f.electron,'CLOSURE.json'));assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')));
 const g=fixture(t),dir=path.join(g.root,'stage');producer.stage(g.requestPath,dir);const executable=path.join(dir,'runtime/backplane-desktop');fs.appendFileSync(executable,'synthetic signed-data suffix');const acceptance=evidence(g,dir,{signing:{signed:true}});assert.equal(producer.archive(dir,acceptance,path.join(g.root,'dist')).target,'linux-x64');
});

test('explicit resident-service policy packages truthful unsupported Windows control without codesign',t=>{
 const f=fixture(t,'windows-x64',true),dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);const result=producer.archive(dir,evidence(f,dir),path.join(f.root,'dist')),desktop=JSON.parse(fs.readFileSync(result.desktopReceipt));
 assert.deepEqual(desktop.lifecyclePolicy,{mode:'resident-service',managedStop:false,managedUpdate:false});const config=JSON.parse(fs.readFileSync(path.join(path.dirname(result.desktopReceipt),'resources/DESKTOP-BUILD.json')));assert.deepEqual(config.lifecyclePolicy,desktop.lifecyclePolicy);const build=JSON.parse(fs.readFileSync(path.join(path.dirname(result.desktopReceipt),'resources/backplane-service/BUILD.json')));assert.equal(build.serviceRuntime,'bun');assert.equal(build.service,'backplane-serve.exe');assert(fs.readFileSync(path.join(path.dirname(result.desktopReceipt),'resources/backplane-service/BUILD.json')).equals(fs.readFileSync(path.join(f.service,'BUILD.json'))));
});
function rebuild(f,change){const p=path.join(f.root,'genuine.json'),b={...JSON.parse(fs.readFileSync(p)),...change};for(const k of Object.keys(change))if(change[k]===undefined)delete b[k];write(p,b);fs.copyFileSync(p,path.join(f.service,'BUILD.json'));f.serviceReceipt.buildReceipt=ref(p);f.serviceReceipt.members=producer.inventory(f.service,f.target);f.request.service.receipt=ref(write(path.join(f.root,'service.json'),f.serviceReceipt));f.freeze.components[f.target].serviceReceiptSha256=f.request.service.receipt.sha256;f.refresh();}
test('Windows takes the JS (Bun) service component; other runtimes and names are refused',t=>{
 for(const change of [{serviceRuntime:undefined},{serviceRuntime:'native-c'},{service:'backplane-serve'}]){const f=fixture(t,'windows-x64',true);rebuild(f,change);assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')),/Bun|name/);}
 const f=fixture(t,'linux-x64');rebuild(f,{serviceRuntime:'native-c'});assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')),/Bun/);
 const g=fixture(t,'linux-x64');rebuild(g,{serviceRuntime:undefined,service:undefined});assert.equal(producer.stage(g.requestPath,path.join(g.root,'stage')).status,'STAGED_UNQUALIFIED');
 const h=fixture(t,'windows-x64');assert.throws(()=>producer.stage(h.requestPath,path.join(h.root,'stage')),/managed control unsupported for the JS service/);
});
test('unsupported Windows control requires explicit complete root resident policy and no advertised controls',t=>{
 for(const mutate of [f=>delete f.freeze.lifecyclePolicy,f=>f.freeze.lifecyclePolicy[f.target].managedStop=true,f=>{f.shellReceipt.features.managedUpdate=true;f.request.shell.receipt=ref(write(path.join(f.root,'shell.json'),f.shellReceipt));f.freeze.components[f.target].shellReceiptSha256=f.request.shell.receipt.sha256;}]){
  const f=fixture(t,'windows-x64',true);mutate(f);f.refresh();assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')));
 }
});
test('resident policy keeps actual startup auth close sandbox and path evidence mandatory',t=>{
 const f=fixture(t,'windows-x64',true),dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);
 for(const change of [{lifecycle:{startup:false}},{lifecycle:{authBound:false}},{lifecycle:{closePreservesService:false}},{lifecycle:{managedStopAdvertised:true}},{lifecycle:{managedUpdateAdvertised:true}},{lifecycle:{shutdown:'approved-service-protocol'}},{lifecycle:{nativeToPosixPaths:false}},{sandbox:{osEnforced:false}}])assert.throws(()=>producer.archive(dir,evidence(f,dir,change),path.join(f.root,'reject-'+crypto.randomUUID())));
 const state=JSON.parse(fs.readFileSync(path.join(dir,'STAGE.json')));delete state.lifecyclePolicy;write(path.join(dir,'STAGE.json'),state);assert.throws(()=>producer.archive(dir,evidence(f,dir),path.join(f.root,'reject-policy')),/root freeze/);
});
test('resident-service lifecycle works on non-Windows targets and never overrides required signing',t=>{
 for(const target of ['linux-x64','linux-arm64','darwin-arm64']){
  const f=fixture(t,target,true),dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);assert.equal(producer.archive(dir,evidence(f,dir),path.join(f.root,'dist')).target,target);
 }
 const f=fixture(t,'windows-x64',true);f.freeze.signingPolicy[f.target]='required';f.refresh();const dir=path.join(f.root,'stage');producer.stage(f.requestPath,dir);assert.throws(()=>producer.archive(dir,evidence(f,dir),path.join(f.root,'dist')),/Signing/);
});

test('a Windows shell successor source is refused: the shell comes from the release commit',t=>{
 const f=fixture(t,'windows-x64',true);f.freeze.windowsShellSource={commit:'7'.repeat(40),tree:'8'.repeat(40),serviceSha256:'9'.repeat(64)};f.refresh();
 assert.throws(()=>producer.stage(f.requestPath,path.join(f.root,'stage')),/successor/);
});
