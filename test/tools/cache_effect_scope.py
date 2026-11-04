"""Three canonical cache policy laws + full small IO interface type closure.

Run once on normal Boat. No application compile, full/root proof, cache seed,
live data or compiler substitution. The server's two call sites remain outside
this scoped type-check and need separate integration qualification.
"""
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / 'build/hub-cache-scope'
NAMES = ['cache_effect_slots', 'cache_effect_budget', 'cache_effect_global_budget']
FILES = ['LAWS.bend', 'PROOF.bend', 'src/server/cacheio.bend', 'src/server/castio.bend',
         'src/server/effects/bounded_cache.js', 'src/server/effects/cast.js',
         'test/cache_effect_test.bend', 'test/check.bend']
RUNNERS = ['scripts/proof-shards.py', 'scripts/proof-slice.py', 'scripts/ci-packet.py',
           'scripts/bend-closure.py', 'scripts/laws-shape.py', 'scripts/bendok.sh',
           'scripts/heavy-guard.py', 'test/tools/cache_effect_scope.py', 'src/server/server.bend']
PYTHON = str(Path(sys.executable).resolve())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv, log, expected=0):
    p = subprocess.run([str(x) for x in argv], cwd=ROOT, capture_output=True, text=True, timeout=90)
    log.write_text(p.stdout + p.stderr)
    if p.returncode != expected:
        raise RuntimeError(f'{log}: expected {expected}, got {p.returncode}')


