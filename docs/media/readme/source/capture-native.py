"""Capture the installed Backplane in an isolated home and virtual display.

Run with uv run --with pillow capture-native.py. No agents are started and
the RoyalBlue54L design is read only. Frames are actual native app pixels.
"""
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

OUT = Path(__file__).resolve().parent.parent
BOARD = Path('/home/h/Documents/Backplane/Backplane_KiCad/demos/royalblue54L_feather/RoyalBlue54L-Feather.kicad_pcb')
APP = Path(os.environ.get('README_CAPTURE_APP', '/home/h/.local/share/backplane/backplane'))
XVFB = Path('/home/h/.local/xvfb-root/usr/bin/Xvfb')
XPOKE = Path('/home/h/Documents/i2cjak/bp-review/ship4/bin/xpoke')
W, H = (3200, 2000) if os.environ.get('README_CAPTURE_PHASE') == 'schematic' else (1600, 1000)


def main():
    home = Path(tempfile.mkdtemp(prefix='backplane-readme-'))
    (OUT / 'source').mkdir(parents=True, exist_ok=True)
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        port = s.getsockname()[1]
    now = int(time.time())
    events = [
        {'$': 'SettingSet', 'key': 'onboarded', 'value': '1', 'at': now},
        {'$': 'ProjectCreated', 'id': 'royalblue', 'title': 'RoyalBlue54L', 'root': str(BOARD.parent), 'at': now},
        {'$': 'ThreadCreated', 'id': 'viewer-demo', 'project': 'royalblue', 'title': 'RoyalBlue54L', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'codex', 'model': 'gpt-6.1-sol', 'at': now},
    ]
    (home / 'events.jsonl').write_text(''.join(json.dumps(x) + '\n' for x in events))
    display = next(n for n in range(90, 120) if not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists())
    env = {**os.environ, 'DISPLAY': f':{display}', 'BACKPLANE_HOME': str(home), 'BACKPLANE_SNAP': str(home / 'frame.ppm'), 'BACKPLANE_NO_UPDATE': '1', 'BACKPLANE_PEERS': '', 'BACKPLANE_KICAD_CLI': '/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli'}
    processes = []
    try:
        xvlog = (home / 'xvfb.log').open('w')
        xv = subprocess.Popen([str(XVFB), f':{display}', '-screen', '0', f'{W}x{H}x24', '-nolisten', 'tcp'], stdout=xvlog, stderr=xvlog)
        processes.append(xv)
        time.sleep(0.5)
        log = (home / 'app.log').open('w')
        app = subprocess.Popen([str(APP), '--home', str(home), '--port', str(port), '--no-tailscale'], env=env, stdout=log, stderr=log)
        processes.append(app)
        def poke(*args):
            subprocess.run([str(XPOKE), *map(str, args)], env=env, check=True)
        def snap(name, settle=2):
            time.sleep(settle)
            Image.open(home / 'frame.ppm').save(OUT / f'{name}.png')
            print(f'Captured {name}', flush=True)
        def show(kind, path):
            token = (home / 'token').read_text().strip()
            data = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': 'viewer_show', 'arguments': {'kind': kind, 'path': str(path)}}}).encode()
            req = urllib.request.Request(f'http://127.0.0.1:{port}/mcp/viewer-demo?token={token}', data=data, headers={'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream'})
            with urllib.request.urlopen(req, timeout=20) as r:
                result = r.read().decode()
            if '"isError":true' in result:
                raise RuntimeError(result)
        time.sleep(3)
        poke('size', W, H)
        poke('key', 'Escape')
        time.sleep(1)
        # Scratch can appear above or below the project; find the thread's
        # grey status dot rather than assuming the sidebar order.
        im = Image.open(home / 'frame.ppm').convert('RGB')
        ys = [y for y in range(100, 300) if (lambda c: 45 < c[0] < 150 and c[2] > c[0] + 4 and max(c) - min(c) < 45)(im.getpixel((18, y)))]
        if not ys:
            raise RuntimeError('No thread status dot found in sidebar')
        poke('click', 120, ys[0] + 2)
        snap('initial', 2)
        if os.environ.get('README_CAPTURE_PHASE') == 'schematic':
            show('schematic', BOARD.parent / 'sch/nRF54L15.kicad_sch')
            time.sleep(4)
            # Find U1's filled body in the larger native frame. Render
            # the schematic at this resolution rather than enlarging a
            # small raster capture in the README composition.
            left = W // 2 + 133
            region = Image.open(home / 'frame.ppm').convert('RGB').crop((left, 100, W, H - 100))
            rw = region.width
            pending = {i for i, c in enumerate(region.getdata()) if c == (32, 31, 19)}
            largest = []
            while pending:
                start = pending.pop()
                component, todo = [start], [start]
                while todo:
                    i = todo.pop()
                    for j in (i - rw, i + rw, i - 1, i + 1):
                        if j in pending:
                            pending.remove(j)
                            component.append(j)
                            todo.append(j)
                if len(component) > len(largest):
                    largest = component
            if not largest:
                raise RuntimeError('U1 body was not found in the native schematic')
            ux = left + (min(i % rw for i in largest) + max(i % rw for i in largest)) // 2
            uy = 100 + (min(i // rw for i in largest) + max(i // rw for i in largest)) // 2
            cx, cy = (left + W) // 2, H // 2 - 22
            poke('wheel', ux, uy, 'up')
            time.sleep(.3)
            poke('drag', ux, uy, cx, cy)
            snap('royalblue-schematic-detail', 2)
            (OUT / 'source/schematic-detail.json').write_text(json.dumps({'app': str(APP), 'schematic': str(BOARD.parent / 'sch/nRF54L15.kicad_sch'), 'zoom': 'one wheel notch at U1, then pan to panel center', 'viewport': [W, H], 'center': [cx, cy]}, indent=2) + '\n')
            return
        show('board', BOARD)
        snap('royalblue-board', 5)
        if os.environ.get('README_CAPTURE_PHASE') == 'board':
            return
        show('schematic', BOARD.parent / 'sch/nRF54L15.kicad_sch')
        snap('royalblue-schematic', 4)
        show('3d', BOARD)
        snap('royalblue-3d', 20)
        poke('click', 460, H - 45)
        snap('agent-model-picker', 2)
        poke('key', 'Escape')
        poke('click', 110, H - 25)
        snap('agent-settings', 2)
        manifest = {'app': str(APP), 'app_sha256': hashlib.sha256(APP.read_bytes()).hexdigest(), 'board': str(BOARD), 'board_sha256': hashlib.sha256(BOARD.read_bytes()).hexdigest(), 'viewport': [W, H], 'temporary_home': str(home), 'events': events, 'capture': 'native Backplane on Xvfb; no design edits or agent turns'}
        (OUT / 'source/capture.json').write_text(json.dumps(manifest, indent=2) + '\n')
        print(json.dumps({'output': str(OUT), 'home': str(home)}), flush=True)
    finally:
        for p in reversed(processes):
            p.terminate()
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()


if __name__ == '__main__':
    main()
