#!/usr/bin/env python3
"""Client-side rows of F05-F22 on Windows: the same rows as gpui-f-rows.py (gpui-f-rows.rows.py), with the GPUI client in the Windows desktop session
(run this IN that session: test/windows/rig/sess.sh), a hub on a temp home with the stand-in `claude` (lib/standin_rows.py, run through the
interpreter its #! names), real input through SendInput, screenshots of the client's area, the real clipboard. A check passes from what the hub
logged, what the client reported (GPUI_METRIC lines) or pixels; never from the driver's own say-so. The client's area is made 1440x900 at the
screen's top left, so the rows' coordinates mean what they mean on Linux.
usage: win-f-rows.py CLIENT_EXE HUB_EXE ASSETS_DIR OUT [PORT] [rows...]   -> OUT/rows.json, OUT/shots/*.png, one line per check"""
import os, sys, json, time, shutil, subprocess, glob, re, ctypes, threading, http.server
from ctypes import wintypes as W
CLIENT, HUB, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:5]]
PORT = int(sys.argv[5]) if len(sys.argv) > 5 else 3891
ONLY = set(sys.argv[6:])
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..'))
SW, SH = 1440, 900
u32 = ctypes.WinDLL('user32', use_last_error=True)
u32.VkKeyScanW.restype = ctypes.c_short
try: ctypes.WinDLL('shcore').SetProcessDpiAwareness(2)
except Exception: u32.SetProcessDPIAware()
rows = []; procs = []
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
for d in ['home', 'proj', 'bin', 'fakehome/.cache', 'fakehome/AppData/Roaming', 'fakehome/AppData/Local', 'shots', 'proj/mech', 'proj/boards']: os.makedirs(os.path.join(OUT, d), exist_ok=True)
P = lambda *a: os.path.join(OUT, *a)
shutil.copy(os.path.join(HERE, 'lib/standin_rows.py'), P('bin/claude'))
for n in ['codex', 'grok', 'muse', 'opencode']:
    open(P('bin', n), 'w', newline='\n').write('#!/bin/sh\nexit 1\n')
shutil.copy(os.path.join(REPO, 'test/files/fixtures/mesh/bracket.step'), P('proj/mech/bracket.step'))
for ext in ('kicad_pcb', 'kicad_sch'): shutil.copy(os.path.join(REPO, 'test/files/fixtures/plot/ecc83-pp.' + ext), P('proj/ecc83.' + ext))
open(P('proj/ecc83.kicad_pro'), 'w').write('{}\n')
open(P('proj/readme.txt'), 'w').write('rig fixture\n')
now = int(time.time())
seed = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'rows', 'root': P('proj'), 'at': now}] + [{'$': 'ThreadCreated', 'id': 't%d' % i, 'project': 'p1', 'title': 'Thread %d' % i, 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + i} for i in (1, 2, 3)]
seed += [{'$': 'SettingSet', 'key': 'buy.t3', 'value': 't0', 'at': now + 3}, {'$': 'ModesSet', 'id': 't1', 'runtime': 'approval-required', 'interaction': 'default', 'model': '', 'at': now + 3}, {'$': 'ModesSet', 'id': 't2', 'runtime': 'full-access', 'interaction': 'plan', 'model': '', 'at': now + 3}]
open(P('home/events.jsonl'), 'w', newline='\n').write(''.join(json.dumps(e) + '\n' for e in seed))
env = dict(os.environ)
env.update(BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', FAKE_LOG=P('fake.log'), FAKE_SECS='600', HOME=P('fakehome'), USERPROFILE=P('fakehome'),
           APPDATA=P('fakehome/AppData/Roaming'), LOCALAPPDATA=P('fakehome/AppData/Local'), XDG_CONFIG_HOME=P('fakehome/.config'), XDG_CACHE_HOME=P('fakehome/.cache'),
           # the stand-ins' #! names python3: this one, first after the stand-ins
           PATH=os.pathsep.join([P('bin'), os.path.dirname(sys.executable), os.environ['PATH']]),
           BACKPLANE_ORDER_SERVER='http://127.0.0.1:%d' % order_srv.server_address[1])
env.pop('SHELL', None)
def spawn(args, log_name, e=None, cwd=None):
    p = subprocess.Popen(args, env=e or env, cwd=cwd, stdout=open(P(log_name), 'ab'), stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW); procs.append(p)
    if args[0] == CLIENT and 'place' in globals():   # a client the rows start again (F09's crash) is placed like the first
        threading.Thread(target=place_when_up, args=(p.pid,), daemon=True).start()
    return p
