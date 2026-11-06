#!/usr/bin/env python3
"""Thread switch timings from BPPerf records (op 6 = navigation): the cached page on screen (stage 8, outcome 3), the fresh page (stage 8, outcome 0),
the model ready (stage 7). usage: ios-switch-table.py LOG [FIRST_OPENS]  (ms; the first three navigations are first opens: no cache)"""
import re, sys, statistics as st
FIRST = int(sys.argv[2]) if len(sys.argv) > 2 else 3
nav = {}  # started ns -> {stage: (ms, outcome)}
order = []
for l in open(sys.argv[1]):
    m = re.search(r'BPPerf (\d+) (\d+) (\d+) (\d+) (\d+) (\d+)', l)
    if not m or m[2] != '6': continue
    stage, started, ended, outcome = int(m[1]), int(m[3]), int(m[4]), int(m[6])
    if started not in nav: nav[started] = {}; order.append(started)
    nav[started].setdefault(stage, []).append(((ended - started) / 1e6, outcome))
rows = [(i, nav[s]) for i, s in enumerate(sorted(nav))]
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(len(xs) * p))] if xs else None
cached, fresh, ready = [], [], []
for i, d in rows:
    for ms, o in d.get(8, []):
        if i < FIRST: continue      # first opens
        (cached if o == 3 else fresh).append(ms)
    for ms, o in d.get(7, []):
        if i >= FIRST: ready.append(ms)
print(f'navigations seen: {len(rows)}; after the first {FIRST} opens:')
for name, xs in (('display, cached page (outcome 3)', cached), ('display, fresh page (outcome 0)', fresh), ('model ready', ready)):
    print(f'  {name}: n={len(xs)}' + (f' p50={st.median(xs):.0f} p95={pct(xs, .95):.0f} max={max(xs):.0f} ms' if xs else ''))
