#!/usr/bin/env python3
"""The composer's one chip and its menu (agent, model, effort; Account and Usage) driven by real mouse and keys (XTest); run by scripts/desktop-chip-menu.sh with DISPLAY
set to a private Xvfb. The menu floats over the thread: opening it, searching in it, moving to another agent's models, and shutting it (Esc, the chip, a press outside)
must leave the composer and every other region of the window where it was. Judged from the client's own G3 layout rectangles (BACKPLANE_GPUI_RECTS=1: `composer`,
`main`, `sidebar`, `rail`, `viewer`, the chip `modelchip` and the menu `chipmenu`), at two window sizes, with and without the viewer. Screenshots land in $R/shots."""
import ctypes, ctypes.util, json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *[str(v) for v in a]], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def last(ev):
    m = None
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_METRIC ') and f'"event":"{ev}"' in l:
            try: m = json.loads(l[12:])
            except ValueError: pass
    return m
def layout():
    time.sleep(0.6); m = last('layout'); return (m or {}).get('rects', {})
def ctl(cid):
    m = last('controls'); return next((c['rect'] for c in (m or {}).get('controls', []) if c['id'] == cid and c.get('rect')), None)
REGIONS = ['composer', 'main', 'sidebar', 'rail', 'topbar', 'viewer', 'modelchip']
def regions(r): return {k: r.get(k) for k in REGIONS if k in r}
def mid(r): return (int(r['x'] + r['w'] / 2), int(r['y'] + r['h'] / 2))
# the window to W x H at the screen's corner (no window manager decides it here)
X11 = ctypes.CDLL(ctypes.util.find_library('X11'))
X11.XOpenDisplay.restype = ctypes.c_void_p; X11.XOpenDisplay.argtypes = [ctypes.c_char_p]
X11.XDefaultRootWindow.restype = ctypes.c_ulong; X11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
X11.XQueryTree.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)), ctypes.POINTER(ctypes.c_uint)]
X11.XMoveResizeWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint]
X11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
X11.XGetGeometry.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong)] + [ctypes.POINTER(ctypes.c_int)] * 2 + [ctypes.POINTER(ctypes.c_uint)] * 4
D = X11.XOpenDisplay(os.environ['DISPLAY'].encode())
def size(w, h):
    root = X11.XDefaultRootWindow(D); r, p, ch, n = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    X11.XQueryTree(D, root, ctypes.byref(r), ctypes.byref(p), ctypes.byref(ch), ctypes.byref(n))
    for i in range(n.value):
        rr, gx, gy, gw, gh, bw, dp = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_int(), ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint()
        X11.XGetGeometry(D, ch[i], ctypes.byref(rr), ctypes.byref(gx), ctypes.byref(gy), ctypes.byref(gw), ctypes.byref(gh), ctypes.byref(bw), ctypes.byref(dp))
        if gw.value > 300 and gh.value > 300: X11.XMoveResizeWindow(D, ch[i], 0, 0, w, h)
    X11.XSync(D, 0); time.sleep(1.5)
def wait_menu(open_, s=3.0):
    t = time.time()
    while time.time() - t < s:
        r = layout()
        if ('chipmenu' in r) == open_: return r
    return layout()

# a thread with something in it: two exchanges with the stand-in
time.sleep(1); x('click', 700, 870); x('type', 'write me a few paragraphs'); x('key', 'Return'); time.sleep(3)
x('type', 'and another one'); x('key', 'Return'); time.sleep(3)
def files():   # the rail's Files: the viewer beside the thread opens or shuts
    w0 = layout()['main']['w']; x('click', *mid(ctl('rail:Files'))); time.sleep(2.5); return w0, layout()['main']['w']
for (w, h) in [(1400, 900), (1100, 650)]:
    for viewer in (False, True):
        size(w, h)
        if viewer: w0, w1 = files(); check(w1 < w0, f'the viewer opens: the thread {w0} -> {w1} wide')
        r = layout()
        tag = f'{w}x{h}' + ('-viewer' if viewer else '')
        before = regions(r); chip = r.get('modelchip')
        if not chip: check(False, f'{tag}: the chip is drawn ({r})'); continue
        shot(f'{tag}-0-shut')
        # open with the chip
        x('move', *mid(chip)); x('click', *mid(chip)); r = wait_menu(True); x('move', 5, h - 5); shot(f'{tag}-1-open')
        menu = r.get('chipmenu')
        check(menu is not None, f'{tag}: the chip opens the menu')
        check(regions(r) == before, f'{tag}: opening the menu moves nothing: {regions(r)} vs {before}')
        if menu:
            check(menu['y'] + menu['h'] <= chip['y'] + 0.5 and menu['y'] >= 0 and menu['x'] >= 0 and menu['x'] + menu['w'] <= w + 0.5, f'{tag}: the menu floats over the chip, inside the window: {menu} chip {chip}')
        # search: fewer models, the menu keeps its size
        x('type', 'son'); r = layout(); shot(f'{tag}-2-search')
        check(regions(r) == before and r.get('chipmenu') == menu, f'{tag}: a search moves nothing and the menu keeps its size: {r.get("chipmenu")} vs {menu}')
        for _ in range(3): x('key', 'BackSpace')
        # another agent's list (Shift+Tab to the agents, Down): the menu keeps its size
        x('chord', 'shift', 'Tab'); x('key', 'Down'); r = layout(); shot(f'{tag}-3-agent')
        check(regions(r) == before and r.get('chipmenu') == menu, f'{tag}: another agent\'s models move nothing and the menu keeps its size: {r.get("chipmenu")} vs {menu}')
        # Esc shuts it
        x('key', 'Escape'); r = wait_menu(False)
        check('chipmenu' not in r and regions(r) == before, f'{tag}: Esc shuts it and nothing moved: {regions(r)}')
        # the chip again shuts it
        x('click', *mid(chip)); wait_menu(True); x('click', *mid(chip)); r = wait_menu(False)
        check('chipmenu' not in r and regions(r) == before, f'{tag}: the chip shuts it again')
        # a press outside shuts it
        # a press outside shuts it (the sidebar's empty foot: the menu may cover the whole thread in a short window)
        sb = before['sidebar']; x('click', *mid(chip)); wait_menu(True); x('click', int(sb['x'] + sb['w'] / 2), int(sb['y'] + sb['h'] - 60)); r = wait_menu(False); shot(f'{tag}-4-outside')
        check('chipmenu' not in r and regions(r) == before, f'{tag}: a press outside shuts it and nothing moved')
        # (shut before the window narrows: in a narrow window the viewer pushes the rail and its Files out of the window)
        if viewer and (w, h) != (1100, 650): w0, w1 = files(); check(w1 > w0, f'the viewer shuts: the thread {w0} -> {w1} wide')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
