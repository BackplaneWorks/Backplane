#!/usr/bin/env python3
"""Pinned threads and picked project icons on the real window (private Xvfb, XTest mouse; scripts/desktop-pins-icons.sh sets it up). Judged by the hub's event log and the
client's own `controls`/`probes` metrics (the sidebar's order is the order its rows register). Screenshots: $R/shots.
  icons: a project named Backplane shows the logo and is never asked about; the others get a pick from the hub's one-shot (USB sniffer: usb teal, Deck: presentation), the person's icon wins, Pick again asks anew (plug)
  pins:  a row's hover pin and the header's Pin pin a thread; pins list first, in the order they were pinned; a drag reorders them; they survive a hub restart;
         a pinned row's pin unpins it."""
import json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *(str(int(v)) if isinstance(v, float) else str(v) for v in a)], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def metrics(ev):
    out = []
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_METRIC '):
            try: m = json.loads(l[12:])
            except ValueError: continue
            if m.get('event') == ev: out.append(m)
    return out
def controls():
    m = metrics('controls'); return m[-1]['controls'] if m else []
def rects():
    r = {c['id']: c['rect'] for c in controls() if c.get('rect')}
    p = metrics('probes')
    for q in (p[-1]['rects'] if p else []): r.setdefault(q['id'], q)
    return r
def centre(rid, s=6.0):
    t = time.time()
    while time.time() - t < s:
        r = rects().get(rid)
        if r: return r['x'] + r['w'] / 2, r['y'] + r['h'] / 2
        time.sleep(0.1)
    return None
def order():  # the project's thread rows as the sidebar lists them
    return [c['id'][7:] for c in controls() if c['id'].startswith('thread:') and c['id'][7:] in ('t1', 't2', 't3', 't4')]
def events(): return [json.loads(l) for l in open(f'{R}/home/events.jsonl') if l.strip()]
def setting(k):
    v = None
    for e in events():
        if e.get('$') == 'SettingSet' and e.get('key') == k: v = e.get('value')
    return v
def until(f, s=8.0):
    t = time.time()
    while time.time() - t < s:
        v = f()
        if v: return v
        time.sleep(0.15)
    return f()
def press(rid, hover_first=None):
    if hover_first:
        h = centre(hover_first)
        if h: x('move', *h); time.sleep(0.3)
    c = centre(rid); check(c is not None, f'{rid} is on screen')
    if c: x('click', *c); time.sleep(0.5)
def pinned(t):
    v = False
    for e in events():
        if e.get('$') == 'ThreadPinned' and e.get('id') == t: v = e.get('pinned')
    return v

time.sleep(2); x('focus', 'Backplane')
# ---- icons: picked off to the side, once a project (the first round runs 15 s after the hub starts)
p1 = until(lambda: setting('project.pick.p1'), 40); p2 = until(lambda: setting('project.pick.p2'), 10)
check(p1 == 'usb teal', f'the USB sniffer is picked usb, teal: {p1}'); check(p2 == 'presentation', f'the deck is picked presentation: {p2}')
seen = open(f'{R}/picks.log').read() if os.path.exists(f'{R}/picks.log') else ''
check('USB 2.0 protocol analyzer' in seen and 'boards/' in seen and 'pitch.pptx' in seen, 'the model read the intent and the top-level files')
check(setting('project.pick.p3') is None and 'Name: Backplane' not in seen, 'a project named Backplane is never asked about: it shows the logo')
time.sleep(1.5); shot('01-icons')
# ---- pins: the hover pin on a row (t3), then t1
press('pin:t3', hover_first='thread:t3'); check(until(lambda: pinned('t3')), 'the row\'s hover pin pins t3')
press('pin:t1', hover_first='thread:t1'); check(until(lambda: pinned('t1')), 'the row\'s pin pins t1')
o = until(lambda: order()[:2] == ['t3', 't1'] and order()); shot('02-pinned-two')
check(o and o[:2] == ['t3', 't1'], f'pins first, in the order they were pinned: {o}')
# the header's Pin on t2
press('thread:t2'); time.sleep(0.6); press('tool:pin'); check(until(lambda: pinned('t2')), 'the header\'s Pin pins t2')
o = until(lambda: order()[:3] == ['t3', 't1', 't2'] and order()); shot('03-header-pin')
check(o and o[:3] == ['t3', 't1', 't2'], f'the newest pin goes to the bottom of the pins: {o}')
# drag t2 onto t3: it takes t3's place
a, b = centre('thread:t2'), centre('thread:t3')
if a and b: x('drag', a[0], a[1], b[0], b[1] - 4)
o = until(lambda: order()[:1] == ['t2'] and order()); shot('04-dragged')
check(any(e.get('$') == 'ThreadOrdered' and e.get('section') == 'pinned' for e in events()), 'the drag is recorded on the pinned shelf')
check(o and o[:3] == ['t2', 't3', 't1'], f'a drag reorders the pins: {o}')
# a hub restart: the pins and their order come back from the log
open(f'{R}/restart', 'w').close(); time.sleep(6)
o = until(lambda: order()[:3] == ['t2', 't3', 't1'] and order(), 20); shot('05-after-restart')
check(o and o[:3] == ['t2', 't3', 't1'], f'the pins and their order survive a hub restart: {o}')
# a pinned row's pin unpins it
press('pin:t3', hover_first='thread:t3'); check(until(lambda: not pinned('t3')), 'a pinned row\'s pin unpins t3')
o = until(lambda: order()[:2] == ['t2', 't1'] and order()); check(o and o[:2] == ['t2', 't1'], f'the unpinned one leaves the pins: {o}')
# ---- the person's icon wins; Pick again clears it and asks anew
hdr = centre('proj:p1') or centre('fold:p1')
if hdr: x('click', hdr[0] + 60, hdr[1], 3); time.sleep(0.6)
press('ctx:Badge'); time.sleep(0.6); shot('06-badge')
press('look:project.icon.p1=cpu'); check(until(lambda: setting('project.icon.p1') == 'cpu'), 'the person picks cpu by hand')
check(setting('project.pick.p1') == 'usb teal', 'the pick is kept beside the person\'s choice'); shot('07-manual')
press('look:again'); check(until(lambda: setting('project.icon.p1') == '' and setting('project.pick.p1') in ('', 'plug')), 'Pick again clears the person\'s icon')
check(until(lambda: setting('project.pick.p1') == 'plug', 15) , f'Pick again asks the model anew: {setting("project.pick.p1")}')
time.sleep(1.5); shot('08-picked-again')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
