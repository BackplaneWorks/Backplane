#!/usr/bin/env python3
"""Client-side rows of F05-F22 on macOS: the GPUI client in the runner's logged-in GUI session, a hub on a temp home with the stand-in `claude` (lib/standin_rows.py),
real input through cliclick and Quartz events, a real clipboard (osascript), screenshots with `screencapture`. A check passes from what the hub logged, what the client reported
(GPUI_METRIC lines) or pixels on a screenshot; never from the driver's own say-so. Screenshots are cropped to the window, so the rows' coordinates are window-local, as on Linux.
usage: gpui-f-rows-mac.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [rows...]   -> OUT/rows.json, OUT/shots/*.png, one line per check
needs: cliclick (brew), pyobjc-framework-Quartz (pip), Pillow"""
import os, sys, json, time, shutil, subprocess, tempfile, glob, re, signal
CLIENT, HUB, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:5]]
PORT = int(sys.argv[5]) if len(sys.argv) > 5 else 3891
ONLY = set(sys.argv[6:])
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..'))
rows = []; procs = []
import threading, http.server
ORDER_SEEN = []
class OrderServer(http.server.BaseHTTPRequestHandler):  # a stand-in order server: the quote upload and the poll, recording every request; it has no pay or buy route
    def log_message(self, *a): pass
    def _send(self, code, obj):
        b = json.dumps(obj).encode(); self.send_response(code); self.send_header('content-type', 'application/json'); self.send_header('content-length', str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_POST(self):
        n = int(self.headers.get('content-length') or 0); body = self.rfile.read(n) if n else b''; ORDER_SEEN.append(('POST', self.path, len(body)))
        self._send(200, {'id': 'q_rig1', 'token': 'tok-rig-1'}) if self.path.startswith('/v1/quotes') else self._send(404, {'error': 'no'})
    def do_GET(self):
        ORDER_SEEN.append(('GET', self.path, 0))
        self._send(200, {'status': 'quoted', 'currency': 'USD', 'total': 12.5, 'lines': [{'name': 'Board', 'qty': 5, 'total': 12.5}]}) if self.path.startswith('/v1/quotes/q_rig1') else self._send(404, {'error': 'no'})
order_srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), OrderServer); threading.Thread(target=order_srv.serve_forever, daemon=True).start()
def log(i, ok, ev):
    rows.append({'id': i, 'ok': bool(ok), 'ev': ev}); print(('ok   ' if ok else 'FAIL ') + i + ' ' + ev, flush=True)
shutil.rmtree(OUT, ignore_errors=True)
for d in ['home', 'proj', 'bin', 'fakehome/.cache', 'shots', 'proj/mech', 'proj/boards']: os.makedirs(os.path.join(OUT, d), exist_ok=True)
RUN = tempfile.mkdtemp(prefix='gr.'); os.chmod(RUN, 0o700)
P = lambda *a: os.path.join(OUT, *a)
shutil.copy(os.path.join(HERE, 'lib/standin_rows.py'), P('bin/claude'))
for n in ['codex', 'grok', 'muse', 'opencode']:
    open(P('bin', n), 'w').write('#!/bin/sh\nexit 1\n')
