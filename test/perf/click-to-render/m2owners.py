#!/usr/bin/env python3
# m2owners.py RUNDIR [MAIN_CELL] : per owner (V1 viewer data, V2 3D, V3 the rest + press-picture chrome), the
# interactions whose CONTENT is later than the frame or than 50 ms, with content p50 per cell and the late items.
import json, os, sys
run = sys.argv[1]; main = sys.argv[2] if len(sys.argv) > 2 else 'R-fast-web'
cells = [c for c in ['R-fast-web', 'R-fast-web-rtt100', 'R-slow-web', 'X-slow-web', 'X-slow-web-rtt100', 'R-fast-phone'] if os.path.exists(f'{run}/{c}/summary.json')]
data = {c: {r['id']: r for r in json.load(open(f'{run}/{c}/summary.json'))['rows']} for c in cells}
V1 = {'viewer-fm', 'viewer-diff', 'viewer-board', 'viewer-schematic', 'viewer-browser', 'viewer-review', 'viewer-switch', 'viewer-board-sch', 'fm-in', 'fm-up', 'reload-fm', 'reload-board', 'reload-diff', 'ph-viewer-fm', 'ph-viewer-diff', 'ph-viewer-board', 'ph-viewer-schematic'}
V2 = {'viewer-3d', 'ph-viewer-3d'}
CHROME = ('viewer-close', 'rail-toggle', 'button.tab', 'tok-pop', 'shade')
f = lambda x: '—' if x is None else f'{x:.0f}'
def owner(i): return 'V1' if i in V1 else 'V2' if i in V2 else 'V3'
m = data.get(main) or {}
out = {'V1': [], 'V2': [], 'V3': []}; CH = {}
for i in sorted(set(k for c in cells for k in data[c]), key=lambda i: -((m.get(i) or {}).get('cnt50') or 0)):
    r = m.get(i) or next((data[c][i] for c in cells if i in data[c]), {})
    cnts = ', '.join(f"{c.replace('-web', '')} {f(data[c][i].get('px50'))}/{f(data[c][i].get('cnt50'))}" for c in cells if i in data[c])
    late = r.get('lateTop') or []
    chrome = [l for l in late if any(k in l['what'] for k in CHROME)]
    rest = [l for l in late if l not in chrome and 'copied log' not in l['what']]
    worst = max((data[c][i].get('cnt50') or 0) for c in cells if i in data[c])
    if worst > 50 or rest:
        out[owner(i)].append(f"- **{i}** (frame/CONTENT p50: {cnts})" + (': late ' + '; '.join(f"`{l['what'][:60]}` @{f(l['dt50'])}" for l in rest[:5]) if rest else ''))
    for l in chrome: CH.setdefault(l['what'][:60], []).append((i, l['dt50']))
for w, xs in CH.items(): out['V3'].insert(0, f"- **press-picture chrome** `{w}` late in {len(xs)} clicks: " + ', '.join(f"{i} @{f(d)}" for i, d in xs))
names = {'V1': 'V1 — viewer data (Files, Diff, Board/Schematic history bar, Browser, Review)', 'V2': 'V2 — 3D', 'V3': 'V3 — everything else (thread body, popovers, Close button / press-picture chrome, shelf, send, Settings, pickers, search, palette)'}
for k in ['V1', 'V2', 'V3']:
    print(f"\n### {names[k]}"); print('\n'.join(out[k]) or '- (nothing over 50 ms)')
