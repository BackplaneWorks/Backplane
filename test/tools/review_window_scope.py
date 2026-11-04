"""Two canonical review-window laws + full unchanged small IO/fold closure.

Run once on normal Boat. No application compile, full/root proof, cache seed,
live data or compiler substitution. The whole typed generic page fold is checked;
the private review-state integration remains separately identified.
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
OUT = ROOT / 'build/hub-review-scope'
NAMES = ['review_window_budget', 'review_window_progress']
FILES = ['LAWS.bend', 'PROOF.bend', *subprocess.check_output(
    [sys.executable, 'scripts/bend-closure.py', 'test/review_window_test.bend'], text=True).splitlines()]
RUNNERS = ['scripts/proof-shards.py', 'scripts/proof-slice.py', 'scripts/ci-packet.py',
           'scripts/bend-closure.py', 'scripts/laws-shape.py', 'scripts/bendok.sh',
           'scripts/heavy-guard.py', 'scripts/local-phase.py', 'test/tools/review_window_scope.py',
           'src/server/review-history.bend', 'test/tools/review_window_test.cjs',
           'test/fixtures/review_history_reference.txt']
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
    assert group.name.startswith('bp-review-controller-')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime', group.name,
                    'MemoryMax=536870912', 'MemorySwapMax=0'], check=True)
    assert (group / 'memory.max').read_text().strip() == '536870912'
    assert (group / 'memory.swap.max').read_text().strip() == '0'
    OUT.mkdir(parents=True)
    head = os.environ['REVIEW_SCOPE_SOURCE_HEAD']
    assert re.fullmatch(r'[0-9a-f]{40}', head)
    sources = {p: sha(ROOT / p) for p in FILES + RUNNERS}
    phase = importlib.util.spec_from_file_location('local_phase', ROOT / 'scripts/local-phase.py')
    tools = importlib.util.module_from_spec(phase)
    phase.loader.exec_module(tools)
    tools.tools()
    run(['bun', 'test', './test/tools/review_window_test.cjs'], OUT / 'bun-boundary.log')
    run([PYTHON, '-IB', 'scripts/laws-shape.py'], OUT / 'laws-shape.log')
    spec = importlib.util.spec_from_file_location('proof_slice', ROOT / 'scripts/proof-slice.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    laws, proof = module.Module(ROOT, 'LAWS.bend'), module.Module(ROOT, 'PROOF.bend')
    def body(text):
        lines = text.splitlines()
        while lines and (not lines[-1].strip() or lines[-1].startswith('#')):
            lines.pop()
        return '\n'.join(lines)
    old = module.Module(ROOT, 'test/fixtures/review_history_reference.txt')
    current = module.Module(ROOT, 'src/server/review-history.bend')
    preserved = {}
    for name, blocks in old.by_name.items():
        if name == 'ReviewHistory.read.local':
            continue
        assert len(blocks) == len(current.by_name[name]) == 1
        assert body(blocks[0]['text']) == body(current.by_name[name][0]['text']), name
        preserved[name] = hashlib.sha256(body(blocks[0]['text']).encode()).hexdigest()
    (OUT / 'source-audit.json').write_text(json.dumps({'kind':'source-only preservation of existing pure folds, merge, baseline and skip policies',
        'reference_source_head':'6b031e406f8f30d4e89743e0e030631290022296', 'preserved':preserved,
        'full_review_history_typecheck_qualified':False},indent=2)+'\n')
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
        (repo / 'LAWS.bend').write_text('import Base\nimport ./src/server/review-window.bend as REVIEWWINDOW\nimport ./src/server/db.bend as HISTORYDB\nimport ./test/review_window_test.bend as REVIEWTEST\n\n' + ''.join(selected[n][0] for n in NAMES))
        (repo / 'PROOF.bend').write_text('import Base\nimport ./LAWS.bend as Laws\nimport ./src/server/review-window.bend as REVIEWWINDOW\nimport ./src/server/db.bend as HISTORYDB\nimport ./test/review_window_test.bend as REVIEWTEST\n\n' + ''.join(selected[n][1] for n in NAMES))
        if negative:
            policy = repo / 'src/server/review-window.bend'
            before = policy.read_text()
            assert before.count('D.HistoryBudget{128, 1048576}') == 1
            policy.write_text(before.replace('D.HistoryBudget{128, 1048576}', 'D.HistoryBudget{127, 1048576}'))
        folder = repo / 'build/proofs'
        run([PYTHON, '-IB', ROOT / 'scripts/proof-shards.py', 'generate', '--root', repo,
             '--out', folder, '--shards', '1'], at / 'generate.log')
        manifest = json.loads((folder / 'manifest.json').read_text())
        assert sorted(manifest['laws']) == sorted(NAMES)
        result = folder / 'receipts/000.json'
        run(['systemd-run', '--user', '--scope', '--quiet', '--unit=bp-review-interface-' + label + '-' + str(os.getpid()),
             '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
             PYTHON, ROOT / 'test/tools/review_window_scope.py', '--worker', repo, folder, result, at / 'resources.json'], at / 'worker.log', 1 if negative else 0)
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
            assert receipt['exit'] == 1 and 'review_window_budget' in receipt['output']
        receipts.append(receipt)
    for p, digest in sources.items():
        assert sha(ROOT / p) == digest, 'source changed during scoped qualification: ' + p
    events = {k: int(v) for k, v in (line.split() for line in (group / 'memory.events').read_text().splitlines())}
    assert events['oom'] == events['oom_kill'] == 0
    summary = {'state': 'passed', 'kind': 'two exact canonical review window laws + full small IO/fold interface type-check',
               'source_head': head, 'law_names': NAMES, 'source_sha256': sources,
               'bun_boundary_cases':7,
               'source_policy_audit_passed':True, 'node_version':subprocess.check_output(['node','--version'],text=True).strip(),
               'bun_revision':subprocess.check_output(['bun','--revision'],text=True).strip(),
               'canonical_blocks_sha256': {n: [hashlib.sha256(b.encode()).hexdigest() for b in blocks] for n, blocks in selected.items()},
               'controller_memory_max_bytes': 536870912, 'controller_swap_max_bytes': 0,
               'controller_memory_peak_bytes': int((group / 'memory.peak').read_text()), 'controller_events': events,
               'worker_memory_max_bytes': [r['memory_max_bytes'] for r in receipts],
               'worker_peak_rss_kib': [r['peak_rss_kib'] for r in receipts],
               'worker_seconds': [r['seconds'] for r in receipts],
               'worker_scope_resources': [r['scope_resources'] for r in receipts],
               'io_main_executed': False, 'server_typecheck_qualified': False, 'review_window_leaf_typecheck_qualified': True, 'full_review_history_typecheck_qualified': False,
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
