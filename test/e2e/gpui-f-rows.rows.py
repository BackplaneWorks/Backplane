MAIN = lambda: rects()['main']
def box(r, pad=0): return (int(r['x']) + pad, int(r['y']) + pad, int(r['x'] + r['w']) - pad, int(r['y'] + r['h']) - pad)
def pause(s=1.2): time.sleep(s)
def sel(i): click_ctl('thread:t%d' % i); pause(1.0)
def chord(c): xin('chord', *c.split())

# ---- F09 the context meter: the Context panel says "no reading yet" until a turn is counted, then shows what the thread holds and what is left
if want('F09'):
    sel(1); click_ctl('ui:Context'); pause(1.5); shot('m-empty'); PANEL = (367, 515, 1265, 780)
    xin('click', 815, 810); xin('type', 'zqshort meter'); xin('key', 'Return')
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 30); pause(3); shot('m-after')
    changed = differs(px('m-empty'), px('m-after'), PANEL)
    keys = sorted({e['key'].split('.')[0] + '.' + e['key'].split('.')[1] for e in events() if e['$'] == 'SettingSet' and e.get('key', '').startswith(('tok.', 'turn.last'))})
    counted = any(e['$'] == 'TurnUsage' for e in events())
    log('F09-meter', changed and counted, 'Context panel before the turn (m-empty.png) and after it (m-after.png): the panel changed=%s once the turn was counted; the hub logged TurnUsage=%s, readings %s. The figures are read from the screenshot, not by the driver' % (changed, counted, keys))
    # drafts survive a crash: type, kill the client, restart it, the text is back
    xin('click', 815, 810); xin('type', 'unsent draft zq9'); pause(2.5)
    client.kill(); client.wait(); time.sleep(1.5)
    client = spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % PORT, '--token-file', P('home/token'), '--script', P('script.json'), '--exit-after', '3000'], 'client.log', cenv); time.sleep(8)
    shot('m-draft'); from PIL import Image
    crop = px('m-draft').crop((367, 788, 1265, 848)); open_composer_has_text = crop.convert('L').getextrema()[1] > 150 and sum(1 for v in crop.convert('L').getdata() if v > 150) > 80
    log('F09-draft', open_composer_has_text, 'typed an unsent draft, killed the client process, started it again: the composer shows text again=%s (m-draft.png)' % open_composer_has_text)


# ---- F15 image paste: a PNG on the real clipboard, Ctrl+V in the composer, Send posts it with the image and the agent is told its path
if want('F15'):
    img = os.path.join(OUT, 'paste.png')
    from PIL import Image
    im = Image.new('RGB', (64, 48), (200, 40, 40))
    for x in range(20, 44):
        for y in range(12, 36): im.putpixel((x, y), (40, 200, 60))
    im.save(img)
    clip_image(img); pause(0.5)
    r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); pause(0.5)
    shot('f15-before'); chord('ctrl v'); pause(2.5); shot('f15-pasted')
    chip = differs(px('f15-before'), px('f15-pasted'), (int(r['x']), int(r['y'] - 120), int(r['x'] + r['w']), int(r['y'] + r['h'])))
    xin('type', 'zqecho look at this'); pause(0.5); xin('key', 'Return')
    sent = until(lambda: any(e['$'] == 'MessagePosted' and e.get('role') == 'user' and 'look at this' in e.get('text', '') for e in events()), 20)
    pause(2); shot('f15-sent')
    files = glob.glob(P('home/attachments/*/*pasted*.png'))
    told = os.path.exists(P('fake.log')) and any('look at this' in l and ('attachments/' in l or 'attachments\\' in l) for l in open(P('fake.log')).read().splitlines() if l.startswith('TURN'))
    same = bool(files) and open(files[0], 'rb').read()[:8] == b'\x89PNG\r\n\x1a\n'
    log('F15-paste', chip and sent and same and told, 'a PNG on the X clipboard pasted with Ctrl+V: composer changed=%s; message posted=%s; stored as a PNG=%s (%s); the agent was told the path=%s' % (chip, sent, same, [os.path.relpath(f, OUT) for f in files][:1], told))

