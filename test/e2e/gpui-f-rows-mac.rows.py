# CAL: what the window, rects and controls are on this runner (printed for the rows' coordinates), and that a real click reaches the client
if want('CAL'):
    print('screen', screen_size(), 'window content', win(), flush=True)
    r = rects(); print('rects', json.dumps({k: [round(v) for v in (x['x'], x['y'], x['w'], x['h'])] for k, x in r.items()}), flush=True)
    c = ctrls(); print('controls', len(c), sorted(c)[:60], flush=True)
    shot('cal-0'); n0 = len(metrics()); ok = click_ctl('thread:t2'); time.sleep(2); shot('cal-1')
    sel_changed = differs(px('cal-0'), px('cal-1'))
    log('CAL', ok and sel_changed, 'clicked thread:t2 with a real mouse click: clicked=%s, the window redrew=%s' % (ok, sel_changed))
