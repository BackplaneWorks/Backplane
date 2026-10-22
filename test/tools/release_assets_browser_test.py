"""New synthetic browser-composition controls; no product or runtime execution."""
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import tarfile
import zipfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('collector', Path(__file__).resolve().parents[2]/'scripts/release-assets.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)


def encode(v):
    return (json.dumps(v, sort_keys=True) + '\n').encode()


def binding(b):
    return {'sha256': hashlib.sha256(b).hexdigest(), 'bytes': len(b)}


class Fixture:
    def __init__(self, target='darwin-arm64', composite=True):
        self.target = target
        self.root = 'backplane-' + target
        self.resources = self.root + ('/Backplane.app/Contents/Resources' if target == 'darwin-arm64' else '/resources' if target == 'windows-x64' else '/runtime/resources')
        self.service = self.resources + '/backplane-service'
        self.files = {}; self.modes = {}
        self.source, self.tree = C.CORRECTED_SERVICE_SOURCE
        self.resident = {'mode':'resident-service', 'managedStop':False, 'managedUpdate':False}
        old_web = self.web({'index.html':b'old browser'}, C.CORRECTED_SERVICE_SOURCE)
        self.put(self.service+'/WEB.json', old_web)
        self.put(self.service+'/backplane-serve'+('.exe' if target == 'windows-x64' else ''), b'not executable fixture')
        self.put(self.service+'/LICENSE.Backplane', b'fixture license')
        build = {'commit':self.source, 'tree':self.tree, 'entrypoint':'src/server/main.bend', 'serviceVersion':'0.12.0', 'versionStamp':'0.12.0', 'telemetryChannel':'release', 'compileInputs':{'version':'0.12.0','versionStamp':'0.12.0','telemetryChannel':'release','google':{'requested':True,'requiredInputsPresent':True},'telemetry':{'requested':True,'requiredInputsPresent':True,'endpoint':'https://backplane.works/api/v1/events'}}}
        self.service_name = 'backplane-serve' + ('.exe' if target == 'windows-x64' else '')
        if target == 'windows-x64':
            build.update(serviceRuntime='bun', service=self.service_name)
        self.put(self.service+'/BUILD.json', build)
        service_members = {n[len(self.service)+1:]:dict(binding(b),type='file',mode=self.modes[n]) for n,b in self.files.items() if n.startswith(self.service+'/')}
        build_ref = self.ref('PROVENANCE/service-build.json',build)
        service = {'kind':'backplane-bend-service','status':'VERIFIED','sourceCommit':self.source,'sourceTree':self.tree,'target':target,'entrypoint':'src/server/main.bend','genuineService':True,'roles':{'service':self.service_name,'web':'web'},'members':service_members,'buildReceipt':build_ref}
        components = {'service':self.ref('PROVENANCE/service.json', service),'shell':self.ref('PROVENANCE/shell.json',{'fixture':'shell'}),'electron':self.ref('PROVENANCE/electron.json',{'fixture':'electron'})}
        components['serviceBuild']=build_ref
        self.put(self.resources+'/app/package.json', {'version':'0.12.0','main':'main.cjs'})
        old_freeze = {'schema':1,'kind':'backplane-release-freeze','status':'FROZEN','commit':self.source,'tree':self.tree,'signingPolicy':{target:'unsigned-portable-allowed'},'lifecyclePolicy':{target:self.resident}}
        old_freeze_ref=self.ref('PROVENANCE/freeze.json',old_freeze)
        components['freeze']=old_freeze_ref
        release={'commit':self.source,'tree':self.tree,'version':'0.12.0','tag':'v0.12.0','channel':'release','freezeReceipt':old_freeze_ref}
        self.old={'schema':1,'kind':'backplane-electron-desktop-package','target':target,'release':release,'components':components,'lifecyclePolicy':self.resident,'webManifestSha256':binding(encode(old_web))['sha256']}
        self.old['payloadManifestSha256']=C.content_digest(self.payload())
        for k,kind in [('signing','desktop-signing'),('sandbox','desktop-sandbox'),('lifecycle','desktop-lifecycle'),('runtimeAcceptance','desktop-runtime-acceptance')]:
            proof={'schema':1,'kind':kind,'status':'PASS','target':target,'commit':self.source,'tree':self.tree,'version':'0.12.0','payloadManifestSha256':self.old['payloadManifestSha256'],'freezeReceiptSha256':old_freeze_ref['sha256']}
            if k=='signing':proof['signed']=False
            if k=='sandbox':proof.update(osEnforced=True,flags=[])
            if k=='runtimeAcceptance':proof['scope']='desktop-release'
            if k=='lifecycle':proof.update(nativeChildIdentity=True,authBound=True,singleInstance=True,reconnect=True,startup=True,closePreservesService=True,windowClose='resident-service',shutdown='unsupported',managedStopAdvertised=False,managedUpdateAdvertised=False,nativeToPosixPaths=True)
            self.old[k]=dict(self.ref('PROVENANCE/'+k+'.json',proof),status='PASS')
        self.old['members']=self.inventory_relative()
        self.receipt=copy.deepcopy(self.old)
        self.receipt.pop('members')
        for n in list(self.files):
            if n.startswith(self.service+'/web/'):del self.files[n];del self.modes[n]
        new_web=self.web({'index.html':b'new browser','LICENSE':b'fixture license'},C.SELECTED_BROWSER_SOURCE)
        self.manifest_pin=binding(encode(new_web))
        browser={'schema':1,'kind':'backplane-selected-browser','status':'VERIFIED','version':'0.12.0','channel':'release','sourceCommit':C.SELECTED_BROWSER_SOURCE[0],'sourceTree':C.SELECTED_BROWSER_SOURCE[1],'entrypoint':'src/web/index.html','webManifest':'WEB.json','webRoot':'web','webManifestSha256':self.manifest_pin['sha256'],'packages':[{'licenses':[{'path':'LICENSE',**binding(b'fixture license')}]}]}
        browser['members']={n:dict(v,type='file',mode=0o644) for n,v in {'WEB.json':self.manifest_pin,**{'web/'+n:v for n,v in new_web['members'].items()}}.items()}
        br=self.ref('PROVENANCE/selected-browser.json',browser)
        self.receipt['components']['selectedBrowser']=br
        self.receipt['components']['selectedBrowserWeb']=self.ref('PROVENANCE/selected-browser-web.json',new_web)
        pin={'mode':'generated-web-subtree-only','sourceCommit':C.SELECTED_BROWSER_SOURCE[0],'sourceTree':C.SELECTED_BROWSER_SOURCE[1],'receipt':{k:br[k] for k in ('sha256','bytes')},'webManifestSha256':self.manifest_pin['sha256'],'resourceWebManifestSha256':self.old['webManifestSha256']}
        if composite:pin['baselineDesktopReceiptSha256']=binding(encode(self.old))['sha256']
        resource={'sourceCommit':self.source,'sourceTree':self.tree,'webManifestSha256':self.old['webManifestSha256']}
        policy={'kind':'backplane-root-staging-disposition','status':'AUTHORIZED_STAGING_POLICY_PENDING_ACTUAL_SERVICE_BINDINGS','publicationOwner':'root','electronVersion':'44.9.2',**{k:release[k] for k in ('version','tag','channel','commit','tree')},'compileInputs':build['compileInputs'],'targets':{target:{'selectedBrowser':pin,'signingPolicy':'unsigned-portable-allowed','lifecyclePolicy':self.resident,'shellReceiptSha256':components['shell']['sha256'],'electronReceiptSha256':components['electron']['sha256']}}}
        service_key='serviceReceiptSha256'
        service_sha=components['service']['sha256']
        policy['targets'][target][service_key]=service_sha
        policy_ref=self.ref('PROVENANCE/root-staging-disposition.json',policy)
        self.policy={k:policy_ref[k] for k in ('sha256','bytes')}
        self.receipt['components']['rootStagingDisposition']=policy_ref
        frozen=dict(old_freeze,compileInputs=build['compileInputs'],selectedBrowser=pin,rootStagingDisposition=self.policy,resourceWeb={target:resource},components={target:{'webManifestSha256':resource['webManifestSha256'],'selectedBrowserReceiptSha256':br['sha256'],service_key:service_sha}})
        self.receipt['release']['freezeReceipt']=self.ref('PROVENANCE/freeze.json',frozen)
        self.receipt['components']['freeze']=self.receipt['release']['freezeReceipt']
        self.receipt.update(resourceWeb=resource,selectedBrowser=dict(pin,rootStagingDisposition=self.policy),webManifestSha256=self.manifest_pin['sha256'])
        self.receipt['payloadManifestSha256']=C.content_digest(self.payload())
        stage={'kind':'backplane-electron-desktop-stage','target':target,'payloadManifestSha256':self.receipt['payloadManifestSha256'],'selectedBrowser':self.receipt['selectedBrowser'],'resourceWeb':resource,'release':self.receipt['release'],'components':copy.deepcopy(self.receipt['components'])}
        self.receipt['stageReceipt']=self.ref('PROVENANCE/selected-browser-stage.json',stage)
        self.receipt['stageReceiptSha256']=self.receipt['stageReceipt']['sha256']
        runtime={'schema':1,'kind':'desktop-selected-browser-runtime','scope':'same-target-selected-browser','status':'PASS','target':target,'version':'0.12.0','sourceCommit':C.SELECTED_BROWSER_SOURCE[0],'sourceTree':C.SELECTED_BROWSER_SOURCE[1],'payloadManifestSha256':self.receipt['payloadManifestSha256'],'stageReceiptSha256':self.receipt['stageReceiptSha256'],'freezeReceiptSha256':self.receipt['release']['freezeReceipt']['sha256'],'selectedBrowserReceiptSha256':br['sha256'],'webManifestSha256':self.manifest_pin['sha256'],'checks':{k:True for k in ('authenticatedOrigin','newAssetsLoaded','routeRetention','draftRetention','inlineBoundedMore','explicitRetry')},'rendererSandbox':{'processType':'renderer','osEnforced':True,'flags':[]},'evidence':[dict(binding(b'actual synthetic raw report'),path='PROVENANCE/selected-browser-evidence/session-report.json')]}
        self.put(self.root+'/PROVENANCE/selected-browser-evidence/session-report.json',b'actual synthetic raw report')
        rr=dict(self.ref('PROVENANCE/selected-browser-runtime.json',runtime),status='PASS')
        if composite:
            baseline={'desktop':self.ref('PROVENANCE/baseline-desktop.json',self.old),'freeze':self.ref('PROVENANCE/baseline-freeze.json',old_freeze),'proofs':{}}
            for k in ('signing','sandbox','lifecycle','runtimeAcceptance'):
                raw=json.loads(self.files[self.root+'/'+self.old[k]['path']]);baseline['proofs'][k]=dict(self.ref('PROVENANCE/baseline-'+k+'.json',raw),status='PASS');self.receipt.pop(k)
            immutable={k:v for k,v in components.items() if k!='freeze'}
            self.receipt['acceptance']={'kind':'desktop-browser-composite','scope':'retained-native-baseline+selected-browser','baseline':baseline,'selectedBrowserRuntime':rr,'baselineDesktopSha256':baseline['desktop']['sha256'],'baselinePayloadManifestSha256':self.old['payloadManifestSha256'],'baselineFreezeReceiptSha256':baseline['freeze']['sha256'],'immutableComponents':immutable,'selectedBrowser':self.receipt['selectedBrowser']}
        else:
            self.receipt['selectedBrowserRuntime']=rr
            for k in ('signing','sandbox','lifecycle','runtimeAcceptance'):
                raw=json.loads(self.files[self.root+'/'+self.receipt[k]['path']]);raw.update(payloadManifestSha256=self.receipt['payloadManifestSha256'],freezeReceiptSha256=self.receipt['release']['freezeReceipt']['sha256']);self.receipt[k]=dict(self.ref(self.receipt[k]['path'],raw),status='PASS')

    def put(self,n,v):
        self.files[n]=v if isinstance(v,bytes) else encode(v);self.modes[n]=0o644
    def ref(self,n,v):
        self.put(self.root+'/'+n,v);return dict(binding(self.files[self.root+'/'+n]),path=n)
    def web(self,files,source):
        files.update({n+'.gz':gzip.compress(b,mtime=0) for n,b in list(files.items()) if n.endswith(('.html','.js','.css'))})
        for n,b in files.items():self.put(self.service+'/web/'+n,b)
        return {'schema':1,'kind':'matching-generated-web','sourceCommit':source[0],'sourceTree':source[1],'entrypoint':'src/web/index.html','root':'web','members':{n:binding(b) for n,b in files.items()}}
    @property
    def links(self):return {}
    @property
    def entries(self):
        result={}
        for n,b in self.files.items():
            info=tarfile.TarInfo(n);info.size=len(b);info.mode=self.modes[n];result[n]=info
        return result
    def inventory(self):return {n:binding(b) for n,b in self.files.items()}
    def inventory_relative(self):return {n[len(self.root)+1:]:dict(binding(b),type='file',mode=self.modes[n]) for n,b in self.files.items()}
    def payload(self):return C.payload_members(self.inventory_relative())
    def mode(self,n):return self.modes[n]
    def read(self,n,limit=8*1024*1024):return self.files[n]
    def change_runtime(self,mutate):
        ref=self.receipt.get('acceptance',{}).get('selectedBrowserRuntime',self.receipt.get('selectedBrowserRuntime'))
        v=json.loads(self.read(self.root+'/'+ref['path']));mutate(v);ref.update(self.ref(ref['path'],v))
    def check(self):
        with patch.object(C,'SELECTED_BROWSER_MANIFEST',self.manifest_pin),patch.object(C,'SELECTED_BROWSER_STAGING_DISPOSITION_SHA256',self.policy['sha256']):
            C.original_resource_web(self,self.root,self.service,self.receipt,self.source,self.tree,'v0.12.0')
            C.selected_browser(self,self.root,self.service,self.receipt,self.target,'0.12.0')
            if self.receipt.get('acceptance'):C.composite_acceptance(self,self.root,self.resources,self.receipt,self.target,self.source,self.tree,'0.12.0')
            else:C.desktop_acceptance(self,self.root,self.receipt,self.target,self.source,self.tree,'0.12.0')


class SourceRoles(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.disposition={'schema':1,'kind':'backplane-root-component-provenance-disposition','status':'SOURCE_SCOPES_ACCEPTED_RUNTIME_GATES_REMAIN','release':'v0.12.0','correctedServiceSource':dict(zip(('commit','tree'),C.CORRECTED_SERVICE_SOURCE)), 'selectedBrowserSource':dict(zip(('commit','tree'),C.SELECTED_BROWSER_SOURCE)), 'selectedBrowserManifest':C.SELECTED_BROWSER_MANIFEST,'selectedBrowserWebArchive':C.SELECTED_WEB_ARCHIVE,'desktopSelectedBrowserRequired':[t for t,surface in C.RELEASE_TARGETS.items() if surface=='desktop'],'targetComponentSources':{t:dict(zip(('commit','tree'),C.SELECTED_BROWSER_SOURCE if t=='web' else C.RETAINED_COMPONENT_SOURCE if t in {'android','ios'} else C.CORRECTED_SERVICE_SOURCE)) for t in C.RELEASE_TARGETS}}
        original=encode(self.disposition)
        self.file=self.root/'disposition.json';self.file.write_bytes(original)
        self.ref=dict(binding(original),path=self.file.name)
        pin=patch.object(C,'BROWSER_DISPOSITION_SHA256',self.ref['sha256']);pin.start();self.addCleanup(pin.stop)
        self.inputs={'componentProvenanceDisposition':self.ref,'targets':{t:{'componentSource':v} for t,v in self.disposition['targetComponentSources'].items()}}
    def check(self):return C.component_sources(self.root,self.inputs,*C.CORRECTED_SERVICE_SOURCE,'v0.12.0')
    def test_exact_new_root_disposition_seven_roles(self):
        self.assertEqual(self.ref['sha256'],C.BROWSER_DISPOSITION_SHA256)
        self.assertEqual(self.check()[0],self.disposition['targetComponentSources'])
    def test_new_policy_old_web_rejected(self):
        self.inputs['targets']['web']['componentSource']=dict(zip(('commit','tree'),C.RETAINED_COMPONENT_SOURCE));self.assertRaisesRegex(ValueError,'all seven',self.check)
    def test_new_policy_arbitrary_mobile_source_rejected(self):
        self.inputs['targets']['android']['componentSource']=dict(zip(('commit','tree'),C.SELECTED_BROWSER_SOURCE));self.assertRaisesRegex(ValueError,'all seven',self.check)
    def test_new_policy_missing_desktop_scope_rejected(self):
        self.inputs['targets']['linux-x64'].pop('componentSource');self.assertRaisesRegex(ValueError,'all seven',self.check)
    def test_new_policy_requires_selected_browser_on_first_desktop(self):
        self.inputs.update(schema=1,kind='backplane-electron-release-inputs',rootFrozen=True,sourceCommit=C.CORRECTED_SERVICE_SOURCE[0],sourceTree=C.CORRECTED_SERVICE_SOURCE[1],tag='v0.12.0',version='0.12.0',channel='release')
        class ReachedTarget(Exception):pass
        def target(*args,**kwargs):
            self.assertEqual(args[1],'linux-x64');self.assertIs(kwargs['browser_required'],True);raise ReachedTarget()
        with patch.object(C,'verify_target',target):self.assertRaises(ReachedTarget,C.verify_inputs,self.root,self.inputs,C.CORRECTED_SERVICE_SOURCE[0],'v0.12.0')
    def test_unapproved_disposition_rejected(self):
        self.ref['sha256']='f'*64;self.assertRaises(ValueError,self.check)
    def test_original_policy_still_retains_only_explicit_920_scopes(self):
        raw=encode({k:self.disposition[k] for k in ('schema','kind','status','release','correctedServiceSource')})
        pin=patch.object(C,'COMPONENT_DISPOSITION_SHA256',binding(raw)['sha256']);pin.start();self.addCleanup(pin.stop)
        self.file.write_bytes(raw);self.ref.clear();self.ref.update(binding(raw),path=self.file.name)
        self.inputs['targets']={t:{} for t in C.RELEASE_TARGETS}
        self.inputs['targets']['web']['componentSource']=dict(zip(('commit','tree'),C.RETAINED_COMPONENT_SOURCE))
        self.assertEqual(self.check()[0]['web'],self.inputs['targets']['web']['componentSource'])
    def test_old_policy_cannot_authorize_new_web_source(self):
        self.test_original_policy_still_retains_only_explicit_920_scopes()
        self.inputs['targets']['web']['componentSource']=dict(zip(('commit','tree'),C.SELECTED_BROWSER_SOURCE));self.assertRaisesRegex(ValueError,'not authorized',self.check)


class BrowserControls(unittest.TestCase):
    def test_mac_composite(self):Fixture().check()
    def test_windows_composite_original_resource_closure(self):Fixture('windows-x64').check()
    def test_linux_x64_full_current_proofs(self):Fixture('linux-x64',False).check()
    def test_linux_arm_full_current_proofs(self):Fixture('linux-arm64',False).check()
    def test_linux_cannot_inherit_composite(self):
        f=Fixture('linux-arm64');self.assertRaisesRegex(ValueError,'only Mac/Windows',f.check)
    def test_missing_real_policy_refuses(self):
        f=Fixture()
        with patch.object(C,'SELECTED_BROWSER_MANIFEST',f.manifest_pin):self.assertRaisesRegex(ValueError,'exact root staging',C.selected_browser,f,f.root,f.service,f.receipt,f.target,'0.12.0')
    def test_changed_native_fails_before_browser(self):
        f=Fixture();f.put(f.service+'/backplane-serve',b'changed native');self.assertRaisesRegex(ValueError,'native SERVICE closure',f.check)
    def test_changed_original_windows_compile_resource(self):
        f=Fixture('windows-x64');f.put(f.service+'/backplane-serve.exe',b'changed service');self.assertRaisesRegex(ValueError,'SERVICE closure',f.check)
    def test_changed_original_web_manifest(self):
        f=Fixture();f.put(f.service+'/WEB.json',{'tampered':True});self.assertRaisesRegex(ValueError,'resourceWeb identity',f.check)
    def test_changed_nonweb_shell_rejected(self):
        f=Fixture();f.put(f.resources+'/app/package.json',{'changed':'shell'});self.assertRaisesRegex(ValueError,'non-web',f.check)
    def test_browser_wrong_target_runtime(self):
        f=Fixture();f.change_runtime(lambda r:r.update(target='windows-x64'));self.assertRaisesRegex(ValueError,'same-target',f.check)
    def test_browser_old_payload_runtime(self):
        f=Fixture();f.change_runtime(lambda r:r.update(payloadManifestSha256=f.old['payloadManifestSha256']));self.assertRaisesRegex(ValueError,'current stage/payload',f.check)
    def test_browser_wrong_stage_runtime(self):
        f=Fixture();f.change_runtime(lambda r:r.update(stageReceiptSha256='a'*64));self.assertRaisesRegex(ValueError,'current stage/payload',f.check)
    def test_namespace_sandbox_bypass_refuses(self):
        f=Fixture();f.change_runtime(lambda r:r['rendererSandbox']['flags'].append('--disable-namespace-sandbox'));self.assertRaisesRegex(ValueError,'renderer OS sandbox',f.check)
    def test_gpu_sandbox_bypass_refuses(self):
        f=Fixture();f.change_runtime(lambda r:r['rendererSandbox']['flags'].append('--disable-gpu-sandbox'));self.assertRaisesRegex(ValueError,'renderer OS sandbox',f.check)
    def test_more_not_exercised_refuses(self):
        f=Fixture();f.change_runtime(lambda r:r['checks'].update(inlineBoundedMore=False));self.assertRaisesRegex(ValueError,'behavior incomplete',f.check)
    def test_old_proof_relabel_refuses(self):
        f=Fixture();a=f.receipt['acceptance'];ref=a['baseline']['proofs']['runtimeAcceptance'];v=json.loads(f.read(f.root+'/'+ref['path']));v['payloadManifestSha256']=f.receipt['payloadManifestSha256'];ref.update(f.ref(ref['path'],v));self.assertRaisesRegex(ValueError,'baseline proof reference',f.check)
    def test_composite_no_top_level_full_pass(self):
        f=Fixture();f.receipt['runtimeAcceptance']=f.old['runtimeAcceptance'];self.assertRaisesRegex(ValueError,'only Mac/Windows',f.check)
    def test_current_stage_raw_bytes_changed(self):
        f=Fixture();f.put(f.root+'/'+f.receipt['stageReceipt']['path'],{'tampered':'stage'});self.assertRaisesRegex(ValueError,'reference changed',f.check)
    def test_root_policy_raw_bytes_changed(self):
        f=Fixture();f.put(f.root+'/'+f.receipt['components']['rootStagingDisposition']['path'],{'tampered':'root'});self.assertRaisesRegex(ValueError,'reference changed',f.check)
    def test_root_policy_target_native_pin_required(self):
        f=Fixture();ref=f.receipt['components']['rootStagingDisposition'];v=json.loads(f.read(f.root+'/'+ref['path']));v['targets'][f.target]['serviceReceiptSha256']='a'*64
        newref=f.ref(ref['path'],v);ref.update(newref);f.policy.update({k:newref[k] for k in ('sha256','bytes')})
        # Keep the claimed policy descriptor paired; actual target contents still refuse.
        f.receipt['selectedBrowser']['rootStagingDisposition'].update(f.policy)
        fr=f.receipt['release']['freezeReceipt'];freeze=json.loads(f.read(f.root+'/'+fr['path']));freeze['rootStagingDisposition']=f.policy;fr.update(f.ref(fr['path'],freeze));f.receipt['components']['freeze']=fr
        self.assertRaisesRegex(ValueError,'original native service/archive',f.check)
    def test_old_baseline_proof_missing_refuses(self):
        f=Fixture();del f.receipt['acceptance']['baseline']['proofs']['sandbox'];self.assertRaisesRegex(ValueError,'baseline proof reference',f.check)
    def test_selected_manifest_tamper(self):
        f=Fixture();ref=f.receipt['components']['selectedBrowserWeb'];f.put(f.root+'/'+ref['path'],{'changed':True});self.assertRaisesRegex(ValueError,'reference changed',f.check)


class EvidencePackage:
    def __init__(self, zip_mode=False):
        self.entries={};self.links={};self.data={}
        self.add('pkg/PROVENANCE/selected-browser-evidence/session-report.json',b'raw report',zip_mode)
    def add(self,name,data,zip_mode=False):
        if zip_mode:
            info=zipfile.ZipInfo(name);info.external_attr=0o100644 << 16
        else:
            info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644
        self.entries[name]=info;self.data[name]=data
        return dict(binding(data),path=name[4:])
    def read(self,name,limit=8*1024*1024):
        data=self.data[name]
        if len(data)>limit:raise ValueError('bounded read exceeded')
        return data
    def refs(self):return [dict(binding(b),path=n[4:]) for n,b in self.data.items()]


class RawEvidenceControls(unittest.TestCase):
    def check(self,p,refs=None):return C.selected_browser_evidence(p,'pkg',p.refs() if refs is None else refs)
    def test_actual_tar_regular_raw_bytes_resolve(self):
        p=EvidencePackage();self.assertEqual(self.check(p),set(p.entries))
    def test_actual_zip_regular_raw_bytes_resolve(self):
        p=EvidencePackage(True);self.assertEqual(self.check(p),set(p.entries))
    def test_original_envelope_not_rewritten(self):
        p=EvidencePackage();refs=p.refs();before=encode(refs);self.check(p,refs);self.assertEqual(encode(refs),before)
    def test_absolute_host_reference_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['path']='/home/runner/raw.json';self.assertRaises(ValueError,self.check,p,r)
    def test_traversal_to_payload_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['path']='PROVENANCE/selected-browser-evidence/../../resources/app/main.cjs';self.assertRaises(ValueError,self.check,p,r)
    def test_reference_outside_namespace_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['path']='resources/backplane-service/web/session-report.json';self.assertRaisesRegex(ValueError,'outside fixed namespace',self.check,p,r)
    def test_nested_filename_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['path']=C.BROWSER_EVIDENCE_PREFIX+'nested/raw.json';self.assertRaisesRegex(ValueError,'filename invalid',self.check,p,r)
    def test_duplicate_reference_refused(self):
        p=EvidencePackage();r=p.refs();self.assertRaisesRegex(ValueError,'duplicate or case',self.check,p,r+r)
    def test_case_colliding_reference_refused(self):
        p=EvidencePackage();r=p.refs();other=dict(r[0],path=r[0]['path'].replace('session-report','SESSION-REPORT'));self.assertRaisesRegex(ValueError,'duplicate or case',self.check,p,r+[other])
    def test_missing_raw_file_refused(self):
        p=EvidencePackage();r=p.refs();del p.entries['pkg/'+r[0]['path']];self.assertRaisesRegex(ValueError,'regular file',self.check,p,r)
    def test_raw_file_symlink_refused(self):
        p=EvidencePackage();n=next(iter(p.entries));p.entries[n].type=tarfile.SYMTYPE;p.links[n]='elsewhere';self.assertRaisesRegex(ValueError,'regular file',self.check,p)
    def test_indirect_parent_refused(self):
        p=EvidencePackage();p.links['pkg/PROVENANCE']='elsewhere';self.assertRaisesRegex(ValueError,'indirect parents',self.check,p)
    def test_parent_file_refused(self):
        p=EvidencePackage();p.entries['pkg/PROVENANCE']=tarfile.TarInfo('pkg/PROVENANCE');self.assertRaisesRegex(ValueError,'indirect parents',self.check,p)
    def test_fifo_mode_refused(self):
        p=EvidencePackage();p.entries[next(iter(p.entries))].type=tarfile.FIFOTYPE;self.assertRaisesRegex(ValueError,'regular file',self.check,p)
    def test_zip_nonregular_mode_refused(self):
        p=EvidencePackage(True);p.entries[next(iter(p.entries))].external_attr=0o020644 << 16;self.assertRaisesRegex(ValueError,'regular file',self.check,p)
    def test_changed_raw_bytes_refused(self):
        p=EvidencePackage();r=p.refs();p.data[next(iter(p.data))]=b'changed';self.assertRaisesRegex(ValueError,'bytes changed',self.check,p,r)
    def test_wrong_raw_hash_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['sha256']='f'*64;self.assertRaisesRegex(ValueError,'bytes changed',self.check,p,r)
    def test_empty_raw_file_refused(self):
        p=EvidencePackage();r=p.refs();r[0]['bytes']=0;self.assertRaisesRegex(ValueError,'size outside bounds',self.check,p,r)
    def test_file_size_bound_refused_before_read(self):
        p=EvidencePackage();r=p.refs();r[0]['bytes']=C.BROWSER_EVIDENCE_FILE_LIMIT+1;self.assertRaisesRegex(ValueError,'size outside bounds',self.check,p,r)
    def test_total_size_bound_refused(self):
        p=EvidencePackage();p.add('pkg/'+C.BROWSER_EVIDENCE_PREFIX+'second.txt',b'abcdef')
        with patch.object(C,'BROWSER_EVIDENCE_TOTAL_LIMIT',12):self.assertRaisesRegex(ValueError,'total bytes',self.check,p)
    def test_empty_list_refused(self):self.assertRaisesRegex(ValueError,'count outside',self.check,EvidencePackage(),[])
    def test_file_count_bound_refused(self):
        p=EvidencePackage();r=p.refs();self.assertRaisesRegex(ValueError,'count outside',self.check,p,r*17)
    def test_extra_unreferenced_raw_file_refused(self):
        p=EvidencePackage();r=p.refs();p.add('pkg/'+C.BROWSER_EVIDENCE_PREFIX+'unreferenced.log',b'extra');self.assertRaisesRegex(ValueError,'unreferenced',self.check,p,r)
    def test_safe_basename_without_extension_whitelist(self):
        p=EvidencePackage();p.add('pkg/'+C.BROWSER_EVIDENCE_PREFIX+'original-report',b'raw original');self.assertEqual(self.check(p),set(p.entries))
    def test_oversized_or_double_dot_basename_refused(self):
        for n in ('a'*129+'.json','raw..json'):
            p=EvidencePackage();r=p.refs();r[0]['path']=C.BROWSER_EVIDENCE_PREFIX+n;self.assertRaisesRegex(ValueError,'filename invalid',self.check,p,r)


if __name__=='__main__':unittest.main()