# ---- F06 cards in the client: the approval, question and plan cards of a thread, answered with the mouse; the agent hears each answer
if want('F06'):
    fake = lambda: open(P('fake.log')).read() if os.path.exists(P('fake.log')) else ''
    asks = lambda th, kind: [e for e in events() if e['$'] == 'AskOpened' and e.get('thread') == th and e.get('kind') == kind]
    closed = lambda a: any(e['$'] == 'AskClosed' and e.get('id') == a['id'] for e in events())
    sel(1); say_ = lambda t: (xin('click', 815, 810), xin('type', t), xin('key', 'Return'))
    say_('zqapprove now'); until(lambda: asks('t1', 'approval'), 20); pause(2); shot('f6-approval')
    a1 = asks('t1', 'approval')[-1]; click_ctl('ask:0:0'); ok1 = until(lambda: closed(a1), 20); until(lambda: 'PERM Bash -> ' in fake(), 15)
    allowed = bool(re.search(r'PERM Bash -> .*allow', fake(), re.I))
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 20)
    n = len(asks('t1', 'approval')); say_('zqapprove again'); until(lambda: len(asks('t1', 'approval')) > n, 20); pause(2)
    a2 = asks('t1', 'approval')[-1]; click_ctl('ask:0:2'); ok2 = until(lambda: closed(a2), 20); until(lambda: re.search(r'PERM Bash -> .*deny', fake(), re.I), 15)
    denied = bool(re.search(r'PERM Bash -> .*deny', fake(), re.I))
    log('F06-approval', ok1 and allowed and ok2 and denied, 'approval card (f6-approval.png) with Allow, Allow for session and Deny: Allow closed the ask and the agent got allow=%s; Deny closed the next and the agent got deny=%s' % (allowed, denied))
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 20)
    say_('zqask'); until(lambda: asks('t1', 'input'), 20); pause(2); shot('f6-question'); q = asks('t1', 'input')[-1]
    ids = sorted(k for k in ctrls() if k.startswith('ask:')); click_ctl(ids[0]) if ids else None; okq = until(lambda: closed(q), 20); until(lambda: 'PERM AskUserQuestion -> ' in fake(), 15)
    heard = bool(re.search(r'PERM AskUserQuestion -> .*(USB-C|Barrel)', fake()))
    log('F06-question', bool(ids) and okq and heard, 'question card "%s" (f6-question.png), option buttons %s: the first closed it and the agent got the answer=%s' % ((q.get('detail') or '')[:30].replace('\n', ' '), ids, heard))
    sel(2); say_('zqplan'); until(lambda: asks('t2', 'plan'), 20); pause(2); shot('f6-plan'); pl = asks('t2', 'plan')
    log('F06-plan', bool(pl) and 'Draw the board' in (pl[-1].get('detail') or ''), 'a thread in plan mode (ModesSet interaction=plan) asked to exit with the plan card (f6-plan.png), AskOpened kind=plan with the plan text=%s' % bool(pl))

