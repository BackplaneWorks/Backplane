#!/usr/bin/env python3
# results.py BASE_RUN TIP_RUN [--metric=px] > RESULTS-tables.md : the at-a-glance table and per-cell tables for the
# repo's RESULTS.md (compare.py's output, cells in a fixed order, the worst 10 after, regressions).
import subprocess, sys, os
here = os.path.dirname(os.path.abspath(__file__))
args = sys.argv[1:]
order = ['R-fast-web', 'R-fast-web-rtt30', 'R-fast-web-rtt100', 'R-fast-web-rtt250', 'R-slow-web', 'R-vslow-web',
         'R-fast-desk', 'R-fast-phone', 'S-fast-web', 'X-fast-web', 'X-slow-web', 'X-vslow-web-rtt100']
base, tip = [a for a in args if not a.startswith('--')][:2]
metric = [a for a in args if a.startswith('--metric=')]
cells = [c for c in order if os.path.exists(f'{base}/{c}/summary.json') and os.path.exists(f'{tip}/{c}/summary.json')]
out = subprocess.run(['python3', f'{here}/compare.py', base, tip, *cells, *metric], capture_output=True, text=True).stdout
print(out)
