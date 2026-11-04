#!/usr/bin/env python3
"""summarize.py ARTIFACT_DIR KERNEL: median/p95 of every 'ms' line over all
fresh processes per variant, peak RSS, digest agreement, binary sizes."""
import glob, os, re, statistics, sys
d, k = sys.argv[1], sys.argv[2]
print(open(f'{d}/machine.txt').read().strip())
rows, digests = {}, {}
for f in sorted(glob.glob(f'{d}/run.*.txt')):
    v = re.match(r'run\.(.+)\.\d\.txt', os.path.basename(f)).group(1)
    t = open(f).read()
    r = rows.setdefault(v, {'ms': [], 'rss': [], 'n': 0})
    r['ms'] += [float(x) for x in re.findall(r'^ms ([\d.]+)', t, re.M)]
    r['rss'] += [int(x) for x in re.findall(r'rss_kib (\d+)', t)]
    r['n'] += 1
    digests.setdefault(v, set()).update(re.findall(r'^digest (.*)$', t, re.M))
ref = None
print(f"{'variant':20} {'procs':>5} {'samples':>7} {'median ms':>10} {'p95 ms':>9} {'min ms':>9} {'peak RSS MiB':>12}  digest")
for v, r in rows.items():
    ms = sorted(r['ms'])
    if not ms: print(f'{v:20} no samples (failed)'); continue
    p95 = ms[min(len(ms) - 1, int(round(0.95 * (len(ms) - 1))))]
    ds = digests[v]
    ref = ref or ds
    print(f"{v:20} {r['n']:5} {len(ms):7} {statistics.median(ms):10.2f} {p95:9.2f} {ms[0]:9.2f} {max(r['rss'])/1024:12.1f}  {'OK' if ds == ref else 'MISMATCH'} ({len(ds)} distinct)")
for f in glob.glob(f'{d}/{k}.[cj]*') + glob.glob(f'{d}/{k}.O[23]'):
    print(f'size {os.path.basename(f)}: {os.path.getsize(f)} bytes')
print(open(f'{d}/build.txt').read().split('ls -l')[0].strip())
