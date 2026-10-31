// Backplane browser host.
//
// This file only moves data: JSON <-> Bend values, a keyed DOM patch of the
// Node tree app.bend's view returns, event delegation back to app.bend's
// act, and the socket. No product decision is made here (AGENTS.md: the UI
// is dumb).

import App from "./app.bend";
import { applyComposerDraft } from "./composer-draft.js";
import Planner from "./timeline-plan.bend";
import History from "./timeline-history.bend";
import { TimelineScroller } from "./timeline-scroller.js";
import { installImageActions } from "./image-actions.js";
import { BubMoveObserver, showBubMove } from "./bub-move.js";
const bubMoves = new BubMoveObserver((before, after) => Wire.plain(App.bub_move(JSON.stringify(before), JSON.stringify(after))));
function bubMoveSync() {
  if (document.visibilityState === "hidden") { bubMoves.disconnect(location.origin);return; }
  if (!(frontendNode && remoteReady || canonicalBoot && App.synced(ui))) return;
  const snapshot = root.querySelector("[data-bub-home]")?.getAttribute("data-bub-home");
  if (!snapshot) return;
  try {
    const move = bubMoves.observe(location.origin, JSON.parse(snapshot));
    if (move) showBubMove(move);
  } catch { /* Older hubs may not supply a lease snapshot. */ }
}
document.addEventListener("visibilitychange", () => { bubMoves.disconnect(location.origin); });
import { installUsageTabs } from "./usage-tabs.js";
import { installWorkspaceKeys, installComposerFocus } from "./workspace-keys.js";
import { installFloatingTerminal } from "./floating-terminal.js";
installUsageTabs();
import { installWorkspaceMenus } from "./workspace-menus.js";
installWorkspaceMenus(document, () => composerKeeper.claim(true));
import { installSubagentInspector } from "./subagent-inspector.js";
installSubagentInspector();
import { installViewerFeedback } from "./viewer-feedback.js";
const viewerFeedback = installViewerFeedback();
import { installWorkspaceHints } from "./workspace-hints.js";
// the keyboard kept in the open thread's composer (workspace-keys.js) and
// Esc's choice of what to put away (workspace.bend's WS.esc.next); the
// probe goes off its plots at once, as its chip's x does through the hub
const composerKeeper = installComposerFocus({ phone: () => composerPhone() });
installWorkspaceKeys({ dispatch, escNext: (term, probe, viewer) => App.esc_next(term, probe, viewer), composer: composerKeeper,
  clearProbe: () => { Plot2d.setProbe("");Solid.setProbe("");dispatch("probe", ""); } });
installWorkspaceHints();
installFloatingTerminal({ dispatch });
import { ConnectionLog } from "./connection-log.js";
import { frameSummary } from "./frame-summary.js";
import { pairBrowser } from "./auth.js";

// Consume the secret before the generic deeplink handler clears the fragment.
const pairingReady = pairBrowser({ location, history, storage: localStorage, fetch: window.fetch.bind(window) }).catch(() => false);
import * as Solid from "./solid.js";
import * as ViewerTools from "./viewer-tools.js";
import { ViewerRpc } from "./viewer-rpc.js";
import * as Plot2d from "./plot2d.js";
import { installDesktopBrowser } from "./desktop-browser.js";
import { restoreCache } from "./replay.js";
import { FocusedCache, FocusedView, captureFocused, cacheIdentity, mergeFocused, scopeKey } from "./focused-cache.js";
import { refreshCacheIdentity } from "./cache-identity.js";
import { discoverViews } from "./thin-discovery.js";
import { Backoff, Retry, watchWake, probeHub, bundleOf, styleOf, deployed, QUIET_MS } from "./reconnect.js";
import { ThinHost, preservePresentationRoute, presentationTarget, refusedFallback } from "./thin-host.js";
import { TerminalOutputPane } from "./terminal-output.js";
import { ModelFeedback } from "./model-feedback.js";
const modelFeedback = new ModelFeedback(() => later());
import { BodyWindows } from "./body-windows.js";
import { ThinContinuations } from "./thin-continuations.js";
import { previewTextFiles } from "./text-preview.js";
import { DraftAttachments } from "./draft-attachments.js";
import { ThinMailDraft, mailReplyField } from "./mail-drafts.js";
import { ThinViews, viewCapability } from "./thin.js";
import * as Wire from "./wire.js";
import { openDraw } from "./draw.js";
import { ViewTransition, withTheme } from "./view-transition.js";
import { T, is, learnJson, learnDom } from "./wire.js";

const CONNECTION_BUNDLE = bundleOf(import.meta.url);

// The theme the newest authoritative frame painted (a hub view, or this
// page's own synced log): the one authority for every picture shown before
// the hub's next view, whichever thread it was retained for. Kept across
// loads so the first paint already has it.
const THEME = "backplane-theme";
let themeNow = "";
try { themeNow = localStorage.getItem(THEME) || ""; } catch {}
// the desktop window keeps the same word for its cold start (desktop/main.cjs)
let desktopTheme = window.backplaneDesktop?.theme || "";
if (!themeNow) themeNow = desktopTheme;
function themeDocument() {
  document.documentElement.classList.toggle("light", themeNow === "light");
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", themeNow === "light" ? "#fafaf8" : "#111410");
}
themeDocument();
function themeSeen(app) {
  const theme = !app ? "" : app.classList.contains("light") ? "light" : "dark";
  if (theme && theme !== desktopTheme && window.backplaneDesktop?.themeSeen) { desktopTheme = theme;window.backplaneDesktop.themeSeen(theme); }
  if (!theme || theme === themeNow) return;
  themeNow = theme;themeDocument();
  try { localStorage.setItem(THEME, theme); } catch {}
}
const themed = shown => withTheme(shown, themeNow);

// the constructors' tags as this bundle names them (wire.js)
learnJson(App.json_tags());
learnDom(App.dom_tags());

// bytes <-> Bend List<U32>
function toList(u8) {
  let xs = { $: "Nil" };
  for (let i = u8.length - 1; i >= 0; i -= 1) xs = { $: "Con", head: u8[i], tail: xs };
  return xs;
}

function fromList(xs) {
  const out = [];
  for (const b of each(xs)) out.push(b);
  return new Uint8Array(out);
}

function* each(list) {
  for (let xs = list; xs && is(xs, "Con"); xs = xs.tail) yield xs.head;
}

// DOM patch
// ---------

const EVENTS = ["click", "input", "change", "contextmenu"];
window.backplaneDesktop?.onFocusAddress?.(() => {
  const address = document.querySelector('[data-bfield^="brurl:"]');
  if (address) { address.focus();address.select(); }
});

// icons (src/web/icon.bend) are inline SVG, which needs its namespace
const SVG_NS = "http://www.w3.org/2000/svg";
const SVG_TAGS = new Set(["svg", "path"]);

function create(v) {
  if (is(v, "Txt")) return document.createTextNode(v.text);
  const el = SVG_TAGS.has(v.tag) ? document.createElementNS(SVG_NS, v.tag) : document.createElement(v.tag);
  el.__v = { $: T.El, tag: v.tag, key: v.key, attrs: { $: "Nil" }, kids: { $: "Nil" } };
  el.__kids = [];
  patch(el, v);
  return el;
}

// bots' cats: an <img data-cat="<look>:<mood>:<px>"> shows app.bend's
// SVG for that name, made once per name and kept
const CATS = new Map();
function catUri(name) {
  let u = CATS.get(name);
  if (u === undefined) {
    u = App.cat(name);
    CATS.set(name, u);
  }
  return u;
}

function setAttrs(el, oldAttrs, attrs) {
  const seen = new Set();
  // Read the new attributes, not an old/recycled element's field owner.
  let field = null;
  for (const a of each(attrs)) if (is(a, "At") && a.name === "data-bfield") field = a.value;
  for (const a of each(attrs)) {
    if (is(a, "At")) {
      seen.add(a.name);
      if (a.name === "value") {
        const value = field !== null && viewMode === "view-v1" && frontendNode ? thin?.fieldValue(field, thinMailDraft.fieldValue(field, a.value)) ?? a.value : a.value;
        // A hub refresh can arrive between typing and the blur that saves a
        // to-do. Keep that focused edit until its change event captures it.
        const editingTodo = el === document.activeElement && el.getAttribute("data-on-change") === "todo-text" &&
          [...each(oldAttrs)].some(old => is(old, "At") && old.name === "value" && old.value !== el.value);
        if (!editingTodo && el.value !== value) el.value = value;
      } else if (el.getAttribute(a.name) !== a.value) {
        el.setAttribute(a.name, a.value);
        if (a.name === "data-cat") el.src = catUri(a.value);
      }
    } else {
      const name = "data-on-" + a.event;
      seen.add(name);
      if (el.getAttribute(name) !== a.action) el.setAttribute(name, a.action);
    }
  }
  for (const a of each(oldAttrs)) {
    const name = is(a, "At") ? a.name : "data-on-" + a.event;
    if (!seen.has(name)) {
      if (name === "value") el.value = "";
      else el.removeAttribute(name);
    }
  }
}

// A piece of the page is left for this side to make (view.bend's View.memo:
// a "bp-memo" element, its attributes and its one text all it is made from):
// app.bend's make makes it, and what it made is kept while the element
// stays the same, so patch finds the very node it drew last time and skips
// it whole. Pieces not on the page for a while are let go.
const MEMO = new Map(); // key -> { k: the element, v: what it stands for, gen }
let memoGen = 0;
function sameMemo(a, b) {
  let x = a.attrs, y = b.attrs;
  for (; is(x, "Con") && is(y, "Con"); x = x.tail, y = y.tail) {
    const p = x.head, q = y.head;
    if (p.$ !== q.$ || p.name !== q.name || p.value !== q.value || p.event !== q.event || p.action !== q.action) return false;
  }
  if (is(x, "Con") || is(y, "Con")) return false;
  const s = is(a.kids, "Con") ? a.kids.head : null, t = is(b.kids, "Con") ? b.kids.head : null;
  return !!s && !!t && s.text === t.text;
}
function expand(k) {
  if (k.tag !== "bp-memo" || !is(k, "El")) return k;
  const id = (is(k.attrs, "Con") ? k.attrs.head.value : "") + "\u0000" + k.key;
  const m = MEMO.get(id);
  if (m && sameMemo(m.k, k)) {
    m.gen = memoGen;
    return m.v;
  }
  const v = App.make(k);
  MEMO.set(id, { k, v, gen: memoGen });
  return v;
}
function memoSweep() {
  memoGen += 1;
  if (MEMO.size < 512) return;
  for (const [id, m] of MEMO) if (memoGen - m.gen > 8) MEMO.delete(id);
}

function patchKey(v) {
  if (!is(v, "El")) return "";
  if (v.key) return v.key;
  if (v.tag !== "button") return "";
  // Fold controls are entries too. Their first value byte is the next
  // open/closed state; the remaining entry id survives a toggle or prepend.
  // Older hubs emitted these without a VNode key. Never recycle one for a
  // different run merely because it is another unkeyed button.
  const attrs = [...each(v.attrs)];
  if (attrs.some(a => is(a, "At") && a.name === "class" && a.value.split(/\s+/).includes("fold"))
    && attrs.some(a => !is(a, "At") && a.event === "click" && a.action === "fold")) {
    const id = attrs.find(a => is(a, "At") && a.name === "data-value")?.value.slice(1);
    if (id) return "fold:" + id;
  }
  return "";
}
function same(a, b) {
  return a.$ === b.$ && (is(a, "Txt") || (a.tag === b.tag && patchKey(a) === patchKey(b)));
}

// make el (whose last vnode is el.__v) match v
function patch(el, v) {
  const old = el.__v;
  setAttrs(el, old.attrs, v.attrs);
  if (el.id === "timeline") {
    let scroller = timelineLists.get(el);
    if (!scroller) {
      scroller = new TimelineScroller(el, {
        plan: input => Wire.plain(Planner.timeline_plan(presentationJson(input))), create, patch,
        key: (node, i) => History.display_key_at(node, BigInt(i)),
        onChange: timelineChanged,
      });
      timelineLists.set(el, scroller);
    }
    scroller.update([...each(v.kids)].map(expand), { follow: !(thin && thin.atNewest?.() === false) });
    el.__v = v;
    return;
  }
  const kids = [];
  for (const k of each(v.kids)) kids.push(expand(k));
  const prev = el.__kids;
  const byKey = new Map();
  const free = [];
  for (const p of prev) {
    if (patchKey(p.v)) byKey.set(p.v.tag + "\u0000" + patchKey(p.v), p);
    else free.push(p);
  }
  const next = [];
  let f = 0;
  for (let i = 0; i < kids.length; i += 1) {
    const k = kids[i];
    let hit = null;
    const key = patchKey(k);
    if (key) {
      hit = byKey.get(k.tag + "\u0000" + key) ?? null;
      if (hit) byKey.delete(k.tag + "\u0000" + key);
    } else {
      while (f < free.length && !same(free[f].v, k)) f += 1;
      if (f < free.length) hit = free[f++];
    }
    let node;
    if (hit) {
      node = hit.node;
      if (is(k, "Txt")) {
        if (node.nodeValue !== k.text) node.nodeValue = k.text;
      } else if (hit.v !== k) {
        patch(node, k);
      }
      hit.used = true;
    } else {
      node = create(k);
    }
    next.push({ v: k, node });
  }
  // the nodes that went go first, so a kept one already in its place is
  // never moved: moving the focused field (the composer, when the menu
  // above it shuts) takes the keyboard from it
  for (const p of prev) if (!p.used && p.node.parentNode === el) el.removeChild(p.node);
  const act = document.activeElement;
  const held = act && act !== document.body && el.contains(act) ? act : null;
  const s0 = held?.selectionStart, s1 = held?.selectionEnd;
  for (let i = 0; i < next.length; i += 1) {
    const at = el.childNodes[i];
    if (at !== next[i].node) el.insertBefore(next[i].node, at ?? null);
  }
  // a node moved all the same (unkeyed siblings matched by place) keeps
  // the keyboard and the caret it had
  if (held && held.isConnected && document.activeElement !== held) {
    held.focus({ preventScroll: true });
    if (s0 != null) try { held.setSelectionRange(s0, s1); } catch {}
  }
  for (const p of next) p.used = false;
  el.__kids = next;
  el.__v = v;
}

// Timing, for a bench only: a page whose window has __bpPerf (set before
// it loads) gets each message's handling, each render's view and patch, in
// ms, pushed onto its arrays. Nothing is timed otherwise.
const PERF = globalThis.__bpPerf || null;
const pnow = () => performance.now();
function trace(phase, detail = {}) {
  if (PERF?.trace) PERF.trace.push({ phase, ms: pnow(), epochMs: performance.timeOrigin + pnow(), ...detail });
}
let canonicalBoot = false;

// Loop
// ----

const root = document.getElementById("root");
root.__v = { $: T.El, tag: "div", key: "", attrs: { $: "Nil" }, kids: { $: "Nil" } };
root.__kids = [];

const now = () => BigInt(Math.floor(Date.now() / 1000));
// this browser's id: part of every message id, so a resend after a dropped
// link is recognised and stored once
const cid = (() => {
  let c = localStorage.getItem("backplane-cid");
  if (!c) {
    c = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
    localStorage.setItem("backplane-cid", c);
  }
  return c;
})();

// the wire's dictionaries (core/cbor.bend), for wire.js
const listOf = (l) => { const out = []; for (let x = l; x && is(x, "Con"); x = x.tail) out.push(x.head); return out; };
const KEYS = listOf(App.cbor_keys());
const WORDS = listOf(App.cbor_words());

let ui = App.page(App.init(now(), cid), location.origin);
if (window.backplaneDesktop) ui = App.desktop(ui);
if (window.backplaneDesktop?.browser) ui = App.desktop_browser(ui);
const desktopBrowser = installDesktopBrowser({bridge:window.backplaneDesktop?.browser,
  isReady:()=>(canonicalBoot||remoteReady)&&socket?.readyState===1,
  onTakeover:owner=>dispatch("tk-takeover",owner)});
