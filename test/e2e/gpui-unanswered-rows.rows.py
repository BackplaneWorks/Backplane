# Rows of gpui-unanswered-rows.py, run inside gpui-f-rows.py's harness (its helpers: xin, shot, until, events, log, P...).
def pause(s=1.2): time.sleep(s)
def chord(c): xin('chord', *c.split())
def drawn(): return probes()   # the window's latest probes: what it draws now, by id (x, y, w, h)
def press(rect): xin('click', int(rect['x'] + rect['w'] / 2), int(rect['y'] + rect['h'] / 2))
def say(text): xin('type', text); pause(0.5); xin('key', 'Return'); pause(1.5)
def setting(k):
    v = ''
    for e in events():
        if e.get('$') == 'SettingSet' and e.get('key') == k: v = e.get('value', '')
    return v
# ---- F26 the notice: a person's mid-turn message with no answer for a minute shows "The agent hasn't answered your message yet" with Interrupt and
# Send again; Send again stops the turn and sends the words as a new turn, and the notice goes.
if want('F26'):
    chord('ctrl k'); pause(1); xin('type', 'Pogo connector research'); pause(1); xin('key', 'Return'); pause(2.5)
    say('compare the three probe families in depth')            # a long turn (the stand-in runs FAKE_SECS)
    ok_run = until(lambda: any(e.get('$') == 'TurnChanged' and e.get('thread') == 't6' and e.get('state') == 'running' for e in events()), 15)
    t0 = time.time(); say('Status please')
    early = setting('unanswered.t6'); pause(20); before = setting('unanswered.t6') == '' and not any(k.startswith('unanswered') for k in drawn())
    got = until(lambda: setting('unanswered.t6') != '', 90); after_s = round(time.time() - t0)
    shown = until(lambda: 'unanswered:again' in drawn(), 15); pause(1.5)
    f = shot('f26-unanswered')
    log('F26a', ok_run and early == '' and before and got and shown and 55 <= after_s <= 80,
        'turn running %s; no notice at +20 s %s; the hub set unanswered.t6 after %d s; the window drew it %s (%s)' % (ok_run, before, after_s, shown, f))
    n0 = len(events()); r = drawn().get('unanswered:again')
    if r: press(r)
    def resent():
        ev = events()[n0:]
        stop = next((i for i, e in enumerate(ev) if e.get('$') == 'TurnChanged' and e.get('thread') == 't6' and e.get('state') == 'interrupted'), None)
        new = next((i for i, e in enumerate(ev) if e.get('$') == 'MessagePosted' and e.get('thread') == 't6' and e.get('role') == 'user' and e.get('text') == 'Status please'), None)
        return stop is not None and new is not None and stop < new
    ok_again = until(resent, 20); gone = until(lambda: setting('unanswered.t6') == '' and 'unanswered:again' not in drawn(), 15); pause(1.5); shot('f26-sent-again')
    log('F26b', ok_again and gone, 'Send again: the turn stopped, then "Status please" went as a new message %s; the notice went %s' % (ok_again, gone))
