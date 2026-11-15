#!/usr/bin/env python3
"""Tracking lag from sampler CSVs: t_ms, host x y w h, follower x y w h, above(0/1).
The follower should sit at host.x+200, host.y+60, host.w-220, host.h-80 (CG coords, top-left origin).
Lag: for each sample where the follower is off, how long ago the host was where the follower's position says."""
import sys, statistics as st
for path in sys.argv[1:]:
    rows = [list(map(float, l.split(','))) for l in open(path) if l.strip() and not l.startswith('t')]
    rows = [r for r in rows if r[3] > 0 and r[7] > 0]
    if not rows: print(path, 'no samples with both windows'); continue
    lag, off, wlag, above = [], 0, [], sum(r[9] for r in rows)
    hx = [(r[0], r[1]) for r in rows]; hw = [(r[0], r[3]) for r in rows]
    def since(series, i, want):
        for j in range(i, -1, -1):
            if abs(series[j][1] - want) < 1: return series[i][0] - series[j][0]
        return None
    for i, r in enumerate(rows):
        if abs(r[5] - (r[1] + 200)) >= 1:
            off += 1; d = since(hx, i, r[5] - 200)
            if d is not None: lag.append(d)
        if abs(r[7] - (r[3] - 220)) >= 1:
            d = since(hw, i, r[7] + 220)
            if d is not None: wlag.append(d)
    q = lambda v, p: sorted(v)[min(len(v) - 1, int(p * len(v)))] if v else float('nan')
    print(f"{path}: {len(rows)} samples over {rows[-1][0]-rows[0][0]:.0f} ms; position off in {off} ({100*off/len(rows):.0f}%); "
          f"move lag ms p50 {q(lag,.5):.1f} p95 {q(lag,.95):.1f} max {max(lag) if lag else float('nan'):.1f}; "
          f"resize lag ms p50 {q(wlag,.5):.1f} p95 {q(wlag,.95):.1f}; directly above host {100*above/len(rows):.1f}% of samples")
    # the stretches where it was not directly above the host
    gaps, start = [], None
    for r in rows:
        if not r[9] and start is None: start = r[0]
        if r[9] and start is not None: gaps.append((start, r[0] - start)); start = None
    if start is not None: gaps.append((start, rows[-1][0] - start))
    print("  not above:", ", ".join(f"at {s:.0f} ms for {d:.0f} ms" for s, d in gaps[:12]) or "never")
