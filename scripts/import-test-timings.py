#!/usr/bin/env python3
"""Import scheduling estimates from a complete set of saved nightly shard logs.

This imports timings, never test passes. Failed/untimed programs stay unknown
and still run. All test names and the old shard assignment must match the
source revision. No Bend or app execution occurs.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('shard', ROOT / 'scripts/test-shard.py')
shard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shard)
HEADER = re.compile(r'^\d{4}-\d\d-\d\dT\S+Z (test/[^ /]+_test\.bend): (.*)$')
OK = re.compile(r'^ok \(([\d.]+)s, ([\d.]+) GB\)$')


def parse_log(text, revision):
    if not re.search(r'^\S+Z ' + re.escape(revision) + r'$', text, re.M):
        raise ValueError('log lacks exact checked-out revision')
    rows = {}
    for line in text.splitlines():
        match = HEADER.fullmatch(line)
        if not match:
            continue
        name, verdict = match.groups()
        if name in rows:
            raise ValueError('duplicate verdict: ' + name)
        timing = OK.fullmatch(verdict)
        row = {'verdict': verdict, 'seconds': None, 'peak_gib': None}
        if timing:
            seconds, peak = map(float, timing.groups())
            if not all(math.isfinite(x) and x >= 0 for x in (seconds, peak)):
                raise ValueError('invalid timing: ' + name)
            row.update(seconds=seconds, peak_gib=peak)
        rows[name] = row
    return rows


def combine(logs, revision, expected, old_times):
    groups = [parse_log(log, revision) for log in logs]
    rows = {}
    for group in groups:
        if rows.keys() & group.keys():
            raise ValueError('same test appears in multiple shards')
        rows.update(group)
    if set(rows) != set(expected):
        raise ValueError('logs do not cover exact source test inventory')
    old_groups = shard.partition(sorted(expected), old_times, len(groups))
    if any(set(actual) != set(wanted) for actual, wanted in zip(groups, old_groups)):
        raise ValueError('logs do not match source shard assignment/order')
    timed = {name: row['seconds'] for name, row in rows.items() if row['seconds'] is not None}
    if not timed:
        raise ValueError('no passing test timings')
    new_groups = shard.partition(sorted(expected), timed, len(groups))
    # Unknown/failed programs contribute no invented duration. These totals are
    # recorded-work models, not a prediction of complete execution wall time.
    totals = lambda partition: [round(sum(timed.get(t, 0) for t in group), 3)
                                for group in partition]
    return timed, {'inventory_count': len(rows), 'timed_count': len(timed),
                   'untimed_or_failed': {n: r for n, r in rows.items() if n not in timed},
                   'old_recorded_work_seconds': totals(old_groups),
                   'new_recorded_work_seconds': totals(new_groups),
                   'peak_gib_max': max(r['peak_gib'] or 0 for r in rows.values())}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--run-url', required=True)
    parser.add_argument('--log', type=Path, action='append', required=True,
                        help='Repeat in 1-based shard order')
    parser.add_argument('--out', type=Path, default=ROOT / 'scripts/test-timings.txt')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.unlink(missing_ok=True)
    if not re.fullmatch('[0-9a-f]{40}', args.revision):
        parser.error('revision must be a complete SHA1')
    expected = {p for p in git('ls-tree', '-r', '--name-only', args.revision, 'test').splitlines()
                if re.fullmatch(r'test/[^/]+_test\.bend', p)} - {'test/cache_identity_before_test.bend'}
    with tempfile.TemporaryDirectory() as tmp:
        previous = Path(tmp) / 'timings'
        previous.write_text(git('show', args.revision + ':scripts/test-timings.txt'))
        old_times = shard.load_timings(previous)
    timed, report = combine([p.read_text() for p in args.log], args.revision, expected, old_times)
    header = (f'# Scheduling estimates, not cached test passes.\n# Source: {args.run_url}\n'
              f'# Revision: {args.revision}; {len(timed)}/{len(expected)} programs timed.\n'
              '# Failed/untimed/new programs still run with the sharder fallback.\n'
              '# Refresh from complete saved logs with scripts/import-test-timings.py,\n'
              '# or explicitly run the full serial scripts/record-test-timings.sh.\n')
    content = header + ''.join(f'{seconds:.2f} {name}\n' for name, seconds in
                                sorted(timed.items(), key=lambda x: (-x[1], x[0])))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.out.with_suffix('.tmp')
    temporary.write_text(content)
    temporary.replace(args.out)
    report.update(revision=args.revision, run_url=args.run_url,
                  log_sha256={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in args.log},
                  importer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  output_sha256=hashlib.sha256(content.encode()).hexdigest(),
                  scope='Advisory scheduling data only; recorded-work model omits unknown durations; no CI wall-time or pass claim')
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