# ---- F16 keybindings: each chord of the shortcuts table through the window's bindings, judged by what the client or the hub then shows
if want('F16'):
    res = []
    def trial(name, chord_, judge, undo=None):
        a = shot('k-%s-a' % name); n0 = len(events()); m0 = len(metrics()); chord(chord_); pause(1.5); b = shot('k-%s-b' % name)
        ok, why = judge(a, b, events()[n0:], metrics()[m0:])
        if undo: undo(); pause(0.8)
        res.append((name, chord_, ok, why)); print('   ', name, chord_, ok, why, flush=True)
    sel(1)
    trial('palette', 'ctrl k', lambda a, b, e, m: (differs(px('k-palette-a'), px('k-palette-b')), 'the command palette drew over the window'), lambda: xin('key', 'Escape'))
    trial('models', 'ctrl m', lambda a, b, e, m: (any(x['event'] == 'pick' for x in m), 'the model menu opened (client `pick` metric)'), lambda: xin('key', 'Escape'))
    r0 = dict(rects())
    trial('sidebar', 'ctrl b', lambda a, b, e, m: (rects().get('main', {}).get('x', 0) < r0['main']['x'] - 50, 'the main pane grew left as the sidebar folded (main.x %s -> %s)' % (r0['main']['x'], rects().get('main', {}).get('x'))), lambda: chord('ctrl b'))
    trial('files', 'ctrl p', lambda a, b, e, m: (differs(px('k-files-a'), px('k-files-b'), box(MAIN())), 'the Files page replaced the timeline'), lambda: chord('ctrl p'))
    trial('diff', 'ctrl d', lambda a, b, e, m: (differs(px('k-diff-a'), px('k-diff-b'), box(MAIN())), 'the Diff page replaced the timeline'), lambda: chord('ctrl d'))
    trial('terminal', 'ctrl j', lambda a, b, e, m: (differs(px('k-terminal-a'), px('k-terminal-b'), box(MAIN())), 'the floating terminal drew'), lambda: chord('ctrl j'))
    chord('ctrl 1'); pause(1.0)
    trial('thread2', 'ctrl 2', lambda a, b, e, m: (differs(px('k-thread2-a'), px('k-thread2-b'), (0, 30, int(MAIN()['x'] + 700), 96)), 'the header moved to the second thread'))
    trial('perf', 'ctrl shift h', lambda a, b, e, m: (differs(px('k-perf-a'), px('k-perf-b')), 'the performance overlay drew'), lambda: chord('ctrl shift h'))
    trial('uber', 'ctrl shift u', lambda a, b, e, m: (differs(px('k-uber-a'), px('k-uber-b'), box(MAIN())), "the uberbot's chat opened"))
    sel(1)
    trial('new', 'ctrl n', lambda a, b, e, m: (any(x['$'] == 'ThreadCreated' for x in e), 'ThreadCreated in the hub log'))
    # stop and settle need a running turn: send a long one by keys
    sel(2); r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); xin('type', 'a long turn'); xin('key', 'Return')
    t2 = lambda: [e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't2']
    until(lambda: any(e['state'] == 'running' for e in t2()), 15)
    trial('stop', 'ctrl period', lambda a, b, e, m: (any(x['$'] == 'TurnChanged' and x.get('state') == 'interrupted' for x in e), 'TurnChanged interrupted in the hub log'))
    trial('settle', 'ctrl shift s', lambda a, b, e, m: (any(x['$'] == 'ThreadSettled' for x in e), 'ThreadSettled in the hub log'))
    log('F16', all(x[2] for x in res), '; '.join('%s %s: %s' % (n, c, 'ok' if ok else 'FAIL (' + w + ')') for n, c, ok, w in res))

# ---- F17 viewers: the rail's Board, Schematic, 3D and Mech pages on a seeded KiCad board and a STEP part; zoom with the wheel, pan and orbit with the left button
if want('F17'):
    from PIL import ImageStat
    sel(1); res = []
    VIEW = (840, 150, 1360, 880)   # the viewer pane below its tab bar
    def busy(name):  # drawn: thousands of pixels that are not the page's background (a "Loading" line or a toolbar is a few hundred, an error line about 2000)
        im = px(name).crop(VIEW).convert('L'); bg = im.getpixel((5, 5)); return sum(1 for v in im.getdata() if abs(v - bg) > 25) > 4000   # a fitted schematic is thin lines: about 5000; a line of text about 2000
    for kind, label in [('Board', 'rail:Board'), ('Schematic', 'rail:Schematic'), ('3D', 'rail:3D'), ('Mech', 'rail:Mech')]:
        click_ctl(label); pause(1.0)
        until(lambda: (shot('v-%s' % kind) and busy('v-%s' % kind)), 40)
        cx, cy = 1100, 480
        drawn = busy('v-%s' % kind)
        for _ in range(4): xin('wheel', cx, cy, 1)
        pause(1.5); shot('v-%s-zoom' % kind); zoomed = differs(px('v-%s' % kind), px('v-%s-zoom' % kind), VIEW)
        xin('drag', cx - 80, cy, cx + 80, cy + 40); pause(1.5); shot('v-%s-pan' % kind); panned = differs(px('v-%s-zoom' % kind), px('v-%s-pan' % kind), VIEW)
        res.append((kind, drawn, zoomed, panned)); print('   ', kind, 'drawn', drawn, 'wheel changed it', zoomed, 'drag changed it', panned, flush=True)
    log('F17', all(r[1] and r[2] and r[3] for r in res), '; '.join('%s: drawn=%s wheel=%s %s=%s' % (k, d, z, 'orbit' if k == '3D' else 'pan', p_) for k, d, z, p_ in res) + ' (frames are not judged: llvmpipe)')

