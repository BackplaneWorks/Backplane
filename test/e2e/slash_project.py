#!/usr/bin/env python3
"""/project in the desktop composer with real keys (XTest); run by scripts/desktop-slash-project.sh with DISPLAY set to a private Xvfb.
Judged from outside the drawing: the hub's event log, the folders made under the temp HOME, and the client's `controls` metric (the Add project
dialog's rows). Screenshots land in $R/shots (01-autocomplete is the `/` list filtered to /project)."""
import json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
HOME = f'{R}/fakehome'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *a], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def events(): return [json.loads(l) for l in open(f'{R}/home/events.jsonl') if l.strip()]
def controls():
    last = None
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_METRIC ') and '"event":"controls"' in l: last = l
    return json.loads(last[12:])['controls'] if last else []
def wait(cond, s=8.0):
    t = time.time()
    while time.time() - t < s:
        v = cond()
        if v: return v
        time.sleep(0.1)
    return cond()
def project(root): return next((e for e in events() if e.get('$') == 'ProjectCreated' and e.get('root') == root), None)
def clear(): x('chord', 'ctrl', 'a'); x('key', 'BackSpace'); time.sleep(0.4)
def picker(): return any(c.get('id') == 'folders-cancel' for c in controls())

# ---- the / list offers it, with its description (the shot the report shows)
x('type', '/pro'); time.sleep(1.5); shot('01-autocomplete')
clear()
# ---- a bare name: made under the home folder (where Add project starts), its first thread opened with the text as its first message
x('type', '/project puck Build a sensor puck'); time.sleep(0.6); x('key', 'Return')
p = wait(lambda: project(f'{HOME}/puck')); check(p is not None and os.path.isdir(f'{HOME}/puck'), f'/project puck made {HOME}/puck: {p}')
th = p and next((e for e in events() if e.get('$') == 'ThreadCreated' and e.get('project') == p['id']), None)
check(th is not None, f'its first thread: {th}')
m = th and wait(lambda: next((e for e in events() if e.get('$') == 'MessagePosted' and e.get('thread') == th['id'] and e.get('role') == 'user'), None))
check(bool(m) and m.get('text') == 'Build a sensor puck', f'the text after the folder is its first message: {m and m.get("text")}')
sel = wait(lambda: any('"view_open"' in l and th['id'] in l for l in open(f'{R}/client.log', errors='replace')), 10) if th else False
check(sel, 'the window opened the new thread'); time.sleep(1.5); shot('02-made')
# ---- ~/ and absolute paths are used as given, missing folders made
x('type', f'/new-project {R}/abs/deep/board'); x('key', 'Return')
check(wait(lambda: project(f'{R}/abs/deep/board')) is not None, 'an absolute path, its missing folders made')
time.sleep(1.0)
# ---- alone: the Add project dialog, its field focused (typing lands there)
x('type', '/project'); time.sleep(0.6); x('key', 'Escape'); x('key', 'Return')
check(wait(picker), 'alone it opens the Add project dialog'); shot('03-dialog')
x('type', 'typed'); time.sleep(0.8)
check(any('typed' in (c.get('label') or '') for c in controls()), f'the dialog field has the keys: {[c.get("label") for c in controls() if c.get("id","").startswith("folder")][:4]}')
x('key', 'Escape'); time.sleep(0.8); x('click', '800', '788'); time.sleep(0.5)  # back in the composer (Esc leaves the dialog's field)
# ---- a folder that holds something: refused, the dialog opens on it, Add takes it as it is
n = len(events())
x('type', '/project full'); x('key', 'Return')
rows = wait(lambda: [c for c in controls() if c.get('id', '').startswith('folder-') and (c.get('label') or '').startswith('Add ')])
check(bool(rows) and 'full' in rows[0]['label'], f'the dialog offers to add it as it is: {rows}'); time.sleep(1.2); shot('04-not-empty')
check(not any(e.get('$') == 'ProjectCreated' for e in events()[n:]), 'nothing was made in it')
x('key', 'Return')
check(wait(lambda: project(f'{HOME}/full')) is not None, 'Add existing made it a project as it is')
check(os.listdir(f'{HOME}/full') == ['notes.txt'] or 'notes.txt' in os.listdir(f'{HOME}/full'), 'its files are untouched')
time.sleep(1.0); x('click', '800', '788'); time.sleep(0.5)
# ---- an invalid name: said inline, nothing made
clear(); n = len(events())
x('type', '/project ..'); x('key', 'Return'); time.sleep(0.4); shot('05a-invalid'); time.sleep(1.1); shot('05-invalid')
check(wait(lambda: any('"refused"' in l and 'cannot be a project' in l for l in open(f'{R}/client.log', errors='replace')), 4), 'the reason is said in the window')
check(not any(e.get('$') == 'ProjectCreated' for e in events()[n:]) and not False, 'an invalid name makes nothing')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
