#!/usr/bin/env python3
"""Package validated 0.14.1 inputs under the explicit Windows-only release gate.

No compiler, payload launch, campaign or fresh runtime acceptance. The original
stage producer verifies all component inventories. Mac sealing remains genuine.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request

PRODUCT = 'e564f904eb277ffbd8d0104d6d8afa90ff1aeb6f'
TREE = '4b09684c1debc0ced67d4068679489f6ab11dd5d'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')

def main():
    product, out = Path('product').resolve(), Path('build/unix-existing').resolve()
    target = os.environ['TARGET']
    assert target in {'linux-x64', 'linux-arm64', 'darwin-arm64'}
    assert not out.exists()
    out.mkdir(parents=True)
    def run(*command):
        subprocess.run([str(x) for x in command], check=True)
    def git(query):
        return subprocess.check_output(['git', '-C', str(product), 'rev-parse', query], text=True).strip()
    assert (git('HEAD'), git('HEAD^{tree}')) == (PRODUCT, TREE)
    pin = os.environ['PACKET_SHA256']
    url = os.environ['PACKET_URL']
    assert re.fullmatch('[a-f0-9]{64}', pin) and url.startswith('https://')
    packet = out / 'inputs.tar.gz'
    with urllib.request.urlopen(url, timeout=240) as response, packet.open('wb') as stream:
        assert response.geturl().startswith('https://')
        shutil.copyfileobj(response, stream)
    assert sha(packet) == pin
    inputs = out / 'inputs'
    with tarfile.open(packet) as archive:
        archive.extractall(inputs, filter='data')
    freeze = json.loads((inputs / 'freeze/FREEZE.json').read_text())
    assert (freeze['commit'], freeze['tree'], freeze['version'], freeze['channel']) == (PRODUCT, TREE, '0.14.1', 'release')
    assert freeze['components'][target]
    for receipt, payload in [('component/SERVICE.json', 'component/payload'),
                             ('components/SHELL.json', 'components/shell-payload'),
                             ('components/ELECTRON.json', 'components/payload')]:
        run('node', product / 'desktop/restore-modes.cjs', inputs / receipt, inputs / payload)
    stage = out / 'stage'
    run('node', product / 'desktop/prepare-native-stage.cjs', 'stage',
        '--components', inputs / 'components/COMPONENT-INPUTS.json',
        '--service', inputs / 'component/SERVICE.json', '--freeze', inputs / 'freeze/FREEZE.json', '--out', stage)
    signing = {'signed': False, 'type': 'unsigned-portable', 'developerId': False, 'notarized': False}
    if target == 'darwin-arm64':
        app, = stage.glob('*.app')
        seal = json.loads(subprocess.check_output([sys.executable, str(product / 'desktop/macos-bundle.py'), 'seal', str(app)], text=True))
        run('codesign', '--verify', '--deep', '--strict', app)
        write(out / 'MACOS-INTEGRITY.json', seal)
        signing.update(signed=True, type='ad-hoc', integrity=seal)
    # Preserve the original producer's signing-change bounds without claiming a UI run.
    run('node', '-e', "require(process.argv[1]).validateSigningChanges(process.argv[2],process.argv[3]==='true')",
        product / 'desktop/produce-release.cjs', stage, str(signing['signed']).lower())
    s = json.loads((stage / 'STAGE.json').read_text())
    dist = out / 'dist'
    final = dist / ('backplane-' + target)
    shutil.copytree(stage, final, symlinks=True)
    (final / 'STAGE.json').rename(final / 'PROVENANCE/STAGE-INPUT.json')
    disposition = {'status': 'FRESH_NONWINDOWS_RUNTIME_NOT_RUN_BY_DIRECT_USER_INSTRUCTION',
        'releaseGate': 'Actual packaged Windows standard-user DeveloperModeOFF cold startup/connection/mainUI/failureUI only',
        'historicalEvidence': '0.14.0 runtime/component evidence remains original; no new PASS or unchanged-shell equivalence is asserted',
        'currentRuntimeAccepted': False, 'newEmission': False, 'compilerInvoked': False}
    write(final / 'PROVENANCE/RUNTIME-DISPOSITION.json', disposition)
    receipt = {'schema': 1, 'kind': 'backplane-electron-desktop-package', 'target': target,
        'release': s['release'], 'entrypoint': {'launcher': s['entrypoint'], 'backend': 'src/server/main.bend'},
        'components': s['components'], 'electronVersion': s['electronVersion'], 'serviceVersion': '0.14.1',
        'shellVersion': '0.14.1', 'webManifestSha256': s['webManifestSha256'], 'signing': signing,
        'runtimeAcceptance': disposition, 'releaseQualified': False, 'published': False}
    def inventory(directory):
        return json.loads(subprocess.check_output(['node', '-e',
            'console.log(JSON.stringify(require(process.argv[1]).inventory(process.argv[2],process.argv[3])))',
            str(product / 'desktop/produce-release.cjs'), str(directory), target], text=True))
    receipt['members'] = inventory(final)
    write(final / 'DESKTOP.json', receipt)
    write(final / 'MANIFEST.json', {'schema': 1, 'kind': 'backplane-electron-package-members',
                                   'target': target, 'files': inventory(final)})
    name = 'backplane-v0.14.1-' + target + '.tar.gz'
    run(sys.executable, product / 'desktop/release-files.py', 'archive', final, dist / name,
        'backplane-' + target + '/', str(s['sourceDateEpoch']))
    artifact = {'path': name, 'sha256': sha(dist / name), 'bytes': (dist / name).stat().st_size}
    (dist / (name + '.sha256')).write_text(artifact['sha256'] + '  ' + name + '\n')
    result = {'status': '0141_PACKAGE_ONLY_UNDER_EXPLICIT_WINDOWS_ONLY_GATE', 'target': target,
        'sourceCommit': PRODUCT, 'sourceTree': TREE, 'version': '0.14.1', 'channel': 'release',
        'artifact': artifact, 'signing': signing, 'runtimeAcceptance': disposition, 'published': False}
    write(out / 'RESULT.json', result)
    print(json.dumps(result), flush=True)

if __name__ == '__main__':
    main()
