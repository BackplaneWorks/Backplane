#!/usr/bin/env python3
"""Start-up table from ios-boot-mac.sh's log: ms from process start; every run must have logged model-ready. usage: ios-boot-table.py LOG"""
import re, sys, statistics as st
# a block's log window (log show --last) can hold the launches before it: only the last launch's marks count, those from the last
# init-enter on (every mark is relative to its own process, so an older launch's would be taken for this one's)
blocks = {}; cur = None
for l in open(sys.argv[1]):
    if l.startswith('=='): cur = l[3:].strip(); blocks[cur] = []; continue
    if cur: blocks[cur].append(l)
d = {}
for k, ls in blocks.items():
    last = max([i for i, l in enumerate(ls) if 'boot init-enter' in l], default=0)
    d[k] = {}
    for l in ls[last:]:
        m = re.search(r'boot (\S+) (\d+)', l)
        if m: d[k].setdefault(m[1], int(m[2]))
cols = ['init-enter', 'first-display', 'first-display-fresh', 'model-ready']
print('%-11s' % '', *['%10s' % c[:10] for c in cols])
bad = []
for k, v in d.items():
    print('%-11s' % k, *['%10s' % v.get(c, '-') for c in cols])
    if k != 'prime' and 'model-ready' not in v: bad.append(k)
for kind in ('fresh', 'relaunch'):
    xs = sorted(v['model-ready'] for k, v in d.items() if k.startswith(kind) and 'model-ready' in v)
    n = sum(1 for k in d if k.startswith(kind))
    if xs: print(f'{kind}: model-ready from process start: n={len(xs)}/{n} median={st.median(xs)} min={xs[0]} max={xs[-1]} p95~{xs[min(len(xs)-1, int(len(xs)*0.95))]}')
if bad: print('FAIL: no model-ready in', bad); sys.exit(1)
