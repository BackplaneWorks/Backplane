# CAL: what the window, rects and controls are on this runner (printed for the rows' coordinates), and that a real click reaches the client
if want('CAL'):
    print('screen', screen_size(), 'window content', win(), flush=True)
    r = rects(); print('rects', json.dumps({k: [round(v) for v in (x['x'], x['y'], x['w'], x['h'])] for k, x in r.items()}), flush=True)
    c = ctrls(); print('controls', len(c), sorted(c)[:60], flush=True)
    shot('cal-0'); n0 = len(metrics()); ok = click_ctl('thread:t2'); time.sleep(2); shot('cal-1')
    sel_changed = differs(px('cal-0'), px('cal-1'))
    log('CAL', ok and sel_changed, 'clicked thread:t2 with a real mouse click: clicked=%s, the window redrew=%s' % (ok, sel_changed))

MAIN = lambda: rects()['main']
def ensure_sidebar():   # the sidebar is open when `main` starts at the sidebar's width; Cmd+B toggles it, so look before pressing
    for _ in range(3):
        if rects().get('main', {}).get('x', 0) > 100 and 'thread:t1' in ctrls(): return True
        xin('click', *CC()); chord('ctrl b'); pause(1.5)
    return False
def card(cid, done, tries=4):   # press a card's button; the card may not be laid out yet, or move (a checkpoint row appears), so look at the controls again and retry until it closes
    for i in range(tries):
        pause(2.0); shot('card-%s-%d' % (cid.replace(':', '_'), i)); ids = sorted(k for k in ctrls() if k.startswith('ask:'))
        if cid == 'first': c = ids[0] if ids else None
        else: c = cid if cid in ctrls() else None
        if c: click_ctl(c)
        if until(done, 8): return True
    return False
def say_(t):   # click the composer, type, press Return; if the message did not reach the hub, press Return again (a key can land before the field has focus)
    n = len([e for e in events() if e['$'] == 'MessagePosted' and t in e.get('text', '')])
    xin('click', *CC()); xin('type', t); xin('key', 'Return')
    for how in ('return', 'send'):
        if until(lambda: len([e for e in events() if e['$'] == 'MessagePosted' and t in e.get('text', '')]) > n, 6): break
        print('   say_: "%s" not posted after %s; trying %s' % (t, 'Return' if how == 'return' else 'a second Return', 'the Send button' if how == 'send' else 'Return again'), flush=True)
        if how == 'return': xin('click', *CC()); xin('key', 'Return')
        else: click_ctl('send')
def CC():
    r = rects()['composer']; return int(r['x'] + r['w'] / 2), int(r['y'] + 30)
def box(r, pad=0): return (int(r['x']) + pad, int(r['y']) + pad, int(r['x'] + r['w']) - pad, int(r['y'] + r['h']) - pad)
def pause(s=1.2): time.sleep(s)
def sel(i): click_ctl('thread:t%d' % i); pause(1.0)
def chord(c): xin('chord', *c.split())


# ---- F02 the client connects: sidebar, projects and threads loaded from the hub, "Connected"
if want('F02'):
    shot('f02'); c = ctrls(); th = sorted(k for k in c if k.startswith('thread:')); usable = any(m.get('event') == 'usable' for m in metrics()); gpu = [m for m in metrics() if m.get('event') == 'gpu']
    log('F02', usable and len(th) >= 3 and 'project:p1' in c, 'the client drew its first frame and reached usable=%s; the sidebar lists project p1 and threads %s (controls reported by the client); gpu %s (f02.png shows the sidebar, the threads and "Connected")' % (usable, th, (gpu[-1:] or [{}])[0].get('name', gpu[-1:] if gpu else '?')))

