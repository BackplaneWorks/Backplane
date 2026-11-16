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

# ---- F15 the lightbox (#1112): the pasted picture opened large; Ctrl+= zooms, a double click toggles 100%, a drag moves it, a click on it keeps it open,
# Ctrl+0 fits it, its menu's Save image as writes the file the hub has, Esc and a click on the dimmed page close it. Sizes come from the client's probes.
if want('F15') or want('F15L'):
    pic = lambda: probes().get('lightbox:pic')
    thumbs = lambda: [k for k in probes() if k.startswith('img:') and 'past' in k]
    until(lambda: thumbs(), 15)
    def open_lb():
        t = probes()[thumbs()[0]]; xin('click', int(t['x'] + t['w'] / 2), int(t['y'] + t['h'] / 2)); return until(lambda: pic() is not None, 10)
    opened = bool(thumbs()) and open_lb(); pause(1); shot('lb-open')
    p0 = pic() or {}; fit_ok = abs(p0.get('w', 0) - 64) < 1.5 and abs(p0.get('h', 0) - 48) < 1.5
    chord('ctrl equal'); until(lambda: abs((pic() or {}).get('w', 0) - 80) < 1.5, 5); step_ok = abs((pic() or {}).get('w', 0) - 80) < 1.5
    chord('ctrl 0'); until(lambda: abs((pic() or {}).get('w', 0) - 64) < 1.5, 5)
    c = pic() or {'x': 0, 'y': 0, 'w': 0, 'h': 0}; xin('dblclick', int(c['x'] + c['w'] / 2), int(c['y'] + c['h'] / 2)); until(lambda: abs((pic() or {}).get('w', 0) - 128) < 1.5, 5)
    toggle_ok = abs((pic() or {}).get('w', 0) - 128) < 1.5
    for _ in range(14): chord('ctrl equal'); time.sleep(0.15)
    pause(1); big = pic() or {}; shot('lb-zoomed'); area = probes().get('lightbox:area') or {}
    # a drag over the picture moves it (it is wider than its area now) and does not close it
    cx_, cy_ = int(area.get('x', 0) + 300), int(area.get('y', 0) + area.get('h', 0) / 2)
    xin('drag', cx_, cy_, cx_ + 150, cy_ + 40); pause(1); moved = pic() or {}
    pan_ok = bool(moved) and abs(moved.get('x', 0) - big.get('x', 0)) > 20
    c = pic() or {'x': 0, 'y': 0, 'w': 0, 'h': 0}; xin('click', cx_, cy_); pause(1); stays = pic() is not None
    chord('ctrl 0'); pause(1)
    # its menu: Save image as writes the original (the test hook stands in for the file dialog)
    c = pic() or {'x': 0, 'y': 0, 'w': 0, 'h': 0}; xin('click', int(c['x'] + c['w'] / 2), int(c['y'] + c['h'] / 2), 3); pause(1); shot('lb-menu')
    has_save = 'ctx:Save image as…' in ctrls(); click_ctl('ctx:Save image as…')
    saved = until(lambda: os.path.exists(P('saved.png')) and os.path.getsize(P('saved.png')) > 0, 10)
    stored = glob.glob(P('home/attachments/*/*pasted*.png'))
    same = saved and bool(stored) and open(P('saved.png'), 'rb').read() == open(stored[0], 'rb').read()
    xin('key', 'Escape'); esc_ok = until(lambda: pic() is None, 5)
    reopened = open_lb(); pause(0.5)
    a = probes().get('lightbox:area') or {'x': 0, 'y': 0}; xin('click', int(a['x'] + 20), int(a['y'] + 20)); bg_ok = until(lambda: pic() is None, 5)
    log('F15-lightbox', opened and fit_ok and step_ok and toggle_ok and pan_ok and stays and has_save and same and esc_ok and reopened and bg_ok,
        'the pasted 64x48 picture opened large=%s at its own size=%s (lb-open.png); Ctrl+= made it 80 px wide=%s; a double click toggled to 2x (fit is 100%% here)=%s; 14 more Ctrl+= gave %s px (lb-zoomed.png) in an area %s px wide; a drag moved it=%s (x %s -> %s); a click on it kept it open=%s; its right-click menu had Save image as=%s (lb-menu.png) and the file written equals the stored attachment=%s; Esc closed it=%s; reopened=%s and a click on the dimmed page closed it=%s'
        % (opened, fit_ok, step_ok, toggle_ok, round(big.get('w', 0)), round(area.get('w', 0)), pan_ok, round(big.get('x', 0)), round(moved.get('x', 0)), stays, has_save, same, esc_ok, reopened, bg_ok))

