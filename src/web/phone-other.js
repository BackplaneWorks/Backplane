// The phone arrangement's helpers for everything that is neither the shell,
// the thread nor the viewers (scratch DESIGN.md's area D): the terminal's
// key row, and keeping a field focused in Settings or on the pairing
// screen above the keyboard.
// Nothing here runs unless phone-shell.js made the page a phone's
// (html.phone); a desktop page never gets the row or the listeners.
//
// The key row is the page's own (outside the hub's tree, so patches never
// touch it): Esc, Tab, a sticky Ctrl, the arrows and the characters a phone
// keyboard hides. Each key goes to the terminal as the keyboard would send
// it (host.js's [data-keys] handler: "<key>\t<mods>"); with Ctrl held, the
// next letter typed on the phone's keyboard goes as Ctrl+<letter>.

export const TERM_KEYS = [
  { label: "Esc", key: "Escape" },
  { label: "Tab", key: "Tab" },
  { label: "Ctrl", ctrl: true },
  { label: "^C", key: "c", mods: 4, aria: "Control C" },
  { label: "←", key: "ArrowLeft", aria: "Left" },
  { label: "↑", key: "ArrowUp", aria: "Up" },
  { label: "↓", key: "ArrowDown", aria: "Down" },
  { label: "→", key: "ArrowRight", aria: "Right" },
  { label: "|", key: "|" },
  { label: "~", key: "~" },
  { label: "/", key: "/" },
  { label: "-", key: "-" },
];

// what one key of the row sends: the [data-keys] value for host.js
export function termKeyValue(field, key, mods) {
  const owner = field?.getAttribute?.("data-for");
  return (owner ? owner + "\t" : "") + key + "\t" + (mods | 0);
}

// text typed while Ctrl is held: its first character as Ctrl+<char>, the
// rest as typed (null: nothing to send as a chord)
export function ctrlChord(text) {
  const ch = String(text || "")[0];
  if (!ch) return null;
  return { key: ch.toLowerCase(), rest: String(text).slice(1) };
}

// Search on a phone also finds threads by their titles (the hub's search
// reads only what was said): the sidebar's rows, every word of the query
// in the title, at most `max`; [{id, title, project}]
export function titleHits(doc, query, max = 8) {
  const words = String(query || "").toLowerCase().match(/[\p{L}\p{N}]+/gu) || [];
  if (!words.length) return [];
  const out = [], seen = new Set();
  for (const row of doc.querySelectorAll(".sidebar div.thread[data-value]")) {
    const id = row.getAttribute("data-value"), title = row.querySelector(".title")?.textContent || "";
    if (!id || seen.has(id) || !words.every(w => title.toLowerCase().includes(w))) continue;
    seen.add(id);
    const head = row.closest("section.project")?.querySelector(".project-head");
    const project = head?.querySelector(".name, .title, .label")?.textContent || "";
    out.push({ id, title, project });
    if (out.length >= max) break;
  }
  return out;
}

export function installPhoneOther({ doc = document, win = window, dispatch }) {
  const html = doc.documentElement;
  const on = () => html.classList.contains("phone");
  let row = null, ctrl = false;
  const field = () => doc.querySelector(".floating-terminal [data-keys], .term-pane [data-keys]");
  const setCtrl = (v) => { ctrl = v;row?.querySelector(".ph-term-ctrl")?.setAttribute("aria-pressed", String(v)); };
  const send = (key, mods) => { const f = field();if (f) dispatch(f.getAttribute("data-keys"), termKeyValue(f, key, mods)); };
  const build = () => {
    row = doc.createElement("div");
    row.className = "ph-term-keys";row.setAttribute("role", "toolbar");row.setAttribute("aria-label", "Terminal keys");
    for (const k of TERM_KEYS) {
      const b = doc.createElement("button");
      b.type = "button";b.textContent = k.label;
      if (k.aria) b.setAttribute("aria-label", k.aria);
      if (k.ctrl) { b.className = "ph-term-ctrl";b.setAttribute("aria-pressed", "false"); }
      b.addEventListener("click", () => {
        if (k.ctrl) { setCtrl(!ctrl);return; }
        send(k.key, (k.mods || 0) | (ctrl ? 4 : 0));setCtrl(false);
      });
      row.append(b);
    }
    // a tap on a key keeps the focus (and so the keyboard) in the terminal
    for (const ev of ["pointerdown", "mousedown"]) row.addEventListener(ev, e => e.preventDefault());
    row.addEventListener("touchstart", e => { if (e.target.closest("button")) e.preventDefault(); }, { passive: false });
    row.addEventListener("touchend", e => { const b = e.target.closest("button");if (b) { e.preventDefault();b.click(); } }, { passive: false });
    doc.body.append(row);
  };
  // Ctrl held: the phone keyboard's next character is the chord's
  doc.addEventListener("input", e => {
    if (!ctrl || !e.target.matches?.("[data-keys]")) return;
    const c = ctrlChord(e.target.value);
    if (!c) return;
    e.stopImmediatePropagation();e.target.value = "";
    send(c.key, 4);setCtrl(false);
    if (c.rest) dispatch(e.target.getAttribute("data-text"), (e.target.getAttribute("data-for") ? e.target.getAttribute("data-for") + "\t" : "") + c.rest);
  }, true);
  doc.addEventListener("keydown", e => {
    if (!ctrl || !e.target.matches?.("[data-keys]") || e.key.length !== 1 || e.ctrlKey || e.metaKey || e.isComposing || e.keyCode === 229) return;
    e.preventDefault();e.stopImmediatePropagation();send(e.key.toLowerCase(), 4 | (e.altKey ? 8 : 0));setCtrl(false);
  }, true);

  // a field focused in Settings (or a bot's settings) scrolls up from under
  // the keyboard once the keyboard is up
  let focused = null;
  const scroller = (el) => {
    for (let e = el.parentElement; e; e = e.parentElement) {
      const oy = win.getComputedStyle(e).overflowY;
      if ((oy === "auto" || oy === "scroll") && e.scrollHeight > e.clientHeight) return e;
    }
    return null;
  };
  const lift = () => {
    if (!on() || !focused?.isConnected || doc.activeElement !== focused) return;
    const r = focused.getBoundingClientRect(), vh = win.visualViewport ? win.visualViewport.height : win.innerHeight;
    // the field a third of the way down what the keyboard leaves
    const want = Math.max(8, Math.round(vh / 3) - r.height / 2);
    if (r.bottom <= vh - 12 && r.top >= 0) return;
    const sc = scroller(focused);
    if (sc) sc.scrollTop += r.top - want;
    else focused.scrollIntoView({ block: "center" });
  };
  doc.addEventListener("focusin", e => {
    if (!on() || !e.target.matches?.("input:not([type=checkbox]):not([type=radio]),textarea,select") || e.target.matches("[data-keys], #composer")) return;
    if (!e.target.closest(".settings-view, .settings, .wb-settings, .bot-settings, .project-home, .pairing-screen")) return;
    focused = e.target;setTimeout(lift, 350);
  });
  win.visualViewport?.addEventListener("resize", () => setTimeout(lift, 60));

  return {
    // after every render: the key row shows while a terminal is open
    rendered() {
      if (!on()) return;
      const open = !!field();
      if (open && !row) build();
      if (row) row.hidden = !open;
      html.classList.toggle("ph-term", open);
      if (!open) setCtrl(false);
    },
  };
}