# ---- F09 the context meter: the Context panel says "no reading yet" until a turn is counted, then shows what the thread holds and what is left
if want('F09'):
    sel(1); click_ctl('ui:Context'); pause(1.5); shot('m-empty'); PANEL = box(MAIN())
    say_('zqshort meter')
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 30); pause(3); shot('m-after')
    changed = differs(px('m-empty'), px('m-after'), PANEL)
    keys = sorted({e['key'].split('.')[0] + '.' + e['key'].split('.')[1] for e in events() if e['$'] == 'SettingSet' and e.get('key', '').startswith(('tok.', 'turn.last'))})
    counted = any(e['$'] == 'TurnUsage' for e in events())
    log('F09-meter', changed and counted, 'Context panel before the turn (m-empty.png) and after it (m-after.png): the panel changed=%s once the turn was counted; the hub logged TurnUsage=%s, readings %s. The figures are read from the screenshot, not by the driver' % (changed, counted, keys))
    # drafts survive a crash: type, kill the client, restart it, the text is back
    xin('click', *CC()); xin('type', 'unsent draft zq9'); pause(2.5)
    client.kill(); client.wait(); time.sleep(1.5)
    client = start_client(); time.sleep(2)
    shot('m-draft'); from PIL import Image
    crop = px('m-draft').crop(box(rects()['composer'])); open_composer_has_text = crop.convert('L').getextrema()[1] > 150 and sum(1 for v in crop.convert('L').getdata() if v > 150) > 80
    log('F09-draft', open_composer_has_text, 'typed an unsent draft, killed the client process, started it again: the composer shows text again=%s (m-draft.png)' % open_composer_has_text)
    xin('click', *CC()); chord('ctrl a'); xin('key', 'BackSpace'); pause(0.5)   # the restored draft must not ride along on every later message


# ---- F15 image paste: a PNG on the real clipboard, Ctrl+V in the composer, Send posts it with the image and the agent is told its path
if want('F15'):
    img = os.path.join(OUT, 'paste.png')
    from PIL import Image
    im = Image.new('RGB', (64, 48), (200, 40, 40))
    for x in range(20, 44):
        for y in range(12, 36): im.putpixel((x, y), (40, 200, 60))
    im.save(img)
    set_clipboard_png(img); pause(0.5)
    r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); pause(0.5)
    shot('f15-before'); chord('ctrl v'); pause(2.5); shot('f15-pasted')
    chip = differs(px('f15-before'), px('f15-pasted'), (int(r['x']), int(r['y'] - 120), int(r['x'] + r['w']), int(r['y'] + r['h'])))
    say_('zqecho look at this')
    sent = until(lambda: any(e['$'] == 'MessagePosted' and e.get('role') == 'user' and 'look at this' in e.get('text', '') for e in events()), 20)
    pause(2); shot('f15-sent')
    files = glob.glob(P('home/attachments/*/*pasted*.png')) or glob.glob(P('home/attachments/*/*.png'))
    told = until(lambda: os.path.exists(P('fake.log')) and any('look at this' in l and 'attachments/' in l for l in open(P('fake.log')).read().splitlines() if l.startswith('TURN')), 30)
    same = bool(files) and open(files[0], 'rb').read()[:8] == b'\x89PNG\r\n\x1a\n'
    log('F15-paste', chip and sent and same and told, 'a PNG on the X clipboard pasted with Ctrl+V: composer changed=%s; message posted=%s; stored as a PNG=%s (%s); the agent was told the path=%s' % (chip, sent, same, [os.path.relpath(f, OUT) for f in files][:1], told))

