#!/usr/bin/env python3
"""The size matrix for the Android app (docs/e2e/SIZES.md): one run per (size, orientation, font scale) on an emulator, each walking the screens in android_scenarios.json,
judging the view tree (`uiautomator dump --windows`) with android_check.py and keeping a screenshot per screen. Findings have check.py's shape, so the report is the desktop's.
usage: android.py --out DIR [--serial emulator-5586] [--apk APK] [--hub backplane-hub] [--fixture fixture.json] [--sizes small,regular,..] [--orient portrait,landscape]
                  [--font 0.85,1.0,1.3] [--only scenario,..] [--quick] [--shard i/n] [--allow-known] [--pkg dev.backplane.mobile.gpui] [--scheme bpgpui]
  The emulator is the caller's (scripts/android-emu.sh start, or reactivecircus/android-emulator-runner in CI); this changes `wm size`, `wm density`, the rotation and
  `font_scale` of it and puts all four back. With --hub it starts the fixture hub (android-fixture.mjs: stand-in agents, temp home) and stops it; with --fixture it uses a running one.
  --apk is installed when given. The hub is reached as 10.0.2.2 (the emulator's name for this machine).
Writes OUT/<size>-<orient>-f<font>/{shots/*.png,findings.json,trees/*.xml}, OUT/report.json, OUT/report.md, OUT/contact-<size>.png. Exit 1 when a finding is new (not in android-known.json)."""
import os, sys, re, json, time, shutil, subprocess, argparse, glob, urllib.request, tempfile
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
sys.path.insert(0, HERE)
import android_check as ck

ap = argparse.ArgumentParser()
ap.add_argument('--out', required=True); ap.add_argument('--serial', default=os.environ.get('ANDROID_SERIAL', 'emulator-5554')); ap.add_argument('--apk', default='')
ap.add_argument('--hub', default=''); ap.add_argument('--fixture', default=''); ap.add_argument('--port', type=int, default=3883)
ap.add_argument('--sizes', default=''); ap.add_argument('--orient', default=''); ap.add_argument('--font', default='0.85,1.0,1.3'); ap.add_argument('--only', default='')
ap.add_argument('--quick', action='store_true', help='regular portrait 1.0 and small portrait 1.3'); ap.add_argument('--shard', default='')
ap.add_argument('--pages', type=int, default=4, help='most pages a scrolling screen is walked'); ap.add_argument('--allow-known', action='store_true'); ap.add_argument('--pkg', default='dev.backplane.mobile.gpui'); ap.add_argument('--scheme', default='bpgpui')
ap.add_argument('--adb', default=os.environ.get('ADB') or shutil.which('adb') or os.path.expanduser('~/Android-sdk-writable/platform-tools/adb'))
A = ap.parse_args()
OUT = os.path.abspath(A.out); os.makedirs(OUT, exist_ok=True)
SZ = json.load(open(os.path.join(HERE, 'sizes.json')))['phones']; SC = json.load(open(os.path.join(HERE, 'android_scenarios.json')))['scenarios']
KNP = os.path.join(HERE, 'android-known.json'); KNOWN = json.load(open(KNP)) if os.path.exists(KNP) else {}
scen_all = [s for s in SC if not A.only or s['name'] in A.only.split(',')]
sizes = [s for s in SZ['android'] if not A.sizes or s['name'] in A.sizes.split(',')]
orients = [o for o in SZ['orientations'] if not A.orient or o in A.orient.split(',')]
fonts = [float(f) for f in A.font.split(',')]
combos = [(s, o, f) for s in sizes for o in orients for f in fonts]
if A.quick: combos = [c for c in combos if (c[0]['name'], c[1], c[2]) in (('regular', 'portrait', 1.0), ('small', 'portrait', 1.3))]
if A.shard:
    i, n = map(int, A.shard.split('/')); combos = [c for k, c in enumerate(combos) if k % n == i]

def adb(*a, t=60, text=True):
    r = subprocess.run([A.adb, '-s', A.serial, *a], capture_output=True, text=text, timeout=t)
    return r.stdout if text else r.stdout
def sh(c, t=60): return adb('shell', c, t=t)
def dump():
    for _ in range(4):
        x = adb('exec-out', 'uiautomator', 'dump', '--windows', '/dev/tty', t=60)
        if x and '<displays' in x: return x[x.index('<?xml'):x.rindex('</displays>') + len('</displays>')] if '<?xml' in x else x[x.index('<displays'):x.rindex('</displays>') + 11]
        time.sleep(0.8)
    return ''
