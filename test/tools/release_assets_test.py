"""Offline package/immutability fixtures, never product or runtime acceptance."""
import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_assets", ROOT / "scripts/release-assets.py")
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)
SHA = "a" * 40
TREE = "b" * 40
TAG = "v0.12.0"
VERSION = "0.12.0"


def encoded(value):
    return (json.dumps(value, sort_keys=True) + "\n").encode()


def item(data):
    return {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def web(root):
    members = {"index.html": b'<script src="chunk-test.js"></script>', "chunk-test.js": b'console.log("offline fixture")'}
    members.update({n + ".gz": gzip.compress(b, mtime=0) for n, b in list(members.items())})
    receipt = {"schema": 1, "kind": "matching-generated-web", "sourceCommit": SHA, "sourceTree": TREE,
               "entrypoint": "src/web/index.html", "root": root, "members": {n: item(b) for n, b in members.items()}}
    return {root + "/" + n: b for n, b in members.items()}, receipt


def service_build():
    return {"commit": SHA, "tree": TREE, "entrypoint": "src/server/main.bend", "serviceVersion": VERSION,
            "versionStamp": VERSION, "telemetryChannel": "release",
            "compileInputs": {"version": VERSION, "versionStamp": VERSION, "telemetryChannel": "release",
                              "google": {"requested": True, "requiredInputsPresent": True},
                              "telemetry": {"requested": True, "requiredInputsPresent": True, "endpoint":"https://backplane.works/api/v1/events"}}}


def archive(path, files):
    if str(path).endswith((".zip", ".apk", ".aab", ".ipa")):
        with zipfile.ZipFile(path, "w") as out:
            for name, data in sorted(files.items()):
                info=zipfile.ZipInfo(name);info.external_attr=(0o100644 << 16);out.writestr(info, data)
    else:
        with tarfile.open(path, "w:gz") as out:
            for name, data in sorted(files.items()):
                info = tarfile.TarInfo(name); info.size = len(data); info.mode = 0o644
                out.addfile(info, io.BytesIO(data))


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.inputs = {"schema": 1, "kind": "backplane-electron-release-inputs", "rootFrozen": True,
                       "sourceCommit": SHA, "sourceTree": TREE, "tag": TAG, "version": VERSION,
                       "channel": "release", "targets": {}}
        for target in assets.RELEASE_TARGETS:
            self.target(target)
        self.save()

    def binding(self, path):
        return dict(item(path.read_bytes()), path=path.name)

    def save(self):
        self.manifest = self.root / "release-inputs.json"
        self.manifest.write_bytes(encoded(self.inputs))

    def target(self, target, mutation=None, proof_mutation=None, resident=None):
        # Windows ships only the resident policy: the JS service has no managed control there.
        resident = target == "windows-x64" if resident is None else resident
        desktop = assets.RELEASE_TARGETS[target] == "desktop"
        if desktop:
            root = "backplane-" + target
            suffix = ".zip" if target == "windows-x64" else ".tar.gz"
            path = self.root / f"backplane-{TAG}-{target}{suffix}"
            resources = root + ("/Backplane.app/Contents/Resources" if target == "darwin-arm64" else
                                "/resources" if target == "windows-x64" else "/runtime/resources")
            service = resources + "/backplane-service"
            payload, wr = web(service + "/web")
            build = service_build()
            payload.update({resources + "/app/package.json": encoded({"version": VERSION, "main": "main.cjs"}),
                            service + "/backplane-serve" + (".exe" if target == "windows-x64" else ""): b'not a product binary',
                            service + "/LICENSE.Backplane": b'offline license fixture',
                            root + "/runtime/LICENSE": b'Electron fixture notice',
                            root + "/runtime/LICENSES.chromium.html": b'Chromium fixture notice'})
            launcher = (root + "/Backplane.app/Contents/MacOS/Backplane" if target == "darwin-arm64" else
                        root + "/Backplane.exe" if target == "windows-x64" else root + "/backplane-open")
            payload[launcher] = b'not a product launcher'
            if target == "windows-x64":
                build.update(serviceRuntime="bun", service="backplane-serve.exe")
            payload[service + "/WEB.json"] = encoded(wr)
            if mutation:
                mutation(payload, build, service)
            payload[service + "/BUILD.json"] = encoded(build)
            payload[root + "/NOTICES.json"] = encoded({"fixture": "not real redistribution evidence"})
            receipt = {"schema": 1, "kind": "backplane-electron-desktop-package", "target": target,
                       "release": {"commit": SHA, "tree": TREE, "version": VERSION, "tag": TAG, "channel": "release"},
                       "electronVersion": "44.9.2", "serviceVersion": VERSION, "shellVersion": VERSION,
                       "noticesManifestSha256": item(payload[root + "/NOTICES.json"])["sha256"],
                       "webManifestSha256": item(payload[service + "/WEB.json"])["sha256"],
                       "members": {p: item(b) for p, b in payload.items()}}
            freeze_data={'signingPolicy':{target:'unsigned-portable-allowed' if resident else 'required'}}
            if resident:freeze_data['lifecyclePolicy']={target:{'mode':'resident-service','managedStop':False,'managedUpdate':False}}
            freeze = encoded(freeze_data)
            freeze_ref = dict(item(freeze), path='PROVENANCE/freeze.json')
            payload[root + '/PROVENANCE/freeze.json'] = freeze
            receipt['release']['freezeReceipt'] = freeze_ref
            if resident:receipt['lifecyclePolicy']=freeze_data['lifecyclePolicy'][target]
            content = {name[len(root)+1:]: dict(item(data),type='file',mode=0o644)
                       for name,data in payload.items() if not name.startswith(root+'/PROVENANCE/')}
            payload_sha=hashlib.sha256(json.dumps(content,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
            receipt['payloadManifestSha256']=payload_sha
            for key,kind in [('signing','desktop-signing'),('sandbox','desktop-sandbox'),
                             ('lifecycle','desktop-lifecycle'),('runtimeAcceptance','desktop-runtime-acceptance')]:
                proof={'schema':1,'kind':kind,'status':'PASS','target':target,'commit':SHA,'tree':TREE,'version':VERSION,
                       'payloadManifestSha256':payload_sha,'freezeReceiptSha256':freeze_ref['sha256']}
                if key=='signing':proof.update(signed=not resident)
                if key=='sandbox':proof.update(osEnforced=True,flags=[])
                if key=='lifecycle':proof.update(nativeChildIdentity=True,authBound=True,singleInstance=True,reconnect=True,
                                                shutdown='approved-service-protocol' if target=='windows-x64' else 'owned-native-child',nativeToPosixPaths=True)
                if key=='runtimeAcceptance':proof.update(scope='desktop-release')
                if key=='lifecycle' and resident:proof.update(startup=True,closePreservesService=True,windowClose='resident-service',shutdown='unsupported',managedStopAdvertised=False,managedUpdateAdvertised=False)
                if proof_mutation:proof_mutation(key,proof)
                name='PROVENANCE/'+key+'.json';payload[root+'/'+name]=encoded(proof)
                receipt[key]=dict(item(payload[root+'/'+name]),path=name,status='PASS')
            receipt['members']={name:dict(item(data),type='file',mode=0o644)for name,data in payload.items()}
            payload[root + "/DESKTOP.json"] = encoded(receipt)
            payload[root + "/MANIFEST.json"] = encoded({"files": {p: item(b) for p, b in payload.items()}})
        else:
            suffix = ".apk" if target == "android" else ".ipa" if target == "ios" else ".tar.gz"
            path = self.root / f"backplane-{TAG}-{target}{suffix}"
            payload = {}
            receipt = {"schema": 1, "kind": "backplane-target-package", "target": target,
                       "sourceCommit": SHA, "sourceTree": TREE, "version": VERSION, "channel": "release"}
            if target == "web":
                payload, wr = web("backplane-web/web"); receipt['web'] = wr
            else:
                bridge = 'assets/bridge.js' if target == 'android' else 'Payload/Backplane.app/bridge.js'
                payload[bridge] = b'offline fixture bridge'
                receipt.update(bridgeMember=bridge, bridgeSha256=item(payload[bridge])["sha256"],
                               configuration="Release", actualNativeVersion=VERSION, buildNumber="12",
                               signing={"mode": "android-release" if target == "android" else "app-store-distribution",
                                        "verified": True, "identity": "OFFLINE FIXTURE ONLY", "debug": False})
            if target == 'android':
                receipt['target']='android-apk'
                receipt['signing']={'certificateSha256':'d0bf89dfe800a49575f7a56bf305197e1399ddef7ce9b87b9b911a253e04d458',
                                    'signatureVerified':True,'debuggable':False}
            if target == 'ios':
                receipt['target']='ios-ipa';receipt['package']='dev.backplane.mobile'
                receipt['signing']={'certificateSha256':'e'*64,'signatureVerified':True,'debuggable':False,'teamId':'OFFLINEFIXTURE',
                    'products':{k:{'signatureVerified':True,'debuggable':False,'certificateSha256':'e'*64,
                                  'profileUuid':'OFFLINEFIXTURE','bundleId':'dev.backplane.mobile'+('.island'if k=='island'else'')}
                                for k in ('app','island')}}
            receipt['members'] = {p: item(b) for p, b in payload.items()}
        archive(path, payload)
        row = {"archive": self.binding(path)}
        if not desktop:
            receipt['archive'] = row['archive']
        rp = self.root / (target + '.producer.json'); rp.write_bytes(encoded(receipt)); row['receipt'] = self.binding(rp)
        if desktop:
            cp = self.root / (path.name + '.sha256'); cp.write_text(item(path.read_bytes())['sha256'] + '  ' + path.name + '\n')
            row['checksum'] = self.binding(cp)
        if target == 'android':
            row['expectedSignerSha256']='d0bf89dfe800a49575f7a56bf305197e1399ddef7ce9b87b9b911a253e04d458'
            ap=self.root/f'backplane-{TAG}-android.aab'
            data={'base/assets/bridge.js':b'offline fixture bridge'};archive(ap,data)
            ar=dict(receipt,target='android-aab',bridgeMember='base/assets/bridge.js',archive=dict(self.binding(ap)))
            ar['members']=[dict(item(b),path=p)for p,b in data.items()]
            arp=self.root/'android-aab.producer.json';arp.write_bytes(encoded(ar))
            row['variants']=[{'archive':self.binding(ap),'receipt':self.binding(arp),'expectedSignerSha256':row['expectedSignerSha256'],'publishedBuildFloor':1}]
        if target=='ios':row.update(expectedSignerSha256='e'*64,expectedPackageId='dev.backplane.mobile',expectedTeamId='OFFLINEFIXTURE')
        if target in ('android','ios'):row['publishedBuildFloor']=1
        self.inputs['targets'][target] = row

    def check(self):
        assets.verify_inputs(self.root, self.inputs, SHA, TAG)

    def collect(self):
        self.save(); assets.collect(self.root, SHA, TAG, self.manifest)

    def mixed_sources(self):
        source, tree = assets.CORRECTED_SERVICE_SOURCE
        self.inputs.update(sourceCommit=source, sourceTree=tree)
        with patch.dict(globals(), SHA=source, TREE=tree):
            for target in assets.RELEASE_TARGETS:
                self.target(target)
        old_source, old_tree = assets.RETAINED_COMPONENT_SOURCE
        with patch.dict(globals(), SHA=old_source, TREE=old_tree):
            for target in ('android', 'ios', 'web'):
                self.target(target)
                self.inputs['targets'][target]['componentSource'] = {'commit': old_source, 'tree': old_tree}
        receipt = self.root/'root-component-provenance.json'
        receipt.write_bytes((ROOT/'test/fixtures/release-component-provenance-0.12.0.json').read_bytes())
        self.inputs['componentProvenanceDisposition'] = self.binding(receipt)
        return source, tree

    def test_root_authorized_mixed_sources_preserve_actual_receipts_and_pending_gates(self):
        source, tree = self.mixed_sources()
        before = {p.name: p.read_bytes() for p in self.root.glob('*.producer.json')}
        self.save(); assets.collect(self.root, source, TAG, self.manifest)
        assets.verify(self.root, source, TAG)
        result = json.loads((self.root/'release-assets.json').read_bytes())
        self.assertEqual((result['sourceCommit'], result['sourceTree']), (source, tree))
        for target in ('android', 'ios', 'web'):
            self.assertEqual(result['componentSources'][target], self.inputs['targets'][target]['componentSource'])
        for target in ('linux-x64', 'linux-arm64', 'darwin-arm64', 'windows-x64'):
            self.assertEqual(result['componentSources'][target], {'commit': source, 'tree': tree})
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.root.glob('*.producer.json')})
        coverage = json.loads((self.root/'release-coverage.json').read_bytes())
        self.assertTrue(all(x['state'] == 'pending' for x in coverage['targets'].values()))
        with self.assertRaises(ValueError): assets.verify_coverage(self.root, coverage, source, TAG)

    def test_mixed_source_override_requires_exact_root_receipt_and_tuple(self):
        source, tree = self.mixed_sources()
        original = self.inputs.pop('componentProvenanceDisposition')
        with self.assertRaisesRegex(ValueError, 'pinned root'): assets.verify_inputs(self.root, self.inputs, source, TAG)
        self.inputs['componentProvenanceDisposition'] = original
        receipt = self.root/original['path']; receipt.write_bytes(receipt.read_bytes()+b' ')
        self.inputs['componentProvenanceDisposition'] = self.binding(receipt)
        with self.assertRaisesRegex(ValueError, 'unapproved component'): assets.verify_inputs(self.root, self.inputs, source, TAG)

    def test_disposition_does_not_allow_desktop_or_arbitrary_historical_source(self):
        source, tree = self.mixed_sources()
        row = self.inputs['targets']['linux-x64']
        row['componentSource'] = dict(zip(('commit', 'tree'), assets.RETAINED_COMPONENT_SOURCE))
        with self.assertRaisesRegex(ValueError, 'not authorized'): assets.verify_inputs(self.root, self.inputs, source, TAG)
        del row['componentSource']
        self.inputs['targets']['android']['componentSource']['commit'] = 'c'*40
        with self.assertRaisesRegex(ValueError, 'not authorized'): assets.verify_inputs(self.root, self.inputs, source, TAG)

    def test_retained_archive_is_not_relabelled_without_explicit_scope(self):
        source, tree = self.mixed_sources()
        del self.inputs['targets']['ios']['componentSource']
        with self.assertRaisesRegex(ValueError, 'source/version/channel'): assets.verify_inputs(self.root, self.inputs, source, TAG)

    def test_android_variants_cannot_override_the_approved_component_source(self):
        source, tree = self.mixed_sources()
        self.inputs['targets']['android']['variants'][0]['componentSource'] = {'commit': source, 'tree': tree}
        with self.assertRaisesRegex(ValueError, 'variants must share'): assets.verify_inputs(self.root, self.inputs, source, TAG)

    def test_collected_component_provenance_cannot_be_changed(self):
        source, tree = self.mixed_sources()
        self.save(); assets.collect(self.root, source, TAG, self.manifest)
        manifest = self.root/'release-assets.json'; output = json.loads(manifest.read_bytes())
        output['componentSources']['android'] = {'commit': source, 'tree': tree}
        manifest.write_bytes(encoded(output))
        with self.assertRaisesRegex(ValueError, 'component source scopes changed'): assets.verify(self.root, source, TAG)

    def test_signed_android_aab_is_a_public_asset_alongside_apk(self):
        self.collect()
        result = json.loads((self.root/'release-assets.json').read_bytes())
        row = self.inputs['targets']['android']
        self.assertIn(row['archive']['path'], result['publicAssets'])
        self.assertIn(row['variants'][0]['archive']['path'], result['publicAssets'])
        self.assertEqual(len(result['publicAssets']), 8)

    def test_public_asset_inventory_cannot_omit_signed_aab(self):
        self.collect()
        manifest = self.root/'release-assets.json'; result = json.loads(manifest.read_bytes())
        result['publicAssets'].remove(self.inputs['targets']['android']['variants'][0]['archive']['path'])
        manifest.write_bytes(encoded(result))
        with self.assertRaisesRegex(ValueError, 'primary and variant'): assets.verify(self.root, SHA, TAG)

    def test_seven_package_fixture_collect_verify_all_acceptance_stays_pending(self):
        self.collect(); assets.verify(self.root, SHA, TAG)
        coverage = json.loads((self.root/'release-coverage.json').read_text())
        self.assertTrue(all(x['state']=='pending' for x in coverage['targets'].values()))
        with self.assertRaises(ValueError):
            assets.verify_coverage(self.root, coverage, SHA, TAG)
        with self.assertRaises(ValueError):
            self.collect()

    def test_every_target_is_required(self):
        for target in assets.RELEASE_TARGETS:
            with self.subTest(target=target):
                row = self.inputs['targets'].pop(target)
                with self.assertRaises(ValueError): self.check()
                self.inputs['targets'][target] = row

    def test_unfrozen_null_commit_and_old_target_rejected(self):
        for field, bad in [('rootFrozen',False),('sourceCommit',None),('sourceTree',None),('version','0.11.2')]:
            old=self.inputs[field];self.inputs[field]=bad
            with self.assertRaises(ValueError):self.check()
            self.inputs[field]=old

    def test_no_native_gui_or_dev_bake_relabel(self):
        for field,bad in [('entrypoint','src/app/main.bend'),('serviceVersion','0.11.2-dev.thin-x')]:
            self.target('linux-x64', lambda p,b,s:b.update({field:bad}))
            with self.assertRaises(ValueError):self.check()
        self.target('linux-x64',lambda p,b,s:b['compileInputs']['telemetry'].update(requested=False))
        with self.assertRaises(ValueError):self.check()

    def test_shell_old_version_cannot_be_relabelled(self):
        def mutate(p,b,s):
            key=next(k for k in p if k.endswith('/app/package.json'));p[key]=encoded({'version':'0.1.0','main':'main.cjs'})
        self.target('linux-x64',mutate)
        with self.assertRaises(ValueError):self.check()

    def test_font_names_signatures_and_css_refused(self):
        for name,data in [('font.woff2',b'fixture'),('disguised.bin',b'OTTO' + b'font'),('theme.css',b'font-family: Berkeley Mono')]:
            self.target('linux-x64',lambda p,b,s:p.update({s+'/'+name:data}))
            with self.assertRaises(ValueError):self.check()

    def test_missing_notices_and_gzip_mismatch_refused(self):
        def missing(p,b,s):p.pop(next(k for k in p if k.endswith('/LICENSES.chromium.html')))
        self.target('linux-x64',missing)
        with self.assertRaises(ValueError):self.check()
        self.target('linux-x64',lambda p,b,s:p.update({s+'/web/index.html.gz':gzip.compress(b'wrong')}))
        with self.assertRaises(ValueError):self.check()

    def test_windows_service_is_the_js_one(self):
        for mutate in (lambda p,b,s:b.update(serviceRuntime='native-c'), lambda p,b,s:b.pop('serviceRuntime'),
                       lambda p,b,s:b.update(service='backplane-serve')):
            self.target('windows-x64',mutate)
            with self.assertRaisesRegex(ValueError,'JS service'):self.check()
        self.target('linux-x64',lambda p,b,s:b.update(serviceRuntime='native-c'))
        with self.assertRaisesRegex(ValueError,'JS service'):self.check()
        self.target('linux-x64',lambda p,b,s:b.update(serviceRuntime='bun',service='backplane-serve'))
        self.target('windows-x64')
        self.check()

    def test_windows_without_managed_control_ships_only_resident(self):
        self.target('windows-x64',resident=False)
        with self.assertRaisesRegex(ValueError,'managed service control'):self.check()
        self.target('windows-x64',lambda p,b,s:b.update(managedControl={'status':'IMPLEMENTED','protocol':1}))
        with self.assertRaisesRegex(ValueError,'unsupported control'):self.check()

    def test_mobile_old_version_or_debug_signing_refused(self):
        for target in ('android','ios'):
            row=self.inputs['targets'][target];p=self.root/row['receipt']['path'];r=json.loads(p.read_text())
            for field,value in [('actualNativeVersion','0.1.0'),('configuration','Debug'),('buildNumber','0')]:
                old=r[field];r[field]=value;p.write_bytes(encoded(r));row['receipt']=self.binding(p)
                with self.assertRaises(ValueError):self.check()
                r[field]=old
            r['signing']['debuggable']=True;p.write_bytes(encoded(r));row['receipt']=self.binding(p)
            with self.assertRaises(ValueError):self.check()
            self.target(target)

    def test_changed_files_extra_files_and_checksum_omit_refused(self):
        self.collect();p=self.root/self.inputs['targets']['web']['archive']['path'];p.write_bytes(b'changed')
        with self.assertRaises(ValueError):assets.verify(self.root,SHA,TAG)

    def test_duplicate_json_and_tar_path_refused(self):
        with self.assertRaises(ValueError):assets.load_json('{"x":1,"x":2}')
        for name in ('../outside','/absolute','C:drive','a/../x'):
            p=self.root/'bad.tar.gz';archive(p,{name:b'x'})
            with self.assertRaises(ValueError):assets.Package(p)

    def test_symlink_escape_and_cycle_refused(self):
        for links in ({'root/link':'../../outside'}, {'root/a':'b','root/b':'a'}):
            p=self.root/'links.tar.gz'
            with tarfile.open(p,'w:gz')as out:
                for name,target in links.items():
                    m=tarfile.TarInfo(name);m.type=tarfile.SYMTYPE;m.linkname=target;out.addfile(m)
            with self.assertRaises(ValueError):assets.Package(p)

    def test_secret_compile_and_signing_files_refused(self):
        self.target('linux-x64',lambda p,b,s:p.update({s+'/bp_google.h':b'fixture private header'}))
        with self.assertRaises(ValueError):self.check()

    def test_local_linux_fixture_cannot_be_promoted_to_desktop_acceptance(self):
        self.target('linux-x64',proof_mutation=lambda k,r:r.update(scope='local-linux-synthetic')if k=='runtimeAcceptance'else None)
        with self.assertRaises(ValueError):self.check()

    def test_approved_historical_android_certificate_continuity_and_wrong_signer(self):
        self.check()
        self.inputs['targets']['android']['expectedSignerSha256']='f'*64
        with self.assertRaises(ValueError):self.check()

    def test_explicit_resident_unsigned_windows_does_not_require_optional_stop(self):
        self.target('windows-x64',resident=True)
        self.check()
        self.target('windows-x64',
                    proof_mutation=lambda k,r:r.update(managedStopAdvertised=True)if k=='lifecycle'else None,resident=True)
        with self.assertRaises(ValueError):self.check()

    def test_ios_actual_root_bundle_team_must_not_fall_back_to_debug_example(self):
        row=self.inputs['targets']['ios']
        for key,value in [('expectedPackageId','dev.backplane.works'),('expectedTeamId','ANOTHERTEAM')]:
            old=row[key];row[key]=value
            with self.assertRaises(ValueError):self.check()
            row[key]=old

    def test_android_aab_cannot_be_missing_or_substituted(self):
        old=self.inputs['targets']['android'].pop('variants')
        with self.assertRaises(ValueError):self.check()
        self.inputs['targets']['android']['variants']=[self.inputs['targets']['android']]
        with self.assertRaises(ValueError):self.check()
        self.inputs['targets']['android']['variants']=old

    def test_final_requires_accepted_rows_and_exact_sealed_archives(self):
        self.collect()
        with self.assertRaises(ValueError):
            assets.verify_final(self.root,self.root,self.root/'release-coverage.json',SHA,TAG)
        manifest=json.loads((self.root/'release-assets.json').read_text())
        self.assertEqual(manifest['sourceTree'],TREE)
        sums=self.root/'SHA256SUMS';sums.write_text('\n'.join(sums.read_text().splitlines()[1:])+'\n')
        with self.assertRaises(ValueError):assets.verify(self.root,SHA,TAG)

    def test_final_acceptance_binds_every_sealed_primary_and_variant(self):
        self.collect()
        with tempfile.TemporaryDirectory()as folder:
            q=Path(folder);coverage=assets.coverage_plan(SHA,TAG)
            for target,row in self.inputs['targets'].items():
                refs=[]
                for r in [row,*row.get('variants',[])]:
                    name=r['archive']['path'];data=(self.root/name).read_bytes();(q/name).write_bytes(data)
                    coverage['files'][name]=dict(item(data),kind='asset',targets=[target]);refs.append(name)
                name=target+'.owner.json';data=encoded({'scope':'OFFLINE DECLARED-COVERAGE FIXTURE ONLY'});(q/name).write_bytes(data)
                coverage['files'][name]=dict(item(data),kind='owner-receipt',targets=[target])
                coverage['targets'][target].update(state='accepted',artifacts=refs,
                    gates={k:name for k in coverage['targets'][target]['gates']})
            manifest=q/'coverage.json';manifest.write_bytes(encoded(coverage))
            assets.verify_final(self.root,q,manifest,SHA,TAG)
            name=self.inputs['targets']['web']['archive']['path'];(q/name).write_bytes(b'other actual bytes')
            coverage['files'][name].update(item((q/name).read_bytes()));manifest.write_bytes(encoded(coverage))
            with self.assertRaises(ValueError):assets.verify_final(self.root,q,manifest,SHA,TAG)

    def test_bake_presence_does_not_log_values(self):
        e={'BACKPLANE_GOOGLE_BAKE':'1','BACKPLANE_GOOGLE_CLIENT_ID':'fixture-id','BACKPLANE_GOOGLE_CLIENT_SECRET':'fixture-secret',
           'BACKPLANE_TELEMETRY_BAKE':'1','BACKPLANE_TELEMETRY_URL':'https://backplane.works/api/v1/events',
           'BACKPLANE_TELEMETRY_KID':'rel-1','BACKPLANE_TELEMETRY_KEY':'a'*64}
        assets.check_bake(e,'release')
        for name in ('BACKPLANE_GOOGLE_CLIENT_SECRET','BACKPLANE_TELEMETRY_KEY','BACKPLANE_TELEMETRY_URL'):
            bad=dict(e);bad[name]=''
            with self.assertRaises(ValueError)as r:assets.check_bake(bad,'release')
            self.assertNotIn('fixture-secret',str(r.exception));self.assertNotIn('a'*64,str(r.exception))


class CoverageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = assets.coverage_plan(SHA, TAG)
        for name, row in self.manifest["targets"].items():
            artifact = name + ".fixture-payload"
            receipt = name + ".fixture-receipt.json"
            (self.root / artifact).write_bytes(b"offline fixture, not a product artifact")
            (self.root / receipt).write_text('{"scope":"offline fixture only"}\n')
            row["state"] = "accepted"
            row["artifacts"] = [artifact]
            row["gates"] = {gate: receipt for gate in row["gates"]}
            for path, kind in [(artifact, "asset"), (receipt, "owner-receipt")]:
                p = self.root / path
                self.manifest["files"][path] = {"kind": kind, "bytes": p.stat().st_size,
                                                "sha256": assets.digest(p), "targets": [name]}

    def verify(self):
        assets.verify_coverage(self.root, self.manifest, SHA, TAG)

    def test_plan_is_unfrozen_and_every_surface_pending(self):
        plan = assets.coverage_plan(None, TAG)
        self.assertIsNone(plan["sourceCommit"])
        self.assertEqual(plan["files"], {})
        self.assertEqual(set(plan["targets"]), {"linux-x64", "linux-arm64", "darwin-arm64",
                                               "windows-x64", "web", "android", "ios"})
        self.assertTrue(all(row["state"] == "pending" for row in plan["targets"].values()))
        with self.assertRaises(ValueError):
            assets.verify_coverage(self.root, plan, SHA, TAG)

    def test_complete_fixture_bindings_pass_without_product_claim(self):
        self.verify()
        self.assertIn("root reviews actual gate scope", self.manifest["scope"])

    def test_telemetry_deferral_is_limited_to_explicit_user_release(self):
        plan = assets.coverage_plan(SHA, "v0.12.0")
        self.assertIs(plan["deferredGates"]["telemetry-privacy"]["accepted"], False)
        self.assertTrue(all("telemetry-privacy" not in row["gates"] for row in plan["targets"].values()))
        future = assets.coverage_plan(SHA, "v0.12.1")
        self.assertEqual(future["deferredGates"], {})
        self.assertTrue(all("telemetry-privacy" in row["gates"] for row in future["targets"].values()))

    def test_deferred_telemetry_cannot_be_relabelled_as_accepted(self):
        self.manifest["deferredGates"]["telemetry-privacy"]["accepted"] = True
        with self.assertRaisesRegex(ValueError, "deferral"):
            self.verify()

    def test_linux_only_cannot_complete_release(self):
        for name, row in self.manifest["targets"].items():
            if name != "linux-x64":
                row["state"] = "pending"
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_mobile_web_and_other_desktop_each_reject(self):
        for name in ("ios", "android", "web", "windows-x64", "darwin-arm64", "linux-arm64"):
            with self.subTest(target=name):
                row = self.manifest["targets"].pop(name)
                with self.assertRaises(ValueError):
                    self.verify()
                self.manifest["targets"][name] = row

    def test_source_version_channel_mismatch_rejects(self):
        for field, wrong in [("sourceCommit", "b" * 40), ("version", "0.11.1"),
                             ("channel", "nightly"), ("tag", "v0.11.1")]:
            with self.subTest(field=field):
                original = self.manifest[field]
                self.manifest[field] = wrong
                with self.assertRaises(ValueError):
                    self.verify()
                self.manifest[field] = original

    def test_sandbox_and_privacy_receipts_required(self):
        for target, gate in [("linux-x64", "os-sandbox"), ("web", "auth-security"),
                             ("ios", "product-parity")]:
            with self.subTest(target=target, gate=gate):
                original = self.manifest["targets"][target]["gates"].pop(gate)
                with self.assertRaises(ValueError):
                    self.verify()
                self.manifest["targets"][target]["gates"][gate] = original

    def test_booleans_or_assets_cannot_substitute_receipt(self):
        gates = self.manifest["targets"]["linux-x64"]["gates"]
        for wrong in (True, None, "linux-x64.fixture-payload"):
            gates["runtime"] = wrong
            with self.assertRaises(ValueError):
                self.verify()

    def test_linux_bindings_cannot_be_reused_for_windows_or_mobile(self):
        for target in ("windows-x64", "ios", "web"):
            row = self.manifest["targets"][target]
            original = row["artifacts"]
            row["artifacts"] = ["linux-x64.fixture-payload"]
            with self.assertRaises(ValueError):
                self.verify()
            row["artifacts"] = original
            receipt = row["gates"]["runtime"]
            row["gates"]["runtime"] = "linux-x64.fixture-receipt.json"
            with self.assertRaises(ValueError):
                self.verify()
            row["gates"]["runtime"] = receipt

    def test_missing_or_unknown_target_scope_rejects(self):
        item = self.manifest["files"]["ios.fixture-receipt.json"]
        for scope in (None, [], ["linux-x64", "unknown"], ["ios", "ios"]):
            item["targets"] = scope
            with self.assertRaises(ValueError):
                self.verify()

    def test_changed_artifact_and_receipt_reject(self):
        for name in ("linux-x64.fixture-payload", "ios.fixture-receipt.json"):
            p = self.root / name
            original = p.read_bytes()
            p.write_bytes(b"changed")
            with self.assertRaises(ValueError):
                self.verify()
            p.write_bytes(original)

    def test_symlink_parent_and_traversal_reject(self):
        p = self.root / "linux-x64.fixture-payload"
        p.unlink()
        p.symlink_to(self.root / "linux-arm64.fixture-payload")
        with self.assertRaises(ValueError):
            self.verify()
        p.unlink()
        p.write_bytes(b"offline fixture, not a product artifact")
        item = self.manifest["files"][p.name]
        for name in ("../outside", "/absolute", "x/../outside", "x\\outside", "C:outside"):
            with self.subTest(path=name), self.assertRaises(ValueError):
                assets.bound_coverage_file(self.root, name, item)
        (self.root / "link").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            assets.bound_coverage_file(self.root, "link/" + p.name, item)

    def test_missing_or_extra_unreferenced_files_reject(self):
        original = self.manifest["files"].pop("web.fixture-payload")
        with self.assertRaises(ValueError):
            self.verify()
        self.manifest["files"]["web.fixture-payload"] = original
        (self.root / "extra").write_bytes(b"fixture")
        self.manifest["files"]["extra"] = {"kind": "asset", "bytes": 7,
                                              "sha256": assets.digest(self.root / "extra"), "targets": ["web"]}
        with self.assertRaises(ValueError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
