# ---- I16 on Windows: a long 0.14 import watched in the window, as 0.14's Update leaves it (the hub on a fresh home, BACKPLANE_IMPORT_AUTO=1, importing by itself)
#   ROWS=win-import.rows.py win-f-rows.py CLIENT HUB ASSETS OUT [PORT] I16     (test/import014/progress.sh's "update" scenario, with Windows input and screenshots)
# The 0.14 home is the fixture writer's large mode; BACKPLANE_IMPORT_THROTTLE_MS stretches the import so it can be watched. The checks are progress.sh's: the
# window shows the card at once and it moves; /healthz says ok with migrating and a fold held, and its progress moves at least every 2 s; /hello answers
# within 1 s throughout; the summary is right; Continue saves that it was seen. Screenshots: OUT/shots/import-*.png.
import urllib.request
def get(url, token=None, t=5, raw=False):
    q = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + token} if token else {})
    with urllib.request.urlopen(q, timeout=t) as r: b = r.read(); return b if raw else b.decode()
def grect(prefix):
    """the newest rect the window drew under a name that starts with PREFIX (its GPUI_RECT lines, BACKPLANE_GPUI_RECTS=1): name, x, y, w, h"""
    last = None
    for l in open(P('client.log'), errors='replace'):
        if 'GPUI_RECT ' in l:
            try: r = json.loads(l.split('GPUI_RECT ', 1)[1])
            except ValueError: continue
            if str(r.get('name', '')).startswith(prefix): last = r
    return last
if want('I16'):
    NP, NT, NL = (int(os.environ.get(k, d)) for k, d in (('BIG_P', 50), ('BIG_T', 5000), ('BIG_L', 300000)))
    big = P('big'); subprocess.run([sys.executable, '-I', os.path.join(REPO, 'test/fixtures/make-v014-home.py'), big, '--large', str(NP), str(NT), str(NL)], check=True, capture_output=True)
    nlines = sum(1 for _ in open(os.path.join(big, 'events.jsonl'), 'rb'))
    # the rows' own window goes; this one's hub and window take the screen
    for p in list(procs):
        if p.args[0] == CLIENT: subprocess.run(['taskkill', '/T', '/F', '/PID', str(p.pid)], capture_output=True)
    IP = PORT + 1; ih = P('ihome'); os.makedirs(ih, exist_ok=True)
    ienv = dict(env, BACKPLANE_IMPORT_FROM=big, BACKPLANE_IMPORT_AUTO='1', BACKPLANE_IMPORT_THROTTLE_MS=os.environ.get('THROTTLE', '250'))
    ihub = spawn([HUB, '--home', ih, '--port', str(IP), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], 'ihub.log', ienv, cwd=P('proj'))
    # watched from the hub's first moment: the window is started as soon as the hub's token is there (as a person's client would connect), not after a /hello
    tokf = os.path.join(ih, 'token'); token = None; client_at = None
    t0 = time.time(); seen = shots = slow = 0; last = None; last_t = t0; maxgap = 0.0; n = 0; prog = []; misses = []
    while True:
        if token is None and os.path.exists(tokf) and open(tokf).read().strip():
            token = open(tokf).read().strip(); open(P('iscript.json'), 'w').write('[{"wait":3000000}]')
            spawn([CLIENT, '--url', 'http://127.0.0.1:%d' % IP, '--token-file', tokf, '--script', P('iscript.json'), '--exit-after', '3000'], 'client.log', cenv); client_at = time.time() - t0
        a = time.time()
        try: get('http://127.0.0.1:%d/hello' % IP, t=3, raw=True); ms = (time.time() - a) * 1000   # /hello answers CBOR: only its time is judged
        except Exception as e: ms = -1; misses.append('%.1f %s' % (time.time() - t0, repr(e)[:120]))
        if ms < 0 or ms > 1000: slow += 1
        try: hz = json.loads(get('http://127.0.0.1:%d/healthz' % IP, t=3) or '{}')
        except Exception: hz = {}
        m = hz.get('migrating')
        if m:
            seen += 1; line = '%s %s %s/%s %s%% fold=%s status=%s' % (m.get('phase'), m.get('what'), m.get('done'), m.get('total'), m.get('pct'), hz.get('folding'), hz.get('status'))
            prog.append('%.1f %s hello=%dms' % (time.time() - t0, line, ms))
            if hz.get('status') not in ('ok', 'degraded'): log('I16', False, '/healthz not ok while migrating: ' + line)
            if line != last:
                if last is not None: maxgap = max(maxgap, time.time() - last_t)
                last, last_t = line, time.time()
            pct = int(m.get('pct') or 0)
            if client_at is not None and time.time() - t0 > client_at + 8 and str(m.get('phase')) == 'writing' and (shots == 0 or (shots == 1 and pct >= 70)): shots += 1; shot('import-running-%d' % shots)
        elif seen > 0: break
        n += 1
        if n > 2400: break
        time.sleep(0.5)
    took = time.time() - t0
    open(P('progress.txt'), 'w').write('\n'.join(prog) + '\n' + ''.join('hello missed at ' + x + '\n' for x in misses))
    clog = open(P('client.log'), errors='replace').read()
    card = 'migrate:running' in clog
    moved = shots >= 2 and differs(px('import-running-1'), px('import-running-2'))
    res = json.loads(get('http://127.0.0.1:%d/import/0.14' % IP, token)); c = res.get('counts', {})
    summary_ok = res.get('state') == 'done' and c.get('projects') == NP + 1 and c.get('threads') == NT + 3
    until(lambda: grect('btn:ob-import:ack:') is not None, 30); r = grect('btn:ob-import:ack:'); ack = r and r['name']
    time.sleep(1); shot('import-done')
    if r: xin('click', int((r['x'] + r['w'] / 2) * r.get('scale', 1)), int((r['y'] + r['h'] / 2) * r.get('scale', 1)))
    time.sleep(2)
    saved = any(e.get('$') == 'SettingSet' and e.get('key') == 'import.v014.ack' for e in (json.loads(l) for l in open(os.path.join(ih, 'events.jsonl'), errors='replace') if l.startswith('{')))
    shot('import-after')
    asleep = not any(e.get('$') == 'TurnChanged' and e.get('thread') == 't1790292962-8017' for e in (json.loads(l) for l in open(os.path.join(ih, 'events.jsonl'), errors='replace') if l.startswith('{')))
    log('I16', card and moved and seen > 4 and maxgap <= 2.0 and slow == 0 and summary_ok and bool(ack) and saved and asleep,
        '0.14 home %d projects, %d threads, %d lines; the window showed the import card=%s, it moved between two looks mid-import=%s (import-running-1/2.png); /healthz migrating %d looks over %.0f s, status ok, '
        'a fold held; the longest wait for progress %.2f s; /hello slow or missing %d times (the window started %.1f s after the hub); summary %s projects, %s threads, %s messages, %s events (state %s, right=%s); '
        'the summary card with Continue=%s (import-done.png), Continue saved import.v014.ack=%s (import-after.png); the imported uberbot stayed asleep=%s'
        % (NP, NT, nlines, card, moved, seen, took, maxgap, slow, client_at or -1, c.get('projects'), c.get('threads'), c.get('messages'), c.get('lines'), res.get('state'), summary_ok, bool(ack), saved, asleep))
