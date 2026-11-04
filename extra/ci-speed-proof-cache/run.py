"""ONE normal-Boat, real-Bend proof cache experiment; no app/full-repo proof."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path.cwd()
LAB = ROOT/'extra/ci-speed-proof-cache'
OUT = ROOT/'build/ci-packet/proof-cache'
PYTHON = str(Path(sys.executable).resolve())
ENV = {**os.environ, 'BEND_NO_TELEMETRY': '1', 'DO_NOT_TRACK': '1',
       'BUN_CONFIG_NO_ANALYTICS': '1', 'PYTHONDONTWRITEBYTECODE': '1', 'BENDTT': '', 'CC': ''}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def command(argv, path, expected=0):
    started = time.monotonic()
    result = subprocess.run([str(v) for v in argv], cwd=ROOT, env=ENV,
                            capture_output=True, text=True, timeout=60)
    path.write_text(result.stdout+result.stderr)
    assert result.returncode == expected, (str(path), result.returncode, expected)
    return time.monotonic()-started


def main():
    assert ENV.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == ENV.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    assert not OUT.exists(), 'existing experiment; inspect it, never repeat automatically'
    directory = Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    assert directory.name.startswith('ascii-cmd-')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime',
                    directory.name, 'MemoryMax=536870912', 'MemorySwapMax=0'], check=True)
    assert (directory/'memory.max').read_text().strip() == '536870912'
    assert (directory/'memory.swap.max').read_text().strip() == '0'
    OUT.mkdir(parents=True)
    identity = json.loads((LAB/'IDENTITY.json').read_text())
    spec = importlib.util.spec_from_file_location('packet_identity', ROOT/'scripts/ci-packet.py')
    packet = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(packet)
    assert packet.toolchain() == identity['toolchain'], 'compiler/library identity differs'
    for name, digest in identity['files'].items():
        assert sha(ROOT/name) == digest, name
    def events():
        return {k: int(v) for k, v in (line.split() for line in (directory/'memory.events').read_text().splitlines())}
    assert events()['oom'] == events()['oom_kill'] == 0
    pc = [PYTHON, '-IB', ROOT/'scripts/proof-cache.py']
    worker_pc = [PYTHON, ROOT/'scripts/proof-cache.py']
    observations = []
    with tempfile.TemporaryDirectory(prefix='bp-real-proof-cache-') as private_cache:
        def snapshot(label, changed=False, false=False):
            at = OUT/label
            repo = at/'tree'
            repo.mkdir(parents=True)
            laws = (LAB/'LAWS.bend.in').read_text()
            proof = (LAB/'PROOF.bend.in').read_text()
            if changed:
                proof += '\n# Exact proof text changed; the theorem remains unchanged.\n'
            if false:
                assert '{1n == 1n : Nat}' in laws
                laws = laws.replace('{1n == 1n : Nat}', '{1n == 2n : Nat}')
            (repo/'LAWS.bend').write_text(laws)
            (repo/'PROOF.bend').write_text(proof)
            folder, plan = repo/'build/proofs', at/'plan.json'
            command([PYTHON, '-IB', ROOT/'scripts/proof-shards.py', 'generate', '--root', repo,
                     '--out', folder, '--shards', '2'], at/'generate.log')
            seconds = command([*pc, 'plan', '--root', repo, '--partition', folder,
                               '--plan', plan, '--cache', private_cache], at/'plan.log')
            state = json.loads((at/'plan.log').read_text())
            assert state['law_count'] == state['shards'] == 2
            return at, repo, folder, plan, state, seconds

        def join(at, repo, plan, label, expected):
            elapsed = command([*pc, 'join', '--root', repo, '--plan', plan, '--cache', private_cache],
                              at/(label+'.log'), expected)
            return json.loads((at/(label+'.log')).read_text()), elapsed

        def execute(at, repo, folder, plan, shard, label, expected=0):
            result = folder/'receipts'/(label+'-'+shard+'.json')
            unit = 'bp-proof-cache-'+label+'-'+shard+'-'+str(os.getpid())+'.scope'
            elapsed = command(['systemd-run', '--user', '--scope', '--quiet', '--unit='+unit,
                '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
                PYTHON, ROOT/'scripts/heavy-guard.py', 'proof-shard', '--', *worker_pc, 'run',
                '--root', repo, '--plan', plan, '--shard', shard, '--result', result,
                '--cache', private_cache], at/(label+'-'+shard+'.log'), expected)
            receipt = json.loads(result.read_text())
            decisions = [json.loads(line) for line in (at/(label+'-'+shard+'.log')).read_text().splitlines()
                         if line.startswith('{') and '"proof_executions"' in line]
            assert len(decisions) == 1 and decisions[0]['proof_executions'] == 1
            assert receipt['actual_bend_flock_held'] is True and receipt['inputs_unchanged'] is True
            assert 0 < receipt['memory_max_bytes'] <= 3700000000
            assert receipt['swap_before'] == receipt['swap_after'] == {'pswpin': 0, 'pswpout': 0}
            assert receipt['manifest_sha256'] == sha(folder/'manifest.json')
            return receipt, elapsed

        at, repo, folder, plan, state, plan_time = snapshot('cold')
        assert len(state['miss']) == 2 and state['hit'] == []
        _, _ = join(at, repo, plan, 'before', 1)
        receipts, cold_time = [], 0
        for index, shard in enumerate(state['miss']):
            receipt, elapsed = execute(at, repo, folder, plan, shard, 'cold')
            receipts.append(receipt); cold_time += elapsed
            if index == 0:
                partial, _ = join(at, repo, plan, 'partial', 1)
                assert len(partial['miss']) == 1
        full, join_time = join(at, repo, plan, 'complete', 0)
        assert len(full['hit']) == 2
        observations.append({'phase': 'cold', 'proof_executions': 2, 'plan_seconds': plan_time,
                             'worker_scope_seconds': cold_time, 'join_seconds': join_time,
                             'peak_worker_rss_kib': max(r['peak_rss_kib'] for r in receipts)})
        warm, warm_time = join(at, repo, plan, 'warm', 0)
        assert warm['miss'] == []
        observations.append({'phase': 'warm', 'proof_executions': 0, 'join_seconds': warm_time})

        at2, repo2, folder2, plan2, state2, _ = snapshot('changed', changed=True)
        assert len(state2['hit']) == len(state2['miss']) == 1
        receipt, elapsed = execute(at2, repo2, folder2, plan2, state2['miss'][0], 'changed')
        _, changed_join = join(at2, repo2, plan2, 'complete', 0)
        observations.append({'phase': 'one-proof-comment-edit', 'proof_executions': 1,
                             'unaffected_cached_shards': 1, 'worker_scope_seconds': elapsed,
                             'join_seconds': changed_join, 'peak_worker_rss_kib': receipt['peak_rss_kib']})

        at3, repo3, folder3, plan3, state3, _ = snapshot('false', changed=True, false=True)
        assert len(state3['hit']) == len(state3['miss']) == 1
        command([*pc, 'join', '--root', repo3, '--plan', plan2, '--cache', private_cache], at3/'stale.log', 2)
        receipt, elapsed = execute(at3, repo3, folder3, plan3, state3['miss'][0], 'false', 1)
        assert receipt['state'] == 'failed' and receipt['exit'] != 0
        assert '- expected :' in receipt['output'] and '- observed :' in receipt['output'] and 'Location:' in receipt['output']
        assert "expected : ':'" not in receipt['output'], 'parse failure is not a false-theorem control'
        incomplete, _ = join(at3, repo3, plan3, 'refused', 1)
        assert len(incomplete['miss']) == 1
        observations.append({'phase': 'false-theorem', 'proof_executions': 1,
                             'false_theorem_rejected': True, 'join_refused': True,
                             'worker_scope_seconds': elapsed, 'peak_worker_rss_kib': receipt['peak_rss_kib']})

        # Tamper a real cache entry, not a caller-declared pass. Restore only the
        # exact previously authenticated bytes; no proof is rerun for this test.
        entries = sorted(Path(private_cache).glob('*/*.json'))
        assert len(entries) == 3
        first_key = json.loads(plan.read_text())['keys']['000']
        entry = Path(private_cache)/first_key[:2]/(first_key+'.json')
        saved = entry.read_bytes()
        corrupted = json.loads(saved); corrupted['mac'] = '0'*64
        entry.write_text(json.dumps(corrupted))
        _, _ = join(at, repo, plan, 'bad-mac', 1)
        entry.write_bytes(saved)
        _, _ = join(at, repo, plan, 'restored-exact-entry', 0)
        missing = json.loads(plan.read_text()); missing['keys'].pop('000')
        (at/'dropped-plan.json').write_text(json.dumps(missing))
        command([*pc, 'join', '--root', repo, '--plan', at/'dropped-plan.json', '--cache', private_cache],
                at/'dropped.log', 2)
        observations.append({'phase': 'skip-controls', 'proof_executions': 0,
                             'tampered_mac_refused': True, 'dropped_key_refused': True,
                             'exact_recorded_entry_restored': True})
        # Never copy the private MAC key into artifacts or a shared cache.
        assert not (OUT/'.mac-key').exists()
    for name, digest in identity['files'].items():
        assert sha(ROOT/name) == digest, name
    assert packet.toolchain() == identity['toolchain'], 'compiler/library identity changed'
    assert events()['oom'] == events()['oom_kill'] == 0
    summary = {'kind': 'real stock-Bend two-law fixture, no production proof qualification',
               'observations': observations, 'toolchain': identity['toolchain'],
               'input_identity_sha256': sha(LAB/'IDENTITY.json'),
               'controller_memory_max_bytes': 536870912, 'controller_swap_max_bytes': 0,
               'controller_memory_peak_bytes': int((directory/'memory.peak').read_text()),
               'controller_events': events(), 'worker_cap_bytes': 3700000000,
               'combined_cap_bytes': 3700000000+536870912,
               'private_mac_key_exported': False, 'inputs_unchanged': True,
               'production_gate_qualified': False, 'full_proof_run': False, 'state': 'passed'}
    (OUT/'SUMMARY.json').write_text(json.dumps(summary, indent=2, sort_keys=True)+'\n')
    print(json.dumps(summary, sort_keys=True))


if __name__ == '__main__':
    main()