const forcedLang = new URLSearchParams(location.search).get("lang");
if (forcedLang === "ja" || forcedLang === "en") ui = App.lang_preview(ui);
ui = App.act(ui, "lang", forcedLang === "ja" || forcedLang === "en" ? forcedLang : (localStorage.getItem("backplane-lang") || "auto")).ui;
ui = App.act(ui, "lang-os", navigator.languages?.[0] || navigator.language || "").ui;
document.documentElement.lang = App.lang(ui);
window.addEventListener("languagechange", () => dispatch("lang-os", navigator.languages?.[0] || navigator.language || ""));
// opened from another hub to show one of this machine's bots ("#bot=<name>")
if (location.hash) {
  try { ui = App.wanted(ui, decodeURIComponent(location.hash)); } catch {}
  history.replaceState(null, "", location.pathname + location.search);
}
// drafts this browser kept (Keep commands), one key per thread
const DRAFT = "backplane-draft:";
// the outbox (see outboxKeep below), declared before anything can send
const OUTBOX = "backplane-outbox", OUTBOX_DAY = 86400000;
const outbox = new Map();
try { for (const o of JSON.parse(localStorage.getItem(OUTBOX) || "[]")) if (o?.msg && Date.now() - o.at < OUTBOX_DAY) outbox.set(o.msg, o); } catch {}
for (let i = 0; i < localStorage.length; i += 1) {
  const k = localStorage.key(i);
  if (k && k.startsWith(DRAFT)) ui = App.restore(ui, k.slice(DRAFT.length), localStorage.getItem(k) ?? "");
}
// Per-thread panel choices belong to this browser, never to hub settings.
const SUBS_CHOICE = "backplane-subs:";
function restoreSubs() {
  try {
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k?.startsWith(SUBS_CHOICE)) {
        const thread = k.slice(SUBS_CHOICE.length);
        const choice = localStorage.getItem(k) ?? "";
        if (choice === "1") ui = App.act(ui, "subs-open", thread).ui;
        else if (choice.startsWith("0")) ui = App.act(ui, "subs-shut", thread + "\t" + choice.slice(1)).ui;
      }
    }
  } catch {}
}
restoreSubs();
// Each thread's viewer (open or shut, its tab, file and sheet) belongs to
// this browser too: kept as client.bend's Viewer.keep packs it whenever
// the hub's answer moves it, and put back before the first view, so a
// reload opens the thread as it was (a fresh view starts from it)
const VIEWER = "backplane-viewer:";
try {
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k?.startsWith(VIEWER)) ui = App.viewer_restore(ui, k.slice(VIEWER.length), localStorage.getItem(k) ?? "");
  }
} catch {}
// The model picker opens up from the composer's top edge and is never
// taller than the room above it (style.css's --composer-top), so a long
// draft does not push its heads out of the window. Kept as the composer
// grows or shrinks (a keystroke redraws only the composer) and as the
// window does.
let composerSeen = null, composerTopKept = "";
// Measured by the observer (its first look at a new composer too), on a
// resize and while the picker is open, not in every render.
const composerRoom = typeof ResizeObserver === "function" ? new ResizeObserver(() => composerTop(true)) : null;
function composerTop(measure = !composerRoom) {
  const c = document.querySelector(".model-layer")?.closest(".composer") || null;
  if (c !== composerSeen) {
    if (composerSeen) composerRoom?.unobserve(composerSeen);
    if (c) composerRoom?.observe(c);
    composerSeen = c;
  }
  if (!measure && !c?.querySelector('.model-layer[aria-hidden="false"]')) return;
  const top = c ? Math.round(c.getBoundingClientRect().top) + "px" : "";
  if (top && top !== composerTopKept) { document.documentElement.style.setProperty("--composer-top", top);composerTopKept = top; }
}
// Reserve the footer's measured room in the reload shell. Its account
// controls can wrap to a second row once the hub supplies the metadata.
// The room is measured as the page goes (a reload, a quit), never while it
// draws: a layout read in every render of a streaming reply costs frames.
let composerFootHeld = false;
function composerGeometry() {
  composerTop();
  if (frontendNode || canonicalBoot) {
    // the hub's view: the footer's own height again
    if (composerFootHeld) { document.querySelector(".thread-view .composer-foot")?.style.removeProperty("min-height");composerFootHeld = false; }
    return;
  }
  const foot = document.querySelector(".thread-view .composer-foot"), thread = currentSession().sel;
  if (!foot || !thread) return;
  foot.style.removeProperty("min-height");
  try {
    const saved = JSON.parse(localStorage.getItem("backplane-composer-foot") || "null");
    if (saved?.thread === thread && Math.abs(saved.width - foot.getBoundingClientRect().width) < 1 && saved.height > 0 && saved.height < 300) {
      foot.style.minHeight = saved.height + "px";composerFootHeld = true;
    }
  } catch {}
}
function keepComposerGeometry() {
  const foot = document.querySelector(".thread-view .composer-foot"), thread = currentSession().sel;
  if (!foot || !thread || !frontendNode && !canonicalBoot) return;
  const r = foot.getBoundingClientRect();
  try { localStorage.setItem("backplane-composer-foot", JSON.stringify({ thread, width: r.width, height: r.height })); } catch {}
}
window.addEventListener("pagehide", keepComposerGeometry);
document.addEventListener("visibilitychange", () => { if (document.visibilityState === "hidden") keepComposerGeometry(); });
let viewerKept = "";
function keepViewer() {
  const sel = App.sel(ui);
  if (!sel || sel.startsWith("@")) return;
  const text = App.viewer_keep(ui);
  if (sel + text === viewerKept) return;
  viewerKept = sel + text;
  try { localStorage.setItem(VIEWER + sel, text); } catch {}
}
try {
  const selected = localStorage.getItem("backplane-selected");
  if (selected) ui = App.presentation_select(ui, selected);
} catch {}
// Reload keeps the Settings category, never any fields edited on that page.
let settingsRestorePage = null;
try {
  const saved = JSON.parse(localStorage.getItem("backplane-settings") || "null");
  if (saved && typeof saved.page === "string") {
    ui = App.act(ui, "flag", "settings").ui;
    ui = App.act(ui, "settings-page", saved.page).ui;
    settingsRestorePage = saved.page;
  }
} catch {}
window.addEventListener("pagehide", () => {
  const session = currentSession();
  try {
    if (session.open?.settings) localStorage.setItem("backplane-settings", JSON.stringify({ page: session.drafts?.["@settings.page"] || "" }));
    else localStorage.removeItem("backplane-settings");
  } catch {}
});
// A new view actor has no form edits. Keep the visible ordinary fields in
// memory across a dropped connection, then replay their existing actions.
// Sealed fields have no data-bfield and never enter this snapshot.
let settingsRecovery = null, settingsRecovering = null;
function keepSettingsRecovery() {
  const content = document.getElementById("settings-content");
  if (!content || settingsRecovery || settingsRecovering) return;
  settingsRecovery = { page: content.dataset.settingsPage || "", thread: currentSession().sel,
    fields: [...content.querySelectorAll("[data-bfield]")].map(el => [el.dataset.bfield, el.value]) };
}
async function recoverSettings(controller) {
  if (!settingsRecovery || settingsRecovering || !controller?.ready) return;
  const saved = settingsRecovery;
  if (!currentSession().open?.settings || saved.thread !== currentSession().sel) { settingsRecovery = null;return; }
  saved.attempt = controller;
  settingsRecovering = controller;
  try {
    if (saved.page && !await controller.perform("settings-page", saved.page)) return;
    for (const [name, value] of saved.fields) {
      if (thin !== controller || !controller.ready) return;
      if (!await controller.perform("bfield", name + "\u001f" + value)) return;
    }
    settingsRecovery = null;
  } finally { settingsRecovering = null;later(); }
}
document.addEventListener("input", e => {
  const el = e.target.closest?.("#settings-content [data-bfield]");
  if (!el || !settingsRecovery) return;
  const field = settingsRecovery.fields.find(([name]) => name === el.dataset.bfield);
  if (field) field[1] = el.value;
});
let socket = null;
const viewerRpc = new ViewerRpc(request => {
  if (!socket || socket.readyState !== 1) throw Error("Connect to the hub to read saved component metadata.");
  socket.send(fromList(App.wire_out(JSON.stringify(request))));
});
const thinContinuations = new ThinContinuations();
const bodyWindows = new BodyWindows({renderBody: node => App.make(node), request: (part, cursor, tail) => thin?.body(part, cursor, tail), changed: () => later()});
let frontend = null, frontendNode = null, wantFrontend = true, fallbackIntent = null;
let viewMode = "discovering", remoteReady = false, remoteEpoch = "", thinSession = null;
let pairingAccepted = false;
const transitionViews = new ViewTransition();
// The picture on screen while the next view is on its way: a retained
// picture of the thread picked (put back as it was last seen), else the one
// already showing, kept (holdPicture). What it was drawn for is
// transitionFor; shownFor is what the hub's view on show was drawn for.
let transitionNode = null, transitionFor = "", transitionPosition = null, shownFor = "";
// the target of the hub's view on show (shownFor's), and the views the hub
// refused this page (refusedView: never asked for again by themselves)
// (viewNote: what the status bar says of the last refusal, for a while)
let shownTarget = null, viewNote = "", viewNoteTimer = 0;
const refusedViews = new Set();
// since when a picture of another target has been kept (style.css's
// data-backplane-held: its own content takes no clicks, and past QUIET_MS
// it is put away, the frame left)
let heldAt = 0;
let focusedNode = null, focusedAtEnd = true, focusedSaveTimer = 0;
let focusedRequestedKey = null;
let thin = null, thinSocket = null, thinAttempt = 0, thinExpiry = 0, thinGreetingTimer = 0, thinDraftTimer = 0, thinFieldTimer = 0;
const terminalOutput = new TerminalOutputPane({ document, patch, current: () => {
  const link = thin?.client.links.get(location.origin), session = currentSession();
  return remoteReady && link ? { generation: link.greeting.generation, epoch: link.greeting.epoch, viewId: link.viewId, target: link.target, owner: session.drafts?.['@terminal.owner'], open: session.open?.term === true } : null;
} });
const focusedStore = new FocusedCache({ indexedDB, report: (state, reason) => trace("focused-cache", { state, reason }) });
function currentSession() { return thinSession || frontend?.session || fallbackIntent?.session || Wire.plain(App.presentation(ui)); }
function focusedScope() {
  return { origin: location.origin, cid, identity: cacheIdentity(document.cookie), bundle: import.meta.url,
    language: App.lang(ui), selected: currentSession().sel };
}
function transitionScope(target = presentationTarget(currentSession())) {
  return { origin: location.origin, identity: cacheIdentity(document.cookie), language: App.lang(ui),
    epoch: thin?.epoch || remoteEpoch, target, selected:currentSession().sel, settings:currentSession().open?.settings===true, route: currentSession().route || null };
}
const targetKey = target => target ? target.kind + ":" + target.id : "";
// Nothing is drawn in between two views: a thread picked, a reconnect, a
// failure keep the hub's last view on screen until the next one comes.
function holdPicture() {
  if (!frontendNode) return;
  transitionNode = frontendNode;transitionFor = shownFor;frontendNode = null;
}
function restoreTransition(target) {
  const shown = transitionViews.get(transitionScope(target));
  if (shown) { transitionFor = targetKey(target);heldAt = 0;transitionPosition = transitionViews.position(transitionScope(target)) || {atEnd:true}; }
  else if (transitionNode && transitionFor !== targetKey(target) && !heldAt) { heldAt = performance.now();setTimeout(later, QUIET_MS + 20); }
  transitionNode = shown ? App.presentation_node(presentationJson(themed(shown))) : transitionNode;
  return !!shown;
}
const focusedView = new FocusedView({
  cache: focusedStore, scope: focusedScope,
  hide: () => { focusedNode = null; later(); },
  view: record => {
    if (canonicalBoot || remoteReady) return;
    const shell = Wire.plain(App.presentation_view(App.presentation_wait(ui)));
    focusedNode = App.presentation_node(presentationJson(themed(mergeFocused(shell, record))));
    focusedAtEnd = record.atEnd;
    trace("focused-view");later();
  },
});
function checkpointFocused() {
  clearTimeout(focusedSaveTimer);focusedSaveTimer = 0;
  if (!canonicalBoot && !remoteReady) return;
  const main = document.querySelector("main.thread-view"), timeline = main?.querySelector("#timeline");
  const title = main?.querySelector(".thread-head .title");
  const app = document.querySelector(".app");
  if (!main || !timeline || !title || !app || main.dataset.thread !== focusedScope().selected) return;
  const record = captureFocused({ main, timeline, title, scope: focusedScope(), epoch: remoteReady ? remoteEpoch : App.origin(ui),
    shellAttrs: [{ name: "class", value: app.className }, { name: "style", value: app.getAttribute("style") || "" }] });
  if (record) void focusedView.save(record).catch(() => trace("focused-cache", { state: "unavailable", reason: "write error" }));
}
function saveFocusedSoon() {
  if (!focusedSaveTimer && (canonicalBoot || remoteReady)) focusedSaveTimer = setTimeout(checkpointFocused, 150);
}
function restoreFocused(force = false) {
  const key = scopeKey(focusedScope());
  if (!force && focusedRequestedKey === key) return Promise.resolve();
  focusedRequestedKey = key;
  return focusedView.restore().catch(() => trace("focused-cache", { state: "unavailable", reason: "read error" }));
}
async function refreshFocusedIdentity() {
  const before = cacheIdentity(document.cookie);
  const result = await refreshCacheIdentity({ fetch: window.fetch.bind(window), readCookie: () => document.cookie,
    writeCookie: value => { document.cookie = value; }, protocol: location.protocol });
  trace("cache-identity", { state: result.state });
  if (result.state === "denied") {
    stopThin(false, "auth-denied");viewMode = "read-only";focusedView.invalidate();
    showViewStatus("Pairing is required to reconnect.");later();return;
  }
  if (before !== cacheIdentity(document.cookie) || result.state === "denied" || result.state === "invalid") {
    transitionViews.clear();transitionNode = null;transitionFor = "";focusedView.invalidate();
    if (!canonicalBoot && !remoteReady) void restoreFocused();
    else saveFocusedSoon();
  }
}
let cacheReady = false;
let queued = false;
let scroll = false;
let focus = null;
// the field that had the keyboard when the link to the hub dropped (its
// id and selection): the view drawn again after the reconnect (a hub
// restart builds it afresh) gives it back, unless the person put the
// keyboard elsewhere meanwhile
let refocus = null, reclaim = false;
function refocusKeep() {
  reclaim = true;
  const el = document.activeElement;
  if (!el || el === document.body || !el.id || !el.closest?.("main, aside, .app")) return;
  refocus = { id: el.id, start: el.selectionStart ?? null, end: el.selectionEnd ?? null };
}
document.addEventListener("selectionchange", () => {
  if (!remoteReady && refocus && document.activeElement?.id === refocus.id) refocusKeep();
});
function refocusGive() {
  if (!refocus || !remoteReady) return;
  const want = refocus, el = document.getElementById(want.id);
  refocus = null;
  if (!el || document.activeElement && document.activeElement !== document.body) return;
  el.focus({ preventScroll: true });
  const n = typeof el.value === "string" ? el.value.length : 0;
  if (want.start !== null && typeof el.setSelectionRange === "function") try { el.setSelectionRange(Math.min(want.start, n), Math.min(want.end ?? want.start, n)); } catch {}
}
let jump = null;
// a search result opened: its entry, brought into view and flashed once its
// thread shows it, paging back through the history while it is not there
// yet (at most 40 pages)
let searchJump = null;
let earlierAsked = false;

// TanStack virtual-core 3.19.9's layout model is ported in timeline-plan.bend.
// The host owns DOM lifetimes; Bend owns rows, geometry, range and following.
const timelineLists = new WeakMap();
let activeTimeline = null, historyNode = null, historyStatus = null;
let held = false, away = "";
function timelineList(tl = document.getElementById("timeline")) { return tl && timelineLists.get(tl); }
function historyClear() { historyNode = null;historyDirection = "";historyPageState(null); }
function historyPageState(state) {
  historyStatus = state;
  let el = document.getElementById("history-status");
  if (!state) { el?.remove();return; }
  if (!el) { el = document.createElement("div");el.id = "history-status";document.body.appendChild(el); }
  el.setAttribute("role", "status");
  el.style.cssText = "position:fixed;z-index:12;max-width:calc(100vw - 32px);background:var(--panel);color:var(--text);border:1px solid var(--line);padding:4px 8px;font-size:12px";
  const box = document.getElementById("timeline")?.getBoundingClientRect();
  el.style.top = Math.max(0, box?.top || 0) + "px";el.style.right = Math.max(8, innerWidth - (box?.right || innerWidth) + 8) + "px";
  el.replaceChildren(document.createTextNode(state.phase === "error" ? "Messages couldn’t load. " : state.direction !== "older" ? "Loading newer messages…" : "Loading earlier messages…"));
  if (state.phase === "error") {
    earlierAsked = laterAsked = false;
    const button = document.createElement("button");button.textContent = "Retry";
    button.onclick = () => thin?.retryPage();
    el.appendChild(button);
  }
}
function historyRow(node) {
  if (!is(node, "El")) return "";
  const attrs = [...each(node.attrs)];
  const classes = attrs.find(a => is(a, "At") && a.name === "class")?.value.split(/\s+/) || [];
  if (classes.includes("live") || classes.includes("sending")) return "";
  return attrs.find(a => is(a, "At") && ["data-id", "data-timeline-entry"].includes(a.name))?.value || "";
}
function anchorOf(tl) {
  const position = timelineList(tl)?.position();
  return position?.anchor ? { id: position.anchor, off: tl.getBoundingClientRect().top - position.offset } : null;
}
function anchorElement(tl, id) {
  return id.startsWith("fold:") ? [...tl.querySelectorAll("button.fold")].find(el => el.getAttribute("data-value")?.slice(1) === id.slice(5))
    : tl.querySelector(`[data-id="${CSS.escape(id)}"]`);
}
function holdEnd(tl) { return timelineList(tl)?.atEnd() ?? true; }
function holdSay(now) {
  if (focusedNode && !frontendNode || now === away) return;
  const was = away;away = now;
  queueMicrotask(() => dispatch(now ? "hold-away" : "hold-end", now || was));
}
function timelineChanged(list) {
  if (list !== activeTimeline || focusedNode && !frontendNode) return;
  held = !list.atEnd() || !!thin && thin.atNewest?.() === false;
  holdSay(held ? App.sel(ui) : "");
  historyCheckSoon();
  bodyWindows.observe(list.el, remoteReady && !!frontendNode);
}
function holdMark() {
  return timelineList()?.rows.map(historyRow).filter(id => id && !id.startsWith("@") && !id.startsWith("control:")).at(-1) || "";
}
function holdBottom() {
  if (thin && thin.atNewest?.() === false) { thin.newest();return; }
  timelineList()?.scrollToEnd();
}
// a <details> that asks for something each time it opens (view.bend's
// board selector, data-on-open "board-list": its boards read again); not
// the person's own act, so offline it says nothing (dispatch's timer)
document.addEventListener("toggle", (e) => {
  const d = e.target;
  const action = d?.open && d.getAttribute?.("data-on-open");
  if (action) dispatch(action, d.getAttribute("data-value") ?? "", true);
}, true);

