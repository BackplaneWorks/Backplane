"""Archive exact configured Linux web bytes; no transform, emission or recompression."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser(description=__doc__)
for n in ('source', 'build', 'tools', 'out'):
    p.add_argument('--'+n, type=Path, required=True)
a = p.parse_args()
s, b, tools, out = [x.resolve() for x in (a.source, a.build, a.tools, a.out)]
commit = 'be904cd4cb21ad6febe17e674918c96945113d07'
tree = '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59'
assert not out.exists()
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=s, text=True).strip() == commit
assert subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], cwd=s, text=True).strip() == tree
def read(f): return json.loads(f.read_text())
def ref(f):
    with f.open('rb') as stream:
        return {'path': str(f), 'sha256': hashlib.file_digest(stream, 'sha256').hexdigest(), 'bytes': f.stat().st_size}
def write(f, d): f.write_text(json.dumps(d, indent=2)+'\n')
build, auth = read(b/'BUILD.json'), read(b/'BUILD-AUTHORIZATION.json')
assert build['source_commit'] == commit and build['source_tree'] == tree
assert build['testOnly'] is True and build['releaseQualified'] is False
assert auth['releaseUseAuthorized'] is True
assert auth['sourceCommit'] == commit and auth['sourceTree'] == tree
assert ref(b/'BUILD.json')['sha256'] == auth['buildReceipt']['sha256']
web, emission = read(b/'WEB.json'), read(b/'WEB-EMISSION.json')
assert web['sourceCommit'] == commit and web['sourceTree'] == tree
assert emission['webManifestSha256'] == ref(b/'WEB.json')['sha256']
subprocess.run([sys.executable, Path(__file__).with_name('prepare-tooling.py'), '--source', s, '--output', tools], check=True)
out.mkdir(parents=True)
release = {'version': '0.14.2', 'tag': 'v0.14.2', 'channel': 'release', 'commit': commit, 'tree': tree}
epoch = int(subprocess.check_output(['git', 'show', '-s', '--format=%ct', commit], cwd=s, text=True))
write(out/'FREEZE.json', {'schema': 1, 'kind': 'backplane-release-freeze', 'status': 'FROZEN', **release,
    'sourceDateEpoch': epoch, 'components': {'web': {'webManifestSha256': ref(b/'WEB.json')['sha256'],
    'emissionReceiptSha256': ref(b/'WEB-EMISSION.json')['sha256']}}})
write(out/'REQUEST.json', {'schema': 1, 'kind': 'backplane-web-release-inputs', 'target': 'web',
    'release': {**release, 'freezeReceipt': ref(out/'FREEZE.json')},
    'web': {'receipt': ref(b/'WEB.json'), 'payload': {'directory': str(b/'web')}},
    'emissionReceipt': ref(b/'WEB-EMISSION.json')})
subprocess.run(['node', tools/'desktop/produce-web-release.cjs', 'archive', '--request', out/'REQUEST.json', '--output', out/'dist'], check=True)
write(out/'WEB-PACKAGE-AUTHORIZATION.json', {'schema': 1, 'status': 'CURRENT_AUTHORIZED_WEB_PACKAGE_READY',
    'sourceCommit': commit, 'sourceTree': tree, 'version': '0.14.2', 'channel': 'release',
    'packageReceipt': ref(out/'dist/WEB-PACKAGE.json'), 'buildAuthorization': ref(b/'BUILD-AUTHORIZATION.json'),
    'originalBuildReceipt': ref(b/'BUILD.json'), 'releaseUseAuthorized': True,
    'historicalFlagsPreserved': {'testOnly': True, 'releaseQualified': False, 'stockEquivalent': False},
    'byteQualification': 'NO_CURRENT_SOURCE_STOCK_COMPARISON_CARRIED_OVER_OPTION2_AFA96792',
    'runtimeAcceptance': 'UNRUN', 'emissionOrTransformOrAssetRecompression': False,
    'inputRunId': 41950676625, 'inputArtifactId': 13380517899})
print(json.dumps({'archive': ref(out/'dist/backplane-v0.14.2-web.tar.gz'),
    'receipt': ref(out/'dist/WEB-PACKAGE.json'), 'authorization': ref(out/'WEB-PACKAGE-AUTHORIZATION.json')}), flush=True)
