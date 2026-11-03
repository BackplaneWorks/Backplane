#!/usr/bin/env python3
"""Profile real repository cache metadata only; never invoke Bend or seed passes."""
import hashlib
import importlib.util
import json
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).with_name('REAL-PLAN.json')
CAP = 536870912


def group():
    rel = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    path = Path('/sys/fs/cgroup')/rel
    memory = (path/'memory.max').read_text().strip()
    swap = (path/'memory.swap.max').read_text().strip()
    if memory != str(CAP) or swap != '0':
        raise RuntimeError('metadata measurement needs its own512MiB/no-swap scope')
    return path


def main():
    cg = group()
    events = lambda: {k: int(v) for k, v in
                      (line.split() for line in (cg/'memory.events').read_text().splitlines())}
    report = {'kind': __doc__, 'memory_max_bytes': CAP, 'swap_max_bytes': 0,
              'events_before': events(), 'paid_cost_dollars': 0,
              'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'cache_helper_sha256': hashlib.sha256((ROOT/'scripts/content-cache.py').read_bytes()).hexdigest()}
    success = False
    try:
        spec = importlib.util.spec_from_file_location('real_plan_cache', ROOT/'scripts/content-cache.py')
        cc = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cc)
        start = time.perf_counter()
        cc.reset_tool_identity()
        units = cc.check_units(ROOT)
        report['initial_key_plan_seconds'] = time.perf_counter()-start
        report['modules'] = len(units)
        report['cacheable_modules'] = sum(key is not None for key, _ in units.values())
        report['uncacheable_modules'] = {n: body for n, (key, body) in units.items() if key is None}
        report['toolchain_identity_sha256'] = cc.sha(cc.canonical(cc.tool_identity()['toolchain']))
        report['module_keyset_sha256'] = cc.sha(cc.canonical({n: k for n, (k, _) in units.items()}))
        planned = {'schema': 1, 'modules': sorted(units),
                   'keys': {n: k for n, (k, _) in units.items()}, 'miss': sorted(units)}
        # All units are declared misses. This only validates metadata; it does
        # not run_check, create a MAC key/entry, or claim a check succeeded.
        with tempfile.TemporaryDirectory(prefix='bp-empty-plan-cache-') as empty_cache:
            start = time.perf_counter()
            cc.validate_check_plan(ROOT, planned, Path(empty_cache))
            report['complete_revalidation_seconds'] = time.perf_counter()-start
        report['inputs_unchanged'] = True
        success = True
    except Exception as error:
        report['error'] = type(error).__name__+': '+str(error)
    report['events_after'] = events()
    report['aggregate_peak_bytes'] = int((cg/'memory.peak').read_text())
    report['peak_rss_kib'] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    report['success'] = success
    report['qualification'] = False
    report['scope'] = 'Actual tool/source metadata; no Bend executable, proof, pass recording or CI gate execution'
    OUT.write_text(json.dumps(report, sort_keys=True, indent=2)+'\n')
    print(json.dumps(report, sort_keys=True))
    return 0 if success else 1


if __name__ == '__main__':
    sys.exit(main())
