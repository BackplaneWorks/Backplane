#!/usr/bin/env python3
"""Measure one serial CI phase, locally or in the existing Boat sandbox."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tarfile
import time
import urllib.request
import zipfile
import zlib

ROOT = Path(__file__).resolve().parent.parent


def measure(command, record, env=None, aggregate=False):
    record = Path(record)
    record.parent.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    result = subprocess.run(command, cwd=ROOT, env=env)
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    data = {'seconds': round(time.monotonic()-start, 3), 'rc': result.returncode,
            'peak_rss_kib': int(after.ru_maxrss / (1024 if sys.platform=='darwin' else 1)),
            'cpu_user_seconds':round(after.ru_utime-before.ru_utime,3),
            'cpu_system_seconds':round(after.ru_stime-before.ru_stime,3),
            'rss_measurement': 'largest process in this phase, not aggregate concurrent RSS'}
    try:
        group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
        if aggregate and group.name.startswith('run-'):
            data['aggregate_peak_bytes'] = int((group/'memory.peak').read_text())
    except (OSError, ValueError, IndexError):
        pass
    record.write_text(json.dumps(data, indent=2)+'\n')
    return result.returncode if result.returncode >= 0 else 128-result.returncode


def tools():
    # Immutable upstream archives, verified before extracting executable members.
    home = Path.home()/'.local/backplane-ci'
    folder = home/'bin'; folder.mkdir(parents=True, exist_ok=True)
    pins = [
        ('bun', 'https://github.com/oven-sh/bun/releases/download/bun-v1.3.10/bun-linux-x64-baseline.zip',
         '41201a8c5ee74a9dcbb1ce25a1104f1f929838b57a845aa78d98379b0ce7cde2', 'bun-linux-x64-baseline/bun'),
        ('node', 'https://nodejs.org/dist/v24.13.1/node-v24.13.1-linux-x64.tar.xz',
         '30215f90ea3cd04dfbc06e762c021393fa173a1d392974298bbc871a8e461089', 'node-v24.13.1-linux-x64/bin/node'),
    ]
    for name, url, checksum, member in pins:
        target = folder/name
        stamp = target.with_suffix('.sha256')
        if target.is_file() and stamp.is_file() and hashlib.sha256(target.read_bytes()).hexdigest() == stamp.read_text():
            continue
        archive = home/(name+'.archive')
        with urllib.request.urlopen(url, timeout=60) as response, archive.open('wb') as out:
            import shutil
            shutil.copyfileobj(response, out)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != checksum:
            raise SystemExit('Tool archive checksum mismatch: '+name)
        if name == 'bun':
            with zipfile.ZipFile(archive) as z: target.write_bytes(z.read(member))
        else:
            with tarfile.open(archive) as t: target.write_bytes(t.extractfile(member).read())
        target.chmod(0o755)
        stamp.write_text(hashlib.sha256(target.read_bytes()).hexdigest())
        archive.unlink()
    os.environ['PATH'] = str(folder)+':'+os.environ['PATH']


def boat(encoded):
    spec = json.loads(zlib.decompress(base64.urlsafe_b64decode(encoded)))
    name = spec['name']
    if name not in ['check', 'proof', 'tests', 'payload', 'focused']:
        raise SystemExit('Unsupported remote phase')
    os.chdir(ROOT)
    os.environ.update(BEND_NO_TELEMETRY='1', BACKPLANE_TELEMETRY='off',
        BACKPLANE_TELEMETRY_BAKE='0', BACKPLANE_GOOGLE_BAKE='0', BACKPLANE_NO_UPDATE='1', BACKPLANE_ORDER_SERVER='http://127.0.0.1:9',
        CHECK_JOBS='1', CHECK_SHARDS='1', CHECK_FALLBACK='0', TEST_JOBS='2', TEST_SHARD_COUNT='1',
        BUN_JSC_forceRAMSize='4294967296')
    env = {**os.environ, 'BP_KEY_ROOTS': ' '.join(spec['roots'])} if spec['roots'] else os.environ
    def inputs():
        return subprocess.check_output(['scripts/inputs-key.sh', 'local-ci-v2:'+spec['gate'], *spec['files']],
                                       cwd=ROOT, env=env, text=True).strip()
    if inputs() != spec['base']:
        raise SystemExit('Remote source/compiler input mismatch; phase not executed')
    folder = ROOT/'build/local-remote'/name; folder.mkdir(parents=True, exist_ok=True)
    if name in ['tests', 'payload']: tools()
    if name == 'payload':
        os.environ.update(BACKPLANE_VERSION_STAMP=spec['version'], BACKPLANE_COMMIT_STAMP=spec['commit'],
            BACKPLANE_SKIP_CHECK='1', BACKPLANE_DEV_SERVICE_ONLY='1', BACKPLANE_MEM_GB='12')
        command = ['scripts/dev.sh', 'stage', str(folder/'payload')]
    else:
        command = ['scripts/test.sh'] if name == 'tests' else ['python3','scripts/local-small.py','--focused-only'] if name == 'focused' else ['scripts/check.sh', name]
        if name == 'tests': os.environ['TEST_TIMINGS_OUT'] = str(folder/'test-timings.txt')
    rc = measure(command, folder/'phase.json')
    data = json.loads((folder/'phase.json').read_text())
    import shutil
    data.update(base=spec['base'], name=name, compiler_sha256=hashlib.sha256(Path(shutil.which('bend')).read_bytes()).hexdigest())
    data['python'] = sys.version
    if name in ['tests','payload']:
        data['bun'] = subprocess.check_output(['bun','--revision'],text=True).strip()
        data['node'] = subprocess.check_output(['node','--version'],text=True).strip()
    if inputs() != spec['base']: rc = 3
    data['rc'] = rc
    (folder/'phase.json').write_text(json.dumps(data,indent=2)+'\n')
    if name == 'payload' and rc == 0:
        import importlib.util
        module = importlib.util.spec_from_file_location('local_build', ROOT/'scripts/local.py')
        local = importlib.util.module_from_spec(module); module.loader.exec_module(local)
        emit_key = subprocess.check_output(['scripts/build-js.sh','--key','src/server/main.bend'],text=True).strip()
        shutil.copy2(ROOT/'build/js/backplane-serve/all.js',folder/'emit.js')
        meta = {'commit':spec['commit'], 'version':spec['version'], 'files':local.payload_hashes(folder),
                'emit_key':emit_key,'emit_sha256':local.digest(folder/'emit.js')}
        if spec.get('bridge_needed',True):
            key = local.phone_bridge(os.environ.copy())
            shutil.copy2(ROOT/'mobile/build/assets/bridge.js', folder/'bridge.js')
            meta.update(phone_bridge_key=key,bridge_sha256=local.digest(folder/'bridge.js'))
        (folder/'REMOTE.json').write_text(json.dumps(meta,indent=2)+'\n')
        for p in [ROOT/'build/js/backplane-serve/metrics.json', ROOT/'build/web-metrics.json']:
            if p.is_file(): shutil.copy2(p,folder/p.name)
    return rc


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--boat')
    parser.add_argument('--record')
    parser.add_argument('--scope', action='store_true')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    a = parser.parse_args()
    sys.exit(boat(a.boat) if a.boat else measure(a.command[1:] if a.command[:1] == ['--'] else a.command,a.record,aggregate=a.scope))
