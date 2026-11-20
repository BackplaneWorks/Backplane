#!/usr/bin/env python3
"""The driver of copy_e2e.sh (run inside its private session bus and display). Real X input through XTest; each check reads the X CLIPBOARD or PRIMARY
with xclip, another X client, as a person's paste in another program would. Screenshots and logs go in OUT."""
import json, os, signal, socket, subprocess, sys, time
CLIENT, HUB, OUT, DISP, XV, ASSETS, ROOT = sys.argv[1:8]
XIN = [sys.executable, '-I', ROOT + '/test/cef/xinput.py']
procs, fails, passes = [], [], []
def check(ok, what, got=''):
    print(('PASS ' if ok else 'FAIL ') + what + ('' if ok else f' ({got!r})'), flush=True)
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
def shot(name): subprocess.run(['import', '-window', 'root', f'{OUT}/{name}.png'], stderr=subprocess.DEVNULL)
def metrics(name):
    out = []
    for line in open(f'{OUT}/client.log', errors='replace'):
        if line.startswith('GPUI_METRIC '):
            try: m = json.loads(line[12:])
            except ValueError: continue
            if m.get('event') == name: out.append(m)
    return out
def last(name, key):
    m = metrics(name); return m[-1].get(key, []) if m else []
def wait(what, f, secs=30):
    t = time.time()
    while time.time() - t < secs:
        v = f()
        if v: return v
        X('move', 3 + int(time.time() * 10) % 5, 3); time.sleep(0.15)  # a move makes a frame, and a frame reports the rectangles
    shot('timeout-' + what.replace(' ', '-')); raise SystemExit(f'FAIL timed out: {what}')
def block(prefix):
    """The rendered text block starting with prefix, as the last frame painted it: {text, rect}."""
    return wait(f'a block "{prefix}"', lambda: next((b for b in last('links', 'blocks') if b['text'].startswith(prefix) and b['rect']['w'] > 0), None))
def probe(prefix):
    return wait(prefix, lambda: next((p for p in last('probes', 'rects') if p['id'].startswith(prefix) and p['w'] > 0), None))
def clip(sel='clipboard'):
    r = subprocess.run(['xclip', '-o', '-selection', sel], capture_output=True, timeout=5)
    return r.stdout.decode(errors='replace') if r.returncode == 0 else None
