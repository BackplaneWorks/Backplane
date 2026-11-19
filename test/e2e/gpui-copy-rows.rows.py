# Rows of gpui-copy-rows.py, run inside gpui-f-rows.py's harness (its helpers: click_ctl, probes, shot, xin, chord, until, log, P...).
# The seed (gpui-copy-rows.seed.py) puts the same reply in a project thread (t1), the uberbot Bub's chat and the room `bringup`.
def pause(s=1.2): time.sleep(s)
def chord(c): xin('chord', *c.split())
def clip():
    try: return subprocess.run(['xclip', '-selection', 'clipboard', '-o', '-t', 'UTF8_STRING'], env=denv, capture_output=True, timeout=10).stdout.decode()
    except Exception: return None
def clip_set(t): subprocess.run(['xclip', '-selection', 'clipboard', '-i'], input=t.encode(), env=denv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)   # its owner goes with the X server
def drawn():
    """Every region the window reported (GPUI_RECT lines, physical px; the last place of each name)."""
    r = {}
    for l in open(P('client.log'), errors='replace'):
        if l.startswith('GPUI_RECT '):
            try: o = json.loads(l[10:]); r[o['name']] = {k: o[k] / o.get('scale', 1) for k in ('x', 'y', 'w', 'h')}
            except Exception: pass
    return r
def probe_like(prefix, contains=''):
    """The first button drawn whose name starts with `prefix` (and holds `contains`): a copy or run button (copyable.rs reports `copy:<id>`, `run:<id>`)."""
    for k, v in sorted(drawn().items()):
        if k.startswith(prefix) and contains in k: return k, v
    return None, None
def press(rect): xin('click', int(rect['x'] + rect['w'] / 2), int(rect['y'] + rect['h'] / 2))
BLOCK = 'echo zq > %s'
SURFACES = [('thread', 'thread:t5', 'zq-thread.marker', P('proj'), 'cp1'), ('bub', 'bot:Bub', 'zq-bub.marker', P('fakehome'), 'cp2'), ('room', 'room:bringup', 'zq-room.marker', P('fakehome'), '')]
def open_surface(ctl):
    if ctl.startswith('thread:'):   # a project's thread, by the command palette as a person would: Ctrl+K, its title, Enter
        chord('ctrl k'); pause(1); xin('type', 'Needs a package'); pause(1); xin('key', 'Return'); pause(2.5)
        return until(lambda: any(k.startswith('copy:ic-cp1-') for k in drawn()), 10)
    ok = until(lambda: ctl in ctrls(), 20) and click_ctl(ctl); pause(2.5); return ok

# ---- F23 the copy button on each chat surface: a click copies the exact text (an inline command; a fenced block whole, no newline after it),
# the button shows its check, and with the button holding the keyboard Ctrl+C copies again. The clipboard is read back from X (xclip).
if want('F23'):
    for name, ctl, marker, _, msg in SURFACES:
        opened = open_surface(ctl); shot('f23-%s-open' % name)
        # the inline command: its button in a timeline is `copy:ic-...`; a room's code pieces are `copy:talk-...` / `copy:post-...`
        k_in, r_in = probe_like('copy:ic-' + msg + '-') if name != 'room' else probe_like('copy:talk-', '-0')
        k_bl, r_bl = probe_like('copy:code-' + msg + '-') if name != 'room' else probe_like('copy:talk-', '-2')
        clip_set('before'); got_in = None
        if r_in: press(r_in); until(lambda: clip() == INLINE, 5); got_in = clip(); pause(0.3); shot('f23-%s-copied' % name)
        clip_set('before'); got_bl = None
        if r_bl: press(r_bl); until(lambda: clip() == BLOCK % marker, 5); got_bl = clip()
        # the keyboard: the block's button has it after the click; Ctrl+C copies its text again
        clip_set('before'); chord('ctrl c'); until(lambda: clip() == BLOCK % marker, 5); got_key = clip()
        ok = opened and got_in == INLINE and got_bl == BLOCK % marker and got_key == BLOCK % marker
        log('F23-%s' % name, ok, '%s: opened=%s; the inline command\'s button (%s) copied %r; the fenced block\'s (%s) copied %r; Ctrl+C on the focused button copied %r (f23-%s-copied.png)'
            % (name, opened, k_in, got_in, k_bl, got_bl, got_key, name))

# ---- F24 Run in terminal: in a project thread and in Bub's chat the command's Run in terminal opens the floating terminal with the command typed,
# not run (its marker file is not written while the person has not pressed Enter); Enter runs it, in the project folder for the thread and in the
# home folder for Bub. A room's terminal opens too (its marker in the home folder: the room has no project).
if want('F24'):
    for name, ctl, marker, where, msg in SURFACES:
        opened = open_surface(ctl)
        k, r = probe_like('run:code-' + msg + '-') if name != 'room' else probe_like('run:talk-', '-2')
        if r: press(r)
        pause(3.5); shot('f24-%s-typed' % name)
        found = lambda: [f for f in glob.glob(P('**', marker), recursive=True)]
        not_run = not found()
        xin('key', 'Return'); ran = until(lambda: bool(found()), 10)
        places = found(); right = any(os.path.dirname(f) == where for f in places)
        pause(1); shot('f24-%s-ran' % name)
        chord('shift Escape'); pause(1)
        log('F24-%s' % name, opened and r is not None and not_run and ran and right,
            '%s: Run in terminal (%s) pressed; 3.5 s later the command had not run (no %s)=%s (f24-%s-typed.png); Enter ran it=%s, in %s (want %s)'
            % (name, k, marker, not_run, name, ran, [os.path.relpath(os.path.dirname(f), OUT) for f in places], os.path.relpath(where, OUT)))

# ---- F25 a selection shows over inline code: in Bub's chat a drag across the inline command tints the command's shaded
# background (its run used to paint that shade over the selection, so the text was selected but showed no highlight there), and Ctrl+C copies what is selected.
if want('F25'):
    from collections import Counter
    opened = open_surface('bot:Bub'); pause(1)
    k, r = probe_like('copy:ic-cp2-'); _, rr = probe_like('run:ic-cp2-')
    if r and rr: r = dict(r, x=min(r['x'], rr['x']))   # the command's buttons start at its end: Run in terminal, then copy
    def shade(img):   # the commonest colour in a strip just inside the code span's end, its background (not its glyphs)
        x1 = int(r['x']) - 20; y = int(r['y'] + r['h'] / 2)
        return Counter(list(img.crop((x1 - 40, y - 2, x1, y + 2)).getdata())).most_common(1)[0][0]
    shot('f25-before'); before = shade(px('f25-before')) if r else None
    clip_set('before')
    # from the command's end back into it (the drag stays inside the span: what it tints is the code's shade alone)
    if r: xin('drag', int(r['x']) - 10, int(r['y'] + r['h'] / 2), int(r['x']) - 150, int(r['y'] + r['h'] / 2)); pause(0.8)
    shot('f25-selected'); after = shade(px('f25-selected')) if r else None
    chord('ctrl c'); until(lambda: (clip() or 'before') != 'before', 5); got = clip() or ''
    log('F25', opened and r is not None and before != after and len(got) > 5 and got in INLINE,
        'Bub: drag over the inline command (%s); its shade %s before, %s selected (f25-selected.png); Ctrl+C copied %r' % (k, before, after, got[-80:]))
