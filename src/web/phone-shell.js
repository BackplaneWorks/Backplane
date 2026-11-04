// The phone arrangement's host side (view.bend's View.phone.page draws it).
//
// - Whether this browser is a phone: `?phone=1` / `?phone=0` (kept in
//   localStorage, `?phone=auto` forgets it), else a coarse pointer on a
//   screen whose short side is at most 600 px. A phone asks the hub for the
//   phone page with `form: "phone"` in every view request (phoneRequest).
// - The page fits the visual viewport: --ph-vh is its height and --ph-kb the
//   on-screen keyboard's, so the composer stays above the keyboard and the
//   page itself never scrolls.
// - Back: the header's ‹ ([data-ph-back]) is also pressed by the system back
//   (one history entry is kept while a screen is pushed) and by a swipe in
//   from the left edge.
// - Rows: a long press sends [data-ph-long] ("action\u001fvalue"), else
//   what a right-click would ([data-on-contextmenu] with its data-value: a
//   thread's or project's menu); a swipe shows [data-ph-lead] (swiping
//   right) or [data-ph-trail] (left) buttons, JSON lists of {label,
//   action, value, tone}; a sidebar thread row has its own (sidebarSwipes).
// - Menus are bottom sheets (style.css); a tap on a sheet's backdrop shuts it.
// Nothing here runs unless the page is a phone's.

export const PHONE_KEY = "backplane-phone";

// "1", "0" or null (no override) from a page's query and what was kept
export function phoneOverride(search, kept) {
  const asked = new URLSearchParams(search || "").get("phone");
  if (asked === "1" || asked === "0") return { value: asked, keep: asked };
  if (asked === "auto") return { value: null, keep: "" };
  return { value: kept === "1" || kept === "0" ? kept : null, keep: null };
}

export function phoneDecide({ override, coarse, shortSide }) {
  if (override === "1") return true;
  if (override === "0") return false;
  return !!coarse && shortSide > 0 && shortSide <= 600;
}

export function phoneForm(win = window) {
  let kept = null;
  try { kept = win.localStorage.getItem(PHONE_KEY); } catch {}
  const o = phoneOverride(win.location?.search, kept);
  try {
    if (o.keep === "") win.localStorage.removeItem(PHONE_KEY);
    else if (o.keep) win.localStorage.setItem(PHONE_KEY, o.keep);
  } catch {}
  const coarse = typeof win.matchMedia === "function" && win.matchMedia("(pointer: coarse)").matches;
  const s = win.screen || {};
  const shortSide = Math.min(s.width || win.innerWidth || 0, s.height || win.innerHeight || 0);
  return phoneDecide({ override: o.value, coarse, shortSide });
}

// what a view request carries; nothing at all for the ordinary page, so a
// desktop's requests (and the hub's render memo keys) are unchanged
export function phoneRequest(on) { return on ? { form: "phone" } : {}; }

const LONG_MS = 450, SLOP = 8, EDGE = 24, BACK_DX = 64, SWIPE_OPEN = 64;

function level(doc) {
  const app = doc.querySelector(".app.phone");
  if (!app) return 0;
  return app.classList.contains("ph-l2") ? 2 : app.classList.contains("ph-l1") ? 1 : 0;
}

function parseLong(v) {
  if (!v) return null;
  const i = v.indexOf("\u001f");
  return i < 0 ? [v, ""] : [v.slice(0, i), v.slice(i + 1)];
}

function parseList(v) {
  if (!v) return [];
  try { const xs = JSON.parse(v);return Array.isArray(xs) ? xs.filter(x => x && typeof x.action === "string") : []; } catch { return []; }
}

// a sidebar thread row's swipes, as the phone apps' (screen.bend's
// Mob.lead/Mob.trail): right settles it; left snoozes it an hour, or opens
// its menu with Delete armed, which asks there
export function sidebarSwipes(el) {
  if (!el?.matches?.(':is(.sidebar, .ph-now) .thread[data-on-click="select"]')) return null;
  const id = el.getAttribute('data-value') || '';
  if (!id) return null;
  return {
    lead: [{ label: 'Settle', action: 'row-settle', value: id, tone: 'settle' }],
    trail: [{ label: 'Snooze 1h', action: 'row-snooze', value: id + '|1h', tone: '' }, { label: 'Delete', action: 'row-delete', value: id, tone: 'danger', menu: true }],
  };
}

