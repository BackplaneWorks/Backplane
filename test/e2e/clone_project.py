#!/usr/bin/env python3
"""Clone from git in the desktop's Add project with real keys and clicks (XTest); run by scripts/desktop-clone-project.sh with DISPLAY set to a
private Xvfb. Judged from outside the drawing: the hub's event log, the clones under the temp HOME, and the client's controls and rects.
Screenshots land in $R/shots."""
import json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
HOME = f'{R}/fakehome'; DOCS = f'{HOME}/Documents'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *a], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def events(): return [json.loads(l) for l in open(f'{R}/home/events.jsonl') if l.strip()]
def metrics(ev):
    out = []
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_METRIC ') and f'"event":"{ev}"' in l:
            try: out.append(json.loads(l[12:]))
            except ValueError: pass
    return out
def controls():
    m = metrics('controls'); return m[-1]['controls'] if m else []
def rects():
    r = {c['id']: c['rect'] for c in controls() if c.get('rect')}
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_RECT '):
            try: q = json.loads(l[10:])
            except ValueError: continue
            if q.get('name'): r[q['name']] = q
    return r
def wait(cond, s=10.0):
    t = time.time()
    while time.time() - t < s:
        v = cond()
        if v: return v
        time.sleep(0.15)
    return cond()
def click(rid, s=8.0):
    r = wait(lambda: rects().get(rid), s)
    if not r: check(False, f'{rid} is drawn'); return False
    x('click', str(int(r['x'] + r['w'] / 2)), str(int(r['y'] + r['h'] / 2))); time.sleep(0.5); return True
def project(root): return next((e for e in events() if e.get('$') == 'ProjectCreated' and e.get('root', '').rstrip('/') == root), None)
def rows(): return [c for c in controls() if c.get('id', '').startswith('folder-')]
def row(prefix): return next((c for c in rows() if (c.get('label') or '').startswith(prefix)), None)
def row_into(name): return next((c for c in rows() if (c.get('label') or '').startswith('Clone ') and ' into ~/Documents/' + name in (c.get('label') or '')), None)
def picker(): return any(c.get('id') == 'folders-cancel' for c in controls())
def opened(th): return any('"view_open"' in l and th in l for l in open(f'{R}/client.log', errors='replace'))
def open_dialog():
    x('click', '800', '788'); time.sleep(0.4); x('chord', 'ctrl', 'a'); x('key', 'BackSpace')
    x('type', '/project'); time.sleep(0.6); x('key', 'Escape'); x('key', 'Return')
    return wait(picker)
KIT, DUO = f'file://{R}/fx/kit.git', f'file://{R}/fx/duo.git'

# ---- Add project, Clone from git: the address typed, then where (the default: ~/Documents/<name>), then Clone; the window goes to its thread
check(open_dialog(), 'Add project opens'); shot('01-dialog')
check(click('proj-tab-1'), 'the Clone from git tab'); time.sleep(0.6); shot('02-clone-tab')
x('type', KIT)
r = wait(lambda: row('Clone '))
check(bool(r) and 'kit' in r['label'], f'the address is offered: {r}')
x('key', 'Return')
r = wait(lambda: row_into('kit'))
check(bool(r) and r['label'].endswith('~/Documents/kit'), f'the default destination: {r}'); time.sleep(0.6); shot('03-destination')
x('key', 'Return')
p = wait(lambda: project(f'{DOCS}/kit'), 30)
check(p is not None and os.path.isfile(f'{DOCS}/kit/boards/kit/kit.kicad_pro'), f'cloned into ~/Documents/kit and added: {p}')
th = p and wait(lambda: next((e for e in events() if e.get('$') == 'ThreadCreated' and e.get('project') == p['id']), None))
check(bool(th), f'its first thread: {th}')
check(bool(th) and wait(lambda: opened(th['id']), 12), 'the window went to the new thread'); check(wait(lambda: not picker()), 'the dialog closed'); time.sleep(1); shot('04-opened')

# ---- a folder that holds something: refused in words, nothing cloned
check(open_dialog(), 'Add project again'); click('proj-tab-1'); x('type', KIT); wait(lambda: row('Clone ')); x('key', 'Return')
r = wait(lambda: row_into('kit'))
check(bool(r) and r['label'].endswith('~/Documents/kit-2'), f'the next free name by default: {r}')
x('chord', 'ctrl', 'a'); x('type', '~/Documents/kit')
r = wait(lambda: row('~/Documents/kit already exists'))
check(bool(r) and "isn't empty" in r['label'], f'a folder that holds something is refused: {r}'); time.sleep(0.6); shot('05-refused')
x('key', 'Escape'); time.sleep(0.6)

# ---- /project <address> <message>: the destination step at once; two KiCad projects: notes first (.backplane.json set), Open goes to the thread with the message
n = len(events())
x('click', '800', '788'); time.sleep(0.4); x('type', f'/project {DUO} lay out the aux board'); time.sleep(0.5); x('key', 'Return')
r = wait(lambda: row_into('duo'))
check(bool(r), f'/project <address> opens the destination step: {r}'); shot('06-slash')
x('key', 'Return')
done = wait(lambda: any(c.get('id') == 'clone-act-0' and 'Open' in (c.get('label') or '') for c in controls()) or rects().get('clone-act-0'), 30)
check(bool(done), 'done, with notes to read: Open'); time.sleep(0.8); shot('07-notes')
bj = f'{DOCS}/duo/.backplane.json'
check(os.path.isfile(bj) and json.load(open(bj)).get('project') == 'boards/duo/duo.kicad_pro', f'.backplane.json names the one named like the repository: {os.path.isfile(bj) and open(bj).read()}')
click('clone-act-0')
p = wait(lambda: project(f'{DOCS}/duo'))
th = p and next((e for e in events() if e.get('$') == 'ThreadCreated' and e.get('project') == p['id']), None)
check(bool(th) and wait(lambda: opened(th['id']), 12), 'Open went to its thread')
m = th and wait(lambda: next((e for e in events()[n:] if e.get('$') == 'MessagePosted' and e.get('thread') == th['id'] and e.get('role') == 'user'), None), 15)
check(bool(m) and m.get('text') == 'lay out the aux board', f'the rest of /project is its first message: {m and m.get("text")}')

# ---- a clone that hangs: Cancel stops it and leaves no folder
port = open(f'{R}/hang.port').read().strip()
check(open_dialog(), 'Add project once more'); click('proj-tab-1'); x('type', f'http://127.0.0.1:{port}/o/hang.git'); wait(lambda: row('Clone ')); x('key', 'Return')
wait(lambda: row_into('hang')); x('key', 'Return')
check(bool(wait(lambda: rects().get('clone-act-0'), 10)), 'the clone runs, with Cancel'); time.sleep(1.0); shot('08-running')
click('clone-act-0')
check(bool(wait(lambda: rects().get('clone-act-1'), 10)), 'stopped: Try again and Back'); shot('09-cancelled')
check(not os.path.exists(f'{DOCS}/hang'), 'the folder it made is gone')
check(not project(f'{DOCS}/hang'), 'nothing was added')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
