#!/usr/bin/env python3
"""Warm-plan consumption controls; mocked toolchain/test-only passes, no Bend."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import time


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    old = subprocess.check_output(['git', 'show', args.before+':scripts/content-cache.py'], cwd=repo)
    fixture = load(repo/'test/tools/content_cache_test.py', 'snapshot_measure_fixture')
    results = {}
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory)/'before.py'
        path.write_bytes(old)
        variants = {'before': load(path, 'before_cache'),
                    'after': load(repo/'scripts/content-cache.py', 'after_cache')}
        variants['before'].ROOT = repo
        for name, mod in variants.items():
            rows = {}
            fixture.cc = mod
            for mutation in ('unchanged', 'dependency_edit', 'new_module', 'deleted_pass'):
                case = fixture.Base()
                case.setUp()
                try:
                    units = mod.check_units(case.root)
                    for key, body in units.values():
                        mod._record(key, body, case.cache)  # test-only local pass fixtures
                    planned = {'schema': 1, 'modules': sorted(units),
                               'keys': {n: k for n, (k, _) in units.items()}, 'miss': []}
                    plan = case.root.parent/'plan.json'
                    plan.write_text(json.dumps(planned))
                    if mutation == 'dependency_edit':
                        case.write('src/leaf.bend', 'invalid changed theorem\n')
                    elif mutation == 'new_module':
                        case.write('src/new.bend', 'import Base\ndef new() -> U32:\n  0\n')
                    elif mutation == 'deleted_pass':
                        mod.entry_path(case.cache, units['src/other.bend'][0]).unlink()
                    start = time.perf_counter()
                    try:
                        result = mod.run_check(case.root, plan, case.cache)
                        rows[mutation] = {'outcome': 'returned', 'exit': result}
                    except mod.CacheError as error:
                        rows[mutation] = {'outcome': 'refused', 'reason': str(error)}
                    rows[mutation]['seconds'] = time.perf_counter()-start
                finally:
                    case.doCleanups()
            results[name] = rows
    report = {'kind': __doc__, 'before_revision': args.before,
              'before_helper_sha256': hashlib.sha256(old).hexdigest(),
              'after_helper_sha256': hashlib.sha256((repo/'scripts/content-cache.py').read_bytes()).hexdigest(),
              'module_count': 4, 'results': results, 'paid_cost_dollars': 0,
              'limits': 'synthetic4-module repo, mocked toolchain and test-only authenticated passes; no real theorem execution or CI timing; n=1 per case'}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n')
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