# ---- F15C Copy image (the person's report: it put nothing usable on the clipboard; gpui offered every copy as text, a picture as an empty
# string). A picture an agent's reply draws from its absolute path, right-clicked, Copy image: the X clipboard offers image/png with the file's
# own bytes and nothing else (no text, no uri-list: snap Firefox pasted a uri-list's file over the image); Ctrl+V in the composer attaches it; Chrome, Firefox and GIMP
# each paste a picture of its size. A 20 MB picture (past one X request: sent in INCR segments; drawn from a smaller copy) copies whole.
def xclip_out(target, timeout=20):
    try: return subprocess.run(['xclip', '-selection', 'clipboard', '-o', '-t', target], env=denv, capture_output=True, timeout=timeout).stdout
    except Exception: return b''
def say_picture(path):
    r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); pause(0.5)
    xin('type', 'zqecho'); chord('shift Return'); xin('type', path); pause(0.3); xin('key', 'Return')
    return until(lambda: ('img:' + os.path.basename(path)) in probes(), 30)
def copy_picture(name):
    t = probes().get('img:' + name)
    if not t: return False
    xin('click', int(t['x'] + t['w'] / 2), int(t['y'] + t['h'] / 2), 3); pause(1)
    return click_ctl('ctx:Copy image')
def browser_paste(which, html):
    """Chrome or Firefox on this display, a page that logs what a Ctrl+V gave it (its items and the picture's size); None when it could not run."""
    page = 'data:text/html;base64,' + __import__('base64').b64encode(html.encode()).decode()
    if which == 'chrome':
        prof = tempfile.mkdtemp(prefix='bpchr.', dir=OUT)
        p = spawn(['google-chrome', '--user-data-dir=' + prof, '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--enable-logging=stderr', '--v=0', '--ozone-platform=x11', '--app=' + page], 'chrome.log', denv)
        lg, title = P('chrome.log'), 'pastetest'
    else:
        prof = os.path.expanduser('~/snap/firefox/common/bp-copyimg-%d' % os.getpid()); os.makedirs(prof, exist_ok=True)   # the snap reads no dot folder
        open(os.path.join(prof, 'user.js'), 'w').write('user_pref("devtools.console.stdout.content", true);\nuser_pref("browser.shell.checkDefaultBrowser", false);\nuser_pref("datareporting.policy.dataSubmissionEnabled", false);\nuser_pref("browser.aboutwelcome.enabled", false);\nuser_pref("dom.events.testing.asyncClipboard", true);\n')
        open(os.path.join(prof, 'paste.html'), 'w').write(html)   # a data: page from the command line is refused as a top-level load
        # the snap moves itself into a scope of its own through systemd on the session bus, so it gets the desktop's bus (a private one has no
        # systemd); --no-remote --new-instance keep it apart from any Firefox the person runs
        fenv = dict(denv, MOZ_ENABLE_WAYLAND='0'); fenv.update({k: os.environ[k] for k in ['DBUS_SESSION_BUS_ADDRESS'] if k in os.environ})
        p = spawn(['firefox', '--no-remote', '--new-instance', '--profile', prof, 'file://' + os.path.join(prof, 'paste.html')], 'firefox.log', fenv)
        lg, title = P('firefox.log'), 'pastetest'
    got = lambda: [l.split('PASTE ', 1)[1].split('"')[0].rstrip() for l in open(lg, errors='replace') if 'PASTE ' in l]
    seen = None
    if until(lambda: 'READY' in open(lg, errors='replace').read(), 40):
        for _ in range(3):
            xin('focus', title); pause(1); chord('ctrl v')
            if until(lambda: got(), 8): seen = got()[-1]; break
    try: p.terminate(); p.wait(10)
    except Exception: pass
    if which == 'firefox': shutil.rmtree(prof, ignore_errors=True)
    return seen
