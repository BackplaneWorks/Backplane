#!/usr/bin/env python3
"""A project row's fold and file links that open inside Backplane, on the real window (private Xvfb, XTest mouse and keys; scripts/desktop-fold-links.sh
sets it up). Judged by the hub's event log and the client's own metrics (`controls`, `probes`, `shown`, GPUI_RECT link rectangles). Screenshots: $R/shots.
  fold:  the chevron on Backplane's row folds its threads away (its count stays), the hub keeps it (sidebar.fold.<key>) across a restart, a second press
         unfolds; Left and Right fold and unfold the row the sidebar has the keyboard on.
  links: a schematic path in another project's message opens Backplane's thread on Schematic with that file; a bare board name opens Board; a file://
         board in no project is drawn in the thread on show; the system's app is never asked (no file.open)."""
import json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *(str(int(v)) if isinstance(v, float) else str(v) for v in a)], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def crop(n, w=330, h=420):  # the sidebar alone, for looking at the row
    subprocess.run(['convert', f'{R}/shots/{n}.png', '-crop', f'{w}x{h}+0+0', f'{R}/shots/{n}-side.png'], check=False)
def lines(): return open(f'{R}/client.log', errors='replace').read().splitlines()
def metrics(ev):
    out = []
    for l in lines():
        if l.startswith('GPUI_METRIC '):
            try: m = json.loads(l[12:])
            except ValueError: continue
            if m.get('event') == ev: out.append(m)
    return out
def controls():
    m = metrics('controls'); return m[-1]['controls'] if m else []
def ids(): return [c['id'] for c in controls()]
def rects():
    r = {c['id']: c['rect'] for c in controls() if c.get('rect')}
    p = metrics('probes')
    for q in (p[-1]['rects'] if p else []): r.setdefault(q['id'], q)
    for l in lines():  # links in messages (check mode's GPUI_RECT), the newest place of each
        if l.startswith('GPUI_RECT '):
            try: q = json.loads(l[10:])
            except ValueError: continue
            if q.get('name', '').startswith('link:'): r[q['name']] = q
    return r
def centre(rid, s=8.0, left=False):
    t = time.time()
    while time.time() - t < s:
        r = rects().get(rid)
        if r: return (r['x'] + min(8, r['w'] / 2) if left else r['x'] + r['w'] / 2), r['y'] + r['h'] / 2
        time.sleep(0.15)
    return None
def press(rid, left=False):
    c = centre(rid, left=left); check(c is not None, f'{rid} is on screen')
    if c: x('click', *c); time.sleep(0.6)
    return c
def events(): return [json.loads(l) for l in open(f'{R}/home/events.jsonl') if l.strip()]
def setting(prefix):
    v = {}
    for e in events():
        if e.get('$') == 'SettingSet' and e.get('key', '').startswith(prefix): v[e['key']] = e.get('value')
    return v
def until(f, s=10.0):
    t = time.time()
    while time.time() - t < s:
        v = f()
        if v: return v
        time.sleep(0.2)
    return f()
def shown():
    m = metrics('shown'); return m[-1] if m else {}
WIG = [0]
def under():  # (the controls metric is the window's own frame: a move of the pointer draws one after the sidebar's redraw)
    WIG[0] ^= 1; x('move', 700, 400 + WIG[0] * 6); time.sleep(0.1)
    return [i for i in ids() if i.startswith('thread:t') and i[7:] in ('t1', 't2', 't3', 't4')]
def folded(): return any(v for v in setting('sidebar.fold.').values())

time.sleep(2); x('focus', 'Backplane')
# ---- fold
check(until(lambda: 'project-fold:p1' in ids(), 30), 'the Backplane row has its chevron')
check(until(lambda: len(under()) == 4), f'its four running threads are under it: {under()}')
time.sleep(1); shot('01-open'); crop('01-open')
c = centre('project-fold:p1')
if c: x('move', *c); time.sleep(0.6); shot('01b-hover'); x('move', 700, 400)
press('project-fold:p1')
check(until(folded), 'the chevron folds it, kept by the hub (sidebar.fold.<key>)')
check(until(lambda: not under()), f'no thread rows under it: {under()}')
check('project:p1' in ids() and 'project-fold:p1' in ids(), 'the row and its chevron stay')
time.sleep(1); shot('02-folded'); crop('02-folded')
# the hub keeps it across a restart
open(f'{R}/restart', 'w').close(); time.sleep(6)
check(until(lambda: 'project-fold:p1' in ids(), 30), 'the row is back after the restart')
time.sleep(2); check(not under(), f'still folded after the restart: {under()}')
press('project-fold:p1')
check(until(lambda: not folded()), 'a second press unfolds it')
check(until(lambda: len(under()) == 4), f'its threads are back: {under()}')
# the keys: a press on the row gives the sidebar the keyboard, Left folds, Right unfolds
press('project:p1', left=True); time.sleep(0.5)
x('key', 'Left'); check(until(folded), 'Left folds the row')
x('key', 'Right'); check(until(lambda: not folded()), 'Right unfolds it')
time.sleep(1); shot('03-keys'); crop('03-keys')
# ---- links: Framework's thread says where Backplane's schematic is
press('thread:t5'); check(until(lambda: shown().get('thread') == 't5'), f'the Framework thread is open: {shown()}')
sch = f'{R}/bp/boards/main/main.kicad_sch'
time.sleep(1.5); shot('04-message')
press(f'link:{sch}')
s = until(lambda: shown().get('thread') in ('t1', 't2', 't3', 't4') and shown().get('open') == 'schematic' and shown())
check(bool(s), f'the schematic link opens a Backplane thread on Schematic: {shown()}')
check(sch in (shown().get('key') or ''), f'with that file as its source: {shown().get("key")}')
time.sleep(3); shot('05-schematic')
# a board named alone in Backplane's newest thread (now on show): found below its folder, on Board
check(shown().get('thread') == 't4', f'the newest Backplane thread took the link: {shown()}')
press('link:main.kicad_pcb')
check(bool(until(lambda: shown().get('open') == 'board' and shown())), f'a bare board name opens Board: {shown()}')
# a board in no project: drawn in the thread on show (Framework's)
press('thread:t5'); until(lambda: shown().get('thread') == 't5'); time.sleep(2)
loose = f'file://{R}/loose/loose.kicad_pcb'
press(f'link:{loose}')
check(bool(until(lambda: shown().get('thread') == 't5' and shown().get('open') == 'board' and 'loose.kicad_pcb' in (shown().get('key') or ''))), f'a board in no project is drawn in the thread on show: {shown()}')
time.sleep(3); shot('06-loose')
# never the system's app
check('"m":"file.open"' not in open(f'{R}/hub.log', errors='replace').read() and not any('file.open' in l and 'link' in l for l in lines()), 'no file was handed to the system')
print('PASS' if not bad else f'FAIL {len(bad)}: ' + '; '.join(bad), flush=True)
sys.exit(1 if bad else 0)
