# ---- F18 on Windows: the Browser tab is the hub's helper's frames (no embedded Chromium in a release): open a page by its address, click, type, scroll,
# a popup and a download, all with real input on the frames. Judged from screenshots of the frame, the client's tab list and the file that lands.
MAIN = lambda: rects()['main']
def pause(s=1.2): time.sleep(s)
def sel(i): click_ctl('thread:t%d' % i); pause(1.0)
def chord(c): xin('chord', *c.split())
if want('F18'):
    import http.server, functools
    from PIL import Image
    pg = P('page'); os.makedirs(pg, exist_ok=True)
    open(os.path.join(pg, 'index.html'), 'w').write('''<!doctype html><html><head><title>f18</title></head>
<body style="margin:0;height:4000px;background:linear-gradient(#202020,#e0e0e0)">
<button id=b style="position:absolute;left:40px;top:40px;width:260px;height:90px;background:#00ff00;border:0" onclick="document.body.style.background='#ff0000'">click</button>
<input id=t style="position:absolute;left:40px;top:170px;width:360px;height:60px;font-size:40px;border:6px solid #0000ff;background:#ffffff">
<a href="popup.html" target="_blank" style="position:absolute;left:40px;top:270px;width:300px;height:60px;display:block;background:#ff00ff"></a>
<a href="file.txt" download style="position:absolute;left:40px;top:360px;width:300px;height:60px;display:block;background:#00ffff"></a>
</body></html>''')
    open(os.path.join(pg, 'popup.html'), 'w').write('<!doctype html><title>popup</title><body style="background:#ffff00">popup</body>')
    open(os.path.join(pg, 'file.txt'), 'w').write('f18 download\n')
    srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=pg))
    threading.Thread(target=srv.serve_forever, daemon=True).start(); url = 'http://127.0.0.1:%d/index.html' % srv.server_address[1]
    VIEW = (826, 143, 1367, 899)   # the frame below the browser's bar (1440x900 client area, no banner)
    def find(name, ok):   # the bounding box of the pixels `ok` likes in the frame of shot `name`
        im = px(name).crop(VIEW); w, h = im.size; d = im.load(); xs, ys = [], []
        for y in range(0, h, 2):
            for x in range(0, w, 2):
                if ok(*d[x, y]): xs.append(x); ys.append(y)
        return (VIEW[0] + min(xs), VIEW[1] + min(ys), VIEW[0] + max(xs), VIEW[1] + max(ys)) if len(xs) > 20 else None
    green = lambda r, g, b: g > 200 and r < 80 and b < 80
    blue = lambda r, g, b: b > 180 and r < 80 and g < 80
    red = lambda r, g, b: r > 200 and g < 60 and b < 60
    mag = lambda r, g, b: r > 200 and b > 200 and g < 80
    cyan = lambda r, g, b: g > 200 and b > 200 and r < 80
    centre = lambda bb: ((bb[0] + bb[2]) // 2, (bb[1] + bb[3]) // 2)
    tabs = lambda: [m for m in metrics() if 'browser_tabs' in json.dumps(m)]
    def tab_count():
        n = 0
        for l in open(P('client.log'), errors='replace'):
            if '"browser_tabs"' in l:
                try: n = len(json.loads(json.loads(l.split('GPUI_LOG ', 1)[1])['browser_tabs']).get('tabs', []))
                except Exception: pass
        return n
    res = []
    sel(1); click_ctl('rail:Browser'); pause(12); shot('b0-open')
    xin('click', 1150, 122); pause(0.5); chord('ctrl a'); xin('type', url); xin('key', 'Return'); pause(8); shot('b1-page')
    bb = find('b1-page', green); res.append(('page drawn (its button seen in the frame)', bool(bb)))
    if bb:
        xin('click', *centre(bb)); pause(4); shot('b2-clicked'); res.append(('a click on the button reached the page (it turned red)', bool(find('b2-clicked', red))))
    tb = find('b1-page', blue)
    if tb:
        xin('click', *centre(tb)); pause(1); xin('type', 'hello windows'); pause(3); shot('b3-typed')
        res.append(('typing reached the field', differs(px('b2-clicked' if bb else 'b1-page'), px('b3-typed'), (tb[0] + 8, tb[1] + 8, tb[2] - 8, tb[3] - 8))))
    else: res.append(('typing reached the field', False)); shot('b3-typed')   # the next step compares with it
    for _ in range(6): xin('wheel', 1100, 600, -1)
    pause(3); shot('b4-scrolled'); res.append(('the wheel scrolled the page', differs(px('b3-typed'), px('b4-scrolled'), VIEW)))
    for _ in range(12): xin('wheel', 1100, 600, 1)
    pause(3); shot('b5-top'); n0 = tab_count()
    pb = find('b5-top', mag)
    if pb: xin('click', *centre(pb)); pause(6); shot('b6-popup')
    res.append(('the popup opened as a new tab (%d -> %d tabs)' % (n0, tab_count()), bool(pb) and tab_count() > n0))
    dl = os.path.join(P('fakehome'), 'Downloads', 'file.txt')
    xin('click', 860, 88); pause(4); shot('b7-back')   # back to the page's own tab (the popup took the front)
    db = find('b7-back', cyan)
    if db: xin('click', *centre(db)); until(lambda: os.path.exists(dl), 30); shot('b8-download')
    res.append(('the download landed in Downloads', os.path.exists(dl) and open(dl).read() == 'f18 download\n'))
    for what, ok in res: print('   ', 'ok' if ok else 'FAIL', what, flush=True)
    log('F18', all(ok for _, ok in res), '; '.join('%s=%s' % (w, o) for w, o in res) + ' (b*.png)')
# ---- F17 Mech with the release's helpers beside the hub: the STEP part drawn on the Mech page (a mesher may take a while on a VM)
if want('F17M'):
    def drawn(name):
        im = px(name).crop((840, 150, 1360, 880)).convert('L'); bg = im.getpixel((5, 5)); return sum(1 for v in im.getdata() if abs(v - bg) > 25) > 4000
    sel(1); click_ctl('rail:Mech'); ok = until(lambda: shot('m-mech') and drawn('m-mech'), 180)
    log('F17-mech-window', ok, 'the Mech page drew the STEP part (m-mech.png) within 180 s=%s' % ok)
