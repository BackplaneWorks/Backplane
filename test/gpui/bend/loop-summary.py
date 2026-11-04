#!/usr/bin/env python3
"""loop-summary.py DIR: per-100-iteration windows of ms and RSS from loop.bend output."""
import re, sys, glob, statistics, os
for f in sorted(glob.glob(sys.argv[1] + '/*.[12].txt')):
    t = open(f).read()
    ms = [int(m) for m in re.findall(r'^it \d+ ms (\d+)', t, re.M)]
    rss = [int(m) * 4 / 1024 for m in re.findall(r'rss_pages (\d+)', t)][1:]  # MiB (4 KiB pages)
    dig = set(re.findall(r'digest (\d+)', t))
    err = open(f.replace('.txt', '.err')).read().strip().splitlines()[-1]
    print(f"== {os.path.basename(f)}: {len(ms)} iterations, {len(dig)} distinct digest, {err}")
    print(" window        median_ms   p95_ms   RSS_MiB(end of window)")
    for a in range(0, len(ms), 200):
        w = sorted(ms[a:a+200]); p95 = w[int(0.95 * (len(w) - 1))]
        print(f" {a+1:5}-{min(a+200,len(ms)):5}   {statistics.median(w):9.1f} {p95:8d}   {rss[min(a+199,len(rss)-1)]:8.1f}")
    # least-squares slope of ms and RSS per 1000 iterations
    n = len(ms); xs = range(n); mx = (n-1)/2
    sl = lambda ys: sum((x-mx)*(y-sum(ys)/n) for x, y in zip(xs, ys)) / sum((x-mx)**2 for x in xs) * 1000
    print(f" slope: {sl(ms):+.2f} ms per 1000 iterations; {sl(rss[:n]):+.2f} MiB per 1000 iterations; first-100 median {statistics.median(ms[:100])} ms, last-100 median {statistics.median(ms[-100:])} ms; max RSS {max(rss):.1f} MiB")
