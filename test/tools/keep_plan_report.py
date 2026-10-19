#!/usr/bin/env python3
"""Require all native Keep.step plan samples; timing observations are not budgets."""
import csv
import json
import math
from pathlib import Path
import sys


def summarize(text):
    samples = {}
    for row in csv.reader(text.splitlines()):
        if not row or row[0] != 'sample':
            continue
        if len(row) != 6:
            raise ValueError(f'expected sample,index,old_us,new_us,old_tiles,new_tiles: {row}')
        index, old_us, new_us, old_tiles, new_tiles = map(int, row[1:])
        if not 0 <= index < 100 or min(old_us, new_us, old_tiles, new_tiles) < 0:
            raise ValueError(f'out-of-range or negative keep-plan sample: {row}')
        if index in samples:
            raise ValueError(f'duplicate keep-plan sample index: {index}')
        if old_tiles != new_tiles:
            raise ValueError(f'reference/current plan counts differ at sample {index}: {old_tiles} != {new_tiles}')
        samples[index] = (old_us, new_us, old_tiles)
    if set(samples) != set(range(100)):
        raise ValueError(f'expected 100 uniquely indexed keep-plan samples; got {len(samples)}')
    def stats(column):
        values = sorted(row[column] for row in samples.values())
        return {'p50_us': values[math.ceil(len(values)*.50)-1],
                'p99_us': values[math.ceil(len(values)*.99)-1], 'max_us': values[-1]}
    return {'scope': 'Native Keep.step whole-plan correctness and raw timing observations; whole-app budgets require separate measurements.',
            'n': 100, 'indices': sorted(samples), 'equal_plan_counts': True,
            'reference': stats(0), 'current': stats(1),
            'tile_counts': [{'index': i, 'count': samples[i][2]} for i in sorted(samples)]}


if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('usage: keep_plan_report.py NATIVE_PLAN.log OUTPUT.json')
    result = summarize(Path(sys.argv[1]).read_text())
    Path(sys.argv[2]).write_text(json.dumps(result, indent=2)+'\n')
    print('100 complete native keep-plan samples summarized')
