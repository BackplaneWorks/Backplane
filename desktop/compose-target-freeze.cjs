'use strict';
// Root-authorized deterministic artifact binding; no acceptance or product IO.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const p=require('./produce-release.cjs'),browserOverlay=require('./selected-browser.cjs');
const POLICY='fee94f08b3ce7d909a1265d9f33820ea816887703e940ca0d7a57374f9509314';
const ref=file=>({path:path.resolve(file),sha256:p.digest(file),bytes:fs.statSync(file).size});
function options(argv){const keys=['--policy','--components','--service','--out'],optional='--selected-browser';assert([8,10].includes(argv.length),'Use '+keys.join(', '));const a={};for(let i=0;i<argv.length;i+=2){assert((keys.includes(argv[i])||argv[i]===optional)&&!a[argv[i]],'Unknown/duplicate option');assert(path.isAbsolute(argv[i+1]),'Absolute paths required');a[argv[i]]=argv[i+1];}assert(keys.every(k=>a[k]),'Missing required option');return a;}
function policyIdentity(policy,components,policySha){
  assert.equal(policySha,POLICY,'Untrusted root disposition');assert.equal(policy.kind,'backplane-root-staging-disposition');assert.equal(policy.status,'AUTHORIZED_STAGING_POLICY_PENDING_ACTUAL_SERVICE_BINDINGS');
  assert.equal(components.kind,'backplane-desktop-prepared-components');assert(p.TARGETS[components.target]);
  for(const key of ['version','tag','channel','commit','tree'])assert.equal(policy[key],components.release[key],'Root source mismatch: '+key);
  assert.equal(policy.version,'0.12.0');assert.equal(policy.electronVersion,'44.9.2');assert.equal(policy.publicationOwner,'root');
  const allowed=policy.targets[components.target];assert(allowed,'Target not authorized');assert.equal(allowed.signingPolicy,'unsigned-portable-allowed');assert.deepEqual(allowed.lifecyclePolicy,{mode:'resident-service',managedStop:false,managedUpdate:false});return allowed;
}
function compose(a){
  const policy=p.json(a['--policy']),components=p.json(a['--components']),out=a['--out'];assert(!fs.existsSync(out),'Freeze output exists');
  const allowed=policyIdentity(policy,components,p.digest(a['--policy'])),target=components.target;
  const executionTooling={...policy.tooling,serviceRuntimeFactorySha256:allowed.serviceRuntimeFactorySha256??policy.tooling.serviceRuntimeFactorySha256};
  assert.equal(p.digest(a['--components']),allowed.componentInputs.sha256,'Prepared target input changed');assert.equal(fs.statSync(a['--components']).size,allowed.componentInputs.bytes);
  const abs=spec=>({receipt:ref(p.reference(spec.receipt)),payload:{directory:path.resolve(path.dirname(a['--components']),spec.payload.directory)}});
  const shell=abs(components.shell),electron=abs(components.electron);assert.equal(shell.receipt.sha256,allowed.shellReceiptSha256);assert.equal(electron.receipt.sha256,allowed.electronReceiptSha256);
  assert.equal(p.digest(path.join(__dirname,'prepare-native-stage.cjs')),policy.tooling.stageAdapterSha256,'Stage execution tool differs from root policy');assert.equal(p.digest(path.join(__dirname,'test/release-smoke.cjs')),policy.tooling.nativeSmokeSha256,'Smoke execution tool differs from root policy');
  const temp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-target-freeze-'));
  try{
    // Every target, Windows included, binds the JS service's SERVICE.json component.
    const service={receipt:ref(a['--service']),payload:{directory:path.join(path.dirname(a['--service']),'payload')}},web=path.join(service.payload.directory,'WEB.json');
    assert.equal(p.digest(path.join(service.payload.directory,'EXECUTION-HELPER.py')),policy.tooling.serviceComponentProducerSha256,'Configured native helper differs from root policy');assert.equal(p.digest(path.join(service.payload.directory,'runtime-evidence/SERVICE-RUNTIME-PRODUCER.py')),executionTooling.serviceRuntimeFactorySha256,'Runtime factory differs from root policy');
    const pins={shellReceiptSha256:shell.receipt.sha256,electronReceiptSha256:electron.receipt.sha256,serviceReceiptSha256:service.receipt.sha256,webManifestSha256:p.digest(web)};
    const selectedBrowser=a['--selected-browser']?browserOverlay.input(a['--selected-browser'],p,ref(a['--policy'])):null;
    assert(Boolean(selectedBrowser)===Boolean(allowed.selectedBrowser),'Root selected browser/input must both be present');
    if(selectedBrowser){browserOverlay.nativeServicePin(allowed,target,pins);assert.equal(allowed.selectedBrowser.resourceWebManifestSha256,pins.webManifestSha256,'Root must retain original resourceWeb');pins.selectedBrowserReceiptSha256=allowed.selectedBrowser.receipt.sha256;}
    const freeze={schema:1,kind:'backplane-release-freeze',status:'FROZEN',...components.release,sourceDateEpoch:policy.sourceDateEpoch,electronVersion:policy.electronVersion,compileInputs:policy.compileInputs,components:{[target]:pins},signingPolicy:{[target]:allowed.signingPolicy},lifecyclePolicy:{[target]:allowed.lifecyclePolicy},rootStagingDisposition:{sha256:POLICY,bytes:fs.statSync(a['--policy']).size},...(allowed.windowsShellSource?{windowsShellSource:allowed.windowsShellSource}:{}),executionTooling,...(selectedBrowser?{selectedBrowser:allowed.selectedBrowser,resourceWeb:{[target]:{sourceCommit:components.release.commit,sourceTree:components.release.tree,webManifestSha256:pins.webManifestSha256}}}:{}),scope:'Root-authorized source/artifact staging freeze for this target only. Actual desktop/sandbox/runtime acceptance remains pending.',published:false};
    const candidate=path.join(temp,'freeze.json');p.write(candidate,freeze);
    const request=path.join(temp,'request.json');p.write(request,{schema:1,kind:'backplane-electron-release-inputs',target,release:{...components.release,freezeReceipt:ref(candidate)},shell,electron,service,...(selectedBrowser?{selectedBrowser}:{})});
    // Reuse actual producer validation: configured stable original BUILD, full
    // membership/architecture/closure/notices, matching web+gzip and exact policy.
    p.validateRequest(p.json(request),temp);
    fs.copyFileSync(candidate,out,fs.constants.COPYFILE_EXCL);return {target,status:'FROZEN_ACTUAL_ARTIFACT_BINDINGS_NOT_RUNTIME_ACCEPTANCE',freeze:ref(out),components:pins,published:false};
  }finally{fs.rmSync(temp,{recursive:true,force:true});}
}
function main(argv){if(argv.length===1&&argv[0]==='--help'){console.log('node desktop/compose-target-freeze.cjs --policy ABS_ROOT_DISPOSITION --components ABS_PREPARED_COMPONENT_INPUTS --service ABS_SERVICE_JSON --out NEW_ABS_FREEZE_JSON [--selected-browser ABS_BROWSER_INPUT_JSON]\nPins authorized actual artifact bytes after existing complete stager validation. No runtime or acceptance claim.');return;}console.log(JSON.stringify(compose(options(argv)),null,2));}
module.exports={options,policyIdentity,compose};
if(require.main===module)try{main(process.argv.slice(2));}catch(e){console.error('Target freeze: '+e.message);process.exitCode=1;}
