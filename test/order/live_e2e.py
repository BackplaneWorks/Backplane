#!/usr/bin/env python3
"""The driver of live_e2e.sh (inside its private session bus): one buy thread end to end in the GPUI window, against the real order server
(order/fabsrv through test/order/local_server.py, its stand-in fab pack and a Stripe double, all on loopback) and the stand-in Claude.
Real X input through XTest: Buy it in the thread's header, the order_quote card's Send, then the Order card's Buy now, which a fake xdg-open
records and opens (that pays the double). The window gets no session bus, so gpui's desktop-portal open fails at once and it falls
back to xdg-open; on a person's desktop the portal opens their browser. Screenshots in OUT. With LIVE_KEEP=1 the hub and order server stay up afterwards for the phones,
their ports in OUT/rig.json, until OUT/stop exists."""
import json, os, signal, socket, subprocess, sys, time, urllib.request
CLIENT, HUB, OUT, DISP, XV, ASSETS, ROOT, PY = sys.argv[1:9]
XIN = [sys.executable, '-I', ROOT + '/test/cef/xinput.py']
procs, fails, passes = [], [], []
def check(ok, what, got=''):
    print(('PASS ' if ok else 'FAIL ') + what + ('' if ok else f' ({got})'), flush=True)
    (passes if ok else fails).append(what)
def spawn(args, log, **kw):
    p = subprocess.Popen(args, stdout=open(f'{OUT}/{log}', 'wb'), stderr=subprocess.STDOUT, **kw); procs.append(p); return p
def stop():
    for p in reversed(procs):
        if p.poll() is None: p.terminate()
    for p in procs:
        try: p.wait(5)
        except subprocess.TimeoutExpired: p.kill()
signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
def free_port():
    while True:
        s = socket.socket(); s.bind(('127.0.0.1', 0)); p = s.getsockname()[1]; s.close()
        if p >= 3799: return p
def X(*a): subprocess.run(XIN + [str(x) for x in a], check=True)
def shot(name):
    subprocess.run(['import', '-window', 'root', f'{OUT}/{name}.png'], stderr=subprocess.DEVNULL); print('shot', f'{OUT}/{name}.png', flush=True)
def metrics(name):
    out = []
    for line in open(f'{OUT}/client.log', errors='replace'):
        if line.startswith('GPUI_METRIC '):
            try: m = json.loads(line[12:])
            except ValueError: continue
            if m.get('event') == name: out.append(m)
    return out
def controls():
    c = metrics('controls')
    return {x['id']: x for x in c[-1]['controls']} if c else {}
def wait(what, f, secs=30):
    t = time.time()
    while time.time() - t < secs:
        v = f()
        if v: return v
        X('move', 3 + int(time.time() * 10) % 5, 3); time.sleep(0.25)
    shot('timeout'); raise SystemExit(f'FAIL timed out: {what}')
def rect_of(id):
    r = controls().get(id, {}).get('rect'); return r if r and r['w'] > 0 else None
def press_rect(r): X('click', int(r['x'] + r['w'] / 2), int(r['y'] + r['h'] / 2)); time.sleep(0.4)
def events():
    out = []
    for l in open(OUT + '/home/events.jsonl', errors='replace'):
        try: out.append(json.loads(l))
        except ValueError: pass
    return out
def get(url):
    try: return urllib.request.urlopen(url, timeout=3).status
    except Exception: return 0