// a viewer's Layers list opens leftward from its button (style.css: the
// button sits at the bar's right end); where that would leave the window,
// the button wrapped to the start of a narrow bar, it opens rightward
document.addEventListener("toggle", (e) => {
  const d = e.target;
  if (!d?.open || !d.classList?.contains("layers")) return;
  const list = d.querySelector(".layer-list");
  if (!list) return;
  list.style.left = "";list.style.right = "";
  if (list.getBoundingClientRect().left < 0) { list.style.left = "0";list.style.right = "auto"; }
}, true);

const thinMailDraft = new ThinMailDraft({
  identity: () => cacheIdentity(document.cookie),
  context: context => { ui = App.mail_local_context(ui, presentationJson(context)); },
  key: () => App.mail_identity(ui),
  field: text => { ui = App.mail_local_field(ui, text); },
  snapshot: () => App.mail_snapshot(ui),
  // Extract only the draft text. The decoder's offline/outbox Ui is never adopted.
  restoreText: saved => App.mail_local_text(App.mail_restore(ui, saved)),
  read: key => localStorage.getItem(key), write: (key, text) => localStorage.setItem(key, text),
});
let mailStorageKey = "";
function mailCheckpoint() {
  if (viewMode !== "compatibility") { thinMailDraft.checkpoint();return; }
  const key = App.mail_identity(ui);
  if (!key) { mailStorageKey = ""; return; }
  try {
    if (key !== mailStorageKey) {
      const saved = localStorage.getItem("backplane-mail:" + key);
      if (saved) ui = App.mail_restore(ui, saved);
      mailStorageKey = key;
    }
    localStorage.setItem("backplane-mail:" + key, App.mail_snapshot(ui));
  } catch {} // Storage failure never sends or discards a captured message.
}
function mailAnchor() {
  const el = document.querySelector('.mail-messages');
  if (!el) return null;
  const box = el.getBoundingClientRect();
  const row = [...el.children].find(row => row.getBoundingClientRect().bottom > box.top);
  return { conv: el.dataset.conv, end: el.scrollHeight-el.scrollTop-el.clientHeight < 48,
    id: row?.dataset.mailId, offset: row ? row.getBoundingClientRect().top-box.top : 0 };
}
function mailRestore(anchor) {
  const el = document.querySelector('.mail-messages');
  if (!el) return;
  if (!anchor || anchor.conv !== el.dataset.conv || anchor.end) { el.scrollTop = el.scrollHeight; return; }
  const row = [...el.children].find(row => row.dataset.mailId === anchor.id);
  if (row) el.scrollTop += row.getBoundingClientRect().top-el.getBoundingClientRect().top-anchor.offset;
}
// the first place a word (as the search's snippet lit it) is written in
// an entry, as a range: whole words first, its case as written, then any
// case
function wordRange(el, word) {
  if (!word) return null;
  const esc = word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  for (const re of [new RegExp(`(^|[^\\p{L}\\p{N}_])(${esc})(?![\\p{L}\\p{N}_])`, "u"), new RegExp(`()(${esc})`, "iu")]) {
    const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let node = walk.nextNode(); node; node = walk.nextNode()) {
      const m = re.exec(node.data);
      if (!m) continue;
      const range = document.createRange(), at = m.index + m[1].length;
      range.setStart(node, at);range.setEnd(node, at + m[2].length);
      return range;
    }
  }
  return null;
}
// a search result's word, lit where it is in its entry (CSS.highlights:
// the page's nodes are not touched) while the entry shows; its entry
// flashed as the jump does. Renders put the entry's class back, so both
// are kept for a while after the jump (searchLit.until)
let searchLit = null;
function searchLight(tl) {
  if (!searchLit || !tl) return;
  if (performance.now() > searchLit.until) { searchLit = null;CSS.highlights?.delete("search-word");return; }
  const el = tl.querySelector(`[data-id="${CSS.escape(searchLit.id)}"]`) || anchorElement(tl, "fold:" + searchLit.id);
  if (!el) return;
  if (performance.now() < searchLit.flash && !el.classList.contains("jumped")) el.classList.add("jumped");
  const range = wordRange(el, searchLit.word);
  if (range && typeof Highlight === "function" && CSS.highlights) CSS.highlights.set("search-word", new Highlight(range));
}
// an entry brought into view and flashed (a Jump, a search result): the
// view holds there through the Bend planner.
// A search result's word is brought to the middle of the view when the
// entry is taller than the view, else the entry is.
function jumpTo(tl, el, word = "") {
  const id = el.getAttribute("data-id") || "fold:" + el.getAttribute("data-value")?.slice(1);
  timelineList(tl)?.scrollToKey(id, { align: "center" });
  el.classList.remove("jumped");
  void el.offsetWidth;
  el.classList.add("jumped");
}
function render() {
  mailCheckpoint();
  const mail = mailAnchor();
  queued = false;
  // Settings categories run through the normal hub action after its first
  // view; do not flash the category index while that action is on its way.
  if (remoteReady && settingsRecovery && settingsRecovery.attempt !== thin) { void recoverSettings(thin);return; }
  if (settingsRecovering === thin && thin) return;
  if (remoteReady && settingsRestorePage !== null) {
    const page = settingsRestorePage;settingsRestorePage = null;
    if (currentSession().open?.settings && page) { dispatch("settings-page", page);return; }
  }
  const tl = document.getElementById("timeline");
  const t0 = PERF ? pnow() : 0;
  let page = frontendNode ? App.presentation_overlay(frontendNode, presentationJson(modelFeedback.project(currentSession())))
    : transitionNode ? App.presentation_overlay(transitionNode, presentationJson(currentSession())) : focusedNode
    || (canonicalBoot ? App.view(ui) : App.presentation_node(presentationJson(themed(Wire.plain(App.presentation_view(App.presentation_wait(ui)))))));
  // Saved transition/startup pictures contain the server's unprojected partial
  // footer. Clean it before their first paint, without borrowing live readers
  // or request authority from the currently selected conversation.
  if (frontendNode || transitionNode || focusedNode) page = thinContinuations.project(page,
    node => frontendNode && node.tag === "bp-memo" && bodyWindows.parts.get(node.key)?.automatic === true ? node : expand(node), !!frontendNode);
  if (frontendNode) page = bodyWindows.project(page);
  const t1 = PERF ? pnow() : 0;
  patch(root, { $: T.El, tag: "div", key: "", attrs: { $: "Nil" }, kids: { $: "Con", head: page, tail: { $: "Nil" } } });
  bubMoveSync();
  terminalOutput.apply(true);
  if (PERF) PERF.render.push([t1 - t0, pnow() - t1]);
  if (frontendNode || canonicalBoot && App.synced(ui)) themeSeen(root.querySelector(".app"));
  if (canonicalBoot) {
    document.documentElement.dataset.backplaneReady = "true";
    document.documentElement.dataset.backplaneSeq = App.seq(ui);
  }
  document.documentElement.dataset.backplaneMode = viewMode;
  document.documentElement.dataset.backplaneRemoteReady = String(remoteReady);
  document.documentElement.dataset.backplaneCached = String(!!(focusedNode || transitionNode) && !frontendNode);
  // what style.css quiets: a picture kept for another target than the one
  // picked (true, then quiet past QUIET_MS), and the page before the hub's
  // first view (its frame only, nothing half-filled)
  const heldFor = !frontendNode && !!transitionNode && transitionFor !== targetKey(presentationTarget(currentSession()));
  document.documentElement.dataset.backplaneHeld = !heldFor ? "false" : heldAt && performance.now() - heldAt >= QUIET_MS ? "quiet" : "true";
  document.documentElement.dataset.backplaneWait = String(!frontendNode && !transitionNode && !focusedNode && !canonicalBoot);
  trace("dom", { seq: App.seq(ui) });
  if (PERF?.trace) requestAnimationFrame(() => trace("paint", { seq: App.seq(ui) }));
  mailRestore(mail);
  desktopBrowser.sync();
  memoSweep();
  const tl2 = document.getElementById("timeline");
  const list = timelineList(tl2);
  if (activeTimeline && activeTimeline !== list) activeTimeline.destroy();
  activeTimeline = list || null;
  list?.refresh();
  if (transitionPosition && list) { list.restore(transitionPosition);transitionPosition = null; }
  else if (scroll) list?.scrollToEnd();
  if (viewMode === "view-v1") document.querySelector("main")?.setAttribute("aria-busy", String(!remoteReady));
  bodyWindows.observe(tl2, remoteReady && !!frontendNode);
  if (historyStatus) historyPageState(historyStatus);
  scroll = false;
  // an entry the history went back to (Jump): brought into view, flashed
  if (jump && tl2) {
    list?.scrollToKey(jump, { align: "center" });
    const el = tl2.querySelector(`[data-id="${CSS.escape(jump)}"]`);
    if (el) jumpTo(tl2, el);
    jump = null;
  }
  if (searchJump && tl2 && searchJump.thread === (thin?.session?.sel ?? App.sel(ui))) {
    list?.scrollToKey(searchJump.id, { align: "center" });
    // (a tool call shut in its fold: the fold it starts)
    const el = tl2.querySelector(`[data-id="${CSS.escape(searchJump.id)}"]`) || anchorElement(tl2, "fold:" + searchJump.id);
    if (el) {
      jumpTo(tl2, el, searchJump.word);
      searchLit = { id: searchJump.id, word: searchJump.word, flash: performance.now() + 2400, until: performance.now() + 15000 };
      searchJump = null;
    } else if (thin?.window?.older && searchJump.pages < 40) {
      if (!searchJump.asked) { searchJump.asked = true; requestAnimationFrame(() => { if (searchJump) { searchJump.asked = false; if (thin.page()) searchJump.pages++; } }); }
    } else if (thin?.ready) searchJump = null;
  }
  searchLight(tl2);
  Solid.mount(document.getElementById("solid"));
  ViewerTools.mount(document.getElementById("solid-workbench"), Solid, { components: (thread, options) => viewerRpc.components(thread, options), asset: (thread, path, kind, revision, options) => viewerRpc.renderAsset(thread, path, kind, revision, options), plot: Plot2d });
  Plot2d.mount([...document.querySelectorAll("canvas.plot2d")]);
  panes();
  attachmentPreviews();
  previewTextFiles();
  composerGeometry();
  earlierAsked = false;
  laterAsked = false;
  if (tl2 && thin?.atNewest?.() === false && tl2.scrollHeight - tl2.scrollTop - tl2.clientHeight <= tl2.clientHeight * 2) requestAnimationFrame(() => laterCheck(document.getElementById("timeline")));
  // A page of entries the timeline does not draw (todo steps, a fold's
  // calls) moves nothing and, at the very top, no scroll comes to ask for
  // the next: still near the top after a render, it is asked for

  if (focus) {
    document.getElementById(focus)?.focus();
    focus = null;
  }
  refocusGive();
  if (reclaim && remoteReady) { reclaim = false;composerKeeper.claim(); } else composerKeeper.check();
  tourSync();
  settingsSync();
  saveFocusedSoon();
}

function later() {
  if (!queued) {
    queued = true;
    requestAnimationFrame(render);
  }
}

// A keystroke in the composer changes only the composer and the asks (an
// answer with the draft): those two are redrawn alone (app.bend's composer
// and asks, the same nodes the page draws), not the whole page, which on a
// long thread took 280 ms a key. Anything else draws the page as before.
let partly = false;
function laterPart() {
  if (queued || partly) return;
  partly = true;
  requestAnimationFrame(() => {
    partly = false;
    if (queued) return;
    const c = document.querySelector(".thread-view .composer");
    if (!c || !c.__v) { render(); return; }
    const t0 = PERF ? pnow() : 0;
    patch(c, App.composer(ui));
    attachmentPreviews();
    const a = document.getElementById("asks");
    if (a && a.__v) patch(a, App.asks(ui));
    if (PERF) PERF.part.push(pnow() - t0);
    trace("dom-part", { seq: App.seq(ui) });
    if (PERF?.trace) requestAnimationFrame(() => trace("paint-part", { seq: App.seq(ui) }));
  });
}

// Compatibility streaming uses the same Bend view and virtual row owner.
// Stable row keys retain mounted message DOM through each update.
let living = false;
function laterLive() {
  if (queued || living) return;
  living = true;
  requestAnimationFrame(() => {
    living = false;
    if (queued) return;
    const t0 = PERF ? pnow() : 0;
    render();
    if (PERF) PERF.live.push(pnow() - t0);
  });
}

function copy(text) {
  if (navigator.clipboard && window.isSecureContext) {
    navigator.clipboard.writeText(text);
    return;
  }
  const t = document.createElement("textarea");
  t.value = text;
  document.body.appendChild(t);
  t.select();
  document.execCommand("copy");
  t.remove();
}

function run(cmds) {
  for (const c of each(cmds)) {
    if (is(c, "Send")) {
      outboxKeep(c.text);
      if (socket && socket.readyState === 1) {
        if (PERF?.trace) { const request = JSON.parse(c.text); trace("send", { id: request.id, method: request.m, queuedBytes: socket.bufferedAmount }); }
        socket.send(fromList(App.wire_out(c.text)));
      }
    } else if (is(c, "Copy")) {
      copy(c.text);
    } else if (is(c, "Focus")) {
      focus = c.id; // after the next render, which may create it
      later();
    } else if (is(c, "Connect")) {
      // another machine's hub serves its own page; "" is this one
      if (c.url) location.href = App.visit(ui, c.url);
    } else if (is(c, "ScrollEnd")) {
      scroll = true;
    } else if (is(c, "Notify")) {
      notify(c);
    } else if (is(c, "Later")) {
      // a client action come due (the history playing); one "after"
      // "drawn" counts its time from when the viewer has drawn what the
      // view it came with shows (each step is seen for its beat)
      const due = () => setTimeout(() => dispatch(c.action, c.value, true), Number(c.ms));
      if (c.after === "drawn") Plot2d.whenDrawn(due); else due();
    } else if (is(c, "Jump")) {
      jump = c.id; // after the next render, which opens its folds
      later();
    } else if (is(c, "Keep")) {
      applyComposerDraft(document, c.thread, c.text);
      // written at once: a crash or a closed tab loses nothing typed
      try {
        if (c.thread === "@lang") localStorage.setItem("backplane-lang", c.text);
        else if (c.text) localStorage.setItem(DRAFT + c.thread, c.text);
        else localStorage.removeItem(DRAFT + c.thread);
      } catch {}
    }
  }
}

// Desk
// ----
// This page is a desk (core/desk.bend): app.bend's desk decides when to
// tell the hub which thread it shows and whether the person has it in
// front of them; the hub then holds the phones' alerts for that thread and
// sends this page a desktop notification for others when it was the desk
// used last.

function deskCheck() {
  if (!canonicalBoot) return;
  const focused = document.visibilityState === "visible" && document.hasFocus();
  const r = App.desk(ui, focused, now());
  ui = r.ui;
  run(r.cmds);
}

for (const ev of ["focus", "blur"]) window.addEventListener(ev, deskCheck);
document.addEventListener("visibilitychange", deskCheck);
for (const ev of ["pointerdown", "keydown", "wheel"]) document.addEventListener(ev, deskCheck, { capture: true, passive: true });

