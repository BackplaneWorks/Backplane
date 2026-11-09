#!/usr/bin/env python3
"""The embedded Browser on this machine, end to end, the same way on Linux, macOS and Windows: a hub on a temp home with the stand-in `claude`
(test/e2e/lib/standin_rows.py), the client showing the thread's Browser tab (embedded Chromium, feature cef), and the agent's own browser tools
(preview_navigate, preview_click, preview_evaluate, preview_snapshot with a screenshot) through the hub's MCP, against a local test page.
Passes only on what the client says of its embedded page (its `cef` report: the page's address and title after the agent's click) and what the
agent's tools answered; the screenshots are the window (OUT/window.png, from the screen) and the agent's own (OUT/agent/3-preview_snapshot.png).
Run it in a desktop session (Windows: test/windows/rig/sess.sh; macOS: from the logged-in session, e.g. through `open` or a LaunchAgent; Linux: DISPLAY set).
usage: agent_proof.py CLIENT HUB HELPER ASSETS OUT [PORT]   -> one line per check, exit 0 when all pass"""
import os, sys, json, time, shutil, subprocess, threading, http.server, functools, socket, platform
CLIENT, HUB, HELPER, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:6]]
PORT = int(sys.argv[6]) if len(sys.argv) > 6 else 0
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..'))
WIN, MAC = os.name == 'nt', platform.system() == 'Darwin'
def free():
    s = socket.socket(); s.bind(('127.0.0.1', 0)); p = s.getsockname()[1]; s.close(); return p
PORT = PORT or free()
shutil.rmtree(OUT, ignore_errors=True)
P = lambda *a: os.path.join(OUT, *a)
for d in ['home', 'proj', 'bin', 'agent', 'fakehome/.cache', 'fakehome/.config', 'fakehome/AppData/Roaming', 'fakehome/AppData/Local']: os.makedirs(P(d), exist_ok=True)
# the test page (a button that sets the title), served on loopback
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(type('Q', (http.server.SimpleHTTPRequestHandler,), {'log_message': lambda *a: None}), directory=os.path.join(HERE, 'page')))
threading.Thread(target=srv.serve_forever, daemon=True).start()
PAGE = 'http://127.0.0.1:%d/index.html' % srv.server_address[1]
shutil.copy(os.path.join(REPO, 'test/e2e/lib/standin_rows.py'), P('bin/claude'))
if not WIN: os.chmod(P('bin/claude'), 0o755)
for n in ['codex', 'grok', 'muse', 'opencode']:
    open(P('bin', n), 'w', newline='\n').write('#!/bin/sh\nexit 1\n')
    if not WIN: os.chmod(P('bin', n), 0o755)
calls = [{'name': 'preview_navigate', 'arguments': {'url': PAGE}},
         {'name': 'preview_click', 'arguments': {'locator': "role=button[name='Click me']"}},
         {'name': 'preview_evaluate', 'arguments': {'expression': 'document.title'}},
         {'name': 'preview_snapshot', 'arguments': {'screenshot': True}}]
json.dump(calls, open(P('calls.json'), 'w'))
now = int(time.time())
seed = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'browser', 'root': P('proj'), 'at': now},
        {'$': 'ThreadCreated', 'id': 't1', 'project': 'p1', 'title': 'Browser proof', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now},
        {'$': 'ModesSet', 'id': 't1', 'runtime': 'full-access', 'interaction': 'default', 'model': '', 'at': now}]
open(P('home/events.jsonl'), 'w', newline='\n').write(''.join(json.dumps(e) + '\n' for e in seed))
env = dict(os.environ)
env.update(BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', BACKPLANE_BROWSER=HELPER, FAKE_LOG=P('fake.log'), FAKE_SECS='5', BP_MCP_CALLS_FILE=P('calls.json'), BP_STANDIN_IMAGES=P('agent'),
           HOME=P('fakehome'), USERPROFILE=P('fakehome'), APPDATA=P('fakehome/AppData/Roaming'), LOCALAPPDATA=P('fakehome/AppData/Local'),
           XDG_CONFIG_HOME=P('fakehome/.config'), XDG_CACHE_HOME=P('fakehome/.cache'),
           PATH=os.pathsep.join([P('bin'), os.path.dirname(sys.executable), os.environ.get('PATH', '')]))
env.pop('SHELL', None) if WIN else None
procs = []
def spawn(args, name, e):
    kw = {'creationflags': subprocess.CREATE_NO_WINDOW} if WIN else {}
    p = subprocess.Popen(args, env=e, cwd=P('proj'), stdout=open(P(name), 'ab'), stderr=subprocess.STDOUT, **kw); procs.append(p); return p
def stop():
    for p in procs:
        try: p.terminate()
        except Exception: pass
    for p in procs:
        try: p.wait(10)
        except Exception:
            try: p.kill()
            except Exception: pass
hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', env)
import urllib.request
for _ in range(150):
    try: urllib.request.urlopen('http://127.0.0.1:%d/hello' % PORT, timeout=1); break
    except Exception: time.sleep(0.2)
else: print('FAIL the hub did not answer'); stop(); sys.exit(1)
script = [{'select': 0}, {'wait': 2500}, {'click': 'rail:Browser'}, {'wait': 8000}, {'cef': {'report': True}},
          {'type': 'zqcalls'}, {'send': True}, {'wait': 30000}, {'cef': {'report': True}}, {'wait': 6000}]