try:
    env = dict(os.environ)
    fport, port, sport = free_port(), free_port(), free_port()
    # the order server: a small board's export first, then the server on loopback (local_server.py refuses a Stripe that is not)
    proj = OUT + '/proj'; os.makedirs(proj + '/mfg', exist_ok=True)
    subprocess.run([PY, ROOT + '/test/order/local_server.py', '--export', proj + '/mfg/1'], check=True)
    json.dump({'name': 'Main board', 'kind': 'pcb', 'source': 'board.kicad_pcb', 'qty': 5, 'options': {'layers': 2},
               'features': {'width_mm': 20, 'height_mm': 15, 'min_drill_mm': 0.3}}, open(proj + '/board.fab.json', 'w'))
    open(proj + '/board.kicad_pcb', 'w').write('(kicad_pcb (version 20240108) (generator "test"))\n')
    os.makedirs(OUT + '/fab', exist_ok=True)
    spawn([PY, ROOT + '/test/order/local_server.py', '--port', str(fport), '--stripe-port', str(sport), '--data', OUT + '/fab'], 'fab.log')
    t = time.time()
    while get(f'http://127.0.0.1:{fport}/v1/health') != 200:
        if time.time() - t > 40: raise SystemExit('FAIL the order server did not start: ' + open(OUT + '/fab.log', errors='replace').read()[-600:])
        time.sleep(0.3)
    spawn([XV + '/usr/bin/Xvfb', DISP, '-screen', '0', '1400x900x24', '-nolisten', 'tcp', '-xkbdir', '/usr/share/X11/xkb'], 'xvfb.log', env={**env, 'LD_LIBRARY_PATH': XV + '/usr/lib/x86_64-linux-gnu'})
    time.sleep(1.5)
    open(f'{OUT}/i3.conf', 'w').write(f'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket {OUT}/i3.sock\n')
    spawn(['i3', '-c', f'{OUT}/i3.conf'], 'i3.log'); time.sleep(1)
    now = int(time.time())
    ev = [{'$': 'SettingSet', 'key': 'restart.continue', 'value': 'off', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.done', 'value': '1', 'at': now}, {'$': 'SettingSet', 'key': 'onboard.uber', 'value': '1', 'at': now},
          {'$': 'ProjectCreated', 'id': 'p1', 'title': 'lamp', 'root': proj, 'at': now},
          {'$': 'ThreadCreated', 'id': 't1', 'project': 'p1', 'title': 'Lamp board', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now},
          {'$': 'MessagePosted', 'thread': 't1', 'msg': 'm1', 'role': 'user', 'text': 'The board is laid out and exported to mfg/1.', 'at': now},
          {'$': 'MessagePosted', 'thread': 't1', 'msg': 'm2', 'role': 'assistant', 'text': 'Done: board.fab.json is current and the gerbers are in mfg/1.', 'at': now}]
    open(f'{OUT}/home/events.jsonl', 'w').write(''.join(json.dumps(e, separators=(',', ':')) + '\n' for e in ev))
    calls = [{'name': 'order_quote', 'arguments': {'dir': 'mfg/1'}}]
    hub = spawn([HUB, '--home', OUT + '/home', '--port', str(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=proj,
                env={**env, 'BACKPLANE_AGENTS': OUT + '/bin', 'BACKPLANE_NO_UPDATE': '1', 'BACKPLANE_KISTACK_SYNC': '0', 'BACKPLANE_ORDER_SERVER': f'http://127.0.0.1:{fport}',
                     'FAKE_LOG': OUT + '/standin.log', 'BP_STANDIN_MODE': 'mcpcalls', 'BP_MCP_CALLS': json.dumps(calls), 'BP_MCP_DELAY_MS': '6000'})
    t = time.time()
    while not os.path.exists(OUT + '/home/token'):
        if time.time() - t > 30: raise SystemExit('FAIL the hub did not start')
        time.sleep(0.2)
    environ = open(f'/proc/{hub.pid}/environ', 'rb').read().split(b'\0')
    if (b'BACKPLANE_AGENTS=' + (OUT + '/bin').encode()) not in environ: raise SystemExit('FAIL the hub has no stand-in folder')
    json.dump([{'select': 0}, {'wait': 900000}], open(OUT + '/script.json', 'w'))
    spawn([CLIENT, '--url', f'http://127.0.0.1:{port}', '--token-file', OUT + '/home/token', '--script', OUT + '/script.json', '--exit-after', '1500'], 'client.log',
          env={**env, 'PATH': OUT + '/bin:' + env['PATH'], 'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + OUT + '/no-bus', 'BACKPLANE_GPUI_NOTIFY_CMD': OUT + '/bin/notify', 'BACKPLANE_GPUI_RECTS': '1', 'BACKPLANE_GPUI_ASSETS': ASSETS, 'VK_ICD_FILENAMES': '/usr/share/vulkan/icd.d/lvp_icd.json',
               'LP_NUM_THREADS': '2', 'LIBGL_ALWAYS_SOFTWARE': '1', 'GALLIUM_DRIVER': 'llvmpipe', 'LD_LIBRARY_PATH': ''})
    wait('the thread', lambda: metrics('usable') and 'ui:Attach' in controls(), 60); time.sleep(2)

    # 1. Buy it, in the thread's header
    r = wait('Buy it in the header', lambda: rect_of('tool:fab-buy'), 40); time.sleep(1); shot('1-buy-it')
    check(True, 'Buy it is in the thread\'s header')
    press_rect(r)
    # 2. the buy thread starts, its agent asks for the quote: the files' approval card
    buy = wait('a buy thread', lambda: next((e['id'] for e in events() if e.get('$') == 'ThreadCreated' and e['id'] != 't1'), None), 20)
    title = next(e.get('title', '') for e in events() if e.get('$') == 'ThreadCreated' and e['id'] == buy)
    check(title.startswith('Buy: lamp'), 'the buy thread is named "Buy: <project>"', title)
    # look away: back on the first thread while the buy thread's agent works, so its need is raised on a thread the window is not watching
    wait('the window on the buy thread', lambda: rect_of(f'thread:{buy}'), 20); time.sleep(1); press_rect(wait('the first thread', lambda: rect_of('thread:t1'), 10))
    def notes(): return open(OUT + '/notify.log').read().splitlines() if os.path.exists(OUT + '/notify.log') else []
    wait('the need notified on the desktop', lambda: any('Needs you' in l for l in notes()), 40)
    check(True, 'a need on a thread the window is not watching raises a desktop notification')
    press_rect(wait('the buy thread row', lambda: rect_of(f'thread:{buy}'), 10))
    def send_button():
        for k, c in controls().items():
            if k.startswith('ask:') and 'Send' in (c.get('label') or '') and c.get('rect', {}).get('w', 0) > 0: return c['rect']
        return None
    r = wait('the files\' approval card with Send', send_button, 60); time.sleep(1); shot('2-quote-card')
    check(any(e.get('$') == 'AskOpened' and e.get('tool') == 'order_quote' for e in events()), 'order_quote asks before anything is sent')
    check(not os.listdir(OUT + '/fab') or not any(f.startswith('q_') for f in os.listdir(OUT + '/fab')), 'nothing reached the order server before Send', os.listdir(OUT + '/fab'))
    press_rect(r)
    # the need is answered: the desktop notification it raised is closed (the hub's notify.clear, by the alert's key)
    wait("the need's notification closed", lambda: any(l.startswith('--close ask-') for l in notes()), 15)
    check(True, 'Send answers the need and its desktop notification is closed')
    # 3. the quote comes back: the Order card with its total, customs line and Buy now
    r = wait('Buy now on the Order card', lambda: rect_of('ui:Buy now'), 120); time.sleep(2); shot('3-order-card')
    q = next((e for e in events() if e.get('$') == 'MessagePosted' and str(e.get('msg', '')).startswith('q:')), None)
    check(q is not None, 'the quote is posted in the buy thread', q)
    card = json.loads(next(e['value'] for e in reversed(events()) if e.get('$') == 'SettingSet' and e.get('key') == f'order.quote.{buy}'))
    check(card.get('payable') is True and card.get('total'), 'the quote can be paid and has a total', card)
    # LIVE_NO_PAY=1: stop at the Order card with Buy now live (for a phone to press it)
    if os.environ.get('LIVE_NO_PAY') != '1':
        # 4. Buy now opens the link (the fake xdg-open records it, and opening it pays the Stripe double)
        press_rect(r)
        opened = wait('Buy now opened a link', lambda: os.path.exists(OUT + '/opened.txt') and open(OUT + '/opened.txt').read().strip(), 15)
        check(opened.startswith(f'http://127.0.0.1:{fport}/q/'), 'Buy now opens the order server\'s page for this quote', opened.split('?')[0])
        wait('the paid step', lambda: any(e.get('$') == 'MessagePosted' and str(e.get('msg', '')).endswith(':paid') for e in events()), 150)
        time.sleep(3); shot('4-paid')
        check(True, 'the test payment comes back as the Paid step')
    real = [l for l in open(OUT + '/hub.log', errors='replace') if 'api.anthropic' in l or 'backplane.works' in l]
    check(not real, 'no real model or order server was called', real[:1])
    json.dump({'hub': port, 'fab': fport, 'stripe': sport, 'token': open(OUT + '/home/token').read().strip(), 'buy': buy}, open(OUT + '/rig.json', 'w'))
    print(f'{len(passes)} passed, {len(fails)} failed', flush=True)
    if os.environ.get('LIVE_KEEP') == '1':
        print('rig up: ' + json.dumps(json.load(open(OUT + '/rig.json')) | {'token': '<in rig.json>'}), flush=True)
        while not os.path.exists(OUT + '/stop'): time.sleep(1)
finally:
    stop()
sys.exit(1 if fails else 0)
