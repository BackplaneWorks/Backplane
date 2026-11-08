#!/usr/bin/env python3
"""Real keys (XTest) against the model picker (0.14's three columns: agent, model, effort meter) and the composer's popup; run by scripts/desktop-picker-keys.sh with DISPLAY set to a private Xvfb.
Judged from outside the drawing: the client's GPUI_METRIC lines (`pick`, `popup`) and what reached the hub's event log. Screenshots land in $R/shots."""
import json, os, subprocess, sys, time
R, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
bad = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: bad.append(what)
def x(*a): subprocess.run(['python3', XIN, *a], check=True)
def shot(n): subprocess.run(['import', '-window', 'root', f'{R}/shots/{n}.png'], check=False)
def metrics(ev):
    out = []
    for l in open(f'{R}/client.log', errors='replace'):
        if l.startswith('GPUI_METRIC '):
            try: m = json.loads(l[12:])
            except ValueError: continue
            if m.get('event') == ev: out.append(m)
    return out
def wait_new(ev, n, s=3.0):
    t = time.time()
    while time.time() - t < s:
        m = metrics(ev)
        if len(m) > n: time.sleep(0.05); return metrics(ev)[-1]
        time.sleep(0.03)
    return None
def press(*keys, ev='pick'):
    n = len(metrics(ev)); x('chord', *keys) if len(keys) > 1 else x('key', keys[0]); return wait_new(ev, n)
def events(): return [json.loads(l) for l in open(f'{R}/home/events.jsonl') if l.strip()]
def changes(): return [e for e in events() if e.get('$') in ('ModesSet', 'ProviderSet') or (e.get('$') == 'SettingSet' and e.get('key', '').startswith('effort.'))]
def settle(n, s=4.0):
    t = time.time()
    while time.time() - t < s and len(changes()) <= n: time.sleep(0.05)
    time.sleep(0.3); return changes()[n:]

