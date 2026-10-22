'use strict';
// New synthetic offline controls only. No executable, archive, runtime, compiler
// or old suite is run. Reuse its fixture DEFINITIONS up to its first test.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),{createRequire}=require('node:module'),zlib=require('node:zlib');
let p=require('../produce-release.cjs'),overlay=require('../selected-browser.cjs');
const composer=require('../compose-target-freeze.cjs'),adapter=require('../prepare-native-stage.cjs');
// Synthetic root pins are injected ONLY into an isolated in-memory copy of the
// exact trust constant. Production has no environment, CLI or callable override.
function trustSyntheticPolicy(sha){
 const moduleFile=path.resolve(__dirname,'../selected-browser.cjs'),producerFile=path.resolve(__dirname,'../produce-release.cjs');
 const load=(file,code,override)=>{const m={exports:{}};const req=createRequire(file);const wrapped=vm.runInThisContext('(function(require,module,exports,__filename,__dirname){'+code+'\n})',{filename:file});wrapped(name=>override?.(name)||req(name),m,m.exports,file,path.dirname(file));return m.exports;};
 const code=fs.readFileSync(moduleFile,'utf8');assert(/const ROOT_POLICY_SHA256=(?:null|'[a-f0-9]{64}');/.test(code));
 overlay=load(moduleFile,code.replace(/const ROOT_POLICY_SHA256=(?:null|'[a-f0-9]{64}');/,'const ROOT_POLICY_SHA256='+JSON.stringify(sha)+';'));
 p=load(producerFile,fs.readFileSync(producerFile,'utf8'),name=>name==='./selected-browser.cjs'?overlay:null);
}
const fixtureFile=path.join(__dirname,'release-producer.test.cjs'),source=fs.readFileSync(fixtureFile,'utf8'),stop=source.indexOf('for(const target of Object.keys(producer.TARGETS))test(');assert(stop>0);
const {fixture,evidence,put,write,ref}=vm.runInThisContext('(function(require){'+source.slice(0,stop)+';return{fixture,evidence,put,write,ref};})', {filename:fixtureFile})(createRequire(fixtureFile));
const utils=()=>p.browserUtilities();
function authorize(f){
 const r=f.request.release,allowed={selectedBrowser:f.freeze.selectedBrowser,signingPolicy:f.freeze.signingPolicy[f.target],lifecyclePolicy:f.freeze.lifecyclePolicy?.[f.target],shellReceiptSha256:f.freeze.components[f.target].shellReceiptSha256,electronReceiptSha256:f.freeze.components[f.target].electronReceiptSha256,serviceReceiptSha256:f.freeze.components[f.target].serviceReceiptSha256,...(f.freeze.windowsShellSource?{windowsShellSource:f.freeze.windowsShellSource}:{})};
 const policy={schema:1,kind:'backplane-root-staging-disposition',status:'AUTHORIZED_STAGING_POLICY_PENDING_ACTUAL_SERVICE_BINDINGS',publicationOwner:'root',version:r.version,tag:r.tag,channel:r.channel,commit:r.commit,tree:r.tree,electronVersion:'44.9.2',sourceDateEpoch:f.freeze.sourceDateEpoch,compileInputs:f.freeze.compileInputs,targets:{[f.target]:allowed}};
 const policyRef=ref(write(path.join(f.root,'root-policy.json'),policy));f.request.selectedBrowser.rootDisposition=policyRef;f.freeze.rootStagingDisposition={sha256:policyRef.sha256,bytes:policyRef.bytes};trustSyntheticPolicy(policyRef.sha256);f.refresh();
}
function browser(f){
 const root=path.join(f.root,'browser'),web=path.join(root,'web');
 for(const[n,text]of Object.entries({'index.html':'<script src="./chunk-new.js"></script>','chunk-new.js':'// new browser fixture, never executed','LICENSE.Web':'Synthetic notice'})){
  put(path.join(web,n),text);if(/\.(html|js)$/.test(n))put(path.join(web,n+'.gz'),zlib.gzipSync(Buffer.from(text)));
 }
 const manifest={schema:1,kind:'matching-generated-web',sourceCommit:'c'.repeat(40),sourceTree:'d'.repeat(40),entrypoint:'src/web/index.html',members:p.inventory(web,'linux-x64')};write(path.join(root,'WEB.json'),manifest);
 const receipt={schema:1,kind:'backplane-selected-browser',status:'VERIFIED',version:'0.12.1',channel:'release',sourceCommit:manifest.sourceCommit,sourceTree:manifest.sourceTree,entrypoint:manifest.entrypoint,webManifest:'WEB.json',webRoot:'web',webManifestSha256:p.digest(path.join(root,'WEB.json')),members:p.inventory(root,'linux-x64'),packages:[{name:'Browser fixture',version:'fixture',sourceDisposition:'Synthetic source-only fixture',licenses:[{path:'LICENSE.Web',sha256:p.digest(path.join(web,'LICENSE.Web'))}]}]};
 const file=write(path.join(f.root,'browser.json'),receipt);f.request.selectedBrowser={receipt:ref(file),payload:{directory:root}};
 f.freeze.selectedBrowser={mode:'generated-web-subtree-only',receipt:{sha256:ref(file).sha256,bytes:ref(file).bytes},sourceCommit:receipt.sourceCommit,sourceTree:receipt.sourceTree,webManifestSha256:receipt.webManifestSha256,resourceWebManifestSha256:p.digest(path.join(f.service,'WEB.json'))};
 f.freeze.components[f.target].selectedBrowserReceiptSha256=ref(file).sha256;
 f.freeze.resourceWeb={[f.target]:{sourceCommit:f.request.release.commit,sourceTree:f.request.release.tree,webManifestSha256:f.freeze.selectedBrowser.resourceWebManifestSha256}};
 authorize(f);return{root,web,receipt,file,manifest};
}
function validate(f){const temp=path.join(f.root,'validation');fs.mkdirSync(temp,{recursive:true});return p.validateRequest(p.json(f.requestPath),temp);}
for(const target of ['linux-x64','linux-arm64','darwin-arm64','windows-x64'])test('new '+target+' overlay validates original then preserves every non-web resource and raw receipt',t=>{
 const f=fixture(t,target,true),b=browser(f),v=validate(f),dest=path.join(f.root,'copy');
 fs.cpSync(v.service.root,dest,{recursive:true});const before=p.inventory(dest,target);overlay.install(v.selectedBrowser,dest,target,utils());const after=p.inventory(dest,target);
 const omitWeb=x=>Object.fromEntries(Object.entries(x).filter(([n])=>!n.startsWith('web/')));
 assert.deepEqual(omitWeb(after),omitWeb(before));assert.equal(p.digest(path.join(dest,'web/index.html')),p.digest(path.join(b.web,'index.html')));
 assert.equal(p.digest(path.join(dest,'WEB.json')),v.selectedBrowser.descriptor.resourceWebManifestSha256);assert.notEqual(p.digest(path.join(dest,'WEB.json')),v.selectedBrowser.descriptor.webManifestSha256);
});
test('original native/matching-web guards reject before selected browser receipt is even read',t=>{
 const f=fixture(t);browser(f);f.request.selectedBrowser.receipt.path='/definitely-not-read/browser.json';put(path.join(f.service,'BUILD.json'),'corrupt');f.refresh();assert.throws(()=>validate(f),/Packaged BUILD/);
 const g=fixture(t);browser(g);g.request.selectedBrowser.receipt.path='/definitely-not-read/browser.json';fs.unlinkSync(path.join(g.service,'web/index.html.gz'));g.refresh();assert.throws(()=>validate(g),/membership/);
});
test('direct CLI/request alone cannot authorize a forged or pending root disposition',t=>{
 const f=fixture(t);browser(f);const v=validate(f),temp=path.join(f.root,'root-rejection');fs.mkdirSync(temp);
 const real=require('../selected-browser.cjs');assert.throws(()=>real.validate(f.request.selectedBrowser,v,temp,utils()),/unresolved|Untrusted/);
 const noPolicy={...f.request.selectedBrowser};delete noPolicy.rootDisposition;assert.throws(()=>overlay.validate(noPolicy,v,temp,utils()),/Untrusted/);
 const wrongPolicy={...f.request.selectedBrowser,rootDisposition:{...f.request.selectedBrowser.rootDisposition,sha256:'0'.repeat(64)}};assert.throws(()=>overlay.validate(wrongPolicy,v,temp,utils()),/Untrusted/);
 const changed=structuredClone(v);changed.freeze.selectedBrowser.sourceTree='f'.repeat(40);assert.throws(()=>overlay.validate(f.request.selectedBrowser,changed,temp,utils()),/Root browser/);
 const input=write(path.join(f.root,'input-with-policy.json'),{receipt:f.request.selectedBrowser.receipt,payload:f.request.selectedBrowser.payload,rootDisposition:{...f.request.selectedBrowser.rootDisposition,path:'root-policy.json'}});
 assert.equal(overlay.input(input,p).rootDisposition.path,path.join(f.root,'root-policy.json'));
 const implicit=write(path.join(f.root,'input-composer-policy.json'),{receipt:f.request.selectedBrowser.receipt,payload:f.request.selectedBrowser.payload});assert.deepEqual(overlay.input(implicit,p,f.request.selectedBrowser.rootDisposition).rootDisposition,f.request.selectedBrowser.rootDisposition);
});
test('missing root/source/manifest/original-web pins fail closed',t=>{
 for(const mutate of [f=>delete f.freeze.rootStagingDisposition,f=>f.freeze.selectedBrowser.sourceCommit='PENDING',f=>f.freeze.selectedBrowser.sourceTree=null,f=>f.freeze.selectedBrowser.receipt.sha256='0'.repeat(64),f=>f.freeze.selectedBrowser.webManifestSha256='0'.repeat(64),f=>f.freeze.selectedBrowser.resourceWebManifestSha256='0'.repeat(64),f=>delete f.freeze.resourceWeb]){
  const f=fixture(t);browser(f);mutate(f);f.refresh();assert.throws(()=>validate(f));
 }
});
test('selected payload cannot sneak native/nonweb members, links, fonts, bad gzip or missing notices',t=>{
 for(const mutate of [b=>put(path.join(b.root,'backplane-serve'),'bad'),b=>fs.symlinkSync('index.html',path.join(b.web,'link')),b=>put(path.join(b.web,'font.woff'),'bad'),b=>put(path.join(b.web,'index.html.gz'),'bad'),b=>fs.unlinkSync(path.join(b.web,'LICENSE.Web'))]){
  const f=fixture(t),b=browser(f);mutate(b);assert.throws(()=>validate(f));
 }
});
test('matching updated inventories still refuse unsafe type/native payload and incomplete asset graph',t=>{
 for(const mutate of [b=>put(path.join(b.root,'extra'),'not web'),b=>put(path.join(b.web,'native.dat'),Buffer.from('7f454c46','hex')),b=>put(path.join(b.web,'index.html'),'<script src="./chunk-missing.js"></script>'),b=>{put(path.join(b.web,'chunk-old.js'),'stale');put(path.join(b.web,'chunk-old.js.gz'),zlib.gzipSync(Buffer.from('stale')));}]){
  const f=fixture(t),b=browser(f);mutate(b);
  const index=fs.readFileSync(path.join(b.web,'index.html'));put(path.join(b.web,'index.html.gz'),zlib.gzipSync(index));
  b.manifest.members=p.inventory(b.web,'linux-x64');write(path.join(b.root,'WEB.json'),b.manifest);b.receipt.webManifestSha256=p.digest(path.join(b.root,'WEB.json'));b.receipt.members=p.inventory(b.root,'linux-x64');write(b.file,b.receipt);
  const r=ref(b.file);f.request.selectedBrowser.receipt=r;f.freeze.selectedBrowser.receipt={sha256:r.sha256,bytes:r.bytes};f.freeze.selectedBrowser.webManifestSha256=b.receipt.webManifestSha256;f.freeze.components[f.target].selectedBrowserReceiptSha256=r.sha256;authorize(f);assert.throws(()=>validate(f),/web-only|Native format|Missing selected browser graph|Unreferenced selected browser/);
 }
});
test('legacy no-browser branch returns no overlay and no input can silently drop it',t=>{
 const f=fixture(t);assert.equal(validate(f).selectedBrowser,undefined);browser(f);delete f.request.selectedBrowser;f.refresh();assert.throws(()=>validate(f),/both/);
 const g=fixture(t);browser(g);delete g.freeze.selectedBrowser;g.refresh();assert.throws(()=>validate(g),/both/);
});
test('portable browser input and optional CLI preserve existing required flags and reject duplicates',t=>{
 const f=fixture(t);browser(f);const input=write(path.join(f.root,'input.json'),{receipt:{...f.request.selectedBrowser.receipt,path:'browser.json'},payload:{directory:'browser'}}),v=overlay.input(input,p);assert.equal(v.payload.directory,path.join(f.root,'browser'));assert.equal(v.receipt.path,path.join(f.root,'browser.json'));
 for(const [fn,keys]of [[composer.options,['--policy','--components','--service','--out']],[adapter.options,['--components','--service','--freeze','--out']]]){
  const base=keys.flatMap(k=>[k,path.join(f.root,k.slice(2))]);assert.equal(Object.keys(fn(base)).length,4);assert.equal(fn([...base,'--selected-browser',input])['--selected-browser'],input);assert.throws(()=>fn([...base,'--out',input]));
 }
});
test('selected-browser membership caller uses the existing native Windows mode projection',t=>{
 const f=fixture(t,'windows-x64',true);browser(f);const v=validate(f),u=utils();let target;
 const original=u.membership;u.membership=(root,members,t)=>{target=t;return original(root,members,t);};
 const temp=path.join(f.root,'second-validation');fs.mkdirSync(temp);overlay.validate(f.request.selectedBrowser,v,temp,u);assert.equal(target,'windows-x64');
 const sample={'index.html':{type:'file',mode:0o644,sha256:'0'.repeat(64),bytes:0}};
 assert.equal(p.membershipExpectation(sample,target,'win32')['index.html'].mode,0o666);assert.equal(sample['index.html'].mode,0o644);
});
function lineage(t,target='darwin-arm64'){
 const f=fixture(t,target,true),base=path.join(f.root,'baseline-stage');p.stage(f.requestPath,base);const old=p.json(path.join(base,'STAGE.json')),e=p.json(evidence(f,base)),oldFreeze=path.join(f.root,'old-freeze.json');fs.copyFileSync(path.join(f.root,'freeze.json'),oldFreeze);
 const members=p.inventory(base,target);delete members['STAGE.json'];
 const d={schema:1,kind:'backplane-electron-desktop-package',target,release:old.release,components:old.components,lifecyclePolicy:old.lifecyclePolicy,payloadManifestSha256:old.payloadManifestSha256,members};for(const k of ['signing','sandbox','lifecycle','runtimeAcceptance'])d[k]=e[k];
 const desktop=ref(write(path.join(f.root,'baseline-desktop.json'),d));browser(f);f.freeze.selectedBrowser.baselineDesktopReceiptSha256=desktop.sha256;authorize(f);
 const stage=path.join(f.root,'new-stage');p.stage(f.requestPath,stage);const s=p.json(path.join(stage,'STAGE.json'));s.stageReceiptSha256=p.digest(path.join(stage,'STAGE.json'));
 const a={kind:'backplane-desktop-browser-composite-acceptance',scope:'retained-native-baseline+selected-browser',baseline:{desktop,freeze:ref(oldFreeze),proofs:Object.fromEntries(['signing','sandbox','lifecycle','runtimeAcceptance'].map(k=>[k,e[k]]))}};
 return{f,s,stage,a,actual:s.members};
}
for(const target of ['darwin-arm64','windows-x64'])test('new '+target+' composite retains exact baseline four-proof bytes and same nonweb components',t=>{
 const x=lineage(t,target),result=overlay.composite(x.a,x.s,x.actual,x.f.freeze,utils());assert.equal(result.lineage.kind,'desktop-browser-composite');
 for(const k of ['signing','sandbox','lifecycle','runtimeAcceptance'])assert.equal(result.proofs[k].ref.sha256,x.a.baseline.proofs[k].sha256);
 assert.equal(result.lineage.baselinePayloadManifestSha256,result.proofs.runtimeAcceptance.receipt.payloadManifestSha256);assert.notEqual(x.s.payloadManifestSha256,result.lineage.baselinePayloadManifestSha256);
});
test('composite rejects raw proof rewrite, wrong baseline policy, native mutation and non-target lineage',t=>{
 for(const change of [x=>x.f.freeze.selectedBrowser.baselineDesktopReceiptSha256='0'.repeat(64),x=>x.a.baseline.proofs.sandbox=x.a.baseline.proofs.signing,x=>{const n=Object.keys(x.actual).find(n=>n.endsWith('/app/main.cjs'));x.actual[n]={...x.actual[n],sha256:'0'.repeat(64)};},x=>x.s.target='linux-x64',x=>x.a.scope='desktop-release']){
  const x=lineage(t);change(x);assert.throws(()=>overlay.composite(x.a,x.s,x.actual,x.f.freeze,utils()));
 }
});
test('new browser runtime requires exact current payload/source/assets, all changed behaviors and actual renderer sandbox',t=>{
 const x=lineage(t),file=path.join(x.f.root,'runtime.json'),log={...ref(put(path.join(x.f.root,'PROVENANCE/selected-browser-evidence/runtime.log'),'synthetic source-only witness')),path:'PROVENANCE/selected-browser-evidence/runtime.log'};
 const good={schema:1,kind:'desktop-selected-browser-runtime',scope:'same-target-selected-browser',status:'PASS',target:x.s.target,version:'0.12.1',payloadManifestSha256:x.s.payloadManifestSha256,stageReceiptSha256:x.s.stageReceiptSha256,freezeReceiptSha256:x.s.release.freezeReceipt.sha256,selectedBrowserReceiptSha256:x.s.selectedBrowser.receipt.sha256,webManifestSha256:x.s.selectedBrowser.webManifestSha256,sourceCommit:x.s.selectedBrowser.sourceCommit,sourceTree:x.s.selectedBrowser.sourceTree,checks:Object.fromEntries(['authenticatedOrigin','newAssetsLoaded','routeRetention','draftRetention','inlineBoundedMore','explicitRetry'].map(k=>[k,true])),rendererSandbox:{processType:'renderer',osEnforced:true,flags:[]},evidence:[log]};
 const validate=r=>overlay.currentRuntime(ref(write(file,r)),x.s,x.s.payloadManifestSha256,utils());const accepted=validate(good);assert.equal(accepted.receipt.kind,good.kind);const copied=path.join(x.f.root,'evidence-only');overlay.copyRuntimeEvidence(accepted,copied,utils());assert(fs.readFileSync(path.join(copied,log.path)).equals(fs.readFileSync(path.join(x.f.root,log.path))));assert.throws(()=>overlay.copyRuntimeEvidence(accepted,copied,utils()),/exist/);
 for(const mutate of [r=>r.target='linux-arm64',r=>r.status='PENDING',r=>r.payloadManifestSha256='0'.repeat(64),r=>r.sourceCommit='0'.repeat(40),r=>r.webManifestSha256='0'.repeat(64),r=>r.checks.explicitRetry=false,r=>r.checks.inlineBoundedMore=false,r=>r.rendererSandbox.osEnforced=false,r=>r.rendererSandbox.processType='browser',r=>r.rendererSandbox.flags=['--no-sandbox'],r=>r.rendererSandbox.flags=['--disable-namespace-sandbox'],r=>r.rendererSandbox.flags=['--disable-gpu-sandbox=true'],r=>r.evidence=[]]){const r=structuredClone(good);mutate(r);assert.throws(()=>validate(r));}
});
test('current stage provenance carries the exact raw receipt and independently verifiable hash',t=>{
 const x=lineage(t),file=path.join(x.stage,'STAGE.json'),out=path.join(x.f.root,'provenance-only'),bound=overlay.stageProvenance(file,out,utils());
 assert.equal(bound.stageReceiptSha256,x.s.stageReceiptSha256);assert.equal(bound.stageReceipt.sha256,p.digest(file));
 assert(fs.readFileSync(path.join(out,bound.stageReceipt.path)).equals(fs.readFileSync(file)));
 assert.throws(()=>overlay.stageProvenance(file,out,utils()),/exist/);
});
test('composer and direct producer require the root-selected native service artifact pin',()=>{
 const sha='a'.repeat(64);
 for(const target of ['linux-x64','linux-arm64','darwin-arm64','windows-x64']){
  // every target, Windows included, pins the service component's SERVICE.json
  const key='serviceReceiptSha256';
  overlay.nativeServicePin({[key]:sha},target,{[key]:sha});
  assert.throws(()=>overlay.nativeServicePin({},target,{[key]:sha}),/native service/);
  assert.throws(()=>overlay.nativeServicePin({[key]:sha},target,{[key]:'b'.repeat(64)}),/native service/);
 }
 const composerSource=fs.readFileSync(path.join(__dirname,'../compose-target-freeze.cjs'),'utf8');
 assert(composerSource.includes('browserOverlay.nativeServicePin(allowed,target,pins)'));
});
test('stage preserves the actual pinned root policy bytes for independent collector authority checks',t=>{
 const x=lineage(t),bound=x.s.components.rootStagingDisposition;
 assert.equal(bound.sha256,x.f.freeze.rootStagingDisposition.sha256);assert.equal(bound.bytes,x.f.freeze.rootStagingDisposition.bytes);
 const file=path.join(x.stage,bound.path);assert(fs.readFileSync(file).equals(fs.readFileSync(x.f.request.selectedBrowser.rootDisposition.path)));
 const view={target:x.s.target,release:x.s.release,freeze:x.f.freeze};assert.equal(overlay.rootAuthority({...bound,path:file},view,utils()).publicationOwner,'root');
 fs.appendFileSync(file,'changed');assert.throws(()=>overlay.rootAuthority({...bound,path:file},view,utils()),/hash\/size/);
});
test('runtime rawrefs enforce agreed namespace/count/individual/total bounds before opening files',()=>{
 const one={path:'PROVENANCE/selected-browser-evidence/report.json',sha256:'a'.repeat(64),bytes:1},u={reference(){throw Error('IO must not run');}};
 for(const evidence of [[],Array.from({length:17},(_,i)=>({...one,path:one.path.replace('report','report'+i)})),[{...one,path:'/tmp/report.json'}],[{...one,path:'PROVENANCE/selected-browser-evidence/../report.json'}],[{...one,path:'PROVENANCE/selected-browser-evidence/a..json'}],[one,{...one}], [{...one,bytes:0}],[{...one,bytes:8*1024*1024+1}],Array.from({length:5},(_,i)=>({...one,path:one.path.replace('report','report'+i),bytes:8*1024*1024}))])assert.throws(()=>overlay.runtimeEvidence({evidence},u),e=>!/IO must not run/.test(e.message));
});
test('runtime rawrefs refuse symlink/nonregular or hash-mismatched source files',t=>{
 const f=fixture(t);browser(f);const dir=path.join(f.root,'PROVENANCE/selected-browser-evidence');fs.mkdirSync(dir,{recursive:true});
 const file=put(path.join(dir,'actual.log'),'fact'),good={...ref(file),path:'PROVENANCE/selected-browser-evidence/actual.log'};
 fs.symlinkSync('actual.log',path.join(dir,'linked.log'));
 for(const bad of [{...good,path:'PROVENANCE/selected-browser-evidence/linked.log'},{...good,sha256:'0'.repeat(64)}]){
  const envelope=write(path.join(f.root,'raw-evidence.json'),{evidence:[bad]});assert.throws(()=>overlay.runtimeEvidence(p.json(envelope),utils()));
 }
 fs.renameSync(dir,dir+'-real');fs.symlinkSync('selected-browser-evidence-real',dir);const envelope=write(path.join(f.root,'linked-parent-evidence.json'),{evidence:[good]});assert.throws(()=>overlay.runtimeEvidence(p.json(envelope),utils()),/parent/);
});
