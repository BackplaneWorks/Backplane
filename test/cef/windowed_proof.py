#!/usr/bin/env python3
"""The Browser tab's windowed Chromium on this machine, the same way on Linux, macOS and Windows: a hub on a temp home and the client showing the
Browser tab, driven by the client's own script. Everything checked comes from the client's `cef` reports, which read Chromium's native child
window back from the window system (its place in the client's window and whether it is shown): it sits over the Browser pane, hides under a gpui
overlay (the Improvements panel) and while another view is shown, follows a layout change (the sidebar folded), shows only the active tab, and
opening the tab leaves the keyboard with the client. Screenshots per step in OUT/<step>.png.
(test/cef/windowed_e2e.sh is the Linux one with a window manager, the X window tree and the person's real keyboard.)
usage: windowed_proof.py CLIENT HUB ASSETS OUT   -> one line per check, exit 0 when all pass"""
import os, sys, json, time, shutil, subprocess, socket, platform
CLIENT, HUB, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:5]]
WIN, MAC = os.name == 'nt', platform.system() == 'Darwin'
shutil.rmtree(OUT, ignore_errors=True)
P = lambda *a: os.path.join(OUT, *a)
for d in ['home', 'proj', 'bin', 'fakehome/.cache', 'fakehome/.config', 'fakehome/AppData/Roaming', 'fakehome/AppData/Local']: os.makedirs(P(d), exist_ok=True)
for n in ['claude', 'codex', 'grok', 'muse', 'opencode']:
    open(P('bin', n), 'w', newline='\n').write('#!/bin/sh\nexit 1\n')
    if not WIN: os.chmod(P('bin', n), 0o755)
s = socket.socket(); s.bind(('127.0.0.1', 0)); PORT = s.getsockname()[1]; s.close()
P1 = 'data:text/html,<body style="margin:0;background:%23e8f0ff"><h1 style="color:%23c00">first tab</h1></body>'
P2 = 'data:text/html,<body style="margin:0;background:%23e0ffe0"><h1>second tab</h1></body>'
now = int(time.time())
seed = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'web', 'root': P('proj'), 'at': now},
        {'$': 'ThreadCreated', 'id': 't1', 'project': 'p1', 'title': 'Windowed', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now}]
open(P('home/events.jsonl'), 'w', newline='\n').write(''.join(json.dumps(e) + '\n' for e in seed))
step = lambda name: [{'mark': name}, {'cef': {'report': True}}, {'wait': 2500}]
sc = [{'select': 0}, {'wait': 2000}, {'click': 'rail:Browser'}, {'wait': 6000}, {'cef': {'goto': P1}}, {'wait': 4000}] + step('shown')
sc += [{'keys': 'secondary-b'}, {'wait': 2000}] + step('sidebar_folded') + [{'keys': 'secondary-b'}, {'wait': 2000}]
sc += [{'click': 'ui:Improvements'}, {'wait': 2000}] + step('overlay') + [{'keys': 'ctrl-alt-i'}, {'wait': 2000}] + step('overlay_closed')
sc += [{'click': 'rail:Files'}, {'wait': 2000}] + step('other_view') + [{'click': 'rail:Browser'}, {'wait': 2500}] + step('back')
sc += [{'browser': {'op': 'new_tab', 'url': P2}}, {'wait': 4000}] + step('two_tabs') + [{'cef': {'tab': 0}}, {'wait': 2000}] + step('first_tab_again') + [{'wait': 3000}]
json.dump(sc, open(P('script.json'), 'w'))
env = dict(os.environ)
env.update(BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', HOME=P('fakehome'), USERPROFILE=P('fakehome'), APPDATA=P('fakehome/AppData/Roaming'),
           LOCALAPPDATA=P('fakehome/AppData/Local'), XDG_CONFIG_HOME=P('fakehome/.config'), XDG_CACHE_HOME=P('fakehome/.cache'))
procs = []
def spawn(args, name, e):
    kw = {'creationflags': subprocess.CREATE_NO_WINDOW} if WIN else {}
    p = subprocess.Popen(args, env=e, cwd=P('proj'), stdout=open(P(name), 'ab'), stderr=subprocess.STDOUT, **kw); procs.append(p); return p
spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', env)
import urllib.request
for _ in range(150):
    try: urllib.request.urlopen('http://127.0.0.1:%d/hello' % PORT, timeout=1); break
    except Exception: time.sleep(0.2)
