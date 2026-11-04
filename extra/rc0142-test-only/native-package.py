"""Matching-host TEST ABI assembly/stage/archive; no emitter or release claim."""
import argparse
import hashlib
import json
import shutil
import os
from pathlib import Path
import subprocess
import sys
import tarfile


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('source', 'tools', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    for name in ('target', 'components-sha256'):
        p.add_argument('--' + name, required=True)
    p.add_argument('--authenticated-inputs', type=Path)
    p.add_argument('--retained-measurement', type=Path)
    p.add_argument('--windows-sha256', default='')
    p.add_argument('--resume-native', type=Path)
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
        inputs = a.resume_native or a.authenticated_inputs
        assert inputs is not None, 'Authenticated Actions artifact required; no URL fallback'
        original = inputs.resolve() / 'components.tar.gz'
        assert original.is_file() and not original.is_symlink() and sha(original) == a.components_sha256
        shutil.copyfile(original, packet)
        with tarfile.open(packet) as tar:
            tar.extractall(out / 'inputs', filter='data')
        args = []
        if a.target == 'windows-x64' and not a.resume_native:
            assert a.windows_sha256 == '42ef24bf69566f3d2d56aebf4b689276831e6e70d629d53d9cb9fc313a357412'
            win = out / 'windows-runtime.tar.gz'
            original = inputs.resolve() / 'windows-runtime.tar.gz'
            assert original.is_file() and not original.is_symlink() and sha(original) == a.windows_sha256
            shutil.copyfile(original, win)
            with tarfile.open(win) as tar:
                tar.extractall(out / 'windows-runtime', filter='data')
            args = ['--windows-runtime', out / 'windows-runtime']
        build = (a.resume_native.resolve() / 'service-component/component/payload') if a.resume_native else source / 'build/test-service'
        service = build / ('backplane-serve.exe' if a.target == 'windows-x64' else 'backplane-serve')
        service.chmod(0o755)
        producer = tools / 'scripts/service-release.py'
        if a.resume_native:
            # Previous native ABI/component succeeded; reuse it byte-for-byte.
            # Only artifact-mode restoration and missing stage/archive follow.
            component = a.resume_native.resolve() / 'service-component/component/SERVICE.json'
            record['nativeAbiReplayed'] = False
            record['componentSource'] = str(component)
        else:
            factory = [tools / 'scripts/service-runtime.py']
            if a.target == 'windows-x64':
                assert a.retained_measurement, 'Reuse existing measured ABI; no fresh measurement'
                factory = [Path(__file__).with_name('reconstruct-measured-component.py'), '--factory', tools / 'scripts/service-runtime.py', '--retained', a.retained_measurement]
                record['nativeAbiReplayed'] = False
                record['componentRecovery'] = 'Authenticated retained measurement and exact member hashes; no DLL observation replay'
            run([sys.executable, *factory, *args,
                '--source-root', source, '--build', build, '--producer', producer,
                '--producer-sha256', sha(producer), '--out', out / 'service-component'], 'abi-and-component')
            component = out / 'service-component/component/SERVICE.json'
        assert json.loads(component.read_text())['releaseQualified'] is False
        def restore_modes(root, members):
            # The whole input archive is already SHA-bound. Python's safe
            # tar filter and Actions strip modes; restore only receipt modes,
            # then let the unchanged stage membership/hash guard verify all.
            if os.name == 'nt':
                return  # original producer applies its Windows mode normalization
            root = root.resolve()
            for name, item in members.items():
                if item['type'] != 'file':
                    continue
                relative = Path(name)
                assert not relative.is_absolute() and '..' not in relative.parts
                path = root / relative
                assert path.is_file() and not path.is_symlink() and root in path.resolve().parents
                mode = item['mode']
                assert isinstance(mode, int) and 0 <= mode <= 0o777
                path.chmod(mode)
        components = out / 'inputs/components'
        restore_modes(components / 'shell-payload', json.loads((components / 'SHELL.json').read_text())['members'])
        restore_modes(components / 'payload', json.loads((components / 'ELECTRON.json').read_text())['members'])
        restore_modes(component.parent / 'payload', json.loads(component.read_text())['members'])
        # freeze-target derives identity from the original measured component,
        # even when a stage-only recovery uses its retained runtime evidence.
        run([sys.executable, source / 'desktop/freeze-target.py', source,
            components, component, out / 'freeze'], 'freeze')
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
