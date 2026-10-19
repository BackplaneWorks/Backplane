"""Actual installed native UI, seeded demo log, private Xvfb and app home.

python3 capture-features.py
Only loopback MCP terminal_open/read are used. No agent turns or messages.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request
from PIL import Image

OUT = Path(os.environ.get('README_CAPTURE_OUT', str(Path(__file__).resolve().parents[1])))
APP = Path(os.environ.get('README_CAPTURE_APP', '/home/h/.local/share/backplane/backplane'))
XVFB = '/home/h/.local/xvfb-root/usr/bin/Xvfb'
XPOKE = '/home/h/Documents/i2cjak/bp-review/ship4/bin/xpoke'
W, H = 1600, 1000


class Key(C.Structure):
    _fields_ = [('type', C.c_int), ('serial', C.c_ulong), ('send_event', C.c_int),
                ('display', C.c_void_p), ('window', C.c_ulong), ('root', C.c_ulong),
                ('subwindow', C.c_ulong), ('time', C.c_ulong), ('x', C.c_int),
                ('y', C.c_int), ('x_root', C.c_int), ('y_root', C.c_int),
                ('state', C.c_uint), ('keycode', C.c_uint), ('same_screen', C.c_int)]


def main():
    (OUT / 'raw').mkdir(parents=True, exist_ok=True)
    (OUT / 'source').mkdir(parents=True, exist_ok=True)
    home = Path(tempfile.mkdtemp(prefix='bp-readme-features-'))
    project = home / 'demo-project'
    project.mkdir()
    (project / 'review.txt').write_text((Path(__file__).resolve().parent / 'review-checklist.txt').read_text())
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    now = int(time.time())
    events = []
    def event(kind, **kw):
        events.append({'$': kind, **kw, 'at': now})
    event('SettingSet', key='onboarded', value='1')
    event('ProjectCreated', id='demo-board', title='Sensor Board', root=str(project))
    for id, title in [('demo-main', 'Review the sensor board'), ('demo-layout', 'Layout review'), ('demo-power', 'Power budget'), ('demo-bot-thread', 'Miso')]:
        event('ThreadCreated', id=id, project='demo-board', title=title, env='local', branch='', worktree='', provider='codex', model='gpt-6.1-sol')
    event('BotSet', id='demo-miso', name='Miso', thread='demo-bot-thread', look=2, persona='Helps review circuits and check design decisions.')
    event('RoomSet', id='demo-hardware', name='Hardware review', members=['demo-miso'])
    for id, summary in [('demo-layout', 'Placement, routing, and connector access.'), ('demo-power', 'Sleep current and battery life.'), ('demo-board', 'A small battery powered environmental sensor.')]:
        event('SettingSet', key='summary.' + id, value=summary)
    event('MessagePosted', thread='demo-main', msg='demo-m1', role='user', text='Keep the design review connected across threads, projects, and bots.')
    event('MessagePosted', thread='demo-main', msg='demo-m2', role='assistant', text='The review is organized here:\n\n[Layout review](backplane://thread/demo-layout)\n\n[Miso](backplane://bot/demo-miso)\n\n[Sensor Board](backplane://project/demo-board)\n\nUse references in the composer to bring the right context into the conversation.')
    (OUT / 'source/demo-events.jsonl').write_text(''.join(json.dumps(e) + '\n' for e in events))
    (home / 'events.jsonl').write_text((OUT / 'source/demo-events.jsonl').read_text())
    display = next(n for n in range(100, 110) if not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists())
    env = {**os.environ, 'DISPLAY': f':{display}', 'HOME': str(home), 'ZDOTDIR': str(home),
           'BACKPLANE_HOME': str(home), 'BACKPLANE_SNAP': str(home / 'frame.ppm'),
           'BACKPLANE_NO_UPDATE': '1', 'BACKPLANE_PEERS': '', 'PATH': '/usr/bin:/bin',
           'BACKPLANE_KICAD_CLI': '/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli',
           'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_CACHE_HOME': str(home / 'cache'),
           'XDG_DATA_HOME': str(home / 'data')}
    processes = []
    captures = []
    try:
        log = (home / 'xvfb.log').open('w')
        processes.append(subprocess.Popen([XVFB, f':{display}', '-screen', '0', f'{W}x{H}x24', '-nolisten', 'tcp'], stdout=log, stderr=log))
        time.sleep(.5)
        log = (home / 'app.log').open('w')
        processes.append(subprocess.Popen([str(APP), '--home', str(home), '--port', str(port), '--no-tailscale'], env=env, cwd=project, stdout=log, stderr=log))
        def poke(*args):
            subprocess.run([XPOKE, *map(str, args)], env=env, check=True)
        def snap(name, delay=.7):
            time.sleep(delay)
            Image.open(home / 'frame.ppm').save(OUT / 'raw' / (name + '.png'))
            captures.append(name)
            print('Captured ' + name, flush=True)
        def symbol(name):
            # xpoke type supports only a subset of ASCII punctuation.
            x = C.CDLL('libX11.so.6')
            x.XOpenDisplay.restype = C.c_void_p
            x.XOpenDisplay.argtypes = [C.c_char_p]
            d = x.XOpenDisplay(env['DISPLAY'].encode())
            for fn in ['XDefaultRootWindow', 'XKeysymToKeycode', 'XSendEvent', 'XFlush', 'XCloseDisplay']:
                getattr(x, fn).argtypes = [C.c_void_p] + ({'XDefaultRootWindow': [], 'XKeysymToKeycode': [C.c_ulong], 'XSendEvent': [C.c_ulong, C.c_int, C.c_long, C.c_void_p], 'XFlush': [], 'XCloseDisplay': []}[fn])
            x.XDefaultRootWindow.restype = C.c_ulong
            x.XStringToKeysym.argtypes = [C.c_char_p]
            x.XStringToKeysym.restype = C.c_ulong
            # The native snap hook's window is the root's only child.
            root = x.XDefaultRootWindow(d)
            rr, parent, children, count = C.c_ulong(), C.c_ulong(), C.POINTER(C.c_ulong)(), C.c_uint()
            x.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]
            x.XQueryTree(d, root, C.byref(rr), C.byref(parent), C.byref(children), C.byref(count))
            w = children[count.value - 1]
            e = Key(2, 0, 1, d, w, root, 0, 0, 500, 900, 500, 900, 1, x.XKeysymToKeycode(d, x.XStringToKeysym(name.encode())), 1)
            x.XSendEvent(d, w, 1, 1, C.byref(e))
            e.type = 3
            x.XSendEvent(d, w, 1, 2, C.byref(e))
            x.XFlush(d)
            x.XCloseDisplay(d)
        def draft(text='', sigil=None):
            poke('click', 600, H - 95)
            poke('ctrl', 'a')
            poke('key', 'BackSpace')
            if sigil:
                symbol(sigil)
            if text:
                poke('type', text)
        def mcp(name, args):
            token = (home / 'token').read_text().strip()
            data = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': name, 'arguments': args}}).encode()
            req = urllib.request.Request(f'http://127.0.0.1:{port}/mcp/demo-main?token={token}', data=data, headers={'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream'})
            with urllib.request.urlopen(req, timeout=20) as r:
                answer = r.read().decode()
            (OUT / 'source' / (name + '.json')).write_text(answer)
            return answer
        time.sleep(3)
        poke('size', W, H)
        poke('key', 'Escape')
        # Select named thread, locate from known title row after first frame.
        snap('startup')
        im = Image.open(home / 'frame.ppm').convert('RGB')
        folder_y = next(y for y in range(80, 200) if (lambda c: c[0] > 160 and c[0] > c[1] * 1.4)(im.getpixel((22, y))))
        poke('click', 120, folder_y + 98)
        snap('selected')
        if os.environ.get('README_CAPTURE_LINKS_ONLY'):
            snap('link-cards')
            (OUT / 'source/capture.json').write_text(json.dumps({'app': str(APP), 'app_sha256': hashlib.sha256(APP.read_bytes()).hexdigest(), 'viewport': [W, H], 'temporary_home': str(home), 'display': env['DISPLAY'], 'captures': captures, 'capture': 'Actual native app pixels from existing links candidate binary; seeded demo event fixture; no agent turns, sent messages, MCP calls, builds, or runtime changes.'}, indent=2) + '\n')
            print('Home: ' + str(home), flush=True)
            return
        for name, sym, text in [('thread', 'greater', ''), ('room', 'numbersign', ''), ('project', 'percent', ''), ('bot', 'at', ''), ('caret', 'asciicircum', '')]:
            draft(text, sym)
            snap('reference-' + name)
            poke('key', 'Escape')
        draft('/bt')
        snap('btw-autocomplete')
        draft('/btw What should I review first?')
        snap('btw-draft')
        draft()
        snap('link-cards')
        answer = json.loads(mcp('terminal_open', {'command': 'vi -u NONE -n -R review.txt', 'title': 'Review checklist · vi', 'cols': 88, 'rows': 16}))
        term = json.loads(answer['result']['content'][0]['text'])['id']
        time.sleep(.6)
        mcp('terminal_send', {'id': term, 'text': '\rjj'})
        snap('inline-terminal', 2)
        mcp('terminal_read', {'id': term})
        poke('click', 700, H - 45)
        snap('model-picker')
        (OUT / 'source/capture.json').write_text(json.dumps({'app': str(APP), 'app_sha256': hashlib.sha256(APP.read_bytes()).hexdigest(), 'installed_commit_file': Path('/home/h/.local/share/backplane/COMMIT').read_text().strip(), 'viewport': [W, H], 'temporary_home': str(home), 'display': env['DISPLAY'], 'captures': captures, 'capture': 'Actual native app pixels; seeded documented event fixture; no agent turns or sent chat messages; loopback terminal_open launches local vi only.'}, indent=2) + '\n')
        print('Home: ' + str(home), flush=True)
    finally:
        for p in reversed(processes):
            p.terminate()
            try:
                p.wait(timeout=4)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()


if __name__ == '__main__':
    main()
