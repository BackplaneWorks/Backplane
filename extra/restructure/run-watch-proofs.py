"""Scoped Watch application-law measurement on normal Boat; no full-gate claim.

Generate the complete canonical partition, then check every shard containing a
Watch law through the merged native guard. No declaration slicing or template
laws. A missing/failed receipt is a failure, never a retry or an inferred pass.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
OUT = ROOT/'build/restructure-watch'
PYTHON = str(Path(sys.executable).resolve())
CAP = 3_700_000_000
CONTROLLER_CAP = 536_870_912
WATCH = {'watch_copy_gate_none', 'watch_live_gate_kept',
         *('watch_copy_' + op + '_none' for op in ('launch', 'check', 'stop', 'intent'))}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def group():
    return Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')


def controller_state(directory):
    return {'cgroup': str(directory),
            'memory_max_bytes': int((directory/'memory.max').read_text()),
            'swap_max_bytes': int((directory/'memory.swap.max').read_text()),
            'peak_bytes': int((directory/'memory.peak').read_text()),
            'events': {k: int(v) for k, v in (line.split() for line in (directory/'memory.events').read_text().splitlines())}}


def main():
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    assert not OUT.exists(), 'existing attempt: preserve and inspect it, never repeat automatically'
    directory = group()
    assert directory.name.startswith('ascii-cmd-'), 'expected the Boat command scope'
    # Same controller ownership as the merged real-Bend #886 experiment.
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime',
                    directory.name, 'MemoryMax='+str(CONTROLLER_CAP), 'MemorySwapMax=0'], check=True)
    start = controller_state(directory)
    assert start['memory_max_bytes'] == CONTROLLER_CAP and start['swap_max_bytes'] == 0
    assert start['events']['oom'] == start['events']['oom_kill'] == 0
    OUT.mkdir(parents=True)
    identity = json.loads((ROOT/'extra/restructure/watch-identity.json').read_text())
    for name, digest in identity['files'].items():
        assert sha(ROOT/name) == digest, name
    folder = OUT/'shards'
    planned = subprocess.run([PYTHON, ROOT/'scripts/proof-shards.py', 'generate', '--out', folder,
                              '--shards', '128'], capture_output=True, text=True)
    (OUT/'generate.log').write_text(planned.stdout+planned.stderr)
    assert planned.returncode == 0, 'canonical partition generation failed'
    manifest = json.loads((folder/'manifest.json').read_text())
    assert manifest['sources'] == identity['canonical_sources'], 'frozen source changed in transport'
    assert manifest['toolchain'] == identity['toolchain'], 'compiler or bundled libraries changed'
    partition = sha(folder/'manifest.json')
    selected = [s for s in manifest['shards'] if WATCH & set(s['primary_laws'])]
    assert set().union(*(set(s['primary_laws']) & WATCH for s in selected)) == WATCH
    observations = []
    failed = False
    for shard in selected:
        name = shard['id']
        result = folder/'receipts'/(name+'.json')
        unit = 'restructure-watch-'+name+'-'+str(os.getpid())+'.scope'
        argv = ['systemd-run', '--user', '--scope', '--quiet', '--unit='+unit,
                '-p', 'MemoryAccounting=yes', '-p', 'MemoryMax='+str(CAP),
                '-p', 'MemorySwapMax=0', '-p', 'RuntimeMaxSec=240', '--',
                PYTHON, ROOT/'scripts/heavy-guard.py', 'proof-shard', '--',
                PYTHON, ROOT/'scripts/proof-shards.py', 'run', '--out', folder,
                '--shard', name, '--expect', partition, '--result', result]
        attempt = subprocess.run([str(v) for v in argv], capture_output=True, text=True)
        (OUT/(name+'.log')).write_text(attempt.stdout+attempt.stderr)
        receipt = json.loads(result.read_text()) if result.is_file() else None
        ok = attempt.returncode == 0 and receipt is not None
        if ok:
            ok = (receipt['state'] == 'passed'
                  and receipt['actual_bend_flock_held'] is True and receipt['inputs_unchanged'] is True
                  and 0 < receipt['memory_max_bytes'] <= CAP and receipt['manifest_sha256'] == partition
                  and receipt['swap_before'] == receipt['swap_after'] == {'pswpin': 0, 'pswpout': 0})
        failed |= not ok
        observations.append({'shard': name, 'laws': shard['primary_laws'],
                             'watch_laws': sorted(WATCH & set(shard['primary_laws'])),
                             'argv': [str(v) for v in argv], 'scope_exit': attempt.returncode,
                             'receipt': receipt, 'passed': ok})
    for name, digest in identity['files'].items():
        assert sha(ROOT/name) == digest, name
    final = controller_state(directory)
    assert final['memory_max_bytes'] == CONTROLLER_CAP and final['swap_max_bytes'] == 0
    assert final['events']['oom'] == final['events']['oom_kill'] == 0
    report = {'scope': 'Full canonical partition; only Watch-containing shards executed; not a full gate',
              'state': 'failed' if failed else 'passed', 'slicer_used': False,
              'canonical_laws': len(manifest['laws']), 'partition_sha256': partition,
              'toolchain': manifest['toolchain'], 'guard_sha256': manifest['guard_sha256'],
              'controller': final, 'worker_cap_bytes': CAP,
              'combined_cap_bytes': CAP+CONTROLLER_CAP, 'identity_sha256': sha(ROOT/'extra/restructure/watch-identity.json'),
              'observations': observations, 'full_gate_qualified': False}
    (OUT/'RESULT.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('toolchain', 'observations')}))
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