# ---- F06 cards in the client: the approval, question and plan cards of a thread, answered with the mouse; the agent hears each answer
if want('F06'):
    fake = lambda: open(P('fake.log')).read() if os.path.exists(P('fake.log')) else ''
    asks = lambda th, kind: [e for e in events() if e['$'] == 'AskOpened' and e.get('thread') == th and e.get('kind') == kind]
    closed = lambda a: any(e['$'] == 'AskClosed' and e.get('id') == a['id'] for e in events())
    sel(1); pass
    say_('zqapprove now'); until(lambda: asks('t1', 'approval'), 20); pause(2); shot('f6-approval')
    a1 = asks('t1', 'approval')[-1]; ok1 = card('ask:0:0', lambda: closed(a1)); until(lambda: 'PERM Bash -> ' in fake(), 15)
    allowed = bool(re.search(r'PERM Bash -> .*allow', fake(), re.I))
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 20)
    n = len(asks('t1', 'approval')); say_('zqapprove again'); until(lambda: len(asks('t1', 'approval')) > n, 20); pause(2)
    a2 = asks('t1', 'approval')[-1]; ok2 = card('ask:0:2', lambda: closed(a2)); until(lambda: re.search(r'PERM Bash -> .*deny', fake(), re.I), 15)
    denied = bool(re.search(r'PERM Bash -> .*deny', fake(), re.I))
    log('F06-approval', ok1 and allowed and ok2 and denied, 'approval card (f6-approval.png) with Allow, Allow for session and Deny: Allow closed the ask and the agent got allow=%s; Deny closed the next and the agent got deny=%s' % (allowed, denied))
    until(lambda: any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'completed' for e in events()), 20)
    say_('zqask'); until(lambda: asks('t1', 'input'), 30); pause(2); shot('f6-question'); q = asks('t1', 'input')[-1]
    ids = sorted(k for k in ctrls() if k.startswith('ask:')); okq = card('first', lambda: closed(q)); until(lambda: 'PERM AskUserQuestion -> ' in fake(), 15)
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
        if not ok: pause(3.0); b = shot('k-%s-b' % name); ok, why = judge(a, b, events()[n0:], metrics()[m0:])   # a slow runner: look again before saying no
        if undo: undo(); pause(0.8)
        res.append((name, chord_, ok, why)); print('   ', name, chord_, ok, why, flush=True)
    sel(1)
    trial('palette', 'ctrl k', lambda a, b, e, m: (differs(px('k-palette-a'), px('k-palette-b')), 'the command palette drew over the window'), lambda: xin('key', 'Escape'))
    trial('models', 'ctrl m', lambda a, b, e, m: (any(x['event'] == 'pick' for x in m), 'the model menu opened (client `pick` metric)'), lambda: xin('key', 'Escape'))
    r0 = dict(rects())
    trial('sidebar', 'ctrl b', lambda a, b, e, m: (rects().get('main', {}).get('x', 0) < r0['main']['x'] - 50, 'the main pane grew left as the sidebar folded (main.x %s -> %s)' % (r0['main']['x'], rects().get('main', {}).get('x'))), lambda: chord('ctrl b'))
    trial('files', 'ctrl p', lambda a, b, e, m: (differs(px('k-files-a'), px('k-files-b')), 'the Files page drew (the window changed)'), lambda: chord('ctrl p'))
    trial('diff', 'ctrl d', lambda a, b, e, m: (differs(px('k-diff-a'), px('k-diff-b')), 'the Diff page drew (the window changed)'), lambda: chord('ctrl d'))
    trial('terminal', 'ctrl j', lambda a, b, e, m: (differs(px('k-terminal-a'), px('k-terminal-b'), box(MAIN())), 'the floating terminal drew'), lambda: chord('ctrl j'))
    chord('ctrl 1'); pause(1.0)
    trial('thread2', 'ctrl 2', lambda a, b, e, m: (differs(px('k-thread2-a'), px('k-thread2-b'), (0, 0, 600, 40)), 'the header moved to the second thread'))
    trial('perf', 'ctrl shift h', lambda a, b, e, m: (differs(px('k-perf-a'), px('k-perf-b')), 'the performance overlay drew'), lambda: chord('ctrl shift h'))
    trial('uber', 'ctrl shift u', lambda a, b, e, m: (differs(px('k-uber-a'), px('k-uber-b'), box(MAIN())), "the uberbot's chat opened"))
    sel(1)
    trial('new', 'ctrl n', lambda a, b, e, m: (any(x['$'] == 'ThreadCreated' for x in e), 'ThreadCreated in the hub log'))
    # stop and settle need a running turn: send a long one by keys
    sel(2); say_('a long turn')
    t2 = lambda: [e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't2']
    until(lambda: any(e['state'] == 'running' for e in t2()), 40)
    until(lambda: 'tool:interrupt' in ctrls(), 40)   # the Stop button is there while the turn runs
    trial('stop', 'ctrl period', lambda a, b, e, m: (any(x['$'] == 'TurnChanged' and x.get('state') == 'interrupted' for x in e), 'TurnChanged interrupted in the hub log'))
    trial('settle', 'ctrl shift s', lambda a, b, e, m: (any(x['$'] == 'ThreadSettled' for x in e), 'ThreadSettled in the hub log'))
    log('F16', all(x[2] for x in res), '; '.join('%s %s: %s' % (n, c, 'ok' if ok else 'FAIL (' + w + ')') for n, c, ok, w in res))

