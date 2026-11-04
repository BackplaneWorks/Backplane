"""Boat-only pinned Disk.sync boundary/component gate. No Bend or app build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile

OUT = Path('build/disk-sync-component')
BUN_ARCHIVE = '41201a8c5ee74a9dcbb1ce25a1104f1f929838b57a845aa78d98379b0ce7cde2'
BUN_EXE = '795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        for part in iter(lambda: f.read(65536), b''):
            h.update(part)
    return h.hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')


def resources(cap):
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == '1'
    assert os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::')[1].strip().lstrip('/')
    read = lambda name: (group / name).read_text().strip()
    events = dict((k, int(v)) for k, v in (s.split() for s in read('memory.events').splitlines()))
    r = dict(memory_max_bytes=int(read('memory.max')), swap_max_bytes=int(read('memory.swap.max')),
             memory_peak_bytes=int(read('memory.peak')), swap_current_bytes=int(read('memory.swap.current')),
             swap_peak_bytes=int(read('memory.swap.peak')), events=events)
    assert 0 < r['memory_max_bytes'] <= cap
    assert r['swap_max_bytes'] == r['swap_current_bytes'] == r['swap_peak_bytes'] == 0
    assert all(events[k] == 0 for k in ['oom', 'oom_kill', 'oom_group_kill'])
    return r


def inputs(args):
    assert sha(args.inputs) == args.sha256
    m = json.loads(Path(args.inputs).read_text())
    # The reviewed runner commits its staged subset with the source SHA as
    # subject. Its snapshot OID/tree are distinct from the source checkout.
    subject = subprocess.check_output(['git', 'show', '-s', '--format=%s', 'HEAD'], text=True).strip()
    assert subject == m['head']
    tracked = set(subprocess.check_output(['git', 'ls-files', '-z']).decode().rstrip('\0').split('\0'))
    assert tracked == set(m['files']) | {args.inputs}
    for name, checksum in m['files'].items():
        assert sha(name) == checksum, name
    return m


def bun():
    folder = OUT / 'tools'
    folder.mkdir(parents=True, exist_ok=True)
    archive = folder / 'bun.zip'
    with urllib.request.urlopen('https://github.com/oven-sh/bun/releases/download/bun-v1.3.10/bun-linux-x64-baseline.zip', timeout=60) as response, archive.open('wb') as f:
        shutil.copyfileobj(response, f, length=65536)
    assert sha(archive) == BUN_ARCHIVE
    target = folder / 'bun'
    with zipfile.ZipFile(archive) as z, z.open('bun-linux-x64-baseline/bun') as src, target.open('wb') as dst:
        shutil.copyfileobj(src, dst, length=65536)
    archive.unlink()
    target.chmod(0o700)
    assert sha(target) == BUN_EXE
    assert subprocess.check_output([str(target.resolve()), '--version'], text=True).strip() == '1.3.10'
    return target.resolve()


def worker(args, manifest):
    resources(3700000000)
    tool = Path(args.bun).resolve()
    assert sha(tool) == BUN_EXE
    env = dict(os.environ, DISK_SYNC_SOURCE_HEAD=manifest['head'], DISK_SYNC_TRANSFER_HARNESS=manifest['template'])
    commands = [
        ('boundary', ['test/tools/disk_sync_test.cjs'], 'DISK_SYNC_BOUNDARY_PASS cases=19', 19),
        ('checkpoint', ['test/tools/store_checkpoint_io_test.cjs'], '25 focused native checkpoint cases passed', 25),
        ('ab', ['test/tools/disk_sync_bench.cjs', '--out', str(OUT / 'RESULTS.json')],
         'DISK_SYNC_COMPONENT_AB_PASS rounds=32+32 bytes=checked order=checked child=closed', None),
    ]
    records = []
    try:
        for kind, command, sentinel, count in commands:
            started = time.monotonic()
            log = OUT / (kind + '.log')
            with log.open('w') as f:
                result = subprocess.run([str(tool), *command], env=env, stdout=f, stderr=subprocess.STDOUT, timeout=480)
            text = log.read_text()
            record = dict(kind=kind, exit=result.returncode, seconds=time.monotonic() - started, log_sha256=sha(log),
                          sentinel=sentinel in text, resources=resources(3700000000))
            records.append(record)
            save('WORKER.json', dict(state='incomplete', records=records))
            assert result.returncode == 0 and sentinel in text and 'FAIL' not in text, kind
            if count is not None:
                assert sum(line.startswith('ok ') for line in text.splitlines()) == count, kind
        report = json.loads((OUT / 'RESULTS.json').read_text())
        assert report['state'] == 'passed' and report['allProbesStartedDuringBurst'] and report['byteAndOrderChecks']
        assert report['identity']['before_function_sha256'] == manifest['baseline_function_sha256']
        assert len(report['samples']) == 64 and len(report['warmups']) == 4
        inputs(args)
        save('WORKER.json', dict(state='passed', head=manifest['head'], records=records, resources=resources(3700000000),
                                 tool_sha256=sha(tool), results_sha256=sha(OUT / 'RESULTS.json'), inputs_unchanged=True,
                                 full_hub=False, packaged_acceptance=False))
        print('DISK_SYNC_GATE_WORKER_PASS boundary19 checkpoint25 ab32+32')
    except BaseException:
        save('WORKER.json', dict(state='failed', head=manifest['head'], records=records, resources=resources(3700000000)))
        raise


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--inputs', required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--worker', action='store_true')
    parser.add_argument('--bun')
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = inputs(args)
    if args.worker:
        worker(args, manifest)
        return
    resources(536870912)
    save('SOURCE.json', dict(submitted_head=manifest['head'], submitted_tree=manifest['tree'],
                             boat_snapshot_head=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                             boat_snapshot_tree=subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], text=True).strip(),
                             exact_staged_files_and_hashes=True, full_checkout=False))
    tool = bun()
    save('TOOLS.json', dict(bun_version='1.3.10', bun_sha256=sha(tool), archive_sha256=BUN_ARCHIVE,
                            python_sha256=sha(Path(sys.executable).resolve())))
    command = ['systemd-run', '--user', '--scope', '--quiet', '--unit=bp-disk-sync-worker-' + manifest['head'][:8],
               '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--', sys.executable, __file__,
               '--inputs', args.inputs, '--sha256', args.sha256, '--worker', '--bun', str(tool)]
    with (OUT / 'worker.log').open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=600)
    save('SUMMARY.json', dict(state='passed' if result.returncode == 0 else 'failed', head=manifest['head'],
                              exit=result.returncode, controller_resources=resources(536870912),
                              inputs_sha256=args.sha256, worker_log_sha256=sha(OUT / 'worker.log'),
                              no_hub=True, no_private_fixture=True, no_bend_or_app_build=True))
    assert result.returncode == 0
    assert 'DISK_SYNC_GATE_WORKER_PASS' in (OUT / 'worker.log').read_text()
    inputs(args)
    print('DISK_SYNC_GATE_PASS boundary19 checkpoint25 ab32+32')


if __name__ == '__main__':
    main()