PASTE_PAGE = '<title>pastetest</title><body style="margin:0"><div id=e contenteditable style="width:100vw;height:100vh">x</div><script>e.focus();console.log("READY");document.addEventListener("paste",ev=>{const it=[...ev.clipboardData.items].map(i=>i.kind+":"+i.type).join(",");const f=ev.clipboardData.files[0];const say=n=>console.log("PASTE items="+it+";size="+n);if(f){createImageBitmap(f).then(b=>say(b.width+"x"+b.height),e=>say("undecodable "+f.size+" bytes "+e))}else say("none")})</script>'
def gimp_paste():
    """GIMP on this display, its window given the keyboard, Edit > Paste as New Image (Ctrl+Shift+V): the share of the screen in the copied
    picture's yellow before and after (GIMP's PDB paste reads its own buffer, not the system clipboard, so its window is driven)."""
    from PIL import Image
    p = spawn(['dbus-run-session', '--', 'gimp-3.2', '-n', '-d', '-f', '-s', '--no-splash'], 'gimp.log', dict(denv, HOME=P('fakehome')))
    def yellow(n):   # in GIMP's half of the screen (i3 tiles it right of Backplane, which draws the same picture)
        im = px(n); w, h = im.size
        return sum(1 for c in im.crop((w // 2, 0, w, h)).getdata() if c[0] > 230 and 180 < c[1] < 220 and c[2] < 30)
    up = until(lambda: subprocess.run(['python3', XIN, 'focus', 'GNU Image'], env=denv, capture_output=True).returncode == 0, 90)
    pause(6)
    if subprocess.run(['python3', XIN, 'focus', 'Welcome'], env=denv, capture_output=True).returncode == 0: xin('key', 'Escape'); pause(1)   # its Welcome dialog
    xin('focus', 'GNU Image'); pause(1); shot('f15c-gimp-before'); chord('ctrl shift v'); pause(5); shot('f15c-gimp-pasted')
    a, b = yellow('f15c-gimp-before'), yellow('f15c-gimp-pasted')
    try: p.terminate(); p.wait(10)
    except Exception: pass
    return up, a, b
if want('F15C'):
    from PIL import Image
    small = P('proj', 'copy-me.png'); im = Image.new('RGB', (96, 64), (30, 90, 200))
    for x in range(30, 60):
        for y in range(10, 50): im.putpixel((x, y), (250, 200, 0))
    im.save(small); want_bytes = open(small, 'rb').read()
    drawn = say_picture(small); pause(1); shot('f15c-drawn')
    copied = drawn and copy_picture('copy-me.png'); pause(1.5)
    targets = xclip_out('TARGETS').decode(errors='replace').split()
    png = xclip_out('image/png')
    clip_ok = sorted(targets) == ['SAVE_TARGETS', 'TARGETS', 'image/png'] and png == want_bytes
    log('F15C-copy', copied and clip_ok, 'a 96x64 picture the reply drew from %s (f15c-drawn.png); right-click, Copy image=%s; the X clipboard offers %s (image/png only); its image/png is the file byte for byte=%s (%d bytes)'
        % (os.path.relpath(small, OUT), copied, targets, png == want_bytes, len(png)))
    # back into Backplane's own composer: Ctrl+V attaches it
    r = rects()['composer']; xin('click', int(r['x'] + r['w'] / 2), int(r['y'] + 30)); pause(0.5); shot('f15c-before'); chord('ctrl v'); pause(2.5); shot('f15c-pasted')
    chip = differs(px('f15c-before'), px('f15c-pasted'), (int(r['x']), int(r['y'] - 120), int(r['x'] + r['w']), int(r['y'] + r['h'])))
    xin('type', 'zqecho pasted back'); pause(0.3); xin('key', 'Return')
    sent = until(lambda: any(e['$'] == 'MessagePosted' and e.get('role') == 'user' and 'pasted back' in e.get('text', '') and 'attachments' in json.dumps(e) for e in events()), 20)
    stored = sorted(glob.glob(P('home/attachments/*/*')), key=os.path.getmtime)
    same = bool(stored) and open(stored[-1], 'rb').read() == want_bytes
    log('F15C-composer', chip and sent and same, "Ctrl+V in Backplane's composer: an attachment appeared=%s (f15c-pasted.png); the message posted with it=%s; the attachment is the picture byte for byte=%s" % (chip, sent, same))
    # each app that is installed here (a runner without one says so and is not judged on it)
    have = {a: bool(shutil.which(b)) for a, b in (('chrome', 'google-chrome'), ('firefox', 'firefox'), ('gimp', 'gimp-3.2'))}
    copy_picture('copy-me.png'); pause(1.5)
    ch = browser_paste('chrome', PASTE_PAGE) if have['chrome'] else 'not installed'; copy_picture('copy-me.png'); pause(1.5)
    ff = browser_paste('firefox', PASTE_PAGE) if have['firefox'] else 'not installed'; copy_picture('copy-me.png'); pause(1.5)
    gup, gy0, gy1 = gimp_paste() if have['gimp'] else (False, 0, 0)
    ch_ok = not have['chrome'] or bool(ch and 'file:image/png' in ch and ch.endswith('96x64'))
    ff_ok = not have['firefox'] or bool(ff and 'image/png' in ff and ff.endswith('96x64'))
    gp_ok = not have['gimp'] or (gup and gy1 > gy0 + 200)
    log('F15C-apps', ch_ok and ff_ok and gp_ok,
        'pasted into Chrome: %r; Firefox: %r (the page logs the clipboard items and the pasted file decoded); GIMP installed=%s, opened=%s and Paste as New Image drew the picture: yellow pixels %d -> %d (f15c-gimp-pasted.png)' % (ch, ff, have['gimp'], gup, gy0, gy1))
    # a 20 MB picture: past one X request (INCR), drawn from a smaller copy (Copy image fetches the original)
    big = P('proj', 'big-noise.png'); Image.frombytes('RGB', (2800, 2400), os.urandom(2800 * 2400 * 3)).save(big, compress_level=0); big_bytes = open(big, 'rb').read()
    drawn_big = say_picture(big); pause(2); copy_picture('big-noise.png')
    got_big = b''
    for _ in range(20):
        got_big = xclip_out('image/png', 60)
        if got_big == big_bytes: break
        time.sleep(1)
    log('F15C-big', drawn_big and got_big == big_bytes, 'a %.1f MB 2800x2400 picture (drawn from a 2048 px copy): Copy image put the original on the clipboard and xclip read it whole (INCR)=%s (%d of %d bytes)'
        % (len(big_bytes) / 1e6, got_big == big_bytes, len(got_big), len(big_bytes)))

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
    # an uberbot to open (the rig's home has none: Ctrl+Shift+U would only have redrawn the thread)
    print('    make bub', subprocess.run(['bun', os.path.join(REPO, 'test/cef/hub_rpc.mjs'), str(PORT), P('home/token'), json.dumps([{'m': 'bots.create', 'p': {'name': 'Bub', 'uber': True}}])], capture_output=True, text=True, timeout=30).stdout.strip(), flush=True)
    until(lambda: any(k.startswith('bot:') for k in ctrls()), 15)
    trial('uber', 'ctrl shift u', lambda a, b, e, m: (differs(px('k-uber-a'), px('k-uber-b'), box(MAIN())), "the uberbot's chat opened"))
    # the uberbot's chat, a project's page over it, then the uberbot's row: its chat is back (the page used to stay up: the hub's selection had not moved)
    ub = next((k for k in ctrls() if k.startswith('bot:')), '')
    click_ctl('project:p1'); pause(1.2); shot('k-ubp-page'); on_page = 'page-new-thread' in ctrls()
    click_ctl(ub); pause(1.5); shot('k-ubp-back'); left = 'page-new-thread' not in ctrls()
    ok = bool(ub) and on_page and left and differs(px('k-ubp-page'), px('k-ubp-back'), box(MAIN()))
    res.append(('uber-after-project', 'click', ok, 'project page shown=%s, then a click on %r: the page gave way=%s (k-ubp-page.png, k-ubp-back.png)' % (on_page, ub, left))); print('    uber-after-project', ok, flush=True)
    # the same from a Needs you row (docs/ALWAYS-ON.md section 8): the project's page, then the row of the thread that asks: its thread opens
    click_ctl('project:p1'); pause(1.2); shot('k-nyp-page'); on_page = 'page-new-thread' in ctrls(); listed = 'thread:t4' in ctrls()
    click_ctl('thread:t4'); pause(1.5); shot('k-nyp-back'); left = 'page-new-thread' not in ctrls()
    ok = listed and on_page and left and differs(px('k-nyp-page'), px('k-nyp-back'), box(MAIN()))
    res.append(('needs-after-project', 'click', ok, 'Needs you lists t4=%s, project page shown=%s, then its row: the page gave way=%s (k-nyp-page.png, k-nyp-back.png)' % (listed, on_page, left))); print('    needs-after-project', ok, flush=True)
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
        if not until(lambda: (shot('v-%s' % kind) and busy('v-%s' % kind)), 25):   # a page that has not drawn is asked again once (open it afresh), then waited for
            click_ctl('rail:Files'); pause(1.5); click_ctl(label); pause(1.0); until(lambda: (shot('v-%s' % kind) and busy('v-%s' % kind)), 40)
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
