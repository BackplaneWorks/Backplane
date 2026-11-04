// On a phone (and any screen with nothing but touch) a text field the page
// focuses by itself raises the on-screen keyboard over half the screen: a
// sheet, a dialog or a thread that opens with its field focused is covered
// before it is read, and a note box that was only being looked at takes the
// keyboard. There, the page focuses a text field only where the person
// asked to type in it: a search they opened (the palette, the sidebar's
// project search, a thread search), a paste box. Everywhere else they tap
// the field, and that tap brings the keyboard. Desktop (a mouse or a
// trackpad anywhere, a physical keyboard's user) keeps every focus.

// FocusField ids (core/client.bend, web/workspace.bend) a touch screen
// still focuses: each is a search the person has just opened
export const ASKED_TO_TYPE = new Set(["workspace-query", "proj-find"]);

// nothing but touch: the phone arrangement, or a coarse pointer and no fine
// one anywhere (a touch laptop or a tablet with a trackpad has one)
export function touchOnly(win = globalThis) {
  const doc = win.document;
  if (doc?.documentElement?.classList.contains("phone")) return true;
  const mq = q => typeof win.matchMedia === "function" && win.matchMedia(q).matches;
  return mq("(pointer: coarse)") && !mq("(any-pointer: fine)");
}

// a field the on-screen keyboard comes up for
export function typingField(el) {
  if (!el || el.readOnly || el.disabled) return false;
  if (el.isContentEditable) return true;
  const tag = String(el.tagName || "").toUpperCase();
  if (tag === "TEXTAREA") return true;
  return tag === "INPUT" && !/^(button|submit|checkbox|radio|range|color|file|image|reset|hidden)$/i.test(el.type || "text");
}

// focus el, unless it is a text field the page would be focusing by itself
// on a touch screen (asked: the person asked to type there); true if it did
export function softFocus(el, { asked = false, win = globalThis, ...options } = {}) {
  if (!el) return false;
  if (!asked && typingField(el) && touchOnly(win)) return false;
  el.focus(options);
  return true;
}