json.dump(script, open(P('script.json'), 'w'))
cenv = dict(env); cenv.update(BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_CEF_PROFILE=P('cef-profile'))
if os.environ.get('BACKPLANE_CEF'): cenv['BACKPLANE_CEF'] = os.environ['BACKPLANE_CEF']
client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '75'], 'client.log', cenv)
def logged(f, needle, secs):
    t = time.time() + secs
    while time.time() < t:
        try:
            if needle in open(P(f), encoding='utf-8', errors='replace').read(): return True
        except FileNotFoundError: pass
        time.sleep(0.3)
    return False
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
        else:
            # Wayland without X: weston's screenshooter (weston started with --debug) writes wayland-screenshot-*.png in its folder
            d = os.path.dirname(path); before = set(os.listdir(d))
            subprocess.run(['weston-screenshooter'], cwd=d, check=False)
            new = [f for f in os.listdir(d) if f not in before and f.endswith('.png')]
            if new: os.replace(os.path.join(d, new[0]), path)
    except Exception as e: print('note: no screenshot (%s)' % e)
ok = fail = 0
def check(what, good, ev=''):
    global ok, fail
    print(('PASS ' if good else 'FAIL ') + what + ('' if good else ' (%s)' % ev), flush=True); ok += bool(good); fail += not good
got_snap = logged('fake.log', 'CALL preview_snapshot', 60)
if got_snap: time.sleep(2); screen(P('window.png'))
logged('client.log', 'clicked:1', 25)
client.wait(90) if client.poll() is None else None
reports = []
for line in open(P('client.log'), encoding='utf-8', errors='replace'):
    if '"event":"cef"' in line and '{' in line:
        try: reports.append(json.loads(line[line.index('{'):]))
        except Exception: pass
r0 = reports[0] if reports else {}; r1 = reports[1] if len(reports) > 1 else {}
def data(r): return r.get('data', r)
r0, r1 = data(r0), data(r1)
check('the Browser tab is the embedded Chromium (a cef report, CDP port %s)' % r0.get('cdp_port'), r0.get('ok') is True and r0.get('cdp_port'), json.dumps(r0)[:300])
fake = open(P('fake.log'), encoding='utf-8', errors='replace').read() if os.path.exists(P('fake.log')) else ''
callres = {l.split()[1]: l for l in fake.splitlines() if l.startswith('CALL ')}
for c in calls:
    l = callres.get(c['name'], '')
    check('the agent\'s %s answered' % c['name'], l.split()[2:3] == ['ok'], l[:300] or 'not called')
check('preview_evaluate read the title the click set (clicked:1)', 'clicked:1' in callres.get('preview_evaluate', ''), callres.get('preview_evaluate', '')[:200])
check('the embedded page the person sees is the agent\'s page (%s)' % r1.get('url'), r1.get('url') == PAGE, json.dumps(r1)[:300])
check('the agent\'s click changed the embedded page (title %s)' % r1.get('title'), r1.get('title') == 'clicked:1', json.dumps(r1)[:300])
# windowed on Windows and macOS the client cannot read Chromium's pixels back (-1): the screen check below is the proof there
if (r1.get('non_white_samples') or 0) >= 0 or not r1.get('windowed'):
    check('the embedded page painted (non-white samples %s)' % r1.get('non_white_samples'), (r1.get('non_white_samples') or 0) > 20, json.dumps(r1)[:300])
def png_pixels(path):
    """(width, height, rows of RGB tuples) of an 8-bit PNG, with the standard library only (no PIL on every runner)."""
    import zlib, struct
    b = open(path, 'rb').read(); i = 8; idat = b''; plte = b''
    while i < len(b):
        n, t = struct.unpack('>I4s', b[i:i + 8]); d = b[i + 8:i + 8 + n]; i += 12 + n
        if t == b'IHDR': w, h, depth, ct = struct.unpack('>IIBB', d[:10])
        elif t == b'PLTE': plte = d
        elif t == b'IDAT': idat += d
    if depth != 8: return w, h, []
    bpp = {2: 3, 6: 4, 3: 1, 0: 1, 4: 2}[ct]; raw = zlib.decompress(idat); stride = w * bpp; rows = []; prev = bytearray(stride); o = 0
    for _ in range(h):
        f = raw[o]; line = bytearray(raw[o + 1:o + 1 + stride]); o += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0; up = prev[x]; c = prev[x - bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + up) & 255
            elif f == 3: line[x] = (line[x] + (a + up) // 2) & 255
            elif f == 4: pa, pb, pc = abs(up - c), abs(a - c), abs(a + up - 2 * c); line[x] = (line[x] + (a if pa <= pb and pa <= pc else up if pb <= pc else c)) & 255
        prev = line
        if ct == 3: rows.append([tuple(plte[3 * line[x]:3 * line[x] + 3]) for x in range(w)])
        elif ct in (0, 4): rows.append([(line[x * bpp],) * 3 for x in range(w)])
        else: rows.append([tuple(line[x * bpp:x * bpp + 3]) for x in range(w)])
    return w, h, rows
green = 0
if os.path.exists(P('window.png')):
    try:
        _, _, rows = png_pixels(P('window.png'))
        green = sum(1 for row in rows[::2] for (r, g, b) in row[::2] if abs(r - 0x22) < 14 and abs(g - 0xaa) < 14 and abs(b - 0x77) < 14)
    except Exception as e: print('note: could not read window.png:', e)
check('the page is on the screen in the client\'s window (its green button in the screenshot: %d px)' % green, green > 100, 'windowed %s' % r1.get('windowed'))
check('the agent\'s screenshot arrived', os.path.exists(P('agent', '3-preview_snapshot.png')), os.listdir(P('agent')))
check('the window was captured', os.path.exists(P('window.png')), 'no snapshot call' if not got_snap else 'screen capture failed')
stop()
print('%d passed, %d failed; OUT %s' % (ok, fail, OUT))
sys.exit(1 if fail else 0)
