#!/usr/bin/env python3
"""Summarize instrumented Bend checkpoints, without running a compiler.

Checkpoint RSS and cumulative high-water marks are not isolated phase peaks.
The census-enabled profile also includes expensive heap-walk overhead.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

STAGES = [
    ('load/parse', 'load:start', 'load:end'),
    ('type-check', 'check:start', 'check:end'),
    ('compiler analysis', 'analysis:start', 'analysis:end'),
    ('JS functions', 'analysis:end', 'functions:end'),
    ('output join', 'join:start', 'join:end'),
]


def summarize(rows, timed):
    if not rows:
        raise ValueError('empty compiler profile')
    previous = -1
    events = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('phase'), str):
            raise ValueError('invalid checkpoint')
        for field in ['seconds', 'rss_bytes', 'max_rss_kib']:
            value = row.get(field)
            if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
                raise ValueError('invalid checkpoint ' + field)
        if row['seconds'] < previous:
            raise ValueError('nonmonotonic checkpoint time')
        previous = row['seconds']
        if not row['phase'].startswith('jsdef:'):
            if row['phase'] in events:
                raise ValueError('multiple compilations need separate profiles')
            events[row['phase']] = row
    stages = []
    for name, begin, end in STAGES:
        if begin not in events or end not in events:
            raise ValueError('missing completed phase: ' + name)
        a, b = events[begin], events[end]
        if b['seconds'] < a['seconds']:
            raise ValueError('reversed phase: ' + name)
        stages.append({'phase': name,
                       'checkpoint_interval_seconds': b['seconds'] - a['seconds'],
                       'end_rss_bytes': b['rss_bytes'],
                       'cumulative_peak_rss_kib': b['max_rss_kib']})
    peak = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', timed)
    exit_code = re.search(r'Exit status:\s*(\d+)', timed)
    if not peak or not exit_code or int(exit_code[1]) != 0:
        raise ValueError('missing or unsuccessful GNU time receipt')
    high_water = int(peak[1])
    if high_water < max(r['max_rss_kib'] for r in rows):
        raise ValueError('process peak contradicts checkpoints')
    samples = [r for r in rows if r['phase'].startswith('jsdef:')]
    top = sorted(samples, key=lambda r: r['rss_bytes'], reverse=True)[:8]
    return {'kind': 'compiler-memory-measurement', 'stages': stages,
            'process_peak_rss_kib': high_water,
            'under_6_decimal_GB': high_water * 1024 < 6_000_000_000,
            'largest_definition_checkpoints': top,
            'scope': 'Diagnostic only; cumulative/checkpoint RSS, not isolated phase allocations or a gate pass. Heap census overhead is included in intervals.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--time', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.unlink(missing_ok=True)
    rows = [json.loads(line) for line in args.profile.read_text().splitlines()]
    result = summarize(rows, args.time.read_text())
    result['inputs_sha256'] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                               for p in [args.profile, args.time]}
    result['reporter_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result[k] for k in ['process_peak_rss_kib', 'under_6_decimal_GB']}))


if __name__ == '__main__':
    main()