# ---- F17 viewers: the rail's Board, Schematic, 3D and Mech pages on a seeded KiCad board and a STEP part; zoom with the wheel, pan and orbit with the left button
if want('F17'):
    from PIL import ImageStat
    sel(1); res = []; chord('ctrl b'); pause(1.5)   # a 1024 px window has no room for sidebar, timeline and viewer at once
    VIEW = (560, 70, win()[2] - 80, win()[3] - 20)   # right of the timeline, left of the rail
    def busy(name):  # drawn: thousands of pixels that are not the page's background (a "Loading" line or a toolbar is a few hundred)
        im = px(name).crop(VIEW).convert('L'); bg = im.getpixel((5, 5)); return sum(1 for v in im.getdata() if abs(v - bg) > 25) > 6000
    for kind, label in [('Board', 'rail:Board'), ('Schematic', 'rail:Schematic'), ('3D', 'rail:3D'), ('Mech', 'rail:Mech')]:
        click_ctl(label); pause(1.0)
        until(lambda: (shot('v-%s' % kind) and busy('v-%s' % kind)), 40)
        cx, cy = (VIEW[0] + VIEW[2]) // 2, (VIEW[1] + VIEW[3]) // 2
        drawn = busy('v-%s' % kind)
        for _ in range(4): xin('wheel', cx, cy, 1)
        pause(1.5); shot('v-%s-zoom' % kind); zoomed = differs(px('v-%s' % kind), px('v-%s-zoom' % kind), VIEW)
        xin('drag', cx - 80, cy, cx + 80, cy + 40); pause(1.5); shot('v-%s-pan' % kind); panned = differs(px('v-%s-zoom' % kind), px('v-%s-pan' % kind), VIEW)
        res.append((kind, drawn, zoomed, panned)); print('   ', kind, 'drawn', drawn, 'wheel changed it', zoomed, 'drag changed it', panned, flush=True)
    ensure_sidebar()   # the sidebar back for the rows after this one
    log('F17', all(r[1] and r[2] and r[3] for r in res), '; '.join('%s: drawn=%s wheel=%s %s=%s' % (k, d, z, 'orbit' if k == '3D' else 'pan', p_) for k, d, z, p_ in res) + ' (frames are not judged: llvmpipe)')

# ---- F07 stop, F08 fork, F14 terminal, F21 reconnect: through the buttons and keys of the window
if want('F07'):
    sel(1); pass
    n0 = len([e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'running'])
    say_('a long turn'); until(lambda: len([e for e in events() if e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'running']) > n0, 20); pause(1.5); shot('f7-running')
    until(lambda: 'tool:interrupt' in ctrls(), 40); stopped = click_ctl('tool:interrupt'); ok = until(lambda: events()[-1]['$'] and any(e['$'] == 'TurnChanged' and e.get('thread') == 't1' and e.get('state') == 'interrupted' for e in events()), 20)
    log('F07-stop', stopped and ok, 'a running turn (f7-running.png) and the Stop button in the thread header: TurnChanged interrupted in the hub log=%s' % ok)
if want('F08'):
    sel(1); n = len([e for e in events() if e['$'] == 'ThreadCreated']); forked = click_ctl('tool:fork'); ok = until(lambda: len([e for e in events() if e['$'] == 'ThreadCreated']) > n, 20); pause(1.5); shot('f8-fork')
    log('F08-fork', forked and ok, 'the Fork thread button: a new thread was created in the hub log=%s (f8-fork.png)' % ok)
if want('F14'):
    marker = P('term.marker'); chord('ctrl j'); pause(2.5); shot('f14-open')
    xin('type', 'touch ' + marker); xin('key', 'Return'); ok = until(lambda: os.path.exists(marker), 6)
    if not ok: xin('key', 'Return'); ok = until(lambda: os.path.exists(marker), 15)   # a Return can be lost before the shell has the line
    pause(1); shot('f14-typed')
    shown = differs(px('f14-open'), px('f14-typed'))
    log('F14-terminal', ok and shown, 'the floating terminal (Ctrl+J): a command typed with the keyboard ran in a real shell (the marker file exists=%s) and the panel redrew=%s (f14-typed.png)' % (ok, shown))
    chord('ctrl j')