// the browser asks the person once, on a first click or key (the only time
// browsers let a page ask)
function askNotify() {
  if ("Notification" in window && Notification.permission === "default") Notification.requestPermission().catch(() => {});
}
for (const ev of ["pointerdown", "keydown"]) document.addEventListener(ev, askNotify, { once: true, capture: true });

function notify(c) {
  if (!("Notification" in window) || Notification.permission !== "granted") return;
  try {
    const n = new Notification(c.title, { body: c.body, tag: c.key || c.thread, icon: "./apple-touch-icon.png" });
    n.onclick = () => {
      window.focus();
      if (c.thread) dispatch("select", c.thread);
      n.close();
    };
  } catch {}
}

// run an action; answers whether it did anything (a key it did nothing
// with stays the browser's)
// A thread's ordering card (client.bend's Fab.sync) is asked for after an
// action; opening a thread in the thin view is no action, so its Ready to
// order bar and Buy it never came. A thread shown asks with "fab-look" (an
// action that changes nothing): at once when it is new here, then at most
// every 20 s while views of it arrive, so a turn's new fab files show.
// Fab.sync itself asks the hub only when the thread's state moved.
const fabLooked = new Map();
function fabLook(target) {
  if (target?.kind !== "thread" || !target.id || !thin?.ready) return;
  const now = Date.now(), last = fabLooked.get(target.id) || 0;
  if (now - last < 20000) return;
  fabLooked.set(target.id, now);
  setTimeout(() => { if (thin?.ready) thin.request("fab-look", ""); }, 0);
}

// The secret box's Save: valueOf read the masked field once (and emptied
// it); the value goes in one secret.set request straight to the hub, never
// through a view action, the outbox, a draft or the client's state. Bend
// builds and checks it (app.bend's secret_req); only the name comes back.
function secretSave(packed) {
  let r;
  try { r = JSON.parse(App.secret_req(packed)); } catch { return true; }
  packed = "";
  if (r.error) return dispatch("secret-err", r.error);
  if (!socket || socket.readyState !== 1) return dispatch("secret-err", "Not connected to the hub");
  socket.send(fromList(App.wire_out(r.rpc)));
  r = null;
  return dispatch("secret-sent", "");
}

// A sealed form's Save (Enter in its fields too; client.bend's Sealed),
// as the secret box's: the form's fields have no binding, so what is typed
// in them stays in this page. Save reads them once (data-sealed), Bend
// builds and checks the form's request (app.bend's sealed_req), and it
// goes straight to the hub, never through a view action, the outbox, a
// draft or the client's state. The hub's answer to it (by its own id, past
// the ordinary ones) empties the fields and says "sealed-sent", or
// "sealed-err" with why, in the form.
const sealedWait = new Map();
let sealedNext = 2 ** 46;
function sealedFields(form) {
  return [...document.querySelectorAll("[data-sealed]")].filter(f => f.getAttribute("data-sealed") === form);
}
function sealedSave(form) {
  const fields = sealedFields(form);
  let r;
  try { r = JSON.parse(App.sealed_req([form, ...fields.map(f => f.value ?? "")].join("\u001f"))); } catch { return true; }
  if (r.error) return dispatch("sealed-err", form + "\u001f" + r.error);
  if (!socket || socket.readyState !== 1) return dispatch("sealed-err", form + "\u001f" + "Not connected to the hub");
  if ([...sealedWait.values()].some(w => w.form === form)) return true;
  let rpc;
  try { rpc = JSON.parse(r.rpc); } catch { return true; }
  r = null;
  const id = sealedNext++;
  rpc.id = id;
  sealedWait.set(id, { form, timer: setTimeout(() => sealedDone(id, false, "The hub did not answer"), 15000) });
  socket.send(fromList(App.wire_out(JSON.stringify(rpc))));
  rpc = null;
  return true;
}
function sealedDone(id, ok, why) {
  const w = sealedWait.get(id);
  if (!w) return;
  sealedWait.delete(id);clearTimeout(w.timer);
  if (ok) for (const f of sealedFields(w.form)) f.value = "";
  dispatch(ok ? "sealed-sent" : "sealed-err", ok ? w.form : w.form + "\u001f" + why);
}
// the hub's answer to a sealed Save (true: it was one)
function sealedReply(envelope) {
  if (!sealedWait.has(envelope.id)) return false;
  sealedDone(envelope.id, envelope.ok === true, typeof envelope.error === "string" && envelope.error ? envelope.error : "The hub refused it");
  return true;
}

// a phone-sized or touch page: the composer never takes the keyboard by
// itself there (its keyboard would cover the thread)
function composerPhone() { return matchMedia("(max-width: 760px), (hover: none) and (pointer: coarse)").matches; }

function dispatch(action, value, timer) {
  if (action === "secret-save") return secretSave(value);
  if (action === "sealed") return sealedSave(value);
  if (["send", "send-alt"].includes(action)) historyClear();
  if (action === "side-toggle" || action === "rail-toggle") return foldToggle(action === "side-toggle" ? "side" : "rail");
  if (action === "select") drawerShut();
  if (["term-toggle", "ws-term-hide", "ws-term-here"].includes(action) || action === "ws-go" && value === "term") terminalOutput.retire();
  else if (["select", "bot", "bot-tab", "uber", "uber-bot", "remote"].includes(action)) terminalOutput.clear();
  if (["select", "ref-go", "ref-back", "ref-fwd", "bot", "bot-tab", "uber", "uber-bot", "remote"].includes(action)) {
    checkpointFocused();
    const timeline = document.getElementById("timeline");
    if (remoteReady && timeline) {
      transitionViews.savePosition(transitionScope(), timelineList(timeline).position());
    }
  }
  if (action === "thin-search-open") {
    if (viewMode === "view-v1" && thin?.ready) showThinSearch({ query: "", rows: [], cursor: "", hasMore: false });
    else showViewStatus("Search requires the current conversation connection.");
    return true;
  }
  if (viewMode !== "compatibility") {
    if ((action === "send" || action === "send-alt") && (!socket || socket.readyState !== 1)) {
      offlineSend = true;
      showViewStatus("Not sent. Reconnect to send; your draft is kept.");
      return true;
    }
    if (action === "select" || action === "draft") {
      if (thin) return thin.request(action, value);
      thinSession = choosePresentation(currentSession(), action, value);
      localPresentation(action, value, thinSession);
      return true;
    }
    // (a press while a thread's view is on its way waits for it: thin-host.js)
    if (thin?.ready || thin?.opening) {
      if (action === "earlier" || action === "view-page") return thin.page();
      if (action === "thin-search") return thin.search(value);
      if (action === "thin-message-more") {
        try { const input = JSON.parse(value);if (bodyWindows.parts.get(input.entryId)?.bodyPage === 1) return bodyWindows.open(input.entryId);return thin.message(input.entryId, input.cursor); } catch { return true; }
      }
      if (action === "thin-cancel") return thin.cancel(Number(value));
      if (action === "mail-send" && thinMailDraft.needsSubmit()) {
        const field = document.getElementById("mail-reply");
        if (!field || !thin.request("bfield", "mrreply\u001f" + field.value)) return true;
      }
      // the hub draws the "N new" pill: it is told what the reader had
      if (action === "hold-away" || action === "hold-end") { thin.away?.(value, action === "hold-away" ? holdMark() : "");return true; }
      if (action === "hold-bottom") { holdBottom();return true; }
      const layer = document.querySelector('[data-local-models="1"]');
      const settled = layer ? modelFeedback.choice(action, value, layer.style.display !== "none") : undefined;
      const thread = currentSession().sel;
      const attachments = ["detach", "send", "send-alt"].includes(action) ? draftAttachmentPaths(currentSession(), thread) : [];
      return thin.request(action, value, accepted => {
        settled?.(accepted);
        if (accepted && attachments.length) {
          const kept = draftAttachmentPaths(currentSession(), thread);
          void draftAttachments.forget(thread, attachments.filter(path => !kept.includes(path))).catch(() => {});
        }
      });
    }
    if (!timer) showViewStatus("Saved conversation is read-only until connected.");
    return true;
  }
  if (!canonicalBoot) {
    if (frontend?.request(action, value)) return true;
    if (action === "draft" || action === "select") {
      const session = fallbackIntent?.session || Wire.plain(App.presentation(ui));
      const chosen = choosePresentation(session, action, value);
      const choices = fallbackIntent?.choices || [];
      const revision = choices.reduce((n, c) => Math.max(n, c.revision), 0) + 1;
      fallbackIntent = { session: chosen, operations: fallbackIntent?.operations || [], choices: [...choices, { action, value, revision }] };
      localPresentation(action, value, chosen);
      return true;
    }
    if (!timer) { ui = App.presentation_blocked(ui); frontendNode = null; later(); }
    return true;
  }
  if (action === "desktop-folder" && window.backplaneDesktop) {
    window.backplaneDesktop.chooseFolder().then(folder => { if (folder) dispatch("proj-cd", folder); });
    return true;
  }
  const t0 = PERF ? pnow() : 0;
  trace("input", { action });
  const r = App.act(ui, action, value);
  if (PERF) PERF.act.push([pnow() - t0, action]);
  ui = r.ui;
  if (action === "select") try { localStorage.setItem("backplane-selected", value); } catch {}
  mailCheckpoint();
  if (action === "lang" || action === "lang-os") document.documentElement.lang = App.lang(ui);
  if (action === "subs-shut" || action === "subs-open") {
    try {
      const sep = value.indexOf("\t");
      const thread = action === "subs-open" ? value : value.slice(0, sep);
      localStorage.setItem(SUBS_CHOICE + thread, action === "subs-open" ? "1" : "0" + value.slice(sep + 1));
    } catch {}
  }
  const did = r.cmds && is(r.cmds, "Con");
  run(r.cmds);
  // a timer is not the person: only their input keeps this desk in use
  if (!timer) deskCheck();
  if (action === "draft") laterPart();
  else if (action === "bfield" && value.startsWith("mrreply\u001f")) trace("dom-input-native", { action });
  else later();
  return did;
}

// a right click on the plot starts a review note there (client.bend's Rc)
Plot2d.onPlace((place) => dispatch("rv-note", place));
// a tap on a sheet symbol opens that sheet instance (client.bend's Sch)
Plot2d.onSheet((value) => dispatch("sch-go", value));
// a tap that probes a part or a net (probe.js) tells the hub, which keeps it
// (client.bend's Probe); the 3D view finds parts on the board's plot
Plot2d.onProbe((value) => { Solid.setProbe(value); dispatch("probe", value); });
Solid.onProbe((value) => { Plot2d.setProbe(value); dispatch("probe", value); });
Solid.useBoard({ parts: (key) => Plot2d.parts(key), thick: (key) => Plot2d.thick(key), need: (key) => Plot2d.need(key) });
Plot2d.onPlotted(() => Solid.relight());
// the probe's chip: its name brings it into view
document.addEventListener("click", (e) => {
  if (!e.target.closest?.('[data-view="probe"]')) return;
  Plot2d.showProbe();
  Solid.showProbe();
});
// (Esc clears the probe: workspace-keys.js, before it shuts the viewer)

// how many terminal cells fit: the thread pane's width, 40% of the height
function termSize() {
  let probe = document.getElementById("term-probe");
  if (!probe) {
    probe = document.createElement("span");
    probe.id = "term-probe";
    probe.className = "term-probe";
    probe.textContent = "0".repeat(20);
    document.body.appendChild(probe);
  }
  const cw = probe.getBoundingClientRect().width / 20 || 7.2;
  const pane = document.getElementById("thread");
  const w = (pane ? pane.clientWidth : window.innerWidth) - 24;
  const h = window.innerHeight * 0.4 - 40;
  return `${Math.max(20, Math.floor(w / cw))}x${Math.max(5, Math.floor(h / 17))}`;
}

function valueOf(el) {
  const sp = el.getAttribute("data-space");
  if (sp !== null) {
    const f = document.getElementById(el.getAttribute("data-field") || "") || el;
    const t = f.value ?? "";
    f.value = "";
    return sp + "\u001f" + t;
  }
  const pre = el.getAttribute("data-pre");
  if (pre !== null) return pre + (el.value ?? "");
  const v = el.getAttribute("data-value");
  if (v === null) return el.value ?? "";
  if (v === "@term-size") return termSize();
  if (v.startsWith("#")) return document.getElementById(v.slice(1))?.value ?? "";
  return v;
}

// what a field sends as it is typed in; a form field (data-bfield) sends
// "<field>\u001f<text>"
function inputValue(el) {
  const f = el.getAttribute("data-bfield");
  return f === null ? el.value : f + "\u001f" + el.value;
}

// Files: each goes whole as the body of one POST /attach to the hub (the
// pairing cookie carries the token); "attach-up" and "attach-end" tell
// app.bend it started and what the hub answered, and it decides the rest
const draftAttachments = new DraftAttachments();
function draftAttachmentPaths(session, thread) {
  return (session.drafts?.["@att:" + thread] || "").split("\n").filter(Boolean);
}
function restoreDraftAttachments(session) {
  const scope = uploadScope(), thread = session.sel;
  if (!scope.controller?.ready || scope.thread !== thread || session.drafts?.["@att.up:" + thread]) return;
  void draftAttachments.sync(thread, draftAttachmentPaths(session, thread),
    (file, previous) => upload("attach", file, scope, previous),
    () => thin === scope.controller && thin.ready && currentSession().sel === thread).catch(() => {});
}
function uploadScope() {
  const controller = viewMode === "compatibility" ? null : thin;
  return { mode: viewMode, controller,
    thread: viewMode === "compatibility" ? App.sel(ui) : currentSession().sel,
    target: controller?.target, epoch: controller?.epoch,
    viewId: controller?.client.links.get(controller.hub)?.viewId };
}
async function uploadFiles(action, files, scope = uploadScope()) {
  for (const file of files) await upload(action, file, scope);
}
async function upload(action, file, scope = uploadScope(), previous = "") {
  const key = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
  const name = file.name || "pasted." + ((file.type || "").split("/")[1] || "bin");
  const { controller: uploadController, thread } = scope;
  const current = () => viewMode === scope.mode && (uploadController
    ? thin === uploadController && uploadController.ready && currentSession().sel === thread && uploadController.epoch === scope.epoch
      && uploadController.target?.kind === scope.target?.kind && uploadController.target?.id === scope.target?.id
      && uploadController.client.links.get(uploadController.hub)?.viewId === scope.viewId
    : App.sel(ui) === thread);
  if (!thread || !current()) return;
  if (viewMode !== "compatibility") {
    if (!remoteReady || !uploadController || !await uploadController.perform("attach-up", JSON.stringify({ name, size: file.size }))) return;
  } else dispatch("attach-up", JSON.stringify({ name, size: file.size }));
  // Admission is owned by the actual actor response. The renderer's separate
  // local App never received attach-up on this path and cannot approve it.
  if (!current() || (uploadController ? uploadController.session.drafts["@att.up:" + thread] : App.uploading(ui, thread)) !== name) return;
  const q = new URLSearchParams({ thread, key, name });
  let answer = "";
  try {
    const r = await fetch(`/attach?${q}`, { method: "POST", body: file, credentials: "same-origin" });
    answer = await r.text();
    JSON.parse(answer);
  } catch {
    answer = JSON.stringify({ text: "the upload did not reach the hub", thread, upload: true });
  }
  if (!current()) return;
  let result;try { result = JSON.parse(answer); } catch { return; }
  if (result.attached && result.thread === thread) {
    try { await draftAttachments.remember(thread, result.attached, file, previous); } catch {
      showViewStatus("This attachment could not be kept for reload. Send it before leaving this page.");
    }
  }
  if (!current()) return;
  if (viewMode === "compatibility") dispatch("attach-end", answer);
  else if (thin === uploadController) {
    // The HTTP body and completion stay bound to the original chosen thread,
    // even if the person selects another thread while the upload travels.
    let completed;try { completed = JSON.parse(answer); } catch { return; }
    if (completed.thread !== thread) return;
    // Finish through the same live actor, not a renderer snapshot that the
    // server correctly refuses as attachment authority. The HTTP result's
    // original thread and exact view fence keep any later selection/draft out
    // of this completion. A retired view never retargets it into a new actor.
    await uploadController.perform("attach-end", answer);
  }
  return result.attached && result.thread === thread ? result.attached : undefined;
}

document.addEventListener("change", (e) => {
  const el = e.target.closest?.("[data-attach]");
  if (!el || !el.files) return;
  const files = [...el.files];
  el.value = "";
  void uploadFiles(el.getAttribute("data-attach"), files);
});

