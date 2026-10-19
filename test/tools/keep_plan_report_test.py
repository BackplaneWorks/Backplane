#!/usr/bin/env python3
"""Pure parser controls; no Bend, native compilation or benchmark execution."""
import sys
sys.dont_write_bytecode = True  # Keep source identity equal across CI jobs.
from keep_plan_report import summarize

rows = [f'sample,{i},{i+10},{i+2},17,17' for i in range(100)]
result = summarize('\n'.join(['ok whole plan exact image', *reversed(rows)]))
assert result['n'] == 100 and result['indices'] == list(range(100))
assert result['reference'] == {'p50_us':59, 'p99_us':108, 'max_us':109}
assert result['current'] == {'p50_us':51, 'p99_us':100, 'max_us':101}
assert result['equal_plan_counts'] is True
print('ok 100 unordered complete samples retain exact indices/counts and raw timing summaries')
for label, bad in [
    ('missing', rows[:-1]), ('duplicate', [*rows, rows[0]]),
    ('wrong index', [rows[0].replace('sample,0,','sample,100,'), *rows[1:]]),
    ('negative timing', ['sample,0,-1,2,17,17', *rows[1:]]),
    ('negative count', ['sample,0,1,2,-1,-1', *rows[1:]]),
    ('unequal count', ['sample,0,1,2,17,18', *rows[1:]]),
    ('malformed', ['sample,0,1,2,17', *rows[1:]]),
    ('nonnumeric', ['sample,0,unknown,2,17,17', *rows[1:]]),
]:
    try:
        summarize('\n'.join(bad))
    except ValueError:
        print('ok '+label+' sample cannot claim complete equal plans')
    else:
        raise AssertionError(label+' unexpectedly accepted')
