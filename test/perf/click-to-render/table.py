#!/usr/bin/env python3
# table.py RUNDIR [RUNDIR...] : markdown rows (slowest first) from run.mjs summaries.
# columns: interaction | state | machine | client | paint p50 / p95 | settle p50 | cold | where | shift | late
import json, sys
rows = []
for d in sys.argv[1:]:
    s = json.load(open(d + '/summary.json'))
    mach = {1: 'fast', 4: 'slow', 6: 'very slow'}.get(s['cpu'], f"cpu{s['cpu']}x") + (f" rtt{s['rtt']}" if s.get('rtt') else '') + (s.get('label') and f" {s['label']}" or '')
    for r in s['rows']:
        r['_state'] = s['state']; r['_mach'] = mach; r['_kind'] = s['kind'] + (('/' + s['device']) if s.get('device') else ''); rows.append(r)
def where(r):
    p = r['paint50']
    if p is None: return ''
    send, recv, recvN, dom = r['send50'], r['recv50'], r['recvN50'], r['dom50']
    if send is None or r['local'] > r['n'] / 2:
        return f"client only (loaf {r['loaf50']:.0f})" if r['loaf50'] else 'client only'
    parts = [f"pre-send {send:.0f}"]
    if recv is not None: parts.append(f"hub {recv - send:.0f}")
    if recvN is not None and recv is not None and recvN - recv > 5: parts.append(f"more msgs {recvN - recv:.0f}")
    if dom is not None and recvN is not None: parts.append(f"patch {dom - recvN:.0f}")
    if dom is not None: parts.append(f"frame {p - dom:.0f}")
    return ', '.join(parts)
f = lambda x: '' if x is None else f'{x:.0f}'
rows.sort(key=lambda r: -(r['paint50'] or 1e9))
print('| interaction | state | machine | client | paint p50 | p95 | settle p50 | cold | where the time goes (p50 ms) | round trip | hub wait | shift | late |')
print('|---|---|---|---|---:|---:|---:|---:|---|---:|---:|---:|---:|')
for r in rows:
    err = f" ({r['errors']} err)" if r['errors'] else ''
    print(f"| {r['id']}{err} | {r['_state']} | {r['_mach']} | {r['_kind']} | {f(r['paint50'])} | {f(r['paint95'])} | {f(r['settle50'])} | {f(r['cold'])} | {where(r)} | {('y' if r.get('roundTrip', 0) * 2 > r['n'] else 'n') if r['n'] else ''} | {f(r.get('hubWait50'))} | {r['shift50'] or 0:.3f} | {f(r['late50'])} |")