// The draw board (draw.js): the composer's [data-draw] button opens it, its
// words on the button (data-draw-*), and the picture it saves is attached
// to the next message the way a picked file is
document.addEventListener("click", (e) => {
  const el = e.target.closest?.("[data-draw]");
  if (!el) return;
  e.preventDefault();
  const d = el.dataset;
  const scope = uploadScope();
  openDraw({ title: d.drawTitle, hint: d.drawHint, save: d.drawSave, cancel: d.drawCancel, length: d.drawLength },
    (file) => upload(el.getAttribute("data-draw"), file, scope));
});

// Drops: files dragged onto a [data-drop] area go to its action, the way
// the attach button's do; its data-drop-hover action hears "1" while they
// hover and "" once they leave or land (app.bend decides what that shows)
let dropOver = null;
function dragsFiles(e) {
  return [...(e.dataTransfer?.types || [])].includes("Files");
}
function dropHover(el) {
  if (el === dropOver) return;
  if (dropOver) dispatch(dropOver.getAttribute("data-drop-hover"), "");
  dropOver = el;
  if (el) dispatch(el.getAttribute("data-drop-hover"), "1");
}

document.addEventListener("dragover", (e) => {
  if (!dragsFiles(e)) return;
  e.preventDefault();
  const el = e.target.closest?.("[data-drop]") || null;
  e.dataTransfer.dropEffect = el ? "copy" : "none";
  dropHover(el);
});

document.addEventListener("dragleave", (e) => {
  if (dropOver && !(e.relatedTarget && dropOver.isConnected && dropOver.contains(e.relatedTarget))) dropHover(null);
});

document.addEventListener("drop", (e) => {
  if (!dragsFiles(e)) return;
  e.preventDefault();
  const el = e.target.closest?.("[data-drop]");
  dropHover(null);
  const files = [...(e.dataTransfer.files || [])];
  if (!el || files.length === 0) return;
  void uploadFiles(el.getAttribute("data-drop"), files);
});

// Row drags: a [data-drag] element dropped on a [data-drag-to] one with the
// same data-drag-act sends that action "dragged|target" (app.bend decides
// what moves where)
const ROW = "application/x-backplane-row";
document.addEventListener("dragstart", (e) => {
  const el = e.target.closest?.("[data-drag]");
  if (!el) return;
  e.dataTransfer.setData(ROW, el.getAttribute("data-drag-act") + "\n" + el.getAttribute("data-drag"));
  e.dataTransfer.effectAllowed = "move";
});

function rowTarget(e) {
  if (![...(e.dataTransfer?.types || [])].includes(ROW)) return null;
  return e.target.closest?.("[data-drag-to]") || null;
}

document.addEventListener("dragover", (e) => {
  if (!rowTarget(e)) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = "move";
});

document.addEventListener("drop", (e) => {
  const el = rowTarget(e);
  if (!el) return;
  e.preventDefault();
  const [act, v] = e.dataTransfer.getData(ROW).split("\n");
  if (act === el.getAttribute("data-drag-act")) dispatch(act, v + "|" + el.getAttribute("data-drag-to"));
});

// a key field that names its terminal (a terminal in the chat) puts the
// name first: "<key>\t..."
const forKey = (el) => (el.getAttribute("data-for") ? el.getAttribute("data-for") + "\t" : "");

// A later hub frame restores the image's class attribute. Keep the load
// result visible after that patch too; a failed cached image fires once.
function attachmentPreviews() {
  for (const img of document.querySelectorAll('img.attachment-preview')) {
    if (img.complete) img.classList.toggle('failed', img.naturalWidth === 0);
  }
}

document.addEventListener("load", e => {
  if (e.target.matches?.("img.attachment-preview")) e.target.classList.remove("failed");
}, true);
document.addEventListener("error", e => {
  if (e.target.matches?.("img.attachment-preview")) {
    e.target.classList.add("failed");e.target.parentElement.setAttribute("title", "Image preview unavailable; remove the attachment and try again.");
  }
}, true);
document.addEventListener("paste", (e) => {
  const cd = e.clipboardData;
  if (!cd) return;
  const keys = e.target.closest?.("[data-keys]");
  if (keys) {
    e.preventDefault();
    dispatch(keys.getAttribute("data-text"), forKey(keys) + cd.getData("text/plain"));
    return;
  }
  const el = e.target.closest?.("[data-paste]");
  const files = [...(cd.files || [])];
  if (files.length === 0) {
    for (const item of cd.items || []) {
      if (item.kind === "file" && item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) files.push(file);
      }
    }
  }
  if (!el || files.length === 0) return;
  e.preventDefault();
  void uploadFiles(el.getAttribute("data-paste"), files);
});

// text typed into a key field without a key event (a phone keyboard)
document.addEventListener("input", (e) => {
  const el = e.target.closest?.("[data-text]");
  if (!el || !el.value) return;
  const v = el.value;
  el.value = "";
  dispatch(el.getAttribute("data-text"), forKey(el) + v);
});

for (const ev of EVENTS) {
  document.addEventListener(ev, (e) => {
    const el = e.target.closest?.(`[data-on-${ev}]`);
    if (!el) return;
    const action = el.getAttribute(`data-on-${ev}`);
    // a row's menu opens where the pointer was (view.bend's View.rmenu
    // reads these)
    // a file link (dom.bend's Dom.file) goes to the hub, not to its href
    if (ev === "click" && el.tagName === "A") e.preventDefault();
    if (ev === "contextmenu") {
      e.preventDefault();
      document.documentElement.style.setProperty("--mx", Math.min(e.clientX, innerWidth - 216) + "px");
      document.documentElement.style.setProperty("--my", e.clientY + "px");
    }
    dispatch(action, ev === "input" ? inputValue(el) : valueOf(el));
    // A mouse send or stop leaves the next keystroke in the message box.
    if (ev === "click" && el.closest(".composer") && ["send", "send-alt", "interrupt"].includes(action)) document.getElementById("composer")?.focus({ preventScroll: true });
  });
}

// A page the person drives (the Browser tab, client.bend's Teach): the
// img names the actions (data-pt, data-wheel, data-keyto, data-type); this
// only turns DOM events into their values. A click is a place in
// thousandths of the frame, the wheel a notch's pixels, a key its name
// (Playwright's), typed characters are sent together.
let frameText = "";
let frameTimer = 0;
function frameFlush(el) {
  clearTimeout(frameTimer);
  const t = frameText;
  frameText = "";
  const a = el && el.getAttribute("data-type");
  if (t && a) dispatch(a, t);
}
document.addEventListener("click", (e) => {
  const el = e.target.closest?.("[data-pt]");
  if (!el || e.button !== 0) return;
  e.preventDefault();
  el.focus();
  const r = el.getBoundingClientRect();
  const th = (v, size) => Math.max(0, Math.min(1000, Math.round((v / Math.max(size, 1)) * 1000)));
  frameFlush(el);
  dispatch(el.getAttribute("data-pt"), th(e.clientX - r.left, r.width) + " " + th(e.clientY - r.top, r.height));
}, true);
document.addEventListener("wheel", (e) => {
  const el = e.target.closest?.("[data-wheel]");
  if (!el) return;
  e.preventDefault();
  frameFlush(el);
  dispatch(el.getAttribute("data-wheel"), String(Math.round(Math.max(-600, Math.min(600, e.deltaY)))));
}, { passive: false });
const FRAME_KEYS = { Enter: 1, Backspace: 1, Tab: 1, Escape: 1, Delete: 1, ArrowLeft: 1, ArrowRight: 1, ArrowUp: 1, ArrowDown: 1, Home: 1, End: 1, PageUp: 1, PageDown: 1 };
document.addEventListener("keydown", (e) => {
  const el = e.target.closest?.("[data-keyto]");
  if (!el || (e.isComposing || e.keyCode === 229) || e.metaKey) return;
  e.preventDefault();
  e.stopImmediatePropagation();
  if (e.key.length === 1 && !e.ctrlKey && !e.altKey) {
    frameText += e.key;
    clearTimeout(frameTimer);
    frameTimer = setTimeout(() => frameFlush(el), 40);
    return;
  }
  frameFlush(el);
  if (e.key.length === 1 && e.ctrlKey) dispatch(el.getAttribute("data-keyto"), "Control+" + e.key.toLowerCase());
  else if (FRAME_KEYS[e.key]) dispatch(el.getAttribute("data-keyto"), (e.shiftKey && e.key === "Tab" ? "Shift+" : "") + e.key);
}, true);

// keys: the page's own shortcuts (app.bend lists them), then a key field
// (the terminal) gets every key as "<key>\t<mods>"
document.addEventListener("keydown", (e) => {
  if (e.defaultPrevented || e.isComposing || e.keyCode === 229 || e.metaKey) return;
  const field = e.target.closest?.("[data-keys]");
  if (field) {
    const mods = (e.shiftKey ? 1 : 0) | (e.ctrlKey ? 4 : 0) | (e.altKey ? 8 : 0);
    if (dispatch(field.getAttribute("data-keys"), forKey(field) + e.key + "\t" + mods)) e.preventDefault();
  }
});

document.addEventListener("keydown", (e) => {
  if (e.isComposing || e.keyCode === 229) return;
  // up and down move the highlight in a menu the field offers (the
  // composer's completions)
  if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
    const arrows = e.target.getAttribute?.("data-arrows");
    if (arrows) {
      e.preventDefault();
      dispatch(arrows, e.key === "ArrowDown" ? "down" : "up");
    }
    return;
  }
  // Esc shuts the menu the field offers
  if (e.key === "Escape" && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
    const esc = e.target.getAttribute?.("data-esc");
    if (esc) {
      e.preventDefault();
      e.stopPropagation();
      dispatch(esc, "");
      return;
    }
  }
  if (e.key === "Tab" && !e.shiftKey) {
    const tab = e.target.getAttribute?.("data-tab");
    if (tab) {
      e.preventDefault();
      dispatch(tab, valueOf(e.target));
    }
    return;
  }
  const el = e.target.closest?.("[data-enter]");
  if (!el || e.key !== "Enter" || e.shiftKey || (e.isComposing || e.keyCode === 229)) return;
  // ctrl/cmd+Enter: the element's other action (send with the other follow-up mode);
  // an empty data-enter leaves Enter to the field (a new line: an intent being edited)
  const alt = (e.ctrlKey || e.metaKey) && el.getAttribute("data-enter-alt");
  const action = alt || el.getAttribute("data-enter");
  if (!action) return;
  e.preventDefault();
  dispatch(action, valueOf(el));
});

