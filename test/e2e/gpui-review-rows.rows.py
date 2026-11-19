# Rows of gpui-review-rows.py, run inside gpui-f-rows.py's harness (its helpers: click_ctl, ctrls, events, shot, xin, until, log, P...).
def pause(s=1.2): time.sleep(s)
def review_set():
    """The values `review.ro.t6` took, in order (agents/review_mode.rs: "1" read-only, "0" the person switched it off)."""
    return [e['value'] for e in events() if e.get('$') == 'SettingSet' and e.get('key') == 'review.ro.t6']
def said(prefix): return any(e.get('$') == 'ActivityLogged' and e.get('thread') == 't6' and str(e.get('summary', '')).startswith(prefix) for e in events())
def away(): xin('move', 5, 5); pause(0.6)   # the pointer off the button: its tooltip does not cover the shot
def button(): return (ctrls().get('ui:review') or {}).get('label')

# ---- F26 the review bar: a plain review request makes the thread read-only, the bar over the message box says so and why, and its one button switches
# review mode off and back on through the hub (`thread.review`), which logs each press. Judged from the hub's log and the client's controls; the shots show it.
if want('F26'):
    xin('chord', 'ctrl', 'k'); pause(1); xin('type', 'Board review'); pause(1); xin('key', 'Return'); pause(2.5)
    shot('f26-0-before'); none_before = button() is None
    c = rects().get('composer')
    if c: xin('click', int(c['x'] + c['w'] / 2), int(c['y'] + c['h'] / 2)); pause(0.5)
    xin('type', 'review the board'); pause(0.5); xin('key', 'Return')
    on = until(lambda: review_set()[-1:] == ['1'], 20) and until(lambda: button() == 'Allow edits', 15); pause(1.5)
    why = said('Read-only: this request looked like a review'); away(); shot('f26-1-read-only')
    pressed_off = click_ctl('ui:review')
    off = until(lambda: review_set()[-1:] == ['0'], 10) and until(lambda: button() == 'Read-only again', 10); pause(1.5)
    told_off = said('You switched review mode off'); away(); shot('f26-2-edits-allowed')
    pressed_on = click_ctl('ui:review')
    again = until(lambda: review_set()[-1:] == ['1'], 10) and until(lambda: button() == 'Allow edits', 10); pause(1.5)
    told_on = said('You switched review mode on'); away(); shot('f26-3-read-only-again')
    log('F26', none_before and on and why and pressed_off and off and told_off and pressed_on and again and told_on,
        'no bar before (%s); "review the board" made it read-only (%s, the line said why: %s); Allow edits pressed (%s) switched it off (%s, logged: %s); Read-only again pressed (%s) switched it back on (%s, logged: %s); values %s'
        % (none_before, on, why, pressed_off, off, told_off, pressed_on, again, told_on, review_set()))
