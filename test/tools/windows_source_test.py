#!/usr/bin/env python3
"""Exercise source/channel receipts with temporary metadata, never a compiler."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

source_root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='bp-windows-source-') as temp:
    root = Path(temp)
    (root / 'scripts').mkdir()
    for name in ('windows-source.py', 'package-windows.py'):
        shutil.copy2(source_root / 'scripts' / name, root / 'scripts' / name)
    (root / '.gitignore').write_text('build/\ndist/\n')
    (root / 'LICENSE').write_text('Metadata fixture only\n')
    for name in ('setup-windows.ps1', 'install-windows-providers.ps1'):
        (root / 'scripts' / name).write_text('# Metadata fixture only\n')
    (root / 'deploy/windows').mkdir(parents=True)
    (root / 'deploy/windows/README.txt').write_text('Metadata fixture only\n')

    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root, text=True,
                                       stderr=subprocess.DEVNULL).strip()

    git('init', '-q')
    git('config', 'user.name', 'i2cjak')
    git('config', 'user.email', 'i2cjak@users.noreply.github.com')
    git('add', '.')
    git('commit', '-qm', 'metadata fixture')
    frozen = git('rev-parse', 'HEAD')
    inputs = root / 'build/windows-emitted'
    inputs.mkdir(parents=True)
    for name in ('backplane.c.gz', 'runtime.c.gz', 'providers.c.gz'):
        (inputs / name).write_bytes(b'metadata fixture; not emitted C')
    # Tiny placeholders exercise ZIP metadata only; these are not native binaries.
    for name in ('backplane.exe', 'backplane-kipc.exe', 'backplane-step2glb.exe'):
        p = root / 'dist' / name
        p.parent.mkdir(exist_ok=True)
        p.write_bytes(b'MZ metadata fixture only')
    for name in ('web/index.html', 'WINDOWS-HELPERS.json', 'licenses/bun/LICENSE.md',
                 'licenses/step2glb/license.occt.txt'):
        p = root / 'dist' / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text('Metadata fixture only\n')

    base_env = {k: v for k, v in os.environ.items()
                if not k.startswith('BACKPLANE_TELEMETRY_')
                and k not in ('BACKPLANE_VERSION', 'BACKPLANE_VERSION_STAMP')}

    def run(script, *args, ok=True, **env):
        result = subprocess.run([sys.executable, 'scripts/' + script, *args],
                                cwd=root, env={**base_env, **env},
                                capture_output=True, text=True)
        assert (result.returncode == 0) == ok, (script, args, result.stdout, result.stderr)
        return result

    for version, channel in (('v0.12.0', 'release'), ('v0.12.0-nightly.20261019', 'nightly')):
        env = {'BACKPLANE_VERSION': version}
        run('windows-source.py', 'record', **env)
        run('windows-source.py', 'check', **env)
        receipt = json.loads((inputs / 'SOURCE.json').read_text())
        assert receipt['schema'] == 2 and receipt['telemetry_channel'] == channel
        run('package-windows.py', version, **env)
        with zipfile.ZipFile(root / f'dist/backplane-{version}-windows-x64.zip') as archive:
            raw = archive.read('backplane-windows-x64/SOURCE.json')
            manifest = json.loads(archive.read('backplane-windows-x64/BUILD.json'))
            assert manifest['telemetryChannel'] == channel
            assert manifest['commit'] == receipt['commit'] == frozen
            assert manifest['sha256']['SOURCE.json'] == hashlib.sha256(raw).hexdigest()
        wrong = 'release' if channel == 'nightly' else 'nightly'
        run('windows-source.py', 'check', ok=False, **env, BACKPLANE_TELEMETRY_CHANNEL=wrong)
        run('windows-source.py', 'check', ok=False, **env, BACKPLANE_TELEMETRY_CHANNEL='user-setting')
        print('ok version-derived channel, package receipt and mismatch refusal:', channel)

    env = {'BACKPLANE_VERSION': 'v0.12.0'}
    run('windows-source.py', 'record', **env)
    run('windows-source.py', 'check', **env, BACKPLANE_TELEMETRY_CHANNEL='release')
    run('windows-source.py', 'check', ok=False, **env, BACKPLANE_VERSION_STAMP='0.12.0-nightly.test')
    run('package-windows.py', 'v0.12.0-nightly.test', ok=False, **env)
    run('package-windows.py', 'v0.12.1', ok=False, **env)
    (inputs / 'SOURCE.json').write_text((inputs / 'SOURCE.json').read_text().replace('"release"', '"nightly"'))
    run('windows-source.py', 'check', ok=False, **env)
    run('windows-source.py', 'record', **env)
    run('windows-source.py', 'check', ok=False, BACKPLANE_VERSION='v0.12.1')
    (inputs / 'backplane.c.gz').write_bytes(b'changed input')
    run('windows-source.py', 'check', ok=False, **env)
    (inputs / 'backplane.c.gz').write_bytes(b'metadata fixture; not emitted C')
    (root / 'LICENSE').write_text('dirty source\n')
    run('windows-source.py', 'check', ok=False, **env)
    git('add', 'LICENSE')
    git('commit', '-qm', 'advance source after emit')
    run('windows-source.py', 'check', ok=False, **env)
    print('ok stamp precedence, package version/channel, tamper, inputs and source guards')
print('WINDOWS_SOURCE_CHANNEL_PACKAGE_METADATA_PASS_NO_NATIVE_BUILD')