// The first-run tour (view.bend's View.tour) is modal while its dialog is on
// the page: the page behind is inert, focus stays in the dialog (Tab goes
// round its buttons), the keys that mean something go through app.bend's
// tour_key (a repeat does nothing), every other key, ctrl/meta combination,
// paste and drop is stopped, and a focused button does its own action. Every
// key pressed while it is up (by KeyboardEvent.code, modified ones too) has
// its repeats dropped after it closes, until the key is let go or pressed
// afresh, so holding Enter through Done never reaches the composer behind.
let tourFocus = null; // where the keyboard was when the dialog came up
let tourOn = false;
const tourEaten = new Set();
let tourDrop = "";
function tourSync() {
  const tour = document.querySelector(".tour");
  const app = document.querySelector(".app");
  if (tour && !tourOn) {
    tourOn = true;
    tourFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  }
  if (app) {
    for (const el of app.children) {
      if (tour && !el.classList.contains("tour") && !el.classList.contains("tour-back")) el.setAttribute("inert", "");
      else el.removeAttribute("inert");
    }
  }
  if (tour) {
    if (!tour.contains(document.activeElement)) (tour.querySelector("button.primary") || tour.querySelector("button"))?.focus();
  } else if (tourOn) {
    tourOn = false;
    // back where it was, unless that went away (Settings shut): the composer
    const back = tourFocus && tourFocus !== document.body && tourFocus.isConnected ? tourFocus : document.getElementById("composer");
    back?.focus?.();
    tourFocus = null;
  }
}
document.addEventListener("keydown", (e) => {
  const tour = document.querySelector(".tour");
  if (!tour) {
    // a key pressed while it was up, still held: its repeats are dropped
    // (its keypress and input with them) until it is let go or pressed afresh
    if (tourEaten.has(e.code)) {
      if (e.repeat) { tourDrop = e.code; e.preventDefault(); e.stopImmediatePropagation(); return; }
      tourEaten.delete(e.code);
    }
    tourDrop = "";
    return;
  }
  if (e.isComposing || e.keyCode === 229) return;
  tourEaten.add(e.code);
  e.stopImmediatePropagation();
  if (e.ctrlKey || e.metaKey || e.altKey) { e.preventDefault(); return; }
  if (e.key === "Tab") {
    e.preventDefault();
    const bs = [...tour.querySelectorAll("button")];
    const at = bs.indexOf(document.activeElement);
    bs[(at + (e.shiftKey ? bs.length - 1 : 1)) % bs.length]?.focus();
    return;
  }
  const act = App.tour_key(e.key);
  const onButton = document.activeElement instanceof HTMLButtonElement && tour.contains(document.activeElement);
  // Enter or Space on a focused button is that button's (the click does it)
  if (onButton && (e.key === "Enter" || e.key === " ")) {
    if (e.repeat) e.preventDefault();
    return;
  }
  e.preventDefault();
  if (!act || e.repeat) return;
  dispatch(act, "");
}, true);
document.addEventListener("keyup", (e) => {
  tourEaten.delete(e.code);
  if (tourDrop === e.code) tourDrop = "";
}, true);
for (const ev of ["paste", "drop", "dragover", "beforeinput", "cut", "keypress"]) {
  document.addEventListener(ev, (e) => {
    if (document.querySelector(".tour") || (tourDrop && (ev === "beforeinput" || ev === "keypress"))) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
}

// Lightbox
// --------
// A click on an image in a thread ([data-lightbox]) shows it over the page.
// app.bend's lb_* hold every decision (zoom, pan, easing, what closes it);
// this feeds them the pointer and sets the style they answer, drawing
// frames only while lb_busy says an ease is running.

let lb = null; // { el, img, v: the Bend Lb, drawing, pointers }
const lbNow = () => BigInt(Math.floor(performance.now())) + 1n;
const lbN = (x) => BigInt(Math.max(0, Math.round(x)));
const lbBox = () => App.lb_box(lbN(lb.img.naturalWidth), lbN(lb.img.naturalHeight), lbN(window.innerWidth), lbN(window.innerHeight));

function lbDraw() {
  if (!lb) return;
  lb.drawing = false;
  if (!lb.img.naturalWidth) return;
  const now = lbNow();
  lb.v = App.lb_stamp(lb.v, now);
  lb.img.style.cssText = App.lb_css(lb.v, lbBox(), now);
  if (App.lb_busy(lb.v, now)) lbLater();
}

function lbLater() {
  if (lb && !lb.drawing) {
    lb.drawing = true;
    requestAnimationFrame(lbDraw);
  }
}

function lbClose() {
  if (!lb) return;
  const focus = lb.focus;
  lb.el.remove();
  lb = null;
  if (focus?.isConnected) focus.focus({ preventScroll: true });
  desktopBrowser.sync();
  composerKeeper.check();
}

function lbPinch() {
  const [a, b] = [...lb.pointers.values()];
  return { d: Math.hypot(a.x - b.x, a.y - b.y), x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

function lbOpen(url) {
  lbClose();
  const el = create(App.lb_node_in(App.lang(ui), url));
  const img = el.querySelector("img");
  lb = { el, img, v: App.lb_open(), drawing: false, pointers: new Map(), pinch: null, focus: document.activeElement };
  installImageActions(el, img, url);
  img.style.visibility = "hidden"; // until loaded; lb_css then sets the whole style
  img.addEventListener("load", lbDraw);
  el.addEventListener("wheel", (e) => {
    e.preventDefault();
    const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    const mag = Math.max(1, Math.round(Math.abs(e.deltaY) * k));
    if (e.deltaY === 0) return;
    lb.v = App.lb_wheel(lb.v, e.deltaY > 0, BigInt(mag), e.ctrlKey, lbN(e.clientX), lbN(e.clientY), lbBox(), lbNow());
    lbDraw();
  }, { passive: false });
  el.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target.closest("button,[role=status]")) return;
    e.preventDefault();
    el.setPointerCapture(e.pointerId);
    lb.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (lb.pointers.size === 2) {
      lb.v = App.lb_release(lb.v, lbN(e.clientX), lbN(e.clientY), lbBox(), lbNow());
      lb.pinch = lbPinch();
    } else if (lb.pointers.size === 1) {
      lb.v = App.lb_press(lb.v, lbN(e.clientX), lbN(e.clientY), lbBox(), lbNow());
    }
  });
  el.addEventListener("pointermove", (e) => {
    if (!lb.pointers.has(e.pointerId)) return;
    lb.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (lb.pinch && lb.pointers.size === 2) {
      const p = lbPinch();
      lb.v = App.lb_pinch(lb.v, lbN(lb.pinch.d), lbN(p.d), lbN(p.x), lbN(p.y), lbBox(), lbNow());
      lb.pinch = p;
    } else if (lb.pointers.size === 1) {
      lb.v = App.lb_move(lb.v, lbN(e.clientX), lbN(e.clientY), lbBox(), lbNow());
    }
    lbDraw();
  });
  const up = (e) => {
    if (!lb || !lb.pointers.has(e.pointerId)) return;
    lb.pointers.delete(e.pointerId);
    if (lb.pinch) {
      if (lb.pointers.size === 0) lb.pinch = null;
      return;
    }
    const closes = e.type === "pointerup" && App.lb_closes(lb.v, lbN(e.clientX), lbN(e.clientY));
    lb.v = App.lb_release(lb.v, lbN(e.clientX), lbN(e.clientY), lbBox(), lbNow());
    if (closes) lbClose();
    else lbDraw();
  };
  el.addEventListener("pointerup", up);
  el.addEventListener("pointercancel", up);
  el.addEventListener("click", (e) => {
    if (e.target.closest("[data-lb-close]")) lbClose();
  });
  document.body.appendChild(el);
  desktopBrowser.sync();
  el.querySelector('[data-lb-close]').focus({ preventScroll: true });
  if (img.complete) lbDraw();
}

// a viewer's Fit button
document.addEventListener("click", (e) => {
  if (!e.target.closest?.('[data-view="fit"]')) return;
  Plot2d.fit();
  Solid.fit();
});
// The hub issues page cursors. The virtual list accumulates their data in
// Bend; changing pages never removes the reader's already loaded rows.
let laterAsked = false, historyFrame = false, historyDirection = "", historyTouchY = null;
function earlierCheck(tl) {
  if (historyDirection !== "older" || earlierAsked || !tl || tl.id !== "timeline" || !thin?.ready || thin.paging || thin.pageError) return;
  if (!thin.window?.older || tl.scrollTop > tl.clientHeight * 2) return;
  earlierAsked = thin.page();
}
function laterCheck(tl) {
  if (historyDirection !== "newer" || laterAsked || !tl || !thin?.ready || thin.atNewest() || thin.paging || thin.pageError) return;
  if (tl.scrollHeight - tl.scrollTop - tl.clientHeight > tl.clientHeight * 2) return;
  laterAsked = thin.newer();
}
function historyCheckSoon() {
  if (historyFrame) return;
  historyFrame = true;
  requestAnimationFrame(() => {
    historyFrame = false;
    const tl = document.getElementById("timeline");
    earlierCheck(tl);laterCheck(tl);
  });
}
document.addEventListener("wheel", e => {
  if (!e.isTrusted || e.ctrlKey || !e.target.closest?.("#timeline")) return;
  const tl = document.getElementById("timeline");
  historyDirection = e.deltaY < 0 ? "older" : e.deltaY > 0 ? "newer" : historyDirection;
  if (e.deltaY < 0) earlierCheck(tl);
  else if (e.deltaY > 0) laterCheck(tl);
}, { capture: true, passive: true });

for (const event of ["touchstart", "touchmove", "keydown"]) document.addEventListener(event, e => {
  if (!e.isTrusted || !e.target.closest?.("#timeline") && !(event === "keydown" && e.target === document.body)) return;
  if (event === "touchstart") historyTouchY = e.touches[0]?.clientY ?? null;
  else if (event === "touchmove") {
    const y = e.touches[0]?.clientY;
    if (typeof y === "number" && typeof historyTouchY === "number") historyDirection = y > historyTouchY ? "older" : y < historyTouchY ? "newer" : historyDirection;
    historyTouchY = y;
  } else if (["PageUp", "Home", "ArrowUp"].includes(e.key)) historyDirection = "older";
  else if (["PageDown", "End", "ArrowDown", " "].includes(e.key)) historyDirection = "newer";
  historyCheckSoon();
}, { capture: true, passive: true });

document.addEventListener("click", (e) => {
  if (e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey) return;
  const a = e.target.closest?.("[data-lightbox]");
  if (!a) return;
  e.preventDefault();
  lbOpen(a.getAttribute("data-lightbox"));
});

// while the lightbox is up, its keys are its own
document.addEventListener("keydown", (e) => {
  if (!lb) return;
  e.stopImmediatePropagation();
  if (e.key === 'Tab') {
    const controls = [...lb.el.querySelectorAll('button:not([hidden]):not(:disabled)')];
    const at = controls.indexOf(document.activeElement);
    e.preventDefault();
    controls[(at + (e.shiftKey ? controls.length - 1 : 1)) % controls.length]?.focus();
  }
  if (App.lb_key(e.key)) {
    e.preventDefault();
    lbClose();
  }
}, true);

window.addEventListener("resize", () => lbDraw());

// The sidebar and the workspace rail fold (a button in each, Ctrl+B for
// the sidebar): the sidebar to a strip with its button, the rail to its
// icons. Layout only, like the viewer's width below: kept in this browser,
// on <html> so a render never undoes it (style.css's html[data-side],
// html[data-rail]).
const FOLDS = { side: ["backplane-side", "shut"], rail: ["backplane-rail", "compact"] };
function foldSet(which, on) {
  const [, word] = FOLDS[which];
  if (on) document.documentElement.dataset[which] = word;
  else delete document.documentElement.dataset[which];
}
function foldToggle(which) {
  // the sidebar folded by itself beside the viewer (panes below): the
  // button brings it back, and folds it again, for as long as the viewer
  // stays open; the person's own choice is left as it was
  if (which === "side" && (document.documentElement.dataset.fold || sideKept)) {
    sideKept = !sideKept;
    panes();
    return true;
  }
  const on = !document.documentElement.dataset[which];
  foldSet(which, on);
  try { localStorage.setItem(FOLDS[which][0], on ? "1" : "0"); } catch {}
  // a sidebar opened by hand while the viewer is open stays open
  if (which === "side" && !on) sideKept = true;
  panes();
  return true;
}
for (const which of Object.keys(FOLDS)) { try { foldSet(which, localStorage.getItem(FOLDS[which][0]) === "1"); } catch {} }

// The panes of a wide page (core/panes.bend decides, app.bend's panes_*):
// with the viewer open, the sidebar folds by itself where the window
// cannot hold it, the thread at its minimum and a fair viewer, and comes
// back when the viewer closes or the window grows; the viewer takes its
// width (dragged, or its share) but never the thread's minimum. Layout
// only, like the folds above: data-fold on <html> and --viewer-w. The
// viewer's left edge (view.bend's View.split) drags its width, kept in
// this browser; a double-click on the edge gives the pane its share back.
// The combined view (Split) keeps a width of its own, by default all but
// the thread's minimum.
const VIEWER_W = "backplane-viewer-w";
const VIEWER_W_SPLIT = "backplane-viewer-w.split";
// the sidebar's width open (style.css's .app columns)
const SIDE_W = 264;
let sideKept = false;
const wide = () => window.matchMedia("(min-width: 761px)").matches;
const splitOn = () => !!document.querySelector("#viewer.split");
const viewerKey = () => (splitOn() ? VIEWER_W_SPLIT : VIEWER_W);
function viewerWidth(w) {
  const root = document.documentElement;
  if (w == null) { root.style.removeProperty("--viewer-w"); return; }
  root.style.setProperty("--viewer-w", `${Math.round(w)}px`);
  const edge = document.querySelector(".viewer-split");
  if (edge) {
    const s = panesSize(), side = root.dataset.fold ? s.strip : s.side;
    const at = drag => Number(App.panes_viewer(panesN(s.w), panesN(side), panesN(s.strip), panesN(s.rail), panesN(drag), splitOn(), true));
    edge.setAttribute("aria-valuemin", String(at(1)));
    edge.setAttribute("aria-valuemax", String(at(s.w)));
    edge.setAttribute("aria-valuenow", String(Math.round(w)));
  }
}
function viewerDrag() {
  let w = 0;
  try { w = Number(localStorage.getItem(viewerKey())) || 0; } catch {}
  return w;
}
// what the panes are measured with: the page, the sidebar as the person
// left it and folded, the rail
function panesSize() {
  const app = document.querySelector(".app");
  const strip = 44; // folded, the sidebar is a strip (style.css)
  const rail = document.querySelector(".workspace-thread > .workspace-bar")?.getBoundingClientRect().width ?? 0;
  return { app, w: app?.getBoundingClientRect().width ?? window.innerWidth,
    side: document.documentElement.dataset.side ? strip : SIDE_W, strip, rail };
}
const panesN = (x) => BigInt(Math.max(0, Math.round(x)));
// The sidebar the person opens where it cannot stand beside the thread's
// minimum and the viewer (App.panes_drawer) lies over the thread as a
// drawer, data-fold="drawer": a click outside it, Esc or a thread picked
// (here or by any other way: drawerSel, the thread it opened over) puts
// it back to its strip
let drawerSel = null;
const selNow = () => (thin?.session?.sel ?? App.sel(ui));
function drawerShut() {
  if (document.documentElement.dataset.fold !== "drawer") return false;
  sideKept = false;
  panes();
  return true;
}
function panes() {
  const root = document.documentElement;
  if (!document.querySelector(".app.panel") || !wide()) {
    // the viewer shut (or a phone-sized page): the sidebar as it was
    sideKept = false;
    drawerSel = null;
    delete root.dataset.fold;
    return;
  }
  if (sideKept && drawerSel !== null && selNow() !== drawerSel) sideKept = false;
  const s = panesSize();
  const fold = App.panes_fold(panesN(s.w), panesN(s.side), panesN(s.strip), panesN(s.rail), sideKept);
  const drawer = App.panes_drawer(panesN(s.w), panesN(s.side), panesN(s.strip), panesN(s.rail), sideKept);
  if (drawer) root.dataset.fold = "drawer"; else if (fold) root.dataset.fold = "auto"; else delete root.dataset.fold;
  drawerSel = drawer ? (drawerSel ?? selNow()) : null;
  viewerWidth(Number(App.panes_viewer(panesN(s.w), panesN(s.side), panesN(s.strip), panesN(s.rail), panesN(viewerDrag()), splitOn(), sideKept)));
}
document.addEventListener("pointerdown", (e) => {
  if (document.documentElement.dataset.fold === "drawer" && e.target instanceof Element && !e.target.closest(".sidebar")) drawerShut();
}, true);
// (on the window, ahead of the page's own Esc: the drawer goes first, the
// viewer stays)
window.addEventListener("keydown", (e) => {
  if (e.key !== "Escape" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return;
  if (!drawerShut()) return;
  e.preventDefault();
  e.stopImmediatePropagation();
}, true);
document.addEventListener("pointerdown", (e) => {
  const edge = e.target instanceof Element ? e.target.closest(".viewer-split") : null;
  if (!edge || e.button !== 0) return;
  e.preventDefault();
  const pane = edge.parentElement;
  const right = pane.getBoundingClientRect().right;
  try { edge.setPointerCapture(e.pointerId); } catch {}
  edge.classList.add("on");
  document.body.classList.add("resizing");
  let w = pane.getBoundingClientRect().width;
  // a drag moves the viewer's edge beside the sidebar as it is now (it
  // never folds or brings back the sidebar), the thread kept its minimum
  const s = panesSize();
  const side = document.documentElement.dataset.fold ? s.strip : s.side;
  const move = (m) => {
    w = Number(App.panes_viewer(panesN(s.w), panesN(side), panesN(s.strip), panesN(s.rail), panesN(Math.max(1, right - m.clientX)), splitOn(), true));
    viewerWidth(w);
  };
  const done = () => {
    edge.removeEventListener("pointermove", move);
    edge.removeEventListener("pointerup", done);
    edge.removeEventListener("pointercancel", done);
    edge.classList.remove("on");
    document.body.classList.remove("resizing");
    try { localStorage.setItem(viewerKey(), String(Math.round(w))); } catch {}
  };
  edge.addEventListener("pointermove", move);
  edge.addEventListener("pointerup", done);
  edge.addEventListener("pointercancel", done);
});
document.addEventListener("dblclick", (e) => {
  if (!(e.target instanceof Element) || !e.target.closest(".viewer-split")) return;
  try { localStorage.removeItem(viewerKey()); } catch {}
  panes();
});
document.addEventListener("keydown", (e) => {
  const edge = e.target.closest?.(".viewer-split");
  if (!edge || !wide() || e.ctrlKey || e.metaKey || e.altKey || e.isComposing || e.keyCode === 229) return;
  if (e.key === "Home") {
    e.preventDefault();
    try { localStorage.removeItem(viewerKey()); } catch {}
    panes();
  } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
    e.preventDefault();
    const s = panesSize(), side = document.documentElement.dataset.fold ? s.strip : s.side;
    const delta = (e.shiftKey ? 64 : 16) * (e.key === "ArrowLeft" ? 1 : -1);
    const w = Number(App.panes_viewer(panesN(s.w), panesN(side), panesN(s.strip), panesN(s.rail),
      panesN(Math.max(1, edge.parentElement.getBoundingClientRect().width + delta)), splitOn(), true));
    viewerWidth(w);
    try { localStorage.setItem(viewerKey(), String(Math.round(w))); } catch {}
  }
});
window.addEventListener("resize", () => { panes();composerGeometry();composerTop(true); });

// Socket
// ------

// Authentication is carried by the same-origin HttpOnly pairing cookie.
// Native clients retain their existing token protocol.

function jsonValue(j, key) {
  for (let f = j?.fields; f && is(f, "Field"); f = f.tail) if (f.key === key) return f.value;
  return { $: T.Null };
}
const presentationJson = value => App.presentation_json(JSON.stringify(value));
const choosePresentation = (session, action, value) => {
  const chosen = Wire.plain(App.presentation_choice(action, presentationJson(session), value));
  return action === "draft" ? preservePresentationRoute(session, chosen) : chosen;
};
function localPresentation(action, value, session) {
  if (action === "select") {
    ui = App.presentation_select(ui, value);
    try { localStorage.setItem("backplane-selected", value); } catch {}
    holdPicture();
    remoteReady = false;
    if (!restoreTransition(presentationTarget(session))) void restoreFocused();
    later();
  } else if (action === "draft") {
    applyComposerDraft(document, session.sel, value);
    ui = App.restore(ui, session.sel, value);
    // the hub's page is redrawn with the draft at once (app.bend's
    // presentation_overlay: the send button and the slash commands follow
    // what is typed even while the hub is busy with a streaming turn)
    if (frontendNode) later();
    try {
      if (value) localStorage.setItem(DRAFT + session.sel, value);
      else localStorage.removeItem(DRAFT + session.sel);
    } catch {}
  }
}
function adoptPresentation(session) { ui = App.presentation_handoff(ui, presentationJson(session)); }
function performPresentation(operation) {
  const r = App.presentation_perform(ui, presentationJson(operation.input), operation.action, operation.value);
  ui = r.ui;
  run(r.cmds);
}
function presentationCommand(command) {
  const tag = { send: "Send", copy: "Copy", focus: "Focus", scroll: "ScrollEnd", connect: "Connect",
    keep: "Keep", notify: "Notify", later: "Later", jump: "Jump" }[command.type];
  if (!tag) throw Error("Unknown presentation command");
  run({ $: "Con", head: { ...command, $: tag }, tail: { $: "Nil" } });
}

// Shared thin transport -> platform IO and the existing normal Bend renderer.
// Neither this path nor its error/retry state touches canonical history.
function showViewStatus(message) {
  clearTimeout(connectingTimer);connectingTimer = 0;
  let box = document.getElementById("view-connection");
  if (!box) {
    box = document.createElement("aside");box.id = "view-connection";box.setAttribute("role", "status");
    box.style.cssText = "position:fixed;bottom:0;left:0;right:0;z-index:30;display:flex;flex-wrap:wrap;gap:8px;padding:8px;background:var(--panel);border-top:1px solid var(--line)";
    const text = document.createElement("span");text.id = "view-connection-text";box.append(text);
    for (const [label, action] of [["Retry now", retryNow]]) {
      const button = document.createElement("button");button.type = "button";button.textContent = label;button.addEventListener("pointerdown", refocusKeep);button.addEventListener("click", action);box.append(button);
    }
    document.body.append(box);
  }
  box.hidden = !message;
  box.style.display = message ? "flex" : "none";
  box.querySelector("button").hidden = message === viewNote;
  document.getElementById("view-connection-text").textContent = message;
  // The page shrinks above the bar instead of sliding under it.
  document.documentElement.style.setProperty("--status-h", message ? box.offsetHeight + "px" : "0px");
}

// A view the hub refused (thin-host.js's refuse): a search result or a
// reload that named something it has no view of. The page goes back to
// the thread on show before, else the project list (refusedFallback), says
// so for a while, and keeps nothing of the refused one: the selection
// saved for a reload is the one gone back to.
function refusedView(target, reason) {
  refusedViews.add(targetKey(target));
  trace("thin-refused", { reason });
  const back = refusedFallback(target, shownTarget, refusedViews);
  if (back === null || !thin) return false;
  searchJump = null;
  try { if (localStorage.getItem("backplane-selected") === target.id) localStorage.removeItem("backplane-selected"); } catch {}
  clearTimeout(viewNoteTimer);
  viewNote = back ? "That conversation can't be opened; back to the one before." : "That conversation can't be opened; here are your projects.";
  viewNoteTimer = setTimeout(() => { viewNote = "";if (remoteReady) showViewStatus(""); }, 8000);
  showViewStatus(viewNote);
  return thin.request("select", back);
}

// A first connection says so only past QUIET_MS (connectingStatus): a bar
// that came and went in a moment moved the whole page twice on every load.
let connectingTimer = 0;
function connectingStatus() {
  clearTimeout(connectingTimer);
  connectingTimer = setTimeout(() => { connectingTimer = 0;if (!remoteReady && !downSince) showViewStatus("Connecting…"); }, QUIET_MS);
}

// Reconnecting
// ------------
// A dropped view connection is tried again by itself: a jittered, growing
// delay (reconnect.js), cut short when the network comes back or the page is
// looked at again; a hub that is starting says how long with Retry-After.
// While it is down the last view stays on screen (actions wait, the draft is
// kept) and the bar says so only if the gap lasts past QUIET_MS.
const connectionLog = new ConnectionLog({ storage: localStorage, bundle: CONNECTION_BUNDLE,
  visibility: () => document.visibilityState || "unknown", online: () => navigator.onLine !== false });
const thinBackoff = new Backoff(), thinRetry = new Retry();
let downSince = 0, downNote = "", downTimer = 0, offlineSend = false;
// The top bar's dot follows the link while the bar says it is down: the
// hub drew it green and cannot redraw it now, so the page turns it (the
// root's hub-down class) and says the words the hub left in data-down.
function connDot(down) {
  document.documentElement.classList.toggle("hub-down", down);
  for (const dot of document.querySelectorAll(".brand-mark .conn")) {
    if (down && dot.dataset.down) {
      if (dot.dataset.upTitle === undefined) dot.dataset.upTitle = dot.title || "";
      dot.title = dot.dataset.down;dot.setAttribute("aria-label", dot.dataset.down);
    } else if (!down && dot.dataset.upTitle !== undefined) {
      dot.title = dot.dataset.upTitle;dot.setAttribute("aria-label", dot.dataset.upTitle);delete dot.dataset.upTitle;
    }
  }
}
function downStatus(note) {
  downNote = note;
  if (!downSince) downSince = Date.now();
  clearTimeout(downTimer);
  const wait = Math.max(0, downSince + QUIET_MS - Date.now());
  downTimer = setTimeout(() => { if (downSince) { showViewStatus(waiting(offlineSend ? "Not sent. Reconnect to send; your draft is kept." : downNote));connDot(true); } }, wait);
}
function upStatus() {
  downSince = 0;clearTimeout(downTimer);downTimer = 0;offlineSend = false;
  thinBackoff.reset();thinRetry.cancel();
  showViewStatus("");connDot(false);
}
function thinReconnect() {
  if (!pairingAccepted || viewMode === "compatibility" || thinRetry.pending) return;
  downStatus(downNote || "Reconnecting… Your draft is kept.");
  const attempt = thinAttempt;
  thinRetry.schedule(thinBackoff.next(), async () => {
    if (viewMode === "compatibility" || attempt !== thinAttempt) return;
    const hub = await probeHub(window.fetch.bind(window));
    // A Retry/wake/new connection can win while the old HTTP probe waits.
    // Its late answer must never replace that newer healthy socket.
    if (viewMode === "compatibility" || attempt !== thinAttempt) { connectionLog.record("retry-discarded", { cause: "stale-retry" });return; }
    if (hub.state === "up") { void startThin(true, "reconnect");return; }
    if (hub.state === "starting") { thinBackoff.retryAfter(hub.retryAfterMs);downStatus("Hub restarting… Your draft is kept."); }
    else downStatus(navigator.onLine === false ? "Offline. Reconnects when the network is back; your draft is kept." : "Hub unreachable. Reconnecting… Your draft is kept.");
    thinReconnect();
  });
}
// The outbox: a message sent in view mode is kept until the hub answers it.
// A drop before the answer (or a Send while down) leaves it here, and every
// new view connection sends it again. The hub stores a client-named message
// once (Hub.dup), so a resend never posts twice. Kept in localStorage, so a
// reload or a crash loses nothing; a refusal puts the text back in its
// thread's draft. Compatibility mode has Bend's own outbox.
function outboxSave() {
  try { if (outbox.size) localStorage.setItem(OUTBOX, JSON.stringify([...outbox.values()]));else localStorage.removeItem(OUTBOX); } catch {}
}
function outboxKeep(text) {
  if (viewMode === "compatibility") return;
  let r;try { r = JSON.parse(text); } catch { return; }
  const msg = r?.p?.msg;
  if (r?.m !== "turn.start" || typeof msg !== "string" || !msg) return;
  outbox.set(msg, { msg, id: r.id, text, thread: String(r.p.thread || ""), body: String(r.p.text || ""), at: Date.now() });
  outboxSave();
}
function outboxReply(envelope) {
  for (const o of outbox.values()) if (o.id === envelope.id) {
    outbox.delete(o.msg);outboxSave();
    if (envelope.ok === false) outboxRefused(o, envelope.error);
    return;
  }
}
function outboxRefused(o, error) {
  const session = currentSession();
  if (thin && session.sel === o.thread && !session.drafts?.[o.thread]) thin.request("draft", o.body);
  else try { if (!localStorage.getItem(DRAFT + o.thread)) localStorage.setItem(DRAFT + o.thread, o.body); } catch {}
  showViewStatus(`A message was not sent (${String(error || "refused")}); its text is back in its draft.`);
}
function outboxFlush() {
  if (!socket || socket.readyState !== 1) return;
  for (const o of outbox.values()) {
    if (Date.now() - o.at > OUTBOX_DAY) { outbox.delete(o.msg);outboxRefused(o, "too old");continue; }
    socket.send(fromList(App.wire_out(o.text)));
  }
  outboxSave();
}
function waiting(note) {
  return outbox.size ? `${note} ${outbox.size === 1 ? "1 message" : outbox.size + " messages"} will send once connected.` : note;
}

function retryNow() {
  if (viewMode === "compatibility") return;
  thinBackoff.reset();
  if (thinRetry.pending) thinRetry.wake();
  else void startThin(false, "retry");
}

// a dialog over the page (the hub's answers to Search and to a message's
// rest): the page dimmed behind it, its title and × in a head, Esc or a
// click outside closes it
function resultPanel(title, kind = "") {
  document.getElementById("thin-result")?.remove();
  const shade = document.createElement("div");shade.id = "thin-result";shade.className = "shade dim";
  const panel = document.createElement("section");panel.className = kind ? "picker " + kind : "picker";
  panel.setAttribute("role", "dialog");panel.setAttribute("aria-modal", "true");panel.setAttribute("aria-label", title);
  const before = document.activeElement;
  const close = () => { shade.remove();root.inert = false;before?.focus?.();desktopBrowser.sync(); };
  const heading = document.createElement("header");heading.className = "phead";
  const label = document.createElement("span");label.className = "ptitle";label.textContent = title;heading.append(label);
  const button = document.createElement("button");button.type = "button";button.textContent = "×";button.setAttribute("aria-label", "Close");button.title = "Close · Esc";button.addEventListener("click", close);heading.append(button);
  panel.append(heading);shade.append(panel);document.body.append(shade);root.inert = true;
  shade.addEventListener("click", e => { if (e.target === shade) close(); });
  shade.addEventListener("keydown", e => {
    if (e.key === "Escape") { e.preventDefault();e.stopPropagation();close(); }
    if (e.key === "Tab") {
      const focusable = [...panel.querySelectorAll("button,input")].filter(e => !e.disabled && !e.hidden);
      const at = focusable.indexOf(document.activeElement), next = (at + (e.shiftKey ? focusable.length - 1 : 1)) % focusable.length;
      e.preventDefault();focusable[next]?.focus();
    }
  });
  button.focus();desktopBrowser.sync();return { panel, close, shade };
}
// The top bar's search (Ctrl+Shift+F) over every thread: the hub answers
// a page of results in the order shown (server/thin-view.bend's
// Search.found: the open thread's first, then by project and thread),
// each with its snippet (snip: plain text and the query's words by turns)
// and when it was said. It asks as the person types, one search at a
// time (what was typed meanwhile goes when the answer comes); More
// results adds the next page; Up/Down choose, Enter opens, Esc closes.
let thinFind = null;
function thinFindHead(row) {
  const head = document.createElement("div");head.className = "find-group";
  if (row.project) {
    const p = document.createElement("span");p.className = "find-project";p.textContent = row.project;
    const sep = document.createElement("span");sep.className = "find-sep";sep.textContent = "/";head.append(p, sep);
  }
  const t = document.createElement("span");t.className = "find-thread";t.textContent = row.title || "Untitled thread";head.append(t);
  if (row.here) { const h = document.createElement("span");h.className = "find-here";h.textContent = "this thread";head.append(h); }
  return head;
}
function thinFindRow(row, n) {
  const button = document.createElement("button");button.type = "button";button.className = "find-hit";
  button.id = "find-hit-" + n;button.setAttribute("role", "option");
  const snip = document.createElement("span");snip.className = "find-snip";
  const pieces = Array.isArray(row.snip) && row.snip.length ? row.snip : [row.text + (row.partial ? " …" : "")];
  pieces.forEach((piece, i) => {
    if (i % 2) { const mark = document.createElement("mark");mark.textContent = piece;snip.append(mark); }
    else if (piece) snip.append(document.createTextNode(piece));
  });
  button.append(snip);
  if (row.when) { const when = document.createElement("span");when.className = "find-when";when.textContent = row.when;button.append(when); }
  return button;
}
function showThinSearch(search) {
  thinFind = null;
  const { panel, close } = resultPanel("Search threads", "search-threads");
  const query = document.createElement("input");query.type = "text";query.value = search.query;
  query.placeholder = "Search every thread";query.autocomplete = "off";
  query.setAttribute("aria-label", "Search every thread");query.setAttribute("role", "combobox");
  query.setAttribute("aria-controls", "find-rows");query.setAttribute("aria-expanded", "true");
  const note = document.createElement("div");note.className = "find-note";note.setAttribute("role", "status");
  const rows = document.createElement("div");rows.id = "find-rows";rows.className = "find-rows";rows.setAttribute("role", "listbox");
  const foot = document.createElement("footer");foot.textContent = "↑↓ choose · Enter open · Esc close";
  panel.append(query, note, rows, foot);
  // asked: the search on its way (its query and cursor); next: what to ask
  // once it is answered; shown: the query whose results are on screen
  const f = { asked: null, next: null, shown: null, hits: [], at: -1, group: null, timer: 0, more: null };
  const choose = at => {
    f.hits[f.at]?.el.classList.remove("on");f.hits[f.at]?.el.setAttribute("aria-selected", "false");
    f.at = at;const hit = f.hits[at];
    if (!hit) { query.removeAttribute("aria-activedescendant");return; }
    hit.el.classList.add("on");hit.el.setAttribute("aria-selected", "true");query.setAttribute("aria-activedescendant", hit.el.id);
    hit.el.scrollIntoView?.({ block: "nearest" });
  };
  const open = row => {
    close();thinFind = null;
    // the word lit in its snippet (else, a match past what the hub reads
    // for snippets, the first word asked)
    const word = Array.isArray(row.snip) && row.snip[1] || (query.value.match(/[\p{L}\p{N}]+/u)?.[0] ?? "");
    // (jump: the entry shown for it, a subagent's step at the call that
    // started it, server/thin-view.bend's Search.row)
    searchJump = row.id ? { id: row.jump || row.id, thread: row.thread, pages: 0, asked: false, word } : null;
    dispatch("select", row.thread);later();
  };
  const ask = (text, cursor) => {
    clearTimeout(f.timer);
    if (!rows.isConnected) return;
    if (!text.trim()) { f.next = null;f.shown = null;f.hits = [];f.at = -1;f.more = null;rows.replaceChildren();note.textContent = "Type a word said in any thread.";return; }
    // one at a time: an answer lost is given up after a while
    if (f.asked && Date.now() - f.asked.at < 8000) { f.next = { text, cursor };return; }
    f.next = null;f.asked = { text, cursor, at: Date.now() };
    if (!cursor) note.textContent = "Searching…";
    if (!thin?.search(text, cursor)) { f.asked = null;note.textContent = "Search needs the hub: not connected."; }
  };
  const draw = (search, more) => {
    if (!more) { rows.replaceChildren();f.hits = [];f.at = -1;f.group = null; }
    f.more?.remove();f.more = null;
    for (const row of search.rows || []) {
      if (row.thread !== f.group) { rows.append(thinFindHead(row));f.group = row.thread; }
      const el = thinFindRow(row, f.hits.length), n = f.hits.length;
      el.addEventListener("click", () => open(row));
      el.addEventListener("mousemove", () => { if (f.at !== n) choose(n); });
      rows.append(el);f.hits.push({ row, el });
    }
    if (search.hasMore && search.cursor) {
      const button = document.createElement("button");button.type = "button";button.className = "find-more";button.textContent = "More results";
      button.addEventListener("click", () => { ask(search.query, search.cursor);query.focus(); });rows.append(button);f.more = button;
    }
    f.shown = search.query;
    note.textContent = f.hits.length ? "" : `Nothing matches “${search.query.trim()}”.`;
    if (!more && f.hits.length) choose(0);
  };
  thinFind = {
    failed: code => {
      f.asked = null;
      if (f.next) { const next = f.next;f.next = null;ask(next.text, next.cursor);return; }
      rows.replaceChildren();f.hits = [];f.at = -1;f.more = null;f.shown = null;
      note.textContent = code === "LIMIT" ? "Search takes one to eight words of letters or digits." : `The search failed (${code}).`;
    },
    answer: search => {
      const asked = f.asked;f.asked = null;
      if (f.next) {
        const next = f.next;f.next = null;
        if (next.text !== search.query || next.cursor) ask(next.text, next.cursor);
        if (next.text !== search.query) return;
      }
      if (search.query !== query.value) return;
      draw(search, !!(asked?.cursor && asked.text === search.query && f.shown === search.query));
    },
  };
  query.addEventListener("input", () => {
    clearTimeout(f.timer);
    if (!query.value.trim()) { ask("", "");return; }
    f.timer = setTimeout(() => ask(query.value, ""), 180);
  });
  query.addEventListener("keydown", e => {
    if (e.isComposing) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (f.hits.length) choose((f.at + (e.key === "ArrowDown" ? 1 : f.hits.length - 1)) % f.hits.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (f.shown === query.value && f.hits[f.at]) open(f.hits[f.at].row);
      else ask(query.value, "");
    }
  });
  if (search.rows?.length) draw(search, false);
  else note.textContent = search.query ? "" : "Type a word said in any thread.";
  query.focus();
}
function thinSearchAnswer(search) {
  if (thinFind && document.getElementById("find-rows")?.isConnected) thinFind.answer(search);
}
function thinSearchFailed(code) {
  if (thinFind && document.getElementById("find-rows")?.isConnected) thinFind.failed(code);
}
function showThinMessage(message) {
  if (viewMode === "view-v1") {
    if (thinContinuations.accept(message)) { showViewStatus("");later(); }
    else showViewStatus("Message continuation is unavailable; reopen it explicitly");
    return;
  }
  const { panel } = resultPanel(message.complete ? "Message" : "Partial message");
  const text = document.createElement("pre");text.textContent = message.text;text.style.cssText = "white-space:pre-wrap;overflow:auto;min-height:0;padding:12px";panel.append(text);
  if (!message.fromStart || message.hasMore) {
    const restart = document.createElement("button");restart.type = "button";restart.textContent = "Read from start";
    restart.addEventListener("click", () => thin?.messageFromStart(message.id));panel.append(restart);
  }
  if (message.hasMore) {
    const more = document.createElement("button");more.type = "button";more.textContent = "More text";
    more.addEventListener("click", () => thin?.message(message.id, message.cursor));panel.append(more);
  }
}
function stopThin(keepView = false, reason = "replaced") {
  bubMoves.disconnect(location.origin);
  terminalOutput.clear();
  checkpointFocused();
  thinRetry.cancel();
  thinContinuations.clear();
  thinAttempt++;clearInterval(thinExpiry);clearTimeout(thinGreetingTimer);clearTimeout(thinDraftTimer);clearTimeout(thinFieldTimer);
  const previous = thin;thin = null;
  const s = thinSocket;thinSocket = null;
  if (socket === s) socket = null;
  if (s) { s.connectionDiagnostic?.initiate(reason);s.onmessage = null;try { s.close(); } catch {} }
  if (previous) previous.disconnect();
  remoteReady = false;desktopBrowser.hide();Plot2d.reset();Solid.reset();ViewerTools.reset();viewerRpc.reset();
  if (!keepView) frontendNode = null;
}
async function startThin(again = false, reason = "initial") {
  if (!pairingAccepted) return;
  if (viewMode === "compatibility") return;
  stopThin(again || downSince > 0, reason);const attempt = thinAttempt;
  viewMode = "discovering";
  if (downSince) downStatus(downNote);
  else connectingStatus();
  later();
  const capability = await discoverViews({ fetch: window.fetch.bind(window), validate: viewCapability });
  if (attempt !== thinAttempt || viewMode === "compatibility") return;
  if (!capability.supported && capability.offline) {
    viewMode = "offline";later();thinReconnect();return;
  }
  if (!capability.supported) {
    viewMode = "read-only";showViewStatus("This hub needs an update to work with this page. Saved conversation is read-only.");later();return;
  }
  viewMode = "view-v1";
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const s = new WebSocket(`${proto}//${location.host}/ws?view=1&frontend=1&enc=cbor&since=0&origin=`);
  thinSocket = s;s.binaryType = "arraybuffer";
  const diagnostic = s.connectionDiagnostic = connectionLog.connection();
  diagnostic.note("created", { cause: reason });
  let ended = false;
  const current = () => !ended && attempt === thinAttempt && viewMode === "view-v1";
  const fail = message => {
    if (!current()) return;
    checkpointFocused();remoteReady = false;desktopBrowser.hide();Plot2d.reset();Solid.reset();ViewerTools.reset();viewerRpc.reset();
    showViewStatus(message);void restoreFocused(true);later();
  };
  const disconnect = event => {
    if (!current()) return;
    if (typeof event === "string") diagnostic.initiate(event);
    trace("thin-disconnect", { reason: typeof event === "string" ? event : event?.type || "connection-ended", code: event?.code });
    ended = true;
    refocusKeep();
    keepSettingsRecovery();
    bubMoves.disconnect(location.origin);
    terminalOutput.clear();
    checkpointFocused();remoteReady = false;desktopBrowser.hide();Plot2d.reset();Solid.reset();ViewerTools.reset();viewerRpc.reset();
    clearTimeout(thinGreetingTimer);clearInterval(thinExpiry);clearTimeout(thinDraftTimer);clearTimeout(thinFieldTimer);
    bodyWindows.disconnect();thin?.disconnect();if (socket === s) socket = null;
    if (!frontendNode) void restoreFocused(true);
    later();thinReconnect();
  };
  try {
    const saved = localStorage.getItem("backplane-view-request-id");
    const next = saved === null ? 2 ** 47 : /^(0|[1-9][0-9]*)$/.test(saved) ? Number(saved) : NaN;
    if (!Number.isSafeInteger(next) || next < 2 ** 47 || next >= 2 ** 48 - 1) throw Error("Invalid view counter");
    thin = new ThinHost({
      hub: location.origin, cid, bodyPages: true, now: () => performance.now(), session: currentSession,
      target: presentationTarget,
      counter: value => localStorage.setItem("backplane-view-request-id", String(value)),
      send: request => {
        if (!current() || s.readyState !== 1) throw Error("View connection closed");
        trace("send", { id: request.id, method: request.m });
        s.send(fromList(App.wire_out(JSON.stringify({ ...request, p: { ...request.p, workspaceLayout: "rail-v1", workspaceChrome: "top-v1", terminalOutput: "pane-v1" } }))));
      },
      choice: choosePresentation, prepare: (session, input, action) => preservePresentationRoute(
        action === "send" || action === "send-alt" ? input : session,
        Wire.plain(App.presentation_prepare(presentationJson(session), presentationJson(input), action))),
      selection: (target, session) => {
        historyClear();
        modelFeedback.clear();
        thinMailDraft.clear();thinContinuations.clear();bodyWindows.disconnect();
        if (target.kind === "thread") { ui = App.presentation_select(ui, target.id);void restoreFocused(); }
        else { focusedRequestedKey = null;focusedView.invalidate(); }
        restoreTransition(target);
      },
      local: localPresentation,
      modelMenuReady: () => {
        // A picker whose cursor moved (data-draft) opens and shuts through
        // the hub, so its picture never shows a cursor dropped since.
        const layer = document.querySelector('[data-local-models="1"]');
        return !!layer && !layer.hasAttribute('data-draft') && !layer.querySelector('input')?.value;
      },
      sessionChanged: (session, detail) => {
        thinSession = session;
        if (detail?.remote) {
          // Only live response choices cross this boundary. State and applied
          // cursor stay in the fresh unsynced recipient, never in a picture.
          ui = App.unsync(App.presentation_handoff(ui, presentationJson(session)));
          document.documentElement.lang = App.lang(ui);
          keepViewer();
          setTimeout(() => restoreDraftAttachments(currentSession()), 0);
          // A hub action can open a thread too (Ctrl+N, Fork, the palette).
          // Keep the accepted selection as well as explicit sidebar clicks.
          try { localStorage.setItem("backplane-selected", session.sel || ""); } catch {}
        }
        if (!detail?.inputOnly) later();
      },
      ack: (session, reply) => preservePresentationRoute(session, Wire.plain(App.presentation_reply(presentationJson(session), presentationJson(reply)))),
      decode: shown => App.presentation_node(presentationJson(shown)),
      terminal: (node, frame) => terminalOutput.receive(node, frame),
      terminalLifetime: () => document.querySelector('.floating-terminal .term-pane')?.getAttribute('data-terminal-incarnation') || '',
      command: command => presentationCommand(command),
      anchor: () => { const tl = document.getElementById("timeline"), a = tl && anchorOf(tl);return { id: a?.id || "", offset: Math.max(0, Math.round(a ? tl.getBoundingClientRect().top - a.off : 0)) }; },
      pagePosition: () => {
        const list = timelineList();
        return list ? { position: { ...list.position(), atEnd: thin.atNewest() && list.atEnd() }, adjacent: "" } : null;
      },
      // Give an immediately following user action priority over an unsent
      // notification read. Timer is bounded and works while the page is hidden.
      deferRefresh: refresh => setTimeout(() => { if (current()) refresh(); }, 16),
      draftRefresh: refresh => { clearTimeout(thinDraftTimer);thinDraftTimer = setTimeout(() => { if (current()) refresh(); }, 75); },
      fieldRefresh: refresh => { clearTimeout(thinFieldTimer);thinFieldTimer = setTimeout(() => { if (current()) refresh(); }, 75); },
      fieldsChanged: (name, text) => { if (name === "mrreply") thinMailDraft.edit(text);later(); },
      operationScope: (action, value) => action === "mail-send" ? thinMailDraft.scope(true) : action === "bfield" && value.startsWith("mrreply\u001f") ? thinMailDraft.scope() : undefined,
      blocked: () => showViewStatus("Wait for the current conversation to connect."),
      viewerChoice: choice => viewerFeedback.choice(choice),
      warning: message => showViewStatus(message),
      pageState: historyPageState,
      refused: refusedView,
      search: thinSearchAnswer, searchFailed: thinSearchFailed, message: showThinMessage, body: (message, input) => bodyWindows.accept(message, input), bodyFailed: (input, reason) => bodyWindows.failed(input, reason),
      view: (node, detail) => {
        if (!current()) return;
        if (detail.page?.direction === "newest") scroll = true;
        const previous = historyNode || node;
        node = History.project(previous, node, historyNode ? detail.page?.direction || "update" : "reset");
        historyNode = node;
        terminalOutput.rebind();
        const field = mailReplyField(detail.shown);
        const accepted = field === undefined ? undefined : thinMailDraft.matches(detail.mailDraftContext, detail.epoch) ? thin.fieldValue("mrreply", field) : field;
        thinMailDraft.bind(detail.mailDraftContext, detail.epoch, accepted);
        thinContinuations.bind(location.origin, detail);
        bodyWindows.bind(location.origin, detail, detail.accountBinding || "", node, held);
        if (detail.position) transitionPosition = detail.position;
        else if (!remoteReady) transitionPosition = transitionViews.position(transitionScope(detail.target)) || {atEnd:true};
        focusedView.remote(detail.epoch);remoteEpoch = detail.epoch;frontendNode = node;transitionNode = null;transitionFor = "";heldAt = 0;
        shownFor = targetKey(detail.target);shownTarget = detail.target;transitionViews.put(transitionScope(detail.target), detail.shown);
        trace("thin-view", { entries: detail.window?.ids?.length || 0 });later();
        fabLook(detail.target);
      },
      state: state => {
        if (!current()) return;
        remoteReady = state.remoteReady === true;
        if (!remoteReady) {
          modelFeedback.clear();
          // Keep the last validated tree as a read-only picture. The local Ui
          // has no canonical bot/catalog state in view mode. Falling back to
          // it here removed the sidebar and cats after a refusal or disconnect.
          // A selection's callback replaces it only with a retained picture of
          // the thread picked; else it stays until the next view comes.
          holdPicture();desktopBrowser.hide();
        }
        if (state.phase === "error") {
          diagnostic.note("view-error", { cause: state.reason === "LIMIT" ? "view-limit" : "view-refused" });
          trace("thin-error", { reason: state.reason });
          fail(state.reason === "LIMIT" ? "This view exceeds the hub's size limit. Your draft is kept; retry explicitly."
            : "Conversation unavailable. Your draft is kept; retry explicitly.");
        }
        else if (remoteReady && !s.up) { diagnostic.note("ready");s.up = true;upStatus();outboxFlush();void checkDeploy(); }
        else if (remoteReady) showViewStatus("");
        if (remoteReady && viewNote) showViewStatus(viewNote);
        later();
      },
    }, ThinViews, next);
  } catch { disconnect("client-setup");try { s.close(); } catch {}return; }
  s.onopen = () => {
    if (!current()) return;
    socket = s;heard = BigInt(Date.now());s.hb = false;diagnostic.note("open");
  };
  s.onmessage = event => {
    if (!current() || typeof event.data === "string") return;
    heard = BigInt(Date.now());
    const bytes = new Uint8Array(event.data);
    diagnostic.frame(bytes.byteLength);
    if (Solid.isPlot(bytes)) {
      const o = Solid.cbor(bytes), at = { bytes: bytes.length, t: performance.now() };
      const key = typeof o?.key === "string" ? o.key : "";
      if (["2|", "3|", "4|"].some(prefix => key.startsWith(prefix))) Solid.got(o, at);
      else if (o) Plot2d.got(o, at);
      return;
    }
    if (bytes.byteLength > 262144) {
      diagnostic.note("error", { cause: "frame-limit", ...frameSummary(bytes, KEYS, WORDS) });
      disconnect("frame-limit");try { s.close(); } catch {}return;
    }
    try {
      const j = Wire.cbor(bytes, KEYS, WORDS), envelope = Wire.plain(j);
      if (envelope.t === "hb") { s.hb = true;diagnostic.heartbeat();return; }
      if (viewerRpc.receive(envelope)) return;
      if (envelope.t === "plotrevision") { ViewerTools.sourceRevision(envelope.key, envelope.revision); return; }
      if (envelope.t === "reply" && sealedReply(envelope)) return;
      if (envelope.t === "reply") outboxReply(envelope);
      if (envelope.t === "view") { diagnostic.greeting(envelope);clearTimeout(thinGreetingTimer);thin.greeting(envelope); }
      else if (!thin.receive(envelope) && envelope.t === "reply" && Number.isSafeInteger(envelope.id) && envelope.id < 2 ** 47) thin.ack(envelope);
      // A thin socket never folds log/changes/info into canonical Ui.
    } catch (error) { diagnostic.note("error", { cause: "decode-error", errorKind: error?.name });disconnect("decode-error");try { s.close(); } catch {} }
  };
  // Capture the actual CloseEvent even after a local replacement retired it.
  s.onclose = event => { diagnostic.closed(event);disconnect(event); };
  s.onerror = event => { diagnostic.note("error", { cause: "transport-error" });disconnect(event); };
  s.thin = true;
  s.dead = reason => { disconnect(reason || "heartbeat-expired");try { s.close(); } catch {} };
  // Include the HTTP Upgrade/connect phase in the greeting deadline.
  thinGreetingTimer = setTimeout(() => { disconnect("greeting-timeout");try { s.close(); } catch {} }, 4000);
  thinExpiry = setInterval(() => { if (current()) thin.expire(); }, 250);
}

// when a frame (a heartbeat included) last arrived, in ms
let heard = BigInt(Date.now()), tick = Date.now(), hiddenAt = 0;
setInterval(() => {
  const t = Date.now(), slept = t - tick > 30000;tick = t;
  if (socket && (slept || !socket.thin || socket.hb) && App.alive_due(heard, BigInt(t))) socket.dead("heartbeat-expired");
}, 5000);
document.addEventListener("visibilitychange", () => { if (document.visibilityState === "hidden") hiddenAt = Date.now(); }, true);
watchWake(window, document, why => {
  const away = hiddenAt ? Date.now() - hiddenAt : 0;
  if (why === "visible") hiddenAt = 0;
  // the network changed, or the page slept a while: a socket may be dead
  // without a close, so a quiet one is replaced (cheap: a bounded view)
  if (socket && (why === "online" || (why === "visible" && away > 60000) || why === "pageshow")) {
    // Heartbeats are 15 seconds apart. A healthy idle socket is often quiet
    // for more than 3 seconds; use the same 40-second lease as the timer.
    if (App.alive_due(heard, BigInt(Date.now()))) socket.dead("wake-expired");
  }
  if (thinRetry.pending) { thinBackoff.reset();thinRetry.wake(); }
});

function clockTick() {
  ui = App.tick(ui, now());
  if (viewMode !== "compatibility") {
    if (!document.hidden && remoteReady) thin?.update();
    return;
  }
  later();
}
setInterval(clockTick, 30000);
document.addEventListener("visibilitychange", () => { if (!document.hidden) clockTick(); });

render();
pairingReady.then(paired => {
  pairingAccepted = paired === true;
  if (!pairingAccepted) {
    viewMode = "read-only";focusedView.invalidate();showViewStatus("Pairing failed. Reopen the pairing link.");return;
  }
  // Neither IO path awaits the other: a blocked IndexedDB request cannot
  // delay capability discovery or the first selected network view.
  void restoreFocused();
  void refreshFocusedIdentity();
  startThin();
});
window.addEventListener("pagehide", () => { thinSocket?.connectionDiagnostic?.note("pagehide", { cause: updating ? "deploy" : "pagehide" });checkpointFocused(); });

// A deploy: the hub serves a new web client. An open page reloads into it at
// a quiet moment (drafts live in localStorage), so it never runs stale.
const RUNNING = bundleOf(import.meta.url);
const RUNNING_STYLES = [...document.querySelectorAll('link[rel="stylesheet"][href]')].map(link => styleOf(link.href)).filter(Boolean);
let lastKey = 0, updating = false;
document.addEventListener("keydown", () => { lastKey = Date.now(); }, true);
async function checkDeploy() {
  if (!RUNNING || updating) return;
  let html = "";
  try { const r = await fetch("/", { cache: "no-store", credentials: "same-origin" }); if (r.ok) html = await r.text(); } catch { return; }
  if (!deployed(RUNNING, html, RUNNING_STYLES)) return;
  updating = true;trace("deploy", { running: RUNNING });
  const quiet = () => {
    const busy = document.getElementById("thin-result") || document.querySelector(".shade, .picker, [aria-modal=true]");
    if (document.visibilityState === "hidden" || (Date.now() - lastKey > 3000 && !busy)) {
      try { checkpointFocused(); } catch {}
      thinSocket?.connectionDiagnostic?.initiate("deploy");
      connectionLog.record("deploy-reload", { cause: "deploy" });
      location.reload();
    } else setTimeout(quiet, 1000);
  };
  quiet();
}

// the app shell works offline where the browser allows it (HTTPS or localhost)
if ("serviceWorker" in navigator && window.isSecureContext) {
  navigator.serviceWorker.register("/sw.js").catch(() => {});
}

// A category change starts at its heading; Back restores its category button.
let settingsPage = null;
function settingsSync() {
  const content = document.getElementById("settings-content");
  if (!content) { settingsPage = null; return; }
  const page = content.dataset.settingsPage;
  if (page === settingsPage) return;
  const previous = settingsPage;
  settingsPage = page;
  content.scrollTop = 0;
  const target = page ? document.getElementById("settings-title") :
    content.querySelector(`[data-value="${CSS.escape(previous || "appearance")}"]`) || content.querySelector("button");
  target?.focus();
}
document.addEventListener("keydown", e => {
  if (e.key !== "Escape" || !document.getElementById("settings-content")) return;
  if (e.target.matches("input, textarea, select")) { e.target.blur(); }
  else { dispatch(settingsPage ? "settings-page" : "flag", settingsPage ? "" : "settings"); }
  e.preventDefault(); e.stopImmediatePropagation();
}, true);
