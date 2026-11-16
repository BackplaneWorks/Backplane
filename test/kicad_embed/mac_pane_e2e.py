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
from AppKit import NSWorkspace

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
K_A, K_K, K_ESC = 0, 40, 53

# ---- the project, the hub's home, KiCad's settings where KiCad on macOS reads them ----
os.makedirs(OUT, exist_ok=True)
subprocess.run([sys.executable, os.path.join(REPO, 'test/kicad_embed/lib/seed_project.py'), OUT, REPO], check=True)
for v in ('10.0', '10.99'):
    src = os.path.join(OUT, 'fh/.config/kicad', v)
    if os.path.isdir(src): shutil.copytree(src, os.path.join(OUT, 'fh/Library/Preferences/kicad', v), dirs_exist_ok=True)
tmp = tempfile.mkdtemp(prefix='bpk-', dir='/tmp')   # a Unix socket path is at most 104 bytes on macOS
port = 3893
env = dict(os.environ, HOME=os.path.join(OUT, 'fh'), TMPDIR=tmp, BACKPLANE_KICAD_CLI=os.path.join(RT, 'bin/kicad-cli'),
           BACKPLANE_AGENTS=os.path.join(OUT, 'bin'), BACKPLANE_NO_UPDATE='1', BACKPLANE_KICAD_DEBUG='1', FAKE_LOG=os.path.join(OUT, 'standin.log'))
hub = subprocess.Popen([HUB, '--home', os.path.join(OUT, 'home'), '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'],
                       env=env, stdout=open(os.path.join(OUT, 'hub.log'), 'w'), stderr=subprocess.STDOUT); procs.append(hub)
wait_for(lambda: subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', f'http://127.0.0.1:{port}/hello']).returncode == 0, 30)
script = [{"select": 0}, {"wait": 2500}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 900000}]
json.dump(script, open(os.path.join(OUT, 'script.json'), 'w'))
cenv = dict(os.environ, HOME=os.path.join(OUT, 'fh'), BACKPLANE_GPUI_ASSETS=ASSETS)
client = subprocess.Popen([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', os.path.join(OUT, 'home/token'), '--script', os.path.join(OUT, 'script.json'), '--exit-after', '900'],
                          env=cenv, stdout=open(os.path.join(OUT, 'client.log'), 'w'), stderr=subprocess.STDOUT); procs.append(client)


def shot(name): subprocess.run(['screencapture', '-x', os.path.join(OUT, name + '.png')])
def hub_log(): return open(os.path.join(OUT, 'hub.log'), errors='replace').read()
def client_log(): return open(os.path.join(OUT, 'client.log'), errors='replace').read()


try:
    cw = wait_for(lambda: max((w for w in wins() if w[1] == client.pid), key=lambda w: w[3][2] * w[3][3], default=None), 60)
    check(bool(cw), 'the client window is on screen')
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
    # ---- Edit, then typing with no click ----
    front = wait_for(lambda: NSWorkspace.sharedWorkspace().frontmostApplication().processIdentifier() == kpid, 5)
    check(bool(front), 'after Edit, with no click, KiCad is the frontmost app (it has the keyboard)')
    n0 = hub_log().count('ET_SELECTION_CHANGED')
    key(K_A, CMD); time.sleep(0.8); key(K_ESC); time.sleep(0.8)
    n1 = hub_log().count('ET_SELECTION_CHANGED')
    check(n1 - n0 >= 2, f'typing with no click reaches the canvas: Cmd+A and Escape change the selection ({n1 - n0} changes)')
    # ---- Cmd+K in KiCad reaches Backplane ----
    k0 = client_log().count('"kicad_key"')
    key(K_K, CMD)
    got = wait_for(lambda: client_log().count('"kicad_key"') > k0, 5)
    check(bool(got), 'Cmd+K pressed in KiCad reaches Backplane (the client takes the reserved chord)')
    time.sleep(1); shot('02-cmd-k'); key(K_ESC); time.sleep(0.5)
    # ---- our window moved: the frame follows ----
    before = kicad_frame()
    subprocess.run(['osascript', '-e', f'tell application "System Events" to set position of (first window of (first process whose unix id is {client.pid})) to {{{cw[3][0] + 60}, {cw[3][1] + 40}}}'], capture_output=True)
    moved = wait_for(lambda: (k := kicad_frame()) and before and (k[3][0] - before[3][0], k[3][1] - before[3][1]) == (60, 40) and k, 5)
    check(bool(moved), f'Backplane\'s window moved: KiCad\'s frame follows it ({before and before[3]} -> {(kicad_frame() or [None]*4)[3]})')
    # ---- full screen: hidden, then back over the pane ----
    subprocess.run(['osascript', '-e', f'tell application "System Events" to set value of attribute "AXFullScreen" of (first window of (first process whose unix id is {client.pid})) to true'], capture_output=True)
    time.sleep(3)
    hidden = not any(w[0] == f[0] for w in wins())
    shot('03-full-screen')
    check(hidden, 'Backplane full screen: KiCad\'s frame leaves the screen')
    subprocess.run(['osascript', '-e', f'tell application "System Events" to set value of attribute "AXFullScreen" of (first window of (first process whose unix id is {client.pid})) to false'], capture_output=True)
    back = wait_for(kicad_frame, 8, 0.25)
    check(bool(back), 'out of full screen: KiCad\'s frame is back over the pane, above Backplane')
    shot('04-back')
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
