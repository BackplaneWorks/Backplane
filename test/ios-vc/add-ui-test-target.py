#!/usr/bin/env python3
"""Add an XCTest UI target to a *disposable Mac copy* of the iOS project."""
import hashlib, pathlib, plistlib, subprocess, sys

if len(sys.argv) != 3:
    sys.exit('usage: add-ui-test-target.py DISPOSABLE_IOS_COPY QualificationUITests.swift')
root = pathlib.Path(sys.argv[1]).resolve()
test = pathlib.Path(sys.argv[2]).resolve()
dedicated = root.name.startswith('backplane-ios-vc-655983')
nested = root.name == 'ios' and root.parent.name == 'mobile' and root.parent.parent.name.startswith('backplane-ios-vc-655983')
if not (dedicated or nested):
    sys.exit('refusing to change a non-dedicated source copy')
project = root / 'Backplane.xcodeproj' / 'project.pbxproj'
if not project.is_file() or not test.is_file():
    sys.exit('project or test source missing')
xml = subprocess.check_output(['plutil', '-convert', 'xml1', '-o', '-', str(project)])
p = plistlib.loads(xml); o = p['objects']
ids = {name: f'F655983000000000000000{i:02X}' for i, name in enumerate(('target','product','source','buildfile','sources','frameworks','resources','debug','release','configlist','proxy','dependency'), 1)}
existing = [i in o for i in ids.values()]
if any(existing):
    if not all(existing): sys.exit('incomplete or colliding test target; refusing source replacement')
    source = o[ids['source']]
    target = o[ids['target']]
    if not (source.get('path') == 'QualificationUITests.swift'
            and target.get('productType') == 'com.apple.product-type.bundle.ui-testing'
            and ids['sources'] in target.get('buildPhases', [])
            and ids['buildfile'] in o[ids['sources']].get('files', [])
            and o[ids['buildfile']].get('fileRef') == ids['source']
            and (root / 'Backplane.xcodeproj/xcshareddata/xcschemes/Qualification.xcscheme').is_file()):
        sys.exit('existing test target does not match the dedicated layout')
    (root / 'QualificationUITests.swift').write_bytes(test.read_bytes())
    actual = hashlib.sha256((root / 'QualificationUITests.swift').read_bytes()).hexdigest()
    print('Updated existing QualificationUITests source; compiled-source SHA256 ' + actual)
    sys.exit(0)
(root / 'QualificationUITests.swift').write_bytes(test.read_bytes())
o[ids['source']] = dict(isa='PBXFileReference', lastKnownFileType='sourcecode.swift', path='QualificationUITests.swift', sourceTree='<group>')
o[ids['product']] = dict(isa='PBXFileReference', explicitFileType='wrapper.cfbundle', includeInIndex='0', path='QualificationUITests.xctest', sourceTree='BUILT_PRODUCTS_DIR')
o[ids['buildfile']] = dict(isa='PBXBuildFile', fileRef=ids['source'])
for name, isa in [('sources','PBXSourcesBuildPhase'), ('frameworks','PBXFrameworksBuildPhase'), ('resources','PBXResourcesBuildPhase')]:
    o[ids[name]] = dict(isa=isa, buildActionMask='2147483647', files=[ids['buildfile']] if name == 'sources' else [], runOnlyForDeploymentPostprocessing='0')
settings = dict(PRODUCT_BUNDLE_IDENTIFIER='dev.backplane.mobile.qualification-tests', PRODUCT_NAME='$(TARGET_NAME)', SWIFT_VERSION='5.0', IPHONEOS_DEPLOYMENT_TARGET='17.0', GENERATE_INFOPLIST_FILE='YES', TEST_TARGET_NAME='Backplane', CODE_SIGNING_ALLOWED='NO')
for name in ('debug', 'release'):
    o[ids[name]] = dict(isa='XCBuildConfiguration', buildSettings=settings.copy(), name=name.capitalize())
o[ids['configlist']] = dict(isa='XCConfigurationList', buildConfigurations=[ids['debug'], ids['release']], defaultConfigurationIsVisible='0', defaultConfigurationName='Debug')
app = 'A10000000000000000000006'; proj = p['rootObject']
o[ids['proxy']] = dict(isa='PBXContainerItemProxy', containerPortal=proj, proxyType='1', remoteGlobalIDString=app, remoteInfo='Backplane')
o[ids['dependency']] = dict(isa='PBXTargetDependency', target=app, targetProxy=ids['proxy'])
o[ids['target']] = dict(isa='PBXNativeTarget', buildConfigurationList=ids['configlist'], buildPhases=[ids['sources'],ids['frameworks'],ids['resources']], buildRules=[], dependencies=[ids['dependency']], name='QualificationUITests', productName='QualificationUITests', productReference=ids['product'], productType='com.apple.product-type.bundle.ui-testing')
o[proj]['targets'].append(ids['target']); o[o[proj]['mainGroup']]['children'].append(ids['source']); o[o[proj]['productRefGroup']]['children'].append(ids['product'])
project.write_bytes(plistlib.dumps(p, fmt=plistlib.FMT_XML, sort_keys=False))
scheme = root / 'Backplane.xcodeproj/xcshareddata/xcschemes/Qualification.xcscheme'
scheme.parent.mkdir(parents=True, exist_ok=True)
scheme.write_text(f'''<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="2600" version="1.7"><BuildAction parallelizeBuildables="YES" buildImplicitDependencies="YES"><BuildActionEntries><BuildActionEntry buildForTesting="YES" buildForRunning="YES" buildForProfiling="NO" buildForArchiving="NO" buildForAnalyzing="NO"><BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="{ids['target']}" BuildableName="QualificationUITests.xctest" BlueprintName="QualificationUITests" ReferencedContainer="container:Backplane.xcodeproj"/></BuildActionEntry></BuildActionEntries></BuildAction><TestAction buildConfiguration="Debug" selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier="Xcode.DebuggerFoundation.Launcher.LLDB" shouldUseLaunchSchemeArgsEnv="YES"><Testables><TestableReference skipped="NO"><BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="{ids['target']}" BuildableName="QualificationUITests.xctest" BlueprintName="QualificationUITests" ReferencedContainer="container:Backplane.xcodeproj"/></TestableReference></Testables></TestAction><LaunchAction buildConfiguration="Debug"/><ProfileAction buildConfiguration="Release"/><AnalyzeAction buildConfiguration="Debug"/><ArchiveAction buildConfiguration="Release"/></Scheme>''')
print('Added isolated QualificationUITests target; product project remains untouched.')
print('Compiled-source SHA256 ' + hashlib.sha256((root / 'QualificationUITests.swift').read_bytes()).hexdigest())
