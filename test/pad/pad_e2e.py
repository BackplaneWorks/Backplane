#!/usr/bin/env python3
"""The driver of pad_e2e.sh (run inside its private session bus). Real X input through XTest; the proof is the image the stand-in agent received.
Picture checks read the PNG out of the agent's image block. Screenshots go in OUT."""
import json, os, re, signal, socket, struct, subprocess, sys, time, zlib
CLIENT, HUB, OUT, DISP, XV, ASSETS, ROOT = sys.argv[1:8]
XIN = [sys.executable, '-I', ROOT + '/test/cef/xinput.py']
procs, fails, passes = [], [], []
def check(ok, what, got=''):
    print(('PASS ' if ok else 'FAIL ') + what + ('' if ok else f' ({got})'), flush=True)
    (passes if ok else fails).append(what)
def spawn(args, log, **kw):
    p = subprocess.Popen(args, stdout=open(f'{OUT}/{log}', 'wb'), stderr=subprocess.STDOUT, **kw); procs.append(p); return p
def stop():
    for p in reversed(procs):
        if p.poll() is None: p.terminate()
    for p in procs:
        try: p.wait(5)
        except subprocess.TimeoutExpired: p.kill()
signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
def free_port():
    s = socket.socket(); s.bind(('127.0.0.1', 0)); p = s.getsockname()[1]; s.close(); return max(p, 3799)
def X(*a): subprocess.run(XIN + [str(x) for x in a], check=True)
def shot(name):
    subprocess.run(['import', '-window', 'root', f'{OUT}/{name}.png'], stderr=subprocess.DEVNULL); return f'{OUT}/{name}.png'
def metrics(name):
    out = []
    for line in open(f'{OUT}/client.log', errors='replace'):
        if line.startswith('GPUI_METRIC '):
            try: m = json.loads(line[12:])
            except ValueError: continue
            if m.get('event') == name: out.append(m)
    return out
def controls():
    c = metrics('controls')
    return {x['id']: x for x in c[-1]['controls']} if c else {}
def wait(what, f, secs=30):
    t = time.time()
    while time.time() - t < secs:
        v = f()
        if v: return v
        X('move', 3 + int(time.time() * 10) % 5, 3); time.sleep(0.15) # a move makes a frame, and a frame reports the rectangles
    shot('timeout'); raise SystemExit(f'FAIL timed out: {what}')
def rect(id):
    def f():
        r = controls().get(id, {}).get('rect')
        return r if r and r['w'] > 0 else None
    return wait(id, f)
def press(id): r = rect(id); X('click', int(r['x'] + r['w'] / 2), int(r['y'] + r['h'] / 2)); time.sleep(0.25)
def pad_open(): return 'pad:paper' in controls()
def paper(x, y): r = rect('pad:paper'); k = r['w'] / 960; return int(r['x'] + x * k), int(r['y'] + y * k)
def drag(a, b): X('drag', *paper(*a), *paper(*b)); time.sleep(0.2)
def click(p): X('click', *paper(*p)); time.sleep(0.25)
def chord(*k): X('chord', *k); time.sleep(0.25)
def key(k): X('key', k); time.sleep(0.25)
def closed(what):
    wait(what, lambda: not pad_open(), 8)

def png_rgb(path):
    """The pixels of an 8-bit RGB or RGBA PNG."""
    b = open(path, 'rb').read(); assert b[:8] == b'\x89PNG\r\n\x1a\n', 'not a PNG'
    i, idat = 8, b''
    while i < len(b):
        n, t = struct.unpack('>I4s', b[i:i + 8]); d = b[i + 8:i + 8 + n]; i += 12 + n
        if t == b'IHDR': w, h, depth, ct = struct.unpack('>IIBB', d[:10])
        if t == b'IDAT': idat += d
    assert depth == 8 and ct in (2, 6), (depth, ct)
    bpp = 3 if ct == 2 else 4; raw = zlib.decompress(idat); stride = w * bpp; rows, prev, o = [], bytearray(stride), 0
    for _ in range(h):
        f = raw[o]; line = bytearray(raw[o + 1:o + 1 + stride]); o += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0; u = prev[x]; c = prev[x - bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + u) & 255
            elif f == 3: line[x] = (line[x] + (a + u) // 2) & 255
            elif f == 4:
                p = a + u - c; pa, pb, pc = abs(p - a), abs(p - u), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else u if pb <= pc else c)) & 255
        rows.append(line); prev = line
    return w, h, lambda x, y: tuple(rows[y][x * bpp:x * bpp + 3])

