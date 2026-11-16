#!/usr/bin/env python3
"""KiCad in the pane on macOS, the whole path: a real hub, the real GPUI client and a real Backplane KiCad with the macOS embedding
(capability embed-macos), on a logged-in macOS desktop (the e2e-macos runner's `kicad-pane` job; a Mac under the hub lock mac-ui).

The client is driven by its own --script (thread, Board, Edit) and by Quartz events; windows are read from the window list (owner pid,
bounds, stacking: no permission needed); the hub's log (BACKPLANE_KICAD_DEBUG) gives KiCad's embed events; the client's log its keys.

Checks, as a person meets them first:
  - Edit: KiCad's board editor is placed over the pane, directly above Backplane's window, and painted;
  - Edit, then typing with no click: KiCad is the frontmost app and its canvas takes the keys (Cmd+A, a plain Escape: the person's
    selection changes, ET_SELECTION_CHANGED in the hub's log);
  - Cmd+K pressed in KiCad reaches Backplane (the client logs the reserved chord);
  - Backplane's window moved: KiCad's frame follows it;
  - Backplane's window full screen: KiCad's frame leaves the screen (another app's window cannot join our full-screen Space), and is back
    over the pane when the window leaves full screen.

usage: mac_pane_e2e.py CLIENT HUB ASSETS RUNTIME OUT
"""
import json, os, shutil, subprocess, sys, tempfile, time
import Quartz
from AppKit import NSWorkspace, NSRunningApplication


def active(pid):
    """The app is the active one (an accessory KiCad is active while it holds the keyboard, though NSWorkspace's frontmost app, the
    menu bar's owner, stays Backplane)."""
    a = NSRunningApplication.runningApplicationWithProcessIdentifier_(pid)
    return bool(a and a.isActive())

CLIENT, HUB, ASSETS, RT, OUT = sys.argv[1:6]
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
FAILS, RES, procs = [], {}, []


def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: FAILS.append(what)
    return ok


def wait_for(f, s, step=0.1):
    end = time.time() + s
    while time.time() < end:
        v = f()
        if v: return v
        time.sleep(step)
    return f()


def wins():
    out = []
    for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID):
        if w.get('kCGWindowLayer', 0) != 0: continue
        b = w['kCGWindowBounds']
        out.append((int(w['kCGWindowNumber']), int(w['kCGWindowOwnerPID']), w.get('kCGWindowOwnerName', ''), (round(b['X']), round(b['Y']), round(b['Width']), round(b['Height']))))
    return out


def key(code, flags=0):
    for down in (True, False):
        e = Quartz.CGEventCreateKeyboardEvent(None, code, down); Quartz.CGEventSetFlags(e, flags); Quartz.CGEventPost(Quartz.kCGHIDEventTap, e); time.sleep(0.03)


CMD = Quartz.kCGEventFlagMaskCommand
K_A, K_K, K_ESC, K_B, K_J, K_C, K_TAB, K_DEL, K_F, K_COMMA = 0, 40, 53, 11, 38, 8, 48, 51, 3, 43
CTRL, SHIFT, ALT = Quartz.kCGEventFlagMaskControl, Quartz.kCGEventFlagMaskShift, Quartz.kCGEventFlagMaskAlternate

# ---- the project, the hub's home, KiCad's settings where KiCad on macOS reads them ----
os.makedirs(OUT, exist_ok=True)
subprocess.run([sys.executable, os.path.join(REPO, 'test/kicad_embed/lib/seed_project.py'), OUT, REPO], check=True)
for v in ('10.0', '10.99'):
    src = os.path.join(OUT, 'fh/.config/kicad', v)
    if os.path.isdir(src): shutil.copytree(src, os.path.join(OUT, 'fh/Library/Preferences/kicad', v), dirs_exist_ok=True)
tmp = tempfile.mkdtemp(prefix='bpk-', dir='/tmp')   # a Unix socket path is at most 104 bytes on macOS
port = 3893
env = dict(os.environ, HOME=os.path.join(OUT, 'fh'), TMPDIR=tmp, BACKPLANE_KICAD_CLI=os.path.join(RT, 'bin/kicad-cli'),
           BACKPLANE_AGENTS=os.path.join(OUT, 'bin'), BACKPLANE_NO_UPDATE='1', BACKPLANE_KICAD_DEBUG='1', BACKPLANE_KICAD_EMBED_TRACE=os.path.join(OUT, 'kicad-embed-trace.log'), FAKE_LOG=os.path.join(OUT, 'standin.log'))
