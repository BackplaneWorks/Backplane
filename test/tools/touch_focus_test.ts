// The web client on a touch screen focuses a text field by itself only for
// a search just opened (src/web/touch-focus.js): a sheet, a dialog or a
// thread that opens never raises the on-screen keyboard; the tap on the
// field does. Desktop keeps every focus.
//   bun test/tools/touch_focus_test.ts
import { softFocus, touchOnly, typingField, ASKED_TO_TYPE } from "../../src/web/touch-focus.js";

let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };

// a window: the phone arrangement (html.phone) or not, and what its pointers are
const win = ({ phone = false, coarse = false, fine = true } = {}) => ({
  document: { documentElement: { classList: { contains: (c: string) => phone && c === "phone" } } },
  matchMedia: (q: string) => ({ matches: q === "(pointer: coarse)" ? coarse : q === "(any-pointer: fine)" ? fine : false }),
});
const el = (tagName: string, extra: Record<string, unknown> = {}) => { const e: any = { tagName, type: "", focused: 0, ...extra }; e.focus = () => { e.focused++; }; return e; };

const phone = win({ phone: true, coarse: true, fine: false }), tablet = win({ coarse: true, fine: false });
const touchLaptop = win({ coarse: true, fine: true }), desktop = win();
check(touchOnly(phone) && touchOnly(tablet), "the phone arrangement and a touch-only screen are touch screens");
check(!touchOnly(touchLaptop) && !touchOnly(desktop), "a touch laptop (a trackpad too) and a desktop are not");

check(typingField(el("TEXTAREA")) && typingField(el("INPUT", { type: "text" })) && typingField(el("INPUT")) && typingField(el("DIV", { isContentEditable: true })), "text fields raise the keyboard");
check(!typingField(el("BUTTON")) && !typingField(el("INPUT", { type: "checkbox" })) && !typingField(el("TEXTAREA", { readOnly: true })) && !typingField(el("INPUT", { disabled: true })), "buttons, checkboxes, read-only and disabled fields do not");

// opening a thread (the composer), a rename (its title), a note, a new bot's name: no keyboard on a phone
for (const tag of ["TEXTAREA", "INPUT"]) {
  const f = el(tag);
  check(softFocus(f, { win: phone }) === false && f.focused === 0, `a phone: a ${tag.toLowerCase()} the page opens is not focused`);
  const t = el(tag);
  check(softFocus(t, { win: tablet }) === false && t.focused === 0, `a touch-only tablet: nor there`);
}
// a search the person opened: focused, the keyboard comes up for it
{ const q = el("INPUT"); check(softFocus(q, { win: phone, asked: true }) && q.focused === 1, "a phone: a search just opened is focused"); }
check(ASKED_TO_TYPE.has("workspace-query") && ASKED_TO_TYPE.has("proj-find"), "the palette's and the project search's fields are asked for");
check(!["composer", "thread-title", "bot-name", "term", "intent", "room-post", "routine-name", "model-q"].some(id => ASKED_TO_TYPE.has(id)), "the composer, rename, bot, terminal, note and picker fields are not");
// what raises no keyboard is focused as before (the keyboard users' tab order, dialogs' buttons)
{ const b = el("BUTTON"), r = el("TEXTAREA", { readOnly: true }); check(softFocus(b, { win: phone }) && b.focused === 1 && softFocus(r, { win: phone }) && r.focused === 1, "a phone: buttons and read-only fields are focused as before"); }
// desktop and a touch laptop: every focus as before
for (const [name, w] of [["desktop", desktop], ["a touch laptop", touchLaptop]] as const) {
  const f = el("TEXTAREA");
  check(softFocus(f, { win: w }) && f.focused === 1, `${name}: a field the page opens is focused as before`);
}
// options pass through to focus(), and nothing is not an error
{ let got: any; const f: any = el("TEXTAREA"); f.focus = (o: any) => { got = o; }; softFocus(f, { win: desktop, preventScroll: true }); check(got?.preventScroll === true && !("win" in got) && !("asked" in got), "focus options pass through"); }
check(softFocus(null, { win: phone }) === false, "no element: nothing");
if (fail) { console.log(`${fail} failed`); process.exit(1); }
console.log("all ok");