for f in glob.glob(P('bin/*')): os.chmod(f, 0o755)
shutil.copy(os.path.join(REPO, 'test/files/fixtures/mesh/bracket.step'), P('proj/mech/bracket.step'))
for ext in ('kicad_pcb', 'kicad_sch'): shutil.copy(os.path.join(REPO, 'test/files/fixtures/plot/ecc83-pp.' + ext), P('proj/ecc83.' + ext))
open(P('proj/ecc83.kicad_pro'), 'w').write('{}\n')
open(P('proj/readme.txt'), 'w').write('rig fixture\n')
now = int(time.time())
seed = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'rows', 'root': P('proj'), 'at': now}] + [{'$': 'ThreadCreated', 'id': 't%d' % i, 'project': 'p1', 'title': 'Thread %d' % i, 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + i} for i in (1, 2, 3)]
seed += [{'$': 'SettingSet', 'key': 'buy.t3', 'value': 't0', 'at': now + 3}, {'$': 'ModesSet', 'id': 't1', 'runtime': 'approval-required', 'interaction': 'default', 'model': '', 'at': now + 3}, {'$': 'ModesSet', 'id': 't2', 'runtime': 'full-access', 'interaction': 'plan', 'model': '', 'at': now + 3}]
open(P('home/events.jsonl'), 'w').write(''.join(json.dumps(e) + '\n' for e in seed))
env = dict(os.environ)
env.update(LC_ALL='C', BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', SHELL='/bin/sh', FAKE_LOG=P('fake.log'), FAKE_SECS='600', PATH=P('bin') + ':' + os.environ['PATH'], BACKPLANE_ORDER_SERVER='http://127.0.0.1:%d' % order_srv.server_address[1])
def spawn(args, log_name, e=None, cwd=None):
    p = subprocess.Popen(args, env=e or env, cwd=cwd, stdout=open(P(log_name), 'ab'), stderr=subprocess.STDOUT); procs.append(p); return p
def cleanup(*_):
    for p in procs:
        try: p.terminate()
        except Exception: pass
    shutil.rmtree(RUN, ignore_errors=True)
    json.dump({'rows': rows}, open(P('rows.json'), 'w'), indent=1)
signal.signal(signal.SIGTERM, lambda *a: (cleanup(), sys.exit(1)))
hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=P('proj'))
for _ in range(100):
    if subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', 'http://127.0.0.1:%d/hello' % PORT]).returncode == 0: break
    time.sleep(0.2)
assert ('BACKPLANE_AGENTS=' + P('bin')) in subprocess.run(['ps', 'eww', '-p', str(hub.pid)], capture_output=True, text=True).stdout, 'hub is not running stand-in agents only'
denv = dict(env)
cenv = dict(denv, BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1')
open(P('script.json'), 'w').write('[{"select":0},{"wait":3000000}]')
def start_client():
    c = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '3000'], 'client.log', cenv)
    time.sleep(8); fit_window(); return c
# ---- the window: found and fitted to the screen through System Events; its content origin is the window's origin plus the title bar
TB = int(os.environ.get('TITLEBAR', '28'))
def osa(src):
    r = subprocess.run(['osascript', '-e', src], capture_output=True, text=True, timeout=30); return r.stdout.strip() if r.returncode == 0 else 'ERR ' + r.stderr.strip()
PROC = os.path.basename(CLIENT)
def fit_window():
    out = osa('tell application "System Events" to tell process "%s" to set frontmost to true' % PROC)
    sw, sh = screen_size()
    osa('tell application "System Events" to tell process "%s" to set position of window 1 to {0, 30}' % PROC)
    osa('tell application "System Events" to tell process "%s" to set size of window 1 to {%d, %d}' % (PROC, sw, sh - 30))
    time.sleep(2)
def screen_size():
    r = subprocess.run(['osascript', '-e', 'tell application "Finder" to get bounds of window of desktop'], capture_output=True, text=True).stdout.strip().split(',')
    return int(r[2]), int(r[3])
def win():   # (x, y, w, h) of the window content in screen coordinates
    p = osa('tell application "System Events" to tell process "%s" to get {position, size} of window 1' % PROC)
    try: x, y, w, h = [int(float(v)) for v in p.replace('{', '').replace('}', '').split(',')]
    except Exception: raise SystemExit('no window found for %s: %s' % (PROC, p))
    return x, y + TB, w, h - TB