hub = subprocess.Popen([HUB, '--home', os.path.join(OUT, 'home'), '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'],
                       env=env, stdout=open(os.path.join(OUT, 'hub.log'), 'w'), stderr=subprocess.STDOUT); procs.append(hub)
wait_for(lambda: subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', f'http://127.0.0.1:{port}/hello']).returncode == 0, 30)
script = [{"select": 0}, {"wait": 2500}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 900000}]
json.dump(script, open(os.path.join(OUT, 'script.json'), 'w'))
cenv = dict(os.environ, HOME=os.path.join(OUT, 'fh'), BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1', BACKPLANE_KICAD_DEBUG='1')
client = subprocess.Popen([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', os.path.join(OUT, 'home/token'), '--script', os.path.join(OUT, 'script.json'), '--exit-after', '900'],
                          env=cenv, stdout=open(os.path.join(OUT, 'client.log'), 'w'), stderr=subprocess.STDOUT); procs.append(client)


def shot(name): subprocess.run(['screencapture', '-x', os.path.join(OUT, name + '.png')])
def hub_log(): return open(os.path.join(OUT, 'hub.log'), errors='replace').read()
def client_log(): return open(os.path.join(OUT, 'client.log'), errors='replace').read()


def last_push():
    """The pane's rect (screen points) and visibility the client last pushed to KiCad."""
    v = None
    for l in client_log().splitlines():
        if '"kicad_host_push"' in l:
            try: j = json.loads(l.split('GPUI_LOG ', 1)[1]); v = (tuple(j['kicad_host_push']), j['visible'])
            except Exception: pass
    return v


def rects():
    """The client's control rects (BACKPLANE_GPUI_RECTS: lists of {id, x, y, w, h} in window points, from the content's top-left), latest."""
    r = {}
    for l in client_log().splitlines():
        if 'GPUI_METRIC ' in l and '"rects"' in l:
            try:
                for e in json.loads(l.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                    if isinstance(e, dict) and 'id' in e: r[e['id']] = e
            except Exception: pass
    return r


def click(x, y, button='left'):
    down, up, b = (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp, Quartz.kCGMouseButtonLeft) if button == 'left' else (Quartz.kCGEventRightMouseDown, Quartz.kCGEventRightMouseUp, Quartz.kCGMouseButtonRight)
    pt = Quartz.CGPointMake(x, y)
    for k in (Quartz.kCGEventMouseMoved, down, up):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(None, k, pt, b)); time.sleep(0.05)


def osa(src):
    r = subprocess.run(['osascript', '-e', src], capture_output=True, text=True)
    if r.returncode != 0: print('osascript: ' + r.stderr.strip()[:300], flush=True)
    return r.stdout.strip()
def win_of(pid):
    """The client's main window for System Events: its largest (after a palette or a popup, `first window` was another one)."""
    sizes = osa(f'tell application "System Events" to get size of every window of (first process whose unix id is {pid})')
    nums = [int(float(v)) for v in sizes.replace('{', '').replace('}', '').split(',') if v.strip()] if sizes else []
    areas = [nums[i] * nums[i + 1] for i in range(0, len(nums) - 1, 2)]
    i = (areas.index(max(areas)) + 1) if areas else 1
    return f'(window {i} of (first process whose unix id is {pid}))'


try:
    cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
    check(bool(cw), 'the client window is on screen')
    # the client opens 1400x900, larger than the runner's 1024x768 display: put it on screen whole, so every click lands
    osa(f'tell application "System Events" to set position of {win_of(client.pid)} to {{0, 25}}')
    osa(f'tell application "System Events" to set size of {win_of(client.pid)} to {{1000, 700}}')
    time.sleep(1.5)
    cw = next((w for w in wins() if w[1] == client.pid and w[0] == cw[0]), cw)
    print(f'client window now {cw and cw[3]}', flush=True)
    # ---- Edit: KiCad's frame over the pane, directly above our window ----
    def kicad_frame():
        order = wins(); ours = next((i for i, w in enumerate(order) if cw and w[0] == cw[0]), None)
        if ours is None: return None
        k = [w for w in order[:ours] if w[1] not in (client.pid, hub.pid) and 'kicad' in w[2].lower()]
        return k[0] if k and order.index(k[0]) == ours - 1 else None
    t0 = time.time()
    f = wait_for(kicad_frame, 180, 0.25)
    RES['edit_to_frame_s'] = round(time.time() - t0, 1)
    shot('01-edit')
    cw = next((w for w in wins() if cw and w[0] == cw[0]), cw)
    inside = f and cw and cw[3][0] <= f[3][0] and cw[3][1] <= f[3][1] and f[3][0] + f[3][2] <= cw[3][0] + cw[3][2] + 1 and f[3][1] + f[3][3] <= cw[3][1] + cw[3][3] + 1
    check(bool(f) and bool(inside), f'Edit: KiCad\'s board editor is over the pane, directly above Backplane ({f and f[3]} in {cw and cw[3]}, {RES["edit_to_frame_s"]} s)')
    if not f: raise SystemExit
    kpid = f[1]
    # ---- the frame's rect is the pane's, within 1 px, at first show and after each layout change ----
    def matches(tag):
        def ok():
            p, k = last_push(), kicad_frame()
            return p and p[1] and k and all(abs(a - b) <= 1 for a, b in zip(k[3], p[0])) and (k[3], p[0])
        got = wait_for(ok, 4, 0.2)
        p, k = last_push(), kicad_frame()
        check(bool(got), f'{tag}: KiCad\'s frame is the pane\'s rect within 1 px ({k and k[3]} vs {p and p[0]})')
    matches('at first show')
    # ---- Edit, then typing with no click ----
    front = wait_for(lambda: active(kpid), 5)
    check(bool(front), 'after Edit, with no click, KiCad is the frontmost app (it has the keyboard)')
    # the canvas takes the keys once KiCad's focused show has run (a person types a moment after Edit): Cmd+A until it acts, 4 s at most
    n0 = hub_log().count('ET_SELECTION_CHANGED'); t0 = time.time(); took = None
    while time.time() - t0 < 4 and took is None:
        key(K_A, CMD); time.sleep(0.4)
        if hub_log().count('ET_SELECTION_CHANGED') > n0: took = round(time.time() - t0, 1)
    key(K_ESC); time.sleep(0.6)
    RES['edit_to_canvas_keys_s'] = took
    check(took is not None, f'typing with no click reaches the canvas after Edit ({took} s after the frame showed)')
    # ---- Cmd+K in KiCad reaches Backplane ----
    k0 = client_log().count('"kicad_key"')
    key(K_K, CMD)
    got = wait_for(lambda: client_log().count('"kicad_key"') > k0, 5)
    check(bool(got), 'Cmd+K pressed in KiCad reaches Backplane (the client takes the reserved chord)')
    time.sleep(1); shot('02-cmd-k')
    check(bool(wait_for(lambda: active(client.pid), 3)), 'with the palette open, Backplane has the keyboard')
    # the palette was opened from KiCad and closed with nothing chosen: the keyboard goes back to KiCad (lane B's e9af1ffe5, every platform),
    # and a second Escape is KiCad's (it keeps the viewer open)
    key(K_ESC)
    check(bool(wait_for(lambda: active(kpid), 4)), 'Escape closes the palette and the keyboard is back in KiCad')
    n0 = hub_log().count('ET_SELECTION_CHANGED'); key(K_A, CMD)
    check(bool(wait_for(lambda: hub_log().count('ET_SELECTION_CHANGED') > n0, 4)), 'and KiCad\'s canvas takes keys again (Cmd+A)')
    key(K_ESC); time.sleep(0.8)
    check(bool(kicad_frame()), 'a second Escape stays in KiCad: the viewer stays open')
    p = last_push()
    board_spot = lambda: (p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9)
    # a click on the board gives KiCad the keyboard; its canvas takes keys (Cmd+A, then a plain Escape inside KiCad)
    click(*board_spot())
    check(bool(wait_for(lambda: active(kpid), 4)), 'a click on the board gives KiCad the keyboard')
    n0 = hub_log().count('ET_SELECTION_CHANGED'); key(K_A, CMD); time.sleep(0.6); key(K_ESC)
    check(bool(wait_for(lambda: hub_log().count('ET_SELECTION_CHANGED') > n0, 4)), 'and its canvas takes keys (Cmd+A, Escape)')
    # a KiCad dialog (Find, Cmd+F from the canvas) gets the keyboard, and Escape closes it
    d0, c0 = hub_log().count('ET_DIALOG_OPENED'), hub_log().count('ET_DIALOG_CLOSED')
    key(K_F, CMD)
    opened = wait_for(lambda: hub_log().count('ET_DIALOG_OPENED') > d0, 5)
    check(bool(opened) and active(kpid), 'a KiCad dialog (Find) opens with the keyboard in it')
    time.sleep(0.5); shot('02c-dialog')
    # Find's own Close button (Cmd+W would close the board editor itself), then Find again: Escape closes it
    def find_dialog():
        return next((w for w in wins() if w[1] == kpid and w[0] != f[0] and w[3][2] < 900), None)
    fd = wait_for(find_dialog, 3)
    if fd: click(fd[3][0] + fd[3][2] - 65, fd[3][1] + 136)
    check(bool(wait_for(lambda: hub_log().count('ET_DIALOG_CLOSED') > c0, 4)), 'its Close button closes it')
    d1, c1 = hub_log().count('ET_DIALOG_OPENED'), hub_log().count('ET_DIALOG_CLOSED')
    p = last_push(); click(*board_spot()); time.sleep(0.6); key(K_F, CMD)
    check(bool(wait_for(lambda: hub_log().count('ET_DIALOG_OPENED') > d1, 5)) and active(kpid), 'Find again, with the keyboard in it')
    time.sleep(0.6); key(K_ESC)
    esc = wait_for(lambda: hub_log().count('ET_DIALOG_CLOSED') > c1, 4)
    check(bool(esc), 'Escape closes it')
    if not esc:
        fd = find_dialog()
        if fd: click(fd[3][0] + fd[3][2] - 65, fd[3][1] + 136); time.sleep(0.6)
    # a click in Backplane's own chrome (a neutral spot: the thread's timeline) gives it the keyboard back
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    click(cw[3][0] + 200, cw[3][1] + 28 + 300)
    check(bool(wait_for(lambda: active(client.pid), 4)), 'a click in Backplane\'s chrome gives it the keyboard back')
    click(*board_spot())
    check(bool(wait_for(lambda: active(kpid), 4)), 'a click on the board gives KiCad the keyboard again')
    # ---- our window moved: the frame follows ----
    before = kicad_frame(); wb = next((w for w in wins() if w[0] == cw[0]), cw)
    osa(f'tell application "System Events" to set position of {win_of(client.pid)} to {{{wb[3][0] + 60}, {wb[3][1] + 40}}}')
    moved = wait_for(lambda: (k := kicad_frame()) and before and (k[3][0] - before[3][0], k[3][1] - before[3][1]) == (60, 40) and k, 5)
    print(f'client window {wb[3]} -> {next((w for w in wins() if w[0] == cw[0]), cw)[3]}', flush=True)
    check(bool(moved), f'Backplane\'s window moved: KiCad\'s frame follows it ({before and before[3]} -> {(kicad_frame() or [None]*4)[3]})')
    matches('after the move')
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    osa(f'tell application "System Events" to set size of {win_of(client.pid)} to {{{cw[3][2] - 120}, {cw[3][3] - 60}}}'); time.sleep(1.5)
    matches('after a resize')
    # Backplane's own keys: a click in its window outside the pane (the sidebar's edge) gives them back first
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    click(cw[3][0] + 20, cw[3][1] + cw[3][3] - 40); time.sleep(0.6)
    key(K_B, CMD); time.sleep(1.5)
    matches('after the sidebar is folded')
    key(K_B, CMD); time.sleep(1.5)
    matches('after the sidebar is back')
    # the Checks panel, by its chip in the editor's strip (Alt+Shift+C types "Ç" on a Mac)
    chip = rects().get('kicad:checks-chip')
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    if chip: click(cw[3][0] + chip['x'] + chip['w'] / 2, cw[3][1] + 28 + chip['y'] + chip['h'] / 2); time.sleep(2)
    matches(f'after a side panel (Checks) opens ({bool(chip)})')
    if chip: click(cw[3][0] + chip['x'] + chip['w'] / 2, cw[3][1] + 28 + chip['y'] + chip['h'] / 2); time.sleep(1.5)
    # ---- the reserved chords from inside KiCad: Cmd+J, and Ctrl+Tab (the physical Ctrl key: Cmd+Tab is macOS's) ----
    p = last_push()
    if p: click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.6)
    j0 = client_log().count('"kicad_key"')
    key(K_J, CMD)
    check(bool(wait_for(lambda: client_log().count('"kicad_key"') > j0, 5)), 'Cmd+J pressed in KiCad reaches Backplane')
    key(K_J, CMD); time.sleep(0.6)
    if p: click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.6)
    s0 = hub_log().count('FK_SCH')
    key(K_TAB, CTRL)
    check(bool(wait_for(lambda: hub_log().count('FK_SCH') > s0, 10)), 'Ctrl+Tab pressed in KiCad goes to the next editor tab (the schematic)')
    time.sleep(2); shot('02b-ctrl-tab')
    key(K_TAB, CTRL | SHIFT); time.sleep(3)
    f = kicad_frame() or f
    # ---- full screen: hidden, then back over the pane ----
    osa(f'tell application "System Events" to set value of attribute "AXFullScreen" of {win_of(client.pid)} to true')
    time.sleep(3)
    hidden = not any(w[0] == f[0] for w in wins())
    shot('03-full-screen')
    check(hidden, 'Backplane full screen: KiCad\'s frame leaves the screen')
    line = wait_for(lambda: rects().get('kicad:pop-full'), 5)
    check(bool(line), 'full screen: the pane says so and offers Pop out')
    osa(f'tell application "System Events" to set value of attribute "AXFullScreen" of {win_of(client.pid)} to false')
    back = wait_for(kicad_frame, 8, 0.25)
    check(bool(back), 'out of full screen: KiCad\'s frame is back over the pane, above Backplane')
    shot('04-back')
    matches('after full screen')
    # ---- Pop out and back in ----
    before_wins = {w[0] for w in wins() if w[1] == client.pid}
    # the window back to its first size (the strip's buttons at a narrow width may sit past its edge)
    osa(f'tell application "System Events" to set size of {win_of(client.pid)} to {{1000, 700}}'); time.sleep(1.5)
    pop = rects().get('kicad:popout')
    print(f'pop-out button {pop} in window {next((w for w in wins() if w[0] == cw[0]), cw)[3]}', flush=True)
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    if pop:
        # Backplane first (the first click on an inactive app's window only activates it, as for a person), then Pop out
        click(cw[3][0] + 200, cw[3][1] + 28 + 300); time.sleep(0.6)
        click(cw[3][0] + pop['x'] + pop['w'] / 2, cw[3][1] + 28 + pop['y'] + pop['h'] / 2)
    newwin = wait_for(lambda: next((w for w in wins() if w[1] == client.pid and w[0] not in before_wins), None), 10)
    check(bool(pop) and bool(newwin), f'Pop out opens the editor in a window of its own ({pop})')
    if newwin:
        def above(n):
            order = wins(); i = next((k for k, w in enumerate(order) if w[0] == n), None)
            return i and order[i - 1][2].lower().find('kicad') >= 0 and order[i - 1]
        check(bool(wait_for(lambda: above(newwin[0]), 8, 0.25)), 'popped out: KiCad\'s frame is over the new window')
        shot('05-popped')
        # its close button (closing the popped-out window puts the editor back in the pane)
        osa(f'tell application "System Events" to tell (first process whose unix id is {client.pid}) to click (first button of (first window whose position is {{{newwin[3][0]}, {newwin[3][1]}}}) whose subrole is "AXCloseButton")')
        time.sleep(3)
        check(bool(wait_for(kicad_frame, 8, 0.25)), 'popped back in: KiCad\'s frame is over the pane again')
    # ---- Cmd+W in KiCad: the pane follows KiCad (lane B's 47b00d199) ----
    K_W, K_RETURN = 13, 36
    board = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb')
    def edit_again():
        e = rects().get('ui:Edit'); cw2 = next((w for w in wins() if w[0] == cw[0]), cw)
        if e: click(cw2[3][0] + e['x'] + e['w'] / 2, cw2[3][1] + 28 + e['y'] + e['h'] / 2)
        return wait_for(kicad_frame, 60, 0.25)
    def unsaved_edit():
        p = last_push(); click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.6)
        key(K_A, CMD); time.sleep(0.5); key(K_DEL); time.sleep(1.2)
    # nothing unsaved: Cmd+W closes the editor, the pane goes back to View, Edit opens a fresh one
    p = last_push(); click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.6)
    fr = kicad_frame(); key(K_W, CMD)
    gone = wait_for(lambda: not any(w[0] == (fr or [0])[0] for w in wins()), 6)
    check(bool(gone) and bool(wait_for(lambda: rects().get('ui:Edit'), 6)), 'Cmd+W with nothing unsaved: the editor closes and the pane is back to View (Edit offered)')
    check(bool(edit_again()), 'Edit opens a fresh editor over the pane')
    # an unsaved edit, then Cancel in KiCad's prompt (Escape): the editor stays
    unsaved_edit(); before = open(board, 'rb').read()
    key(K_W, CMD); time.sleep(1.5); shot('07-close-prompt'); key(K_ESC); time.sleep(1.5)
    check(bool(kicad_frame()) and open(board, 'rb').read() == before, 'Cmd+W with an unsaved edit, then Cancel: the editor stays, nothing written')
    # the same, then Save (Return, the prompt's default): written, closed, the pane in View
    key(K_W, CMD); time.sleep(1.5); key(K_RETURN)
    saved = wait_for(lambda: open(board, 'rb').read() != before, 8)
    check(bool(saved) and bool(wait_for(lambda: not kicad_frame(), 6)) and bool(wait_for(lambda: rects().get('ui:Edit'), 6)), 'Cmd+W with an unsaved edit, then Save: written, the editor closes, the pane is back to View')
    check(bool(edit_again()), 'and Edit opens it again')
    # ---- quitting Backplane with unsaved changes: KiCad keeps them, nothing is written, Edit brings them back ----
    board = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb'); before_bytes = open(board, 'rb').read()
    p = last_push()
    if p: click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.6)
    m0 = hub_log().count('ET_MODIFIED_CHANGED')
    key(K_A, CMD); time.sleep(0.6); key(K_DEL); time.sleep(1.5)
    check(bool(wait_for(lambda: hub_log().count('ET_MODIFIED_CHANGED') > m0, 5)), 'the person\'s edit leaves the board unsaved in KiCad')
    fr = kicad_frame()
    client.terminate(); client.wait(10)
    check(bool(fr) and bool(wait_for(lambda: not any(w[0] == fr[0] for w in wins()), 3, 0.05)), 'Backplane quit: KiCad\'s frame leaves the screen at once')
    check(subprocess.run(['kill', '-0', str(kpid)]).returncode == 0, 'KiCad keeps running with the unsaved change (the hub keeps it)')
    check(open(board, 'rb').read() == before_bytes, 'nothing was written to the board on disk')
    client = subprocess.Popen([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', os.path.join(OUT, 'home/token'), '--script', os.path.join(OUT, 'script.json'), '--exit-after', '300'],
                              env=cenv, stdout=open(os.path.join(OUT, 'client2.log'), 'w'), stderr=subprocess.STDOUT); procs.append(client)
    cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
    check(bool(wait_for(kicad_frame, 60, 0.25)), 'Backplane again, Edit: the same KiCad is back over the pane')
    check(bool(wait_for(lambda: (k := kicad_frame()) and k[1] == kpid, 5)) and open(board, 'rb').read() == before_bytes, 'the unsaved change is still in the same KiCad, never written to disk')
    shot('06-reopened')
finally:
    for p in procs:
        try: p.terminate()
        except Exception: pass
    time.sleep(2)
    for p in procs:
        try: p.kill()
        except Exception: pass
    left = subprocess.run(['pgrep', '-f', RT + '/'], capture_output=True, text=True).stdout.strip()
    if left: subprocess.run(['pkill', '-f', RT + '/'])
    json.dump({'failures': FAILS, 'results': RES, 'left_running': left}, open(os.path.join(OUT, 'summary.json'), 'w'), indent=2)
    print(json.dumps({'failures': FAILS, 'results': RES, 'left_running': left}, indent=2))
sys.exit(1 if FAILS else 0)
