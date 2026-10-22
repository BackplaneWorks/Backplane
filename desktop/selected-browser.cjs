'use strict';
// Optional, root-pinned web composition. Original resource validation belongs
// to the producer and MUST complete before validate() is called.
const fs=require('node:fs'),path=require('node:path');
const ROOT_POLICY_SHA256='fee94f08b3ce7d909a1265d9f33820ea816887703e940ca0d7a57374f9509314'; // Exact root-owned canonical selected-browser policy.
const HEX=/^[a-f0-9]{64}$/,GIT=/^[a-f0-9]{40}$/;
const check=(v,m)=>{if(!v)throw Error(m);};
const proofs=['signing','sandbox','lifecycle','runtimeAcceptance'];
const sameRef=(a,b)=>a&&b&&a.sha256===b.sha256&&a.bytes===b.bytes;
function input(file,u,rootDisposition){
 const spec=u.json(file),base=path.dirname(path.resolve(file));
 check(['payload,receipt','payload,receipt,rootDisposition'].includes(Object.keys(spec).sort().join(',')),'Browser input needs receipt/payload and rootDisposition');
 check(spec.receipt&&typeof spec.receipt.path==='string'&&spec.payload&&typeof spec.payload==='object','Invalid selected browser input');
 const receipt={...spec.receipt,path:path.resolve(base,spec.receipt.path)},payload={...spec.payload};
 check(Boolean(payload.directory)!==Boolean(payload.archive),'Browser payload needs exactly directory or archive');
 if(payload.directory)payload.directory=path.resolve(base,payload.directory);
 else{check(typeof payload.archive.path==='string','Invalid browser archive reference');payload.archive={...payload.archive,path:path.resolve(base,payload.archive.path)};}
 const supplied=spec.rootDisposition;
 return{receipt,payload,...(supplied?{rootDisposition:{...supplied,path:path.resolve(base,supplied.path)}}:rootDisposition?{rootDisposition}:{})};
}
function nativeServicePin(allowed,target,pins){
 const name='serviceReceiptSha256';
 check(HEX.test(allowed?.[name]||'')&&allowed[name]===pins[name],'Root native service artifact pin mismatch');
}
function rootAuthority(rootDisposition,v,u){
 check(HEX.test(ROOT_POLICY_SHA256||''),'Root selected-browser policy pin is unresolved');
 check(rootDisposition?.sha256===ROOT_POLICY_SHA256&&sameRef(rootDisposition,v.freeze.rootStagingDisposition),'Untrusted/mismatched selected browser root disposition');
 const policy=u.readReference(rootDisposition),allowed=policy.targets?.[v.target];
 check(policy.kind==='backplane-root-staging-disposition'&&policy.status==='AUTHORIZED_STAGING_POLICY_PENDING_ACTUAL_SERVICE_BINDINGS'&&policy.publicationOwner==='root','Invalid selected-browser root policy');
 check(['version','tag','channel','commit','tree'].every(k=>policy[k]===v.release[k])&&policy.electronVersion==='44.9.2','Root/native release identity mismatch');
 check(u.canonical(policy.compileInputs)===u.canonical(v.freeze.compileInputs),'Root configured compile tuple differs');
 check(allowed&&u.canonical(allowed.selectedBrowser)===u.canonical(v.freeze.selectedBrowser)&&allowed.signingPolicy===v.freeze.signingPolicy[v.target]&&u.canonical(allowed.lifecyclePolicy)===u.canonical(v.freeze.lifecyclePolicy?.[v.target]),'Root browser/native disposition differs from freeze');
 check(allowed.shellReceiptSha256===v.freeze.components[v.target].shellReceiptSha256&&allowed.electronReceiptSha256===v.freeze.components[v.target].electronReceiptSha256&&u.canonical(allowed.windowsShellSource??null)===u.canonical(v.freeze.windowsShellSource??null),'Root immutable component/source pins differ');
 nativeServicePin(allowed,v.target,v.freeze.components[v.target]);
 return policy;
}
function validate(spec,v,temp,u){
 const pin=v.freeze.selectedBrowser;
 if(!spec&&!pin)return null;
 check(spec&&pin,'Selected browser request/freeze must both be present');
 check(pin.mode==='generated-web-subtree-only','Invalid browser composition mode');
 check(GIT.test(pin.sourceCommit)&&GIT.test(pin.sourceTree),'Selected browser source must be frozen');
 rootAuthority(spec.rootDisposition,v,u);
 const component=u.payload(spec,temp,'selected-browser'),r=component.receipt;
 check(sameRef(component.ref,pin.receipt)&&v.freeze.components[v.target].selectedBrowserReceiptSha256===component.ref.sha256,'Selected browser not pinned by root freeze');
 check(r.schema===1&&r.kind==='backplane-selected-browser'&&r.status==='VERIFIED'&&r.version==='0.12.1'&&r.channel==='release'&&r.entrypoint==='src/web/index.html','Unqualified selected browser receipt');
 check(r.sourceCommit===pin.sourceCommit&&r.sourceTree===pin.sourceTree,'Selected browser source mismatch');
 const original=u.digest(path.join(v.service.root,'WEB.json'));
 check(pin.resourceWebManifestSha256===original&&v.freeze.components[v.target].webManifestSha256===original,'Original resource web binding changed');
 check(u.canonical(v.freeze.resourceWeb?.[v.target])===u.canonical({sourceCommit:v.release.commit,sourceTree:v.release.tree,webManifestSha256:original}),'Missing separate original resourceWeb lineage');
 check(r.webManifest==='WEB.json'&&r.webRoot==='web','Selected browser roles must be exact');
 const manifestFile=path.join(component.root,'WEB.json'),manifest=u.json(manifestFile),manifestSha=u.digest(manifestFile);
 check(HEX.test(pin.webManifestSha256||'')&&pin.webManifestSha256===manifestSha&&r.webManifestSha256===manifestSha,'Selected browser manifest mismatch');
 const members=u.membership(component.root,r.members,v.target);
 check(Object.keys(members).every(n=>members[n].type==='file'&&(n==='WEB.json'||n.startsWith('web/'))),'Selected browser payload is web-only; links forbidden');
 const web=path.join(component.root,'web');u.generatedWeb(web,manifest,{...v.release,commit:r.sourceCommit,tree:r.sourceTree});u.notices(web,r);
 const reached=new Set(),pending=['index.html',...(manifest.members['sw.js']?['sw.js']:[])];
 while(pending.length){const n=pending.pop();if(reached.has(n))continue;check(manifest.members[n],'Missing selected browser graph member');reached.add(n);
  if(/\.(?:html|js|css)$/.test(n))for(const hit of fs.readFileSync(path.join(web,n),'utf8').matchAll(/(?:["'(])((?:\.\.?\/|[\w.-]+\/)*(?:chunk-|favicon-|apple-touch-icon-)[\w.-]+\.(?:js|css|svg|png))["')]/g)){const linked=path.posix.normalize(path.posix.join(path.posix.dirname(n),hit[1]));check(!linked.startsWith('../')&&manifest.members[linked],'Missing selected browser graph member');pending.push(linked);}
 }
 for(const n of Object.keys(manifest.members))if(/\.(?:html|js|css)$/.test(n))check(reached.has(n),'Unreferenced selected browser code asset');
 for(const n of Object.keys(manifest.members)){
  check(!/\.(?:exe|dll|so|dylib|node)$/i.test(n),'Native code in selected browser');
  const b=fs.readFileSync(path.join(web,n)),magic=b.subarray(0,4).toString('hex');
  check(!['7f454c46','cffaedfe','cefaedfe','cafebabe'].includes(magic)&&b.subarray(0,2).toString()!=='MZ','Native format in selected browser');
 }
 return{...component,web,manifest,manifestFile,rootDisposition:spec.rootDisposition,descriptor:{mode:pin.mode,sourceCommit:r.sourceCommit,sourceTree:r.sourceTree,receipt:{sha256:component.ref.sha256,bytes:component.ref.bytes},webManifestSha256:manifestSha,resourceWebManifestSha256:original,rootStagingDisposition:v.freeze.rootStagingDisposition,...(pin.baselineDesktopReceiptSha256?{baselineDesktopReceiptSha256:pin.baselineDesktopReceiptSha256}:{})}};
}
function install(selected,serviceRoot,target,u){
 if(!selected)return;
 const before=u.inventory(serviceRoot,target),web=path.join(serviceRoot,'web');
 fs.rmSync(web,{recursive:true});u.copy(selected.web,web,true);
 u.generatedWeb(web,selected.manifest,{commit:selected.descriptor.sourceCommit,tree:selected.descriptor.sourceTree});
 const after=u.inventory(serviceRoot,target),withoutWeb=x=>Object.fromEntries(Object.entries(x).filter(([n])=>!n.startsWith('web/')));
 check(u.canonical(withoutWeb(before))===u.canonical(withoutWeb(after)),'Browser composition changed non-web service resource');
 u.notices(web,selected.receipt);
}
function stageProvenance(stageFile,output,u){
 check(fs.lstatSync(stageFile).isFile(),'Current stage receipt must be a regular file');
 const name='PROVENANCE/selected-browser-stage.json',dest=path.join(output,name);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.copyFileSync(stageFile,dest,fs.constants.COPYFILE_EXCL);
 const sha256=u.digest(stageFile),bytes=fs.statSync(stageFile).size;check(u.digest(dest)===sha256,'Current stage receipt copy changed');
 return{stageReceiptSha256:sha256,stageReceipt:{path:name,sha256,bytes}};
}
function runtimeEvidence(receipt,u){
 const refs=receipt.evidence;check(Array.isArray(refs)&&refs.length>0&&refs.length<=16,'Runtime evidence count must be1..16');
 let total=0;const names=new Set();
 for(const ref of refs){
  check(ref&&typeof ref.path==='string'&&/^PROVENANCE\/selected-browser-evidence\/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(ref.path)&&!ref.path.includes('..'),'Unsafe runtime evidence namespace');
  check(!names.has(ref.path.toLowerCase()),'Duplicate runtime evidence path');names.add(ref.path.toLowerCase());
  check(HEX.test(ref.sha256||'')&&Number.isSafeInteger(ref.bytes)&&ref.bytes>0&&ref.bytes<=8*1024*1024,'Runtime evidence hash/size must be nonempty and <=8MiB');total+=ref.bytes;
 }
 check(total<=32*1024*1024,'Runtime evidence total exceeds32MiB');
 return refs.map(ref=>{const file=u.reference(ref);let cursor=file;for(let i=0;i<ref.path.split('/').length;i++){const stat=fs.lstatSync(cursor);check(i===0?stat.isFile():stat.isDirectory(),'Runtime evidence source has a link/nonregular parent');cursor=path.dirname(cursor);}return{ref,file};});
}
function copyRuntimeEvidence(runtime,output,u){
 const inputs=runtimeEvidence(runtime.receipt,u),provenance=path.join(output,'PROVENANCE'),dir=path.join(provenance,'selected-browser-evidence');
 fs.mkdirSync(output,{recursive:true});check(fs.lstatSync(output).isDirectory(),'Runtime evidence output is a link');fs.mkdirSync(provenance,{recursive:true});check(fs.lstatSync(provenance).isDirectory(),'Runtime evidence provenance is a link');
 check(!fs.existsSync(dir),'Runtime evidence namespace already exists');fs.mkdirSync(dir);
 for(const {ref,file}of inputs){const dest=path.join(output,ref.path);fs.copyFileSync(file,dest,fs.constants.COPYFILE_EXCL);check(u.digest(dest)===ref.sha256&&fs.statSync(dest).size===ref.bytes,'Runtime evidence copy changed');}
 check(fs.readdirSync(dir).sort().join('\n')===inputs.map(({ref})=>path.basename(ref.path)).sort().join('\n'),'Unexpected runtime evidence namespace member');
}

function currentRuntime(ref,s,payloadHash,u){
 const r=u.readReference(ref),b=s.selectedBrowser;
 check(r.schema===1&&r.kind==='desktop-selected-browser-runtime'&&r.scope==='same-target-selected-browser'&&r.status==='PASS'&&r.target===s.target&&r.version==='0.12.1','Missing same-target selected browser runtime');
 check(r.payloadManifestSha256===payloadHash&&r.stageReceiptSha256===s.stageReceiptSha256&&r.freezeReceiptSha256===s.release.freezeReceipt.sha256,'Browser runtime must bind current stage/payload/freeze');
 check(r.selectedBrowserReceiptSha256===b.receipt.sha256&&r.webManifestSha256===b.webManifestSha256&&r.sourceCommit===b.sourceCommit&&r.sourceTree===b.sourceTree,'Browser runtime source/assets mismatch');
 const required=['authenticatedOrigin','newAssetsLoaded','routeRetention','draftRetention','inlineBoundedMore','explicitRetry'];
 check(required.every(k=>r.checks?.[k]===true),'Incomplete selected browser behavior coverage');
 check(r.rendererSandbox?.processType==='renderer'&&r.rendererSandbox.osEnforced===true&&Array.isArray(r.rendererSandbox.flags)&&r.rendererSandbox.flags.every(f=>typeof f==='string')&&!r.rendererSandbox.flags.some(f=>['--no-sandbox','--disable-sandbox','--disable-namespace-sandbox','--disable-gpu-sandbox','--disable-setuid-sandbox','--disable-seccomp-filter-sandbox'].includes(f.split('=')[0])),'Missing actual renderer OS sandbox');
 runtimeEvidence(r,u);
 return{ref,receipt:r};
}
function composite(a,s,actual,frozen,u){
 check(a.kind==='backplane-desktop-browser-composite-acceptance'&&a.scope==='retained-native-baseline+selected-browser','Wrong composite acceptance kind/scope');
 check(['darwin-arm64','windows-x64'].includes(s.target)&&s.selectedBrowser,'Composite baseline is restricted to retained Mac/Windows components');
 const b=a.baseline,d=u.readReference(b?.desktop),oldFreeze=u.readReference(b?.freeze);
 check(HEX.test(frozen.selectedBrowser?.baselineDesktopReceiptSha256||'')&&b.desktop.sha256===frozen.selectedBrowser.baselineDesktopReceiptSha256,'Baseline desktop not pinned by root');
 check(d.schema===1&&d.kind==='backplane-electron-desktop-package'&&d.target===s.target&&d.release.version==='0.12.1'&&d.release.commit===s.release.commit&&d.release.tree===s.release.tree&&!d.selectedBrowser,'Baseline must be original same-target desktop');
 check(sameRef(d.components.freeze,b.freeze)&&oldFreeze.kind==='backplane-release-freeze'&&oldFreeze.status==='FROZEN'&&oldFreeze.commit===s.release.commit&&oldFreeze.tree===s.release.tree,'Baseline freeze mismatch');
 check(u.canonical(oldFreeze.signingPolicy?.[s.target])===u.canonical(frozen.signingPolicy?.[s.target]),'Baseline signing disposition changed');
 check(u.canonical(d.lifecyclePolicy??null)===u.canonical(s.lifecyclePolicy??null)&&u.canonical(d.windowsShellSource??null)===u.canonical(s.windowsShellSource??null),'Baseline native policy/source differs');
 for(const k of ['shell','electron','service','serviceBuild'])check((!d.components[k]&&!s.components[k])||sameRef(d.components[k],s.components[k]),'Baseline immutable component differs: '+k);
 const webPrefix=u.TARGETS[s.target].resources+'/backplane-service/web/';
 const native=x=>Object.fromEntries(Object.entries(x).filter(([n])=>!n.startsWith(webPrefix)&&!n.startsWith('PROVENANCE/')&&!['STAGE.json','DESKTOP.json','MANIFEST.json'].includes(n)));
 const oldContent=Object.fromEntries(Object.entries(d.members).filter(([n])=>!n.startsWith('PROVENANCE/')&&!['STAGE.json','DESKTOP.json','MANIFEST.json'].includes(n)));
 check(u.contentDigest(oldContent)===d.payloadManifestSha256,'Baseline member manifest corrupt');
 check(u.canonical(native(d.members))===u.canonical(native(actual)),'Composite changed non-web/native/runtime/vendor/shell bytes');
 const baselineProofs={};
 for(const k of proofs){
  const ref=b.proofs?.[k],r=u.readReference(ref);
  check(sameRef(ref,d[k])&&r.schema===1&&r.status==='PASS'&&r.target===s.target&&r.version==='0.12.1'&&r.commit===s.release.commit&&r.tree===s.release.tree&&r.payloadManifestSha256===d.payloadManifestSha256&&r.freezeReceiptSha256===b.freeze.sha256,'Baseline proof identity changed: '+k);
  baselineProofs[k]={ref,receipt:r};
 }
 return{proofs:baselineProofs,baseline:{desktop:b.desktop,freeze:b.freeze},lineage:{kind:'desktop-browser-composite',scope:a.scope,baselineDesktopSha256:b.desktop.sha256,baselinePayloadManifestSha256:d.payloadManifestSha256,baselineFreezeReceiptSha256:b.freeze.sha256,immutableComponents:['shell','electron','service','serviceBuild','windowsResourceArchive'].filter(k=>d.components[k]).reduce((o,k)=>({...o,[k]:d.components[k]}),{}),selectedBrowser:s.selectedBrowser}};
}
module.exports={input,nativeServicePin,rootAuthority,validate,install,stageProvenance,runtimeEvidence,copyRuntimeEvidence,currentRuntime,composite};
