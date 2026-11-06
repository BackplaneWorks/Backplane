#!/usr/bin/env python3
# m2table.py RUNDIR [MAIN_CELL] : round 2 (CONTENT) table: per interaction, per cell frame p50 / content p50,
# ranked by the main cell's content p50 (slowest first); then the "late after the press frame" list per interaction.
# frame = the old predicate's pixels (px: the frame that first shows the result, e.g. an empty viewer pane);
# content = the content the person came for is on screen AND final (last change of its signature, pixels).
import json, os, sys
run = sys.argv[1]; main = sys.argv[2] if len(sys.argv) > 2 else 'R-fast-web'
order = ['R-fast-web', 'R-fast-web-rtt30', 'R-fast-web-rtt100', 'R-fast-web-rtt250', 'R-slow-web', 'R-vslow-web', 'R-fast-desk', 'R-fast-phone', 'X-fast-web', 'X-slow-web', 'X-slow-web-rtt100', 'X-vslow-web-rtt100']
cells = sorted((c for c in os.listdir(run) if os.path.exists(f'{run}/{c}/summary.json')), key=lambda c: order.index(c) if c in order else 99)
data = {c: {r['id']: r for r in json.load(open(f'{run}/{c}/summary.json'))['rows']} for c in cells}
m = data.get(main) or data[cells[0]]
f = lambda x: '—' if x is None else f'{x:.0f}'
ids = list(dict.fromkeys(i for c in cells for i in data[c]))
key = lambda i: -(((m.get(i) or {}).get('cnt50')) or ((m.get(i) or {}).get('px50')) or -1)
ids.sort(key=key)
import statistics
print('| cell | clicks | median frame -> CONTENT p50 | frame within 50 / 100 ms | CONTENT within 50 / 100 ms | content > frame + 50 ms | content waited on a hub msg |')
print('|---|---:|---|---|---|---:|---:|')
for c in cells:
    rs = [r for r in data[c].values() if r.get('cnt50') is not None and r.get('px50') is not None]
    if not rs: continue
    w = lambda k, t: sum(1 for r in rs if r[k] <= t)
    print(f"| {c} | {len(rs)} | {statistics.median(r['px50'] for r in rs):.0f} -> **{statistics.median(r['cnt50'] for r in rs):.0f}** | {w('px50', 50)} / {w('px50', 100)} | **{w('cnt50', 50)} / {w('cnt50', 100)}** | {sum(1 for r in rs if r['cnt50'] > r['px50'] + 50)} | {sum(1 for r in rs if r.get('cHub', 0) * 2 > r.get('n', 1))} |")
print()
hdr = ['interaction'] + [f'{c} frame / CONTENT' for c in cells] + [f'content − frame ({main})', f'content after a hub msg ({main})', 'late after the press frame (main cell; median ms after the input, reps)']
print('| ' + ' | '.join(hdr) + ' |'); print('|' + '---|' * len(hdr))
for i in ids:
    r = m.get(i) or {}
    row = [i + (' (content = frame)' if r.get('contentIsFrame') else '')]
    for c in cells:
        x = data[c].get(i)
        if not x: row.append(''); continue
        miss = f" ({x['cntMissing']} no content)" if x.get('cntMissing') else ''
        row.append(f"{f(x.get('px50'))} / **{f(x.get('cnt50'))}**{miss}" + (f" ({x['errors']} err)" if x.get('errors') else ''))
    d = None if r.get('cnt50') is None or r.get('px50') is None else r['cnt50'] - r['px50']
    rt = ('y' if r.get('cHub', 0) * 2 > r.get('n', 0) else 'n') if r.get('n') and 'cHub' in r else ''
    if rt == 'y' and r.get('cHubLast50') is not None: rt += f" (last @{f(r['cHubLast50'])})"
    groups = {}
    for l in (r.get('lateTop') or []):
        el, _, txt = l['what'].partition(' ')
        g = groups.setdefault(el, {'n': 0, 'txt': txt.strip('"')[:40], 'dts': [], 'reps': l['reps']}); g['n'] += 1; g['dts'].append(l['dt50'])
    groups.pop('div.toast', None) if 'copied log' in (groups.get('div.toast') or {}).get('txt', '') else None
    late = '; '.join(f"{el}{' x' + str(g['n']) if g['n'] > 1 else ''} \"{g['txt']}\" @{f(sorted(g['dts'])[len(g['dts']) // 2])}" + ('' if g['reps'] >= r.get('n', 0) else f" ({g['reps']}/{r.get('n')})") for el, g in list(groups.items())[:6])
    row += [f(d), rt, late.replace('|', '/')]
    print('| ' + ' | '.join(row) + ' |')
