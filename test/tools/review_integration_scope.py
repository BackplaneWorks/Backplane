"""Two existing review merge laws + whole unchanged review integration closure.

Run once on normal Boat. No application compile, full/root proof, cache seed,
live data or compiler substitution. The whole actual ReviewHistory/test closure is checked; no full server or root proof.
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
OUT = ROOT / 'build/hub-review-adapter-scope'
NAMES = ['review_history_keeps_newest', 'review_history_overlap_once']
ENTRY = 'test/review_history_adapter_test.bend'
FILES = []
RUNNERS = ['scripts/proof-shards.py', 'scripts/proof-slice.py', 'scripts/ci-packet.py',
           'scripts/bend-closure.py', 'scripts/laws-shape.py', 'scripts/bendok.sh',
           'scripts/heavy-guard.py', 'scripts/heavy-boat.sh', 'scripts/local-phase.py', 'test/tools/review_integration_scope.py',
           'src/server/review-history.bend', 'test/tools/review_window_test.cjs',
           'test/fixtures/review_adapter_reference.txt', 'test/fixtures/window_file_reference.json',
           'src/server/thin-view.bend', 'test/fixtures/review_history_adapter_runtime.json',
           'test/tools/review_history_adapter_runtime.py', 'extra/hub-review-qualification/SUMMARY.json']
PYTHON = str(Path(sys.executable).resolve())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv, log, expected=0):
    p = subprocess.run([str(x) for x in argv], cwd=ROOT, capture_output=True, text=True, timeout=900)
    log.write_text(p.stdout + p.stderr)
    if p.returncode != expected:
        unit = next((str(x).split('=', 1)[1] for x in argv if str(x).startswith('--unit=')), None)
        if unit:
            details = {'launcher_exit':p.returncode, 'unit':unit+'.scope', 'cause':'unclassified; inspect actual unit/journal'}
            for label, command in [('unit', ['systemctl','--user','show',unit+'.scope','-p','Result','-p','OOMPolicy','-p','MemoryMax','-p','MemorySwapMax']),
                                   ('journal', ['journalctl','--user','--no-pager','-u',unit+'.scope','-n','30','-o','json'])]:
                observed = subprocess.run(command,capture_output=True,text=True,timeout=10)
                details[label] = {'exit':observed.returncode,'stdout':observed.stdout,'stderr':observed.stderr}
            log.with_suffix('.failure.json').write_text(json.dumps(details,indent=2)+'\n')
        raise RuntimeError(f'{log}: expected {expected}, got {p.returncode}')


def main():
    global FILES
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    assert not OUT.exists(), 'inspect the existing attempt; do not repeat automatically'
    group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    assert group.name.startswith('bp-review-integration-controller-')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime', group.name,
                    'MemoryMax=536870912', 'MemorySwapMax=0'], check=True)
    assert (group / 'memory.max').read_text().strip() == '536870912'
    assert (group / 'memory.swap.max').read_text().strip() == '0'
    OUT.mkdir(parents=True)
    FILES = ['LAWS.bend', 'PROOF.bend', *subprocess.check_output([PYTHON, 'scripts/bend-closure.py', ENTRY],text=True).splitlines()]
    assert 'src/server/window-file.bend' in FILES and 'src/server/review-history.bend' in FILES
    assert not any(name.startswith(('src/web/', 'src/mobile/', 'src/app/')) or name=='src/server/thin-view.bend' for name in FILES), 'actual adapter closure still imports a renderer'
    head = os.environ['REVIEW_INTEGRATION_SOURCE_HEAD']
    assert re.fullmatch(r'[0-9a-f]{40}', head)
    sources = {p: sha(ROOT / p) for p in FILES + RUNNERS}
    previous = json.loads((ROOT / 'extra/hub-review-qualification/SUMMARY.json').read_text())
    assert previous['state'] == 'passed'
    preserved_boundary = ['src/server/db.bend', 'src/server/effects/db.js', 'src/server/review-window.bend', 'test/tools/review_window_test.cjs']
    for name in preserved_boundary:
        assert sha(ROOT / name) == previous['source_sha256'][name], name
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
    old = module.Module(ROOT, 'test/fixtures/review_adapter_reference.txt')
    current = module.Module(ROOT, 'src/server/review-history.bend')
    preserved = {}
    for name, blocks in old.by_name.items():
        assert len(blocks) == len(current.by_name[name]) == 1
        assert body(blocks[0]['text']) == body(current.by_name[name][0]['text']), name
        preserved[name] = hashlib.sha256(body(blocks[0]['text']).encode()).hexdigest()
    reference = json.loads((ROOT / 'test/fixtures/window_file_reference.json').read_text())
    leaf = module.Module(ROOT, 'src/server/window-file.bend')
    thin = module.Module(ROOT, 'src/server/thin-view.bend')
    assert set(leaf.imports)=={'Base','M'} and leaf.imports['M'][1]=='src/core/model.bend'
    assert set(leaf.by_name)=={'State.window.file'}
    assert body(leaf.by_name['State.window.file'][0]['text']) == body(reference['definition'])
    signature = reference['definition'].splitlines()[0]
    delegate = signature+'\n  WF.State.window.file(st, key, es)'
    assert body(thin.by_name['State.window.file'][0]['text'])==delegate
    current_thin = (ROOT / 'src/server/thin-view.bend').read_text()
    restored = current_thin.replace(delegate, reference['definition'].rstrip('\n'),1).replace('import ./window-file.bend as WF\n','',1)
    assert hashlib.sha256(restored.encode()).hexdigest()==reference['source_sha256'], 'unrelated thin-view source changed'
    adapter_before = (ROOT / 'test/fixtures/review_adapter_reference.txt').read_text()
    assert (ROOT / 'src/server/review-history.bend').read_text()==adapter_before.replace('import ./thin-view.bend as V','import ./window-file.bend as V',1)
    (OUT / 'source-audit.json').write_text(json.dumps({'kind':'exact production helper extraction/transparent delegate; whole adapter bodies preserved',
        'reference_source_head':reference['source_head'], 'preserved_adapter_defs':preserved,
        'extracted_definition_sha256':hashlib.sha256(body(reference['definition']).encode()).hexdigest(),
        'thin_view_restored_source_sha256':hashlib.sha256(restored.encode()).hexdigest(),
        'actual_whole_closure':FILES[2:], 'declaration_slicing':False, 'thin_view_delegate_typecheck_qualified':False},indent=2)+'\n')
    selected = {}
    for name in NAMES:
        assert len(laws.by_name[name]) == len(proof.by_name['Laws.' + name]) == 1
        selected[name] = [laws.by_name[name][0]['text'], proof.by_name['Laws.' + name][0]['text']]
    receipts = []
    for label, negative in [('positive', False), ('changed-merge-negative', True)]:
        at = OUT / label
        repo = at / 'source'
        repo.mkdir(parents=True)
        for p in FILES[2:]:
            target = repo / p
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / p, target)
            assert sha(target) == sources[p]
        (repo / 'LAWS.bend').write_text('import Base\nimport ./src/server/review-history.bend as REVIEWHISTORY\nimport ./src/core/model.bend as M\nimport ./test/review_history_adapter_test.bend as REVIEWTEST\n\n' + ''.join(selected[n][0] for n in NAMES))
        (repo / 'PROOF.bend').write_text('import Base\nimport ./LAWS.bend as Laws\nimport ./src/server/review-history.bend as REVIEWHISTORY\nimport ./src/core/model.bend as M\nimport ./test/review_history_adapter_test.bend as REVIEWTEST\n\n' + ''.join(selected[n][1] for n in NAMES))
        if negative:
            policy = repo / 'src/server/review-history.bend'
            before = policy.read_text()
            assert before.count('List.append(&2, M.Entry, known, ReviewHistory.older(stored, ReviewHistory.ids(known, Map.new(&2, Bool)), Nil{}))') == 1
            policy.write_text(before.replace('List.append(&2, M.Entry, known, ReviewHistory.older(stored, ReviewHistory.ids(known, Map.new(&2, Bool)), Nil{}))', 'stored'))
        folder = repo / 'build/proofs'
        run([PYTHON, '-IB', ROOT / 'scripts/proof-shards.py', 'generate', '--root', repo,
             '--out', folder, '--shards', '1'], at / 'generate.log')
        manifest = json.loads((folder / 'manifest.json').read_text())
        assert sorted(manifest['laws']) == sorted(NAMES)
        result = folder / 'receipts/000.json'
        run(['systemd-run', '--user', '--scope', '--quiet', '--unit=bp-review-integration-' + label + '-' + str(os.getpid()),
             '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
             PYTHON, ROOT / 'test/tools/review_integration_scope.py', '--worker', repo, folder, result, at / 'resources.json'], at / 'worker.log', 1 if negative else 0)
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
            assert receipt['exit'] == 1 and any(name in receipt['output'] for name in NAMES)
        receipts.append(receipt)
    for p, digest in sources.items():
        assert sha(ROOT / p) == digest, 'source changed during scoped qualification: ' + p
    events = {k: int(v) for k, v in (line.split() for line in (group / 'memory.events').read_text().splitlines())}
    assert events['oom'] == events['oom_kill'] == 0
    summary = {'state': 'passed', 'kind': 'two existing exact canonical review merge laws + whole actual revised M.State adapter/test type closure',
               'source_head': head, 'law_names': NAMES, 'source_sha256': sources, 'compiled_source_files':FILES[2:],
               'boundary_reused_from':previous['source_head'], 'boundary_inputs_unchanged':preserved_boundary,
               'source_policy_audit_passed':True, 'runtime_tests_executed':False, 'integration_assertions_runtime_qualified':False,
               'canonical_blocks_sha256': {n: [hashlib.sha256(b.encode()).hexdigest() for b in blocks] for n, blocks in selected.items()},
               'controller_memory_max_bytes': 536870912, 'controller_swap_max_bytes': 0,
               'controller_memory_peak_bytes': int((group / 'memory.peak').read_text()), 'controller_events': events,
               'worker_memory_max_bytes': [r['memory_max_bytes'] for r in receipts],
               'worker_peak_rss_kib': [r['peak_rss_kib'] for r in receipts],
               'worker_seconds': [r['seconds'] for r in receipts],
               'worker_scope_resources': [r['scope_resources'] for r in receipts],
               'io_main_executed': False, 'server_typecheck_qualified': False, 'full_review_history_typecheck_qualified': True, 'thin_view_delegate_typecheck_qualified':False,
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
