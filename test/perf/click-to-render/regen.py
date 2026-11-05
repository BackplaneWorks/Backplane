#!/usr/bin/env python3
# regen.py CELLDIR : rebuild CELLDIR/summary.json rows from the per-interaction raw files (as run.mjs computes them)
import json, os, sys
d = sys.argv[1]; S = json.load(open(f'{d}/summary.json'))
def pct(xs, p):
    s = sorted(x for x in xs if x is not None)
    return s[min(len(s) - 1, int(p * (len(s) - 1) + 0.5))] if s else None
rows = []
for f in sorted(os.listdir(d)):
    if not f.endswith('.json') or f == 'summary.json': continue
    runs = json.load(open(f'{d}/{f}')); i = f[:-5]
    warm = [r for r in runs if not r.get('cold') and not r.get('error') and r.get('paint') is not None]
    cold = next((r for r in runs if r.get('cold') and not r.get('error')), None)
    m = lambda k, p=0.5: pct([r.get(k) for r in warm], p)
    row = {'id': i, 'n': len(warm), 'errors': sum(1 for r in runs if r.get('error')),
           'paint50': m('paint'), 'paint95': m('paint', .95), 'px50': m('px'), 'px95': m('px', .95), 'content50': m('content'),
           'confirm50': m('confirm'), 'dom50': m('dom'), 'settle50': m('settle'), 'cold': cold.get('paint') if cold else None,
           'send50': m('send'), 'recv50': m('recv1'), 'recvN50': m('recvN'), 'shift50': m('shift'), 'late50': m('nLate'),
           'local': sum(1 for r in warm if r.get('send') is None or r['send'] > r['dom'])}
    if all('shiftCls' in r for r in warm) and warm: row['shiftCls50'] = m('shiftCls'); row['shiftClsMax'] = max(r['shiftCls'] for r in warm)
    rows.append(row)
S['rows'] = rows; S['regenerated'] = True
json.dump(S, open(f'{d}/summary.json', 'w'), indent=1); print(len(rows), 'rows')
