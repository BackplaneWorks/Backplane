#!/usr/bin/env python3
# board.py RUN_DIR [MAIN_CELL] : one row per interaction, columns per cell (R-fast-web, R-fast-web-rtt100, R-slow-web, ...)
# found under RUN_DIR/<cell>/summary.json; sorted by the main cell's paint p50 (slowest first). Prints markdown.
import json, os, sys
run = sys.argv[1]; main = sys.argv[2] if len(sys.argv) > 2 else 'R-fast-web'
cells = sorted(c for c in os.listdir(run) if os.path.exists(f'{run}/{c}/summary.json'))
order = ['R-fast-web', 'R-fast-web-rtt30', 'R-fast-web-rtt100', 'R-fast-web-rtt250', 'R-slow-web', 'R-vslow-web', 'R-fast-desk', 'S-fast-web', 'X-fast-web', 'X-slow-web']
cells.sort(key=lambda c: order.index(c) if c in order else 99)
data = {c: {r['id']: r for r in json.load(open(f'{run}/{c}/summary.json'))['rows']} for c in cells}
# round trip / hub wait from the raw reps (older summaries lack them)
def med(xs):
    xs = sorted(xs); return xs[len(xs) // 2] if xs else None
for c in cells:
    for i, r in data[c].items():
        try: reps = [x for x in json.load(open(f'{run}/{c}/{i}.json')) if not x.get('cold') and not x.get('error') and x.get('paint') is not None]
        except (OSError, ValueError): continue
        r['roundTrip'] = sum(1 for x in reps if x.get('recv1') is not None and x['recv1'] <= x['dom'])
        r['hubWait50'] = med([(x['recvN'] - x['send']) if x.get('recvN') is not None and x.get('send') is not None and x['recvN'] <= x['dom'] else 0 for x in reps])
ids = list(dict.fromkeys(i for c in cells for i in data[c]))
m = data.get(main) or data[cells[0]]
f = lambda x: '—' if x is None else f'{x:.0f}'
def where(r):
    p = r['paint50']
    if p is None: return ''
    send, recv, recvN, dom = r['send50'], r['recv50'], r['recvN50'], r['dom50']
    if send is None or r['local'] > r['n'] / 2: return 'client only'
    out = [f"pre-send {send:.0f}"]
    if recv is not None: out.append(f"hub {recv - send:.0f}")
    if recvN is not None and recv is not None and recvN - recv > 5: out.append(f"+msgs {recvN - recv:.0f}")
    if dom is not None and recvN is not None: out.append(f"patch {dom - recvN:.0f}")
    if dom is not None: out.append(f"frame {p - dom:.0f}")
    return ', '.join(out)
ids.sort(key=lambda i: -((m.get(i) or {}).get('paint50') or 1e9))
hdr = ['interaction'] + [f'{c} p50/p95' for c in cells] + [f'within 16/50/100 ({main})', f'confirmed by hub ({main})', f'content ({main})', f'settle ({main})', 'cold', 'round trip', 'hub wait', f'where ({main}, p50 ms)', 'shift', 'late']
print('| ' + ' | '.join(hdr) + ' |'); print('|' + '---|' * len(hdr))
for i in ids:
    r = m.get(i) or {}
    row = [i + (f" ({r['errors']} err)" if r.get('errors') else '')]
    for c in cells:
        x = data[c].get(i); row.append(f"{f(x['paint50'])} / {f(x['paint95'])}" if x and x['paint50'] is not None else ('err' if x else ''))
    rt = ('y' if r.get('roundTrip', 0) * 2 >= r.get('n', 0) else 'n') if r.get('n') else ''
    p = r.get('paint50'); band = '' if p is None else ('16 ✓' if p <= 16 else '50 ✓' if p <= 50 else '100 ✓' if p <= 100 else '✗')
    row += [band, f(r.get('confirm50')), f(r.get('content50')), f(r.get('settle50')), f(r.get('cold')), rt, f(r.get('hubWait50')), where(r) if r else '', f"{r.get('shift50') or 0:.3f}", f(r.get('late50'))]
    print('| ' + ' | '.join(row) + ' |')
