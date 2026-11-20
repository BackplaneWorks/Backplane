#!/usr/bin/env python3
"""The size matrix for the GPUI client (docs/e2e/SIZES.md): one run per (window size, text scale), each walking every screen in scenarios.json with the client's layout audit
(BACKPLANE_GPUI_AUDIT=1, BACKPLANE_GPUI_SIZE=WxH, text_pct in the client's local settings), judging each frame with check.py and keeping a screenshot per screen.
usage: matrix.py --platform linux|windows|mac --client EXE --hub EXE --assets DIR --out DIR [--sizes 800x600,...] [--text 80,100,125] [--only scenario,...] [--port N] [--display N]
  linux: a private Xvfb per size (physical size; Xft.dpi for the HiDPI case) with i3 filling it. windows/mac: the window is BACKPLANE_GPUI_SIZE logical px in the desktop session
  (run IN the session); the HiDPI case runs at its logical size (the machine's own scale applies).
Writes OUT/<size>-t<pct>/{audit.jsonl,client.log,shots/*.png,findings.json}, OUT/report.json, OUT/report.md, OUT/contact-<size>.png. Exit 1 when a finding is new (not in known.json)."""
import os, sys, json, time, shutil, subprocess, tempfile, glob, argparse, threading
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
sys.path.insert(0, HERE)
import check
ap = argparse.ArgumentParser()
ap.add_argument('--platform', default='linux'); ap.add_argument('--client', required=True); ap.add_argument('--hub', required=True); ap.add_argument('--assets', required=True); ap.add_argument('--out', required=True)
ap.add_argument('--sizes', default=''); ap.add_argument('--text', default=''); ap.add_argument('--only', default=''); ap.add_argument('--port', type=int, default=3861); ap.add_argument('--display', type=int, default=61)
ap.add_argument('--backend', default='sway' if __import__('shutil').which('sway') and not __import__('shutil').which('i3') else 'xvfb', help='linux: sway (headless Wayland, any GPU) or xvfb (+i3, lavapipe, what CI has)')
ap.add_argument('--shard', default='', help='i/n: this runner takes every n-th (size, text) pair')
ap.add_argument('--allow-known', action='store_true', help='record the findings as known.json (the first run on a new platform)')
A = ap.parse_args()
OUT = os.path.abspath(A.out); CLIENT = os.path.abspath(A.client); HUB = os.path.abspath(A.hub); ASSETS = os.path.abspath(A.assets)
SZ = json.load(open(os.path.join(HERE, 'sizes.json'))); SC = json.load(open(os.path.join(HERE, 'scenarios.json')))
KNOWN = json.load(open(os.path.join(HERE, 'known.json'))) if os.path.exists(os.path.join(HERE, 'known.json')) else {}
IS_WIN = A.platform == 'windows'
sizes = [s for s in SZ['desktop'] if not A.sizes or s['name'] in A.sizes.split(',')]
pcts = [int(p) for p in (A.text.split(',') if A.text else SZ['text_pct'])]
combos = [(s, p) for s in sizes for p in pcts]
if A.shard:
    i, n = map(int, A.shard.split('/')); combos = [c for k, c in enumerate(combos) if k % n == i]
scen_all = [s for s in SC['scenarios'] if not A.only or s['name'] in A.only.split(',')]
os.makedirs(OUT, exist_ok=True)
procs = []
def P(*a): return os.path.join(OUT, *a)
now = int(time.time())

def fixture(root):
    """a hub home: a project with long and short thread titles, one with a very long project name, a streaming-turn thread, an order, files"""
    for d in ['home', 'proj/mech', 'bin', 'fh/.cache']: os.makedirs(os.path.join(root, d), exist_ok=True)
    shutil.copy(os.path.join(HERE, '..', 'lib/standin_rows.py'), os.path.join(root, 'bin', 'claude'))
    for n in ['codex', 'grok', 'muse', 'opencode']: open(os.path.join(root, 'bin', n), 'w', newline='\n').write('#!/bin/sh\nexit 1\n')
    for f in glob.glob(os.path.join(root, 'bin', '*')): os.chmod(f, 0o755)
    shutil.copy(os.path.join(REPO, 'test/files/fixtures/mesh/bracket.step'), os.path.join(root, 'proj/mech/bracket.step'))
    for ext in ('kicad_pcb', 'kicad_sch'): shutil.copy(os.path.join(REPO, 'test/files/fixtures/plot/ecc83-pp.' + ext), os.path.join(root, 'proj/ecc83.' + ext))
    open(os.path.join(root, 'proj/ecc83.kicad_pro'), 'w').write('{}\n')
    titles = ['Thread 1', 'Route the USB-C power path and add the input protection the datasheet asks for', 'x', 'Fix multi-machine: phone adding hubs, connecting to existing hub', 'Thread 5', 'Thread 6', 'Thread 7', 'Thread 8']
    ev = [{'$': 'ProjectCreated', 'id': 'p1', 'title': 'rows', 'root': os.path.join(root, 'proj'), 'at': now},
          {'$': 'ProjectCreated', 'id': 'p2', 'title': 'A project with a rather long name that has to fit the sidebar somehow', 'root': os.path.join(root, 'proj'), 'at': now}]
    ev += [{'$': 'ThreadCreated', 'id': 't%d' % (i + 1), 'project': 'p1' if i < 6 else 'p2', 'title': t, 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + i} for i, t in enumerate(titles)]
    ev += [{'$': 'ModesSet', 'id': 't2', 'runtime': 'approval-required', 'interaction': 'default', 'model': '', 'at': now + 9}]
    open(os.path.join(root, 'home/events.jsonl'), 'w', newline='\n').write(''.join(json.dumps(e) + '\n' for e in ev))