# ---- the menu opens on the thread's own, in the model column
c = press('ctrl', 'm'); shot('01-open')
check(c and c['col'] == 'model' and c['model'] == 'opus' and c['provider'] == '@claude', f'Ctrl+M opens on the own model: {c}')
# ---- Down and Up go round the rows
seen = [c['model']]
for _ in range(5): seen.append(press('Down')['model'])
check(seen == ['opus', 'sonnet', 'haiku', '', 'fable', 'opus'], f'Down walks and wraps: {seen}')
up = press('Up'); check(up['model'] == 'fable', f'Up comes back: {up["model"]}')
up = press('Up'); check(up['model'] == '', 'Up reaches default'); up = press('Up'); check(up['model'] == 'haiku', f'Up over the top wraps: {up["model"]}')
press('Down'); press('Down'); press('Down')   # back on opus
# ---- Tab and Shift+Tab go round the columns
t = [press('Tab')['col'], press('Tab')['col'], press('Tab')['col']]; check(t == ['effort', 'provider', 'model'], f'Tab order: {t}')
t = [press('shift', 'Tab')['col'], press('shift', 'Tab')['col'], press('shift', 'Tab')['col']]; check(t == ['provider', 'effort', 'model'], f'Shift+Tab order: {t}')
# ---- Left and Right step the effort from the model column, with the search empty, and stop at the ends
e0 = metrics('pick')[-1]['effort']
l = press('Right'); check(l['col'] == 'model', f'Right keeps the column: {l}'); shot('02-effort')
vals = [press('Left')['effort'] for _ in range(7)]
check(vals[-1] == vals[-2] == '', f'Left stops at auto: {vals}')
# ---- in the effort column (0.14's meter) Up is more effort and Down less, stopping at the ends
press('Tab'); d = press('Down'); u = press('Up'); shot('03-meter')
check(d['col'] == 'effort' and u['effort'] != d['effort'], f'Up and Down step the meter: {d} {u}')
press('shift', 'Tab')
# ---- Esc shuts and applies nothing; the keyboard goes back to the composer
n = len(changes()); x('key', 'Escape'); time.sleep(0.5); shot('04-esc')
check(len(changes()) == n, 'Esc applies nothing')
# ---- search: the first match, the side arrows stay with the text, Enter takes it
c = press('ctrl', 'm'); check(c['model'] == 'opus' and c['col'] == 'model', f'reopened on the own, the moved cursor gone: {c}')
n = len(metrics('pick')); x('type', 'son'); c = wait_new('pick', n); time.sleep(0.3); c = metrics('pick')[-1]
check(c['model'] == 'sonnet' and c['col'] == 'model', f'search goes to the first match: {c}'); shot('05-search')
n = len(metrics('pick')); x('key', 'Left'); time.sleep(0.3); check(len(metrics('pick')) == n, 'Left in a search with text moves the caret, not the effort')
n = len(changes()); x('key', 'Return'); got = settle(n); shot('06-enter')
check(any(e.get('$') == 'ModesSet' and e.get('model') == 'sonnet' for e in got), f'Enter applied the model: {got}')
# ---- the cursor across columns: one Enter applies model and effort
c = press('ctrl', 'm'); check(c['model'] == 'sonnet', f'the hub picture came back: {c}')
press('Down'); press('Tab'); press('Down')
n = len(changes()); x('key', 'Return'); got = settle(n, 2.0); time.sleep(0.8); got = changes()[n:]
check(any(e.get('model') == 'haiku' for e in got) and any(e.get('$') == 'SettingSet' for e in got), f'one Enter: model and effort: {got}')
# ---- another agent: one switch, with its model and effort
press('ctrl', 'm'); press('shift', 'Tab'); c = press('Down'); check(c['provider'] == '@codex' and c['model'] == '', f'Down on the provider column: its default model: {c}'); shot('07-provider')
n = len(changes()); x('key', 'Return'); got = settle(n); shot('08-switched')
check(any(e.get('$') == 'ProviderSet' and e.get('provider') == 'codex' for e in got), f'Enter switched the agent: {got}')
# back on Claude for the mention
press('ctrl', 'm'); press('shift', 'Tab'); c = press('Up'); check(c['provider'] == '@claude', f'Up back: {c}')
n = len(changes()); x('key', 'Return'); settle(n); time.sleep(0.5)
base = [dict(e) for e in changes()]
# ---- the & popup: the keyboard is back in the composer
n = len(metrics('popup')); x('type', '&'); time.sleep(1.2); shot('09-amp')
r = [press('Down', ev='popup') for _ in range(4)]
check(all(r) and [m['ix'] for m in r] == [1, 2, 3, 0], f'Down in the & popup wraps: {r}')
u = press('Up', ev='popup'); check(u and u['ix'] == 3, f'Up wraps: {u}'); shot('10-amp-cursor')
x('key', 'Escape'); time.sleep(0.5); shot('11-amp-esc')
n = len(metrics('popup')); x('key', 'Down'); time.sleep(0.4); check(len(metrics('popup')) == n, 'Esc shut the popup: Down is the text box again')
x('key', 'BackSpace'); time.sleep(0.6)
x('type', '&op'); time.sleep(1.2); x('key', 'Return'); time.sleep(1.0); shot('12-amp-picked')
x('key', 'Return'); time.sleep(2.5); shot('13-sent')
msgs = [e for e in events() if e.get('$') == 'MessagePosted' and e.get('role') == 'user' and '&claude/opus' in json.dumps(e)]
check(len(msgs) == 1, f'Enter took the row and the next Enter sent it: {[m.get("text") for m in msgs]}')
check(changes() == base, 'a & mention changes no model')
# ---- the popup scrolls: the slash commands run past its eight rows
x('type', '/'); time.sleep(1.2)
rows = press('Up', ev='popup'); shot('14-slash-end'); print('slash rows', rows)
check(rows and rows['rows'] > 0 and rows['ix'] == rows['rows'] - 1, f'Up from the first goes to the last: {rows}')
x('key', 'Escape'); x('key', 'BackSpace')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
