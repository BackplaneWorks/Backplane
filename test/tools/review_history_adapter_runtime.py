"""Whole actual ReviewHistory/M.State adapter emit/runtime on capped normal Boat.

Uses the merged heavy-boat route. Never changes compile-shard's command
allowlist or requests an application compile. No proof rerun or cache seed.
"""
from collections import Counter
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / 'build/hub-review-adapter-runtime'
PYTHON = str(Path(sys.executable).resolve())
ENTRY = 'test/review_history_adapter_test.bend'
FILES = []


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT / 'scripts' / name)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def scope(cap):
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    maximum = int((group / 'memory.max').read_text())
    assert 0 < maximum <= cap and (group / 'memory.swap.max').read_text().strip() == '0'
    physical = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal:'))
    assert physical <= 8 * 1024 ** 3
    return group


def resources(group):
    return {'cgroup': str(group), 'memory_max_bytes': int((group / 'memory.max').read_text()),
            'swap_max_bytes': int((group / 'memory.swap.max').read_text()),
            'memory_peak_bytes': int((group / 'memory.peak').read_text()),
            'events': {k: int(v) for k, v in (line.split() for line in (group / 'memory.events').read_text().splitlines())}}


def compile_adapter():
    group = scope(3700000000)
    guard = module('heavy-guard.py')
    # Retain native serialization and the original 3 GiB admission floor.
    # The explicit adapter-test command uses heavy-boat's approved capped route;
    # it does not masquerade as a compile-shard command.
    available = guard.available()
    if available < guard.FLOORS['compile-shard']:
        return 75
    lock = open(guard.lock_path(), 'a+')
    try:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        with open(guard.lock_path(), 'a+') as probe:
            try:
                fcntl.flock(probe.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                pass
            else:
                raise AssertionError('native lock was not held against a fresh descriptor')
        environment = {**os.environ, guard.FD_VAR: str(lock.fileno()), 'BEND_NO_TELEMETRY': '1'}
        previous_fd = os.environ.get(guard.FD_VAR)
        os.environ[guard.FD_VAR] = str(lock.fileno())
        assert guard.inherited_lock(guard.lock_path()) is not None
        if previous_fd is None:
            del os.environ[guard.FD_VAR]
        else:
            os.environ[guard.FD_VAR] = previous_fd
        bend = Path(shutil.which('bend')).resolve()
        command = ['scripts/heavy-boat.sh', 'bend', 'test/review_history_adapter_test.bend', '-o', str(OUT / 'review-adapter.js')]
        start = time.monotonic()
        p = subprocess.run(command, capture_output=True, text=True, timeout=900, env=environment, pass_fds=(lock.fileno(),))
        (OUT / 'emit.log').write_text(p.stdout + p.stderr)
        report = {'state': 'passed' if p.returncode == 0 else 'failed', 'exit': p.returncode,
                  'command': command, 'seconds': time.monotonic() - start,
                  'peak_child_rss_kib': resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                  'actual_native_flock_held': True, 'admission_floor_bytes': guard.FLOORS['compile-shard'],
                  'available_bytes_at_admission': available,
                  'bend_path': str(bend), 'bend_sha256': sha(bend), 'resources': resources(group)}
        (OUT / 'compiler.json').write_text(json.dumps(report, indent=2) + '\n')
        return p.returncode
    finally:
        lock.close()


def main():
    global FILES
    group = scope(536870912)
    assert group.name.startswith('bp-review-adapter-runtime-controller-')
    assert not OUT.exists(), 'inspect any existing attempt before another run'
    OUT.mkdir(parents=True)
    previous = json.loads((ROOT / 'build/hub-review-adapter-scope/SUMMARY.json').read_text())
    assert previous['state']=='passed' and previous['full_review_history_typecheck_qualified'] and previous['negative_control_refused']
    assert previous['source_head']==os.environ['REVIEW_ADAPTER_RUNTIME_SOURCE_HEAD']
    FILES = subprocess.check_output([PYTHON,'scripts/bend-closure.py',ENTRY],text=True).splitlines()
    assert FILES==previous['compiled_source_files']
    assert 'src/server/review-history.bend' in FILES and 'src/server/window-file.bend' in FILES
    assert not any(name.startswith(('src/web/','src/mobile/','src/app/')) or name=='src/server/thin-view.bend' for name in FILES)
    for name, digest in previous['source_sha256'].items():
        assert sha(ROOT / name) == digest, name
    parser = module('proof-slice.py')
    laws, proof = parser.Module(ROOT, 'LAWS.bend'), parser.Module(ROOT, 'PROOF.bend')
    for name, digests in previous['canonical_blocks_sha256'].items():
        actual = [hashlib.sha256(laws.by_name[name][0]['text'].encode()).hexdigest(),
                  hashlib.sha256(proof.by_name['Laws.' + name][0]['text'].encode()).hexdigest()]
        assert actual == digests, name
    module('local-phase.py').tools()
    unit = 'bp-review-adapter-runtime-compiler-' + str(os.getpid())
    command = ['systemd-run', '--user', '--scope', '--quiet', '--unit=' + unit,
               '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
               PYTHON, 'test/tools/review_history_adapter_runtime.py', '--compile-adapter']
    p = subprocess.run(command, capture_output=True, text=True, timeout=960)
    (OUT / 'worker.log').write_text(p.stdout + p.stderr)
    assert p.returncode == 0, 'actual adapter compiler route failed: inspect retained worker/emit logs'
    emitted = OUT / 'review-adapter.js'
    assert emitted.is_file()
    p = subprocess.run(['bun', str(emitted)], capture_output=True, text=True, timeout=120)
    output = p.stdout + p.stderr
    (OUT / 'runtime.log').write_text(output)
    contract_path = ROOT / 'test/fixtures/review_history_adapter_runtime.json'
    contract = json.loads(contract_path.read_text())
    lines = [line for line in output.splitlines() if line.strip()]
    checks = [line[3:] for line in lines if line.startswith('ok ')]
    assert p.returncode==contract['require_process_exit'] and 'FAIL' not in output, output
    assert len(checks)==contract['expected_ok_case_count'], output
    assert Counter(checks)==Counter(contract['expected_case_occurrences']), output
    assert lines and lines[-1]==contract['required_terminal_line'], output
    for name, digest in previous['source_sha256'].items():
        assert sha(ROOT / name) == digest, name
    report = {'state': 'passed', 'source_head': os.environ['REVIEW_ADAPTER_RUNTIME_SOURCE_HEAD'],
              'source_sha256': {name: sha(ROOT / name) for name in FILES},
              'tools_sha256': {name: sha(ROOT / name) for name in ['scripts/heavy-boat.sh', 'scripts/heavy-guard.py', 'test/tools/review_history_adapter_runtime.py']},
              'reused_law_receipt_source': previous['source_head'], 'proof_executions': 0,
              'checks': checks, 'runtime_contract_sha256':sha(contract_path), 'expected_case_count':contract['expected_ok_case_count'], 'FAIL_lines_rejected':True, 'terminal_close_observed':True, 'emitted_sha256': sha(emitted), 'bun_sha256': sha(shutil.which('bun')),
              'compiler': json.loads((OUT / 'compiler.json').read_text()), 'runtime_resources': resources(group),
              'actual_M_State_adapter_type_and_runtime_qualified': True, 'thin_view_delegate_typecheck_qualified': False, 'full_server_qualified': False, 'packaged_performance_pass': False}
    assert report['runtime_resources']['events']['oom'] == report['runtime_resources']['events']['oom_kill'] == 0
    assert report['compiler']['resources']['events']['oom'] == report['compiler']['resources']['events']['oom_kill'] == 0
    (OUT / 'SUMMARY.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    if sys.argv[1:] == ['--compile-adapter']:
        sys.exit(compile_adapter())
    assert len(sys.argv) == 1
    main()
