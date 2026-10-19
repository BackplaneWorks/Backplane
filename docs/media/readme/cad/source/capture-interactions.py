"""Capture the installed Backplane in an isolated home and virtual display.

Run with uv run --no-project --python /usr/bin/python3 python capture-interactions.py. No agents are started and
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
W, H = 1600, 1000


def main():
    home = Path(tempfile.mkdtemp(prefix='backplane-readme-'))
    (OUT / 'source').mkdir(parents=True, exist_ok=True)
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        port = s.getsockname()[1]
    now = int(time.time())
    mech = os.environ.get('README_CAPTURE_KIND') == 'mech'
    step = Path(os.environ.get('README_CAPTURE_STEP', '/home/h/Documents/Backplane/Backplane_Demos/phaser_1248/mechanical/sources/1590BB_STEP/1590BB.stp'))
    title = 'Hammond 1590BB enclosure' if mech else 'RoyalBlue54L'
    root = step.parent.parent.parent.parent if mech else BOARD.parent
    events = [
        {'$': 'SettingSet', 'key': 'onboarded', 'value': '1', 'at': now},
        {'$': 'ProjectCreated', 'id': 'royalblue', 'title': title, 'root': str(root), 'at': now},
        {'$': 'ThreadCreated', 'id': 'viewer-demo', 'project': 'royalblue', 'title': title, 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'codex', 'model': 'gpt-6.1-sol', 'at': now},
    ]
    (home / 'events.jsonl').write_text(''.join(json.dumps(x) + '\n' for x in events))
    display = next(n for n in range(110, 120) if not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists())
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
            Image.open(home / 'frame.ppm').save(OUT / 'source' / f'{name}.png')
            print(f'Captured {name}', flush=True)
        def show(kind, path):
            token = (home / 'token').read_text().strip()
            data = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': 'viewer_show', 'arguments': {'kind': kind, 'path': str(path)}}}).encode()
            req = urllib.request.Request(f'http://127.0.0.1:{port}/mcp/viewer-demo?token={token}', data=data, headers={'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream'})
            with urllib.request.urlopen(req, timeout=20) as r:
                result = r.read().decode()
            (OUT / 'source' / ('viewer-show-' + kind + '.json')).write_text(result)
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
        if os.environ.get('README_CAPTURE_KIND') == 'mech':
            show('mech', step)
            snap('enclosure-mech-native', 14)
        else:
            show('board', BOARD)
            time.sleep(12)
            snap('board-loaded-native', 1)
            poke('click', 1237, 378)
            snap('pad-selected-native', 1)
            poke('click', 1040, 862)
            snap('pad-mentioned-native', 1)
            poke('click', 110, H - 25)
            for i in range(28):
                poke('wheel', 700, 650, 'down')
                time.sleep(.2)
                if i in (23, 27):
                    snap('settings-' + str(i+1) + '-native', 1)
            snap('kicad-settings-native', 1)
        manifest = {'app': str(APP), 'app_sha256': hashlib.sha256(APP.read_bytes()).hexdigest(), 'board': str(BOARD), 'board_sha256': hashlib.sha256(BOARD.read_bytes()).hexdigest(), 'viewport': [W, H], 'temporary_home': str(home), 'events': events, 'capture': 'native Backplane on private Xvfb; no CAD edits or agent turns', 'mode': os.environ.get('README_CAPTURE_KIND', 'interaction'), 'step': str(step) if mech else None, 'step_sha256': hashlib.sha256(step.read_bytes()).hexdigest() if mech else None}
        (OUT / ('source/capture-' + os.environ.get('README_CAPTURE_KIND', 'interaction') + '.json')).write_text(json.dumps(manifest, indent=2) + '\n')
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