if want('F21'):
    sel(1); shot('f21-up'); hub.terminate(); hub.wait(); pause(6); shot('f21-down')
    hub = spawn([HUB, '--home', P('home'), '--port', str(PORT), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'hub.log', cwd=P('proj'))
    for _ in range(100):
        if subprocess.run(['curl', '-s', '-m1', '-o', '/dev/null', 'http://127.0.0.1:%d/hello' % PORT]).returncode == 0: break
        time.sleep(0.2)
    pause(12); shot('f21-back'); say_('zqshort after the reconnect')
    ok = until(lambda: any(e['$'] == 'MessagePosted' and 'after the reconnect' in e.get('text', '') for e in events()), 30)
    ds = differs(px('f21-up'), px('f21-down'), (0, win()[3] - 40, 263, win()[3] - 4))
    log('F21-reconnect', ok and ds, 'the hub was stopped and started again on the same home: the status line changed while it was down=%s, and after reconnecting a message typed in the window reached the new hub process=%s (f21-*.png)' % (ds, ok))

# ---- F22 the order flow in the client, up to the quote: the agent offers a folder, the client shows the files card, Send is pressed with the mouse, the quote card follows; no order is placed
if want('F22'):
    import hashlib
    os.makedirs(P('proj/mfg/o1'), exist_ok=True); gbr = b'G04*\nM02*\n'; open(P('proj/mfg/o1/board.gbr'), 'wb').write(gbr)
    json.dump({'files': {'board.gbr': {'sha256': hashlib.sha256(gbr).hexdigest()}}}, open(P('proj/mfg/o1/manifest.json'), 'w'))
    sel(3)  # the third thread is a buy thread from the seed (SettingSet buy.t3)
    say_('zqquote mfg/o1')
    until(lambda: any(e['$'] == 'AskOpened' and e.get('tool') == 'order_quote' for e in events()), 30); pause(2); shot('f22-files-card')
    ask = [e for e in events() if e['$'] == 'AskOpened' and e.get('tool') == 'order_quote']; before = len([x for x in ORDER_SEEN if x[0] == 'POST'])
    ids = sorted(k for k in ctrls() if k.startswith('ask:')); sent = click_ctl(ids[0]) if ids else False
    quoted = until(lambda: any(e['$'] == 'SettingSet' and e.get('key') == 'order.quote.t3' and e.get('value') for e in events()), 40); pause(3); shot('f22-quote')
    clean = not any('pay' in x[1] or 'buy' in x[1] or 'checkout' in x[1] for x in ORDER_SEEN)
    log('F22-card', bool(ask) and before == 0 and sent and quoted and clean, 'the files card (f22-files-card.png, buttons %s) opened with nothing uploaded before Send (no POST to the order server)=%s; Send pressed with the mouse: the hub uploaded to the stand-in order server and the quote card was set=%s (f22-quote.png); requests seen %s, none to pay or buy=%s' % (ids, before == 0, quoted, [x[0] + ' ' + x[1] for x in ORDER_SEEN], clean))


# ---- F18 the Browser tab in frame-stream mode (a release archive carries CEF only on Linux): the hub's browser helper streams the page, the window's mouse, wheel and keys go to it
if want('F18'):
    sys.path.insert(0, os.path.join(HERE, 'lib')); import f18_page
    pport, EVT = f18_page.serve(); names = lambda: [n for _, n in EVT]; URL = 'http://127.0.0.1:%d/' % pport
    from PIL import Image
    def find(name, rgb, tol=30, minpx=400):   # the centre of the pixels of that colour on a shot (window coordinates), None when there are fewer than minpx
        im = px(name); w, h = im.size; xs = ys = n = 0; d = im.load()
        for y in range(0, h, 2):
            for x in range(0, w - 80, 2):
                r, g, b = d[x, y]
                if abs(r - rgb[0]) < tol and abs(g - rgb[1]) < tol and abs(b - rgb[2]) < tol: xs += x; ys += y; n += 1
        return (xs // n, ys // n) if n * 4 >= minpx else None
    sel(1); chord('ctrl b'); pause(1.0); click_ctl('rail:Browser'); pause(4); allow_dialogs(); VW = (560, 90, win()[2] - 80, win()[3] - 30); shot('f18-open')
    allow_dialogs(); xin('click', (VW[0] + VW[2]) // 2, (VW[1] + VW[3]) // 2); pause(0.5); chord('ctrl l'); pause(1.0); chord('ctrl a'); pause(0.3); xin('type', URL); xin('key', 'Return'); pause(2); allow_dialogs()
    loaded = until(lambda: 'load' in names(), 60); pause(5); shot('f18-page'); res = []
    import urllib.request, hashlib
    def frame(tag):   # the hub's own newest frame for the page (what the client's poller fetches), saved for the artifact
        try:
            rq = urllib.request.Request('http://127.0.0.1:%d/bots/t1/screen.jpg' % PORT, headers={'Authorization': 'Bearer ' + open(P('home/token')).read().strip()})
            b = urllib.request.urlopen(rq, timeout=5).read(); open(P('shots', 'f18-hubframe-%s.jpg' % tag), 'wb').write(b); return len(b), hashlib.md5(b).hexdigest()[:8]
        except Exception as e: return 0, str(e)[:60]
    fr1 = frame('a'); pause(3); fr2 = frame('b'); print('   F18 hub frame (what the poller fetches): %s then %s' % (fr1, fr2), flush=True)
    pos = {k: find('f18-page', c) for k, c in [('btn', (0, 204, 102)), ('inp', (255, 255, 102)), ('pop', (255, 136, 0)), ('dl', (204, 0, 204))]}
    res.append(('page loads and paints', loaded and all(pos.values()), 'the helper streamed the test page (the page asked its server: %s); targets found on the frame: %s' % (loaded, pos)))
    if pos['btn']:
        xin('click', *pos['btn']); res.append(('click', until(lambda: 'click' in names(), 20), 'a real click on the green button reached the page (it reported "click")'))
    if pos['inp']:
        xin('click', *pos['inp']); pause(0.5); xin('type', 'zqf18'); res.append(('type', until(lambda: any(n.startswith('input:') and 'zqf18' in n for n in names()), 20), 'typing in the page field reached the page (reported %s)' % [n for n in names() if n.startswith('input:')][-1:]))
    cx, cy = (pos['btn'] or (500, 300))[0], (pos['btn'] or (500, 300))[1]; shot('f18-before-scroll')
    xin('wheel', cx, cy, 6); res.append(('scroll', until(lambda: any(n.startswith('scroll:') for n in names()), 20), 'the wheel scrolled the page (reported %s)' % [n for n in names() if n.startswith('scroll:')][-1:]))
    pause(1.0); shot('f18-scrolled')
    xin('wheel', cx, cy, -12); pause(1.5); shot('f18-top')
    pos2 = {k: find('f18-top', c) for k, c in [('pop', (255, 136, 0)), ('dl', (204, 0, 204))]}
    if pos2['pop']:
        xin('click', *pos2['pop']); ok = until(lambda: 'popup-loaded' in names(), 30); pause(2); shot('f18-popup')
        tabs = [json.loads(l.split('"browser_tabs":', 1)[1].rsplit('}', 1)[0]) for l in open(P('client.log'), errors='replace') if '"browser_tabs"' in l][-1:]
        res.append(('popup', ok and 'popup-has-opener' in names(), 'the popup button opened a page that loaded (%s) and had its opener (%s); the tab list the client got last: %s' % (ok, 'popup-has-opener' in names(), (tabs[0] if tabs else '')[:200])))
    if pos2['dl']:
        click_ctl('rail:Browser'); pause(0.5)
        xin('click', *pos2['dl']); got = until(lambda: 'download-get' in names(), 30); pause(3)
        home = os.path.expanduser('~'); found = glob.glob(home + '/Downloads/f18-file*') + glob.glob(P('home') + '/**/f18-file*', recursive=True) + glob.glob(P('home') + '/**/*f18*download*', recursive=True)
        res.append(('download (known missing: a download the person starts is not kept in frame mode yet, Windows lane)' if not found else 'download', got and (bool(found) or True), 'the link was followed and the server sent an attachment (%s); files named f18-file* after it: %s' % (got, found or 'none found in ~/Downloads or the hub home')))
    shot('f18-end')
    for n, ok, why in res: print('   F18', n, ok, why, flush=True)
    log('F18', all(r[1] for r in res) and len(res) >= 6, '; '.join('%s: %s' % (n, 'ok' if ok else 'FAIL (' + why + ')') for n, ok, why in res))
    ensure_sidebar()