def steps_for(scens):
    out = []
    for s in scens:
        out += s['steps'] + [{'wait': 700}, {'audit': s['name']}, {'mark': s['name']}, {'wait': 1500}]
        out += s.get('after', [])
    return out

def run_one(size, pct, port, disp):
    tag = '%s-t%d' % (size['name'], pct); d = P(tag); shutil.rmtree(d, ignore_errors=True)
    for x in ['shots', 'root']: os.makedirs(os.path.join(d, x), exist_ok=True)
    root = os.path.join(d, 'root'); fixture(root)
    scale = size['dpi'] / 96.0; lw, lh = round(size['w'] / scale), round(size['h'] / scale)
    local = os.path.join(d, 'local.json'); json.dump({'text_pct': pct, 'sidebar_open': True}, open(local, 'w'))
    script = os.path.join(d, 'script.json'); json.dump([{'wait': 2500}] + steps_for(scen_all) + [{'wait': 500}], open(script, 'w'))
    env = dict(os.environ)
    env.update(BACKPLANE_AGENTS=os.path.join(root, 'bin'), BACKPLANE_NO_UPDATE='1', HOME=os.path.join(root, 'fh'), XDG_CONFIG_HOME=os.path.join(root, 'fh/.config'), XDG_CACHE_HOME=os.path.join(root, 'fh/.cache'),
               XDG_DATA_HOME=os.path.join(root, 'fh/.local/share'), PATH=os.path.join(root, 'bin') + os.pathsep + os.path.dirname(sys.executable) + os.pathsep + os.environ['PATH'], FAKE_SECS='600', FAKE_LOG=os.path.join(d, 'fake.log'),
               BACKPLANE_GPUI_ASSETS=ASSETS, BACKPLANE_GPUI_LOCAL=local, BACKPLANE_GPUI_AUDIT='1', BACKPLANE_GPUI_SIZE='%dx%d' % (lw, lh), LC_ALL='C')
    if IS_WIN: env.update(USERPROFILE=os.path.join(root, 'fh'), APPDATA=os.path.join(root, 'fh/AppData/Roaming'), LOCALAPPDATA=os.path.join(root, 'fh/AppData/Local'))
    env.pop('SHELL', None) if IS_WIN else env.update(SHELL='/bin/sh')
    mine = []
    def spawn(args, log, e=env, cwd=None):
        kw = {'creationflags': subprocess.CREATE_NO_WINDOW} if IS_WIN else {}
        p = subprocess.Popen(args, env=e, cwd=cwd, stdout=open(os.path.join(d, log), 'ab'), stderr=subprocess.STDOUT, **kw); mine.append(p); procs.append(p); return p
    grab = None
    try:
        if A.platform == 'linux' and A.backend == 'sway':
            # headless sway: a Wayland compositor with one output of the physical size and the scale; the client's window fills it (no GPU display needed beyond a render node)
            run = tempfile.mkdtemp(prefix='sm.', dir=os.path.expanduser('~/.cache/bp-tmp') if os.path.isdir(os.path.expanduser('~/.cache/bp-tmp')) else None); os.chmod(run, 0o700)
            open(os.path.join(d, 'sway.conf'), 'w').write('default_border none\nfloating_modifier Mod4\noutput HEADLESS-1 resolution %dx%d scale %g\n' % (size['w'], size['h'], scale))
            se = dict(env, XDG_RUNTIME_DIR=run, WLR_BACKENDS='headless', WLR_LIBINPUT_NO_DEVICES='1'); se.pop('WAYLAND_DISPLAY', None); se.pop('SWAYSOCK', None); se.pop('DISPLAY', None)
            spawn(['sway', '-c', os.path.join(d, 'sway.conf')], 'sway.log', se)
            for _ in range(50):
                if glob.glob(os.path.join(run, 'wayland-*[0-9]')) and glob.glob(os.path.join(run, 'sway-ipc*')): break
                time.sleep(0.2)
            # the client is an X11 client (gpui feature x11): it runs on the compositor's Xwayland, whose DISPLAY the compositor tells its children
            env.update(XDG_RUNTIME_DIR=run, WAYLAND_DISPLAY=os.path.basename(sorted(glob.glob(os.path.join(run, 'wayland-*[0-9]')))[0]), SWAYSOCK=sorted(glob.glob(os.path.join(run, 'sway-ipc*')))[0])
            dfile = os.path.join(run, 'display'); subprocess.run(['swaymsg', 'exec', 'sh -c "echo $DISPLAY > %s"' % dfile], env=env, capture_output=True)
            for _ in range(50):
                if os.path.exists(dfile) and open(dfile).read().strip(): break
                time.sleep(0.2)
            env['DISPLAY'] = open(dfile).read().strip(); env.pop('WAYLAND_DISPLAY', None)
            def grab(f): subprocess.run(['grim', f], env=dict(env, WAYLAND_DISPLAY=os.path.basename(sorted(glob.glob(os.path.join(run, 'wayland-*[0-9]')))[0])))
        elif A.platform == 'linux':
            run = tempfile.mkdtemp(prefix='sm.'); os.chmod(run, 0o700); env.update(XDG_RUNTIME_DIR=run); disp_s = ':%d' % disp
            for k in ['I3SOCK', 'DBUS_SESSION_BUS_ADDRESS', 'WAYLAND_DISPLAY']: env.pop(k, None)
            xv = os.environ.get('XVFB') or shutil.which('Xvfb') or os.path.expanduser('~/.cache/xvfb-root/usr/bin/Xvfb'); xe = dict(env)
            if os.environ.get('XVFB_LIB'): xe['LD_LIBRARY_PATH'] = os.environ['XVFB_LIB']
            spawn([xv, disp_s, '-screen', '0', '%dx%dx24' % (size['w'], size['h']), '-dpi', str(size['dpi']), '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log', xe); time.sleep(2)
            env['DISPLAY'] = disp_s
            if size['dpi'] != 96 and shutil.which('xrdb'): subprocess.run(['xrdb', '-merge'], input=('Xft.dpi: %d\n' % size['dpi']).encode(), env=env)
            open(os.path.join(d, 'i3.conf'), 'w').write('font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' % run)
            if shutil.which('i3'): spawn(['i3', '-c', os.path.join(d, 'i3.conf')], 'i3.log'); time.sleep(1)   # no i3 (this desktop): the window is placed by the client itself, which is enough on a bare Xvfb of the same size
            env.update(VK_ICD_FILENAMES=(sorted(glob.glob('/usr/share/vulkan/icd.d/lvp_icd*.json')) or ['/usr/share/vulkan/icd.d/lvp_icd.json'])[0], LP_NUM_THREADS='2')
            def grab(f):
                # ImageMagick's import, then xwd when it returns a black image (the CI runner's ImageMagick 6 does, as in gpui-f-rows.py); PIL's grab last
                try:
                    if shutil.which('import'):
                        subprocess.run(['import', '-window', 'root', f], env=env, timeout=30)
                        from PIL import Image
                        if Image.open(f).convert('L').getextrema()[1] == 0 and shutil.which('xwd'):
                            subprocess.run(['sh', '-c', 'xwd -root -silent | convert xwd:- "png:$1"', 'sh', f], env=env, timeout=30)
                        return
                except Exception: pass
                from PIL import ImageGrab
                ImageGrab.grab(xdisplay=disp_s).save(f)
        else:
            from PIL import ImageGrab
            def grab(f): ImageGrab.grab(all_screens=True).save(f)
        spawn([HUB, '--home', os.path.join(root, 'home'), '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=os.path.join(root, 'proj'))
        for _ in range(100):
            if subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', 'http://127.0.0.1:%d/hello' % port]).returncode == 0: break
            time.sleep(0.2)
        cl = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % port, '--token-file', os.path.join(root, 'home/token'), '--script', script, '--exit-after', '600'], 'client.log')
        seen_marks, audits = set(), {}; log = os.path.join(d, 'client.log'); end = time.time() + 420
        while time.time() < end and cl.poll() is None:
            for l in open(log, errors='replace'):
                if 'GPUI_METRIC ' not in l: continue
                try: m = json.loads(l.split('GPUI_METRIC ', 1)[1])
                except Exception: continue
                if m.get('event') == 'audit': audits[m['name']] = m
                if m.get('event') == 'mark' and m['name'] not in seen_marks:
                    seen_marks.add(m['name']); grab(os.path.join(d, 'shots', m['name'] + '.png'))
            if len(audits) >= len(scen_all): break
            time.sleep(0.4)
        cl.terminate()
    finally:
        for p in mine:
            try: p.terminate()
            except Exception: pass
        for p in mine:
            try: p.wait(10)
            except Exception: p.kill()
        if A.platform == 'linux': shutil.rmtree(run, ignore_errors=True)
    findings = []
    for s in scen_all:
        m = audits.get(s['name'])
        if not m or not m.get('frame'): findings.append({'rule': 'no-frame', 'scenario': s['name'], 'text': s['name'], 'detail': 'the client reported no audited frame for this screen'}); continue
        findings += check.check(m['frame'], dict(s, name=s['name']))
    with open(os.path.join(d, 'audit.jsonl'), 'w') as f:
        for m in audits.values(): f.write(json.dumps(m) + '\n')
    new, old = check.split_known(findings, KNOWN)
    json.dump({'size': size['name'], 'logical': [lw, lh], 'text_pct': pct, 'new': new, 'known': old, 'screens': len(audits)}, open(os.path.join(d, 'findings.json'), 'w'), indent=1)
    print('%-22s t%-3d %2d/%d screens  %d new, %d known' % (size['name'], pct, len(audits), len(scen_all), len(new), len(old)), flush=True)
    for f in new[:12]: print('   FAIL %-9s %-14s %s: %s' % (f['rule'], f['scenario'], f['text'][:60], f['detail']), flush=True)
    return {'size': size['name'], 'logical': [lw, lh], 'text_pct': pct, 'new': new, 'known': old, 'screens': len(audits)}

report = []
for k, (s, p) in enumerate(combos):
    report.append(run_one(s, p, A.port + k % 5, A.display + k % 5))
json.dump(report, open(P('report.json'), 'w'), indent=1)
# contact sheet per size: the screens of the 100% run (or the first run) as thumbnails, titled
try:
    from PIL import Image, ImageDraw
    for s in sizes:
        runs = [r for r in report if r['size'] == s['name']]
        if not runs: continue
        r = next((x for x in runs if x['text_pct'] == 100), runs[0]); shots = sorted(glob.glob(P('%s-t%d' % (s['name'], r['text_pct']), 'shots', '*.png')))
        if not shots: continue
        cols = 4; tw = 360; ims = []
        for f in shots:
            im = Image.open(f).convert('RGB'); th = max(1, round(im.height * tw / im.width)); im = im.resize((tw, th)); ImageDraw.Draw(im).text((4, 3), os.path.basename(f)[:-4], fill=(255, 255, 0)); ims.append(im)
        rowh = max(i.height for i in ims); rows_n = (len(ims) + cols - 1) // cols; sheet = Image.new('RGB', (cols * tw, rows_n * rowh), (20, 20, 20))
        for i, im in enumerate(ims): sheet.paste(im, ((i % cols) * tw, (i // cols) * rowh))
        sheet.save(P('contact-%s.png' % s['name']))
except Exception as e: print('contact sheet: %s' % e)
L = ['## Size matrix on %s' % A.platform, '', '| size (logical) | text | screens | new | known |', '|---|---|---|---|---|']
for r in report: L.append('| %s (%dx%d) | %d%% | %d/%d | %d | %d |' % (r['size'], *r['logical'], r['text_pct'], r['screens'], len(scen_all), len(r['new']), len(r['known'])))
L.append('')
for r in report:
    for f in r['new']: L.append('- **%s t%d** `%s` %s: %s (%s)' % (r['size'], r['text_pct'], f['rule'], f['scenario'], f['text'][:70], f['detail']))
open(P('report.md'), 'w').write('\n'.join(L) + '\n')
if A.allow_known:
    kn = dict(KNOWN)
    for r in report:
        for f in r['new']: kn[check.key(f)] = 'seen on %s %s t%d' % (A.platform, r['size'], r['text_pct'])
    json.dump(kn, open(os.path.join(HERE, 'known.json'), 'w'), indent=1, sort_keys=True)
sys.exit(1 if any(r['new'] for r in report) and not A.allow_known else 0)
