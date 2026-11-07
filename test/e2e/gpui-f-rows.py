#!/usr/bin/env python3
"""Client-side rows of F05-F22 on Linux: the GPUI client in a private Xvfb with its own i3, a hub on a temp home with the stand-in `claude` (lib/standin_rows.py),
real X input through XTest (test/cef/xinput.py), a real clipboard (xclip). A check passes from what the hub logged, what the client reported (GPUI_METRIC lines) or pixels
on a screenshot; never from the driver's own say-so. Frame budgets are not judged here: llvmpipe is software rendering (see docs/e2e/MATRIX.md).
usage: gpui-f-rows.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N] [rows...]   -> OUT/rows.json, OUT/shots/*.png, one line per check
env: XVFB (default ~/.cache/xvfb-root/usr/bin/Xvfb) XVFB_LIB (its libraries)"""
import os, sys, json, time, shutil, subprocess, tempfile, glob, re, signal
CLIENT, HUB, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:5]]
PORT = int(sys.argv[5]) if len(sys.argv) > 5 else 3891
N = int(sys.argv[6]) if len(sys.argv) > 6 else 77
ONLY = set(sys.argv[7:])
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..'))
XV = os.environ.get('XVFB', os.path.expanduser('~/.cache/xvfb-root/usr/bin/Xvfb')); XLIB = os.environ.get('XVFB_LIB', os.path.expanduser('~/.cache/xvfb-root/usr/lib/x86_64-linux-gnu'))
XIN = os.path.join(REPO, 'test/cef/xinput.py'); DISP = ':%d' % N
rows = []; procs = []
def log(i, ok, ev):
    rows.append({'id': i, 'ok': bool(ok), 'ev': ev}); print(('ok   ' if ok else 'FAIL ') + i + ' ' + ev, flush=True)
shutil.rmtree(OUT, ignore_errors=True)
for d in ['home', 'proj', 'bin', 'fakehome/.cache', 'shots', 'proj/mech', 'proj/boards']: os.makedirs(os.path.join(OUT, d), exist_ok=True)
RUN = tempfile.mkdtemp(prefix='gr.'); os.chmod(RUN, 0o700)   # short: an i3 socket path over 107 bytes never draws
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
seed = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'rows', 'root': P('proj'), 'at': now}] + [{'$': 'ThreadCreated', 'id': 't%d' % i, 'project': 'p1', 'title': 'Thread %d' % i, 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + i} for i in (1, 2)]
seed += [{'$': 'ModesSet', 'id': 't1', 'runtime': 'approval-required', 'interaction': 'default', 'model': '', 'at': now + 3}, {'$': 'ModesSet', 'id': 't2', 'runtime': 'full-access', 'interaction': 'plan', 'model': '', 'at': now + 3}]
open(P('home/events.jsonl'), 'w').write(''.join(json.dumps(e) + '\n' for e in seed))
env = dict(os.environ)
for k in ['I3SOCK', 'DBUS_SESSION_BUS_ADDRESS', 'WAYLAND_DISPLAY']: env.pop(k, None)
env.update(XDG_RUNTIME_DIR=RUN, LC_ALL='C', BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', SHELL='/bin/sh', FAKE_LOG=P('fake.log'), FAKE_SECS='600', HOME=P('fakehome'), XDG_CACHE_HOME=P('fakehome/.cache'),
           XDG_CONFIG_HOME=P('fakehome/.config'), XDG_DATA_HOME=P('fakehome/.local/share'), PATH=P('bin') + ':' + os.environ['PATH'])
def spawn(args, log_name, e=None, cwd=None):
    p = subprocess.Popen(args, env=e or env, cwd=cwd, stdout=open(P(log_name), 'ab'), stderr=subprocess.STDOUT); procs.append(p); return p
def cleanup(*_):
    for p in procs:
        try: p.terminate()
        except Exception: pass
    shutil.rmtree(RUN, ignore_errors=True)
    json.dump({'rows': rows}, open(P('rows.json'), 'w'), indent=1)
signal.signal(signal.SIGTERM, lambda *a: (cleanup(), sys.exit(1)))
if os.path.exists('/tmp/.X11-unix/X%d' % N): sys.exit('display %s is in use' % DISP)
hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=P('proj'))
for _ in range(100):
    if subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', 'http://127.0.0.1:%d/hello' % PORT]).returncode == 0: break
    time.sleep(0.2)
assert ('BACKPLANE_AGENTS=' + P('bin')) in open('/proc/%d/environ' % hub.pid).read().replace('\0', '\n'), 'hub is not running stand-in agents only'
xenv = dict(env, LD_LIBRARY_PATH=XLIB)
spawn([XV, DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log', xenv); time.sleep(2)
open(P('i3.conf'), 'w').write('font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' % RUN)
denv = dict(env, DISPLAY=DISP); spawn(['i3', '-c', P('i3.conf')], 'i3.log', denv); time.sleep(1)
cenv = dict(denv, VK_ICD_FILENAMES='/usr/share/vulkan/icd.d/lvp_icd.json', LP_NUM_THREADS='2', BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1')
open(P('script.json'), 'w').write('[{"select":0},{"wait":3000000}]')
client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '3000'], 'client.log', cenv)
time.sleep(6)
# ---- helpers
def xin(*a): subprocess.run(['python3', XIN, *map(str, a)], env=denv, check=False, timeout=30)
def shot(name):
    f = P('shots', name + '.png'); subprocess.run(['import', '-window', 'root', f], env=denv); return f
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
exec(open(os.path.join(HERE, 'gpui-f-rows.rows.py')).read())
cleanup()
sys.exit(0)