def set_clip(text):
    """Another program owns the CLIPBOARD with this text (xclip in the foreground: it ends when a copy takes the selection from it)."""
    p = subprocess.Popen(['xclip', '-i', '-quiet', '-selection', 'clipboard'], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    procs.append(p); p.stdin.write(text.encode()); p.stdin.close(); time.sleep(0.4)
def chord(*k): X('chord', *k); time.sleep(0.4)
def key(k): X('key', k); time.sleep(0.3)
def word_at(b, dx=6): r = b['rect']; return int(r['x'] + dx), int(r['y'] + 8)

REPLY = 'Alpha bravo charlie delta echo foxtrot.'
DRAFT = 'copy is not working for me rn.'
try:
    env = dict(os.environ)
    spawn([XV + '/usr/bin/Xvfb', DISP, '-screen', '0', '1400x900x24', '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log')
    time.sleep(1.5)
    open(f'{OUT}/i3.conf', 'w').write(f'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket {OUT}/i3.sock\n')
    spawn(['i3', '-c', f'{OUT}/i3.conf'], 'i3.log'); time.sleep(1)
    now = int(time.time()); port = free_port()
    ev = [{'$': 'SettingSet', 'key': 'restart.continue', 'value': 'off', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.done', 'value': '1', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.uber', 'value': '1', 'at': now},
          {'$': 'ProjectCreated', 'id': 'p1', 'title': 'copy', 'root': OUT + '/proj', 'at': now},
          {'$': 'ThreadCreated', 'id': 't1', 'project': 'p1', 'title': 'Copy', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now}]
    open(f'{OUT}/home/events.jsonl', 'w').write(''.join(json.dumps(e, separators=(',', ':')) + '\n' for e in ev))
    hub = spawn([HUB, '--home', OUT + '/home', '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=OUT + '/proj',
                env={**env, 'BACKPLANE_AGENTS': OUT + '/bin', 'BACKPLANE_NO_UPDATE': '1', 'BACKPLANE_KISTACK_SYNC': '0', 'BACKPLANE_ORDER_SERVER': 'http://127.0.0.1:9', 'FAKE_LOG': OUT + '/standin.log', 'BP_STANDIN_MODE': 'plain'})
    t = time.time()
    while not os.path.exists(OUT + '/home/token'):
        if time.time() - t > 30: raise SystemExit('FAIL the hub did not start')
        time.sleep(0.2)
    if (b'BACKPLANE_AGENTS=' + (OUT + '/bin').encode()) not in open(f'/proc/{hub.pid}/environ', 'rb').read().split(b'\0'): raise SystemExit('FAIL the hub has no stand-in folder')
    json.dump([{'select': 0}, {'wait': 900000}], open(OUT + '/script.json', 'w'))
    spawn([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', OUT + '/home/token', '--script', OUT + '/script.json', '--exit-after', '400'], 'client.log',
          env={**env, 'BACKPLANE_GPUI_RECTS': '1', 'BACKPLANE_GPUI_ASSETS': ASSETS, 'VK_ICD_FILENAMES': '/usr/share/vulkan/icd.d/lvp_icd.json', 'LP_NUM_THREADS': '2', 'LIBGL_ALWAYS_SOFTWARE': '1', 'GALLIUM_DRIVER': 'llvmpipe', 'LD_LIBRARY_PATH': ''})
    wait('the thread', lambda: metrics('usable'), 40); time.sleep(2)
    g = metrics('gpu'); print('gpu:', g[-1] if g else None, flush=True)

    # a reply to select in: the stand-in answers the text between echo: and :echo as it is
    X('type', f'echo:{REPLY}:echo'); time.sleep(0.3); key('Return')
    b = block(REPLY[:12]); time.sleep(1); shot('1-reply')

    # 1. the composer's own copy (the person's last good copy): a draft, selected, Ctrl+C
    X('type', DRAFT); time.sleep(0.3); chord('ctrl', 'a'); chord('ctrl', 'c')
    check(clip() == DRAFT, 'Ctrl+C in the message box copies its selection', clip())

    # 2. the draft stays selected in the composer; a word double-clicked in the reply is what Ctrl+C copies now
    b = block(REPLY[:12]); X('dblclick', *word_at(b)); time.sleep(0.5)
    check(clip('primary') == 'Alpha', 'a double-clicked word in a reply is the PRIMARY selection', clip('primary'))
    chord('ctrl', 'c'); shot('2-word')
    check(clip() == 'Alpha', 'Ctrl+C copies the reply selection, not the message box\'s older one', clip())
    # the keyboard stayed in the message box: typing goes on there (Ctrl+A, Ctrl+C copy the draft and what was typed)
    X('type', ' ok'); time.sleep(0.3); chord('ctrl', 'a'); chord('ctrl', 'c')
    check(clip() == DRAFT + ' ok', 'the message box kept the keyboard and its text', clip())
    chord('ctrl', 'a'); key('BackSpace')

    # 3. with the keyboard in the find box (Ctrl+F), a triple-clicked paragraph copies with Ctrl+C
    chord('ctrl', 'f'); time.sleep(0.5); set_clip('stale')
    b = block(REPLY[:12]); X('tripleclick', *word_at(b, 40)); time.sleep(0.5); chord('ctrl', 'c'); shot('3-para')
    check((clip() or '').strip() == REPLY, 'Ctrl+C copies a triple-clicked paragraph with the keyboard in the find box', clip())
    key('Escape'); key('Escape')

    # 4. a drag across the paragraph, the keyboard nowhere in particular
    set_clip('stale'); b = block(REPLY[:12]); r = b['rect']
    X('drag', int(r['x'] + 1), int(r['y'] + 8), int(r['x'] + 1) + 60, int(r['y'] + 8)); time.sleep(0.4)
    want = clip('primary') or ''
    chord('ctrl', 'c')
    check(len(want) >= 3 and REPLY.startswith(want) and clip() == want, 'Ctrl+C copies a dragged selection', (want, clip()))

    # 5. right-click on a selection: its menu's Copy
    set_clip('stale'); b = block(REPLY[:12]); X('dblclick', *word_at(b)); time.sleep(0.4)
    X('click', *word_at(b), 3); time.sleep(0.6); p = probe('ctx:Copy'); shot('5-menu')
    X('click', int(p['x'] + p['w'] / 2), int(p['y'] + p['h'] / 2)); time.sleep(0.5)
    check(clip() == 'Alpha', 'right-click › Copy copies the selection', clip())

    # 6. the reply's Copy (the hover copy under a message) writes the whole message to the CLIPBOARD
    set_clip('stale'); X('move', *word_at(block(REPLY[:12]))); time.sleep(0.5)
    top = block(REPLY[:12])['rect']['y']  # the reply's Copy: the first message Copy under the reply's text
    p = wait("the reply's Copy", lambda: min((p for p in last('probes', 'rects') if p['id'].startswith('msgcopy:') and p['w'] > 0 and p['y'] > top), key=lambda p: p['y'], default=None)); X('click', int(p['x'] + p['w'] / 2), int(p['y'] + p['h'] / 2)); time.sleep(0.5)
    check((clip() or '').strip() == REPLY, "a message's Copy copies it", clip())
    # and with that copy button just pressed, a selection's Ctrl+C is still the selection's
    b = block(REPLY[:12]); X('dblclick', *word_at(b)); time.sleep(0.4); chord('ctrl', 'c')
    check(clip() == 'Alpha', 'after a Copy button, Ctrl+C copies the new selection', clip())

    # 7. the copy outlives the window's focus: another window takes the keyboard, the CLIPBOARD still holds it
    other = spawn(['xmessage', '-name', 'other', 'another program'], 'xmessage.log'); time.sleep(1.2); shot('7-other')
    check(clip() == 'Alpha' and clip('primary') == 'Alpha', 'the copy survives the window losing focus', (clip(), clip('primary')))
    other.terminate(); time.sleep(0.5)

    real = [l for l in open(OUT + '/hub.log', errors='replace') if 'api.anthropic' in l]
    check(not real, 'no real model was called', real[:1])
finally:
    stop()
print(f'{len(passes)} passed, {len(fails)} failed', flush=True)
sys.exit(1 if fails else 0)