try:
    env = dict(os.environ)
    spawn([XV + '/usr/bin/Xvfb', DISP, '-screen', '0', os.environ.get('PAD_SCREEN', '1400x900') + 'x24', '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log', env={**env, 'LD_LIBRARY_PATH': XV + '/usr/lib/x86_64-linux-gnu'})
    time.sleep(1.5)
    open(f'{OUT}/i3.conf', 'w').write(f'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket {OUT}/i3.sock\n')
    spawn(['i3', '-c', f'{OUT}/i3.conf'], 'i3.log'); time.sleep(1)
    now = int(time.time()); port = free_port()
    ev = [{'$': 'SettingSet', 'key': 'restart.continue', 'value': 'off', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.done', 'value': '1', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.uber', 'value': '1', 'at': now},
          {'$': 'ProjectCreated', 'id': 'p1', 'title': 'pad', 'root': OUT + '/proj', 'at': now},
          {'$': 'ThreadCreated', 'id': 't1', 'project': 'p1', 'title': 'Sketch', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now}]
    open(f'{OUT}/home/events.jsonl', 'w').write(''.join(json.dumps(e, separators=(',', ':')) + '\n' for e in ev))
    hub = spawn([HUB, '--home', OUT + '/home', '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=OUT + '/proj',
                env={**env, 'BACKPLANE_AGENTS': OUT + '/bin', 'BACKPLANE_NO_UPDATE': '1', 'BACKPLANE_KISTACK_SYNC': '0', 'BACKPLANE_ORDER_SERVER': 'http://127.0.0.1:9', 'FAKE_LOG': OUT + '/standin.log', 'BP_STANDIN_MODE': 'plain', 'BP_STANDIN_IMAGES': OUT + '/images'})
    t = time.time()
    while not os.path.exists(OUT + '/home/token'):
        if time.time() - t > 30: raise SystemExit('FAIL the hub did not start')
        time.sleep(0.2)
    environ = open(f'/proc/{hub.pid}/environ', 'rb').read().split(b'\0')
    if (b'BACKPLANE_AGENTS=' + (OUT + '/bin').encode()) not in environ: raise SystemExit('FAIL the hub has no stand-in folder')
    json.dump([{'select': 0}, {'wait': 900000}], open(OUT + '/script.json', 'w'))
    spawn([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', OUT + '/home/token', '--script', OUT + '/script.json', '--exit-after', '400'], 'client.log',
          env={**env, 'BACKPLANE_GPUI_RECTS': '1', 'BACKPLANE_GPUI_ASSETS': ASSETS, 'VK_ICD_FILENAMES': '/usr/share/vulkan/icd.d/lvp_icd.json', 'LP_NUM_THREADS': '2', 'LIBGL_ALWAYS_SOFTWARE': '1', 'GALLIUM_DRIVER': 'llvmpipe', 'LD_LIBRARY_PATH': ''})
    wait('the thread', lambda: 'ui:Draw' in controls() and metrics('usable'), 40); time.sleep(2)
    g = metrics('gpu'); print('gpu:', g[-1] if g else None, flush=True)
    check(controls()['ui:Draw']['enabled'], 'the composer has a Draw button')

    # the message box keeps its words through everything the pad does
    X('type', 'keep this text'); time.sleep(0.4)
    press('ui:Draw'); wait('the pad', pad_open, 8)
    check(True, 'the Draw button opens the pad'); shot('1-open')
    press('pad:attach'); check(pad_open(), 'Attach does nothing on an empty pad')

    # pen, undo
    drag((100, 100), (500, 100)); drag((100, 200), (500, 200)); shot('2-pen')
    chord('Control_L', 'z')                                    # takes back the second stroke only
    press('pad:color:3'); drag((100, 300), (500, 300))          # a chosen colour
    # eraser: a gap in the first stroke
    press('pad:eraser'); drag((300, 60), (300, 140))
    # boxes: one to keep, one deleted by Delete, one by Backspace
    press('pad:box'); press('pad:color:2')
    drag((600, 100), (800, 200)); drag((600, 300), (800, 400)); key('Delete')
    drag((600, 420), (800, 500)); key('BackSpace'); shot('3-boxes')
    # a length on the kept box's right side; Esc in the box puts away only the box
    click((800, 150)); rect('pad:length'); X('chord', 'Control_L', 'a'); X('type', '30 mm'); key('Return')
    click((720, 100)); rect('pad:length'); X('type', '9'); key('Escape'); check(pad_open(), 'Esc in the length box leaves the pad open')
    # keys behind the pad do nothing
    chord('Control_L', 'n'); key('b'); time.sleep(0.5)
    check(sum('"ThreadCreated"' in l for l in open(OUT + '/home/events.jsonl')) == 1, 'Ctrl+N under the pad makes no thread')
    check(pad_open(), 'a bare key leaves the pad as it is')
    shot('4-final'); press('pad:attach'); closed('Attach closes the pad'); check(True, 'Attach closes the pad')
    t = time.time()
    while time.time() - t < 15:
        names = os.listdir(OUT + '/home/attachments/t1') if os.path.isdir(OUT + '/home/attachments/t1') else []
        if names: break
        time.sleep(0.2)
    check(len(names) == 1 and re.search(r'drawing-\d{8}-\d{6}\.png$', names[0]) is not None, 'Attach uploads drawing-<stamp>.png through /attach', names)
    time.sleep(1); shot('5-chip')

    # a second pad: Delete with nothing selected clears, Ctrl+Z brings it back, Esc drops it all
    press('ui:Draw'); wait('the second pad', pad_open, 8)
    drag((100, 100), (500, 100)); key('Delete'); press('pad:attach'); check(pad_open(), 'Delete with nothing selected clears the pad (Attach has nothing)')
    chord('Control_L', 'z'); shot('6-restored'); key('Escape'); closed('Esc closes the pad'); check(True, 'Esc closes the pad and drops its drawing')
    time.sleep(0.8)
    check(len(os.listdir(OUT + '/home/attachments/t1')) == 1, 'a pad closed with Esc attaches nothing')

    # Cancel and a press outside drop it too
    press('ui:Draw'); wait('the third pad', pad_open, 8); drag((100, 100), (500, 100)); press('pad:cancel'); closed('Cancel closes the pad'); check(True, 'Cancel closes the pad')
    press('ui:Draw'); wait('the fourth pad', pad_open, 8); X('click', 3, 3); time.sleep(0.3); closed('a press outside closes the pad'); check(True, 'a press outside closes the pad')
    time.sleep(0.5)
    # the keyboard came back: more text, and here Ctrl+Z is the message box's own
    X('type', ' zzz'); time.sleep(0.3); chord('Control_L', 'z'); time.sleep(0.3)
    press('send')
    t = time.time(); log = ''
    while time.time() - t < 20 and 'IMAGE ' not in log:
        log = open(OUT + '/standin.log', errors='replace').read() if os.path.exists(OUT + '/standin.log') else ''; time.sleep(0.2)
    m = re.search(r'IMAGE (\S+) (\S+) (\S+)\n.*?TEXT (.*)', log)
    check(m is not None, 'the agent receives an image block', log[-300:])
    if not m: raise SystemExit(1)
    check((m[2], m[3]) == ('image/png', 'base64'), 'the block is a base64 PNG', m.groups())
    text = json.loads(m[4])
    # the stand-in's join puts a space for the image block; the box's Ctrl+Z takes back one character, so one z less proves it is the box's, and that the pad's never reached it
    check(text.strip().split('\n')[0] == 'keep this text zz', 'Ctrl+Z, Delete and Backspace in the pad left the message; in the box Ctrl+Z undid the typing', text[:120])
    w, h, px = png_rgb(m[1]); WHITE = (255, 255, 255); RED = (0xe5, 0x48, 0x4d); BLUE = (0x2f, 0x80, 0xed); GREEN = (0x3e, 0x9b, 0x4f)
    check((w, h) == (960, 600), 'the picture is 960x600', (w, h))
    near = lambda x, y, c: any(px(x, y + d) == c for d in range(-3, 4)) # a window pixel is more than a paper pixel in a small window
    check(near(200, 100, RED) and near(400, 100, RED), 'the pen drew the first stroke in the first colour', px(200, 100))
    check(not near(300, 100, RED), 'the eraser cut it', px(300, 100))
    check(not near(300, 200, RED), 'Ctrl+Z took back one stroke', px(300, 200))
    check(near(300, 300, BLUE), 'a chosen colour draws', px(300, 300))
    check(px(601, 150) == GREEN, 'the box is there in its colour', px(601, 150))
    check(px(601, 350) == WHITE, 'Delete removed the picked box', px(601, 350))
    check(px(601, 460) == WHITE, 'Backspace removed the picked box', px(601, 460))
    check(px(718, 150) == GREEN and px(790, 150) == WHITE, 'a typed length set the width (30 mm is 120 px)', (px(718, 150), px(790, 150)))
    check(near(660, 199, GREEN), 'Esc in the length box changed nothing', px(660, 199))
    check(any(px(x, y) != WHITE for x in range(635, 685) for y in range(78, 94)), 'the typed length is written on the picture')
    check(px(597, 96) == WHITE, 'the attachment carries no selection marks', px(597, 96))
    time.sleep(1); shot('7-sent')
    if 'ui:Settings' in controls():
        press('ui:Settings'); time.sleep(1)
        for _ in range(int(os.environ.get('PAD_SETTINGS_WHEEL', '30'))): X('wheel', 900, 500, -1)
        time.sleep(0.6); shot('8-settings') # the shortcut rows
    s = png_rgb(f'{OUT}/1-open.png')
    lit = sum(s[2](x, y) != (0, 0, 0) for x in range(0, s[0], 37) for y in range(0, s[1], 37))
    check(lit > 50, 'the screenshots show the window', lit)
    real = [l for l in open(OUT + '/hub.log', errors='replace') if 'api.anthropic' in l]
    check(not real, 'no real model was called', real[:1])
finally:
    stop()
print(f'{len(passes)} passed, {len(fails)} failed', flush=True)
sys.exit(1 if fails else 0)
