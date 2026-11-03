#!/usr/bin/env python3
"""Two-law fixture key retention across umasks; mocked tools, no Bend execution."""
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
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--before', required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    repo = Path(__file__).resolve().parents[3]
    old = subprocess.check_output(['git', 'show', args.before+':scripts/content-cache.py'], cwd=repo)
    fixture = load(repo/'test/tools/content_cache_test.py', 'mode_fixture')
    records = {}
    with tempfile.TemporaryDirectory() as temporary:
        path = Path(temporary)/'before.py'
        path.write_bytes(old)
        variants = {'before': load(path, 'before_cache'),
                    'after': load(repo/'scripts/content-cache.py', 'after_cache')}
        variants['before'].ROOT = repo
        for name, mod in variants.items():
            fixture.cc = mod
            case = fixture.Shards()
            case.setUp()
            try:
                before = case.keys()
                for source in case.root.rglob('*'):
                    if source.is_file():
                        source.chmod(0o600)
                start = time.perf_counter()
                after = case.keys()  # same verified partition; no regeneration
                records[name] = {'key_plan_seconds': time.perf_counter()-start,
                                 'retained_keys': sum(before[n] == k for n, k in after.items()),
                                 'total_shards': len(after)}
            finally:
                case.doCleanups()
    report = {'kind': __doc__, 'before_revision': args.before,
              'before_helper_sha256': hashlib.sha256(old).hexdigest(),
              'after_helper_sha256': hashlib.sha256((repo/'scripts/content-cache.py').read_bytes()).hexdigest(),
              'mutation': 'chmod all fixture source and generated files from 0644 to0600',
              'results': records, 'paid_cost_dollars': 0,
              'scope': 'real canonical partition/key code; synthetic2-law fixture and mocked toolchain; no proofs or pass recording; n=1 each'}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n')
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
