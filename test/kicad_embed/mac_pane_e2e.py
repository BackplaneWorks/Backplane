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
import json, os, shutil, socket, subprocess, sys, tempfile, time

# The driver: `quartz` (the e2e-macos runner: Quartz events, osascript, screencapture; TCC granted there) or `vnc` (the Mac mini gate:
# the harness runs in h's GUI session as a LaunchAgent, its input and screen go through a loopback Screen Sharing relay
# (mac/vnc_relay.py) and the window list through the gate helper (mac/gate_helper.m): no Accessibility or Screen Recording grant)
DRIVER = os.environ.get('MAC_DRIVER', 'quartz')
# event flags as Quartz numbers (kCGEventFlagMask*), so neither driver needs the other's modules
CMD, CTRL, SHIFT, ALT = 0x100000, 0x40000, 0x20000, 0x80000
# macOS virtual key codes
K_A, K_K, K_ESC, K_B, K_J, K_C, K_TAB, K_DEL, K_F, K_COMMA = 0, 40, 53, 11, 38, 8, 48, 51, 3, 43
K_FDEL = 117  # forward delete (fn+delete)

if DRIVER == 'quartz':
    import Quartz
    from AppKit import NSRunningApplication

    def active(pid):
        """The app is the active one (an accessory KiCad is active while it holds the keyboard, though NSWorkspace's frontmost app, the
        menu bar's owner, stays Backplane)."""
        a = NSRunningApplication.runningApplicationWithProcessIdentifier_(pid)
        return bool(a and a.isActive())

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

    def click(x, y, button='left'):
        down, up, b = (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp, Quartz.kCGMouseButtonLeft) if button == 'left' else (Quartz.kCGEventRightMouseDown, Quartz.kCGEventRightMouseUp, Quartz.kCGMouseButtonRight)
        pt = Quartz.CGPointMake(x, y)
        for k in (Quartz.kCGEventMouseMoved, down, up):
            Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(None, k, pt, b)); time.sleep(0.05)

    def helper(req):
        # the gate helper's `windows` answer, from Quartz: every on-screen window, any layer, front to back (no titles)
        if req != 'windows': return None
        out = []
        for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements, Quartz.kCGNullWindowID):
            b = w['kCGWindowBounds']
            out.append({'number': int(w['kCGWindowNumber']), 'pid': int(w['kCGWindowOwnerPID']), 'owner': w.get('kCGWindowOwnerName', ''),
                        'layer': int(w.get('kCGWindowLayer', 0)), 'bounds': [b['X'], b['Y'], b['Width'], b['Height']]})
        return out

    def _mouse(kind, x, y):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(None, kind, Quartz.CGPointMake(x, y), Quartz.kCGMouseButtonLeft)); time.sleep(0.03)

    def relay(**r):
        # the VNC relay's ops, with Quartz events (the checks are written once for both drivers)
        op = r.get('op')
        if op == 'move': _mouse(Quartz.kCGEventMouseMoved, r['x'], r['y'])
        elif op == 'click': click(r['x'], r['y'])
        elif op == 'drag':
            _mouse(Quartz.kCGEventLeftMouseDown, r['x0'], r['y0']); n = 20
            for i in range(1, n + 1): _mouse(Quartz.kCGEventLeftMouseDragged, r['x0'] + (r['x1'] - r['x0']) * i / n, r['y0'] + (r['y1'] - r['y0']) * i / n); time.sleep(r.get('seconds', 0.6) / n)
            _mouse(Quartz.kCGEventLeftMouseUp, r['x1'], r['y1'])
        elif op == 'text':
            for ch in r['text']:
                for down in (True, False):
                    e = Quartz.CGEventCreateKeyboardEvent(None, 0, down); Quartz.CGEventKeyboardSetUnicodeString(e, 1, ch); Quartz.CGEventPost(Quartz.kCGHIDEventTap, e); time.sleep(0.02)
        elif op == 'png':
            if not r.get('rect'): raise ValueError('png needs a rect (the test windows only): the whole screen is never saved')
            x, y, w, h = (round(v) for v in r['rect']); subprocess.run(['screencapture', '-x', '-R', f'{x},{y},{w},{h}', r['path']])
        return {'ok': True}

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

    def win_move(pid, x, y): osa(f'tell application "System Events" to set position of {win_of(pid)} to {{{x}, {y}}}')
    def win_size(pid, w, h): osa(f'tell application "System Events" to set size of {win_of(pid)} to {{{w}, {h}}}')
    def win_full(pid, on): osa(f'tell application "System Events" to set value of attribute "AXFullScreen" of {win_of(pid)} to {"true" if on else "false"}')
    def win_close(pid, bounds):
        osa(f'tell application "System Events" to tell (first process whose unix id is {pid}) to click (first button of (first window whose position is {{{bounds[0]}, {bounds[1]}}}) whose subrole is "AXCloseButton")')
else:
    GATE = os.path.expanduser(os.environ.get('MAC_GATE_DIR', '~/bp-gate-m'))

    def _ask(path, line):
        c = socket.socket(socket.AF_UNIX); c.settimeout(60); c.connect(path)
        c.sendall((line + '\n').encode()); r = c.makefile().readline(); c.close()
        return r

    def helper(req): return json.loads(_ask(os.path.join(GATE, 'helper.sock'), req) or 'null')

    def relay(**op):
        r = json.loads(_ask(os.path.join(GATE, 'vnc.sock'), json.dumps(op)))
        if not r.get('ok'): print(f'relay {op.get("op")}: {r.get("error")}', flush=True)
        return r

    def active(pid): return bool((helper(f'active {pid}') or {}).get('active'))

    def wins():
        return [(int(w['number']), int(w['pid']), w['owner'], tuple(round(v) for v in w['bounds'])) for w in helper('windows') or [] if w['layer'] == 0]

    # macOS virtual key codes the checks use, to the keysym names the relay types (Screen Sharing: Super is Command)
    KEYNAME = {1: 's', 0: 'a', 40: 'k', 53: 'escape', 11: 'b', 38: 'j', 8: 'c', 48: 'tab', 51: 'backspace', 3: 'f', 43: ',', 13: 'w', 36: 'return', 117: 'delete'}

    def key(code, flags=0):
        mods = [m for m, bit in (('ctrl', CTRL), ('alt', ALT), ('shift', SHIFT), ('cmd', CMD)) if flags & bit]
        relay(op='chord', combo='+'.join(mods + [KEYNAME[code]]))

    # windows by hand, as a person moves them (the title bar's empty part left of the title, the bottom-right corner, the green and red
    # buttons: 13 pt below the top, at 14, 36 and 58 pt from the left on macOS 26)
    def _main(pid): return max((w for w in wins() if w[1] == pid), key=lambda w: w[3][2] * w[3][3], default=None)

    def win_move(pid, x, y):
        b = _main(pid)[3]; _guard(b[0] + 120, b[1] + 12, 'a drag'); relay(op='drag', x0=b[0] + 120, y0=b[1] + 12, x1=x + 120, y1=y + 12, seconds=0.8); time.sleep(0.5)

    def win_size(pid, w, h):
        b = _main(pid)[3]; _guard(b[0] + b[2] - 2, b[1] + b[3] - 2, 'a drag'); relay(op='drag', x0=b[0] + b[2] - 2, y0=b[1] + b[3] - 2, x1=b[0] + w - 2, y1=b[1] + h - 2, seconds=0.8); time.sleep(0.5)

    def win_full(pid, on):
        if on:
            # the window active first (its title bar): a click on an inactive window's green button only activates it
            b = _main(pid)[3]; _guard(b[0] + 120, b[1] + 12, 'a click'); relay(op='click', x=b[0] + 120, y=b[1] + 12); time.sleep(0.6)
            _guard(b[0] + 58, b[1] + 13, 'a click'); relay(op='click', x=b[0] + 58, y=b[1] + 13)
        else:
            # as a person leaves full screen: the pointer at the screen's top shows the title bar, then its green button
            # (Ctrl+Cmd+F needs an Exit Full Screen menu item; the client has none)
            # (the menu bar takes the top 24 pt there: the title bar it reveals has its buttons about 44 pt down, green about 60 pt in;
            # a click at 13 pt opened the app menu at Quit)
            for gx, gy in ((60, 44), (58, 40), (62, 48)):
                relay(op='move', x=400, y=1); time.sleep(1.8); relay(op='move', x=gx, y=gy); time.sleep(0.8)
                n0 = client_log().count('"kicad_mac_full_screen":false')
                relay(op='click', x=gx, y=gy)
                if wait_for(lambda: client_log().count('"kicad_mac_full_screen":false') > n0, 4): break
                relay(op='chord', combo='escape'); time.sleep(0.8)  # whatever opened instead (a menu) goes

    def win_close(pid, bounds): _guard(bounds[0] + 14, bounds[1] + 13, 'a click'); relay(op='click', x=bounds[0] + 14, y=bounds[1] + 13)


# ---- shared by both drivers: no input on the person's windows, pictures of our windows only ----
# never a click on the person's windows: the front-most window at the point (any layer) must be this run's Backplane or KiCad,
# else the run stops (the Mac mini gate, Nov 16: a lost window lookup sent clicks to the person's Chrome and Music)
def _our_pids():
    ps = subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines()
    return {client.pid} | {int(l.split()[0]) for l in ps if OUT in l and 'kicad' in l.lower()}
def _guard(x, y, what):
    # the client gone (its --exit-after, a crash): its window is gone too, and what is under the point now is the person's
    if client.poll() is not None: raise SystemExit(f'REFUSED {what} at ({round(x)}, {round(y)}): the client has exited ({client.returncode})')
    mine = _our_pids()
    for w in helper('windows') or []:
        bx, by, bw, bh = w['bounds']
        # the Dock's own full-screen window (transparent, under its bar: the e2e-macos runner lists it over everything) takes no click;
        # its bar, a smaller window of its own, still counts
        if w.get('owner') == 'Dock' and bw >= 1000 and bh >= 600: continue
        if bx <= x < bx + bw and by <= y < by + bh and w.get('layer', 0) < 1000:
            if int(w['pid']) in mine: return
            raise SystemExit(f'REFUSED {what} at ({round(x)}, {round(y)}): the window there is {w.get("owner", "?")}\'s, not this run\'s')
    raise SystemExit(f'REFUSED {what} at ({round(x)}, {round(y)}): no window of this run there')
def shot_to(path):
    # only the test's own windows (Backplane's and KiCad's), never the screen: their union's bounds
    ours = [w for w in wins() if w[1] == client.pid or 'kicad' in w[2].lower() and OUT in ' '.join(subprocess.run(['ps', '-o', 'command=', '-p', str(w[1])], capture_output=True, text=True).stdout.split()[:1] or [''])]
    ours = ours or [w for w in wins() if w[1] == client.pid]
    if not ours: return
    x0 = min(w[3][0] for w in ours); y0 = min(w[3][1] for w in ours)
    x1 = max(w[3][0] + w[3][2] for w in ours); y1 = max(w[3][1] + w[3][3] for w in ours)
    relay(op='png', path=path, rect=[x0, y0, x1 - x0, y1 - y0])

if DRIVER == 'quartz':
    _qclick = click
    def click(x, y, button='left'): _guard(x, y, 'a click'); _qclick(x, y, button)
else:
    def click(x, y, button='left'): _guard(x, y, 'a click'); relay(op='click', x=x, y=y, button=1 if button == 'left' else 3)


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


# ---- the project, the hub's home, KiCad's settings where KiCad on macOS reads them ----
os.makedirs(OUT, exist_ok=True)
subprocess.run([sys.executable, os.path.join(REPO, 'test/kicad_embed/lib/seed_project.py'), OUT, REPO], check=True)
for v in ('10.0', '10.99'):
    src = os.path.join(OUT, 'fh/.config/kicad', v)
    if os.path.isdir(src): shutil.copytree(src, os.path.join(OUT, 'fh/Library/Preferences/kicad', v), dirs_exist_ok=True)
if os.environ.get('MAC_MODE') != 'libs':
    with open(os.path.join(OUT, 'bin/claude'), 'w') as f:
        f.write('#!/bin/sh\nexec /usr/bin/python3 ' + os.path.join(REPO, 'test/kicad_embed/mac/standin_agent.py') + ' "$@"\n')
    os.chmod(os.path.join(OUT, 'bin/claude'), 0o755)
if os.environ.get('MAC_MODE') in ('autosave', 'recover'):
    for v in ('10.99',):
        kc = os.path.join(OUT, 'fh/Library/Preferences/kicad', v, 'kicad_common.json')
        j = json.load(open(kc)) if os.path.isfile(kc) else {}
        j.setdefault('system', {})['local_history_debounce'] = 3; j.setdefault('auto_backup', {})['enabled'] = True
        os.makedirs(os.path.dirname(kc), exist_ok=True); json.dump(j, open(kc, 'w'))
if os.environ.get('MAC_MODE') == 'lock':
    # another live KiCad has the project open: a stand-in writes KiCad's lock JSON and holds its flock (lane B's embed_foreign_lock_e2e)
    LCK = os.path.join(OUT, 'proj', '~ecc83-pp.kicad_pro.lck')
    holder = subprocess.Popen([sys.executable, '-c', 'import fcntl,json,sys,time\nf=open(sys.argv[1],"w");json.dump({"hostname":sys.argv[2],"username":sys.argv[3],"token":"other"},f);f.flush()\nfcntl.flock(f.fileno(),fcntl.LOCK_EX)\ntime.sleep(3600)',
                               LCK, os.uname().nodename.split('.')[0], os.environ.get('USER', 'h')])
    procs.append(holder); time.sleep(1)
if os.environ.get('MAC_MODE') == 'update':
    # the starting state: bp.12.5 installed by the hub's installer (placed where it puts it; an APFS clone of the published runtime)
    _ver = json.load(open(os.path.join(RT, 'manifest.json')))['version']
    _m = os.path.join(OUT, 'home', 'kicad', _ver)
    if not os.path.isdir(_m):
        os.makedirs(os.path.dirname(_m), exist_ok=True); subprocess.run(['cp', '-cR', RT, _m], check=True)
    RT = _m
if os.environ.get('FRESH_AGENT') == 'during':
    # lane B's embed_fresh_edit_e2e: KiCad's launcher 12 s late, in a clone of the runtime where the hub's installer puts it
    _ver = json.load(open(os.path.join(RT, 'manifest.json')))['version']
    _m = os.path.join(OUT, 'home', 'kicad', _ver)
    if not os.path.isdir(_m):
        os.makedirs(os.path.dirname(_m), exist_ok=True); subprocess.run(['cp', '-cR', RT, _m], check=True)
    _l = os.path.join(_m, 'bin', 'kicad'); os.rename(_l, _l + '.real')
    open(_l, 'w').write('#!/bin/sh\nsleep 12\nexec "$(dirname "$0")/kicad.real" "$@"\n'); os.chmod(_l, 0o755)
    RT = _m
if os.environ.get('MAC_MODE') == 'libs':
    for root in (os.path.join(OUT, 'fh/.config/kicad'), os.path.join(OUT, 'fh/Library/Preferences/kicad')):
        for dp, _, fs in os.walk(root):
            for t in ('sym-lib-table', 'fp-lib-table', 'design-block-lib-table'):
                if t in fs: os.unlink(os.path.join(dp, t))
    # the fresh design (lib/libgate_design.py) is the pane's own project, so the agent edits the documents open in the embedded KiCad and
    # the editor's checks run there (in a subfolder project the hub served them headless: GetMarkers is not available there)
    sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/lib'))
    import libgate_design as LG
    _p = os.path.join(OUT, 'proj'); _seed = os.path.join(OUT, 'seed'); os.makedirs(_seed, exist_ok=True)
    for _e in ('kicad_pro', 'kicad_pcb', 'kicad_sch'): shutil.copy(os.path.join(_p, f'ecc83-pp.{_e}'), os.path.join(_seed, f'ecc83-pp.{_e}'))
    _gd = os.path.join(OUT, 'libgate-new'); LG.make_project(_gd, *(os.path.join(_seed, f'ecc83-pp.{_e}') for _e in ('kicad_pro', 'kicad_pcb', 'kicad_sch')))
    for _e in ('kicad_pcb', 'kicad_sch'): shutil.copy(os.path.join(_gd, f'libgate.{_e}'), os.path.join(_p, f'ecc83-pp.{_e}'))
    subprocess.run(['git', '-C', _p, 'commit', '-qam', 'libs gate: a new design'], capture_output=True)
    subprocess.run(['git', '-C', _p, 'push', '-q', 'origin', 'HEAD'], capture_output=True)
    # the runtime where the hub's installer puts it, <home>/kicad/<version> (L1: the hub's managed one): an APFS clone, instant, no space
    _ver = json.load(open(os.path.join(RT, 'manifest.json')))['version']
    _managed = os.path.join(OUT, 'home', 'kicad', _ver)
    if not os.path.isdir(_managed):
        os.makedirs(os.path.dirname(_managed), exist_ok=True)
        subprocess.run(['cp', '-cR', RT, _managed], check=True)
    RT = _managed
    # the Mac has no bun for the stand-in agent: the Python one, same mcpcall protocol
    with open(os.path.join(OUT, 'bin/claude'), 'w') as f:
        f.write('#!/bin/sh\nexec /usr/bin/python3 ' + os.path.join(REPO, 'test/kicad_embed/mac/standin_agent.py') + ' "$@"\n')
    os.chmod(os.path.join(OUT, 'bin/claude'), 0o755)
if os.environ.get('MAC_MODE') == 'narrow':
    # a fresh profile's own panels (Climb B: the seed's pcbnew.json and eeschema.json hide them; KiCad's defaults are what a person meets)
    for root in (os.path.join(OUT, 'fh/.config/kicad'), os.path.join(OUT, 'fh/Library/Preferences/kicad')):
        for dp, _, fs in os.walk(root):
            for t in ('pcbnew.json', 'eeschema.json'):
                if t in fs: os.unlink(os.path.join(dp, t))
DOCS_DIR = os.path.expanduser('~/Documents/bp-gate-docs-test')
if os.path.isdir(DOCS_DIR): shutil.rmtree(DOCS_DIR)  # left by a run that died
if os.environ.get('MAC_MODE') == 'docs':
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lib')); from project_copy import copy_project
    copy_project(os.path.join(OUT, 'proj'), os.path.join(DOCS_DIR, 'proj'), symlinks=True)
    ev = os.path.join(OUT, 'home', 'events.jsonl')
    open(ev, 'w').write(open(ev).read().replace(os.path.join(OUT, 'proj'), os.path.join(DOCS_DIR, 'proj')))
    # a fresh copy of the test hub at a path of its own: macOS asks this binary anew, nothing of the person's is reset
    _hc = os.path.join(OUT, 'hub-' + os.environ.get('DOCS_ANSWER', 'allow')); shutil.copy2(HUB, _hc); HUB = _hc
tmp = tempfile.mkdtemp(prefix='bpk-', dir='/tmp')   # a Unix socket path is at most 104 bytes on macOS
port = 3893
env = dict(os.environ, HOME=os.path.join(OUT, 'fh'), TMPDIR=tmp, BACKPLANE_KICAD_NO_SYSTEM='1', BACKPLANE_KICAD_CLI=os.path.join(RT, 'bin/kicad-cli'),
           BACKPLANE_AGENTS=os.path.join(OUT, 'bin'), BACKPLANE_NO_UPDATE='1', BACKPLANE_KICAD_DEBUG='1', BACKPLANE_KICAD_EMBED_TRACE=os.path.join(OUT, 'kicad-embed-trace.log'), FAKE_LOG=os.path.join(OUT, 'standin.log'),
           # the Mac mini gate: KiCad's documents folder in the test home. Its default, ~/Documents/KiCad, makes macOS ask the hub (KiCad's
           # responsible process) for the Documents folder, and KiCad's UI thread waits on the answer: first Edit stalled 30 s, then
           # "KiCad could not start" (a person's first Edit meets the same prompt; the runner has TCC granted)
           **({'KICAD_DOCUMENTS_HOME': os.path.join(OUT, 'fh', 'Documents')} if DRIVER == 'vnc' else {}),
           # KiCad on macOS takes its config folder from the account's real home, not $HOME: without this the gate's KiCad seeded and wrote
           # the person's own ~/Library/Preferences/kicad/10.99 (the Mac mini, Nov 16). The hub reads $HOME, so both point at the test home.
           KICAD_CONFIG_HOME=os.path.join(OUT, 'fh', 'Library', 'Preferences', 'kicad'))
if os.environ.get('MAC_MODE') == 'update':
    env.pop('BACKPLANE_KICAD_CLI', None)
    if os.environ.get('BACKPLANE_KICAD_FEED'): env['BACKPLANE_KICAD_FEED'] = os.environ['BACKPLANE_KICAD_FEED']
hub = subprocess.Popen([HUB, '--home', os.path.join(OUT, 'home'), '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'],
                       env=env, stdout=open(os.path.join(OUT, 'hub.log'), 'w'), stderr=subprocess.STDOUT); procs.append(hub)