def cleanup(*_):
    for p in procs:
        try: subprocess.run(['taskkill', '/T', '/F', '/PID', str(p.pid)], capture_output=True)   # its tree: only the pids this driver started
        except Exception: pass
    json.dump({'rows': rows}, open(P('rows.json'), 'w'), indent=1)
def hub_up():
    for _ in range(150):
        if subprocess.run(['curl.exe', '-s', '-m1', '-o', 'NUL', 'http://127.0.0.1:%d/hello' % PORT]).returncode == 0: return True
        time.sleep(0.2)
    return False
hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=P('proj'))
if not hub_up(): cleanup(); sys.exit('the hub did not answer')
cenv = dict(env, BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1')
open(P('script.json'), 'w').write('[{"select":0},{"wait":3000000}]')
client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '3000'], 'client.log', cenv)

# ---- the client's window: found by its pid, its client area made SW x SH at the screen's top left
WNDENUMPROC = ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
def window_of(pid):
    found = []
    def cb(h, _):
        p = W.DWORD(); u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u32.IsWindowVisible(h) and u32.GetWindowTextLengthW(h) > 0:
            r = W.RECT(); u32.GetWindowRect(h, ctypes.byref(r)); found.append(((r.right - r.left) * (r.bottom - r.top), h))
        return True
    # the client has more than one visible, titled window (a small helper beside the main one): the main one is the largest
    u32.EnumWindows(WNDENUMPROC(cb), 0); return max(found)[1] if found else None
HWND = [None]
def place(pid=None):
    h = window_of(pid or client.pid)
    if not h: return False
    HWND[0] = h
    u32.ShowWindow(h, 9)  # SW_RESTORE: a maximised window ignores a size
    r = W.RECT(0, 0, SW, SH); style = u32.GetWindowLongW(h, -16); ex = u32.GetWindowLongW(h, -20)
    u32.AdjustWindowRectEx(ctypes.byref(r), style, False, ex)
    u32.SetWindowPos(h, 0, r.left, r.top, r.right - r.left, r.bottom - r.top, 0x0040)  # SWP_SHOWWINDOW
    u32.SetForegroundWindow(h); return True
def place_when_up(pid):
    for _ in range(40):
        if place(pid): break
        time.sleep(0.5)
    time.sleep(3); place(pid)   # once more after the first frames: a window may size itself as it opens
def origin():
    pt = W.POINT(0, 0); u32.ClientToScreen(HWND[0], ctypes.byref(pt)); return pt.x, pt.y
for _ in range(60):
    if place(): break
    time.sleep(0.5)
time.sleep(6); place()

# ---- input: SendInput, in the client's coordinates
class MI(ctypes.Structure): _fields_ = [('dx', W.LONG), ('dy', W.LONG), ('mouseData', W.DWORD), ('dwFlags', W.DWORD), ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]
class KI(ctypes.Structure): _fields_ = [('wVk', W.WORD), ('wScan', W.WORD), ('dwFlags', W.DWORD), ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]
class HI(ctypes.Structure): _fields_ = [('uMsg', W.DWORD), ('wParamL', W.WORD), ('wParamH', W.WORD)]
class _U(ctypes.Union): _fields_ = [('mi', MI), ('ki', KI), ('hi', HI)]
class INPUT(ctypes.Structure): _fields_ = [('type', W.DWORD), ('u', _U)]
def send(*ins):
    arr = (INPUT * len(ins))(*ins); u32.SendInput(len(ins), arr, ctypes.sizeof(INPUT)); time.sleep(0.03)
def mouse(flags, data=0): i = INPUT(type=0); i.u.mi = MI(0, 0, data & 0xffffffff, flags, 0, 0); return i
def kbd(vk=0, scan=0, flags=0): i = INPUT(type=1); i.u.ki = KI(vk, scan, flags, 0, 0); return i
def move(x, y):
    ox, oy = origin(); u32.SetCursorPos(int(ox + x), int(oy + y)); time.sleep(0.05)