def shot(path):
    b = subprocess.run([A.adb, '-s', A.serial, 'exec-out', 'screencap', '-p'], capture_output=True, timeout=60).stdout
    if b[:4] == b'\x89PNG': open(path, 'wb').write(b)

STATE = {'size': None, 'dpi': 160}
def tree():
    x = dump()
    return x, (ck.parse(x, A.pkg, STATE['dpi'], STATE['size']) if x else None)
def app_nodes(t):
    return [n for w in t['windows'] if w['pkg'] == A.pkg for n in w['nodes']] if t else []
def find(t, rx, last=True):
    ms = [n for n in app_nodes(t) if n['text'] and re.search(rx, n['text']) or (rx == 'EditText' and n.get('edit'))]
    return (ms[-1] if last else ms[0]) if ms else None
def tap_node(n): sh('input tap %d %d' % ((n['b'][0] + n['b'][2]) // 2, (n['b'][1] + n['b'][3]) // 2))
def foreground():
    return A.pkg in sh('dumpsys activity activities | grep -m1 topResumedActivity')
def launch():
    sh('monkey -p %s -c android.intent.category.LAUNCHER 1' % A.pkg); time.sleep(2.5)

def hub_link():
    req = urllib.request.Request('http://127.0.0.1:%d/auth/invitations' % A.port, method='POST', headers={'Authorization': 'Bearer ' + FX['token']})
    link = json.loads(urllib.request.urlopen(req, timeout=10).read())['link']
    return link.replace('127.0.0.1', '10.0.2.2').replace('localhost', '10.0.2.2')
def open_link(kind, arg=None):
    u = '%s://%s' % (A.scheme, kind) + ('?url=' + urllib.parse.quote(arg, safe='') if arg else '')
    sh("am start -W -a android.intent.action.VIEW -d '%s' %s" % (u, A.pkg)); time.sleep(1.5)

def settle(fast=False):
    """wait until two dumps agree (an animation or a load has ended)"""
    prev = None
    if not fast: return dump()
    for _ in range(6):
        x = dump(); k = re.sub(r'\s+', '', x)
        if k == prev: return x
        prev = k; time.sleep(0.5)
    return x

def to_home():
    """paired, on the thread list: back out of anything, relaunch when the app left"""
    for _ in range(5):
        if 'mInputShown=true' in sh('dumpsys input_method | grep -m1 mInputShown'): sh('input keyevent KEYCODE_BACK'); time.sleep(0.6)   # a keyboard left up covers the list
        x, t = tree()
        if t and find(t, r'^Add project$') and find(t, r'^Settings$'): return t
        if not foreground(): launch(); continue
        sh('input keyevent KEYCODE_BACK'); time.sleep(0.8)
    launch(); return tree()[1]

def run_steps(steps):
    for st in steps:
        if 'tap' in st:
            for k in range(7):
                x, t = tree(); n = find(t, st['tap'])
                if n: tap_node(n); break
                if k == 2: [swipe_down() or time.sleep(0.3) for _ in range(3)]   # a list left scrolled by the last screen: back to its top
                elif k >= 4: swipe_up()   # a row below the fold
                time.sleep(0.6)
            else: return 'control not found: ' + st['tap']
            time.sleep(0.3)
        elif 'wait' in st: time.sleep(st['wait'] / 1000)
        elif 'type' in st: sh("input text '%s'" % st['type'].replace(' ', '%s').replace("'", '')); time.sleep(0.4)
        elif 'hide_keyboard' in st: sh('input keyevent KEYCODE_BACK'); time.sleep(0.6)
        elif 'link' in st: open_link('pair', hub_link())
        elif 'scroll' in st: pass
    return None

def swipe_down():
    W, H = STATE['size']; sh('input swipe %d %d %d %d 300' % (W // 2, H * 0.55, W // 2, H * 0.95))   # inside the list: a drag that starts on the header does not scroll it
def swipe_up():
    W, H = STATE['size']; sh('input swipe %d %d %d %d 300' % (W // 2, H * 0.9, W // 2, H * 0.45))

def run_combo(size, orient, font):
    tag = '%s-%s-f%g' % (size['name'], orient, font); d = os.path.join(OUT, tag); shutil.rmtree(d, ignore_errors=True)
    for x in ('shots', 'trees'): os.makedirs(os.path.join(d, x))
    pw, ph = round(size['w'] * size['density'] / 160), round(size['h'] * size['density'] / 160)
    sh('wm size %dx%d' % (pw, ph)); sh('wm density %d' % size['density'])
    sh('cmd window user-rotation lock %d' % (1 if orient == 'landscape' else 0))   # `settings put system user_rotation` does nothing on a headless emulator
    sh('settings put system font_scale %g' % font)
    STATE['dpi'] = size['density']; STATE['size'] = (ph, pw) if orient == 'landscape' else (pw, ph)
    sh('am force-stop ' + A.pkg); time.sleep(1); launch()
    for _ in range(6):
        rot = re.search(r'hierarchy rotation="(\d)"', dump() or '')
        if rot and (rot.group(1) in ('1', '3')) == (orient == 'landscape'): break
        sh('cmd window user-rotation lock %d' % (1 if orient == 'landscape' else 0)); time.sleep(1.5)
    if not rot or (rot.group(1) in ('1', '3')) != (orient == 'landscape'): sys.exit('the emulator did not rotate to %s (hierarchy rotation %s): its results would be wrong' % (orient, rot and rot.group(1)))
    findings, seen, paired = [], set(), False
    for sc in scen_all:
        t0 = time.time()
        if sc.get('pre') == 'unpair':
            open_link('unpair'); time.sleep(0.8); paired = False
        else:
            if not paired:
                open_link('pair', hub_link()); x, t = tree(); n = find(t, r'^Pair$'); n and tap_node(n); time.sleep(4); paired = True
            to_home()
        err = run_steps(sc['steps'])
        x = settle(sc.get('settle', False))
        t = ck.parse(x, A.pkg, STATE['dpi'], STATE['size']) if x else None
        if err or not t:
            findings.append({'rule': 'no-frame', 'scenario': sc['name'], 'text': sc['name'], 'detail': err or 'no view tree'}); continue
        open(os.path.join(d, 'trees', sc['name'] + '.xml'), 'w').write(x); shot(os.path.join(d, 'shots', sc['name'] + '.png'))
        pages = [t]
        if sc.get('scroll'):
            key = lambda p: [(n['text'], n['b']) for n in app_nodes(p)]   # the app's own nodes: the status bar's clock changes
            last = key(t)
            for _ in range(A.pages - 1):
                swipe_up(); time.sleep(0.7); x2 = dump(); p2 = ck.parse(x2, A.pkg, STATE['dpi'], STATE['size']) if x2 else None
                if not p2 or key(p2) == last: break
                last = key(p2); pages.append(p2)
        for k, pg in enumerate(pages):
            for f in ck.check(pg, dict(sc, name=sc['name'], text=[])):
                key = (f['rule'], f['scenario'], f['text'])
                if key not in seen: seen.add(key); findings.append(f)
        have = ' | '.join(n['text'].lower() for pg in pages for n in app_nodes(pg))   # a text may be on any page of a scrolling screen
        for want in sc.get('text', []):
            if want.lower() not in have: findings.append({'rule': 'missing', 'scenario': sc['name'], 'text': want, 'detail': 'text not in the view tree'})
        print('   %-22s %2d pages %4.1fs' % (sc['name'], len(pages), time.time() - t0), flush=True)
    new, old = ck.split_known(findings, KNOWN)
    json.dump({'size': size['name'], 'orient': orient, 'font': font, 'px': [pw, ph], 'dp': [size['w'], size['h']], 'new': new, 'known': old, 'screens': len(scen_all)}, open(os.path.join(d, 'findings.json'), 'w'), indent=1)
    print('%-8s %-9s f%-4g %dx%d dp %d new, %d known' % (size['name'], orient, font, size['w'], size['h'], len(new), len(old)), flush=True)
    for f in new[:10]: print('   FAIL %-8s %-14s %s: %s' % (f['rule'], f['scenario'], f['text'][:50], f['detail']), flush=True)
    return {'size': size['name'], 'orient': orient, 'font': font, 'dp': [size['w'], size['h']], 'new': new, 'known': old, 'screens': len(scen_all)}

FX = {}; procs = []
def start_fixture():
    if A.fixture: FX.update(json.load(open(A.fixture))); A.port = int(FX['url'].rsplit(':', 1)[1]); return
    if not A.hub or not os.path.isfile(A.hub): sys.exit('--hub (an existing backplane-hub) or --fixture')
    root = tempfile.mkdtemp(prefix='andfx.', dir=os.path.expanduser('~/.cache/bp-tmp') if os.path.isdir(os.path.expanduser('~/.cache/bp-tmp')) else None)
    p = subprocess.Popen([shutil.which('bun') or os.path.expanduser('~/.bun/bin/bun'), os.path.join(HERE, 'android-fixture.mjs'), '--hub-bin', os.path.abspath(A.hub), '--root', root + '/fx', '--port', str(A.port), '--threads', '6'],
                         cwd=REPO, env=dict(os.environ, PERF_REPO=REPO), stdout=open(root + '/fx.log', 'w'), stderr=subprocess.STDOUT); procs.append((p, root))
    for _ in range(180):
        if os.path.exists(root + '/fx/fixture.json'): break
        if p.poll() is not None: sys.exit('fixture hub failed: ' + open(root + '/fx.log').read()[-500:])
        time.sleep(1)
    else: sys.exit('fixture hub not ready in 180 s: ' + open(root + '/fx.log').read()[-500:])
    FX.update(json.load(open(root + '/fx/fixture.json')))
    A.port = int(FX['url'].rsplit(':', 1)[1])

import urllib.parse
report = []
try:
    start_fixture()
    if A.apk:
        print(subprocess.run([A.adb, '-s', A.serial, 'install', '-r', '-g', A.apk], capture_output=True, text=True).stdout.strip()[-60:])
    sh('pm grant %s android.permission.POST_NOTIFICATIONS' % A.pkg); sh('cmd uimode night no')
    for s, o, f in combos:
        report.append(run_combo(s, o, f))
finally:
    for c in ('wm size reset', 'wm density reset', 'settings put system font_scale 1.0', 'cmd window user-rotation free'): sh(c)
    for p, root in procs:
        p.terminate()
        try: p.wait(15)
        except Exception: p.kill()
        shutil.rmtree(root, ignore_errors=True)
json.dump(report, open(os.path.join(OUT, 'report.json'), 'w'), indent=1)
try:
    from PIL import Image, ImageDraw
    for s in sizes:
        runs = [r for r in report if r['size'] == s['name']]
        if not runs: continue
        r = next((x for x in runs if x['orient'] == 'portrait' and x['font'] == 1.0), runs[0]); shots = sorted(glob.glob(os.path.join(OUT, '%s-%s-f%g' % (s['name'], r['orient'], r['font']), 'shots', '*.png')))
        if not shots: continue
        ims = []; th = 480
        for f in shots:
            im = Image.open(f).convert('RGB'); tw = max(1, round(im.width * th / im.height)); im = im.resize((tw, th)); ImageDraw.Draw(im).text((4, 3), os.path.basename(f)[:-4], fill=(255, 0, 0)); ims.append(im)
        cols = 8; tw = max(i.width for i in ims); rows = (len(ims) + cols - 1) // cols; sheet = Image.new('RGB', (cols * tw, rows * th), (20, 20, 20))
        for k, im in enumerate(ims): sheet.paste(im, ((k % cols) * tw, (k // cols) * th))
        sheet.save(os.path.join(OUT, 'contact-%s.png' % s['name']))
except Exception as e: print('contact sheet: %s' % e)
L = ['## Size matrix on Android', '', '| size (dp) | orientation | font | screens | new | known |', '|---|---|---|---|---|---|']
for r in report: L.append('| %s (%dx%d) | %s | %g | %d | %d | %d |' % (r['size'], *r['dp'], r['orient'], r['font'], r['screens'], len(r['new']), len(r['known'])))
L.append('')
for r in report:
    for f in r['new']: L.append('- **%s %s f%g** `%s` %s: %s (%s)' % (r['size'], r['orient'], r['font'], f['rule'], f['scenario'], f['text'][:70], f['detail']))
open(os.path.join(OUT, 'report.md'), 'w').write('\n'.join(L) + '\n')
if A.allow_known:
    kn = dict(KNOWN)
    for r in report:
        for f in r['new']: kn['%s|%s' % (f['rule'], f['text'])] = 'seen on android %s %s f%g' % (r['size'], r['orient'], r['font'])
    json.dump(kn, open(KNP, 'w'), indent=1, sort_keys=True)
sys.exit(1 if any(r['new'] for r in report) and not A.allow_known else 0)