wait_for(lambda: subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', f'http://127.0.0.1:{port}/hello']).returncode == 0, 30)
script = [{"select": 0}, {"wait": 2500}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 5400000 if os.environ.get("MAC_MODE") == "front" else 900000}]
json.dump(script, open(os.path.join(OUT, 'script.json'), 'w'))
cenv = dict(os.environ, HOME=os.path.join(OUT, 'fh'), BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1', BACKPLANE_KICAD_DEBUG='1')
client = subprocess.Popen([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', os.path.join(OUT, 'home/token'), '--script', os.path.join(OUT, 'script.json'), '--exit-after', '5400' if os.environ.get('MAC_MODE') in ('bs', 'narrow', 'libs', 'front') else '900'],
                          env=cenv, stdout=open(os.path.join(OUT, 'client.log'), 'w'), stderr=subprocess.STDOUT); procs.append(client)


def shot(name): shot_to(os.path.join(OUT, name + '.png'))
def hub_log(): return open(os.path.join(OUT, 'hub.log'), errors='replace').read()
CLIENT_LOG = [os.path.join(OUT, 'client.log')]  # the running client's log (a restarted client writes its own)
def client_log(): return open(CLIENT_LOG[0], errors='replace').read()


def last_push():
    """The pane's rect (screen points) and visibility the client last pushed to KiCad."""
    v = None
    for l in client_log().splitlines():
        if '"kicad_host_push"' in l:
            try: j = json.loads(l.split('GPUI_LOG ', 1)[1]); v = (tuple(j['kicad_host_push']), j['visible'])
            except Exception: pass
    return v


def backing_scale():
    """The client window's pixels per point: its own log says the pane in pixels (pane_px) beside the rect pushed to KiCad in points
    (1 on the e2e-macos runner, 2 on a Retina Mac: the mini's rects were twice their point positions and every rect click missed)."""
    s = 1.0
    for l in client_log().splitlines():
        if '"kicad_host_push"' in l:
            try:
                j = json.loads(l.split('GPUI_LOG ', 1)[1])
                if j.get('pane_px') and j['kicad_host_push'][2]: s = j['pane_px'][2] / j['kicad_host_push'][2]
            except Exception: pass
    return round(s * 4) / 4 or 1.0


NOTICE_IDS = ('kicad:notice', 'kicad:notice-start', 'kicad:asking', 'kicad:pop-full', 'kicad-update')


def notices_outside(tag, since_line=0):
    """Every notice the client drew since `since_line` of its log (the strip's "KiCad is asking", its notice line, the start notice, the
    full-screen pop-out line, the update row) lies outside the pane's rect: on macOS KiCad's frame is over the pane, so a notice drawn
    there is hidden. Returns {id: (inside?, rect)}; screenshots the window as it is."""
    lines = client_log().splitlines()[since_line:]
    k = backing_scale(); seen = {}; pane = None
    for l in lines:
        if 'GPUI_METRIC ' in l and '"rects"' in l:
            try:
                for e in json.loads(l.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                    if isinstance(e, dict) and any(e.get('id', '') == n or e.get('id', '').startswith(n + ':') for n in NOTICE_IDS): seen[e['id']] = e
            except Exception: pass
        if '"kicad_host_push"' in l:
            try: pane = json.loads(l.split('GPUI_LOG ', 1)[1]).get('pane_px') or pane
            except Exception: pass
    if pane is None:
        for l in reversed(lines and client_log().splitlines()):
            if '"kicad_host_push"' in l:
                pane = json.loads(l.split('GPUI_LOG ', 1)[1]).get('pane_px'); break
    out = {}
    for i, e in seen.items():
        x, y, w, h = e['x'], e['y'], e['w'], e['h']
        inside = bool(pane) and x < pane[0] + pane[2] and pane[0] < x + w and y < pane[1] + pane[3] and pane[1] < y + h and w > 0 and h > 0
        out[i] = (inside, [round(v / k) for v in (x, y, w, h)])
    shot(f'notices-{tag}')
    return out


def rects():
    """The client's control rects (BACKPLANE_GPUI_RECTS: lists of {id, x, y, w, h} in window pixels, from the content's top-left), latest,
    in window points."""
    r = {}
    for l in client_log().splitlines():
        if 'GPUI_METRIC ' in l and '"rects"' in l:
            try:
                for e in json.loads(l.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                    if isinstance(e, dict) and 'id' in e: r[e['id']] = e
            except Exception: pass
    k = backing_scale()
    return {i: dict(e, x=e['x'] / k, y=e['y'] / k, w=e['w'] / k, h=e['h'] / k) for i, e in r.items()} if k != 1 else r


try:
    cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
    check(bool(cw), 'the client window is on screen')
    # the client opens 1400x900, larger than the runner's 1024x768 display: put it on screen whole, so every click lands
    win_move(client.pid, 0, 25)
    win_size(client.pid, 1000, 700)
    time.sleep(1.5)
    cw = next((w for w in wins() if w[1] == client.pid and w[0] == cw[0]), cw)
    print(f'client window now {cw and cw[3]}', flush=True)
    # ---- MAC_MODE=docs: a project kept in ~/Documents (the lead's OK: exactly ~/Documents/bp-gate-docs-test/, removed after): macOS asks
    # Backplane for the Documents folder; while it waits the pane says so, the hub does not restart KiCad, there is one KiCad; then the
    # answer (DOCS_ANSWER allow|deny): Allow, the editor opens; Don't Allow, the pane says what to do instead of hanging ----
    if os.environ.get('MAC_MODE') == 'docs':
        ans = os.environ.get('DOCS_ANSWER', 'allow')
        def kicad_pids():
            kids = {}
            for l in subprocess.run(['ps', '-axo', 'pid=,ppid=,comm='], capture_output=True, text=True).stdout.splitlines():
                parts = l.split(None, 2)
                if len(parts) == 3: kids.setdefault(int(parts[1]), []).append((int(parts[0]), parts[2]))
            out, todo = [], [hub.pid]
            while todo:
                for pid, comm in kids.get(todo.pop(), []):
                    todo.append(pid)
                    if comm.rsplit('/', 1)[-1] in ('kicad', 'pcbnew', 'eeschema'): out.append(pid)
            return out
        PROMPT_OWNERS = ('UserNotificationCenter', 'universalAccessAuthWarn', 'tccd', 'CoreServicesUIAgent', 'SecurityAgent')
        def prompt():
            for w in helper('windows') or []:
                if any(o in w['owner'] for o in PROMPT_OWNERS) and w['bounds'][2] > 150: return w
            return None
        pr = wait_for(prompt, 90, 0.5)
        RES['docs_prompt'] = pr and {'owner': pr['owner'], 'bounds': pr['bounds']}
        shot('docs-1-prompt')
        check(bool(pr), f'docs: a project in ~/Documents makes macOS ask for Backplane ({RES["docs_prompt"]})')
        if not pr:
            ws = helper('windows') or []
            print('INFO docs: no prompt; on screen: ' + json.dumps(sorted({(w['owner'], w['layer']) for w in ws})), flush=True)
            hl = [l for l in hub_log().splitlines() if 'Documents' in l or 'Operation not permitted' in l or 'denied' in l.lower()]
            print('INFO docs: hub lines about it: ' + json.dumps(hl[:5]), flush=True)
        if pr:
            time.sleep(12)  # past the hub's 5 s "not ready" and its old 30 s restart point would come later: the message is said meanwhile
            said = 'mac-permission-prompt' in client_log() or 'macOS is asking for permission' in client_log()
            waits = 'waiting on a macOS prompt' in hub_log()
            n = kicad_pids(); RES['docs_kicads_waiting'] = n
            shot('docs-2-waiting')
            check(said, 'docs: while the prompt waits, the pane says macOS is asking for permission for Backplane')
            check(waits and 'starting it once more' not in hub_log(), 'docs: the hub waits on the prompt and does not restart KiCad')
            check(len(n) <= 1, f'docs: one KiCad while the prompt waits, never a second ({n})')
            x, y, w, h = pr['bounds']
            click(x + w * (0.73 if ans == 'allow' else 0.27), y + h * 0.85); time.sleep(1.5)
            shot('docs-3-answered')
            if ans == 'allow':
                f = wait_for(kicad_frame, 90, 0.5); shot('docs-4-allowed')
                check(bool(f), 'docs: Allow: the editor opens over the pane')
            else:
                time.sleep(20); shot('docs-4-denied')
                info_lines = [l for l in client_log().splitlines() if '"kicad_info"' in l]
                last = info_lines[-1] if info_lines else ''
                stuck = '"state":"starting"' in last and 'mac-permission-prompt' not in last
                check(not stuck, f'docs: Don\'t Allow: the pane says what to do rather than hanging on "Starting KiCad…" ({last[:200]})')
                print('INFO docs deny: the pane as shown in docs-4-denied.png', flush=True)
        raise SystemExit(1 if FAILS else 0)
    if os.environ.get('MAC_MODE') == 'lock':
        def rect_since(i, line):
            r = None
            for l in client_log().splitlines()[line:]:
                if 'GPUI_METRIC ' in l and '"rects"' in l:
                    try:
                        for e in json.loads(l.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                            if isinstance(e, dict) and e.get('id') == i: r = e
                    except Exception: pass
            k = backing_scale()
            return r and dict(r, x=r['x'] / k, y=r['y'] / k, w=r['w'] / k, h=r['h'] / k)
        def press(r):
            b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
        def kicads():
            out = []
            for l in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines():
                if '--parent-pid' in l and '--embedded' in l and OUT in l: out.append(l.split()[0])
            return out
        def kicad_frame():
            order = wins(); ours = next((i for i, w in enumerate(order) if cw and w[0] == cw[0]), None)
            if ours is None: return None
            k = [w for w in order[:ours] if w[1] not in (client.pid, hub.pid) and 'kicad' in w[2].lower()]
            return k[0] if k and order.index(k[0]) == ours - 1 else None
        holders = lambda: subprocess.run(['lsof', '-t', '--', LCK], capture_output=True, text=True).stdout.split()
        fow = lambda: 'File Open Warning' in hub_log()
        o = wait_for(lambda: rect_since('kicad:lock-open', 0), 60, 0.5); c = rect_since('kicad:lock-cancel', 0); time.sleep(1); shot('lock-1-asked')
        check(bool(o) and bool(c) and not kicads() and not fow(), f'lock L1: the pane asks Open anyway or Cancel; no KiCad started, no File Open Warning (KiCads {kicads()})')
        mark = len(client_log().splitlines())
        if c: press(c)
        time.sleep(2.5); rt = wait_for(lambda: rect_since('kicad:retry', mark), 10, 0.5); shot('lock-2-cancelled')
        # the click is handled (the macOS end-marker hitbox over the pane's controls does not block it): the prompt goes, and the answer is sent
        # the client logs the RPC (kicad_rpc_sent editor.kicad.lock), the hub the answer ("the person chose Cancel"); the prompt's buttons are
        # not in the client's latest frame
        sent = [l for l in client_log().splitlines()[mark:] if '"kicad_rpc_sent":"editor.kicad.lock"' in l]
        said = [l for l in hub_log().splitlines() if 'the person chose Cancel' in l] if sent else []
        last = next((l for l in reversed(client_log().splitlines()) if 'GPUI_METRIC ' in l and '"rects"' in l), '')
        gone_prompt = '"kicad:lock-cancel"' not in last and '"kicad:lock-open"' not in last
        check(bool(c) and bool(said) and gone_prompt, f'lock L2a: the Cancel click is handled: the prompt goes ({gone_prompt}), editor.kicad.lock answer cancel sent ({(said or [""])[0].strip()[:160]!r})')
        check(not kicads() and bool(rt) and str(holder.pid) in holders(), f'lock L2: Cancel: no KiCad starts, Try again offered, the other KiCad still holds its lock ({holders()})')
        mark = len(client_log().splitlines())
        if rt: press(rt)
        o2 = wait_for(lambda: rect_since('kicad:lock-open', mark), 30, 0.5)
        if o2: press(o2)
        f = wait_for(kicad_frame, 90, 0.5); shot('lock-3-opened')
        check(bool(o2) and bool(f) and not fow(), f'lock L3: Try again asks again; Open anyway paints the editor, no File Open Warning (holders now {holders()})')
        raise SystemExit(1 if FAILS else 0)
    if os.environ.get('MAC_MODE') == 'update':
        row = wait_for(lambda: rects().get('kicad-update'), 120, 0.5)
        n = notices_outside('update-offer', 0)
        check(bool(row), f'update: the first Edit with bp.12.5 offers the update in its row ({row})')
        check(bool(row) and not n.get('kicad-update', (True,))[0], f'update: the row is outside the pane\'s rect ({n.get("kicad-update")})')
        go = rects().get('kicad-update:go'); cwu = next((w for w in wins() if w[0] == cw[0]), cw)
        if go: click(cwu[3][0] + go['x'] + go['w'] / 2, cwu[3][1] + 28 + go['y'] + go['h'] / 2)
        kd = os.path.join(OUT, 'home', 'kicad')
        new_cli = lambda: next((os.path.join(kd, d, 'bin', 'kicad-cli') for d in (os.listdir(kd) if os.path.isdir(kd) else []) if 'bp.13' in d and os.path.isfile(os.path.join(kd, d, 'bin', 'kicad-cli'))), None)
        inst = wait_for(new_cli, 1200, 2); shot('update-2-installed')
        check(bool(go) and bool(inst), f'update: Update installs bp.13 from the feed ({inst})')
        ro = wait_for(lambda: rects().get('kicad-update:reopen'), 30, 0.5)
        if ro: click(cwu[3][0] + ro['x'] + ro['w'] / 2, cwu[3][1] + 28 + ro['y'] + ro['h'] / 2)
        def _frame_above_ours():  # as kicad_frame (defined after this block): a KiCad window directly above Backplane's
            order = wins(); ours = next((i for i, w in enumerate(order) if cw and w[0] == cw[0]), None)
            if ours is None: return None
            k = [w for w in order[:ours] if w[1] not in (client.pid, hub.pid) and 'kicad' in w[2].lower()]
            return k[0] if k and order.index(k[0]) == ours - 1 else None
        nv = os.path.basename(os.path.dirname(os.path.dirname(inst))) if inst else '10.99.0-bp.13'
        f = wait_for(_frame_above_ours, 180, 0.5); shot('update-3-reopened')
        p13 = [l for l in subprocess.run(['ps', '-axo', 'command='], capture_output=True, text=True).stdout.splitlines() if f'{nv}/' in l and OUT in l and 'kicad' in l.lower()]
        check(bool(f) and bool(p13), f'update: the pane reopens on {nv}, KiCad over it ({"Reopen pressed" if ro else "no Reopen button"}; {len(p13)} bp.13 KiCad processes)')
        raise SystemExit(1 if FAILS else 0)
    # ---- Edit: KiCad's frame over the pane, directly above our window ----
    def kicad_frame():
        order = wins(); ours = next((i for i, w in enumerate(order) if cw and w[0] == cw[0]), None)
        if ours is None: return None
        k = [w for w in order[:ours] if w[1] not in (client.pid, hub.pid) and 'kicad' in w[2].lower()]
        return k[0] if k and order.index(k[0]) == ours - 1 else None
    t0 = time.time()
    if os.environ.get('FRESH_AGENT') == 'during':
        import threading
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac')); import hub_ws
        def _agent():
            time.sleep(4)
            hub_ws.send(port, open(os.path.join(OUT, 'home/token')).read().strip(), 'turn.start',
                        {'thread': 't1', 'text': 'mcpcall:' + json.dumps({'name': 'kicad_ipc_status', 'arguments': {'path': os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb')}}), 'msg': f'fresh-{time.time_ns()}'})
        threading.Thread(target=_agent, daemon=True).start()
    f = wait_for(kicad_frame, 180, 0.25)
    RES['edit_to_frame_s'] = round(time.time() - t0, 1)
    shot('01-edit')
    cw = next((w for w in wins() if cw and w[0] == cw[0]), cw)
    inside = f and cw and cw[3][0] <= f[3][0] and cw[3][1] <= f[3][1] and f[3][0] + f[3][2] <= cw[3][0] + cw[3][2] + 1 and f[3][1] + f[3][3] <= cw[3][1] + cw[3][3] + 1
    check(bool(f) and bool(inside), f'Edit: KiCad\'s board editor is over the pane, directly above Backplane ({f and f[3]} in {cw and cw[3]}, {RES["edit_to_frame_s"]} s)')
    if not f: raise SystemExit(1)
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
    # ---- MAC_MODE=autosave: with the keyboard in Backplane (KiCad not the active app), an agent commits a change over the API; a .history
    # snapshot appears within the debounce (3 s here); KiCad killed; Open the editor again: KiCad offers the recovery, and the pane names it ----
    # ---- MAC_MODE=recover: the same, then fork embed's fix/mac-recovery-alert: KiCad's native recovery alert (an NSAlert before its API
    # answers) is reported to the hub (ET_DIALOG_OPENED from the new KiCad), in front of Backplane's window and centred on the pane. The pane
    # sits right of the screen's centre (the client window at the left, 1000 pt wide: pane centre x ~680, the screen's 512), so an alert centred
    # on the screen (AppKit's own place) is not on the pane's centre; the run says when it cannot tell them apart ----
    if os.environ.get('MAC_MODE') in ('autosave', 'recover'):
        REC = os.environ.get('MAC_MODE') == 'recover'
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac')); import hub_ws
        token = open(os.path.join(OUT, 'home/token')).read().strip(); pcb = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb')
        def mcalls(): return [l for l in open(os.path.join(OUT, 'standin.log'), errors='replace').read().splitlines() if l.startswith('MCPCALL')] if os.path.isfile(os.path.join(OUT, 'standin.log')) else []
        def ask(name, args, wait=120):
            n0 = len(mcalls())
            hub_ws.send(port, token, 'turn.start', {'thread': 't1', 'text': 'mcpcall:' + json.dumps({'name': name, 'arguments': args}), 'msg': f'as-{time.time_ns()}'})
            wait_for(lambda: len(mcalls()) > n0, wait, 0.3)
            l = mcalls()[-1] if len(mcalls()) > n0 else ''
            try: return json.loads(json.loads(l.split(' ', 2)[2])['result']['content'][0]['text'])
            except Exception: return {'raw': l[:300]}
        hist = os.path.join(OUT, 'proj', '.history')
        snaps = lambda: int(subprocess.run(['git', '-C', hist, 'rev-list', '--all', '--count'], capture_output=True, text=True).stdout.strip() or 0) if os.path.isdir(hist) else 0
        # the keyboard to Backplane: a click in its chrome
        cwa = next((w for w in wins() if w[0] == cw[0]), cw)
        click(cwa[3][0] + 200, cwa[3][1] + 28 + 300); time.sleep(1)
        check(not active(kpid), 'autosave: the keyboard is in Backplane (KiCad is not the active app)')
        C = 'kiapi.common.commands.'
        docs = ask('kicad_ipc', {'path': pcb, 'type': C + 'GetOpenDocuments', 'message': {'type': 'DOCTYPE_PCB'}})
        doc = ((docs.get('message') or {}).get('documents') or [None])[0]
        h = {'document': doc}
        s0 = snaps()
        b = ask('kicad_ipc', {'path': pcb, 'type': C + 'BeginCommit', 'save': False, 'message': {'header': h}})
        tid = (b.get('message') or {}).get('id')
        ask('kicad_ipc', {'path': pcb, 'type': C + 'CreateItems', 'save': False, 'message': {'header': h, 'items': [{'@type': 'type.googleapis.com/kiapi.board.types.Track', 'id': {'value': str(__import__('uuid').uuid4())},
            'start': {'x_nm': 120000000, 'y_nm': 100000000}, 'end': {'x_nm': 140000000, 'y_nm': 100000000}, 'width': {'value_nm': 300000}, 'layer': 'BL_F_Cu'}]}})
        # save: false (lane J's 5c3cddabc saves an agent's landed change by default): the change stays unsaved, as the recovery needs
        e = ask('kicad_ipc', {'path': pcb, 'type': C + 'EndCommit', 'save': False, 'message': {'header': h, 'id': tid, 'action': 'CMA_COMMIT', 'message': 'agent add track'}})
        RES['autosave_commit'] = {'doc': bool(doc), 'tid': bool(tid), 'end': str(e)[:160]}
        got = wait_for(lambda: snaps() > s0, 3 + 12, 0.5)
        check(bool(got), f'autosave: a .history snapshot within the debounce after the agent\'s commit ({s0} -> {snaps()}; {RES["autosave_commit"]})')
        # KiCad killed; Open the editor again; KiCad offers the recovery
        kk = []
        for l in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines():
            if '--parent-pid' in l and '--embedded' in l and OUT in l: kk.append(int(l.split()[0]))
        def embed_events():
            out = []
            for ln in hub_log().splitlines():
                i = ln.find('kicad embed event: ')
                if i < 0: continue
                try: out.append(json.loads(ln[i + 19:]))
                except ValueError: pass
            return out
        ev0 = len(embed_events())
        for p in kk:
            try: os.kill(p, 9)
            except OSError: pass
        oa = wait_for(lambda: rects().get('kicad:open-again'), 20, 0.5); shot('autosave-1-closed')
        if oa:
            b2 = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b2[0] + oa['x'] + oa['w'] / 2, b2[1] + 28 + oa['y'] + oa['h'] / 2)
        # macOS: KiCad's recovery question is a native alert before its API is up (no dialog event): a window of KiCad's own on screen
        # while it is not ready, or (where the fork reports it) the dialog event
        def alert():
            # any window level: AppKit's alert is not at the normal level (seen on e2e-macos 44247744393: the alert was on screen, this missed it)
            ks = [w for w in (helper('windows') or []) if 'kicad' in w['owner'].lower() and w['pid'] not in (client.pid, hub.pid) and 150 < w['bounds'][2] < 700 and 80 < w['bounds'][3] < 520]
            return ks or ('Recover Unsaved Changes' in hub_log())
        rec = wait_for(alert, 90, 0.5); time.sleep(2); shot('autosave-2-recovery')
        named = 'kicad-recover-prompt' in client_log() or 'kicad-recover-prompt' in hub_log() or 'Recover Unsaved Changes' in client_log()
        RES['autosave_alert'] = str(rec)[:160]
        check(bool(oa) and bool(rec), f'autosave: KiCad killed, Open the editor again: KiCad offers Recover Unsaved Changes (open-again {bool(oa)})')
        check(named, 'autosave: the pane names it ("KiCad is asking: Recover Unsaved Changes")')
        if REC:
            def new_kicads():
                return [int(l.split()[0]) for l in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines()
                        if '--parent-pid' in l and '--embedded' in l and OUT in l and int(l.split()[0]) not in kk]
            def the_alert():  # (index front to back, bounds) of the new KiCad's small window: the alert; any window level
                ws = helper('windows') or []; nk = set(new_kicads())
                for i, w in enumerate(ws):
                    b = w['bounds']
                    if w['pid'] in nk and 150 < b[2] < 700 and 80 < b[3] < 520: return i, b, ws
                return None
            al = wait_for(the_alert, 60, 0.5)
            dlg = wait_for(lambda: [e for e in embed_events()[ev0:] if e.get('type') == 'ET_DIALOG_OPENED'], 30, 0.5)
            RES['recover_alert'] = {'alert': al and al[1], 'dialog_events': [e.get('detail') for e in dlg or []]}
            check(bool(dlg), f'recover: the recovery alert is reported to the hub (ET_DIALOG_OPENED from the new KiCad: {[e.get("detail") for e in dlg or []]})')
            pp = last_push(); P = pp[0] if pp else None
            if al and P:
                i, b, ws = al
                ci = next((j for j, w in enumerate(ws) if w['number'] == cw[0]), None)
                check(ci is not None and i < ci, f'recover: the alert is in front of Backplane\'s window (alert {i}, Backplane {ci}, front to back)')
                acx, acy = b[0] + b[2] / 2, b[1] + b[3] / 2; pcx = P[0] + P[2] / 2
                disp = Quartz.CGDisplayBounds(Quartz.CGMainDisplayID()) if DRIVER == 'quartz' else None
                scx = disp.origin.x + disp.size.width / 2 if disp else None
                print(f'INFO recover: alert {b}, centre ({acx:.0f}, {acy:.0f}); pane {P}, centre x {pcx:.0f}; screen centre x {scx}', flush=True)
                if scx is not None and abs(pcx - scx) < 80: print('INFO recover: the pane\'s centre is near the screen\'s: the centring check cannot tell them apart here', flush=True)
                check(abs(acx - pcx) <= 40 and P[1] <= acy <= P[1] + P[3], f'recover: the alert is centred on the pane (|dx| {abs(acx - pcx):.0f} pt, centre y {acy:.0f} in the pane\'s {P[1]:.0f}..{P[1] + P[3]:.0f})')
            else:
                check(False, f'recover: the alert window of the new KiCad is on screen (alert {bool(al)}, pane {bool(P)})')
            shot('recover-1-alert')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=leave (#1199): every way the pane leaves the page takes KiCad's frame off screen, and coming back brings it: (a) the
    # viewer's Close, (b) the rail closing the lit page, (c) another thread, (d) the Files page, (e) a Diff over the page, (f) Settings over
    # the page. Each from KiCad holding the keyboard (a click on its canvas): within 1 s no window of the embedded KiCad covers the pane's
    # rect (the window list) and Backplane is the active app; back, KiCad's frame is directly above Backplane's window at the pane's rect
    # within 250 ms of the press, and the keyboard is KiCad's (at once, or after a click on its canvas: said) ----
    if os.environ.get('MAC_MODE') == 'leave':
        P = last_push()[0]
        def rects_now():  # the rects of the client's latest frame only (rects() keeps every id ever drawn: a stale one was pressed, e2e-macos 44250085437)
            last = None
            for l in client_log().splitlines():
                if 'GPUI_METRIC ' in l and '"rects"' in l: last = l
            r = {}
            try:
                for e in json.loads(last.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                    if isinstance(e, dict) and 'id' in e: r[e['id']] = e
            except Exception: pass
            k = backing_scale()
            return {i: dict(e, x=e['x'] / k, y=e['y'] / k, w=e['w'] / k, h=e['h'] / k) for i, e in r.items()} if k != 1 else r
        def press_rect(i):
            r = rects_now().get(i)
            if not r: return False
            b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2); return True
        def press_prefix(pre, skip=()):
            ids = sorted(k for k in rects() if k.startswith(pre) and k not in skip)
            return press_rect(ids[0]) and ids[0] if ids else False
        def kicad_over_pane():  # KiCad's windows on screen that cover most of the pane's rect (its frame; not a tooltip)
            out = []
            for w in wins():
                if w[1] != kpid: continue
                x0, y0 = max(w[3][0], P[0]), max(w[3][1], P[1]); x1, y1 = min(w[3][0] + w[3][2], P[0] + P[2]), min(w[3][1] + w[3][3], P[1] + P[3])
                if x1 > x0 and y1 > y0 and (x1 - x0) * (y1 - y0) > 0.3 * P[2] * P[3]: out.append(w)
            return out
        def at_pane():  # KiCad's frame directly above Backplane's window, at the pane's rect (2 pt)
            f = kicad_frame()
            return f if f and all(abs(a - b) <= 2 for a, b in zip(f[3], P)) else None
        def canvas_click(): click(P[0] + P[2] * 0.5, P[1] + P[3] * 0.5); time.sleep(0.6); key(K_ESC); time.sleep(0.3)
        # where a return's time goes: the client's pushes, stamped as they reach its log (a reader every 5 ms: the client writes them
        # unbuffered), the hub's SetHostGeometry requests (their own ms stamps), and the window list's first sight of the frame
        import threading
        PUSHES = []
        def _tail():
            n = 0
            while True:
                try: ls = open(CLIENT_LOG[0], errors='replace').read().splitlines()
                except OSError: ls = []
                now = time.time()
                for l in ls[n:]:
                    if '"kicad_host_push"' in l:
                        try: PUSHES.append((now, json.loads(l.split('GPUI_LOG ', 1)[1])))
                        except Exception: pass
                n = len(ls); time.sleep(0.005)
        threading.Thread(target=_tail, daemon=True).start()
        def hub_geoms(): return [int(m) / 1000 for m in __import__('re').findall(r'SetHostGeometry[^\n]*\(at (\d+)\)', hub_log())]
        def timeline(tag, t_click, t_seen):
            pv = next((t for t, j in PUSHES if t >= t_click and j.get('visible')), None)
            pj = next((j for t, j in PUSHES if t >= t_click and j.get('visible')), {})
            hg = next((t for t in hub_geoms() if t >= t_click - 0.01), None)
            ms = lambda t: f'{(t - t_click) * 1000:.0f} ms' if t else 'none'
            print(f'INFO timeline {tag}: click 0; first push visible:true {ms(pv)} (in_page {pj.get("in_page")}, covered {pj.get("covered")}, hitbox {pj.get("hitbox")}); '
                  f'hub SetHostGeometry {ms(hg)}; frame seen above our window {ms(t_seen)}', flush=True)
        # the baseline: View then Edit on the warm pane (the return path main already has), click to the frame front-most at the pane
        wait_for(at_pane, 10, 0.2); P = last_push()[0]
        if press_rect('ui:View'):
            wait_for(lambda: not kicad_over_pane(), 5, 0.05); time.sleep(1.5)
            tb = time.time(); pe = press_rect('ui:Edit'); fb = wait_for(at_pane, 30, 0.02); ts = time.time()
            print(f'INFO leave baseline: View then Edit on a warm pane, the frame front-most at the pane in {(ts - tb) * 1000:.0f} ms (Edit pressed {pe}; seen {bool(fb)})', flush=True)
            timeline('baseline View->Edit', tb, ts if fb else None)
            if fb: P = last_push()[0]
        # (c) another thread: a second thread of the project made through the hub, the sidebar unfolded so both rows are drawn (the pane
        # narrows; P follows); (c) presses the other thread's row and comes back on the first one's
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac')); import hub_ws
        hub_ws.send(port, open(os.path.join(OUT, 'home/token')).read().strip(), 'thread.create', {'project': 'p1', 'title': 'Other thread'}, wait=1.0)
        time.sleep(2.0)
        if not any(k.startswith('thread:') for k in rects_now()):
            folds = [k for k in rects_now() if 'sidebar' in k.lower() or 'fold' in k.lower()]
            print(f'INFO leave: sidebar controls {folds}', flush=True)
            press_rect('ui:Fold sidebar') or (folds and press_rect(folds[0])); time.sleep(1.5)
        rows = sorted(k for k in rects_now() if k.startswith('thread:'))
        home_thread = 'thread:t1' if 'thread:t1' in rows else (rows[0] if rows else None)
        other_thread = next((k for k in rows if k != home_thread), None)
        print(f'INFO leave: the sidebar shows {rows}; home {home_thread}, other {other_thread}', flush=True)
        if home_thread: press_rect(home_thread); time.sleep(1.0)
        wait_for(at_pane, 10, 0.2); P = last_push()[0]
        def jump(title, timed=False):  # the command bar (Cmd+K): a thread by its title, Return; the sidebar may not list a new worktree thread
            cwa = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(cwa[0] + 150, cwa[1] + 28 + 250); time.sleep(0.5)
            if not press_rect('ui:Jump or run a command'): key(K_K, CMD)
            time.sleep(0.8); relay(op='text', text=title); time.sleep(1.2)
            t = time.time(); key(36); return (True, t) if timed else True
        def back_from_settings():
            # Settings is a sheet over the window: Escape closes it (the gear behind it is not pressable; e2e-macos 44296739808 left it open);
            # a second Escape is harmless (the viewer is already shut by Settings)
            key(K_ESC); time.sleep(1.0); key(K_ESC); time.sleep(0.8)
            t = time.time(); return press_rect('rail:Board'), t
        def open_keysheet():
            cwa = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(cwa[0] + 150, cwa[1] + 28 + 250); time.sleep(0.6)
            PUSH_MARK[0] = len(client_log().splitlines())
            key(122); return True  # F1
        PUSH_MARK = [0]
        ways = [
            ('a', 'the viewer\'s Close', lambda: press_rect('ui:CloseViewer'), lambda: press_rect('rail:Board')),
            ('b', 'the rail closing the lit page', lambda: press_rect('rail:Board'), lambda: press_rect('rail:Board')),
            # the sidebar lists the selected thread (the other one's row went once it was left: e2e-macos 44288284194), so both ways go through
            # the command bar's "Jump or run a command" with the thread's title
            ('c', 'another thread', lambda: jump('Other thread'), lambda: jump('Embedded KiCad E2E', timed=True)),
            ('d', 'the Files page', lambda: press_rect('rail:Files'), lambda: press_rect('rail:Board')),
            ('e', 'a Diff over the page', lambda: press_rect('rail:Diff'), lambda: press_rect('rail:Board')),
            # opening Settings shuts the viewer by design (host settings.rs, 0.14's Settings.viewer.shut): back is Settings closed, then the
            # Board page, the return timed from that press
            ('f', 'Settings over the page', lambda: press_rect('ui:Settings'), back_from_settings),
            # the keyboard sheet over the page with the viewer open (#1199's covered path): Backplane takes the keyboard (a click in its
            # chrome), F1; back is Escape (keysheet_close before escape_rest), the viewer still open
            ('g', 'the keyboard sheet over the page', open_keysheet, lambda: (key(K_ESC), True)[1]),
        ]
        for tag, label, leave_, back_ in ways:
            if not at_pane():
                f0 = wait_for(at_pane, 10, 0.1)
                if not f0: check(False, f'leave {tag}: KiCad\'s frame over the pane before {label}'); continue
            canvas_click(); k_before = active(kpid)
            pressed_ = leave_(); t0 = time.time()
            gone = wait_for(lambda: not kicad_over_pane(), 1.0, 0.05); dt_gone = time.time() - t0; shot(f'leave-{tag}')
            bp_active = active(client.pid) and not active(kpid)
            check(bool(pressed_) and bool(gone), f'leave {tag}: {label} takes KiCad\'s frame off screen within 1 s (pressed {pressed_}; KiCad had the keyboard {k_before}; KiCad windows over the pane {[w[3] for w in kicad_over_pane()]})')
            check(bp_active, f'leave {tag}: Backplane is the active app with the keyboard after {label}')
            time.sleep(1.0)
            if tag == 'g':
                cov = [l for l in client_log().splitlines()[PUSH_MARK[0]:] if '"kicad_host_push"' in l and '"covered":true' in l]
                check(bool(cov), f'leave g: a kicad_host_push said covered:true under the sheet ({(cov or [""])[0].split("GPUI_LOG ", 1)[-1][:160]!r})')
            tb = time.time(); b_ = back_()
            if isinstance(b_, tuple): b_, tb = b_
            f2 = wait_for(at_pane, 3, 0.02); dt = time.time() - tb
            timeline(f'{tag} back', tb, time.time() if f2 else None)
            if not f2 and rects_now().get('ui:Edit'):
                print(f'INFO leave {tag}: the page came back on View: Edit pressed', flush=True)
                tb = time.time(); press_rect('ui:Edit'); f2 = wait_for(at_pane, 30, 0.02); dt = time.time() - tb
            if not f2:  # where it ended up, for the record
                f2 = wait_for(at_pane, 10, 0.1); dt = time.time() - tb
            k_now = bool(f2) and active(kpid); shot(f'leave-{tag}-back')
            check(bool(f2), f'leave {tag}: back after {label}, KiCad\'s frame is front-most at the pane\'s rect (pressed {b_}; {dt * 1000:.0f} ms; frame {kicad_frame() and kicad_frame()[3]}, pane {last_push()[0]})')
            print(f'INFO leave {tag}: back in {dt * 1000:.0f} ms; KiCad has the keyboard {k_now} before any click', flush=True)
            if f2:
                check(dt <= 0.25, f'leave {tag}: the editor is back within ~200 ms of the press ({dt * 1000:.0f} ms)')
                P = last_push()[0]
                if not k_now: canvas_click()
                check(active(kpid), f'leave {tag}: the keyboard is in KiCad ({"at once" if k_now else "after a click on the canvas"})')
            else:
                P = last_push()[0]
        for l in client_log().splitlines():
            if '"kicad_host_push"' in l and '"covered":true' in l.replace(' ', ''): print('INFO leave: a push said covered: ' + l.split('GPUI_LOG ', 1)[-1][:200], flush=True); break
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=swap (#1199): KiCad's frame can end off the client's last pushed rect, or not show (e2e-macos 44255662838: pushes 500
    # false, 524 true, 500 true, the frame stayed at 524; 44274808579 returns 3 and 4). The mechanism is not settled: SetHostGeometry goes on
    # KiCad's events socket and is acknowledged before its deferred apply, ShowFrame/HideFrame on the main socket, so one connection's
    # reply order does not fix the order KiCad applies them in (the integrator's review; embed.mm's owner confirms or refutes). Close the viewer, the Board page
    # again, SWAP_REPS times: each return's burst (visible:true at one rect, then another within 150 ms: the bottom strip a frame late) is
    # recorded, and 1 s later the frame must be at the last rect the client pushed (2 pt). Run with BACKPLANE_KICAD_MAC_SETTLE_MS=0 (pane_env)
    # to see KiCad's own order (fails on bp.13.1: e2e-macos 44274808579, a lone show left no frame, a later Full size rect was dropped), and
    # with the default settle send to see the workaround. Kept as the regression for a fork fix: settle off must pass once KiCad applies
    # its show and geometry in order ----
    if os.environ.get('MAC_MODE') == 'swap':
        print(f'INFO swap: BACKPLANE_KICAD_MAC_SETTLE_MS={os.environ.get("BACKPLANE_KICAD_MAC_SETTLE_MS", "(default)")}', flush=True)
        def rects_now():
            last = None
            for l in client_log().splitlines():
                if 'GPUI_METRIC ' in l and '"rects"' in l: last = l
            r = {}
            try:
                for e in json.loads(last.split('GPUI_METRIC ', 1)[1]).get('rects', []) or []:
                    if isinstance(e, dict) and 'id' in e: r[e['id']] = e
            except Exception: pass
            k = backing_scale()
            return {i: dict(e, x=e['x'] / k, y=e['y'] / k, w=e['w'] / k, h=e['h'] / k) for i, e in r.items()} if k != 1 else r
        def press_rect(i):
            r = rects_now().get(i)
            if not r: return False
            b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2); return True
        def kframe():
            return next((w for w in wins() if w[1] == kpid and w[3][2] > 200 and w[3][3] > 200), None)
        import threading
        PUSHES = []
        def _tail():
            n = 0
            while True:
                try: ls = open(CLIENT_LOG[0], errors='replace').read().splitlines()
                except OSError: ls = []
                now = time.time()
                for l in ls[n:]:
                    if '"kicad_host_push"' in l:
                        try: PUSHES.append((now, json.loads(l.split('GPUI_LOG ', 1)[1])))
                        except Exception: pass
                n = len(ls); time.sleep(0.005)
        threading.Thread(target=_tail, daemon=True).start()
        def host_settled(t=8):  # every editor.kicad.host the client has sent is answered (their replies lagged past 1 s in 44274818743)
            def pending():
                ls = client_log().splitlines()
                sent = {l.split('"id":', 1)[1].split(',', 1)[0] for l in ls if '"kicad_rpc_sent":"editor.kicad.host"' in l}
                got = {l.split('"id":', 1)[1].split(',', 1)[0] for l in ls if '"kicad_rpc_reply":"editor.kicad.host"' in l}
                return sent - got
            return wait_for(lambda: not pending(), t, 0.05)
        reps = int(os.environ.get('SWAP_REPS', '10')); provoked = wrong = 0
        for i in range(reps):
            wait_for(kframe, 10, 0.2)
            press_rect('ui:CloseViewer'); time.sleep(1.2)
            t0 = time.time(); press_rect('rail:Board')
            toggled = False
            if i % 2 == 1:  # every other return: Full size pressed the moment the first visible:true push is seen (a second rect at once)
                if wait_for(lambda: any(t >= t0 and j.get('visible') for t, j in PUSHES), 2, 0.005): toggled = press_rect('kicad:full')
            time.sleep(1.0); wait_for(kframe, 5, 0.05); host_settled(); time.sleep(1.0)  # 1 s after the return's last push, near enough: the pushes settle within it
            burst = [(round((t - t0) * 1000), j['kicad_host_push'], j.get('visible'), j.get('settle')) for t, j in PUSHES if t >= t0 and j.get('kicad_host_push')]
            vis = [b for b in burst if b[2]]
            prov = len(vis) >= 2 and any(v[1] != vis[0][1] and v[0] - vis[0][0] <= 150 for v in vis[1:])
            last = burst[-1][1] if burst else (PUSHES[-1][1]['kicad_host_push'] if PUSHES else None); f = kframe()  # no push in the return: the last one before it
            if last is None and not burst: print(f'INFO swap {i + 1}: no push to judge against (excluded)', flush=True); continue
            ok = bool(f and last) and all(abs(a - b) <= 2 for a, b in zip(f[3], last))
            provoked += prov; wrong += (not ok)
            print(f'INFO swap {i + 1}: Full size toggled {toggled}; pushes {burst}; burst provoked {prov}; frame {f and f[3]}; last push {last}; {"at the last rect" if ok else "NOT at the last rect"}', flush=True)
            if toggled:  # back to the normal size for the next return, confirmed by a push at the narrower width (else the next return is not judged)
                n0 = len(PUSHES); press_rect('kicad:full')
                ok_back = wait_for(lambda: any(j['kicad_host_push'][2] < 800 for _, j in PUSHES[n0:]), 3, 0.05); time.sleep(1.0)
                if not ok_back: print(f'INFO swap {i + 1}: Full size off pushed no narrower rect: the next return is judged on its own pushes only', flush=True); PUSHES.append((time.time(), {'kicad_host_push': None}))
        print(f'INFO swap: {provoked} of {reps} returns provoked a burst; {wrong} ended off the last pushed rect', flush=True)
        check(wrong == 0, f'swap: every return ends at the last rect the client pushed ({wrong} of {reps} did not; {provoked} bursts provoked)')
        # (B) geometry while KiCad runs its checks (the lead's: return 3 was SetHostGeometry refused, "a task is running: DRC"): an agent's
        # RunChecks started, and while it runs (it must run longer than 300 ms, said) the pane changes size (Full size) and, on alternate
        # tries, the viewer closes and the Board page comes back; the frame must end visible at the client's LAST rect once the checks end
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac')); import hub_ws
        token = open(os.path.join(OUT, 'home/token')).read().strip(); pcbf = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb')
        def mcalls(): return [l for l in open(os.path.join(OUT, 'standin.log'), errors='replace').read().splitlines() if l.startswith('MCPCALL')] if os.path.isfile(os.path.join(OUT, 'standin.log')) else []
        def ask(name, args, wait=120):
            n0 = len(mcalls())
            hub_ws.send(port, token, 'turn.start', {'thread': 't1', 'text': 'mcpcall:' + json.dumps({'name': name, 'arguments': args}), 'msg': f'sw-{time.time_ns()}'})
            wait_for(lambda: len(mcalls()) > n0, wait, 0.3)
        b_reps = int(os.environ.get('SWAP_CHECK_REPS', '4')); b_wrong = b_during = 0
        for i in range(b_reps):
            wait_for(kframe, 10, 0.2); time.sleep(0.5)
            log0 = len(client_log().splitlines()); n0 = len(PUSHES)
            done = []
            th = threading.Thread(target=lambda: (ask('kicad_ipc', {'path': pcbf, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB'}}), done.append(time.time())), daemon=True)
            tc = time.time(); th.start(); time.sleep(0.25)
            if i % 2 == 0: press_rect('kicad:full')
            else: press_rect('ui:CloseViewer'); time.sleep(0.4); press_rect('rail:Board')
            th.join(120); dur = (done[0] - tc) if done else None; time.sleep(1.5); wait_for(kframe, 5, 0.05); host_settled(); time.sleep(0.5)
            busy = [l for l in client_log().splitlines()[log0:] if 'a task is running' in l]
            during = bool(busy) or (dur is not None and dur > 0.3)
            b_during += during
            last = next((j['kicad_host_push'] for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None); lastvis = next((j.get('visible') for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None)
            f = kframe(); ok = bool(lastvis) and bool(f and last) and all(abs(a - b) <= 2 for a, b in zip(f[3], last))
            b_wrong += (not ok)
            print(f'INFO swap B{i + 1}: {"Full size" if i % 2 == 0 else "close and reopen"} during RunChecks ({dur and round(dur * 1000)} ms; busy replies {len(busy)}); pushes {[(round((t - tc) * 1000), j["kicad_host_push"], j.get("visible")) for t, j in PUSHES[n0:] if j.get("kicad_host_push")]}; frame {f and f[3]}; last {last} visible {lastvis}; {"ok" if ok else "NOT at the last rect"}', flush=True)
            if i % 2 == 0: press_rect('kicad:full'); time.sleep(1.5)
        check(b_during > 0, f'swap B: the pane changed while KiCad ran its checks ({b_during} of {b_reps} tries ran longer than 300 ms or met a busy reply)')
        check(b_wrong == 0, f'swap B: after geometry during the checks, the frame ends visible at the LAST rect ({b_wrong} of {b_reps} did not)')
        # (C) a newer hide is not undone: the Board page, then the viewer closed again the moment the first visible:true push is seen; 1.5 s
        # later no KiCad frame may cover the pane (a deferred show must not win over the later hide)
        c_reps = int(os.environ.get('SWAP_HIDE_REPS', '4')); c_wrong = 0
        def over_pane():
            pp = (last_push() or [[0, 0, 0, 0]])[0]
            return [w for w in wins() if w[1] == kpid and w[3][2] > 200 and w[3][3] > 200 and abs(w[3][0] - pp[0]) < 400]
        for i in range(c_reps):
            wait_for(kframe, 10, 0.2); press_rect('ui:CloseViewer'); time.sleep(1.2)
            t0 = time.time(); n0 = len(PUSHES); press_rect('rail:Board')
            seen = wait_for(lambda: any(j.get('visible') for _, j in PUSHES[n0:] if j.get('kicad_host_push')), 3, 0.005)
            press_rect('ui:CloseViewer'); time.sleep(1.5)
            stale = over_pane(); c_wrong += bool(stale)
            print(f'INFO swap C{i + 1}: show seen {bool(seen)}, then closed; pushes {[(round((t - t0) * 1000), j["kicad_host_push"], j.get("visible")) for t, j in PUSHES[n0:] if j.get("kicad_host_push")]}; KiCad over the pane after 1.5 s {[w[3] for w in stale]}', flush=True)
            press_rect('rail:Board'); wait_for(kframe, 10, 0.1); time.sleep(0.8)
        check(c_wrong == 0, f'swap C: a hide right after a show is not undone ({c_wrong} of {c_reps} left KiCad over the pane)')
        # (D) no answer at all (the lead's: an unanswered first push must expire and be sent again): KiCad stopped (SIGSTOP) for SWAP_STALL_S
        # (default 10, past the client's 8 s), the viewer closed and the Board page pressed while it cannot answer, then KiCad resumed. With
        # the keyboard left in Backplane (a click in its chrome after the press): the frame must end visible at the LAST rect, and KiCad must
        # not take the keyboard by itself afterwards (no delayed focus steal from a retried raise)
        stall = float(os.environ.get('SWAP_STALL_S', '10'))
        wait_for(kframe, 10, 0.2); press_rect('ui:CloseViewer'); time.sleep(1.2)
        kp = [int(l.split()[0]) for l in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines() if '--embedded' in l and OUT in l]
        log0 = len(client_log().splitlines()); n0 = len(PUSHES)
        for p_ in kp: os.kill(p_, __import__('signal').SIGSTOP)  # by name: 19 is SIGCONT on macOS (17 is SIGSTOP)
        try:
            t0 = time.time(); press_rect('rail:Board'); time.sleep(0.8)
            cwa = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(cwa[0] + 150, cwa[1] + 28 + 250)
            time.sleep(max(0.0, stall - (time.time() - t0)))
        finally:
            for p_ in kp: os.kill(p_, __import__('signal').SIGCONT)
        host_settled(15); time.sleep(3.0)
        lines = client_log().splitlines()[log0:]
        refused = [l for l in lines if 'kicad_host_refused' in l]; retried = [l for l in lines if '"retry":true' in l]
        last = next((j['kicad_host_push'] for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None); lastvis = next((j.get('visible') for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None)
        f = kframe(); okd = bool(lastvis) and bool(f and last) and all(abs(a - b) <= 2 for a, b in zip(f[3], last))
        print(f'INFO swap D: KiCad stopped {stall:.0f} s across the return; pushes {[(round((t - t0) * 1000), j["kicad_host_push"], j.get("visible"), j.get("retry")) for t, j in PUSHES[n0:] if j.get("kicad_host_push")]}; refused {len(refused)}, retried {len(retried)}; frame {f and f[3]}; last {last}', flush=True)
        check(okd, f'swap D: after KiCad could not answer for {stall:.0f} s, the frame ends visible at the LAST rect (frame {f and f[3]}, last {last})')
        time.sleep(4.0)
        check(not active(kpid), 'swap D: KiCad did not take the keyboard by itself after the recovery (no delayed focus steal)')
        # (E) a reconnect while a refusal waits (#1199's rejoin): an agent's RunChecks running, Full size pressed (KiCad refuses the
        # geometry while it checks), then the hub stopped past the client's 35 s heartbeat so the client drops and re-makes its connection
        # to the same hub and KiCad; the frame must end visible at the LAST rect
        wait_for(kframe, 10, 0.2); time.sleep(0.5)
        log0 = len(client_log().splitlines()); n0 = len(PUSHES)
        th = threading.Thread(target=lambda: ask('kicad_ipc', {'path': pcbf, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB'}}), daemon=True)
        te = time.time(); th.start(); time.sleep(0.25); press_rect('kicad:full')
        refused_e = wait_for(lambda: any('kicad_host_refused' in l for l in client_log().splitlines()[log0:]), 5, 0.05)
        os.kill(hub.pid, __import__('signal').SIGSTOP)  # the client hears nothing past its 35 s heartbeat
        try: time.sleep(float(os.environ.get('SWAP_HUB_STOP_S', '45')))
        finally: os.kill(hub.pid, __import__('signal').SIGCONT)
        # the reconnect: a second "status":"Connected" (the client logs one per connection) after the stop
        rejoined = wait_for(lambda: any('"status":"Connected"' in l or '"status":"Reconnecting' in l for l in client_log().splitlines()[log0:]), 30, 0.2)
        th.join(60); host_settled(20); time.sleep(2.0)
        lines = client_log().splitlines()[log0:]
        last = next((j['kicad_host_push'] for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None); lastvis = next((j.get('visible') for _, j in reversed(PUSHES) if j.get('kicad_host_push')), None)
        f = kframe(); oke = bool(lastvis) and bool(f and last) and all(abs(a - b) <= 2 for a, b in zip(f[3], last))
        status_e = [l.split("GPUI_LOG ", 1)[-1][:90] for l in lines if '"status"' in l][:3]  # outside the f-string: the Mac mini's Python is 3.9
        print(f'INFO swap E: refusal waiting before the stop {bool(refused_e)}; reconnect seen {bool(rejoined)} ({status_e}); pushes {[(round((t - te) * 1000), j["kicad_host_push"], j.get("visible"), j.get("retry")) for t, j in PUSHES[n0:] if j.get("kicad_host_push")]}; frame {f and f[3]}; last {last}', flush=True)
        check(bool(rejoined), 'swap E: the client dropped and re-made its hub connection')
        check(oke, f'swap E: after a reconnect with a refusal waiting, the frame ends visible at the LAST rect (frame {f and f[3]}, last {last}; refusal before {bool(refused_e)})')
        # #1225 (PR #1261): the new connection's pictures keep the Board viewer open. Each picture the client takes logs the hub's
        # thread.viewer.open (kicad_pane.rs kicad_warm, "kicad_pic"); from the reconnect on, every one must say board, the first included
        # (before #1261 the fresh session's first picture said no viewer, which shut it)
        ri = next((i for i, l in enumerate(lines) if '"status":"Connected"' in l), None)
        pics = []
        for l in (lines[ri + 1:] if ri is not None else []):
            if '"kicad_pic"' in l:
                try: pics.append(json.loads(l.split('GPUI_LOG ', 1)[1]).get('viewer_open'))
                except Exception: pass
        print(f'INFO swap E: after the reconnect the hub says viewer_open {pics[:6]}{" ..." if len(pics) > 6 else ""} ({len(pics)} pictures; last {pics[-1] if pics else None!r})', flush=True)
        check(bool(pics) and all(v == 'board' for v in pics), f'swap E: after the reconnect every picture keeps the Board viewer open (#1225; {len(pics)} pictures, not board: {[v for v in pics if v != "board"][:3]})')
        press_rect('kicad:full'); time.sleep(1.0)
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=hide (#1198): Cmd+H with KiCad's canvas focused hides Backplane and KiCad's frame (a runtime with comment-key-63: the
    # host reserves Ctrl+H); unhidden, KiCad's frame is directly above Backplane's window again and a click gives KiCad the keyboard; Cmd+H
    # with Backplane focused does the same. A runtime without comment-key-63 keeps Cmd+H KiCad's own (high contrast): nothing hides ----
    if os.environ.get('MAC_MODE') == 'hide':
        if DRIVER == 'vnc':
            # the Mac mini gate (#1198): input goes through the relay as everywhere, but this row also unhides and activates Backplane and
            # reads app and window state, which a process in the GUI session may do without a permission (the gate's venv has PyObjC);
            # assigned as globals, so no other mode's use of the names in this function turns local
            import importlib
            globals()['Quartz'] = importlib.import_module('Quartz'); globals()['NSRunningApplication'] = importlib.import_module('AppKit').NSRunningApplication
            KEYNAME[4] = 'h'
        caps = json.load(open(os.path.join(RT, 'manifest.json'))).get('capabilities') or []
        reserved_h = 'comment-key-63' in caps
        P = last_push()[0]
        def kicad_over_pane():
            out = []
            for w in wins():
                if w[1] != kpid: continue
                x0, y0 = max(w[3][0], P[0]), max(w[3][1], P[1]); x1, y1 = min(w[3][0] + w[3][2], P[0] + P[2]), min(w[3][1] + w[3][3], P[1] + P[3])
                if x1 > x0 and y1 > y0 and (x1 - x0) * (y1 - y0) > 0.3 * P[2] * P[3]: out.append(w)
            return out
        def ours_on_screen(): return any(w[0] == cw[0] for w in wins())
        def unhide():
            # as a person's Dock click or Cmd+Tab: Backplane is the active app before the canvas click (the case that fails, e2e-macos
            # 44540065022: Backplane active, a click on KiCad's frame does not make KiCad active). As textkeys: wait for it; when the request
            # from here is refused (macOS 14+), System Events on the runner, a click on Backplane's title bar on the Mac mini (no
            # Accessibility grant there; its empty part, left of the title, as win_move uses)
            a = NSRunningApplication.runningApplicationWithProcessIdentifier_(client.pid); a.unhide(); time.sleep(0.4); a.activateWithOptions_(0)
            if not wait_for(lambda: active(client.pid), 2, 0.1):
                if DRIVER == 'quartz':
                    osa(f'tell application "System Events" to set frontmost of (first process whose unix id is {client.pid}) to true')
                else:
                    b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + 120, b[1] + 12)
            check(bool(wait_for(lambda: active(client.pid), 5, 0.1)), 'hide: unhidden, Backplane is the active app again')
            time.sleep(0.5)  # the activation settled: no late activation of ours comes after the click
        def canvas_click(): click(P[0] + P[2] * 0.5, P[1] + P[3] * 0.5); time.sleep(0.6); key(K_ESC); time.sleep(0.3)
        for who in ('kicad', 'backplane'):
            if who == 'kicad': canvas_click()
            else:
                cwa = next((w for w in wins() if w[0] == cw[0]), cw); click(cwa[3][0] + 200, cwa[3][1] + 28 + 300); time.sleep(1)
            print(f'INFO hide: Cmd+H with {who} focused (KiCad active {active(kpid)}; the runtime reserves Ctrl+H: {reserved_h})', flush=True)
            key(4, CMD)
            hid = wait_for(lambda: not ours_on_screen() and not kicad_over_pane(), 5, 0.1)
            if who == 'kicad' and not reserved_h:
                check(not hid and ours_on_screen(), 'hide: a runtime without comment-key-63 keeps Cmd+H KiCad\'s own (nothing hides)')
                continue
            check(bool(hid), f'hide: Cmd+H with {who} focused hides Backplane and KiCad\'s frame (Backplane on screen {ours_on_screen()}; KiCad over the pane {len(kicad_over_pane())})')
            unhide()
            back = wait_for(kicad_frame, 10, 0.1); shot(f'hide-{who}-back')
            check(bool(back), f'hide: unhidden, KiCad\'s frame is directly above Backplane\'s window over the pane ({who})')
            if back:
                P = last_push()[0]
                # what is under the click, every layer (wins() keeps layer 0 only): e2e-macos 44540065022, the Backplane-focused case, a
                # click there left Backplane active and never reached KiCad though KiCad's frame was directly above Backplane's window
                cx, cy = P[0] + P[2] * 0.5, P[1] + P[3] * 0.5
                under = [(int(w.get('kCGWindowNumber', 0)), int(w.get('kCGWindowOwnerPID', 0)), w.get('kCGWindowOwnerName', ''), int(w.get('kCGWindowLayer', 0)),
                          round(float(w.get('kCGWindowAlpha', 1)), 2), w.get('kCGWindowName') or '', tuple(round(v) for v in (b['X'], b['Y'], b['Width'], b['Height'])))
                         for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID) or []
                         for b in [w.get('kCGWindowBounds') or {}] if b and b['X'] <= cx < b['X'] + b['Width'] and b['Y'] <= cy < b['Y'] + b['Height']]
                ka = NSRunningApplication.runningApplicationWithProcessIdentifier_(kpid)
                print(f'INFO hide: under the click at ({round(cx)}, {round(cy)}) front to back ({who}): {under[:6]}; KiCad hidden {bool(ka and ka.isHidden())}, '
                      f'Backplane pid {client.pid}, KiCad pid {kpid}', flush=True)
                canvas_click()
                got = active(kpid); time.sleep(1.0); kept = active(kpid)
                print(f'INFO hide: after the click ({who}): KiCad active {got}, a second on {kept}; Backplane active {active(client.pid)}', flush=True)
                check(got and kept, f'hide: after the unhide a click gives KiCad the keyboard, and it keeps it ({who})')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=textkeys (#1172, #1198): Backplane's own keys on a Mac, with the editor open over the pane. In the message box: Option+Left/Right
    # by word, Cmd+Left/Right to the line's start/end, Cmd+Up/Down to the text's, Shift selecting, Option+Backspace a word, Cmd+Backspace to the
    # line's start; a backtick is typed (it no longer shows the terminal from a text field). The text is read back by Cmd+A, Cmd+C (pbpaste).
    # Cmd+H with the message box, the terminal (Cmd+J) and a viewer (the Files page) focused hides Backplane's window; unhidden, it is back.
    # Last, Cmd+W with the terminal focused closes the window by the red button's path (the client's "close" metric, exit 0); the hub stays up ----
    if os.environ.get('MAC_MODE') == 'textkeys':
        if DRIVER != 'quartz': raise SystemExit('MAC_MODE=textkeys posts Quartz events and uses System Events: the e2e-macos runner only')
        K_LEFT, K_RIGHT, K_DOWN, K_UP, K_RET, K_GRAVE, K_H, K_W = 123, 124, 125, 126, 36, 50, 4, 13
        def press_rect(i):
            r = rects().get(i)
            if not r: return False
            b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2); time.sleep(0.6); return True
        def box():  # the message box: no rect of its own; it is the strip above Send (Send's row is under it, the box 60 pt high)
            r = rects().get('send')
            if not r: return False
            b = next((w for w in wins() if w[0] == cw[0]), cw)[3]; click(b[0] + r['x'] - 120, b[1] + 28 + r['y'] - 38); time.sleep(0.6); return True
        def where(what):
            print(f'INFO textkeys {what}: Backplane active {active(client.pid)}, KiCad active {active(kpid)}, terminal opens so far {hub_log().count("term.open")}', flush=True)
        def composer_text():
            subprocess.run(['pbcopy'], input=b'<nothing copied>'); key(K_A, CMD); time.sleep(0.15); key(8, CMD); time.sleep(0.3)
            t = subprocess.run(['pbpaste'], capture_output=True).stdout.decode('utf8', 'replace'); key(K_RIGHT); time.sleep(0.15); return t
        def typed(t):
            # each character with no modifier flags of its own: right after a Cmd chord a posted character took Cmd from the session's
            # state (run 44268640536: the Y after Cmd+Left arrived as Cmd+A, key code 0, and selected everything)
            for ch in t:
                for down in (True, False):
                    e = Quartz.CGEventCreateKeyboardEvent(None, 0, down); Quartz.CGEventKeyboardSetUnicodeString(e, 1, ch); Quartz.CGEventSetFlags(e, 0)
                    Quartz.CGEventPost(Quartz.kCGHIDEventTap, e); time.sleep(0.02)
            time.sleep(0.2)
        def ours_on_screen(): return any(w[0] == cw[0] for w in wins())
        def unhide():
            # unhidden, macOS had Finder in front and a click meant for us went to its window (run 44268640536): Backplane is made the
            # front app by System Events (granted on the runner, as win_move uses it) and must be the active app before the next case
            a = NSRunningApplication.runningApplicationWithProcessIdentifier_(client.pid); a.unhide(); time.sleep(0.4); a.activateWithOptions_(0)
            if not wait_for(lambda: active(client.pid), 2, 0.1):
                osa(f'tell application "System Events" to set frontmost of (first process whose unix id is {client.pid}) to true')
            check(bool(wait_for(lambda: active(client.pid), 5, 0.1)), 'textkeys: unhidden, Backplane is the active app again')
        def hub_up(): return subprocess.run(['curl', '-s', '-m2', '-o', '/dev/null', f'http://127.0.0.1:{port}/hello']).returncode == 0
        check(box(), 'textkeys: the message box is on screen (under Send\'s rect)'); where('the message box clicked'); key(K_A, CMD); key(K_DEL)
        typed('keep'); key(K_RET, SHIFT); typed('ab cd')
        check(composer_text() == 'keep\nab cd', f'textkeys: typed two lines ({composer_text()!r})')
        steps = [('Option+Left, X: before the word', lambda: (key(K_LEFT, ALT), typed('X')), 'keep\nab Xcd'),
                 ('Cmd+Left, Y: the line\'s start', lambda: (key(K_LEFT, CMD), typed('Y')), 'keep\nYab Xcd'),
                 ('Cmd+Right, Option+Backspace: a word back from the line\'s end', lambda: (key(K_RIGHT, CMD), key(K_DEL, ALT)), 'keep\nYab '),
                 ('Option+Shift+Left, Z: the word selected and typed over', lambda: (key(K_LEFT, ALT | SHIFT), typed('Z')), 'keep\nZ'),
                 ('Cmd+Up, S: the text\'s start', lambda: (key(K_UP, CMD), typed('S')), 'Skeep\nZ'),
                 ('Cmd+Down, E: the text\'s end', lambda: (key(K_DOWN, CMD), typed('E')), 'Skeep\nZE'),
                 ('Cmd+Shift+Left, Q: the line selected back to its start', lambda: (key(K_LEFT, CMD | SHIFT), typed('Q')), 'Skeep\nQ'),
                 ('Option+Right after Cmd+Up: by word, not to the line\'s end', lambda: (key(K_UP, CMD), key(K_RIGHT, ALT), typed('!')), 'Skeep!\nQ'),
                 ('Cmd+Down, Cmd+Backspace: back to the line\'s start', lambda: (key(K_DOWN, CMD), key(K_DEL, CMD)), 'Skeep!\n'),
                 ('a backtick is typed, no terminal', lambda: key(K_GRAVE), 'Skeep!\n`')]
        for what, do, want in steps:
            do(); time.sleep(0.3); got = composer_text()
            check(got == want, f'textkeys: {what} ({got!r}, want {want!r})')
        key(K_A, CMD); key(K_DEL); time.sleep(0.3)
        # after an unhide the runner's activation from this script may not make Backplane the active app (macOS 14+): each case starts with a click in it
        for who, focus in (('the message box', lambda: box()), ('the terminal', lambda: (box(), key(K_J, CMD), time.sleep(1.2))),
                           ('a viewer (the Files page)', lambda: (box(), key(K_J, CMD), time.sleep(0.6), press_rect('rail:Files'), time.sleep(1.0)))):
            focus(); where(who); key(K_H, CMD)
            hid = wait_for(lambda: not ours_on_screen(), 5, 0.1)
            check(bool(hid), f'textkeys: Cmd+H with {who} focused hides Backplane\'s window')
            unhide(); back = wait_for(ours_on_screen, 10, 0.1)
            check(bool(back), f'textkeys: unhidden, Backplane\'s window is back ({who})'); time.sleep(0.8)
        press_rect('rail:Board'); time.sleep(1.5)
        box(); key(K_J, CMD); time.sleep(1.2); where('before Cmd+W')
        check(hub_up(), 'textkeys: the hub answers before Cmd+W')
        key(K_W, CMD)
        ended = wait_for(lambda: client.poll() is not None, 10, 0.1)
        check(bool(ended) and client.returncode == 0, f'textkeys: Cmd+W with the terminal focused ends the client, exit 0 ({client.poll()})')
        check('"event":"close"' in client_log(), 'textkeys: by the window\'s close path (its "close" metric in the log)')
        check(hub_up(), 'textkeys: the hub stays up after Cmd+W')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=zoom: the zoom chords in the embedded board editor (fork embed's fix/mac-cmd-equals-zoom: plain Cmd+= zooms in), judged by
    # the board's size on screen (KiCad has no view readback): the extent of the board's coloured pixels (copper, silk; the grid, ratsnest and
    # text are grey) inside the canvas, where the canvas is where those pixels differ between two zoom levels (the panels' swatches do not).
    # From a fit: Cmd+- twice (smaller each time), plain Cmd+= (larger: the row under test), Cmd+Shift+= (larger: KiCad's own Ctrl++,
    # the control), Cmd+- (smaller), Cmd+0 (back to the fit). The pointer stays on the board's centre, KiCad's zoom anchor ----
    if os.environ.get('MAC_MODE') == 'zoom':
        if DRIVER != 'quartz': raise SystemExit('MAC_MODE=zoom reads pixels: the e2e-macos runner only')
        def grab(r):
            img = Quartz.CGWindowListCreateImage(Quartz.CGRectMake(*r), Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID, Quartz.kCGWindowImageNominalResolution)
            w, h, bpr = Quartz.CGImageGetWidth(img), Quartz.CGImageGetHeight(img), Quartz.CGImageGetBytesPerRow(img)
            d = bytes(Quartz.CGDataProviderCopyData(Quartz.CGImageGetDataProvider(img)))
            sx, sy = w / r[2], h / r[3]; m = set()
            for y in range(0, int(r[3]), 2):
                row = int(y * sy) * bpr
                for x in range(0, int(r[2]), 2):
                    o = row + int(x * sx) * 4; px = d[o:o + 3]
                    if max(px) - min(px) > 70 and max(px) > 90: m.add((x, y))
            return m
        def extent(m, R=None):
            q = [p for p in m if not R or (R[0] <= p[0] <= R[2] and R[1] <= p[1] <= R[3])]
            if len(q) < 20: return None
            xs = sorted(p[0] for p in q); ys = sorted(p[1] for p in q); n = len(q)
            # the 2nd and 98th percentiles, so a stray pixel does not make the size
            return (xs[n // 50], ys[n // 50], xs[-1 - n // 50], ys[-1 - n // 50])
        P = last_push()[0]; pane = (P[0], P[1], P[2], P[3])
        def to_canvas(): key(K_ESC); time.sleep(0.3); click(P[0] + P[2] * 0.5, P[1] + P[3] * 0.5); time.sleep(0.4); key(K_ESC); time.sleep(0.3)
        tr_path = os.path.join(OUT, 'kicad-embed-trace.log')
        def trace_from(n): return [l.split('macos: ', 1)[-1].strip() for l in open(tr_path, errors='replace').read().splitlines()[n:] if 'macos: ' in l] if os.path.isfile(tr_path) else []
        def tr_n(): return len(open(tr_path, errors='replace').read().splitlines()) if os.path.isfile(tr_path) else 0
        def chord(code, fl, tag):
            n = tr_n(); key(code, fl); time.sleep(1.8); m = grab(pane); relay(op='png', rect=list(pane), path=os.path.join(OUT, f'zoom-{tag}.png'))
            return m, trace_from(n)
        to_canvas(); key(115); time.sleep(2.0)  # Home: KiCad's Zoom to Fit
        m0 = grab(pane); relay(op='png', rect=list(pane), path=os.path.join(OUT, 'zoom-0-fit.png'))
        e0 = extent(m0)
        if e0: relay(op='move', x=P[0] + (e0[0] + e0[2]) / 2, y=P[1] + (e0[1] + e0[3]) / 2); time.sleep(0.4)
        m1, t1 = chord(27, CMD, '1-out'); m2, t2 = chord(27, CMD, '2-out')
        R = extent(m0 ^ m2); print(f'INFO zoom: canvas content box {R} (pane {P})', flush=True)
        if R:
            c2 = extent(m2, R)
            if c2: relay(op='move', x=P[0] + (c2[0] + c2[2]) / 2, y=P[1] + (c2[1] + c2[3]) / 2); time.sleep(0.4)
        m3, t3 = chord(24, CMD, '3-cmd-equals'); m4, t4 = chord(24, CMD | SHIFT, '4-cmd-shift-equals')
        m5, t5 = chord(27, CMD, '5-out'); m6, t6 = chord(29, CMD, '6-cmd-0')
        def size(m):
            e = extent(m, R) if R else None
            return ((e[2] - e[0]) + (e[3] - e[1])) / 2 if e else 0.0
        s = [size(m) for m in (m0, m1, m2, m3, m4, m5, m6)]
        RES['zoom_sizes'] = s; print(f'INFO zoom: board size (pt) fit {s[0]:.0f}, Cmd+- {s[1]:.0f}, Cmd+- {s[2]:.0f}, Cmd+= {s[3]:.0f}, Cmd+Shift+= {s[4]:.0f}, Cmd+- {s[5]:.0f}, Cmd+0 {s[6]:.0f}', flush=True)
        for tag, t in (('Cmd+-', t1), ('Cmd+=', t3), ('Cmd+Shift+=', t4), ('Cmd+0', t6)): print(f'INFO zoom: trace after {tag}: {t}', flush=True)
        ok_m = bool(R) and s[0] > 40
        check(ok_m, f'zoom: the board is measurable in the canvas (fit {s[0]:.0f} pt)')
        if ok_m:
            check(s[1] < s[0] * 0.9 and s[2] < s[1] * 0.9, f'zoom: Cmd+- zooms out ({s[0]:.0f} -> {s[1]:.0f} -> {s[2]:.0f})')
            check(s[3] > s[2] * 1.12, f'zoom: plain Cmd+= zooms in ({s[2]:.0f} -> {s[3]:.0f})')
            check(s[4] > s[3] * 1.12, f'zoom: Cmd+Shift+= (KiCad\'s Ctrl++) zooms in ({s[3]:.0f} -> {s[4]:.0f})')
            check(s[5] < s[4] * 0.9, f'zoom: Cmd+- zooms out again ({s[4]:.0f} -> {s[5]:.0f})')
            check(abs(s[6] - s[0]) <= s[0] * 0.08, f'zoom: Cmd+0 zooms to fit ({s[5]:.0f} -> {s[6]:.0f}, the fit {s[0]:.0f})')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=first: a fresh home's first Edit puts KiCad over the pane within 30 s (the checks above), and stops ----
    if os.environ.get('MAC_MODE') == 'first':
        check(RES['edit_to_frame_s'] <= 30, f'first Edit on a fresh home: KiCad over the pane within 30 s ({RES["edit_to_frame_s"]} s)')
        if os.environ.get('FRESH_AGENT') == 'during':
            check('File Open Warning' not in hub_log(), 'fresh Edit with an agent during the start: no "File Open Warning" dialog')
            lck = os.path.join(OUT, 'proj', '~ecc83-pp.kicad_pro.lck')
            holders = subprocess.run(['lsof', '-t', '--', lck], capture_output=True, text=True).stdout.split() if os.path.exists(lck) else []
            check(len(holders) <= 1, f'fresh Edit with an agent during the start: at most one process holds the project lock ({holders})')
            sl = open(os.path.join(OUT, 'standin.log'), errors='replace').read() if os.path.isfile(os.path.join(OUT, 'standin.log')) else ''
            check('MCPCALL kicad_ipc_status' in sl, 'fresh Edit with an agent during the start: the agent\'s kicad_ipc_status was answered')
            pro = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pro')
            headless = [l for l in hub_log().splitlines() if l.startswith('kicad ipc: ') and pro in l and ' up' in l]
            check(not headless, f'fresh Edit with an agent during the start: no headless KiCad opens the project meanwhile ({headless[:1]})')
            ans = next((l for l in sl.splitlines() if l.startswith('MCPCALL kicad_ipc_status')), '')
            check('gui (fork)' in ans, f'fresh Edit with an agent during the start: the agent is answered by the editor\'s KiCad, "gui (fork)" ({ans[:220]})')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=restart: Backplane killed mid-edit, restarted: Edit painted, no dialog; KiCad killed: Edit brings a new one;
    # Backplane started with KiCad gone and Edit at once: one KiCad ----
    if os.environ.get('MAC_MODE') == 'restart':
        def kicads():
            kids = {}
            for l in subprocess.run(['ps', '-axo', 'pid=,ppid=,comm='], capture_output=True, text=True).stdout.splitlines():
                parts = l.split(None, 2)
                if len(parts) == 3: kids.setdefault(int(parts[1]), []).append((int(parts[0]), parts[2]))
            out, todo = [], [hub.pid]
            while todo:
                for pid, comm in kids.get(todo.pop(), []):
                    todo.append(pid)
                    if comm.rsplit('/', 1)[-1] in ('kicad', 'pcbnew', 'eeschema'): out.append(pid)
            return out
        def start_client(tag):
            CLIENT_LOG[0] = os.path.join(OUT, f'client-{tag}.log')
            c = subprocess.Popen([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', os.path.join(OUT, 'home/token'), '--script', os.path.join(OUT, 'script.json'), '--exit-after', '600'],
                                 env=cenv, stdout=open(CLIENT_LOG[0], 'w'), stderr=subprocess.STDOUT); procs.append(c)
            return c
        # R1: Backplane killed in the middle of an edit (a selection made in KiCad), started again: Edit shows the editor, no dialog
        p = last_push(); click(p[0][0] + p[0][2] * 0.1, p[0][1] + p[0][3] * 0.9); time.sleep(0.5); key(K_A, CMD); time.sleep(0.8)
        d0 = hub_log().count('ET_DIALOG_OPENED')
        client.kill(); client.wait(10)
        gone = wait_for(lambda: not kicad_frame(), 5, 0.1)
        client = start_client('r1'); t0 = time.time()
        cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
        f = wait_for(kicad_frame, 60, 0.25); RES['r1_edit_s'] = round(time.time() - t0, 1)
        shot('r1-after-restart')
        check(bool(gone), 'restart 1: Backplane killed mid-edit: KiCad\'s frame leaves the screen')
        check(bool(f) and hub_log().count('ET_DIALOG_OPENED') == d0, f'restart 1: Backplane started again, Edit shows the editor over the pane with no dialog ({RES["r1_edit_s"]} s)')
        # R2: KiCad killed: Edit brings a new one over the pane
        old = kicads(); print(f'INFO KiCad processes before the kill: {old}', flush=True)
        for pid in old:
            try: os.kill(pid, 9)
            except OSError: pass
        wait_for(lambda: not kicad_frame(), 10, 0.2); time.sleep(3)
        wait_for(lambda: rects().get('kicad:open-again') or rects().get('kicad:retry'), 15, 0.5)
        say_closed = 'closed_unexpectedly' in client_log()
        e = rects().get('kicad:open-again') or rects().get('kicad:retry') or rects().get('ui:Edit'); cw2 = next((w for w in wins() if w[0] == cw[0]), cw)
        check(say_closed and bool(rects().get('kicad:open-again')), 'restart 2: KiCad killed: the pane says KiCad closed unexpectedly and offers Open the editor again (no "could not start")')
        RES['r2_pressed'] = e and e['id']; shot('r2-offered')
        if e: click(cw2[3][0] + e['x'] + e['w'] / 2, cw2[3][1] + 28 + e['y'] + e['h'] / 2)
        f2 = wait_for(lambda: (k := kicad_frame()) and k[1] not in old and k, 90, 0.5); shot('r2-after-kill')
        check(bool(f2), f'restart 2: KiCad killed: Edit brings a new KiCad over the pane ({e and e["id"]})')
        # R3: Backplane and KiCad both gone; Backplane started, its script presses Edit at once: one KiCad process for the project
        client.kill(); client.wait(10)
        for pid in kicads():
            try: os.kill(pid, 9)
            except OSError: pass
        time.sleep(3)
        client = start_client('r3')
        cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
        f3 = wait_for(kicad_frame, 90, 0.5); time.sleep(8)
        n = kicads(); RES['r3_kicads'] = n; shot('r3-warm-edit')
        check(bool(f3) and len(n) == 1, f'restart 3: Backplane started with Edit at once: KiCad over the pane, one KiCad process ({n})')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=bs: ⌫ in the embedded editor (fork embed's fix/mac-backspace-delete): 1 Cmd+A then ⌫ deletes (board, schematic),
    # 2 ⌫ with nothing selected deletes what is under the pointer, 3 ⌫ while routing takes back the last segment (screenshots),
    # 4 ⌫ in a text field edits the text (Find's field; screenshot), 5 fn+delete still deletes. KiCad's edit events judge 1, 2, 5.
    if os.environ.get('MAC_MODE') == 'bs':
        K_X, K_Z = 7, 6
        KEYNAME.update({7: 'x', 6: 'z'}) if DRIVER == 'vnc' else None
        def evs():
            out = []
            for ln in hub_log().splitlines():
                i = ln.find('kicad embed event: ')
                if i < 0: continue
                try: out.append(json.loads(ln[i + 19:]))
                except ValueError: pass
            return out
        seq = lambda: max((int(e.get('seq', 0)) for e in evs()), default=0)
        def edits_since(n, frame=None):  # KiCad's own edit events after seq n: (created, deleted) counts each
            r = []
            for e in evs():
                if int(e.get('seq', 0)) > n and e.get('type') == 'ET_DOCUMENT_EDITED':
                    try: d = json.loads(e.get('detail') or '{}')
                    except ValueError: d = {}
                    r.append((len(d.get('created') or []), len(d.get('deleted') or [])))
            return r
        def checks_idle(n, t=20):  # the checks KiCad runs after an edit have reported (ET_CHECKS_DONE after seq n)
            return wait_for(lambda: any(int(e.get('seq', 0)) > n and e.get('type') == 'ET_CHECKS_DONE' for e in evs()), t)
        def sel_refs():
            for e in reversed(evs()):
                if e.get('type') == 'ET_SELECTION_CHANGED': return (e.get('selection') or {}).get('references') or []
            return []
        def board_click(fx=0.5, fy=0.5):
            p = last_push(); click(p[0][0] + p[0][2] * fx, p[0][1] + p[0][3] * fy); time.sleep(0.6)
        def deleted(after, tag):
            n = seq(); after()
            ok = bool(wait_for(lambda: any(d for c, d in edits_since(n)), 4)); shot(f'bs-{tag}')
            if ok:  # undo once KiCad's checks are done: the next case starts from the same board
                checks_idle(n); m = seq(); key(K_Z, CMD); wait_for(lambda: any(c for c, d in edits_since(m)), 4); checks_idle(m)
            return ok
        if os.environ.get('BS_PROBE') == 'undo':  # which delete path leaves Cmd+Z working: fn+delete, ⌫, ⌫ then Cmd+S
            def try_undo(tag):
                m = seq(); key(K_Z, CMD); u = bool(wait_for(lambda: any(c for c, d in edits_since(m)), 4)); checks_idle(m, 8)
                print(f'INFO probe {tag}: Cmd+Z {"undoes" if u else "does nothing"}', flush=True); return u
            for tag, k in (('fn+delete', K_FDEL), ('⌫', K_DEL), ('fn+delete again', K_FDEL)):
                key(K_ESC); time.sleep(0.3); board_click(0.1, 0.9); n = seq(); key(K_A, CMD); time.sleep(0.6); key(k)
                d = bool(wait_for(lambda: any(d for c, d in edits_since(n)), 4)); checks_idle(n, 8)
                print(f'INFO probe {tag}: delete {"lands" if d else "does nothing"}', flush=True)
                if d and not try_undo(tag):
                    board_click(0.1, 0.9); try_undo(tag + ', after a click')
                    if not try_undo(tag + ', again'):
                        key(1, CMD); time.sleep(2); try_undo(tag + ', after Cmd+S')
            raise SystemExit(0)
        if os.environ.get('BS_PROBE') == 'route':  # 3 and 4 judged without pixels where it can be: tracks counted from KiCad's edit events
            def pt(fx, fy):
                p = last_push(); return p[0][0] + p[0][2] * fx, p[0][1] + p[0][3] * fy
            def route(y, back):
                # p1 → p2 across, p2 → p3 down: one track each. Without ⌫, finish at p3 (2 tracks). With ⌫ after p3, finish where the
                # route then ends, p2: 1 track if ⌫ took the last segment back, 2 or more if it did not
                key(K_ESC); time.sleep(0.3); key(K_ESC); time.sleep(0.3); key(K_X); time.sleep(0.8)
                n = seq(); p1, p2, p3 = pt(0.22, y), pt(0.36, y), pt(0.36, y + 0.08)
                for q in (p1, p2, p3): click(*q); time.sleep(0.7)
                end = p3
                if back:
                    relay(op='move', x=p3[0] + 30, y=p3[1] + 25); time.sleep(0.8); shot('bs-route-before')
                    key(K_DEL); time.sleep(0.8); shot('bs-route-after'); end = p2
                relay(op='move', x=end[0], y=end[1]); time.sleep(0.5); click(*end); click(*end); time.sleep(1.5)
                created = sum(c for c, d in edits_since(n)); key(K_ESC); time.sleep(0.3); key(K_ESC); time.sleep(0.6)
                return created
            board_click(0.1, 0.9)
            t0 = open(os.path.join(OUT, 'kicad-embed-trace.log'), errors='replace').read().count('Backspace handed')
            a = route(0.56, False); b = route(0.70, True)
            t1 = open(os.path.join(OUT, 'kicad-embed-trace.log'), errors='replace').read().count('Backspace handed')
            print(f'INFO bs 3: without ⌫ the route committed {a} tracks; with ⌫ and finished where it then ends, {b} (⌫ handed to wx {t1 - t0} times)', flush=True)
            check(a >= 2 and b == 1, 'bs 3: ⌫ while routing takes back the last segment (1 track committed, not 2)')
            # 4: Find's field: a, b, c as key presses, then ⌫; the field's text in a picture of Backplane's window only
            board_click(0.1, 0.9); key(K_F, CMD); time.sleep(1.5)
            for k in (0, 11, 8): key(k); time.sleep(0.15)
            shot('bs-text-before'); key(K_DEL); time.sleep(0.6); shot('bs-text-field'); key(K_ESC); time.sleep(1.0)
            print('INFO bs 4: Find\'s field in bs-text-before.png ("abc") and bs-text-field.png ("ab")', flush=True)
            raise SystemExit(1 if FAILS else 0)
        if os.environ.get('BS_PROBE') == 'focus':  # a click in Backplane takes the keyboard back from the embedded KiCad (the lead's item 5)
            cwa = next((w for w in wins() if w[0] == cw[0]), cw)[3]
            def home_has(text):
                r = subprocess.run(['grep', '-rlF', text, os.path.join(OUT, 'home')], capture_output=True, text=True); return bool(r.stdout.strip())
            for i, (where, at) in enumerate((('the chat box', (cwa[0] + 230, cwa[1] + 620)), ('the chat box after Cmd+A on the board', (cwa[0] + 230, cwa[1] + 620)))):
                board_click(0.1, 0.9); time.sleep(0.5)
                if i == 1: key(K_A, CMD); time.sleep(0.6)
                k_active = active(kpid)
                n = seq(); click(*at); time.sleep(1.0)
                after_active = active(kpid)
                mark = f'lanemfocus{i}{int(time.time()) % 100000}'
                relay(op='text', text=mark); time.sleep(0.5); key(36); time.sleep(3)
                in_bp = home_has(mark)
                k_ev = sorted({e.get('type') for e in evs() if int(e.get('seq', 0)) > n} - {'ET_PROGRESS', 'ET_FRAME_DRAWN', 'ET_CHECKS_DONE'})
                c, d = (sum(x) for x in zip(*edits_since(n))) if edits_since(n) else (0, 0)
                print(f'INFO focus {i}: KiCad active before the click {k_active}, after {after_active}; KiCad events after the click {k_ev}; edits created {c} deleted {d}', flush=True)
                check(in_bp and not (c or d), f'focus: after a click in {where}, typing goes to Backplane (the message "{mark}" is in the thread{"" if in_bp else ": NOT found"}; KiCad edits {c + d})')
            raise SystemExit(1 if FAILS else 0)
        if os.environ.get('BS_PROBE') == 'keys':  # climb-j's edit-keys table (lib/edit_keys_table.json), same row ids, plus extra_* menu chords
            if DRIVER == 'vnc': KEYNAME.update({6: 'z', 7: 'x', 9: 'v', 16: 'y', 35: 'p', 31: 'o', 12: 'q', 5: 'g', 24: '=', 27: '-'})
            K_Z, K_X, K_V, K_Y, K_S = 6, 7, 9, 16, 1
            W = 4
            sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac')); import hub_ws
            token = open(os.path.join(OUT, 'home/token')).read().strip(); pcbf = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb'); schf = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_sch')
            tr = lambda: open(os.path.join(OUT, 'kicad-embed-trace.log'), errors='replace').read().splitlines() if os.path.isfile(os.path.join(OUT, 'kicad-embed-trace.log')) else []
            def since(n, typ): return [e for e in evs() if int(e.get('seq', 0)) > n and e.get('type') == typ]
            def press(code, flags=0):  # (seq before, trace lines before)
                n, t = seq(), len(tr()); key(code, flags); return n, t
            def ed(n): r = edits_since(n); return sum(c for c, d in r), sum(d for c, d in r)
            def wait_ed(n, want):  # want: 'c' or 'd'
                wait_for(lambda: (ed(n)[0] if want == 'c' else ed(n)[1]) > 0, W); time.sleep(0.4); return ed(n)
            def mcalls(): return [l for l in open(os.path.join(OUT, 'standin.log'), errors='replace').read().splitlines() if l.startswith('MCPCALL')] if os.path.isfile(os.path.join(OUT, 'standin.log')) else []
            def ask(name, args, wait=60):
                n0 = len(mcalls())
                hub_ws.send(port, token, 'turn.start', {'thread': 't1', 'text': 'mcpcall:' + json.dumps({'name': name, 'arguments': args}), 'msg': f'as-{time.time_ns()}'})
                wait_for(lambda: len(mcalls()) > n0, wait, 0.3)
                l = mcalls()[-1] if len(mcalls()) > n0 else ''
                try: return json.loads(json.loads(l.split(' ', 2)[2])['result']['content'][0]['text'])
                except Exception: return {'raw': l[:300]}
            REAL = {'pcb': ['KOT_PCB_FOOTPRINT', 'KOT_PCB_SHAPE', 'KOT_PCB_TEXT', 'KOT_PCB_TRACE', 'KOT_PCB_VIA', 'KOT_PCB_ARC', 'KOT_PCB_ZONE'],
                    'sch': ['KOT_SCH_JUNCTION', 'KOT_SCH_NO_CONNECT', 'KOT_SCH_LINE', 'KOT_SCH_TEXT', 'KOT_SCH_LABEL', 'KOT_SCH_GLOBAL_LABEL', 'KOT_SCH_HIER_LABEL', 'KOT_SCH_SYMBOL']}
            MARK = {'pcb': 'KOT_PCB_MARKER', 'sch': 'KOT_SCH_MARKER'}
            def agent_items(E, f):  # an agent's read through the hub (GetItems): ({@type: count}, answered?, what it said)
                C = 'kiapi.common.commands.'
                d = ask('kicad_ipc', {'path': f, 'type': C + 'GetOpenDocuments', 'message': {'type': 'DOCTYPE_PCB' if E == 'pcb' else 'DOCTYPE_SCHEMATIC'}})
                doc = ((d.get('message') or {}).get('documents') or [None])[0]
                r = ask('kicad_ipc', {'path': f, 'type': C + 'GetItems', 'message': {'header': {'document': doc}, 'types': REAL[E] + [MARK[E]]}})
                msg = r.get('message') or {}
                if 'items' not in msg and msg.get('status') != 'IRS_OK': return {}, False, str(r)[:160]
                items = msg.get('items') or []
                byk = {}
                for it in items: k = str(it.get('@type', '?')).rsplit('.', 1)[-1]; byk[k] = byk.get(k, 0) + 1
                return byk, True, ''
            ROWS = []
            def row(ed_, rid, verdict, detail, t0=None):
                tl = [l.split('macos: ', 1)[-1] for l in tr()[t0:]] if t0 is not None else []
                tl = [l for l in tl if 'handed to wx' in l][-2:]
                ROWS.append((ed_, rid, verdict, detail)); line = f'ROW {ed_} {rid}: {verdict} ({detail}){"; trace: " + " | ".join(tl) if tl else ""}'
                gating = not rid.startswith('extra_')
                if gating: check(verdict == 'works', line)
                else: print('INFO ' + line, flush=True)
            def settle(n): checks_idle(n, 6); time.sleep(0.3)
            def canvas(): key(K_ESC); time.sleep(0.3); board_click(0.1, 0.9); time.sleep(0.3)
            def judge_cd(n, want, wrong):  # an edit of the wanted kind, or the opposite
                c, d = wait_ed(n, want)
                good = c if want == 'c' else d; bad = d if want == 'c' else c
                if good and not (wrong and bad): return 'works', f'created {c}, deleted {d}'
                if c or d: return 'wrong', f'created {c}, deleted {d}'
                return 'nothing', 'no edit event'
            def first_keys(E):
                # the very first keys on a fresh frame, no click (Edit gives KiCad the keyboard): Cmd+A selects, Cmd+C puts the editor's
                # text on the pasteboard, Cmd+V and a click paste every item (fork f71f546af8: a check no longer leaves the keys on a side panel)
                subprocess.run(['pbcopy'], input=b'lane-m-before-copy')
                n, t = press(K_A, CMD); wait_for(lambda: since(n, 'ET_SELECTION_CHANGED'), W); time.sleep(0.4)
                sel = [e.get('selection') or {} for e in since(n, 'ET_SELECTION_CHANGED')]
                k = max((len(x.get('references') or []) + len(x.get('nets') or []) for x in sel), default=0)
                key(8, CMD); time.sleep(0.8); pb = subprocess.run(['pbpaste'], capture_output=True).stdout.decode(errors='replace')
                kt = pb != 'lane-m-before-copy' and pb.lstrip().startswith('(')
                key(K_ESC); time.sleep(0.4)
                p = last_push(); relay(op='move', x=p[0][0] + p[0][2] * 0.5, y=p[0][1] + p[0][3] * 0.5); time.sleep(0.4)
                m = seq(); key(K_V, CMD); time.sleep(1.0); board_click(0.5, 0.5); c, d = wait_ed(m, 'c'); settle(m); key(K_ESC); time.sleep(0.4)
                ok = k > 1 and kt and c > 1
                row(E, 'first_keys_no_click', 'works' if ok else ('nothing' if not (k or kt or c) else 'wrong'),
                    f'Cmd+A selected {k} (refs+nets); the pasteboard after Cmd+C {"holds KiCad text (" + str(len(pb)) + " chars)" if kt else "has no KiCad text"}; Cmd+V and a click created {c}', t)
                if c: m = seq(); key(K_Z, CMD); wait_ed(m, 'd'); settle(m)
            def table(E, f):
                other_text = ''  # what KiCad's own Cmd+C puts on the pasteboard (copy_paste below records it): another KiCad's copy is the same text
                canvas(); canvas()
                n, t = press(K_A, CMD); wait_for(lambda: since(n, 'ET_SELECTION_CHANGED'), W); time.sleep(0.4)
                sel = [e.get('selection') or {} for e in since(n, 'ET_SELECTION_CHANGED')]
                many = any(len(x.get('references') or []) + len(x.get('nets') or []) > 1 or x.get('count', 0) > 1 for x in sel)
                row(E, 'select_all', 'works' if many else ('wrong' if sel else 'nothing'), f'{len(sel)} selection events; last {str(sel[-1])[:90] if sel else "-"}', t)
                n, t = press(K_FDEL); v = judge_cd(n, 'd', True); row(E, 'delete', *v, t)
                if v[0] == 'works':  # Cmd+Z at once, before KiCad's check: told apart from an undo list the check emptied
                    n, t = press(K_Z, CMD); v2 = judge_cd(n, 'c', True); row(E, 'extra_undo_at_once', *v2, t); settle(n)
                    if v2[0] == 'works':
                        canvas(); key(K_A, CMD); time.sleep(0.6); n = seq(); key(K_FDEL); judge_cd(n, 'd', True)
                settle(n)
                # redo at once after the undo's edit (before KiCad's in-editor check runs), and again after ET_CHECKS_DONE: on bp.13 the
                # check after an undo empties the redo list (climb-j, Linux), so a missing menu key and an emptied list are told apart
                n, t = press(K_Z, CMD); v = judge_cd(n, 'c', True); row(E, 'undo', *v, t); undo_ok = v[0] == 'works'
                # redo by Cmd+Y: the table's "where KiCad binds it"; KiCad on macOS binds redo to Cmd+Shift+Z only (redo_shift_z), so here n/a
                n, t = press(K_Y, CMD); v = judge_cd(n, 'd', True); row(E, 'extra_redo_cmd_y', v[0] + (' (n/a on macOS: KiCad binds redo to Cmd+Shift+Z)' if v[0] == 'nothing' else ''), v[1], t); settle(n)
                if v[0] == 'works': m = seq(); key(K_Z, CMD); wait_ed(m, 'c')
                n, t = press(K_Z, CMD | SHIFT); v = judge_cd(n, 'd', True)
                row(E, 'extra_redo_at_once', v[0], v[1] + ' (Cmd+Shift+Z at once after the undo, before KiCad\'s check)', t); settle(n)
                m = seq(); key(K_Z, CMD); wait_ed(m, 'c'); settle(m)  # undone again, and the check after it done: as a person who pauses
                n, t = press(K_Z, CMD | SHIFT); v = judge_cd(n, 'd', True)
                row(E, 'redo_shift_z', v[0], v[1] + ' (after ET_CHECKS_DONE; "nothing" here with extra_redo_at_once working = the check emptied the redo list)', t)
                settle(n)
                if v[0] == 'works': m = seq(); key(K_Z, CMD); wait_ed(m, 'c'); settle(m)
                # undo and redo every time: 10 of 10 each (the earlier 6-of-7 failures must not recur)
                canvas(); key(K_A, CMD); time.sleep(0.5); m = seq(); key(K_FDEL); wait_ed(m, 'd'); settle(m)
                nu = nr = 0
                for _ in range(10):  # each redo after the undo's check, as redo_shift_z
                    m = seq(); key(K_Z, CMD); c, d = wait_ed(m, 'c'); nu += bool(c and not d); settle(m)
                    m = seq(); key(K_Z, CMD | SHIFT); c, d = wait_ed(m, 'd'); nr += bool(d and not c); settle(m)
                m = seq(); key(K_Z, CMD); wait_ed(m, 'c'); settle(m)
                row(E, 'undo_10', 'works' if nu == 10 else ('nothing' if nu == 0 else 'wrong'), f'Cmd+Z undid {nu} of 10')
                row(E, 'redo_10', 'works' if nr == 10 else ('nothing' if nr == 0 else 'wrong'), f'Cmd+Shift+Z redid {nr} of 10')
                # undo_markers (fork d2ce4ba5c6): a check's markers in select all; Delete, undo, redo, undo; a check and an agent read between each
                kwins0 = {w[0] for w in wins() if w[1] == kpid}
                notes, bad = [], []
                def real(byk): return {k: v for k, v in byk.items() if 'Marker' not in k}
                rc = ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB' if E == 'pcb' else 'FK_SCH'}})
                ran_direct = 'message' in rc
                def between(tag):
                    n = seq()
                    if ran_direct: ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB' if E == 'pcb' else 'FK_SCH'}})
                    done = checks_idle(n, 30)
                    byk, ok, said = agent_items(E, f)
                    notes.append(f'{tag}: check {"done" if done else "no ET_CHECKS_DONE"}, markers {sum(v for k, v in byk.items() if "Marker" in k)}, agent read {"ok" if ok else "FAILED " + said}')
                    if not ok: bad.append(f'the agent read after {tag} failed')
                    return real(byk)
                c0 = between('start')
                canvas(); key(K_A, CMD); time.sleep(0.6)
                def mstep(tag, code, fl, back):
                    n = seq(); key(code, fl); c, d = wait_ed(n, 'c' if back else 'd'); settle(n)
                    now = between(tag)
                    if back and now != c0: bad.append(f'after {tag} not every real item is back: {now} (was {c0})')
                    if not back and sum(now.values()) >= sum(c0.values()): bad.append(f'after {tag} items remain: {now}')
                    if not (c or d): bad.append(f'{tag}: no edit')
                    notes.append(f'{tag}: created {c} deleted {d}, real items {sum(now.values())}')
                mstep('delete', K_FDEL, 0, False); mstep('undo', K_Z, CMD, True)
                mstep('redo', K_Z, CMD | SHIFT, False); mstep('undo again', K_Z, CMD, True)
                newk = [w for w in wins() if w[1] == kpid and w[0] not in kwins0]
                if newk: bad.append(f'KiCad showed {len(newk)} new window(s): {[w[3] for w in newk]}')
                for nl in notes: print(f'INFO undo_markers {E}: {nl}', flush=True)
                how = 'run through the hub (RunChecks)' if ran_direct else "KiCad's own after each edit (RunChecks refused through the hub: " + str(rc)[:100] + ')'  # Python 3.9: no backslash in an f-string
                print(f'INFO undo_markers {E}: checks {how}', flush=True)
                row(E, 'undo_markers', 'works' if not bad else ('nothing' if all(b.endswith('no edit') for b in bad) else 'wrong'), ('; '.join(bad) or f'every real item back at each undo ({sum(c0.values())}: {c0}), no dialog'))
                if E == 'pcb':  # select_markers_agent_check (fork e7f1bdb7a3, Carbon's crash): Cmd+A with the check's markers in it, an agent's
                    # check frees them, the pointer moves over the canvas: KiCad keeps running, and the selection is the same minus the markers
                    C = 'kiapi.common.commands.'
                    def selection():
                        d = ask('kicad_ipc', {'path': f, 'type': C + 'GetOpenDocuments', 'message': {'type': 'DOCTYPE_PCB'}}); doc = ((d.get('message') or {}).get('documents') or [None])[0]
                        it = ((ask('kicad_ipc', {'path': f, 'type': C + 'GetSelection', 'message': {'header': {'document': doc}}}).get('message') or {}).get('items') or [])
                        mk = [i for i in it if 'Marker' in str(i.get('@type', ''))]
                        return len(it) - len(mk), len(mk)
                    n0 = seq(); ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB'}}); checks_idle(n0, 30)
                    gm = ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.GetMarkers', 'message': {'kind': 'FK_PCB'}})
                    gmm = gm.get('message') or {}
                    nm = max([len(v) for v in gmm.values() if isinstance(v, list)] + [0])
                    kpid0 = kpid; canvas(); key(K_A, CMD); time.sleep(0.8)
                    real0, mk0 = selection()
                    n1 = seq(); ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB'}}); cd = checks_idle(n1, 30)
                    p = last_push()
                    for fx, fy in ((0.3, 0.3), (0.5, 0.5), (0.7, 0.6), (0.4, 0.7), (0.6, 0.4)):
                        relay(op='move', x=p[0][0] + p[0][2] * fx, y=p[0][1] + p[0][3] * fy); time.sleep(0.4)
                    time.sleep(1.5)
                    alive = bool(kicad_frame()) and any(OUT in l and str(kpid0) in l.split()[0:1] for l in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout.splitlines())
                    stall = open(os.path.join(OUT, 'kicad-embed-trace.log'), errors='replace').read().count('STALL') if os.path.isfile(os.path.join(OUT, 'kicad-embed-trace.log')) else 0
                    real1, mk1 = selection() if alive else (0, 0)
                    detail = (f'GetMarkers: {nm} markers ({str(gmm)[:80]}); after Cmd+A the selection held {real0} items and {mk0} markers; agent check '
                              f'{"done" if cd else "no ET_CHECKS_DONE"}; KiCad {"running" if alive else "GONE"}; STALL lines {stall}; after: {real1} items and {mk1} markers')
                    if not alive or stall: v = 'wrong'
                    elif not (nm or mk0): v = 'nothing (not judged: no markers on the board or in the selection)'
                    else: v = 'works' if real1 == real0 and real0 > 1 and mk1 == 0 else 'wrong'
                    row(E, 'select_markers_agent_check', v, detail)
                    key(K_ESC); time.sleep(0.6)
                if not undo_ok:
                    print(f'INFO {E}: undo is nothing, so the rows after it run on what is left (no restore): judge them with that in mind', flush=True)
                canvas(); key(K_A, CMD); time.sleep(0.5)
                n, t = press(K_DEL); v = judge_cd(n, 'd', True); row(E, 'backspace', *v, t); settle(n)
                if v[0] == 'works' and undo_ok: m = seq(); key(K_Z, CMD); wait_ed(m, 'c'); settle(m)
                canvas(); key(K_A, CMD); time.sleep(0.5)
                subprocess.run(['pbcopy'], input=b'lane-m-before-copy'); key(8, CMD); time.sleep(0.8)
                pb = subprocess.run(['pbpaste'], capture_output=True).stdout.decode(errors='replace')
                other_text = pb if pb != 'lane-m-before-copy' else ''
                pb_kicad = pb != 'lane-m-before-copy' and ('(footprint' in pb or '(symbol' in pb or '(kicad_' in pb or '(segment' in pb or 'lib_symbols' in pb)
                print(f'INFO {E} copy: after Cmd+C the pasteboard {"holds KiCad text" if pb_kicad else ("is unchanged" if pb == "lane-m-before-copy" else "changed but not to KiCad text")} ({len(pb)} chars: {pb[:60]!r})', flush=True)
                key(K_ESC); time.sleep(0.4)
                p = last_push(); relay(op='move', x=p[0][0] + p[0][2] * 0.5, y=p[0][1] + p[0][3] * 0.5); time.sleep(0.4)
                n, t = press(K_V, CMD); time.sleep(1.0); board_click(0.5, 0.5); v = judge_cd(n, 'c', True); row(E, 'copy_paste', *v, t); settle(n)
                if v[0] == 'works' and undo_ok: m = seq(); key(K_Z, CMD); wait_ed(m, 'd'); settle(m)
                # paste from another KiCad: its clipboard text (the editor's s-expression) on the macOS pasteboard, Cmd+V and a click here
                if not other_text: row(E, 'paste_from_other', 'nothing', 'not judged: Cmd+C put no KiCad text on the pasteboard to paste from'); other_text = None
                canvas()
                if other_text: subprocess.run(['pbcopy'], input=b'lane-m-elsewhere'); time.sleep(0.3); subprocess.run(['pbcopy'], input=other_text.encode())
                p = last_push(); relay(op='move', x=p[0][0] + p[0][2] * 0.5, y=p[0][1] + p[0][3] * 0.5); time.sleep(0.4)
                if other_text:
                    n, t = press(K_V, CMD); time.sleep(1.0); board_click(0.5, 0.5); v = judge_cd(n, 'c', True)
                    row(E, 'paste_from_other', *v, t); settle(n); key(K_ESC); time.sleep(0.4)
                    if v[0] == 'works' and undo_ok: m = seq(); key(K_Z, CMD); wait_ed(m, 'd'); settle(m)
                canvas(); key(K_A, CMD); time.sleep(0.5)
                n, t = press(K_X, CMD); v = judge_cd(n, 'd', True); row(E, 'cut', *v, t); settle(n)
                p = last_push(); relay(op='move', x=p[0][0] + p[0][2] * 0.5, y=p[0][1] + p[0][3] * 0.5); time.sleep(0.4)
                n, t = press(K_V, CMD); time.sleep(1.0); board_click(0.5, 0.5); v = judge_cd(n, 'c', True); row(E, 'cut_paste', *v, t); settle(n)
                key(K_ESC); time.sleep(0.3)
                m0, b0 = os.path.getmtime(f), open(f, 'rb').read()
                n, t = press(K_S, CMD); saved = wait_for(lambda: os.path.getmtime(f) > m0, 6)
                row(E, 'save', 'works' if saved and open(f, 'rb').read() != b0 else ('wrong' if saved else 'nothing'), 'the file changed' if saved else 'the file unchanged', t)
                if E == 'pcb':  # typing in a text field across a check: the keys stay in the field, none go to the canvas
                    def field_text():
                        subprocess.run(['pbcopy'], input=b'lane-m-none'); key(K_A, CMD); time.sleep(0.3); key(8, CMD); time.sleep(0.5)
                        return subprocess.run(['pbpaste'], capture_output=True).stdout.decode(errors='replace')
                    def across(open_field, where):
                        canvas(); open_field(); time.sleep(1.2)
                        n0 = seq(); key(0); time.sleep(0.12); key(11); time.sleep(0.5)  # a, b as key presses (the runner's unicode events did not reach wx)
                        if field_text() != 'ab': key(K_ESC); time.sleep(0.6); return None
                        key(124); time.sleep(0.3)  # the caret to the end
                        n1 = seq(); rc = ask('kicad_ipc', {'path': f, 'type': 'kiapi.backplane.embed.RunChecks', 'message': {'kind': 'FK_PCB'}}); cd = checks_idle(n1, 30)
                        key(8); time.sleep(0.12); key(2); time.sleep(0.5); got = field_text()  # c, d
                        # the board's canvas only: a schematic frame's empty selection change (its own checks freeing their markers, e2e-macos
                        # 44251169563) is not a key that left the field
                        canvas_ev = [e.get('type') for e in evs() if int(e.get('seq', 0)) > n0 and e.get('type') in ('ET_DOCUMENT_EDITED', 'ET_SELECTION_CHANGED') and (e.get('frame') or {}).get('kind') == 'FK_PCB']
                        key(K_ESC); time.sleep(0.6)
                        return (got == 'abcd' and not canvas_ev and bool(cd)), f'{where}: the field holds {got!r} after "ab", a check ({"done" if cd else "no ET_CHECKS_DONE"}; RunChecks {"ok" if "message" in rc else str(rc)[:60]}), "cd"; canvas events {canvas_ev}'
                    # the Find dialog's field (Cmd+F): not the Search panel by Cmd+G, which is also Group in the PCB editor (fork embed: Cmd+G,
                    # then typing, hung KiCad on the base build), so no typed key can reach the canvas if a field did not open
                    fd0 = seq(); r = None
                    def open_find(): key(K_F, CMD)
                    canvas(); open_find(); opened = wait_for(lambda: since(fd0, 'ET_DIALOG_OPENED'), W); key(K_ESC); time.sleep(0.8)
                    if opened: r = across(open_find, 'the Find dialog (Cmd+F)')
                    row(E, 'type_across_check', ('works' if r[0] else 'wrong') if r else 'nothing',
                        r[1] if r else ('Find opened but its field did not take "ab" (not judged as KiCad: a harness path)' if opened else 'Find did not open'))
                canvas(); n, t = press(K_F, CMD); wait_for(lambda: since(n, 'ET_DIALOG_OPENED'), W)
                dl = [e.get('detail') for e in since(n, 'ET_DIALOG_OPENED')]
                row(E, 'find', 'works' if 'Find' in dl else ('wrong' if dl else 'nothing'), f'dialogs {dl}', t); key(K_ESC); time.sleep(1.0)
                # the other menu chords: what they open or do (not gating; the trace says whether wx took them)
                for name, code, fl, close in (('extra_print', 35, CMD, True), ('extra_preferences', 43, CMD, True), ('extra_open', 31, CMD, True),
                                              ('extra_search_panel', 5, CMD, False), ('extra_zoom_in', 24, CMD | SHIFT, False), ('extra_zoom_in_plain', 24, CMD, False), ('extra_zoom_out', 27, CMD, False)):
                    canvas(); n, t = press(code, fl); time.sleep(2.0)
                    dl = [e.get('detail') for e in since(n, 'ET_DIALOG_OPENED')]; c, d = ed(n)
                    other = sorted({e.get('type') for e in evs() if int(e.get('seq', 0)) > n} - {'ET_PROGRESS', 'ET_FRAME_DRAWN', 'ET_CHECKS_DONE', 'ET_FOCUS_IN', 'ET_FOCUS_OUT'})
                    alive = bool(kicad_frame())
                    row(E, name, 'opened ' + ', '.join(map(str, dl)) if dl else ('edit' if c or d else 'no dialog, no edit'), f'events {other}; KiCad frame {"there" if alive else "GONE"}', t)
                    if dl and close:
                        m = seq(); key(K_ESC); closed = wait_for(lambda: since(m, 'ET_DIALOG_CLOSED'), W)
                        print(f'INFO ROW {E} {name}: Escape {"closes it" if closed else "does NOT close it"}', flush=True)
                        if not closed: key(K_ESC); time.sleep(1.0)
                    if not alive: print(f'INFO {E}: KiCad frame gone after {name}; the table stops here', flush=True); return False
                if True:  # last, in both editors: Cmd+W (KiCad's Close: the pane goes back to View, Edit brings a fresh frame); Cmd+Q (schematic only)
                    canvas(); m0 = seq(); key(1, CMD); wait_for(lambda: since(m0, 'ET_MODIFIED_CHANGED') or True, 3); time.sleep(1.0)  # saved first: no save prompt
                    canvas(); n, t = press(13, CMD)
                    gone = wait_for(lambda: since(n, 'ET_FRAME_CLOSED') or not kicad_frame(), 10); offered = wait_for(lambda: rects().get('ui:Edit'), 10)
                    print(f'INFO ROW {E} extra_close_window: ET_FRAME_CLOSED {"seen" if since(n, "ET_FRAME_CLOSED") else "not seen"}', flush=True)
                    back = None
                    if offered:
                        r = rects().get('ui:Edit'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
                        click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2); back = wait_for(kicad_frame, 60, 0.5)
                    dl = [e.get('detail') for e in since(n, 'ET_DIALOG_OPENED')]
                    row(E, 'extra_close_window', 'works' if gone and offered and back else 'wrong', f'frame gone {bool(gone)}; the pane offers Edit {bool(offered)}; Edit brings it back {bool(back)}; dialogs {dl}', t)
                    if back and E == 'sch':
                        time.sleep(4); board_click(0.1, 0.9); kp = {w[1] for w in wins() if 'kicad' in w[2].lower()}
                        n, t = press(12, CMD); time.sleep(3)
                        dl = [e.get('detail') for e in since(n, 'ET_DIALOG_OPENED')]; alive = bool(kicad_frame())
                        v = 'nothing (KiCad stays)' if alive and not dl else (f'the save gate ({dl})' if alive else 'KiCad QUIT')
                        row(E, 'extra_quit', v, f'frame there {alive}; dialogs {dl}', t)
                        if dl: key(K_ESC); time.sleep(1.0)
                    if back and E == 'pcb': time.sleep(4); canvas()
                    if not back: return False
                return True
            # ⌫ while routing, inside the board: pictures of KiCad's frame only, right before and after ⌫
            def kshot(tag):
                k = kicad_frame()
                if k: relay(op='png', rect=list(k[3]), path=os.path.join(OUT, tag + '.png'))
            key(K_ESC); time.sleep(0.3); board_click(0.1, 0.9); key(K_ESC); time.sleep(0.3); key(K_X); time.sleep(0.8)
            q = [(0.30, 0.42), (0.55, 0.42), (0.55, 0.62)]
            for fx, fy in q: board_click(fx, fy); time.sleep(0.3)
            pp = last_push(); relay(op='move', x=pp[0][0] + pp[0][2] * 0.62, y=pp[0][1] + pp[0][3] * 0.66); time.sleep(0.8)
            t0 = len(tr()); kshot('bs-route-before'); key(K_DEL); time.sleep(1.0); kshot('bs-route-after')
            print(f'INFO bs 3: ⌫ while routing, pictures bs-route-before.png / bs-route-after.png of KiCad\'s frame; trace {[l.split("macos: ",1)[-1] for l in tr()[t0:]]}', flush=True)
            key(K_ESC); time.sleep(0.4); key(K_ESC); time.sleep(0.6)
            first_keys('pcb')
            board_click(0.1, 0.9)
            go = table('pcb', pcbf)
            if go:  # per_actor_undo (PCB): the person deletes everything, an agent's track lands (save: false), the person's Cmd+Z
                canvas(); key(K_A, CMD); time.sleep(0.5); n = seq(); key(K_FDEL); wait_ed(n, 'd'); settle(n)
                C = 'kiapi.common.commands.'
                docs = ask('kicad_ipc', {'path': pcbf, 'type': C + 'GetOpenDocuments', 'message': {'type': 'DOCTYPE_PCB'}})
                h = {'document': ((docs.get('message') or {}).get('documents') or [None])[0]}
                b = ask('kicad_ipc', {'path': pcbf, 'type': C + 'BeginCommit', 'save': False, 'message': {'header': h}}); tid = (b.get('message') or {}).get('id')
                ask('kicad_ipc', {'path': pcbf, 'type': C + 'CreateItems', 'save': False, 'message': {'header': h, 'items': [{'@type': 'type.googleapis.com/kiapi.board.types.Track', 'id': {'value': str(__import__('uuid').uuid4())},
                    'start': {'x_nm': 120000000, 'y_nm': 100000000}, 'end': {'x_nm': 140000000, 'y_nm': 100000000}, 'width': {'value_nm': 300000}, 'layer': 'BL_F_Cu'}]}})
                ask('kicad_ipc', {'path': pcbf, 'type': C + 'EndCommit', 'save': False, 'message': {'header': h, 'id': tid, 'action': 'CMA_COMMIT', 'message': 'agent add track'}})
                time.sleep(1.5); canvas()
                n, t = press(K_Z, CMD); c, d = wait_ed(n, 'c'); settle(n)
                m0 = os.path.getmtime(pcbf); key(K_S, CMD); wait_for(lambda: os.path.getmtime(pcbf) > m0, 6)
                txt = open(pcbf, errors='replace').read(); has = '(start 120 100)' in txt and '(end 140 100)' in txt
                v = 'works' if c > 0 and d == 0 and has else ('wrong' if d > 0 or not has else 'nothing')
                row('pcb', 'per_actor_undo', v, f'created {c}, deleted {d}; the saved file {"has" if has else "lacks"} the agent\'s track; agent commit {"ok" if tid else "no id"}', t)
            if go:
                r = rects().get('kicad-tab:SCH'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
                if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
                time.sleep(6); first_keys('sch'); board_click(0.1, 0.9); table('sch', schf)  # Cmd+W and Cmd+Q last, in the schematic: they may end the editor
            RES['edit_keys'] = ROWS
            raise SystemExit(1 if FAILS else 0)
        if os.environ.get('BS_PROBE') == 'order':  # the ⌫ cases ordered so none needs Cmd+Z to restore the board (undo is lost on macOS)
            K_DEL_ = K_DEL
            # 3: ⌫ while routing takes back the last segment: judged in cropped pictures of KiCad's own frame
            key(K_ESC); time.sleep(0.4); board_click(0.5, 0.5); key(K_ESC); time.sleep(0.4)
            key(K_X); time.sleep(0.8); board_click(0.5, 0.35); board_click(0.65, 0.35); shot('bs-route-before'); key(K_DEL_); time.sleep(1.0); shot('bs-route-after')
            key(K_ESC); time.sleep(0.4); key(K_ESC); time.sleep(0.6)
            print('INFO bs 3: routing before/after ⌫ in bs-route-before.png and bs-route-after.png (judged by eye)', flush=True)
            # 4: ⌫ in Find's field: "abc" then ⌫
            board_click(0.1, 0.9); key(K_F, CMD); time.sleep(1.5)
            relay(op='text', text='abc'); time.sleep(0.4); key(K_DEL_); time.sleep(0.6); shot('bs-text-field'); key(K_ESC); time.sleep(1.0)
            print('INFO bs 4: Find\'s field after "abc" then ⌫ in bs-text-field.png (judged by eye: "ab")', flush=True)
            # 2: the pointer on a point where a click selects exactly one footprint, the selection cleared, then ⌫
            spot = None
            for fx, fy in ((0.5, 0.5), (0.35, 0.5), (0.65, 0.5), (0.5, 0.35), (0.5, 0.65), (0.35, 0.35), (0.65, 0.65), (0.35, 0.65), (0.65, 0.35), (0.25, 0.5), (0.75, 0.5), (0.2, 0.3), (0.8, 0.7)):
                key(K_ESC); time.sleep(0.4); board_click(fx, fy); time.sleep(0.4)
                if len(sel_refs()) == 1: spot = (fx, fy, sel_refs()[0]); break
            key(K_ESC); time.sleep(0.6)
            if spot:
                print(f'INFO bs 2: the pointer on {spot[2]} at ({spot[0]}, {spot[1]}) of the canvas, nothing selected', flush=True)
                n = seq(); key(K_DEL_)
                check(bool(wait_for(lambda: any(d for c, d in edits_since(n)), 4)), 'bs 2: ⌫ with nothing selected deletes the item under the pointer')
            else:
                check(False, 'bs 2: no point of the canvas selected exactly one footprint (the case could not be set up)')
            # the schematic: Cmd+A then ⌫
            r = rects().get('kicad-tab:SCH'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
            if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
            time.sleep(6); board_click(0.1, 0.9)
            n = seq(); key(K_A, CMD); time.sleep(0.6); key(K_DEL_)
            check(bool(wait_for(lambda: any(d for c, d in edits_since(n)), 4)), 'bs 1: Cmd+A then ⌫ deletes the selection in the embedded schematic editor')
            print(f'INFO embed trace "Backspace handed to wx": {open(os.path.join(OUT, "kicad-embed-trace.log"), errors="replace").read().count("Backspace handed to wx")} times', flush=True)
            raise SystemExit(1 if FAILS else 0)
        trace = lambda:open(os.path.join(OUT, 'kicad-embed-trace.log'), errors='replace').read().count('Backspace handed to wx') if os.path.isfile(os.path.join(OUT, 'kicad-embed-trace.log')) else 0
        # the menu chords (Edit > Undo, Redo, File > Save are menu key equivalents too, like Delete's ⌫): judged by KiCad's own edit events
        # (created = undo of a delete, deleted = redo), never by modified-flag events, which trail the edit itself
        board_click(0.1, 0.9)
        n0 = seq(); key(K_A, CMD); time.sleep(0.6); key(K_DEL)
        d_ok = bool(wait_for(lambda: any(d for c, d in edits_since(n0)), 4))
        check(d_ok, 'bs 1: Cmd+A then ⌫ deletes the selection in the embedded board editor')
        # Cmd+Z at once after the delete (while KiCad's checks may still run), as a person would
        n1 = seq(); key(K_Z, CMD)
        u_fast = bool(wait_for(lambda: any(c for c, d in edits_since(n1)), 4))
        check(u_fast, 'keys: Cmd+Z right after a delete undoes it (KiCad\'s checks still running)')
        if not u_fast:
            checks_idle(n0); n1 = seq(); key(K_Z, CMD)
            u_late = bool(wait_for(lambda: any(c for c, d in edits_since(n1)), 4))
            print(f'INFO keys: Cmd+Z once the checks are done: {"undoes" if u_late else "does nothing either"}', flush=True)
        checks_idle(n1)
        n2 = seq(); key(K_Z, CMD | SHIFT)
        r_ok = bool(wait_for(lambda: any(d for c, d in edits_since(n2)), 4))
        check(r_ok, 'keys: Cmd+Shift+Z redoes in the embedded board editor (the delete again)')
        if r_ok:
            checks_idle(n2); n3 = seq(); key(K_Z, CMD); wait_for(lambda: any(c for c, d in edits_since(n3)), 4); checks_idle(n3)
        RES['bs_board_items_back'] = not edits_since(n0) or sum(c for c, d in edits_since(n0)) >= sum(d for c, d in edits_since(n0))
        pcbf = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb'); m0 = os.path.getmtime(pcbf)
        key(K_ESC); time.sleep(0.3); board_click(0.1, 0.9); time.sleep(0.4); key(1, CMD)  # Cmd+S (keycode 1 = s)
        saved = wait_for(lambda: os.path.getmtime(pcbf) > m0, 6)
        check(bool(saved), 'keys: Cmd+S saves the board in the embedded editor (the file on disk changes)')
        # 2: find a point where a click selects exactly one footprint, clear the selection there, then ⌫ with the pointer still on it
        spot = None
        for fx, fy in ((0.5, 0.5), (0.35, 0.5), (0.65, 0.5), (0.5, 0.35), (0.5, 0.65), (0.35, 0.35), (0.65, 0.65), (0.35, 0.65), (0.65, 0.35), (0.25, 0.5), (0.75, 0.5)):
            key(K_ESC); time.sleep(0.4); board_click(fx, fy); time.sleep(0.4)
            if len(sel_refs()) == 1: spot = (fx, fy, sel_refs()[0]); break
        key(K_ESC); time.sleep(0.6)
        if spot:
            print(f'INFO bs 2: the pointer on {spot[2]} at ({spot[0]}, {spot[1]}) of the canvas, nothing selected', flush=True)
            check(deleted(lambda: key(K_DEL), 'pcb-hover'), 'bs 2: ⌫ with nothing selected deletes the item under the pointer')
        else:
            check(False, 'bs 2: no point of the canvas selected exactly one footprint (the case could not be set up)')
        board_click(0.1, 0.9)
        check(deleted(lambda: (key(K_A, CMD), time.sleep(0.6), key(K_FDEL)), 'pcb-fdel'), 'bs 5: fn+delete still deletes')
        # 3: routing from a pad (the board's middle has U1's pads), two clicks, then ⌫ takes back the last segment
        key(K_ESC); time.sleep(0.4); board_click(0.5, 0.5); key(K_ESC); time.sleep(0.4)
        key(K_X); time.sleep(0.8); board_click(0.5, 0.35); board_click(0.65, 0.35); shot('bs-route-before'); key(K_DEL); time.sleep(1.0); shot('bs-route-after')
        key(K_ESC); time.sleep(0.4); key(K_ESC); time.sleep(0.6)
        print('INFO bs 3: routing before/after ⌫ in bs-route-before.png and bs-route-after.png (judged by eye)', flush=True)
        # 4: ⌫ in a text field: Find's field takes "abc", ⌫ leaves "ab"
        board_click(0.1, 0.9); key(K_F, CMD); time.sleep(1.5)
        relay(op='text', text='abc'); time.sleep(0.4); key(K_DEL); time.sleep(0.6); shot('bs-text-field'); key(K_ESC); time.sleep(1.0)
        print('INFO bs 4: Find\'s field after "abc" then ⌫ in bs-text-field.png (judged by eye: "ab")', flush=True)
        # the schematic
        r = rects().get('kicad-tab:SCH'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
        if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
        time.sleep(6); board_click(0.1, 0.9)
        check(deleted(lambda: (key(K_A, CMD), time.sleep(0.6), key(K_DEL)), 'sch-all'), 'bs 1: Cmd+A then ⌫ deletes the selection in the embedded schematic editor')
        RES['bs_trace_handed'] = trace(); print(f'INFO embed trace "Backspace handed to wx": {RES["bs_trace_handed"]} times', flush=True)
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=narrow: side panels give way in a narrow pane (embed-autopanels): the canvas at least 60 % of the pane at 611, 480
    # and 1000 px, the board and the schematic; at Full size the panels are back (the frame at least 150 px wider than the canvas).
    # The canvas is found in the frame's own picture by lib/canvas_box.py (Climb B's measure); a box that is the whole frame is no measure.
    if os.environ.get('MAC_MODE') == 'narrow':
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/lib'))
        import numpy as np
        from canvas_box import canvas_box
        def frame_canvas(tag):
            k = kicad_frame()
            if not k:  # not directly above Backplane's window: say what is between (owner names only), and measure KiCad's frame by its pid
                order = wins(); ours = next((i for i, w in enumerate(order) if w[0] == cw[0]), len(order))
                between = [w[2] for w in order[:ours] if w[1] != kpid]
                k = next((w for w in order if w[1] == kpid and w[3][2] > 200), None)
                print(f'INFO narrow {tag}: KiCad\'s frame is not directly above Backplane\'s window; above it: {between}; measured by its pid ({bool(k)})', flush=True)
                print(f'INFO narrow {tag}: windows (number, pid, bounds), front first: {[(w[0], "client" if w[1] == client.pid else "kicad" if w[1] == kpid else "other", w[3]) for w in order if w[1] in (client.pid, kpid)]}; Backplane\'s was {cw[0]}; the last push {last_push()}', flush=True)
                if not k: return None
            path = os.path.join(OUT, f'crop-{tag}.rgb')
            relay(op='crop', x=k[3][0], y=k[3][1], w=k[3][2], h=k[3][3], path=path)
            raw = open(path, 'rb').read(); nl = raw.index(b'\n'); w, h = map(int, raw[:nl].split())
            a = np.frombuffer(raw[nl + 1:], dtype=np.uint8).reshape(h, w, 3)
            x0, y0, x1, y1 = canvas_box(a)
            whole = (x0, y0) == (0, 0) and (x1, y1) == (w - 1, h - 1)
            shot(f'narrow-{tag}')
            return {'frame_px': w, 'canvas_px': x1 - x0, 'share': round((x1 - x0) / w, 3), 'whole_frame': whole}
        folded = [False]
        def set_pane(target):
            # the pane does not grow one for one with the window (the thread's column takes a share): steps by the slope seen so far
            slope, prev = 1.0, None
            for _ in range(8):
                p = last_push(); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
                if not p: break
                if abs(p[0][2] - target) <= 3: break
                if prev and b[2] != prev[0] and p[0][2] != prev[1]: slope = max(0.2, min(2.0, (p[0][2] - prev[1]) / (b[2] - prev[0])))
                prev = (b[2], p[0][2])
                ww = max(500, min(1900, round(b[2] + (target - p[0][2]) / slope))); mark(f'window width {ww}'); win_size(client.pid, ww, b[3]); time.sleep(2.0)
            p = last_push()
            if p and p[0][2] < target - 3 and not folded[0]:
                # wider than the window allows with the sidebar open (a 1000 pt pane on a 1920 pt display): fold it (Cmd+B in Backplane)
                b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
                mark('fold: click in the sidebar'); click(b[0] + 20, b[1] + b[3] - 40); time.sleep(0.6); mark('fold: Cmd+B'); key(K_B, CMD); time.sleep(2.0); folded[0] = True
                return set_pane(target)
            time.sleep(3.0)  # the panels follow the width (EmbedAutoPanels)
            return last_push()[0][2] if last_push() else None
        TR0 = [time.time()]
        if os.environ.get('ZTRACE'):
            helper(f'trace {client.pid} {kpid} {os.path.join(OUT, "ztrace.txt")} 400'); TR0[0] = time.time()
        mark = lambda what: print(f'INFO ztrace {round((time.time() - TR0[0]) * 1000)} ms: {what}', flush=True)
        for tab in ('PCB', 'SCH'):
            if tab == 'SCH':
                r = rects().get('kicad-tab:SCH'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
                if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
                wait_for(lambda: (k := kicad_frame()) and k, 60, 0.5); time.sleep(5)
            for target in (611, 480, 1000):
                mark(f'{tab} set_pane {target}'); got = set_pane(target); mark(f'{tab} measure {target} (pane {got})'); m = frame_canvas(f'{tab}-{target}'); RES[f'narrow_{tab}_{target}'] = {'pane': got, **(m or {})}
                # 1000: on a 1920 pt display the widest pane beside Backplane's chrome is about 967 (the sidebar folded): that is the row there
                widest = target >= 1000 and got and got >= 950
                check(bool(m) and not m['whole_frame'] and m['share'] >= 0.6 and got and (abs(got - target) <= 6 or widest),
                      f'narrow: {tab} in a {target} px pane ({got}{", the widest this display allows" if widest and abs(got - target) > 6 else ""}): the canvas has at least 60 % of it ({m})')
            # Full size: the pane takes the room, and the panels come back
            r = rects().get('kicad:full'); b = next((w for w in wins() if w[0] == cw[0]), cw)[3]
            if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
            time.sleep(4); m = frame_canvas(f'{tab}-full'); RES[f'narrow_{tab}_full'] = m
            check(bool(m) and not m['whole_frame'] and (m['frame_px'] - m['canvas_px']) / backing_scale() >= 150, f'narrow: {tab} at Full size: the side panels are back, the frame 150 pt or more wider than the canvas ({m})')
            r = rects().get('kicad:full')
            if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
            time.sleep(2)
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=front: KiCad's frame is the front-most window over the pane's rect (the gate's blank pane, Nov 16: at a 967 pt pane
    # in a 1900 pt window the frame sat at the pane's rect but behind Backplane's window). Pane widths 611, 967 and the widest the
    # display allows, the sidebar folded and open, the board and the schematic; after Edit (View, then Edit), after a click in
    # Backplane's chrome and one back in the pane, after two presses on Backplane's title bar and none back, after a tab switch and back: FRONT_REPS (10) of each, every one judged by the
    # window server's list (the helper's, front first) three times over 0.6 s: the first normal window that touches the pane's rect
    # is KiCad's, at the pane's rect within 2 pt. A width the display cannot give with the sidebar open is said, not failed.
    if os.environ.get('MAC_MODE') == 'front':
        REPS = int(os.environ.get('FRONT_REPS', '10'))
        # the window never past the display's right edge
        if os.environ.get('FRONT_SCREEN_W'): SCREEN_W = int(os.environ['FRONT_SCREEN_W'])
        elif DRIVER == 'quartz': SCREEN_W = int(Quartz.CGDisplayPixelsWide(Quartz.CGMainDisplayID()))
        else: SCREEN_W = 1920
        WIDEST = min(1900, SCREEN_W - 20)
        print(f'INFO front: display {SCREEN_W} pt wide, widest window {WIDEST}', flush=True)
        def alive():
            if client.poll() is not None: raise SystemExit(f'STOPPED front: the client has exited ({client.returncode}); nothing more is clicked or typed')
        def ours():
            # never the window's last known place: a window that is not in the list now is not clicked, sized or typed at (Nov 16: the
            # lookup was lost and the run's clicks went to the person's own windows)
            alive()
            w = next((w for w in wins() if w[0] == cw[0] and w[1] == client.pid), None)
            if not w: raise SystemExit(f"STOPPED front: Backplane's window {cw[0]} is not on screen; nothing more is clicked or typed")
            return w[3]
        def keys_ours(what):
            # a chord goes to whichever app has the keyboard: only when that is this run's Backplane or KiCad
            if not (active(client.pid) or active(kpid)): raise SystemExit(f'STOPPED front: {what}: neither Backplane nor KiCad of this run is the active app')
        def touches(a, b): return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]
        def front_once():
            """None when KiCad's frame is the front-most normal window over the pane's rect and at it; else what is wrong, in words."""
            p = last_push()
            if not p or not p[1]: return f'the client has not pushed a visible pane ({p})'
            top = next((w for w in wins() if touches(w[3], p[0])), None)
            if not top: return 'no window over the pane'
            who = 'Backplane' if top[1] == client.pid else 'KiCad' if top[1] == kpid else 'another app'
            if top[1] != kpid: return f"{who}'s window {top[0]} {top[3]} is in front over the pane {p[0]}"
            if max(abs(a - b) for a, b in zip(top[3], p[0])) > 2: return f"KiCad's window {top[3]} is not at the pane {p[0]}"
            return None
        def front():
            """Judged three times over 0.6 s after the step has settled (an order that holds for one look only is the bug)."""
            wait_for(lambda: front_once() is None, 3, 0.1)
            bad = []
            for _ in range(3):
                b = front_once()
                if b: bad.append(b)
                time.sleep(0.3)
            return bad
        def click_rect(i):
            r = rects().get(i); b = ours()
            if r: click(b[0] + r['x'] + r['w'] / 2, b[1] + 28 + r['y'] + r['h'] / 2)
            return bool(r)
        def rects_now():
            """The controls of the client's last drawn frame only (rects() keeps every id ever drawn: a control that is gone stays there)."""
            for l in reversed(client_log().splitlines()):
                if 'GPUI_METRIC ' in l and '"rects"' in l:
                    try: return {e['id']: e for e in json.loads(l.split('GPUI_METRIC ', 1)[1]).get('rects', []) or [] if isinstance(e, dict) and 'id' in e}
                    except Exception: pass
            return {}
        def is_folded(): return 'ui:Unfold sidebar' in rects_now()
        def fold(want):
            """The sidebar folded or open, by its own control and judged by what is drawn (a narrow window starts with it folded: run
            44006494316 pressed Cmd+B blind there and opened it, so its "folded" rows were the open ones and the other way round)."""
            if is_folded() == want: return True
            click_rect('ui:Fold sidebar' if want else 'ui:Unfold sidebar'); time.sleep(2.0)
            return is_folded() == want
        def pane_to(target):
            """The pane to `target` pt wide by the window's width (None: the widest window the display gives); the width reached."""
            if target is None:
                b = ours(); win_size(client.pid, min(WIDEST, SCREEN_W - b[0]), b[3]); time.sleep(2.5); ours()
                return last_push()[0][2] if last_push() else None
            slope, prev = 1.0, None
            for _ in range(8):
                p = last_push(); b = ours()
                if not p or abs(p[0][2] - target) <= 3: break
                if prev and b[2] != prev[0] and p[0][2] != prev[1]: slope = max(0.2, min(2.0, (p[0][2] - prev[1]) / (b[2] - prev[0])))
                prev = (b[2], p[0][2])
                win_size(client.pid, max(500, min(WIDEST, SCREEN_W - b[0], round(b[2] + (target - p[0][2]) / slope))), b[3]); time.sleep(2.0); ours()
            time.sleep(1.0)
            return last_push()[0][2] if last_push() else None
        def tab_now(): return 'SCH' if '"to":"SCH"' in ([l for l in client_log().splitlines() if '"kicad_tab"' in l] or [''])[-1] else 'PCB'
        def to_tab(t):
            click_rect(f'kicad-tab:{t}'); time.sleep(1.5)
        STEPS = {}
        def after_edit():
            if not click_rect('ui:View'): return 'no View control'
            wait_for(lambda: rects().get('ui:Edit') and not kicad_frame(), 10, 0.25); time.sleep(0.5)
            click_rect('ui:Edit'); wait_for(kicad_frame, 20, 0.25)
        def after_chrome():
            b = ours(); click(b[0] + b[2] / 2, b[1] + 14); time.sleep(0.7)   # Backplane's title bar: its window comes to the front
            p = last_push()[0]; click(p[0] + p[2] / 2, p[1] + p[3] / 2); time.sleep(0.3)  # and back into the editor
        def after_tab(tab):
            other = 'SCH' if tab == 'PCB' else 'PCB'
            to_tab(other); to_tab(tab)
        def after_press():
            # the gate's blank pane: a press on our window that draws nothing of ours (its title bar, while Backplane already has the
            # keyboard) brings it in front of the frame; nothing is clicked after it: the client must put the frame back by itself
            b = ours(); click(b[0] + b[2] / 2, b[1] + 14); time.sleep(0.9); click(b[0] + b[2] / 2 + 6, b[1] + 14); time.sleep(0.5)
        if os.environ.get('ZTRACE') and DRIVER == 'vnc': helper(f'trace {client.pid} {kpid} {os.path.join(OUT, "ztrace.txt")} 5400')
        elif os.environ.get('ZTRACE'):
            # the runner has no gate helper: the same 60 Hz trace from here (ms, then the normal windows front to back down to the later
            # of ours and KiCad's: number:a|b|o:bounds, a = the client, b = KiCad, o = anyone else), a line on each change
            import threading
            def ztrace():
                t0, last = time.time(), None
                with open(os.path.join(OUT, 'ztrace.txt'), 'w') as f:
                    while client.poll() is None:
                        row, seen = [], set()
                        for w in wins():
                            tag = 'a' if w[1] == client.pid else 'b' if w[1] == kpid else 'o'
                            row.append(f'{w[0]}:{tag}:{",".join(map(str, w[3]))}'); seen.add(tag)
                            if {'a', 'b'} <= seen: break
                        line = ' '.join(row)
                        if line != last: f.write(f'{round((time.time() - t0) * 1000)} {line}\n'); f.flush(); last = line
                        time.sleep(1 / 60)
            threading.Thread(target=ztrace, daemon=True).start()
        n_log = lambda what: client_log().count(what)
        JUDGED = set()
        for fold_on in (False, True):
            for target in (611, 967, None):
                if not fold(fold_on): print(f'INFO front: the sidebar could not be {"folded" if fold_on else "opened"} by its control before {target or "widest"} (drawn: {"folded" if is_folded() else "open"}): not judged', flush=True); continue
                got = pane_to(target)
                # the window's width can fold or open the sidebar by itself: a row is only ever named by what is drawn
                if is_folded() != fold_on: print(f'INFO front: at {target or "widest"} the sidebar does not stay {"folded" if fold_on else "open"} on this display: not judged', flush=True); continue
                JUDGED.add(fold_on)
                name = f'{"folded" if fold_on else "open"}-{target or "widest"}'
                if target and (not got or abs(got - target) > 6):
                    print(f'INFO front {name}: the display gives a {got} pt pane with the sidebar {"folded" if fold_on else "open"}, not {target}: judged at {got}', flush=True)
                for tab in ('PCB', 'SCH'):
                    to_tab(tab); wait_for(kicad_frame, 30, 0.5); time.sleep(2)
                    for step, act in (('edit', after_edit), ('chrome', after_chrome), ('press', after_press), ('tab', lambda: after_tab(tab))):
                        good, why = 0, []
                        for i in range(REPS):
                            skip = act()
                            if skip: why.append(skip); break
                            bad = front()
                            if bad: why.append(f'{i + 1}: {bad[-1]}')
                            # another app's window over the pane: this run's windows are no longer the front ones; it stops here (no
                            # picture: the screen now shows windows that are not the run's)
                            if any('another app' in b for b in bad):
                                check(False, f'front: {tab}, sidebar {"folded" if fold_on else "open"}, pane {got} pt, after {step}, try {i + 1}: {bad[-1]}')
                                raise SystemExit(f'STOPPED front: another app\'s window is over the pane; nothing more is clicked or typed')
                            else: good += 1
                        RES[f'front_{name}_{tab}_{step}'] = {'pane': got, 'good': good, 'of': REPS, 'why': why[:3]}
                        check(good == REPS, f'front: {tab}, sidebar {"folded" if fold_on else "open"}, pane {got} pt, after {step}: KiCad\'s frame is the front-most window over the pane {good} of {REPS}{" (" + "; ".join(why[:3]) + ")" if why else ""}')
        check(JUDGED == {False, True}, f'front: judged with the sidebar open and folded (drawn states judged: {sorted("folded" if j else "open" for j in JUDGED)})')
        # what the client did about it (its own log): the orders it had to repeat, and whether the pane ever said the editor is behind
        RES['front_client'] = {'orders': n_log('"kicad_mac_order"'), 'verified': n_log('"kicad_mac_order_verified"'), 'said_behind': n_log('"kicad_mac_buried"')}
        print(f'INFO front: the client re-ordered {RES["front_client"]["orders"]} times, proved {RES["front_client"]["verified"]}, said "behind" {RES["front_client"]["said_behind"]} times', flush=True)
        check(RES['front_client']['said_behind'] == 0, 'front: the pane never had to say the editor is behind Backplane\'s window')
        raise SystemExit(1 if FAILS else 0)
    # ---- MAC_MODE=libs: the libraries row L1-L7 on this runtime and this Mac (L8 is mac/libtables_l8.py) ----
    if os.environ.get('MAC_MODE') == 'libs':
        sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/lib')); sys.path.insert(0, os.path.join(REPO, 'test/kicad_embed/mac'))
        import re as _re
        import libgate_design as LG
        import hub_ws
        token = open(os.path.join(OUT, 'home/token')).read().strip()
        cli = os.path.join(RT, 'bin/kicad-cli')
        SS = os.path.join(RT, 'KiCad.app/Contents/SharedSupport')
        proj = os.path.join(OUT, 'proj')
        gsch, gpcb = os.path.join(proj, 'ecc83-pp.kicad_sch'), os.path.join(proj, 'ecc83-pp.kicad_pcb')
        # L1-L5, L7, L9: lane J's libraries_row.py (one definition for the three platforms; its macOS hooks), on this pane's project
        cfgp = os.path.join(OUT, 'libraries_row.json')
        json.dump({'out': OUT, 'port': port, 'token_file': os.path.join(OUT, 'home/token'), 'hub_rpc': ['/usr/bin/python3', os.path.join(REPO, 'test/kicad_embed/mac/hub_rpc.py')],
                   'standin_log': os.path.join(OUT, 'standin.log'), 'thread': 't1', 'runtime_root': RT, 'kicad_cli': cli, 'managed_parent': os.path.join(OUT, 'home', 'kicad'),
                   'config_root': os.path.join(OUT, 'fh/Library/Preferences/kicad'), 'settings_version': '10.99', 'project_dir': proj,
                   'pcb': gpcb, 'sch': gsch, 'platform': 'macos'}, open(cfgp, 'w'))
        r = subprocess.run([sys.executable, os.path.join(REPO, 'test/kicad_embed/libraries_row.py'), 'a', cfgp], capture_output=True, text=True, env=env, timeout=3600)
        for line in (r.stdout + r.stderr).splitlines():
            print('  ' + line, flush=True)
            if line.startswith(('PASS ', 'FAIL ')): check(line.startswith('PASS '), 'libraries_row ' + line.split(' ', 1)[1][:300])
        # L6: the embedded choosers list the libraries (their titles say how many items loaded: read from the screenshots)
        cwp = next((w for w in wins() if w[0] == cw[0]), cw)
        def chooser(tab, key_name, name):
            r = rects().get(tab)
            if r: click(cwp[3][0] + r['x'] + r['w'] / 2, cwp[3][1] + 28 + r['y'] + r['h'] / 2)
            time.sleep(5)
            pp = last_push()
            if pp: click(pp[0][0] + pp[0][2] * 0.5, pp[0][1] + pp[0][3] * 0.6)
            time.sleep(0.8); d0 = hub_log().count('ET_DIALOG_OPENED')
            key(key_name)
            opened = wait_for(lambda: hub_log().count('ET_DIALOG_OPENED') > d0, 20, 0.5)
            time.sleep(10)  # the count in the title grows while the libraries load
            shot(f'L6-{name}')
            title = next((l for l in reversed(hub_log().splitlines()) if 'dialog opened' in l), '')
            key(K_ESC); time.sleep(1.5)
            return bool(opened), title[:160]
        o_sym = chooser('kicad-tab:SCH', 0, 'symbols')
        check(o_sym[0], f'L6: the embedded schematic editor\'s symbol chooser opens (A); its item count is in L6-symbols.png ({o_sym[1]})')
        o_fp = chooser('kicad-tab:PCB', 0, 'footprints')
        check(o_fp[0], f'L6: the embedded board editor\'s footprint chooser opens (A); its item count is in L6-footprints.png ({o_fp[1]})')
        raise SystemExit(1 if FAILS else 0)
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
    mark_find = len(client_log().splitlines())
    key(K_F, CMD)
    opened = wait_for(lambda: hub_log().count('ET_DIALOG_OPENED') > d0, 5)
    check(bool(opened) and active(kpid), 'a KiCad dialog (Find) opens with the keyboard in it')
    time.sleep(0.5); shot('02c-dialog')
    n = notices_outside('find', mark_find)
    print(f'INFO notices while Find is up: {n}', flush=True)
    check(not any(v[0] for v in n.values()), f'while KiCad\'s Find dialog is up, no pane notice is drawn inside the pane\'s rect, under KiCad\'s frame ({n})')
    # Escape closes it (KiCad has the keyboard; a click aimed at its Close button once hit Backplane's own close and shut the viewer)
    key(K_ESC)
    check(bool(wait_for(lambda: hub_log().count('ET_DIALOG_CLOSED') > c0, 4)), 'Escape closes it')
    # a click in Backplane's own chrome (a neutral spot: the thread's timeline) gives it the keyboard back
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    click(cw[3][0] + 200, cw[3][1] + 28 + 300)
    check(bool(wait_for(lambda: active(client.pid), 4)), 'a click in Backplane\'s chrome gives it the keyboard back')
    click(*board_spot())
    check(bool(wait_for(lambda: active(kpid), 4)), 'a click on the board gives KiCad the keyboard again')
    # ---- our window moved: the frame follows ----
    before = kicad_frame(); wb = next((w for w in wins() if w[0] == cw[0]), cw)
    win_move(client.pid, wb[3][0] + 60, wb[3][1] + 40)
    wa = lambda: next((w for w in wins() if w[0] == cw[0]), cw)[3]
    # the frame follows by as much as the window really moved (a drag near the screen's edge may move it less than asked)
    moved = wait_for(lambda: (k := kicad_frame()) and before and (wa()[0], wa()[1]) != (wb[3][0], wb[3][1])
                     and (k[3][0] - before[3][0], k[3][1] - before[3][1]) == (wa()[0] - wb[3][0], wa()[1] - wb[3][1]) and k, 5)
    print(f'client window {wb[3]} -> {wa()}', flush=True)
    check(bool(moved), f'Backplane\'s window moved: KiCad\'s frame follows it ({before and before[3]} -> {(kicad_frame() or [None]*4)[3]})')
    matches('after the move')
    cw = next((w for w in wins() if w[0] == cw[0]), cw)
    win_size(client.pid, cw[3][2] - 120, cw[3][3] - 60); time.sleep(1.5)
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
    win_full(client.pid, True)
    time.sleep(3)
    hidden = not any(w[0] == f[0] for w in wins())
    shot('03-full-screen')
    check(hidden, 'Backplane full screen: KiCad\'s frame leaves the screen')
    mark_full = max(0, len(client_log().splitlines()) - 400)
    line = wait_for(lambda: rects().get('kicad:pop-full'), 5)
    check(bool(line), 'full screen: the pane says so and offers Pop out')
    n = notices_outside('full-screen', mark_full)
    # (in full screen KiCad's frame is off the screen: the line takes the pane's place and is seen there)
    print(f'INFO notices in full screen: {n} (screenshot notices-full-screen.png)', flush=True)
    win_full(client.pid, False)
    back = wait_for(kicad_frame, 8, 0.25)
    check(bool(back), 'out of full screen: KiCad\'s frame is back over the pane, above Backplane')
    shot('04-back')
    matches('after full screen')
    # ---- Pop out and back in ----
    before_wins = {w[0] for w in wins() if w[1] == client.pid}
    # the window back to its first size (the strip's buttons at a narrow width may sit past its edge)
    # and on screen (run 18: at x=-376 after full screen, so the click meant to make Backplane active fell off the screen)
    win_move(client.pid, 20, 30); time.sleep(0.8)
    win_size(client.pid, 1000, 700); time.sleep(1.5)
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
        win_close(client.pid, newwin[3])
        time.sleep(3)
        check(bool(wait_for(kicad_frame, 8, 0.25)), 'popped back in: KiCad\'s frame is over the pane again')
    # ---- Cmd+W in KiCad: the pane follows KiCad (lane B's 47b00d199) ----
    K_W, K_RETURN = 13, 36
    board = os.path.join(OUT, 'proj', 'ecc83-pp.kicad_pcb')
    def edit_again():
        e = rects().get('ui:Edit'); cw2 = next((w for w in wins() if w[0] == cw[0]), cw)
        if e: click(cw2[3][0] + e['x'] + e['w'] / 2, cw2[3][1] + 28 + e['y'] + e['h'] / 2)
        return wait_for(kicad_frame, 60, 0.25)
    def delete_selection(tag):
        """Delete what is selected as a Mac person does: the keyboard's delete key (Backspace); forward delete if KiCad did not take it."""
        m0 = hub_log().count('ET_MODIFIED_CHANGED') + hub_log().count('ET_DOCUMENT_EDITED')
        key(K_DEL)
        if wait_for(lambda: hub_log().count('ET_MODIFIED_CHANGED') + hub_log().count('ET_DOCUMENT_EDITED') > m0, 2.5): RES[f'delete_{tag}'] = 'backspace'; return
        key(K_FDEL); RES[f'delete_{tag}'] = 'forward delete' if wait_for(lambda: hub_log().count('ET_MODIFIED_CHANGED') + hub_log().count('ET_DOCUMENT_EDITED') > m0, 2.5) else 'neither'
        print(f'INFO delete ({tag}): Backspace did not delete the selection in KiCad; {RES[f"delete_{tag}"]} did', flush=True)
    def unsaved_edit():
        # one item: a click on the board's middle selects what is there (U1's footprint); the Save case then writes the board without it
        p = last_push(); click(p[0][0] + p[0][2] * 0.5, p[0][1] + p[0][3] * 0.5); time.sleep(0.8)
        delete_selection('cmd-w'); time.sleep(0.6)
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
    key(K_A, CMD); time.sleep(0.6); delete_selection('quit'); time.sleep(1.0)
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
    def descendants(roots):
        kids = {}
        for l in subprocess.run(['ps', '-axo', 'pid=,ppid='], capture_output=True, text=True).stdout.splitlines():
            try: a, b = map(int, l.split()); kids.setdefault(b, []).append(a)
            except ValueError: pass
        out, todo = set(), list(roots)
        while todo:
            for k in kids.get(todo.pop(), []):
                if k not in out: out.add(k); todo.append(k)
        return out
    ours = descendants([p.pid for p in procs])
    for p in procs:
        try: p.terminate()
        except Exception: pass
    time.sleep(2)
    for p in procs:
        try: p.kill()
        except Exception: pass
    # what this run started and nothing else: the descendants of its own processes (the hub's KiCad among them), taken before they were
    # stopped (the repo's rule: never end processes by name or pattern)
    still = [pid for pid in sorted(ours) if subprocess.run(['kill', '-0', str(pid)], capture_output=True).returncode == 0]
    for pid in still:
        try: os.kill(pid, 9)
        except OSError: pass
    left = '\n'.join(map(str, still))
    if os.path.isdir(DOCS_DIR): shutil.rmtree(DOCS_DIR, ignore_errors=True); RES['docs_folder_removed'] = not os.path.isdir(DOCS_DIR)
    json.dump({'failures': FAILS, 'results': RES, 'left_running': left}, open(os.path.join(OUT, 'summary.json'), 'w'), indent=2)
    print(json.dumps({'failures': FAILS, 'results': RES, 'left_running': left}, indent=2))
sys.exit(1 if FAILS else 0)