cenv = dict(env); cenv.update(BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_CEF_PROFILE=P('cef-profile'))
client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '80'], 'client.log', cenv)
def screen(path):
    """The whole screen as a PNG at `path`, with what the OS has (Windows: PIL, else PowerShell's System.Drawing); nothing when there is no way."""
    try:
        if WIN:
            try:
                from PIL import ImageGrab; ImageGrab.grab(all_screens=True).save(path); return
            except ImportError: pass
            ps = ("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; "
                  "$m=New-Object System.Drawing.Bitmap $b.Width,$b.Height; $g=[System.Drawing.Graphics]::FromImage($m); $g.CopyFromScreen($b.Left,$b.Top,0,0,$m.Size); "
                  "$m.Save('%s',[System.Drawing.Imaging.ImageFormat]::Png)" % path.replace("'", "''"))
            subprocess.run(['powershell', '-NoProfile', '-Command', ps], check=False, timeout=30)
        elif MAC: subprocess.run(['screencapture', '-x', path], check=False, timeout=30)
        elif os.environ.get('WAYLAND_DISPLAY') and shutil.which('grim'): subprocess.run(['grim', path], check=False, timeout=30)
        elif os.environ.get('DISPLAY') and shutil.which('import'): subprocess.run(['import', '-window', 'root', path], check=False, timeout=30)
    except Exception as e: print('note: no screenshot (%s)' % e)
def log(): return open(P('client.log'), encoding='utf-8', errors='replace').read().splitlines() if os.path.exists(P('client.log')) else []
seen = set(); t0 = time.time()
while client.poll() is None and time.time() - t0 < 120:
    for l in log():
        if '"event":"mark"' in l:
            m = json.loads(l[l.index('{'):])['name']
            if m not in seen: seen.add(m); time.sleep(0.5); screen(P(m + '.png'))
    time.sleep(0.25)
for p in procs:
    try: p.terminate()
    except Exception: pass
L = log(); marks = []; reps = {}
for l in L:
    if '"event":"mark"' in l: marks.append(json.loads(l[l.index('{'):])['name'])
    elif '"event":"cef"' in l and marks:
        try: reps.setdefault(marks[-1], json.loads(l[l.index('{'):]))
        except Exception: pass
ok = fail = 0
def check(what, good, ev=''):
    global ok, fail
    print(('PASS ' if good else 'FAIL ') + what + ('' if good else ' (' + str(ev)[:300] + ')')); ok += bool(good); fail += not good
def nat(m): r = reps.get(m, {}); return r, (r.get('native') or {})
def at_pane(r, n):  # the child's place in the client's window (physical px on Windows and X11, points on macOS) against the pane's (logical px)
    return any(abs(n.get('x', -99) - round(r.get('view_x', 0) * s)) <= 2 and abs(n.get('y', -99) - round(r.get('view_y', 0) * s)) <= 2 for s in (1, 1.25, 1.5, 1.75, 2, 2.5, 3))
mode = [l for l in L if 'cef_mode' in l]
check('the client chose windowed Chromium (%s)' % (mode[0][9:] if mode else 'no cef_mode line'), bool(mode) and '"windowed"' in mode[0], mode)
r, n = nat('shown'); check('Chromium\'s window is shown over the Browser pane (%s, pane at %s,%s)' % (n, r.get('view_x'), r.get('view_y')), n.get('shown') and at_pane(r, n) and n.get('w', 0) > 200, r)
check('opening the Browser tab left the keyboard with the client (taken: %s)' % r.get('taken'), r.get('taken') is False, r)
r0, n0 = nat('shown'); r, n = nat('sidebar_folded')
check('a layout change moves it with the pane (x %s -> %s)' % (n0.get('x'), n.get('x')), n.get('shown') and n.get('x') != n0.get('x') and n.get('w') != n0.get('w') and at_pane(r, n), r)
r, n = nat('overlay'); check('a gpui overlay over the pane hides it (covered %s, shown %s)' % (r.get('covered'), n.get('shown')), r.get('covered') and not n.get('shown'), r)
r, n = nat('overlay_closed'); check('closing the overlay shows it again', not r.get('covered') and n.get('shown'), r)
r, n = nat('other_view'); check('another view in the pane hides it (shown %s)' % n.get('shown'), bool(n) and not n.get('shown'), r)
r, n = nat('back'); check('back on the Browser tab it is shown again', n.get('shown') and at_pane(r, n), r)
r, _ = nat('two_tabs'); ns = r.get('natives', [])
check('with two tabs only the active one is shown (%s)' % [x and x.get('shown') for x in ns], len(ns) == 2 and ns[1] and ns[1].get('shown') and ns[0] and not ns[0].get('shown'), ns)
r, _ = nat('first_tab_again'); ns = r.get('natives', [])
check('switching tabs swaps them (%s)' % [x and x.get('shown') for x in ns], len(ns) == 2 and ns[0] and ns[0].get('shown') and ns[1] and not ns[1].get('shown'), ns)
print('%d passed, %d failed; OUT %s' % (ok, fail, OUT)); sys.exit(1 if fail else 0)
