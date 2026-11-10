#!/usr/bin/env python3
"""Opening a long thread paints once, and reading its history never flashes (Nov 10: opening Bub flashed: history pages read for ever, each one
re-laying the rows on screen; the newest rows faded in again; pictures drawn as blank boxes for a few frames).

A hub on a temp home seeded with a long thread (400 turns: tool calls, replies, a picture every fifth reply) and a short one; the GPUI client in a
private Xvfb with its own i3, the screen captured by ffmpeg x11grab at 30 fps. Checks, from the frames and the client's own metrics:
  open    after the long thread is opened from the short one, the timeline changes once (the frame it is drawn in) and then holds still
  pages   no history page is read twice (the client's `history` metrics name each page)
  still   once the reader stops scrolling, pages that land above hold the screen still: no frame changes after the last scroll settles
  blank   no frame shows a picture's box empty (a picture area of the timeline's background colour where the picture was drawn before)
usage: gpui-open-flash.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N]   -> OUT/rows.json, OUT/frames/*.png, one line per check
env: XVFB (default ~/.cache/xvfb-root/usr/bin/Xvfb) XVFB_LIB (its libraries). Needs ffmpeg (x11grab), i3, Python PIL."""
import os, sys, json, time, shutil, subprocess, tempfile, glob, signal
from PIL import Image, ImageChops, ImageStat, ImageDraw
CLIENT, HUB, ASSETS, OUT = [os.path.abspath(a) for a in sys.argv[1:5]]
PORT = int(sys.argv[5]) if len(sys.argv) > 5 else 3893
N = int(sys.argv[6]) if len(sys.argv) > 6 else 79
XV = os.environ.get('XVFB') or os.path.expanduser('~/.cache/xvfb-root/usr/bin/Xvfb')
XLIB = os.environ.get('XVFB_LIB', os.path.expanduser('~/.cache/xvfb-root/usr/lib/x86_64-linux-gnu'))
W, H, FPS = 1440, 900, 30
rows, procs = [], []
P = lambda *a: os.path.join(OUT, *a)
def log(i, ok, ev):
    rows.append({'id': i, 'ok': bool(ok), 'ev': ev}); print(('ok   ' if ok else 'FAIL ') + i + ' ' + ev, flush=True)

shutil.rmtree(OUT, ignore_errors=True)
for d in ('home', 'proj', 'bin', 'fakehome/.cache', 'frames'): os.makedirs(P(d), exist_ok=True)
for n in ('claude', 'codex', 'grok', 'muse', 'opencode', 'agy', 'pi', 'omp'):
    open(P('bin', n), 'w').write('#!/bin/sh\nexit 1\n'); os.chmod(P('bin', n), 0o755)
# pictures the replies name (absolute paths: the hub serves them through /message-image)
for k in range(8):
    im = Image.new('RGB', (900, 560), (40 + 25 * k, 90, 160 - 15 * k)); d = ImageDraw.Draw(im)
    for x in range(0, 900, 60): d.line([(x, 0), (900 - x, 560)], fill=(240, 240, 240), width=3)
    im.save(P('proj', 'pic%d.png' % k))
now = int(time.time()) - 400 * 60
ev = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'flash', 'root': P('proj'), 'at': now},
      {'$': 'ThreadCreated', 'id': 't_long', 'project': 'p1', 'title': 'Long thread', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now},
      {'$': 'SettingSet', 'key': 'onboard.done', 'value': '1', 'at': now}, {'$': 'SettingSet', 'key': 'restart.continue', 'value': 'off', 'at': now}]