def main():
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    assert not OUT.exists(), 'inspect the existing attempt; do not repeat automatically'
    group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    assert group.name.startswith('ascii-cmd-')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime', group.name,
                    'MemoryMax=536870912', 'MemorySwapMax=0'], check=True)
    assert (group / 'memory.max').read_text().strip() == '536870912'
    assert (group / 'memory.swap.max').read_text().strip() == '0'
    OUT.mkdir(parents=True)
    head = os.environ['CACHE_SCOPE_SOURCE_HEAD']
    assert re.fullmatch(r'[0-9a-f]{40}', head)
    sources = {p: sha(ROOT / p) for p in FILES + RUNNERS}
    spec = importlib.util.spec_from_file_location('proof_slice', ROOT / 'scripts/proof-slice.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    laws, proof = module.Module(ROOT, 'LAWS.bend'), module.Module(ROOT, 'PROOF.bend')
    selected = {}
    for name in NAMES:
        assert len(laws.by_name[name]) == len(proof.by_name['Laws.' + name]) == 1
        selected[name] = [laws.by_name[name][0]['text'], proof.by_name['Laws.' + name][0]['text']]
    receipts = []
    for label, negative in [('positive', False), ('changed-budget-negative', True)]:
        at = OUT / label
        repo = at / 'source'
        repo.mkdir(parents=True)
        for p in FILES[2:]:
            target = repo / p
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / p, target)
            assert sha(target) == sources[p]
        (repo / 'LAWS.bend').write_text('import Base\nimport ./src/server/cacheio.bend as CACHEIO\n\n' + ''.join(selected[n][0] for n in NAMES))
        (repo / 'PROOF.bend').write_text('import Base\nimport ./LAWS.bend as Laws\nimport ./src/server/cacheio.bend as CACHEIO\nimport ./test/cache_effect_test.bend as CACHEBOUNDARY\n\n' + ''.join(selected[n][1] for n in NAMES))
        if negative:
            policy = repo / 'src/server/cacheio.bend'
            before = policy.read_text()
            assert before.count('CacheBudget{4096, 524288, 3001}') == 1
            policy.write_text(before.replace('CacheBudget{4096, 524288, 3001}', 'CacheBudget{4096, 524287, 3001}'))
        folder = repo / 'build/proofs'
        run([PYTHON, '-IB', ROOT / 'scripts/proof-shards.py', 'generate', '--root', repo,
             '--out', folder, '--shards', '1'], at / 'generate.log')
        manifest = json.loads((folder / 'manifest.json').read_text())
        assert sorted(manifest['laws']) == sorted(NAMES)
        result = folder / 'receipts/000.json'
        run(['systemd-run', '--user', '--scope', '--quiet', '--unit=bp-cache-interface-' + label + '-' + str(os.getpid()),
             '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
             PYTHON, ROOT / 'test/tools/cache_effect_scope.py', '--worker', repo, folder, result, at / 'resources.json'], at / 'worker.log', 1 if negative else 0)
        receipt = json.loads(result.read_text())
        resources = json.loads((at / 'resources.json').read_text())
        assert resources['events_after']['oom'] == resources['events_after']['oom_kill'] == 0
        assert resources['memory_max_bytes'] <= 3700000000 and resources['swap_max_bytes'] == 0
        receipt['scope_resources'] = resources
        assert receipt['state'] == ('failed' if negative else 'passed')
        assert receipt['actual_bend_flock_held'] and receipt['inputs_unchanged']
        assert 0 < receipt['memory_max_bytes'] <= 3700000000
        assert receipt['swap_before'] == receipt['swap_after']
        if negative:
            assert receipt['exit'] == 1 and 'cache_effect_budget' in receipt['output']
        receipts.append(receipt)
    for p, digest in sources.items():
        assert sha(ROOT / p) == digest, 'source changed during scoped qualification: ' + p
    events = {k: int(v) for k, v in (line.split() for line in (group / 'memory.events').read_text().splitlines())}
    assert events['oom'] == events['oom_kill'] == 0
    summary = {'state': 'passed', 'kind': 'three exact canonical cache policy laws + small IO interface type-check',
               'source_head': head, 'law_names': NAMES, 'source_sha256': sources,
               'canonical_blocks_sha256': {n: [hashlib.sha256(b.encode()).hexdigest() for b in blocks] for n, blocks in selected.items()},
               'controller_memory_max_bytes': 536870912, 'controller_swap_max_bytes': 0,
               'controller_memory_peak_bytes': int((group / 'memory.peak').read_text()), 'controller_events': events,
               'worker_memory_max_bytes': [r['memory_max_bytes'] for r in receipts],
               'worker_peak_rss_kib': [r['peak_rss_kib'] for r in receipts],
               'worker_seconds': [r['seconds'] for r in receipts],
               'worker_scope_resources': [r['scope_resources'] for r in receipts],
               'io_main_executed': False, 'server_typecheck_qualified': False,
               'full_proof_run': False, 'production_shards_qualified': False, 'cache_pass_seeded': False,
               'negative_control_refused': True, 'inputs_unchanged': True}
    (OUT / 'SUMMARY.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))


def worker(repo, folder, result, resource_path):
    group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    maximum = int((group / 'memory.max').read_text())
    swap = int((group / 'memory.swap.max').read_text())
    assert 0 < maximum <= 3700000000 and swap == 0
    def events():
        return {k: int(v) for k, v in (line.split() for line in (group / 'memory.events').read_text().splitlines())}
    before = events()
    p = subprocess.run([PYTHON, str(ROOT / 'scripts/heavy-guard.py'), 'proof-shard', '--',
        PYTHON, str(ROOT / 'scripts/proof-shards.py'), 'run', '--root', repo, '--out', folder,
        '--shard', '000', '--expect', sha(Path(folder) / 'manifest.json'), '--result', result], cwd=ROOT)
    after = events()
    resources = {'memory_max_bytes': maximum, 'swap_max_bytes': swap,
        'memory_peak_bytes': int((group / 'memory.peak').read_text()), 'events_before': before, 'events_after': after}
    Path(resource_path).write_text(json.dumps(resources, indent=2) + '\n')
    if after['oom'] or after['oom_kill']:
        return 3
    return p.returncode


if __name__ == '__main__':
    if sys.argv[1:2] == ['--worker']:
        assert len(sys.argv) == 6
        sys.exit(worker(*sys.argv[2:]))
    main()
