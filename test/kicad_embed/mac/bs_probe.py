#!/usr/bin/python3
"""Does ⌫ delete a selected footprint in a stand-alone pcbnew on this Mac? (the embedded pane's ⌫ did nothing, Nov 16)
Run in the GUI session (gate_run.py), with the gate helper and the VNC relay up:
    bs_probe.py PCBNEW_BINARY BOARD OUT TAG [--hotkeys PRIMARY]
KiCad's config goes to OUT/TAG/cfg (KICAD_CONFIG_HOME). Prints what changed in the canvas after ⌫ (and, for comparison, after fn+delete
on a fresh copy), and screenshots: before, selected, after ⌫, the Edit menu.
"""
import json, os, shutil, socket, subprocess, sys, time

GATE = os.path.expanduser('~/bp-gate-m')
def ask(path, line):
    c = socket.socket(socket.AF_UNIX); c.settimeout(60); c.connect(path); c.sendall((line + '\n').encode()); r = c.makefile().readline(); c.close(); return r
def helper(q): return json.loads(ask(os.path.join(GATE, 'helper.sock'), q) or 'null')
def relay(**o): return json.loads(ask(os.path.join(GATE, 'vnc.sock'), json.dumps(o)))

pcbnew, board, out, tag = sys.argv[1:5]
hot = sys.argv[sys.argv.index('--hotkeys') + 1] if '--hotkeys' in sys.argv else ''
d = os.path.join(out, tag); shutil.rmtree(d, ignore_errors=True); os.makedirs(os.path.join(d, 'cfg', '10.99')); os.makedirs(os.path.join(d, 'cfg', '10.0'))
SEED = os.path.expanduser('~/bp-e2e/src/test/kicad_embed/lib/seed_project.py')
_t = os.path.join(out, tag + '-seed'); shutil.rmtree(_t, ignore_errors=True)
subprocess.run(['/usr/bin/python3', SEED, _t, os.path.expanduser('~/bp-e2e/src')], capture_output=True)
for v in ('10.99', '10.0'):
    src = os.path.join(_t, 'fh/.config/kicad', v, 'kicad_common.json')
    if os.path.isfile(src): shutil.copy(src, os.path.join(d, 'cfg', v, 'kicad_common.json'))
for v in ('10.99', '10.0'):
    if hot: open(os.path.join(d, 'cfg', v, 'user.hotkeys'), 'w').write(f'common.Interactive.delete\t{hot}\tDel\n')
env = dict(os.environ, KICAD_CONFIG_HOME=os.path.join(d, 'cfg'), KICAD_DOCUMENTS_HOME=os.path.join(d, 'docs'))

def crop(k, name):
    x, y, w, h = k
    p = os.path.join(d, name + '.rgb'); relay(op='crop', x=x, y=y, w=w, h=h, path=p)
    raw = open(p, 'rb').read(); nl = raw.index(b'\n'); return raw[nl + 1:]
def diff(a, b): return round(sum(1 for i in range(0, min(len(a), len(b)), 3) if abs(a[i] - b[i]) + abs(a[i + 1] - b[i + 1]) + abs(a[i + 2] - b[i + 2]) > 60) / max(1, len(a) // 3), 4)

def run(key_combo, label):
    bd = os.path.join(d, label); os.makedirs(bd, exist_ok=True)
    for f in os.listdir(os.path.dirname(board)):
        if f.startswith('ecc83-pp.'): shutil.copy(os.path.join(os.path.dirname(board), f), bd)
    p = subprocess.Popen([pcbnew, os.path.join(bd, 'ecc83-pp.kicad_pcb')], env=env, stdout=open(os.path.join(d, label + '.log'), 'w'), stderr=subprocess.STDOUT)
    win = None
    for _ in range(120):
        time.sleep(1)
        ws = [w for w in helper('windows') if w['pid'] == p.pid and w['layer'] == 0]
        if ws:
            win = max(ws, key=lambda w: w['bounds'][2] * w['bounds'][3])
            if win['bounds'][2] > 600: break
    if not win: relay(op='png', path=os.path.join(d, label + '-nowin.png')); p.kill(); return {'error': 'no pcbnew window'}
    time.sleep(4)
    # pcbnew to the front (a window restored at login covered the screen's middle once): LaunchServices, no permission needed
    app = pcbnew.split('/Contents/MacOS/')[0]
    subprocess.run(['/usr/bin/open', '-a', app], capture_output=True); time.sleep(2)
    if not (helper(f'active {p.pid}') or {}).get('active'):
        relay(op='png', path=os.path.join(d, label + '-notfront.png')); p.kill(); return {'error': 'pcbnew is not the active app: nothing pressed'}
    win = max([w for w in helper('windows') if w['pid'] == p.pid and w['layer'] == 0], key=lambda w: w['bounds'][2] * w['bounds'][3])
    # a first-start question (library tables, data collection): the default answer
    if len([w for w in helper('windows') if w['pid'] == p.pid and w['layer'] == 0]) > 1:
        relay(op='png', path=os.path.join(d, label + '-dialog.png'))
    x, y, w, h = win['bounds']
    relay(op='click', x=x + w / 2, y=y + h / 2); time.sleep(0.4)
    relay(op='chord', combo='cmd+shift+f' if False else 'home'); time.sleep(1.5)  # Home: zoom to fit
    canvas = (x + w * 0.2, y + h * 0.2, w * 0.55, h * 0.6)
    relay(op='png', path=os.path.join(d, label + '-1-before.png'))
    relay(op='click', x=x + w * 0.47, y=y + h * 0.5); time.sleep(1.0)   # a footprint near the board's middle
    relay(op='png', path=os.path.join(d, label + '-2-selected.png'))
    a = crop(canvas, label + '-sel')
    if not (helper(f'active {p.pid}') or {}).get('active'): p.kill(); return {'error': 'pcbnew lost the front before the key: nothing pressed'}
    relay(op='chord', combo=key_combo); time.sleep(1.5)
    relay(op='png', path=os.path.join(d, label + '-3-after.png'))
    b = crop(canvas, label + '-after')
    # the Edit menu: its title in the menu bar (the app's menus start after its name)
    relay(op='click', x=x + w / 2, y=y + 40); time.sleep(0.5)
    relay(op='png', path=os.path.join(d, label + '-4-menubar.png'))
    p.kill(); p.wait(10)
    return {'changed_after_key': diff(a, b)}

res = {'backspace': run('backspace', 'bs'), 'forward_delete': run('delete', 'fdel')}
print(json.dumps({'tag': tag, 'pcbnew': pcbnew, 'hotkeys': hot, **res}), flush=True)