for i in range(400):
    at = now + i * 60
    ev.append({'$': 'MessagePosted', 'thread': 't_long', 'msg': 'u%d' % i, 'role': 'user', 'text': 'Question %d: look at the board again and tell me what changed.' % i, 'at': at})
    for j in range(6):
        ev.append({'$': 'ActivityLogged', 'thread': 't_long', 'act': 'a%d-%d' % (i, j), 'tone': 'tool', 'kind': 'shell', 'summary': 'rg -n "net_%d_%d" boards/main --max-count 3' % (i, j), 'at': at + 1})
    text = 'Answer %d. The board has %d nets left to route.\n\n- Placement: unchanged\n- Routing: %d tracks\n\nWhat I checked:\n\n1. DRC\n2. ERC\n3. The BOM' % (i, 400 - i, i * 7)
    if i % 5 == 0: text += '\n\nThe render: %s' % P('proj', 'pic%d.png' % (i // 5 % 8))
    ev.append({'$': 'MessagePosted', 'thread': 't_long', 'msg': 'r%d' % i, 'role': 'assistant', 'text': text, 'at': at + 2})
ev.append({'$': 'ThreadCreated', 'id': 't_short', 'project': 'p1', 'title': 'Short thread', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 400 * 60 + 5})
ev.append({'$': 'MessagePosted', 'thread': 't_short', 'msg': 's1', 'role': 'user', 'text': 'hello', 'at': now + 400 * 60 + 6})
open(P('home/events.jsonl'), 'w').write(''.join(json.dumps(e) + '\n' for e in ev))

RUN = tempfile.mkdtemp(prefix='of.'); os.chmod(RUN, 0o700)
env = dict(os.environ)
for k in ('I3SOCK', 'DBUS_SESSION_BUS_ADDRESS', 'WAYLAND_DISPLAY', 'DISPLAY'): env.pop(k, None)
env.update(XDG_RUNTIME_DIR=RUN, LC_ALL='C', BACKPLANE_AGENTS=P('bin'), BACKPLANE_NO_UPDATE='1', SHELL='/bin/sh', HOME=P('fakehome'), XDG_CACHE_HOME=P('fakehome/.cache'),
           XDG_CONFIG_HOME=P('fakehome/.config'), XDG_DATA_HOME=P('fakehome/.local/share'), PATH=P('bin') + ':' + os.environ['PATH'])
def spawn(args, name, e=None):
    p = subprocess.Popen(args, env=e or env, stdout=open(P(name), 'wb'), stderr=subprocess.STDOUT, start_new_session=True); procs.append(p); return p
def cleanup(*_):
    for p in reversed(procs):
        try: os.killpg(p.pid, signal.SIGTERM)
        except Exception: pass
    shutil.rmtree(RUN, ignore_errors=True)
    json.dump({'rows': rows}, open(P('rows.json'), 'w'), indent=1)
signal.signal(signal.SIGTERM, lambda *a: (cleanup(), sys.exit(1)))
def metrics(): return [json.loads(l.split('GPUI_METRIC ', 1)[1]) for l in open(P('client.log'), errors='replace') if 'GPUI_METRIC ' in l]

try:
    if os.path.exists('/tmp/.X11-unix/X%d' % N): sys.exit('display :%d is in use' % N)
    spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log')
    import urllib.request
    for _ in range(240):
        try: urllib.request.urlopen('http://127.0.0.1:%d/hello' % PORT, timeout=1); break
        except Exception: time.sleep(0.5)
    spawn([XV, ':%d' % N, '-screen', '0', '%dx%dx24' % (W, H), '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log', dict(env, LD_LIBRARY_PATH=XLIB)); time.sleep(2)
    open(P('i3.conf'), 'w').write('font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' % RUN)
    denv = dict(env, DISPLAY=':%d' % N); spawn(['i3', '-c', P('i3.conf')], 'i3.log', denv); time.sleep(1)
    cenv = dict(denv, VK_ICD_FILENAMES=(sorted(glob.glob('/usr/share/vulkan/icd.d/lvp_icd*.json')) or ['/usr/share/vulkan/icd.d/lvp_icd.json'])[0], LP_NUM_THREADS='4', BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_RECTS='1')
    # the short thread first, then the long one opened from it; then the reader scrolls up into its history and stops
    steps = [{'wait': 5000}, {'select': 0}, {'wait': 2500}, {'select': 1}, {'wait': 2500}, {'select': 0}, {'wait': 2500}, {'mark': 'open'}, {'select': 1}, {'wait': 3000},
             {'mark': 'up'}] + [{'scroll': -900}, {'wait': 120}] * 25 + [{'mark': 'stop'}, {'wait': 4000}, {'mark': 'end'}]
    json.dump(steps, open(P('script.json'), 'w'))
    t_rec = time.time()
    rec = spawn(['ffmpeg', '-v', 'error', '-y', '-f', 'x11grab', '-framerate', str(FPS), '-video_size', '%dx%d' % (W, H), '-i', ':%d' % N, '-t', '40', '-vf', 'scale=%d:-1' % (W // 2), P('frames/f_%04d.png')], 'ffmpeg.log', denv)
    client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '1000'], 'client.log', cenv)
    t_client = time.time()
    client.wait(timeout=120)
    try: os.killpg(rec.pid, signal.SIGINT); rec.wait(timeout=20)
    except Exception: pass
finally:
    cleanup()

ms = metrics()
start = next((m['epoch_ms'] for m in ms if m.get('event') == 'process_start'), None)
marks = {m['name']: m['epoch_ms'] for m in ms if m.get('event') == 'mark'}
frames = sorted(glob.glob(P('frames/f_*.png')))
# frame k was grabbed about (t_rec + k/FPS); the client's clock is epoch ms. ffmpeg starts within a few frames of t_rec: the checks allow for that.
def frame_at(epoch_ms): return max(0, int(round((epoch_ms / 1000.0 - t_rec) * FPS)))
# the timeline: right of the sidebar, above the composer, left of the right rail (half-size frames). The caret and the "Working" dots are outside it.
BOX = (int(W * 0.10), int(H * 0.06), int(W * 0.47), int(H * 0.42))
def tl(i): return Image.open(frames[i]).convert('L').crop(BOX)
def diff(a, b): return ImageStat.Stat(ImageChops.difference(a, b)).mean[0]
need = [k for k in ('open', 'up', 'stop', 'end') if k not in marks]
if need or not frames: log('rig', False, 'missing marks %s or no frames (%d)' % (need, len(frames)))
else:
    # ffmpeg's first frame comes a little after t_rec: start a few frames early (the short thread holds still before the open)
    a, b = max(1, frame_at(marks['open']) - 6), min(len(frames) - 1, frame_at(marks['up']) - 3)
    seq = [tl(i) for i in range(a, b)]
    changes = [a + i + 1 for i in range(len(seq) - 1) if diff(seq[i], seq[i + 1]) > 0.4]
    # the first change is the long thread drawn; nothing after it within the 3 s the reader looks at it
    log('open', len(changes) >= 1 and all(c - changes[0] <= 1 for c in changes), 'timeline changes after open at frames %s (want one, or two adjacent: the frame boundary)' % changes[:12])
    pages = [m.get('page') for m in ms if m.get('event') == 'history']
    twice = sorted({p for p in pages if p is not None and pages.count(p) > 1})
    log('pages', len(pages) > 0 and not twice, '%d pages read, read twice: %s' % (len(pages), twice[:8]))
    s, e = frame_at(marks['stop']) + int(FPS * 0.7), min(len(frames) - 1, frame_at(marks['end']))
    seq = [tl(i) for i in range(s, e)]
    moved = [s + i + 1 for i in range(len(seq) - 1) if diff(seq[i], seq[i + 1]) > 0.4]
    log('still', len(seq) > 10 and not moved, 'frames changed after scrolling stopped: %s (pages landing above must not move the screen)' % moved[:12])
    # a blank picture box: a region that was a picture (many colours) in frame i-1 and is a flat rectangle in frame i while the rest did not move
    blank = []
    for i in range(max(1, frame_at(marks['open']) - 6), e):
        x, y = tl(i - 1), tl(i)
        if diff(x, y) < 0.4: continue
        flat = ImageStat.Stat(y).stddev[0] < 4 and ImageStat.Stat(x).stddev[0] > 20
        if flat: blank.append(i)
    log('blank', not blank, 'frames where the timeline went flat: %s' % blank[:12])
cleanup()
sys.exit(0 if rows and all(r['ok'] for r in rows) else 1)
