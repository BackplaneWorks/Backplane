#!/usr/bin/env python3
"""Thread open timings from the app's own "Backplane open cached=<bool> ms=<n>" log lines (touch to the thread on screen). usage: ios-open-table.py FILE"""
import re, sys, statistics as st
c, f = [], []
for l in open(sys.argv[1]):
    m = re.search(r'Backplane open cached=(true|false) ms=([0-9.]+)', l)
    if m: (c if m[1] == 'true' else f).append(float(m[2]))
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(len(xs) * p))]
for name, xs in (('with a saved copy', c), ('without one', f)):
    print(f'{name}: n={len(xs)}' + (f' p50={st.median(xs):.0f} p95={pct(xs, .95):.0f} max={max(xs):.0f} ms' if xs else ''))
