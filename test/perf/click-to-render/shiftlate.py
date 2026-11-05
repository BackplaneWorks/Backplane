#!/usr/bin/env python3
# shiftlate.py BASE_RUN TIP_RUN CELL : per interaction, what moved (layout-shift sources: element, y/x from -> to, when,
# in how many warm reps) and what appeared late (visible text/elements added after the result's frame: element, text,
# how late), each late item marked "both" (also late on base) or "new" (late only on the tip: we now show earlier).
import json, os, sys, collections
args = [a for a in sys.argv[1:] if a not in ('--summary', '--cls')]; SUMMARY = '--summary' in sys.argv; CLS = '--cls' in sys.argv  # --cls: only shifts without recent input (Chromium's CLS rule)
base, tip = args[0], args[1]; cells = args[2:]
cell = cells[0]
def reps(run, i):
    try: return [r for r in json.load(open(f'{run}/{cell}/{i}.json')) if not r.get('cold') and not r.get('error') and r.get('paint') is not None]
    except (OSError, ValueError): return []
def shifts(rs):
    c = collections.defaultdict(list); tot = 0
    for r in rs:
        tot += r.get('shiftCls' if CLS else 'shift') or 0
        for sh in r.get('shifts', []):
            if CLS and sh.get('recent', True): continue
            for s in sh['src']:
                f, t = s['from'], s['to']
                c[s['n']].append((round(sh['t']), t[0] - f[0], t[1] - f[1], t[3] - f[3]))
    return tot / max(len(rs), 1), c
def lates(rs):
    c = collections.defaultdict(list)
    for r in rs:
        for l in r.get('late', []): c[(l['el'], l['text'][:40])].append(l['dt'])
    return c
def summary_row(c):
    global cell; cell = c
    ids = sorted({f[:-5] for f in os.listdir(f'{tip}/{cell}') if f.endswith('.json') and f != 'summary.json'} & {f[:-5] for f in os.listdir(f'{base}/{cell}') if f.endswith('.json')})
    cb = ct = 0; nb = nt = 0; lb_n = lt_new = lt_both = 0; unk_b = [False]; unk_t = [False]
    for i in ids:
        T, B = reps(tip, i), reps(base, i)
        if not T or not B: continue
        sb, _ = shifts(B); st, _ = shifts(T)
        if CLS and any('shiftCls' not in r for r in B): unk_b[0] = True
        if CLS and any('shiftCls' not in r for r in T): unk_t[0] = True
        cb += sb; ct += st; nb += sb > 0.01; nt += st > 0.01
        lb, lt = lates(B), lates(T)
        lb_n += sum(len(v) for v in lb.values()) / len(B)
        for k, v in lt.items():
            if k in lb: lt_both += len(v) / len(T)
            else: lt_new += len(v) / len(T)
    n = max(len(ids), 1)
    B_ = '—' if unk_b[0] else None; T_ = '—' if unk_t[0] else None
    print(f'| {cell} | {len(ids)} | {B_ or nb} -> {T_ or nt} | {B_ or f"{cb / n:.3f}"} -> {T_ or f"{ct / n:.3f}"} | {lb_n:.1f} | {lt_both:.1f} | {lt_new:.1f} |')
if SUMMARY:
    print('| cell | interactions | shifting (>0.01) before -> after | mean CLS / interaction before -> after | late items / rep, all interactions: before | after, late in both | after, NEW (shown earlier) |')
    print('|---|---:|---|---|---:|---:|---:|')
    for c in cells: summary_row(c)
    sys.exit(0)
ids = sorted({f[:-5] for f in os.listdir(f'{tip}/{cell}') if f.endswith('.json') and f != 'summary.json'})
print(f'### {cell}: tip {os.path.basename(tip)} vs base\n')
for i in ids:
    T, B = reps(tip, i), reps(base, i)
    if not T: continue
    st, sc = shifts(T); sb, _ = shifts(B)
    lt, lb = lates(T), lates(B)
    if st < 0.01 and not lt: continue
    n = len(T)
    print(f'- **{i}** (shift/rep {sb:.3f} base -> {st:.3f} tip; late items/rep {sum(len(v) for v in lb.values()) / max(len(B), 1):.1f} -> {sum(len(v) for v in lt.values()) / n:.1f})')
    for el, v in sorted(sc.items(), key=lambda kv: -len(kv[1]))[:5]:
        ts = sorted(x[0] for x in v); dx = sorted(x[1] for x in v)[len(v) // 2]; dy = sorted(x[2] for x in v)[len(v) // 2]; dh = sorted(x[3] for x in v)[len(v) // 2]
        print(f'  - moves: `{el}` dy {dy:+.0f} dx {dx:+.0f} dh {dh:+.0f} px at ~{ts[len(ts) // 2]} ms ({len(v)}x in {n} reps)')
    for (el, text), v in sorted(lt.items(), key=lambda kv: -len(kv[1]))[:5]:
        tag = 'both' if (el, text) in lb else 'NEW'
        print(f'  - late [{tag}]: `{el}` "{text}" at ~{sorted(v)[len(v) // 2]} ms ({len(v)}x in {n} reps)')
print()