# ---- F07 stop, F08 fork, F14 terminal, F21 reconnect: through the buttons and keys of the window
if want('F07'):
    sel(1); say_ = lambda t: (xin('click', 815, 810), xin('type', t), xin('key', 'Return'))
    n0 = len([e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'running'])
    say_('a long turn'); until(lambda: len([e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'running']) > n0, 20); pause(1.5); shot('f7-running')
    stopped = click_ctl('tool:interrupt'); ok = until(lambda: events()[-1]['$'] and any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'interrupted' for e in events()), 20)
    log('F07-stop', stopped and ok, 'a running turn (f7-running.png) and the Stop button in the thread header: TurnChanged interrupted in the hub log=%s' % ok)
if want('F08'):
    sel(1); n = len([e for e in events() if e['$'] == 'ThreadCreated']); forked = click_ctl('tool:fork'); ok = until(lambda: len([e for e in events() if e['$'] == 'ThreadCreated']) > n, 20); pause(1.5); shot('f8-fork')
    log('F08-fork', forked and ok, 'the Fork thread button: a new thread was created in the hub log=%s (f8-fork.png)' % ok)
if want('F14'):
    marker = P('term.marker').replace('\\', '/');   # the terminal's shell is POSIX sh, Git's on Windows
    chord('ctrl j'); pause(2.5); shot('f14-open')
    xin('type', 'touch ' + marker); xin('key', 'Return'); ok = until(lambda: os.path.exists(marker), 15); pause(1); shot('f14-typed')
    shown = differs(px('f14-open'), px('f14-typed'))
    log('F14-terminal', ok and shown, 'the floating terminal (Ctrl+J): a command typed with the keyboard ran in a real shell (the marker file exists=%s) and the panel redrew=%s (f14-typed.png)' % (ok, shown))
    chord('ctrl j')
if want('F21'):
    sel(1); shot('f21-up'); hub.terminate(); hub.wait(); pause(6); shot('f21-down')
    hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=P('proj'))
    for _ in range(100):
        if subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', 'http://127.0.0.1:%d/hello' % PORT]).returncode == 0: break
        time.sleep(0.2)
    pause(12); shot('f21-back'); xin('click', 815, 810); xin('type', 'zqshort after the reconnect'); xin('key', 'Return')
    ok = until(lambda: any(e['$'] == 'MessagePosted' and 'after the reconnect' in e.get('text', '') for e in events()), 30)
    ds = differs(px('f21-up'), px('f21-down'), (0, 870, 263, 900))
    log('F21-reconnect', ok and ds, 'the hub was stopped and started again on the same home: the status line changed while it was down=%s, and after reconnecting a message typed in the window reached the new hub process=%s (f21-*.png)' % (ds, ok))

# ---- F22 the order flow in the client, up to the quote: the agent offers a folder, the client shows the files card, Send is pressed with the mouse, the quote card follows; no order is placed
if want('F22'):
    import hashlib
    os.makedirs(P('proj/mfg/o1'), exist_ok=True); gbr = b'G04*\nM02*\n'; open(P('proj/mfg/o1/board.gbr'), 'wb').write(gbr)
    json.dump({'files': {'board.gbr': {'sha256': hashlib.sha256(gbr).hexdigest()}}}, open(P('proj/mfg/o1/manifest.json'), 'w'))
    sel(3)  # the third thread is a buy thread from the seed (SettingSet buy.t3)
    r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); xin('type', 'zqquote mfg/o1'); xin('key', 'Return')
    until(lambda: any(e['$'] == 'AskOpened' and e.get('tool') == 'order_quote' for e in events()), 30); pause(2); shot('f22-files-card')
    ask = [e for e in events() if e['$'] == 'AskOpened' and e.get('tool') == 'order_quote']; before = len([x for x in ORDER_SEEN if x[0] == 'POST'])
    ids = sorted(k for k in ctrls() if k.startswith('ask:')); sent = click_ctl(ids[0]) if ids else False
    quoted = until(lambda: any(e['$'] == 'SettingSet' and e.get('key') == 'order.quote.t3' and e.get('value') for e in events()), 40); pause(3); shot('f22-quote')
    clean = not any('pay' in x[1] or 'buy' in x[1] or 'checkout' in x[1] for x in ORDER_SEEN)
    log('F22-card', bool(ask) and before == 0 and sent and quoted and clean, 'the files card (f22-files-card.png, buttons %s) opened with nothing uploaded before Send (no POST to the order server)=%s; Send pressed with the mouse: the hub uploaded to the stand-in order server and the quote card was set=%s (f22-quote.png); requests seen %s, none to pay or buy=%s' % (ids, before == 0, quoted, [x[0] + ' ' + x[1] for x in ORDER_SEEN], clean))
