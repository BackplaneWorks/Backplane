#!/usr/bin/env python3
"""Option (b) numbers from sampler CSVs (see sampler.m for the columns).
The follower should sit at host.x+200, host.y+60, host.w-220, host.h-80 (CG coords, top-left origin).
Lag: for a sample where the follower is off, how long ago the host was where the follower's position says."""
import sys, collections
def q(v, p): return sorted(v)[min(len(v) - 1, int(p * len(v)))] if v else float('nan')
for path in sys.argv[1:]:
    rows = []
    for l in open(path):
        f = l.strip().split(',')
        if len(f) >= 13: rows.append([float(x) for x in f[:12]] + [f[12]])
    both = [r for r in rows if r[3] > 0 and r[7] > 0]
    if not both: print(path, 'no samples with both windows'); continue
    lag, wlag, off = [], [], 0
    def since(i, col, want):
        # signed: positive when the follower shows where the host was earlier, negative when it is already where the
        # host's reported bounds get to later (push mode: the host sends its own frame before the window server reports it).
        # Only within 250 ms either way: the host sweeps back and forth, so a farther match is an earlier sweep.
        best = None
        for j in range(len(both)):
            dt = both[i][0] - both[j][0]
            if abs(dt) <= 250 and abs(both[j][col] - want) < 1 and (best is None or abs(dt) < abs(best)): best = dt
        return best
    for i, r in enumerate(both):
        if abs(r[5] - (r[1] + 200)) >= 1:
            off += 1; d = since(i, 1, r[5] - 200)
            if d is not None: lag.append(d)
        if abs(r[7] - (r[3] - 220)) >= 1:
            d = since(i, 3, r[7] + 220)
            if d is not None: wlag.append(d)
    above = sum(r[9] for r in both)
    print(f"{path}: {len(rows)} samples over {rows[-1][0]-rows[0][0]:.0f} ms; follower off its place in {100*off/len(both):.0f}% of samples; "
          f"move lag ms (signed, + behind) p5 {q(lag,.05):.1f} p50 {q(lag,.5):.1f} p95 {q(lag,.95):.1f}, |lag| p95 {q([abs(x) for x in lag],.95):.1f}, unmatched {off-len(lag)}; "
          f"resize lag ms p50 {q(wlag,.5):.1f} p95 {q(wlag,.95):.1f}; directly above the host in {100*above/len(both):.1f}%")
    gaps, start = [], None
    for r in rows:
        bad = not r[9]
        if bad and start is None: start = r[0]
        if not bad and start is not None: gaps.append((start, r[0] - start, r[12])); start = None
    if start is not None: gaps.append((start, rows[-1][0] - start, rows[-1][12]))
    print("  not directly above the host:", "; ".join(f"at {s:.0f} ms for {d:.0f} ms (stack {o or 'none'})" for s, d, o in gaps[:14]) or "never")
    orders = collections.Counter(r[12] for r in rows if 'C' in r[12])
    if orders: print("  while the other app's window was up, front to back (H host, F follower, C cover):", dict(orders.most_common()))
    hostless = [r[0] for r in rows if r[3] <= 0]
    if hostless: print(f"  host not on this screen's list for {len(hostless)*2} ms (from {hostless[0]:.0f} ms): full screen moves it to its own Space; follower on screen then: {sum(1 for r in rows if r[3] <= 0 and r[10])} of {len(hostless)} samples")
