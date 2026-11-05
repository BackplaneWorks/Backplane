"""Write current release authorization beside a new BUILD; never rewrite it."""
import argparse
import json
from pathlib import Path
from authorized_inputs import OLD_SOURCE, digest, read, reference, regular, require

p = argparse.ArgumentParser(description=__doc__)
for name in ('binding', 'build', 'out'):
    p.add_argument('--'+name, type=Path, required=True)
for name in ('binding-sha256', 'build-sha256'):
    p.add_argument('--'+name, required=True)
a = p.parse_args()
binding_path, build_path = map(regular, (a.binding, a.build))
require(digest(binding_path) == a.binding_sha256 and digest(build_path) == a.build_sha256, 'Pinned receipt mismatch')
b, build = read(binding_path), read(build_path)
require(b.get('kind') == 'authorized-source-compiler-release-input-binding' and b.get('releaseUseAuthorized') is True,
        'Verified new source authorization required')
require(build.get('source_commit') == b['sourceCommit'] != OLD_SOURCE
        and build.get('source_tree') == b['sourceTree'], 'Old/foreign BUILD cannot be promoted')
require(build.get('version') == '0.14.2' and build.get('versionStamp') == '0.14.2'
        and build.get('telemetryChannel') == 'release' and build.get('serviceRuntime') == 'bun', 'Wrong configured identity')
config = build['compileInputs']
require(config.get('version') == '0.14.2' and config.get('versionStamp') == '0.14.2'
        and config.get('telemetryChannel') == 'release', 'Wrong compile tuple')
require(all(config.get(k, {}).get('requested') is True and config[k].get('requiredInputsPresent') is True
            for k in ('google', 'telemetry')), 'Protected configured bake witnesses missing')
require(config['telemetry'].get('endpoint') == 'https://backplaneworks.com/api/v1/events', 'Wrong telemetry endpoint')
target = build['target']
require(target in ('linux-x64', 'linux-arm64', 'darwin-arm64', 'windows-x64'), 'Unknown service target')
binary = build_path.parent/('backplane-serve.exe' if target == 'windows-x64' else 'backplane-serve')
require(digest(regular(binary)) == build['binary_sha256'], 'Actual compiled binary differs')
require(build.get('buildProvenance', {}).get('compilerMode') == b['compilerMode']
        and build['buildProvenance'].get('producerWorkId') == b['producerWorkId'], 'Compiler/emitter provenance differs')
value = {'schema': 1, 'kind': 'current-release-build-authorization-disposition',
    'candidate': b['candidate'], 'target': target, 'sourceCommit': b['sourceCommit'], 'sourceTree': b['sourceTree'],
    'version': '0.14.2', 'channel': 'release', 'buildReceipt': reference(build_path),
    'binary': reference(binary), 'inputBinding': reference(binding_path),
    'releaseUseAuthorized': True, 'stockByteEquivalent': False,
    'byteQualification': b['byteQualification'],
    'stockComparison': b['stockComparison'],
    'reviewedByteReference': b['reviewedByteReference'],
    'currentSourceStockReferenceExists': b['currentSourceStockReferenceExists'],
    'fullStockByteEquivalenceClaimAllowed': b['fullStockByteEquivalenceClaimAllowed'],
    'compilerMode': b['compilerMode'], 'producerWorkId': b['producerWorkId'],
    'originalBuildStatus': build.get('status'),
    'originalBuildFlags': {k: build.get(k) for k in ('testOnly', 'releaseQualified', 'stockEquivalent')},
    'originalBuildProvenance': build.get('buildProvenance'),
    'qualificationScope': 'Configured candidate build identity plus current user authorization; platform gates remain separate',
    'platformAcceptance': 'PENDING', 'historicalProducerReceiptsRewritten': False,
    'publication': 'PREAUTHORIZED only after actual combined standard-user KiCad health/manual/install/msvcrt and OMP PASS'}
with a.out.open('x') as stream:
    json.dump(value, stream, indent=2)
    stream.write('\n')
require(digest(build_path) == a.build_sha256, 'Original BUILD changed')
print(json.dumps({'status': 'CURRENT_AUTHORIZATION_SIDECAR_WRITTEN_ORIGINAL_BUILD_UNCHANGED', 'receipt': reference(a.out)}))
