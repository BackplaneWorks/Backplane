#!/usr/bin/env python3
# compare.py BASE_RUN TIP_RUN [CELL...] : before -> after per interaction and cell (p50 / p95, speed-up), share within
# 16 / 50 / 100 ms before and after, layout-shift and late-content counts, the worst 10 after, regressions (> +20 % and
# > +10 ms at p50) flagged. Cells default to those both runs have. Markdown on stdout.
import json, os, sys
args = [a for a in sys.argv[1:] if not a.startswith('--metric=')]
METRIC = next((a.split('=', 1)[1] for a in sys.argv[1:] if a.startswith('--metric=')), 'paint')  # paint (next frame) or px (end of the frame with the change)
base, tip = args[0], args[1]
def load(run):
    out = {}
    for c in os.listdir(run):
        p = f'{run}/{c}/summary.json'
        if os.path.exists(p):
            out[c] = {r['id']: r for r in json.load(open(p))['rows']}
            for r in out[c].values():
                if METRIC != 'paint': r['paint50'], r['paint95'] = r.get(METRIC + '50'), r.get(METRIC + '95')
    return out
B, T = load(base), load(tip)
cells = args[2:] or [c for c in B if c in T]
order = ['R-fast-web', 'R-fast-web-rtt30', 'R-fast-web-rtt100', 'R-fast-web-rtt250', 'R-slow-web', 'R-vslow-web', 'R-fast-desk', 'X-fast-web', 'X-slow-web', 'X-vslow-web-rtt100']
cells.sort(key=lambda c: order.index(c) if c in order else 99)
f = lambda x: '—' if x is None else f'{x:.0f}'
def band(xs, t): xs = [x for x in xs if x is not None]; return f"{sum(x <= t for x in xs)}/{len(xs)}" if xs else '—'
print(f'## Before (base) -> after ({os.path.basename(tip)}), metric {METRIC}\n')
regress = []
print('| cell | n | median p50 before -> after | slowest 80 %: mean p50 before -> after | geo-mean speed-up | within 16 ms | within 50 ms | within 100 ms | shift>0.01 (all) | shift>0.01 (CLS: no recent input) | late>0 |')
print('|---|---:|---|---|---:|---|---|---|---|---|---|')
for c in cells:
    ids = [i for i in B[c] if i in T[c] and B[c][i]['paint50'] is not None and T[c][i]['paint50'] is not None]
    b = [B[c][i]['paint50'] for i in ids]; t = [T[c][i]['paint50'] for i in ids]
    med = lambda xs: sorted(xs)[len(xs) // 2] if xs else None
    sh = lambda D: sum(1 for i in ids if (D[c][i].get('shift50') or 0) > 0.01)
    la = lambda D: sum(1 for i in ids if (D[c][i].get('late50') or 0) > 0)
    cl = lambda D: ('—' if any('shiftCls50' not in D[c][i] for i in ids) else str(sum(1 for i in ids if (D[c][i].get('shiftCls50') or 0) > 0.01)))
    import math
    k = max(1, round(len(ids) * 0.8)); slow = sorted(range(len(ids)), key=lambda j: -b[j])[:k]
    sb = sum(b[j] for j in slow) / k; st = sum(t[j] for j in slow) / k
    geo = math.exp(sum(math.log(max(b[j], 0.1) / max(t[j], 0.1)) for j in range(len(ids))) / len(ids)) if ids else 0
    print(f"| {c} | {len(ids)} | {f(med(b))} -> {f(med(t))} | {sb:.0f} -> {st:.0f} ({sb / max(st, 0.1):.1f}x) | {geo:.1f}x | {band(b,16)} -> {band(t,16)} | {band(b,50)} -> {band(t,50)} | {band(b,100)} -> {band(t,100)} | {sh(B)} -> {sh(T)} | {cl(B)} -> {cl(T)} | {la(B)} -> {la(T)} |")
for c in cells:
    print(f'\n### {c}\n\n| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |')
    print('|---|---:|---:|---:|---|---|---|')
    ids = sorted(set(B[c]) & set(T[c]), key=lambda i: -(B[c][i]['paint50'] or 0))
    for i in ids:
        b, t = B[c][i], T[c][i]
        if b['paint50'] is None or t['paint50'] is None:
            print(f"| {i} | {f(b['paint50'])} / {f(b['paint95'])} | {f(t['paint50'])} / {f(t['paint95'])} | unmeasured | | |"); continue
        sp = b['paint50'] / max(t['paint50'], 0.1)
        flag = ''
        if t['paint50'] > b['paint50'] * 1.2 and t['paint50'] - b['paint50'] > 10: flag = ' **REGRESSION**'; regress.append((c, i, b['paint50'], t['paint50']))
        print(f"| {i}{flag} | {f(b['paint50'])} / {f(b['paint95'])} | {f(t['paint50'])} / {f(t['paint95'])} | {sp:.1f}x | {f(b.get('confirm50'))} -> {f(t.get('confirm50'))} | {f(b.get('settle50'))} -> {f(t.get('settle50'))} | {(b.get('shift50') or 0):.3f} -> {(t.get('shift50') or 0):.3f} |")
allt = sorted(((T[c][i]['paint50'], c, i) for c in cells for i in T[c] if T[c][i]['paint50'] is not None), reverse=True)[:10]
print('\n### Worst 10 after\n\n| interaction | cell | p50 |\n|---|---|---:|')
for p, c, i in allt: print(f'| {i} | {c} | {p:.0f} |')
print('\n### Regressions (p50 > +20 % and > +10 ms)\n')
print('\n'.join(f'- {c} {i}: {b:.0f} -> {t:.0f} ms' for c, i, b, t in regress) or '- none')
