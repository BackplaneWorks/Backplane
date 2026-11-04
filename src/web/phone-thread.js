// The thread screen in the phone arrangement (DESIGN.md: B), where it acts
// unlike a desktop page. Everything here waits for html.phone (the phone
// shell's class), so a desktop or a narrow desktop window is unchanged.

const onPhone = doc => !!doc.documentElement?.classList.contains('phone');

// Return in the draft is a new line on a phone; its Send button sends. A
// phone's keyboard has no Shift+Return, and a thought typed in two lines
// went out as two messages. (Completions keep Return: data-enter "skill".)
export function phoneKeepsReturn(doc, el, action, event) {
  return onPhone(doc) && action === 'send' && el?.id === 'composer' && !event.ctrlKey && !event.metaKey;
}

// A file link in a reply (dom.bend's Dom.file) opens its menu on a phone,
// as its right-click does elsewhere (open it in Backplane's viewer, copy
// its path): a tap opened the file on the hub machine's own screen.
export function phoneClickAction(doc, el, action) {
  return onPhone(doc) && action === 'file-open' && el?.matches?.('a.link.file') ? 'file-menu' : action;
}

// The keyboard coming up (or the draft growing a line) shrinks the timeline
// from below; what was just above the composer stays in view, as a phone's
// messages do, rather than slide under the keyboard:
// - at the end, the timeline stays at its end while the keyboard is up
//   (a draft's view reply put the old top back, a few lines up, ~0.4 s after
//   the first key, with no scroll of the person's);
// - elsewhere its top moves by what its height changed.
// Only a finger's drag (or a wheel) moves it otherwise. Runs after the timeline
// planner's own observer (made later), which keeps the top on a resize.
const NEAR = 48, FINGER_MS = 700;
// The thread hidden behind the phone's full-screen viewer (display: none)
// loses its scroll offset and its planner plans for a view 0 px tall: back
// from the board, the person was somewhere else in the thread. The reading
// place (the planner's own position(): its anchor row and offset) is kept
// while the timeline shows and given back to the planner (restore) when it
// shows again. (list: host.js's timelineList, the timeline's planner)
export function installPhoneThread({ document: doc = document, window: win = window, list = () => null, latest = () => {}, newest = () => true } = {}) {
  // A page that cannot record (no getUserMedia: a plain-http origin) would
  // dictate with the hub machine's microphone, far from the phone; the
  // phone's keyboard has its own dictation (style.css hides the mic)
  if (!win.navigator?.mediaDevices?.getUserMedia) doc.documentElement?.classList.add('ph-nomic');
  const RO = win.ResizeObserver;
  if (!RO) return null;
  let el = null, height = 0, seen = null, end = false, finger = 0, kept = null, keptEl = null, keeping = 0;
  const keep = () => {
    keeping = 0;
    if (!el || el.clientHeight === 0 || !onPhone(doc)) return;
    const l = list(el);
    if (l?.position) { kept = l.position(); keptEl = el; }
  };
  const atEnd = t => t.scrollHeight - t.scrollTop - t.clientHeight < NEAR;
  const shutMore = () => { for (const d of doc.querySelectorAll?.('.composer-more[open]') || []) d.open = false; };
  const keyboard = () => doc.activeElement?.id === 'composer';
  const toEnd = t => { const max = t.scrollHeight - t.clientHeight; if (t.scrollTop < max - 1) t.scrollTop = max; };
  // Far from the end (a screen and a half, or on an earlier page of the
  // history), a ↓ at the timeline's foot goes back to the latest, as a
  // phone's messages do (host.js's holdBottom, the pill's own way down).
  // The hub's "N new ↓" pill says it when something new came: never both.
  let down = null;
  const place = () => {
    const t = el;
    const far = !!t && t.isConnected && t.clientHeight > 0 && onPhone(doc) && !doc.querySelector('.hold-pill')
      && (t.scrollHeight - t.scrollTop - t.clientHeight > t.clientHeight * 1.5 || !newest());
    if (!far) { if (down) down.hidden = true; return; }
    if (!down) {
      down = doc.createElement('button');
      down.type = 'button'; down.className = 'ph-latest'; down.textContent = '↓';
      down.title = 'Go to the latest'; down.setAttribute('aria-label', 'Go to the latest');
      down.addEventListener('click', () => { down.hidden = true; end = true; latest(); });
      doc.body.appendChild(down);
    }
    down.style.top = Math.round(t.getBoundingClientRect().bottom - 56) + 'px';
    down.hidden = false;
  };
  const resized = entries => {
    for (const e of entries) {
      const t = e.target, h = t.clientHeight;
      if (t !== el) continue;
      // (hidden behind the viewer: its ⋯ sheet shut; shown again: its
      // reading place back)
      if (h === 0 && height > 0) shutMore();
      if (height === 0 && h > 0 && kept && keptEl === t && onPhone(doc)) list(t)?.restore?.(kept);
      else if (height > 0 && h > 0 && h !== height && onPhone(doc)) {
        if (end) toEnd(t);
        else t.scrollTop = Math.max(0, Math.min(t.scrollHeight - h, t.scrollTop + (height - h)));
      }
      height = h;
    }
    place();
  };
  // the timeline on show (a thread opened draws another), with an observer
  // made after the planner's, so it runs after the planner's in a frame
  const watch = () => {
    const t = doc.getElementById('timeline');
    if (t === el) return;
    seen?.disconnect();
    el = t; height = t ? t.clientHeight : 0; seen = null; end = !!t && atEnd(t);
    if (t) { seen = new RO(resized); seen.observe(t); }
  };
  const moved = () => { finger = Date.now(); };
  // (a tap anywhere but the composer, a reply's "Replying to" going to its
  // message say, is the person's own move: the end is no longer held; a
  // touch in the composer, before its keyboard comes up, asks afresh)
  doc.addEventListener('touchstart', event => {
    if (!onPhone(doc)) return;
    watch();
    end = !!el && !!event.target?.closest?.('.composer') && atEnd(el);
  }, { capture: true, passive: true });
  for (const type of ['touchstart', 'pointerdown', 'focusin']) doc.addEventListener(type, () => { if (onPhone(doc)) { watch(); place(); } }, { capture: true, passive: true });
  for (const type of ['touchmove', 'wheel']) doc.addEventListener(type, moved, { capture: true, passive: true });
  doc.addEventListener('scroll', event => {
    const t = event.target;
    if (t?.id !== 'timeline' || !onPhone(doc)) return;
    if (t !== el) { watch(); return; }
    if (end && t.scrollHeight - t.scrollTop - t.clientHeight > 2 && keyboard() && Date.now() - finger > FINGER_MS) { toEnd(t); return; }
    end = atEnd(t);
    place();
    if (!keeping) keeping = win.setTimeout(keep, 150);
  }, { capture: true, passive: true });
  // Send (or Stop, Steer) pressed while typing gives the draft its focus
  // back, so the keyboard stays up for the next message, as a phone's
  // messages do. The press still takes the focus first: that blur ends an
  // Android keyboard's composition, which completes the draft that is sent
  // and lets the page clear it (kept focused, the sent text stayed in the
  // field and the next message repeated it). The focus comes back in the
  // same tap (a listener on the window runs after the page's own), which
  // iOS needs to keep its keyboard.
  let back = 0;
  doc.addEventListener('mousedown', event => {
    back = onPhone(doc) && keyboard() && event.target?.closest?.('.composer .foot-end > button') ? Date.now() : 0;
  }, true);
  win.addEventListener?.('click', () => {
    if (!back || Date.now() - back > 1000) { back = 0;return; }
    back = 0;
    doc.getElementById('composer')?.focus?.({ preventScroll: true });
  });
  return { watch };
}
