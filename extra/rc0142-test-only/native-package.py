"""Matching-host TEST ABI assembly/stage/archive; no emitter or release claim."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import urllib.request


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fetch(url, expected, output):
    assert url.startswith('https://') and len(expected) == 64
    with urllib.request.urlopen(url, timeout=240) as response, output.open('xb') as stream:
        assert response.geturl().startswith('https://')
        count = 0
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            assert count <= 2 * 1024**3
            stream.write(chunk)
    assert sha(output) == expected, 'Public input packet hash differs'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('source', 'tools', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    for name in ('target', 'components-url', 'components-sha256'):
        p.add_argument('--' + name, required=True)
    p.add_argument('--windows-url', default='')
    p.add_argument('--windows-sha256', default='')
    a = p.parse_args()
    source, tools, out = a.source.resolve(), a.tools.resolve(), a.out.resolve()
    assert not out.exists(), 'Inspect prior native attempt; no overwrite/retry'
    out.mkdir()
    record = {'status': 'INCOMPLETE', 'testOnly': True, 'releaseQualified': False,
        'sourceCommit': 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4',
        'sourceTree': 'ec6c85b4a8b62a0227f09151a9ec52a362b20ae2',
        'target': a.target, 'version': '0.14.2', 'compilerMode': 'composite-settle-41262875',
        'producerWorkId': 'w-t1792701529-415584-1614985', 'published': False}
    def run(argv, name):
        record['phase'] = name
        with (out / (name + '.log')).open('w', encoding='utf-8') as log:
            subprocess.run(list(map(str, argv)), cwd=source, stdout=log, stderr=subprocess.STDOUT,
                           check=True, timeout=900)
    try:
        packet = out / 'components.tar.gz'
        fetch(a.components_url, a.components_sha256, packet)
        with tarfile.open(packet) as tar:
            tar.extractall(out / 'inputs', filter='data')
        args = []
        if a.target == 'windows-x64':
            assert a.windows_sha256 == '42ef24bf69566f3d2d56aebf4b689276831e6e70d629d53d9cb9fc313a357412'
            win = out / 'windows-runtime.tar.gz'
            fetch(a.windows_url, a.windows_sha256, win)
            with tarfile.open(win) as tar:
                tar.extractall(out / 'windows-runtime', filter='data')
            args = ['--windows-runtime', out / 'windows-runtime']
        build = source / 'build/test-service'
        service = build / ('backplane-serve.exe' if a.target == 'windows-x64' else 'backplane-serve')
        service.chmod(0o755)
        producer = tools / 'scripts/service-release.py'
        run([sys.executable, tools / 'scripts/service-runtime.py', *args,
            '--source-root', source, '--build', build, '--producer', producer,
            '--producer-sha256', sha(producer), '--out', out / 'service-component'], 'abi-and-component')
        component = out / 'service-component/component/SERVICE.json'
        assert json.loads(component.read_text())['releaseQualified'] is False
        run([sys.executable, source / 'desktop/freeze-target.py', source,
            out / 'inputs/components', component, out / 'freeze'], 'freeze')
        run(['node', tools / 'desktop/prepare-native-stage.cjs', 'stage',
            '--components', out / 'inputs/components/COMPONENT-INPUTS.json', '--service', component,
            '--freeze', out / 'freeze/FREEZE.json', '--out', out / 'stage'], 'stage')
        if a.target == 'darwin-arm64':
            app = out / 'stage/Backplane.app'
            run([sys.executable, tools / 'desktop/macos-bundle.py', 'seal', app], 'adhoc-seal')
            run(['codesign', '--verify', '--deep', '--strict', app], 'codesign-verify')
            record['signing'] = {'type': 'ad-hoc', 'developerId': False, 'notarized': False}
        else:
            record['signing'] = 'unsigned-portable'
        run(['node', Path(__file__).with_name('archive-test-stage.cjs'), tools,
             out / 'stage', out / 'dist'], 'test-archive')
        packaged = json.loads((out / 'dist/TEST-PACKAGE.json').read_text())
        record.update(status='TEST_ONLY_PACKAGE_READY_RUNTIME_UNVERIFIED',
                      archive=packaged['archive'], buildProvenance=packaged['buildProvenance'])
    except Exception as error:
        record.update(status='FAIL', error=str(error))
        raise
    finally:
        (out / 'RESULT.json').write_text(json.dumps(record, indent=2) + '\n')
        print(json.dumps(record), flush=True)


if __name__ == '__main__':
    main()
