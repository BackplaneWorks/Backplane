# Rows of gpui-think-rows.py, run inside gpui-f-rows.py's harness (its helpers: xin, shot, until, ctrls, probes, click_ctl, log, P...).
def pause(s=1.2): time.sleep(s)
def chord(c): xin('chord', *c.split())
def drawn():
    """What the window draws now, by name: its probes, then the GPUI_RECT lines of check mode (the newest place of each)."""
    r = {k: v for k, v in probes().items()}
    for l in open(P('client.log'), errors='replace'):
        if l.startswith('GPUI_RECT '):
            try: o = json.loads(l[10:]); r[o['name']] = {k: o[k] / o.get('scale', 1) for k in ('x', 'y', 'w', 'h')}
            except Exception: pass
    return r
def think_id(): return next((k for k in sorted(ctrls()) if k.startswith('think:')), None)
def body():
    return next((v for k, v in drawn().items() if k.startswith('think-body:')), None)
def head(): t = think_id(); return (ctrls().get(t) or {}).get('rect') if t else None
# ---- F27 a thought: shut it shows its first line beside a chevron (one line, no body), as Show thinking says; a click opens the whole of it (three
# paragraphs, the last one's words included) and gives it the keyboard; Enter shuts it, Space opens it again, a click shuts it. Shots f27-*.png.
if want('F27'):
    chord('ctrl k'); pause(1); xin('type', 'Thinks first'); pause(1); xin('key', 'Return'); pause(2.5)
    found = until(lambda: think_id() is not None, 20); tid = think_id(); h0 = head()
    shut0 = found and body() is None and h0 is not None and h0['h'] < 34
    f_shut = shot('f27-folded')
    log('F27a', shut0, 'thread opened; the thought %s is drawn shut: one line (head %s px high), no body (%s)' % (tid, h0 and round(h0['h']), f_shut))
    click_ctl(tid); opened = until(lambda: body() is not None, 10); pause(1); b = body(); f_open = shot('f27-opened')
    log('F27b', opened and b is not None and b['h'] > 3 * (h0 or {'h': 30})['h'], 'a click opened it: its body drawn %s px high (three paragraphs) (%s)' % (b and round(b['h']), f_open))
    xin('key', 'Return'); shut_key = until(lambda: body() is None, 10); pause(0.8); shot('f27-shut-by-enter')
    xin('key', 'space'); open_key = until(lambda: body() is not None, 10); pause(0.8); shot('f27-opened-by-space')
    click_ctl(tid); shut_click = until(lambda: body() is None, 10); pause(0.8); shot('f27-shut-by-click')
    log('F27c', shut_key and open_key and shut_click, 'with the keyboard on it Enter shut it %s and Space opened it %s; a click shut it again %s' % (shut_key, open_key, shut_click))
