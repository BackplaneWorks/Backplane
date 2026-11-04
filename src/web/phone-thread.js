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
export function installPhoneThread({ document: doc = document, window: win = window } = {}) {
  // A page that cannot record (no getUserMedia: a plain-http origin) would
  // dictate with the hub machine's microphone, far from the phone; the
  // phone's keyboard has its own dictation (style.css hides the mic)
  if (!win.navigator?.mediaDevices?.getUserMedia) doc.documentElement?.classList.add('ph-nomic');
  const RO = win.ResizeObserver;
  if (!RO) return null;
  let el = null, height = 0, seen = null, end = false, finger = 0;
  const atEnd = t => t.scrollHeight - t.scrollTop - t.clientHeight < NEAR;
  const keyboard = () => doc.activeElement?.id === 'composer';
  const toEnd = t => { const max = t.scrollHeight - t.clientHeight; if (t.scrollTop < max - 1) t.scrollTop = max; };
  const resized = entries => {
    for (const e of entries) {
      const t = e.target, h = t.clientHeight;
      if (t !== el) continue;
      if (height > 0 && h !== height && onPhone(doc)) {
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
  for (const type of ['touchstart', 'pointerdown', 'focusin']) doc.addEventListener(type, () => { if (onPhone(doc)) watch(); }, { capture: true, passive: true });
  for (const type of ['touchmove', 'wheel']) doc.addEventListener(type, moved, { capture: true, passive: true });
  doc.addEventListener('scroll', event => {
    const t = event.target;
    if (t?.id !== 'timeline' || !onPhone(doc)) return;
    if (t !== el) { watch(); return; }
    if (end && !atEnd(t) && keyboard() && Date.now() - finger > FINGER_MS) { toEnd(t); return; }
    end = atEnd(t);
  }, { capture: true, passive: true });
  return { watch };
}
