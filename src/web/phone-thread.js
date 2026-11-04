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
export function installPhoneThread({ document: doc = document, window: win = window, list = () => null } = {}) {
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
  const keyboard = () => doc.activeElement?.id === 'composer';
  const toEnd = t => { const max = t.scrollHeight - t.clientHeight; if (t.scrollTop < max - 1) t.scrollTop = max; };
  const resized = entries => {
    for (const e of entries) {
      const t = e.target, h = t.clientHeight;
      if (t !== el) continue;
      // (shown again after being hidden: its reading place back)
      if (height === 0 && h > 0 && kept && keptEl === t && onPhone(doc)) list(t)?.restore?.(kept);
      else if (height > 0 && h > 0 && h !== height && onPhone(doc)) {
        if (end) toEnd(t);
        else t.scrollTop = Math.max(0, Math.min(t.scrollHeight - h, t.scrollTop + (height - h)));
      }
      height = h;
    }
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
  for (const type of ['touchstart', 'pointerdown', 'focusin']) doc.addEventListener(type, () => { if (onPhone(doc)) watch(); }, { capture: true, passive: true });
  for (const type of ['touchmove', 'wheel']) doc.addEventListener(type, moved, { capture: true, passive: true });
  doc.addEventListener('scroll', event => {
    const t = event.target;
    if (t?.id !== 'timeline' || !onPhone(doc)) return;
    if (t !== el) { watch(); return; }
    if (end && t.scrollHeight - t.scrollTop - t.clientHeight > 2 && keyboard() && Date.now() - finger > FINGER_MS) { toEnd(t); return; }
    end = atEnd(t);
    if (!keeping) keeping = win.setTimeout(keep, 150);
  }, { capture: true, passive: true });
  // (Send leaves the keyboard to go down: keeping the draft's focus on a
  // press of Send left an Android keyboard's word being composed out of
  // what was sent, which only the field's blur commits)
  return { watch };
}