client = start_client()
time.sleep(2)
# ---- helpers: input
import Quartz
def cli(*a): subprocess.run(['cliclick', *a], check=False, timeout=30)
KEYS = {'Return': 'return', 'Escape': 'esc', 'BackSpace': 'delete', 'Tab': 'tab', 'space': 'space', 'Delete': 'fwd-delete', 'Up': 'arrow-up', 'Down': 'arrow-down', 'Left': 'arrow-left', 'Right': 'arrow-right'}
def abs_(x, y): ox, oy, _, _ = win(); return int(ox + x), int(oy + y)
def xin(cmd, *a):
    if cmd == 'click': x, y = abs_(a[0], a[1]); cli('m:%d,%d' % (x, y), 'w:60', 'c:%d,%d' % (x, y))
    elif cmd == 'type': cli('w:80', 't:' + str(a[0]))
    elif cmd == 'key': cli('kp:' + KEYS.get(a[0], a[0].lower()))
    elif cmd == 'wheel':
        x, y = abs_(a[0], a[1]); cli('m:%d,%d' % (x, y)); time.sleep(0.2)
        for _ in range(abs(int(a[2]))): Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateScrollWheelEvent(None, Quartz.kCGScrollEventUnitLine, 1, 3 if int(a[2]) > 0 else -3)); time.sleep(0.15)
    elif cmd == 'drag':
        x1, y1 = abs_(a[0], a[1]); x2, y2 = abs_(a[2], a[3]); cli('dd:%d,%d' % (x1, y1), 'dm:%d,%d' % ((x1 + x2) // 2, (y1 + y2) // 2), 'dm:%d,%d' % (x2, y2), 'du:%d,%d' % (x2, y2))
    elif cmd == 'chord': chord_(a)
    time.sleep(0.1)
MODS = {'ctrl': 'cmd', 'cmd': 'cmd', 'shift': 'shift', 'alt': 'alt'}   # the rows say ctrl for the secondary key, which is Cmd on a Mac (keys.rs)
def chord_(names):
    *mods, k = names; ms = ','.join(MODS[m] for m in mods); k = {'period': '.', 'comma': ',', 'bracketleft': '[', 'bracketright': ']'}.get(k, k)
    cli('kd:' + ms, 't:' + k, 'ku:' + ms) if len(k) == 1 else cli('kd:' + ms, 'kp:' + KEYS.get(k, k), 'ku:' + ms)
def chord(c): xin('chord', *c.split())
def shot(name):
    f = P('shots', name + '.png'); full = f + '.full.png'
    subprocess.run(['screencapture', '-x', full]); from PIL import Image
    im = Image.open(full).convert('RGB'); x, y, w, h = win(); sc = im.size[0] / float(screen_size()[0])
    im.crop((int(x * sc), int(y * sc), int((x + w) * sc), int((y + h) * sc))).save(f); os.remove(full); return f
def set_clipboard_png(path): osa('set the clipboard to (read (POSIX file "%s") as «class PNGf»)' % path)
def metrics(): return [json.loads(l.split('GPUI_METRIC ', 1)[1]) for l in open(P('client.log'), errors='replace') if 'GPUI_METRIC ' in l]
def rects():
    r = {}
    for m in metrics():
        if m.get('event') == 'layout': r.update(m.get('rects', {}))
    return r
def ctrls():
    c = {}
    for m in metrics():
        if m.get('event') == 'controls':
            c = {x['id']: x for x in m.get('controls', [])} if m.get('controls') else c
    return c
def events():
    out = []
    for l in open(P('home/events.jsonl'), errors='replace'):
        try: out.append(json.loads(l))
        except Exception: pass
    return out
def until(f, s=15):
    end = time.time() + s
    while time.time() < end:
        if f(): return True
        time.sleep(0.2)
    return bool(f())
def click_ctl(cid):
    c = ctrls().get(cid)
    if not c: return False
    r = c['rect']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + r['h'] / 2)); return True
def px(name):
    from PIL import Image
    return Image.open(P('shots', name + '.png')).convert('RGB')
def differs(a, b, box=None):
    from PIL import ImageChops
    if box: a, b = a.crop(box), b.crop(box)
    return ImageChops.difference(a, b).getbbox() is not None
def want(i):
    return not ONLY or i in ONLY
exec(open(os.path.join(HERE, 'gpui-f-rows-mac.rows.py')).read())
cleanup()
sys.exit(0 if all(r['ok'] for r in rows) else 1)