VK = {'Return': 0x0D, 'Escape': 0x1B, 'Tab': 0x09, 'BackSpace': 0x08, 'Delete': 0x2E, 'space': 0x20, 'Up': 0x26, 'Down': 0x28, 'Left': 0x25, 'Right': 0x27,
      'Home': 0x24, 'End': 0x23, 'Page_Up': 0x21, 'Page_Down': 0x22, 'Insert': 0x2D, 'grave': 0xC0, 'comma': 0xBC, 'period': 0xBE, 'slash': 0xBF, 'minus': 0xBD,
      'equal': 0xBB, 'bracketleft': 0xDB, 'bracketright': 0xDD, 'semicolon': 0xBA, 'apostrophe': 0xDE, 'backslash': 0xDC,
      'ctrl': 0x11, 'shift': 0x10, 'alt': 0x12, 'super': 0x5B}
VK.update({'F%d' % n: 0x6F + n for n in range(1, 13)})
EXT = {0x26, 0x28, 0x25, 0x27, 0x24, 0x23, 0x21, 0x22, 0x2D, 0x2E}
def vk_of(name):
    if name in VK: return VK[name]
    if len(name) == 1 and name.isalnum(): return ord(name.upper())
    raise SystemExit('no virtual key for ' + name)
def key_down(v): send(kbd(v, 0, 1 if v in EXT else 0))
def key_up(v): send(kbd(v, 0, (1 if v in EXT else 0) | 2))
def xin(cmd, *a):
    if cmd == 'move': move(a[0], a[1])
    elif cmd == 'click':
        move(a[0], a[1]); b = int(a[2]) if len(a) > 2 else 1; dn, up = {1: (2, 4), 2: (0x20, 0x40), 3: (8, 0x10)}[b]; send(mouse(dn)); time.sleep(0.06); send(mouse(up))
    elif cmd == 'dblclick': xin('click', a[0], a[1]); xin('click', a[0], a[1])
    elif cmd == 'type':
        for ch in str(a[0]):
            # as a keyboard types it: the key this layout has for the character (VkKeyScanW), Shift held when it says so; a character
            # no key makes goes as itself (KEYEVENTF_UNICODE, VK_PACKET), as an on-screen keyboard or an IME sends it
            r = u32.VkKeyScanW(ord(ch)) if ord(ch) < 0x80 else -1
            if r == -1 or r & 0xff == 0xff: send(kbd(0, ord(ch), 4), kbd(0, ord(ch), 4 | 2)); continue
            v, sh = r & 0xff, (r >> 8) & 1
            if sh: key_down(0x10)
            key_down(v); key_up(v)
            if sh: key_up(0x10)
    elif cmd == 'key': v = vk_of(a[0]); key_down(v); key_up(v)
    elif cmd == 'chord':
        held = [vk_of(m) for m in a[:-1]]
        for v in held: key_down(v)
        v = vk_of(a[-1]); key_down(v); key_up(v)
        for v in reversed(held): key_up(v)
    elif cmd == 'drag':
        x0, y0, x1, y1 = [float(v) for v in a[:4]]; move(x0, y0); send(mouse(2)); time.sleep(0.08)
        for i in range(1, 11): move(x0 + (x1 - x0) * i / 10, y0 + (y1 - y0) * i / 10)
        send(mouse(4))
    elif cmd == 'wheel': move(a[0], a[1]); send(mouse(0x0800, 120 if int(a[2]) > 0 else -120))
    time.sleep(0.05)
def shot(name):
    from PIL import ImageGrab
    f = P('shots', name + '.png'); ox, oy = origin(); ImageGrab.grab(bbox=(ox, oy, ox + SW, oy + SH), all_screens=True).save(f); return f
def clip_image(img):
    """the real clipboard holds the PNG, as a person's copy would put it there"""
    subprocess.run(['powershell', '-NoProfile', '-STA', '-Command', "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; [Windows.Forms.Clipboard]::SetImage([Drawing.Image]::FromFile('%s'))" % img], capture_output=True)
def metrics():
    out = []   # a line the client is still writing (or two clients' lines run together after F09's restart) is not one yet
    for l in open(P('client.log'), errors='replace'):
        if 'GPUI_METRIC ' in l:
            try: out.append(json.loads(l.split('GPUI_METRIC ', 1)[1]))
            except ValueError: pass
    return out
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
denv = env
try:
    exec(open(os.path.join(HERE, 'gpui-f-rows.rows.py')).read())
finally:
    cleanup()
sys.exit(0)
