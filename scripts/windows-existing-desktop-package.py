#!/usr/bin/env python3
"""Native Windows assembly/smoke of an existing approved service; no compiler."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import urllib.request


def sha(path):
    with Path(path).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def identities(inputs, commit, tree):
    assert re.fullmatch('[0-9a-f]{40}', commit) and re.fullmatch('[0-9a-f]{40}', tree)
    service = read(inputs / 'component/SERVICE.json')
    components = read(inputs / 'components/COMPONENT-INPUTS.json')
    freeze = read(inputs / 'freeze/FREEZE.json')
    expected = {'commit': commit, 'tree': tree, 'version': '0.14.2',
                'channel': 'release', 'tag': 'v0.14.2'}
    assert components['release'] == expected
    assert all(freeze[k] == v for k, v in expected.items())
    assert (service['sourceCommit'], service['sourceTree'], service['version'],
            service['channel'], service['target'], service['status']) == (
                commit, tree, '0.14.2', 'release', 'windows-x64', 'VERIFIED')
    assert components['target'] == 'windows-x64' and freeze['status'] == 'FROZEN'
    return service, freeze


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--product', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    assert sys.platform == 'win32', 'Windows resource branding requires native Windows'
    product, out = args.product.resolve(), args.out.resolve()
    assert not out.exists(), 'New output required; inspect an earlier attempt before retry'
    out.mkdir(parents=True)
    commit, tree = os.environ['PRODUCT_COMMIT'], os.environ['PRODUCT_TREE']
    result = {'status': 'INCOMPLETE', 'phase': 'inputs', 'commit': commit, 'tree': tree,
              'target': 'windows-x64', 'version': '0.14.2', 'published': False,
              'compilerInvoked': False, 'nativeAbiReplayed': False,
              'signing': 'unsigned-portable; no Backplane Authenticode identity',
              'toolSha256': sha(__file__)}
    env = dict(os.environ)
    env['PYTHON'] = sys.executable
    env['BACKPLANE_PRODUCER_PYTHON'] = sys.executable
    system_root = [k for k in env if k.lower() == 'systemroot']
    assert len(system_root) == 1
    env['SystemRoot'] = env.pop(system_root[0])

    def run(*cmd):
        subprocess.run(list(map(str, cmd)), check=True, env=env)

    try:
        assert re.fullmatch('[0-9a-f]{40}', commit) and re.fullmatch('[0-9a-f]{40}', tree)
        for query, expected in [('HEAD', commit), ('HEAD^{tree}', tree)]:
            assert subprocess.check_output(['git', '-C', str(product), 'rev-parse', query], text=True).strip() == expected
        assert not subprocess.check_output(['git', '-C', str(product), 'status', '--porcelain', '--untracked-files=no'], text=True).strip()
        url, pin = os.environ['PACKET_URL'], os.environ['PACKET_SHA256']
        assert url.startswith('https://') and re.fullmatch('[0-9a-f]{64}', pin)
        packet = out / 'inputs.tar.gz'
        with urllib.request.urlopen(url, timeout=180) as response, packet.open('wb') as f:
            assert response.geturl().startswith('https://')
            while block := response.read(1024 * 1024):
                f.write(block)
        assert sha(packet) == pin, 'Packet checksum mismatch'
        inputs = out / 'inputs'
        with tarfile.open(packet) as a:
            a.extractall(inputs, filter='data')
        service, freeze = identities(inputs, commit, tree)
        result.update(packetSha256=pin, serviceReceiptSha256=sha(inputs / 'component/SERVICE.json'),
                      freezeReceiptSha256=sha(inputs / 'freeze/FREEZE.json'))
        result['phase'] = 'native-stage-and-icon'
        run('node', product / 'desktop/prepare-native-stage.cjs', 'stage',
            '--components', inputs / 'components/COMPONENT-INPUTS.json',
            '--service', inputs / 'component/SERVICE.json',
            '--freeze', inputs / 'freeze/FREEZE.json', '--out', out / 'stage')
        result['phase'] = 'essential-native-desktop-smoke'
        run('node', product / 'desktop/test/release-smoke.cjs', '--stage', out / 'stage', '--out', out / 'evidence')
        result['phase'] = 'strict-archive'
        run('node', product / 'desktop/archive-after-smoke.cjs', '--stage', out / 'stage',
            '--evidence', out / 'evidence', '--out', out / 'acceptance', '--dist', out / 'dist')
        result.update(status='PASS', phase='done')
    except Exception as e:
        result.update(status='FAIL', error=str(e))
        raise
    finally:
        (out / 'RESULT.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