function longAction(el) {
  const own = el.closest?.("[data-ph-long]:not([data-ph-long=''])");
  if (own) return parseLong(own.getAttribute("data-ph-long"));
  const menu = el.closest?.("[data-on-contextmenu]");
  if (menu) return [menu.getAttribute("data-on-contextmenu"), menu.getAttribute("data-value") || ""];
  return null;
}

const SWIPE_SEL = "[data-ph-lead]:not([data-ph-lead='']),[data-ph-trail]:not([data-ph-trail='']),:is(.sidebar, .ph-now) .thread[data-on-click='select']";
function swipeItems(el, side) {
  const own = parseList(el.getAttribute("data-ph-" + side));
  if (own.length) return own;
  return sidebarSwipes(el)?.[side] || [];
}

export function installPhoneShell({ on, doc = document, win = window, dispatch }) {
  if (!on) return { on: false, rendered() {} };
  const html = doc.documentElement;
  html.classList.add("phone");

  // the visual viewport: its height, and how much the keyboard covers
  const vv = win.visualViewport;
  const fit = () => {
    const h = vv ? vv.height : win.innerHeight;
    const kb = vv ? Math.max(0, win.innerHeight - vv.height - vv.offsetTop) : 0;
    html.style.setProperty("--ph-vh", Math.round(h) + "px");
    html.style.setProperty("--ph-kb", Math.round(kb) + "px");
    // iOS pans the visual viewport instead of resizing it: the page's
    // fixed shell follows, so the header never slides off the top
    html.style.setProperty("--ph-top", Math.round(vv ? Math.max(0, vv.offsetTop) : 0) + "px");
    html.classList.toggle("ph-kb", kb > 80);
    // little height left (landscape with the keyboard up): a compact header
    html.classList.toggle("ph-short", h < 360);
    // iOS scrolls the page to show a focused field; the page never scrolls
    if (win.scrollY || win.scrollX) win.scrollTo(0, 0);
  };
  fit();
  vv?.addEventListener("resize", fit);
  vv?.addEventListener("scroll", fit);
  win.addEventListener("resize", fit);
  win.addEventListener("orientationchange", () => setTimeout(fit, 250));

  // what the system back shuts before it goes back a screen: a menu, the
  // row menu, a sheet, search, the palette, the terminal
  const overlay = () => {
    const menu = doc.querySelector(".local-menu[open]:not([data-inline='true'])");
    if (menu) return () => { menu.open = false; };
    if (doc.querySelector(".rowmenu")) return () => dispatch("menu-close", "");
    const sheet = doc.querySelector(".ph-sheet-layer .ph-sheet-back");
    if (sheet) return () => sheet.click();
    const search = doc.querySelector("#thin-result .phead button");
    if (search) return () => search.click();
    if (doc.querySelector(".workspace-palette")) return () => dispatch("ws-palette-close", "");
    if (doc.querySelector(".floating-terminal")) return () => dispatch("ws-term-hide", "");
    return null;
  };
  const back = () => { const b = doc.querySelector(".app.phone .ph-head [data-ph-back]");if (b) { b.click();return true; } return false; };

  // the system back: one entry is kept in the history while a screen is
  // pushed; going back pops it and presses ‹
  let guarded = false, ignorePop = 0;
  win.addEventListener("popstate", () => {
    if (ignorePop) { ignorePop--;return; }
    guarded = false;
    const shut = overlay();
    if (shut) shut();
    else if (level(doc) > 0) back();
  });
  const guard = () => {
    const want = level(doc) > 0 || !!overlay();
    if (want && !guarded) { win.history.pushState({ phone: 1 }, ""); guarded = true; }
    else if (!want && guarded) { guarded = false;ignorePop++;win.history.back(); }
  };

  // a menu opened in the page (no hub frame) keeps a back entry too
  doc.addEventListener("toggle", () => guard(), true);
  // (search and the palette open in the page too, after a tap)
  doc.addEventListener("click", () => setTimeout(guard, 0));

  // touches: the back edge swipe, long press, row swipes
  let t = null, longFired = false, openRow = null;
  const closeRow = () => {
    if (!openRow) return;
    openRow.row.style.transform = "";openRow.row.classList.remove("ph-swiped");openRow.strip.remove();openRow = null;
  };
  const showStrip = (row, side, items) => {
    closeRow();
    const r = row.getBoundingClientRect();
    const strip = doc.createElement("div");
    strip.className = "ph-swipe-strip ph-swipe-" + side;
    strip.style.top = r.top + "px";strip.style.height = r.height + "px";
    for (const it of items) {
      const b = doc.createElement("button");
      b.type = "button";b.className = "ph-swipe-btn" + (it.tone ? " " + it.tone : "");b.textContent = it.label || it.action;
      b.addEventListener("click", e => {
        e.stopPropagation();closeRow();
        // Delete asks first, in the row's menu (the menu opens, Delete armed)
        if (it.menu) dispatch("row-menu", it.value ?? "");
        dispatch(it.action, it.value ?? "");
      });
      strip.append(b);
    }
    doc.body.append(strip);
    const w = strip.getBoundingClientRect().width;
    row.style.transform = `translateX(${side === "lead" ? w : -w}px)`;row.classList.add("ph-swiped");
    openRow = { row, strip };
  };
  doc.addEventListener("touchstart", e => {
    if (e.touches.length !== 1) { t = null;return; }
    const p = e.touches[0], target = e.target;
    if (openRow && !openRow.strip.contains(target)) closeRow();
    const act = longAction(target);
    const swipeEl = target.closest?.(SWIPE_SEL);
    t = { x: p.clientX, y: p.clientY, edge: p.clientX < EDGE && level(doc) > 0 && !target.closest?.("#viewer canvas, .viewer-stage"), swipeEl, axis: null, timer: 0 };
    longFired = false;
    if (act) {
      // a busy page runs the timer late, with the finger's lift still
      // queued: let queued input go first, so a tap stays a tap
      const mine = t;
      t.timer = setTimeout(() => setTimeout(() => {
        if (t !== mine || t.axis) return;
        longFired = true;win.navigator.vibrate?.(10);
        dispatch(act[0], act[1]);
      }, 0), LONG_MS);
    }
  }, { passive: true, capture: true });
  doc.addEventListener("touchmove", e => {
    if (!t) return;
    const p = e.touches[0], dx = p.clientX - t.x, dy = p.clientY - t.y;
    if (!t.axis && Math.hypot(dx, dy) > SLOP) { t.axis = Math.abs(dx) > Math.abs(dy) ? "x" : "y";clearTimeout(t.timer); }
    if (t.axis === "x" && t.swipeEl && !t.edge) t.swipeEl.style.transform = `translateX(${Math.max(-120, Math.min(120, dx))}px)`;
  }, { passive: true, capture: true });
  doc.addEventListener("touchend", e => {
    if (!t) return;
    clearTimeout(t.timer);
    const p = e.changedTouches[0], dx = p.clientX - t.x, dy = p.clientY - t.y, s = t;t = null;
    if (s.axis !== "x") return;
    if (s.edge && dx > BACK_DX && Math.abs(dy) < dx / 2) { back();return; }
    if (s.swipeEl) {
      s.swipeEl.style.transform = "";
      const side = dx > 0 ? "lead" : "trail", items = swipeItems(s.swipeEl, side);
      if (Math.abs(dx) > SWIPE_OPEN && items.length) showStrip(s.swipeEl, side, items);
    }
  }, { passive: true, capture: true });
  doc.addEventListener("touchcancel", () => { if (t) clearTimeout(t.timer);t = null; }, { passive: true, capture: true });
  // a long press's lifting click and the browser's own menu do nothing more
  doc.addEventListener("click", e => {
    if (longFired) { longFired = false;e.preventDefault();e.stopPropagation();return; }
    // a tap on a menu sheet's backdrop (the open <details> itself) shuts it
    const t2 = e.target;
    if (t2?.matches?.(".local-menu[open]")) { t2.open = false;e.preventDefault();e.stopPropagation(); }
  }, true);
  // a phone's own long-press menu (Chrome's contextmenu) would open the
  // same menu again on top of ours, and shut it: the long press owns it
  // (only there: a long press on a reply's text is how one selects it)
  doc.addEventListener("contextmenu", e => {
    if (!e.target.closest?.("[data-on-contextmenu],[data-ph-long]:not([data-ph-long=''])")) return;
    e.preventDefault();e.stopPropagation();
  }, true);

  return {
    on: true,
    back,
    // after every paint: keep the history entry in step with the screen
    rendered() { guard();if (openRow && !openRow.row.isConnected) closeRow(); },
  };
}
