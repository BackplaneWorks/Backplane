#!/usr/bin/env python3
"""A bot chat's model menu on the real window (private Xvfb, XTest keys): Bub's chat lists the provider's real models (not only "default"), another agent's list shows on
moving to it, a model picked is recorded on the bot's chat thread and the effort too. Judged by the client's `pick` metrics and the hub's event log. Screenshots: $R/shots."""
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
    time.sleep(0.4); return changes()[n:]
time.sleep(2)
c = press('ctrl', 'm'); shot('01-bot-menu')
check(c and c['provider'] == '@claude', f'the menu opens on the bot chat: {c}')
seen = [c['model']]
for _ in range(3): seen.append(press('Down')['model'])
shot('02-bot-models')
check(len(set(seen)) > 1, f'claude lists more than "default" in a bot chat: {seen}')
n = len(changes()); x('key', 'Return'); got = settle(n); shot('03-picked')
check(any(e.get('$') == 'ModesSet' and e.get('id', e.get('thread')) in ('tb1', None) and e.get('model') for e in got), f'the model picked is recorded on the bot thread: {got}')
# another agent: its models show on moving to it
press('ctrl', 'm'); press('shift', 'Tab'); c = press('Down'); shot('04-codex')
check(c and c['provider'] == '@codex', f'the agent column moves to codex: {c}')
# the stand-in codex lists no models of its own (the unit test covers every agent's list): the switch itself is what a bot chat must apply
n = len(changes()); x('key', 'Return'); got = settle(n); shot('06-codex-picked')
check(any(e.get('$') == 'ProviderSet' and e.get('provider') == 'codex' for e in got), f'the switch to codex is recorded: {got}')
print('FAILED: ' + '; '.join(bad) if bad else 'ALL PASS'); sys.exit(1 if bad else 0)
