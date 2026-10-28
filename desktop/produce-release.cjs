'use strict';
// Artifact assembly only. No network, compiler, signing command or payload execution.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const browserOverlay=require('./selected-browser.cjs');
const VERSION='0.13.0',ELECTRON='44.9.2',HEX=/^[a-f0-9]{64}$/,GIT=/^[a-f0-9]{40}$/;
const TARGETS={
 'linux-x64':{suffix:'.tar.gz',root:'backplane-linux-x64',executable:'runtime/backplane-desktop',resources:'runtime/resources'},
 'linux-arm64':{suffix:'.tar.gz',root:'backplane-linux-arm64',executable:'runtime/backplane-desktop',resources:'runtime/resources'},
 'darwin-arm64':{suffix:'.tar.gz',root:'backplane-darwin-arm64',executable:'Backplane.app/Contents/MacOS/Backplane',resources:'Backplane.app/Contents/Resources'},
 'windows-x64':{suffix:'.zip',root:'backplane-windows-x64',executable:'Backplane.exe',resources:'resources'},
};
function linuxLauncher(){return String.raw`#!/bin/sh
set -eu
BP_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
BP_SANDBOX="$BP_ROOT/runtime/chrome-sandbox"
if command -v unshare >/dev/null 2>&1 && unshare --user --map-root-user /bin/true >/dev/null 2>&1; then
  exec "$BP_ROOT/runtime/backplane-desktop" "$@"
fi
if [ -f "$BP_SANDBOX" ] && [ ! -L "$BP_SANDBOX" ] && [ "$(stat -c '%u:%g:%a' -- "$BP_SANDBOX" 2>/dev/null)" = '0:0:4755' ]; then
  exec "$BP_ROOT/runtime/backplane-desktop" "$@"
fi
printf '%s\n' 'Backplane cannot start: the Linux OS sandbox needs working unprivileged user namespaces or a root-owned mode-4755 chrome-sandbox helper.' >&2
printf '%s\n' 'Ask your administrator to run this command for this installation:' >&2
BP_QUOTED=$(printf '%s' "$BP_SANDBOX" | sed "s/'/'\\\\''/g")
printf "sudo chown root:root '%s' && sudo chmod 4755 '%s'\n" "$BP_QUOTED" "$BP_QUOTED" >&2
exit 1
`;}
const fail=message=>{throw Error(message);};
const check=(value,message)=>{if(!value)fail(message);};
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const digest=p=>sha(fs.readFileSync(p));
const bases=new WeakMap();
function json(p){const filename=path.resolve(p),value=JSON.parse(fs.readFileSync(filename,'utf8'));const bind=o=>{if(!o||typeof o!=='object')return;bases.set(o,path.dirname(filename));for(const child of Object.values(o))bind(child);};bind(value);return value;}
const ordered=value=>Array.isArray(value)?value.map(ordered):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(k=>[k,ordered(value[k])])):value;
const canonical=value=>JSON.stringify(ordered(value));
const write=(p,value)=>fs.writeFileSync(p,JSON.stringify(value,null,2)+'\n');
function relative(name){
 check(typeof name==='string'&&name.length>0&&!name.includes('\\')&&!name.includes(':')&&!name.includes('\0')&&!name.startsWith('/')&&name.split('/').every(p=>p&&p!=='.'&&p!=='..'),'Unsafe member path');return name;
}
function inside(root,name){return path.join(root,...relative(name).split('/'));}
function reference(ref){
 check(ref&&typeof ref.path==='string'&&ref.path&&HEX.test(ref.sha256)&&Number.isSafeInteger(ref.bytes)&&ref.bytes>=0,'Invalid artifact reference');
 const filename=path.resolve(bases.get(ref)||process.cwd(),ref.path);check(fs.lstatSync(filename).isFile()&&fs.statSync(filename).size===ref.bytes&&digest(filename)===ref.sha256,'Artifact hash/size mismatch');return filename;
}
function readReference(ref){return json(reference(ref));}
function runPython(args){execFileSync(process.env.BACKPLANE_PRODUCER_PYTHON||'python3',[path.join(__dirname,'release-files.py'),...args],{stdio:['ignore','pipe','pipe'],maxBuffer:1024*1024});}
function inventory(root,target){
 const result={},seen=new Set();
 function walk(dir){for(const name of fs.readdirSync(dir).sort()){
  const p=path.join(dir,name),rel=path.relative(root,p).split(path.sep).join('/');relative(rel);
  check(!seen.has(rel.toLowerCase()),'Case-colliding payload member');seen.add(rel.toLowerCase());
  check(!/(^|\/)(?:\.env(?:\..*)?|token|events\.jsonl|api-keys\.json|keyring|browser-profile|electron-profile|fontconfig)(\/|$)/i.test(rel),'Private/cache payload member');
  const s=fs.lstatSync(p);check(s.isSymbolicLink()||!(s.mode&0o7000),'Privileged payload mode');
  if(s.isSymbolicLink()){
   check(target!=='windows-x64','Windows symlink/reparse payload');const link=fs.readlinkSync(p);check(!path.isAbsolute(link)&&!link.includes('\\')&&!link.includes(':'),'Unsafe link');
   const resolved=path.resolve(path.dirname(p),link);check(resolved.startsWith(path.resolve(root)+path.sep),'Escaping payload link');check(fs.existsSync(p),'Dangling/cyclic payload link');
   result[rel]={type:'symlink',target:link};
  }else if(s.isDirectory())walk(p);
  else{
   check(s.isFile(),'Special payload member');check(!(s.mode&0o7000),'Privileged payload mode');
   check(!/\.(?:ttf|otf|woff2?|ttc)$/i.test(name),'Bundled font payload');
   const bytes=fs.readFileSync(p),magic=bytes.subarray(0,4).toString('hex');check(!['00010000','4f54544f','774f4646','774f4632','74746366'].includes(magic),'Font-format payload');
   result[rel]={type:'file',sha256:sha(bytes),bytes:s.size,mode:s.mode&0o777};
  }
 }}walk(root);return result;
}
function membershipExpectation(members,target,platform=process.platform){
 if(target!=='windows-x64'||platform!=='win32')return members;
 // Native Windows reports writable/read-only regular files as 0666/0444.
 // Original POSIX modes stay in their source receipts; only this comparison
 // projects the owner-write bit into the representation NTFS exposes to Node.
 return Object.fromEntries(Object.entries(members).map(([name,v])=>{
  check(v&&v.type==='file','Windows symlink/reparse payload');
  check(Number.isInteger(v.mode)&&v.mode>=0&&v.mode<=0o777,'Invalid/privileged declared payload mode');
  return [name,{...v,mode:(v.mode&0o200)?0o666:0o444}];
 }));
}
function membership(root,members,target){
 check(members&&typeof members==='object'&&!Array.isArray(members),'Missing payload inventory');
 const actual=inventory(root,target);check(canonical(actual)===canonical(membershipExpectation(members,target)),'Payload membership/hash/mode mismatch');return actual;
}
function identity(r,release,target,kind){
 check(r&&r.schema===1&&r.kind===kind&&r.status==='VERIFIED','Unverified component receipt');
 check(r.target===target&&r.sourceCommit===release.commit&&r.sourceTree===release.tree&&r.version===VERSION&&r.channel==='release','Component source/version/target mismatch');
}
// Every target's shell comes from the release commit itself. The 0.12.0
// Windows launcher successor (a frozen older shell source) is gone: a freeze
// that still names one is refused.
function shellIdentity(r,release,target,freeze){
 check(freeze.windowsShellSource===undefined,'Windows shell successor sources are no longer accepted; the shell comes from the release commit');
 return identity(r,release,target,'backplane-electron-shell');
}
function architecture(file,target){
 const f=fs.openSync(file,'r'),b=Buffer.alloc(512);let n;try{n=fs.readSync(f,b,0,b.length,0);}finally{fs.closeSync(f);}
 if(target.startsWith('linux-'))check(n>=20&&b.subarray(0,4).toString('hex')==='7f454c46'&&b[4]===2&&b[5]===1&&b.readUInt16LE(18)===(target==='linux-x64'?62:183),'Wrong ELF target');
 else if(target==='darwin-arm64')check(n>=12&&b.readUInt32LE(0)===0xfeedfacf&&b.readUInt32LE(4)===0x100000c,'Wrong Mach-O target');
 else{check(n>=64&&b.toString('ascii',0,2)==='MZ','Not a PE executable');const offset=b.readUInt32LE(60),head=Buffer.alloc(6),fd=fs.openSync(file,'r');try{check(fs.readSync(fd,head,0,6,offset)===6,'Invalid PE header');}finally{fs.closeSync(fd);}check(head.toString('ascii',0,4)==='PE\0\0'&&head.readUInt16LE(4)===0x8664,'Wrong PE target');}
}
function payload(spec,temp,name){
 check(spec&&spec.receipt&&spec.payload,'Missing component input');const receipt=readReference(spec.receipt);let root;
 if(spec.payload.directory){check(typeof spec.payload.directory==='string','Invalid payload directory');root=path.resolve(bases.get(spec.payload)||process.cwd(),spec.payload.directory);check(fs.lstatSync(root).isDirectory(),'Invalid payload directory');}
 else{const file=reference(spec.payload.archive),dest=path.join(temp,name);runPython(['extract',file,dest]);root=spec.payload.root?inside(dest,spec.payload.root):dest;check(fs.lstatSync(root).isDirectory(),'Missing archive root');}
 return{root,receipt,ref:spec.receipt};
}
function notices(root,manifest){
 check(manifest&&Array.isArray(manifest.packages)&&manifest.packages.length>0,'Missing dependency notices');
 for(const p of manifest.packages){check(typeof p.name==='string'&&p.name&&typeof p.version==='string'&&p.version&&Array.isArray(p.licenses)&&p.licenses.length>0,'Incomplete package notice');
  check(p.sourceDisposition&&typeof p.sourceDisposition==='string','Missing redistribution/source disposition');
  for(const n of p.licenses){check(HEX.test(n.sha256)&&digest(inside(root,n.path))===n.sha256,'Notice missing/hash mismatch');}
 }
}
function closure(root,receipt,target){
 const runtime=inside(root,receipt.runtimeManifest),dep=inside(root,receipt.closureManifest);check(digest(runtime)===receipt.runtimeManifestSha256&&digest(dep)===receipt.closureSha256,'Runtime/closure receipt mismatch');
 const r=json(runtime),c=json(dep);check(r.target===target&&c.target===target&&c.measured===true,'Unmeasured/wrong-target runtime closure');check(r&&Array.isArray(r.inventory)&&r.inventory.length>0,'Missing runtime inventory');
 for(const v of r.inventory){check(typeof v.package==='string'&&typeof v.version==='string'&&typeof v.architecture==='string','Missing runtime identity');check(digest(inside(root,v.path))===v.sha256&&fs.statSync(inside(root,v.path)).size===v.bytes,'Runtime member mismatch');}
 check(c.status==='PASS'&&Array.isArray(c.edges)&&Array.isArray(c.unresolvedMandatory)&&c.unresolvedMandatory.length===0,'Unresolved runtime dependency closure');
 for(const e of c.edges){relative(e.from);check(fs.existsSync(inside(root,e.from)),'Missing closure source');check(['normal','delay','dynamic'].includes(e.kind)&&typeof e.name==='string'&&e.name,'Incomplete closure edge');if(e.system===true)check(e.classification==='approved-system','Unclassified system dependency');else check(typeof e.resolved==='string'&&fs.existsSync(inside(root,e.resolved))&&fs.statSync(inside(root,e.resolved)).isFile(),'Unresolved mandatory non-system edge');}
 notices(root,r);
}
function configuredService(build,freeze){
 check((build.entrypoint||build.entry)==='src/server/main.bend'&&(build.serviceVersion||build.version)===VERSION&&build.versionStamp===VERSION&&build.telemetryChannel==='release','Missing configured stable service identity');
 const config=build.compileInputs;
 check(config&&config.version===VERSION&&config.versionStamp===VERSION&&config.telemetryChannel==='release','Missing configured compile identity');
 for(const name of ['google','telemetry'])check(config[name]?.requested===true&&config[name]?.requiredInputsPresent===true,'Missing configured Google/telemetry bake witness');
 check(Object.keys(config).sort().join(',')==='google,telemetry,telemetryChannel,version,versionStamp'&&Object.keys(config.google).sort().join(',')==='requested,requiredInputsPresent'&&Object.keys(config.telemetry).sort().join(',')==='endpoint,requested,requiredInputsPresent','Compile witness must contain only the public configuration tuple');
 check(config.telemetry.endpoint==='https://backplaneworks.com/api/v1/events','Wrong/missing public telemetry endpoint');
 check(canonical(config)===canonical(freeze.compileInputs),'Compile inputs differ from frozen public configuration');
}
function generatedWeb(root,manifest,release){
 check(manifest.schema===1&&manifest.kind==='matching-generated-web'&&manifest.sourceCommit===release.commit&&manifest.sourceTree===release.tree&&manifest.entrypoint==='src/web/index.html','Wrong matching web receipt');
 const actual=inventory(root,'linux-x64');check(Object.keys(actual).every(k=>actual[k].type==='file'),'Web links are forbidden');
 check(Object.keys(actual).sort().join('\n')===Object.keys(manifest.members||{}).sort().join('\n'),'Web membership mismatch');
 check(manifest.members['index.html'],'Missing web index');
 for(const [name,v]of Object.entries(manifest.members)){relative(name);if(/\.(?:html|js|css)$/.test(name))check(manifest.members[name+'.gz'],'Missing gzip sibling');check(actual[name].sha256===v.sha256&&actual[name].bytes===v.bytes,'Web hash/size mismatch');if(name.endsWith('.gz')){const plain=inside(root,name.slice(0,-3));check(fs.existsSync(plain)&&require('node:zlib').gunzipSync(fs.readFileSync(inside(root,name))).equals(fs.readFileSync(plain)),'Invalid gzip sibling');}}
}
function serviceFile(target){return target==='windows-x64'?'backplane-serve.exe':'backplane-serve';}
function residentPolicy(policy){
 return policy?.mode==='resident-service'&&policy.managedStop===false&&policy.managedUpdate===false;
}
function validateRequest(request,temp){
 check(request.schema===1&&request.kind==='backplane-electron-release-inputs'&&TARGETS[request.target],'Unsupported request/target');const target=request.target,release=request.release;
 check(release&&release.version===VERSION&&release.tag==='v'+VERSION&&release.channel==='release'&&GIT.test(release.commit)&&GIT.test(release.tree),'Actual stable release freeze required');
 const freeze=readReference(release.freezeReceipt);check(freeze.schema===1&&freeze.kind==='backplane-release-freeze'&&freeze.status==='FROZEN'&&['version','tag','channel','commit','tree'].every(k=>freeze[k]===release[k])&&Number.isSafeInteger(freeze.sourceDateEpoch)&&freeze.sourceDateEpoch>=315532800&&freeze.sourceDateEpoch<=4354819199,'Frozen source identity mismatch');
 const pins=freeze.components?.[target];check(pins,'Target missing from release freeze');
 const shell=payload(request.shell,temp,'shell'),electron=payload(request.electron,temp,'electron');
 check(pins.shellReceiptSha256===shell.ref.sha256&&pins.electronReceiptSha256===electron.ref.sha256,'Shell/Electron not in freeze');shellIdentity(shell.receipt,release,target,freeze);identity(electron.receipt,release,target,'backplane-electron-runtime');
 membership(shell.root,shell.receipt.members,target);membership(electron.root,electron.receipt.members,target);
 check(electron.receipt.electronVersion===ELECTRON,'Wrong Electron version');check(shell.receipt.electronVersion===ELECTRON,'Shell Electron pin mismatch');
 const pkg=json(path.join(shell.root,'package.json'));check(pkg.version===VERSION&&pkg.main==='main.cjs'&&pkg.name==='backplane-desktop','Packaged shell version/entry mismatch');
 check(shell.receipt.features?.defaultPackagedHome===true&&shell.receipt.features?.contextIsolation===true&&shell.receipt.features?.sandbox===true&&shell.receipt.features?.nodeIntegration===false,'Missing shell lifecycle/isolation source qualification');
 notices(shell.root,shell.receipt);notices(electron.root,electron.receipt);closure(electron.root,electron.receipt,target);
 // Every target's service is the same JS (Bun) component; Windows names it backplane-serve.exe.
 const exe=serviceFile(target),service=payload(request.service,temp,'service');check(pins.serviceReceiptSha256===service.ref.sha256,'Service not in freeze');identity(service.receipt,release,target,'backplane-bend-service');
 check(service.receipt.genuineService===true&&service.receipt.entrypoint==='src/server/main.bend'&&service.receipt.roles?.service===exe&&service.receipt.roles.web==='web','Not a genuine service entry');
 const build=readReference(service.receipt.buildReceipt);check(/^VERIFIED_GENUINE/.test(build.status)&&build.target===target&&build.entry==='src/server/main.bend'&&build.version===VERSION&&build.source_tree===release.tree&&build.source_commit===release.commit&&build.binary_sha256===digest(path.join(service.root,exe)),'Missing genuine target build provenance');
 check(target==='windows-x64'?build.serviceRuntime==='bun':build.serviceRuntime===undefined||build.serviceRuntime==='bun','Windows requires the JS (Bun) service runtime');check(build.service===undefined||build.service===exe,'Service file name differs from its build');
 check(digest(path.join(service.root,'BUILD.json'))===service.receipt.buildReceipt.sha256,'Packaged BUILD must preserve the actual genuine build receipt');configuredService(build,freeze);
 membership(service.root,service.receipt.members,target);closure(service.root,service.receipt,target);notices(service.root,service.receipt);architecture(path.join(service.root,exe),target);
 if(target==='windows-x64'){
  // The JS service has no Windows managed-control protocol: only the resident policy (no managed stop/update) ships.
  check(residentPolicy(freeze.lifecyclePolicy?.[target]),'Windows managed control unsupported for the JS service');
  check(shell.receipt.features.windowsManagedControl==='UNSUPPORTED','Resident shell must preserve unsupported Windows control');
  check(shell.receipt.features.windowsNativePathMapping===true,'Shell lacks Windows startup path mapping');
 }
 check(pins.webManifestSha256===digest(path.join(service.root,'WEB.json')),'Matching web not in release freeze');generatedWeb(path.join(service.root,'web'),json(path.join(service.root,'WEB.json')),release);
 if(freeze.lifecyclePolicy?.[target]){check(residentPolicy(freeze.lifecyclePolicy[target]),'Unknown/incomplete lifecycle policy');check(shell.receipt.features.managedStop===false&&shell.receipt.features.managedUpdate===false,'Resident shell advertises unsupported stop/update');}
 const upstreamExe=target==='darwin-arm64'?'Electron.app/Contents/MacOS/Electron':target==='windows-x64'?'electron.exe':'electron';architecture(inside(electron.root,upstreamExe),target);
 check(freeze.signingPolicy?.[target]&&['required','unsigned-portable-allowed'].includes(freeze.signingPolicy[target]),'Missing target signing policy');
 const validated={request,target,release,freeze,shell,electron,service};
 // Every original native/service/matching-web guard above runs before this optional branch.
 const selectedBrowser=browserOverlay.validate(request.selectedBrowser,validated,temp,browserUtilities());
 return{...validated,...(selectedBrowser?{selectedBrowser}:{})};
}
function browserUtilities(){return{payload,json,digest,membership,membershipExpectation,generatedWeb,notices,inventory,copy,canonical,readReference,reference,contentDigest,TARGETS};}
function copy(source,dest,preserveModes=false){fs.mkdirSync(path.dirname(dest),{recursive:true});fs.cpSync(source,dest,{recursive:true,dereference:false,verbatimSymlinks:true,errorOnExist:true,force:false,...(preserveModes?{filter:()=>true}:{})});}
function publicReference(ref,name){return{path:'PROVENANCE/'+name,sha256:ref.sha256,bytes:ref.bytes};}
// A Windows stage's content is digested as NTFS shows it (0666 writable, 0444
// read-only) on every host, so a stage made on Linux inspects the same on the
// Windows target it is smoked on (a .zip keeps no POSIX modes anyway).
function content(stage,target){const all=inventory(stage,target);for(const n of Object.keys(all))if(['STAGE.json','DESKTOP.json','MANIFEST.json'].includes(n)||n.startsWith('PROVENANCE/'))delete all[n];return target==='windows-x64'?membershipExpectation(all,target,'win32'):all;}
function contentDigest(members){return sha(Buffer.from(canonical(members)));}
function stage(requestFile,output){
 check(path.isAbsolute(output)&&!fs.existsSync(output),'Stage must be a new absolute directory');const temp=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-producer-'));
 try{
  const v=validateRequest(json(requestFile),temp),{target,release,shell,electron,service,freeze,selectedBrowser}=v,t=TARGETS[target];fs.mkdirSync(output,{recursive:true});
  if(target.startsWith('linux-')){copy(electron.root,path.join(output,'runtime'));fs.renameSync(path.join(output,'runtime/electron'),inside(output,t.executable));}
  else if(target==='windows-x64'){for(const n of fs.readdirSync(electron.root))copy(path.join(electron.root,n),path.join(output,n));fs.renameSync(path.join(output,'electron.exe'),inside(output,t.executable));}
  else{
   copy(path.join(electron.root,'Electron.app'),path.join(output,'Backplane.app'));fs.renameSync(path.join(output,'Backplane.app/Contents/MacOS/Electron'),inside(output,t.executable));
   check(typeof shell.receipt.bundleIdentifier==='string'&&/^[A-Za-z0-9.-]+$/.test(shell.receipt.bundleIdentifier),'Missing macOS bundle identifier');runPython(['plist',path.join(output,'Backplane.app/Contents/Info.plist'),shell.receipt.bundleIdentifier,VERSION]);
   // Electron's license/Chromium notices live beside Electron.app upstream.
   for(const n of Object.keys(electron.receipt.members))if(!n.startsWith('Electron.app/')&&electron.receipt.members[n].type==='file')copy(inside(electron.root,n),inside(output,'NOTICES/electron/'+n));
  }
  const resources=inside(output,t.resources);fs.mkdirSync(resources,{recursive:true});check(!fs.existsSync(path.join(resources,'app'))&&!fs.existsSync(path.join(resources,'backplane-service')),'Runtime contains conflicting product resources');
  copy(shell.root,path.join(resources,'app'));copy(service.root,path.join(resources,'backplane-service'));
  membership(path.join(resources,'app'),shell.receipt.members,target);membership(path.join(resources,'backplane-service'),service.receipt.members,target);
  browserOverlay.install(selectedBrowser,path.join(resources,'backplane-service'),target,browserUtilities());
  for(const [n,expected]of Object.entries(membershipExpectation(electron.receipt.members,target))){
   if(target==='darwin-arm64'&&n==='Electron.app/Contents/Info.plist')continue;
   const mapped=target.startsWith('linux-')?'runtime/'+(n==='electron'?'backplane-desktop':n):target==='windows-x64'?(n==='electron.exe'?'Backplane.exe':n):n.startsWith('Electron.app/')?'Backplane.app/'+n.slice(13).replace(/^Contents\/MacOS\/Electron$/,'Contents/MacOS/Backplane'):'NOTICES/electron/'+n;
   const dest=inside(output,mapped),st=fs.lstatSync(dest);
   if(expected.type==='symlink')check(st.isSymbolicLink()&&fs.readlinkSync(dest)===expected.target,'Copied runtime link changed');else check(st.isFile()&&digest(dest)===expected.sha256&&st.size===expected.bytes&&(st.mode&0o777)===expected.mode,'Copied runtime bytes/mode changed');
  }
  // Brand the assembled runtime only after verifying upstream bytes, before signing.
  // Older frozen shell receipts without these assets retain their existing contract.
  if(target==='darwin-arm64'&&fs.existsSync(path.join(shell.root,'backplane.icns'))){
   copy(path.join(shell.root,'backplane.icns'),path.join(output,'Backplane.app/Contents/Resources/Backplane.icns'));
   runPython(['plist-icon',path.join(output,'Backplane.app/Contents/Info.plist'),'Backplane.icns']);
  }
  if(target==='windows-x64'&&fs.existsSync(path.join(shell.root,'backplane.ico'))){
   const brand=path.join(__dirname,'brand-windows.py');
   const r=require('node:child_process').spawnSync(process.env.PYTHON||'python3',[brand,inside(output,t.executable),path.join(shell.root,'backplane.ico')],{encoding:'utf8'});
   check(r.status===0,'Windows icon resource update failed: '+r.stderr);
  }
  const build={serviceVersion:VERSION,electronVersion:ELECTRON,sourceTree:release.tree,sourceCommit:release.commit,...(freeze.lifecyclePolicy?.[target]?{lifecyclePolicy:freeze.lifecyclePolicy[target]}:{}),qualification:'staged only; post-assembly signing/sandbox/lifecycle/runtime evidence required'};
  check(!fs.existsSync(path.join(resources,'DESKTOP-BUILD.json')),'Conflicting desktop resource metadata');
  // The service's BUILD.json is immutable; Electron's own build configuration is separate.
  write(path.join(resources,'DESKTOP-BUILD.json'),build);
  if(shell.receipt.features.directBrowser){build.desktopBrowser={protocol:1,guestManagerSha256:digest(path.join(resources,'app/browser-guest.cjs')),preloadSha256:digest(path.join(resources,'app/browser-recording-preload.cjs'))};write(path.join(resources,'DESKTOP-BUILD.json'),build);}
  if(target.startsWith('linux-'))fs.writeFileSync(path.join(output,'backplane-open'),linuxLauncher(),{mode:0o755});
  const provenance=path.join(output,'PROVENANCE');fs.mkdirSync(provenance);const refs={};
  for(const [name,ref]of Object.entries({freeze:release.freezeReceipt,shell:shell.ref,electron:electron.ref,service:service.ref})){fs.copyFileSync(reference(ref),path.join(provenance,name+'.json'));refs[name]=publicReference(ref,name+'.json');}
  if(service.receipt.buildReceipt){const ref=service.receipt.buildReceipt;fs.copyFileSync(reference(ref),path.join(provenance,'service-build.json'));refs.serviceBuild=publicReference(ref,'service-build.json');}
  if(selectedBrowser){const authority=selectedBrowser.rootDisposition;fs.copyFileSync(reference(authority),path.join(provenance,'root-staging-disposition.json'));refs.rootStagingDisposition=publicReference(authority,'root-staging-disposition.json');fs.copyFileSync(reference(selectedBrowser.ref),path.join(provenance,'selected-browser.json'));refs.selectedBrowser=publicReference(selectedBrowser.ref,'selected-browser.json');const file=selectedBrowser.manifestFile,ref={path:file,sha256:digest(file),bytes:fs.statSync(file).size};fs.copyFileSync(file,path.join(provenance,'selected-browser-web.json'));refs.selectedBrowserWeb=publicReference(ref,'selected-browser-web.json');}
  const noticesManifest={schema:1,kind:'backplane-desktop-notices',target,packages:[]};
  for(const [part,obj,prefix]of [['shell',shell,t.resources+'/app/'],['electron',electron,target.startsWith('linux-')?'runtime/':target==='windows-x64'?'':''],['service',service,t.resources+'/backplane-service/']]){
   const packages=part==='service'?[...obj.receipt.packages,...json(path.join(obj.root,obj.receipt.runtimeManifest)).packages]:obj.receipt.packages;
   for(const p of packages){const licenses=p.licenses.map(n=>({...n,path:part==='electron'&&target==='darwin-arm64'?(n.path.startsWith('Electron.app/')?'Backplane.app/'+n.path.slice(13):'NOTICES/electron/'+n.path):prefix+n.path}));noticesManifest.packages.push({...p,component:part,licenses});}
  }write(path.join(output,'NOTICES.json'),noticesManifest);notices(output,noticesManifest);
  const signable=electron.receipt.signablePaths||[];check(Array.isArray(signable)&&signable.every(n=>electron.receipt.members[n]?.type==='file'),'Invalid runtime signing paths');for(const n of signable)architecture(inside(electron.root,n),target);
  const record={schema:1,kind:'backplane-electron-desktop-stage',status:'STAGED_UNQUALIFIED',target,release:{...release,freezeReceipt:refs.freeze},electronVersion:ELECTRON,serviceVersion:VERSION,shellVersion:VERSION,entrypoint:t.executable,backendEntrypoint:'src/server/main.bend',components:refs,...(freeze.windowsShellSource?{windowsShellSource:freeze.windowsShellSource}:{}),webManifestSha256:selectedBrowser?selectedBrowser.descriptor.webManifestSha256:digest(path.join(service.root,'WEB.json')),...(selectedBrowser?{resourceWeb:{sourceCommit:release.commit,sourceTree:release.tree,webManifestSha256:selectedBrowser.descriptor.resourceWebManifestSha256},selectedBrowser:selectedBrowser.descriptor}:{}),noticesManifestSha256:digest(path.join(output,'NOTICES.json')),sourceDateEpoch:freeze.sourceDateEpoch,signingPolicy:freeze.signingPolicy[target],...(freeze.lifecyclePolicy?.[target]?{lifecyclePolicy:freeze.lifecyclePolicy[target]}:{}),signablePaths:signable.map(n=>target.startsWith('linux-')?'runtime/'+(n==='electron'?'backplane-desktop':n):target==='windows-x64'?(n==='electron.exe'?'Backplane.exe':n):n.replace(/^Electron\.app\//,'Backplane.app/').replace('/MacOS/Electron','/MacOS/Backplane'))};
  // signable is receipt metadata, not part of the exact filesystem inventory.
  record.members=content(output,target);record.payloadManifestSha256=contentDigest(record.members);write(path.join(output,'STAGE.json'),record);
  return{status:record.status,target,stage:output,stageReceiptSha256:digest(path.join(output,'STAGE.json')),payloadManifestSha256:record.payloadManifestSha256,releaseQualified:false};
 }finally{fs.rmSync(temp,{recursive:true,force:true});}
}
function archive(stageDir,acceptanceFile,output){
 check(path.isAbsolute(output)&&!fs.existsSync(output),'Distribution output must be new');const stageFile=path.join(stageDir,'STAGE.json'),s=json(stageFile);check(s.schema===1&&s.kind==='backplane-electron-desktop-stage'&&s.status==='STAGED_UNQUALIFIED'&&TARGETS[s.target]&&s.release.version===VERSION,'Invalid stable stage');
 const a=json(acceptanceFile),actual=content(stageDir,s.target),payloadHash=contentDigest(actual);check(a.schema===1&&['backplane-desktop-target-acceptance','backplane-desktop-browser-composite-acceptance'].includes(a.kind)&&a.target===s.target&&a.stageReceiptSha256===digest(stageFile)&&a.payloadManifestSha256===payloadHash,'Acceptance does not bind actual target/final payload');
 check(s.release.tag==='v'+VERSION&&s.release.channel==='release'&&GIT.test(s.release.commit)&&GIT.test(s.release.tree)&&s.payloadManifestSha256===contentDigest(s.members),'Invalid stage source/version binding');
 check(!fs.existsSync(path.join(stageDir,'DESKTOP.json'))&&!fs.existsSync(path.join(stageDir,'MANIFEST.json')),'Stage contains conflicting package receipts');
 check(s.entrypoint===TARGETS[s.target].executable,'Wrong target launch path');architecture(inside(stageDir,s.entrypoint),s.target);
 const sourceFiles=Object.values(s.components).filter(r=>r.path).map(r=>r.path.slice('PROVENANCE/'.length)).sort();check(fs.readdirSync(path.join(stageDir,'PROVENANCE')).sort().join('\n')===sourceFiles.join('\n'),'Unexpected provenance payload');
 const frozen=readReference({...s.components.freeze,path:inside(stageDir,s.components.freeze.path)});check(canonical(s.windowsShellSource??null)===canonical(frozen.windowsShellSource??null),'Windows shell provenance must match root freeze');shellIdentity(readReference({...s.components.shell,path:inside(stageDir,s.components.shell.path)}),s.release,s.target,frozen);check(canonical(s.lifecyclePolicy??null)===canonical(frozen.lifecyclePolicy?.[s.target]??null),'Lifecycle policy must match root freeze');check(s.signingPolicy===frozen.signingPolicy?.[s.target],'Signing policy must match root freeze');
 let composite=null,selectedBrowserRuntime=null;
 if(s.selectedBrowser){
  browserOverlay.rootAuthority({...s.components.rootStagingDisposition,path:inside(stageDir,s.components.rootStagingDisposition.path)},{target:s.target,release:s.release,freeze:frozen},browserUtilities());
  check(frozen.selectedBrowser&&canonical(s.selectedBrowser)===canonical({mode:frozen.selectedBrowser.mode,sourceCommit:frozen.selectedBrowser.sourceCommit,sourceTree:frozen.selectedBrowser.sourceTree,receipt:frozen.selectedBrowser.receipt,webManifestSha256:frozen.selectedBrowser.webManifestSha256,resourceWebManifestSha256:frozen.selectedBrowser.resourceWebManifestSha256,rootStagingDisposition:frozen.rootStagingDisposition,...(frozen.selectedBrowser.baselineDesktopReceiptSha256?{baselineDesktopReceiptSha256:frozen.selectedBrowser.baselineDesktopReceiptSha256}:{})}),'Selected browser stage/freeze mismatch');
  check(s.components.selectedBrowser.sha256===s.selectedBrowser.receipt.sha256&&s.components.selectedBrowserWeb.sha256===s.selectedBrowser.webManifestSha256&&s.webManifestSha256===s.selectedBrowser.webManifestSha256,'Selected browser provenance mismatch');
  const serviceRoot=inside(stageDir,TARGETS[s.target].resources+'/backplane-service');check(digest(path.join(serviceRoot,'WEB.json'))===s.resourceWeb.webManifestSha256&&s.resourceWeb.webManifestSha256===frozen.components[s.target].webManifestSha256,'Original resourceWeb receipt changed');
  generatedWeb(path.join(serviceRoot,'web'),readReference({...s.components.selectedBrowserWeb,path:inside(stageDir,s.components.selectedBrowserWeb.path)}),{commit:s.selectedBrowser.sourceCommit,tree:s.selectedBrowser.sourceTree});
  notices(path.join(serviceRoot,'web'),readReference({...s.components.selectedBrowser,path:inside(stageDir,s.components.selectedBrowser.path)}));
  selectedBrowserRuntime=browserOverlay.currentRuntime(a.selectedBrowserRuntime,{...s,stageReceiptSha256:digest(stageFile)},payloadHash,browserUtilities());
 }else check(!frozen.selectedBrowser&&!a.selectedBrowserRuntime,'Unexpected selected browser acceptance');
 if(a.kind==='backplane-desktop-browser-composite-acceptance')composite=browserOverlay.composite(a,s,actual,frozen,browserUtilities());
 const proofs=composite?composite.proofs:{};if(!composite)for(const k of ['signing','sandbox','lifecycle','runtimeAcceptance']){
  const ref=a[k],r=readReference(ref);check(r.schema===1&&r.status==='PASS'&&r.target===s.target&&r.version===VERSION&&r.commit===s.release.commit&&r.tree===s.release.tree&&r.payloadManifestSha256===payloadHash&&r.freezeReceiptSha256===s.release.freezeReceipt.sha256,'Missing/wrong-target/pending '+k+' evidence');proofs[k]={ref,receipt:r};
 }
 check(proofs.signing.receipt.kind==='desktop-signing'&&typeof proofs.signing.receipt.signed==='boolean'&&(s.signingPolicy!=='required'||proofs.signing.receipt.signed),'Signing required or undeclared');
 check(proofs.sandbox.receipt.kind==='desktop-sandbox'&&proofs.sandbox.receipt.osEnforced===true&&Array.isArray(proofs.sandbox.receipt.flags)&&!proofs.sandbox.receipt.flags.includes('--no-sandbox'),'Unqualified OS sandbox');
 const lifecycle=proofs.lifecycle.receipt;check(lifecycle.kind==='desktop-lifecycle'&&lifecycle.nativeChildIdentity===true&&lifecycle.authBound===true&&lifecycle.singleInstance===true&&lifecycle.reconnect===true&&lifecycle.windowClose==='resident-service','Unqualified service lifecycle');
 if(s.lifecyclePolicy){check(residentPolicy(s.lifecyclePolicy),'Unknown lifecycle policy');check(lifecycle.startup===true&&lifecycle.closePreservesService===true&&lifecycle.shutdown==='unsupported'&&lifecycle.managedStopAdvertised===false&&lifecycle.managedUpdateAdvertised===false,'Unqualified resident-service lifecycle');}
 else{check(s.target!=='windows-x64','Windows managed control unsupported for the JS service');check(lifecycle.shutdown==='owned-native-child','Unsupported target shutdown');}
 if(s.target==='windows-x64')check(lifecycle.nativeToPosixPaths===true,'Unqualified Windows path mapping');
 check(proofs.runtimeAcceptance.receipt.kind==='desktop-runtime-acceptance'&&proofs.runtimeAcceptance.receipt.scope==='desktop-release','Missing full target desktop acceptance');
 const changed=[...new Set([...Object.keys(s.members),...Object.keys(actual)])].filter(n=>canonical(s.members[n])!==canonical(actual[n]));
 for(const n of changed){const signature=s.target==='darwin-arm64'&&/^Backplane\.app\/(?:.*\/)?_CodeSignature\/[^/]+$/.test(n);check(signature||s.signablePaths.includes(n),'Unexpected post-stage payload mutation');check(actual[n]?.type==='file'&&(signature||s.members[n]?.type==='file'),'Signing cannot remove files or add links');if(!signature){check(actual[n].mode===s.members[n].mode,'Signing changed executable mode');architecture(inside(stageDir,n),s.target);}check(!n.startsWith(TARGETS[s.target].resources+'/app/')&&!n.startsWith(TARGETS[s.target].resources+'/backplane-service/'),'Signed mutation cannot rewrite app/service/web');}
 check(changed.length===0||proofs.signing.receipt.signed,'Unsigned payload changed after staging');
 for(const [name,ref]of Object.entries(s.components))if(ref.path)reference({...ref,path:inside(stageDir,ref.path)});
 const finalRoot=path.join(output,TARGETS[s.target].root);fs.mkdirSync(output,{recursive:true});copy(stageDir,finalRoot,true);fs.unlinkSync(path.join(finalRoot,'STAGE.json'));check(contentDigest(content(finalRoot,s.target))===payloadHash,'Payload changed while archiving');
 const evidence={};for(const [k,v]of Object.entries(proofs)){const name=(composite?'baseline-':'')+k+'.json',n='PROVENANCE/'+name;fs.copyFileSync(reference(v.ref),inside(finalRoot,n));evidence[k]={...publicReference(v.ref,name),status:'PASS'};}
 if(selectedBrowserRuntime){const ref=selectedBrowserRuntime.ref;fs.copyFileSync(reference(ref),inside(finalRoot,'PROVENANCE/selected-browser-runtime.json'));const runtime={...publicReference(ref,'selected-browser-runtime.json'),status:'PASS'};browserOverlay.copyRuntimeEvidence(selectedBrowserRuntime,finalRoot,browserUtilities());Object.assign(evidence,browserOverlay.stageProvenance(stageFile,finalRoot,browserUtilities()));if(composite){const baseline={proofs:Object.fromEntries(['signing','sandbox','lifecycle','runtimeAcceptance'].map(k=>[k,evidence[k]]))};for(const [name,ref]of Object.entries(composite.baseline)){const file='baseline-'+name+'.json';fs.copyFileSync(reference(ref),inside(finalRoot,'PROVENANCE/'+file));baseline[name]=publicReference(ref,file);}for(const k of ['signing','sandbox','lifecycle','runtimeAcceptance'])delete evidence[k];evidence.acceptance={...composite.lineage,baseline,selectedBrowserRuntime:runtime};}else evidence.selectedBrowserRuntime=runtime;}
 const desktop={schema:1,kind:'backplane-electron-desktop-package',target:s.target,release:s.release,entrypoint:{launcher:s.entrypoint,backend:'src/server/main.bend'},...(s.lifecyclePolicy?{lifecyclePolicy:s.lifecyclePolicy}:{}),components:s.components,...(s.windowsShellSource?{windowsShellSource:s.windowsShellSource}:{}),electronVersion:ELECTRON,serviceVersion:VERSION,shellVersion:VERSION,...(s.selectedBrowser?{resourceWeb:s.resourceWeb,selectedBrowser:s.selectedBrowser}:{}),webManifestSha256:s.webManifestSha256,noticesManifestSha256:s.noticesManifestSha256,...evidence,payloadManifestSha256:payloadHash,members:inventory(finalRoot,s.target)};write(path.join(finalRoot,'DESKTOP.json'),desktop);write(path.join(finalRoot,'MANIFEST.json'),{schema:1,kind:'backplane-electron-package-members',target:s.target,files:inventory(finalRoot,s.target)});
 const name='backplane-v'+VERSION+'-'+s.target+TARGETS[s.target].suffix,filename=path.join(output,name);runPython(['archive',finalRoot,filename,TARGETS[s.target].root+'/',String(s.sourceDateEpoch)]);fs.writeFileSync(filename+'.sha256',digest(filename)+'  '+name+'\n');return{status:'ASSEMBLED_FROM_BOUND_TARGET_ACCEPTANCE',target:s.target,archive:filename,sha256:digest(filename),desktopReceipt:path.join(finalRoot,'DESKTOP.json'),desktopReceiptSha256:digest(path.join(finalRoot,'DESKTOP.json')),published:false};
}
function inspectStage(dir){const s=json(path.join(dir,'STAGE.json'));check(s.kind==='backplane-electron-desktop-stage'&&TARGETS[s.target],'Invalid stage');return{status:'INSPECTED_NOT_QUALIFIED',target:s.target,version:s.release.version,stageReceiptSha256:digest(path.join(dir,'STAGE.json')),payloadManifestSha256:contentDigest(content(dir,s.target))};}
function main(argv){
 const [op,...rest]=argv;if(op==='describe'){console.log(JSON.stringify({interface:'backplane-electron-release-producer-v2',targets:Object.keys(TARGETS),version:VERSION,electronVersion:ELECTRON,operations:['inspect --stage ABS_STAGE','stage --request REQUEST.json --output NEW_ABS_STAGE','archive --stage ABS_STAGE --acceptance ACCEPTANCE.json --output NEW_ABS_DIST'],execution:'Offline supplied-artifact assembly only; stage is unqualified; archive requires bound target evidence'},null,2));return;}
 const args={};check(rest.length%2===0,'Expected named option/value pairs');for(let i=0;i<rest.length;i+=2){check(rest[i].startsWith('--')&&!Object.hasOwn(args,rest[i]),'Invalid/duplicate option');args[rest[i]]=rest[i+1];}
 let result;if(op==='stage'){check(Object.keys(args).sort().join(',')==='--output,--request','Usage: stage --request FILE --output NEW_ABS_STAGE');result=stage(path.resolve(args['--request']),args['--output']);}
 else if(op==='inspect'){check(Object.keys(args).join(',')==='--stage','Usage: inspect --stage DIR');result=inspectStage(path.resolve(args['--stage']));}
 else if(op==='archive'){check(Object.keys(args).sort().join(',')==='--acceptance,--output,--stage','Usage: archive --stage DIR --acceptance FILE --output NEW_ABS_DIST');result=archive(path.resolve(args['--stage']),path.resolve(args['--acceptance']),args['--output']);}
 else fail('Unknown operation; use describe');console.log(JSON.stringify(result));
}
if(require.main===module){try{main(process.argv.slice(2));}catch(e){console.error('Desktop producer: '+e.message);process.exitCode=1;}}
module.exports={stage,archive,inspectStage,inventory,membershipExpectation,contentDigest,architecture,validateRequest,generatedWeb,reference,readReference,json,write,sha,digest,canonical,runPython,payload,identity,shellIdentity,TARGETS,browserUtilities,linuxLauncher};
