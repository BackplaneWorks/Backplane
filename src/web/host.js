// Backplane browser host.
//
// This file only moves data: JSON <-> Bend values, a keyed DOM patch of the
// Node tree app.bend's view returns, event delegation back to app.bend's
// act, and the socket. No product decision is made here (AGENTS.md: the UI
// is dumb).

import App from "./app.bend";
import { pairBrowser } from "./auth.js";

// Consume the secret before the generic deeplink handler clears the fragment.
// a hub that is starting (building its store) answers 503: pairing waits and
// says so, and the link's token is kept until it pairs
const pairingReady = pairBrowser({ location, history, storage: localStorage, fetch: window.fetch.bind(window),
  onWait: note => { try { showViewStatus(note); } catch {} } }).catch(() => false);
import * as Solid from "./solid.js";
import * as Plot2d from "./plot2d.js";
import { installDesktopBrowser } from "./desktop-browser.js";
import { restoreCache } from "./replay.js";
import { FocusedCache, FocusedView, captureFocused, cacheIdentity, mergeFocused, scopeKey } from "./focused-cache.js";
import { refreshCacheIdentity } from "./cache-identity.js";
import { discoverViews } from "./thin-discovery.js";
import { Backoff, Retry, watchWake, probeHub, bundleOf, deployed, QUIET_MS } from "./reconnect.js";
import { ThinHost, preservePresentationRoute, presentationTarget } from "./thin-host.js";
import { ThinContinuations } from "./thin-continuations.js";
import { ThinMailDraft, mailReplyField } from "./mail-drafts.js";
import { ThinViews, viewCapability } from "./thin.js";
import * as Wire from "./wire.js";
import { openDraw } from "./draw.js";
import { T, is, learnJson, learnDom } from "./wire.js";

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

const EVENTS = ["click", "input", "contextmenu"];

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
        if (el.value !== value) el.value = value;
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
  if (MEMO.size < 2000) return;
  for (const [id, m] of MEMO) if (memoGen - m.gen > 200) MEMO.delete(id);
}

function same(a, b) {
  return a.$ === b.$ && (is(a, "Txt") || (a.tag === b.tag && a.key === b.key));
}

// make el (whose last vnode is el.__v) match v
function patch(el, v) {
  const old = el.__v;
  setAttrs(el, old.attrs, v.attrs);
  const kids = [];
  for (const k of each(v.kids)) kids.push(expand(k));
  const prev = el.__kids;
  const byKey = new Map();
  const free = [];
  for (const p of prev) {
    if (is(p.v, "El") && p.v.key) byKey.set(p.v.tag + "\u0000" + p.v.key, p);
    else free.push(p);
  }
  const next = [];
  let f = 0;
  for (let i = 0; i < kids.length; i += 1) {
    const k = kids[i];
    let hit = null;
    if (is(k, "El") && k.key) {
      hit = byKey.get(k.tag + "\u0000" + k.key) ?? null;
      if (hit) byKey.delete(k.tag + "\u0000" + k.key);
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
try {
  const selected = localStorage.getItem("backplane-selected");
  if (selected) ui = App.presentation_select(ui, selected);
} catch {}
let socket = null;
const thinContinuations = new ThinContinuations();
let frontend = null, frontendNode = null, wantFrontend = true, fallbackIntent = null;
let viewMode = "discovering", remoteReady = false, remoteEpoch = "", thinSession = null;
let pairingAccepted = false;
let focusedNode = null, focusedAtEnd = true, focusedSaveTimer = 0;
let focusedRequestedKey = null;
let thin = null, thinSocket = null, thinAttempt = 0, thinExpiry = 0, thinGreetingTimer = 0, thinDraftTimer = 0, thinFieldTimer = 0;
const focusedStore = new FocusedCache({ indexedDB, report: (state, reason) => trace("focused-cache", { state, reason }) });
function currentSession() { return thinSession || frontend?.session || fallbackIntent?.session || Wire.plain(App.presentation(ui)); }
function focusedScope() {
  return { origin: location.origin, cid, identity: cacheIdentity(document.cookie), bundle: import.meta.url,
    language: App.lang(ui), selected: currentSession().sel };
}
const focusedView = new FocusedView({
  cache: focusedStore, scope: focusedScope,
  hide: () => { focusedNode = null; later(); },
  view: record => {
    if (canonicalBoot || remoteReady) return;
    const shell = Wire.plain(App.presentation_view(App.presentation_wait(ui)));
    focusedNode = App.presentation_node(presentationJson(mergeFocused(shell, record)));
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
  if (!main || !timeline || !title || !app) return;
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
    stopThin();viewMode = "read-only";focusedView.invalidate();
    showViewStatus("Pairing is required to reconnect.");later();return;
  }
  if (before !== cacheIdentity(document.cookie) || result.state === "denied" || result.state === "invalid") {
    focusedView.invalidate();
    if (!canonicalBoot && !remoteReady) void restoreFocused();
    else saveFocusedSoon();
  }
}
let cacheReady = false;
let queued = false;
let scroll = false;
let focus = null;
let jump = null;
let earlierAsked = false;

// the entry at the top of the timeline's view, and where it is in the
// window (so a header or panel above the timeline growing moves it not)
function anchorOf(tl) {
  const top = tl.getBoundingClientRect().top;
  for (const el of tl.querySelectorAll("[data-id]")) {
    const r = el.getBoundingClientRect();
    if (r.bottom > top + 1) return { id: el.getAttribute("data-id"), off: r.top };
  }
  return null;
}

// Reading while the thread grows (client.bend's Hold): only a timeline at
// its end (App.hold_at_end: within a few px) follows. Scrolled up, the
// entry at the view's top and its offset are held, and whatever changes
// size (a render, the streaming row, an image loading, the subagents or
// todo panels, the asks and composer below) is made good before the page
// paints, so nothing read moves. Leaving the end and coming back are told
// to the client ("hold-away"/"hold-end": the "N new" pill).
let held = null;
let away = "";
// where the last of our own corrections left the timeline: its scroll
// event is ours, not the person's, and keeps the offset held (no drift)
let fixedAt = -1;
let heldSize = null;
function holdSize(tl) {
  return { tl, height: tl.scrollHeight, view: tl.clientHeight, width: tl.clientWidth };
}
function holdEnd(tl) {
  return App.hold_at_end(BigInt(Math.max(0, Math.round(tl.scrollHeight - tl.scrollTop - tl.clientHeight))));
}
function holdFix(tl) {
  if (!tl || tl.id !== "timeline") return;
  heldSize = holdSize(tl);
  if (!held) {
    const end = tl.scrollHeight - tl.clientHeight;
    if (tl.scrollTop < end) { tl.scrollTop = end; fixedAt = tl.scrollTop; }
    return;
  }
  const el = tl.querySelector(`[data-id="${CSS.escape(held.id)}"]`);
  if (!el) return;
  const d = el.getBoundingClientRect().top - held.off;
  if (d) { tl.scrollTop += d; fixedAt = tl.scrollTop; }
}
// tell the client once each way; the thread is the one the view was away in
function holdSay(now) {
  if (focusedNode && !frontendNode) return;
  if (now === away) return;
  const was = away;
  away = now;
  queueMicrotask(() => dispatch(now ? "hold-away" : "hold-end", now || was));
}
// the person scrolled the timeline: the end follows, elsewhere holds
function holdRead(tl) {
  if (focusedNode && !frontendNode) return;
  if (!tl || tl.id !== "timeline") return;
  if (tl.scrollTop === fixedAt) return;
  // Shrinking content can clamp scrollTop and queue a scroll event before
  // ResizeObserver runs. It belongs to the changed layout, so repair the
  // original anchor before deciding whether the reader reached the end.
  const size = holdSize(tl);
  if (held && heldSize?.tl === tl && (size.height !== heldSize.height || size.view !== heldSize.view || size.width !== heldSize.width)) {
    holdFix(tl);
    return;
  }
  heldSize = size;
  fixedAt = -1;
  held = holdEnd(tl) ? null : anchorOf(tl);
  holdSay(held ? App.sel(ui) : "");
}
const holdSizes = typeof ResizeObserver === "function" ? new ResizeObserver(() => holdFix(document.getElementById("timeline"))) : null;
function holdWatch(tl) {
  if (!holdSizes) return;
  holdSizes.disconnect();
  if (!tl) return;
  holdSizes.observe(tl);
  for (const c of tl.children) holdSizes.observe(c);
}
document.addEventListener("scroll", (e) => holdRead(e.target), true);

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
function render() {
  mailCheckpoint();
  const mail = mailAnchor();
  queued = false;
  const tl = document.getElementById("timeline");
  const t0 = PERF ? pnow() : 0;
  const page = frontendNode ? thinContinuations.project(App.presentation_overlay(frontendNode, presentationJson(currentSession())), expand)
    : focusedNode || App.view(canonicalBoot ? ui : App.presentation_wait(ui));
  const t1 = PERF ? pnow() : 0;
  patch(root, { $: T.El, tag: "div", key: "", attrs: { $: "Nil" }, kids: { $: "Con", head: page, tail: { $: "Nil" } } });
  if (PERF) PERF.render.push([t1 - t0, pnow() - t1]);
  if (canonicalBoot) {
    document.documentElement.dataset.backplaneReady = "true";
    document.documentElement.dataset.backplaneSeq = App.seq(ui);
  }
  document.documentElement.dataset.backplaneMode = viewMode;
  document.documentElement.dataset.backplaneRemoteReady = String(remoteReady);
  document.documentElement.dataset.backplaneCached = String(!!focusedNode && !frontendNode);
  trace("dom", { seq: App.seq(ui) });
  if (PERF?.trace) requestAnimationFrame(() => trace("paint", { seq: App.seq(ui) }));
  mailRestore(mail);
  desktopBrowser.sync();
  memoSweep();
  const tl2 = document.getElementById("timeline");
  if (focusedNode && !frontendNode && tl !== tl2 && focusedAtEnd && tl2) tl2.scrollTop = tl2.scrollHeight;
  // another timeline (or a send, a new thread: ScrollEnd) starts at its end
  if (scroll || tl !== tl2) {
    held = null;
    holdSay("");
  }
  holdFix(tl2);
  holdWatch(tl2);
  scroll = false;
  // an entry the history went back to (Jump): brought into view, flashed
  if (jump && tl2) {
    const el = tl2.querySelector(`[data-id="${CSS.escape(jump)}"]`);
    if (el) {
      el.scrollIntoView({ block: "center" });
      el.classList.remove("jumped");
      void el.offsetWidth;
      el.classList.add("jumped");
    }
    jump = null;
  }
  Solid.mount(document.getElementById("solid"));
  Plot2d.mount(document.getElementById("plot"));
  earlierAsked = false;
  // a page of entries the timeline does not draw (todo steps, a fold's
  // calls) moves nothing and, at the very top, no scroll comes to ask for
  // the next: still near the top after a render, it is asked for
  if (tl2 && tl2.querySelector("[data-earlier]") && tl2.scrollTop <= tl2.clientHeight * 2) requestAnimationFrame(() => earlierCheck(document.getElementById("timeline")));
  if (focus) {
    document.getElementById(focus)?.focus();
    focus = null;
  }
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
    const a = document.getElementById("asks");
    if (a && a.__v) patch(a, App.asks(ui));
    if (PERF) PERF.part.push(pnow() - t0);
    trace("dom-part", { seq: App.seq(ui) });
    if (PERF?.trace) requestAnimationFrame(() => trace("paint-part", { seq: App.seq(ui) }));
  });
}

// A delta changes only the shown thread's streaming row (app.bend's part
// says "live"): that row alone is redrawn in place (app.bend's live, keyed
// as the page keys it), the page when it has no such row.
let living = false;
function laterLive() {
  if (queued || living) return;
  living = true;
  requestAnimationFrame(() => {
    living = false;
    if (queued) return;
    const tl = document.getElementById("timeline");
    const t0 = PERF ? pnow() : 0;
    const v = expand(App.live(ui));
    const hit = tl && tl.__kids && is(v, "El") ? tl.__kids.find((p) => is(p.v, "El") && p.v.key === v.key && p.v.tag === v.tag) : null;
    if (!hit) { render(); return; }
    // away, the pill comes with the first of it (a page render, once)
    if (held && !tl.querySelector(".hold-pill")) { render(); return; }
    patch(hit.node, v);
    hit.v = v;
    holdFix(tl);
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
      // a client action come due (the history playing)
      setTimeout(() => dispatch(c.action, c.value, true), Number(c.ms));
    } else if (is(c, "Jump")) {
      jump = c.id; // after the next render, which opens its folds
      later();
    } else if (is(c, "Keep")) {
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
function dispatch(action, value, timer) {
  if (action === "thin-search-open") {
    if (viewMode === "view-v1" && thin?.ready) showThinSearch({ query: "", rows: [], cursor: "", hasMore: false });
    else showViewStatus("Search requires the current conversation connection.");
    return true;
  }
  if (viewMode !== "compatibility") {
    if (action === "select" || action === "draft") {
      if (thin) return thin.request(action, value);
      thinSession = choosePresentation(currentSession(), action, value);
      localPresentation(action, value, thinSession);
      return true;
    }
    if (thin?.ready) {
      if (action === "earlier" || action === "view-page") return thin.page();
      if (action === "thin-search") return thin.search(value);
      if (action === "thin-message-more") {
        try { const input = JSON.parse(value);return thin.message(input.entryId, input.cursor); } catch { return true; }
      }
      if (action === "thin-cancel") return thin.cancel(Number(value));
      if (action === "mail-send" && thinMailDraft.needsSubmit()) {
        const field = document.getElementById("mail-reply");
        if (!field || !thin.request("bfield", "mrreply\u001f" + field.value)) return true;
      }
      if (action === "hold-away" || action === "hold-end") return true;
      return thin.request(action, value);
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
async function upload(action, file, scope = uploadScope()) {
  const key = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
  const name = file.name || "pasted." + ((file.type || "").split("/")[1] || "bin");
  if (file.size === 0) return;
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
const KEYS_PAGE = App.keys_page().split(",");
const KEYS_TERM = App.keys_term().split(",");

document.addEventListener("keydown", (e) => {
  if (e.isComposing || e.keyCode === 229) return;
  const field = e.target.closest?.("[data-keys]");
  const combo = (e.ctrlKey ? "ctrl+" : "") + (e.altKey ? "alt+" : "") + (e.shiftKey ? "shift+" : "") + e.key.toLowerCase();
  if ((field ? KEYS_TERM : KEYS_PAGE).includes(combo)) {
    e.preventDefault();
    dispatch("key", combo + "\t" + termSize());
    return;
  }
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
  e.preventDefault();
  // ctrl/cmd+Enter: the element's other action (send with the other follow-up mode)
  const alt = (e.ctrlKey || e.metaKey) && el.getAttribute("data-enter-alt");
  dispatch(alt || el.getAttribute("data-enter"), valueOf(el));
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
  lb.el.remove();
  lb = null;
  desktopBrowser.sync();
}

function lbPinch() {
  const [a, b] = [...lb.pointers.values()];
  return { d: Math.hypot(a.x - b.x, a.y - b.y), x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

function lbOpen(url) {
  lbClose();
  const el = create(App.lb_node_in(App.lang(ui), url));
  const img = el.querySelector("img");
  lb = { el, img, v: App.lb_open(), drawing: false, pointers: new Map(), pinch: null };
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
    if (e.target.closest("[data-lb-close]")) return;
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
  document.activeElement?.blur?.();
  if (img.complete) lbDraw();
}

// a viewer's Fit button
document.addEventListener("click", (e) => {
  if (!e.target.closest?.('[data-view="fit"]')) return;
  Plot2d.fit();
  Solid.fit();
});
// the timeline scrolled within two screens of its top while earlier
// entries are left out (view.bend's mark): ask for them, once a render
function earlierCheck(tl) {
  if (viewMode !== "compatibility" && (!olderGestureUntil || performance.now() > olderGestureUntil)) return;
  if (earlierAsked || !(tl instanceof Element) || tl.id !== "timeline") return;
  const mark = tl.querySelector("[data-earlier]");
  if (!mark || tl.scrollTop > tl.clientHeight * 2) return;
  earlierAsked = true;
  olderGestureUntil = 0;
  dispatch("earlier", mark.getAttribute("data-earlier") ?? "");
}
document.addEventListener("scroll", (e) => earlierCheck(e.target), true);
let olderGestureUntil = 0, olderTouchY = null;
function noteOlderGesture(e) {
  if (!e.isTrusted) return;
  const tl = e.target.closest?.("#timeline");
  if (!tl && !(e.type === "keydown" && e.target === document.body)) return;
  let older = false;
  if (e.type === "wheel") older = e.deltaY < 0 && !e.ctrlKey;
  else if (e.type === "touchstart") olderTouchY = e.touches[0]?.clientY ?? null;
  else if (e.type === "touchmove") {
    const y = e.touches[0]?.clientY;
    older = typeof y === "number" && typeof olderTouchY === "number" && y > olderTouchY;
    olderTouchY = y ?? null;
  } else if (e.type === "pointerdown") {
    older = e.button === 0 && tl.scrollHeight > tl.clientHeight && e.clientX >= tl.getBoundingClientRect().right - 16;
  } else if (e.type === "keydown") older = ["PageUp", "Home", "ArrowUp"].includes(e.key) && !e.target.isContentEditable;
  if (older) olderGestureUntil = performance.now() + 1200;
}
for (const event of ["wheel", "touchstart", "touchmove", "pointerdown", "keydown"]) document.addEventListener(event, noteOlderGesture, { capture: true, passive: true });

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
  if (App.lb_key(e.key)) {
    e.preventDefault();
    lbClose();
  }
}, true);

window.addEventListener("resize", () => lbDraw());

// The viewer's width: its left edge (view.bend's View.split) drags it, the
// width is kept in this browser, and a double-click on the edge gives the
// pane its usual share back. The thread keeps at least 360 px.
const VIEWER_W = "backplane-viewer-w";
function viewerWidth(w) {
  const root = document.documentElement;
  if (w == null) { root.style.removeProperty("--viewer-w"); return; }
  root.style.setProperty("--viewer-w", `${Math.round(w)}px`);
}
function viewerClamp(w) {
  const app = document.querySelector(".app");
  const side = app?.querySelector(".sidebar")?.getBoundingClientRect().width ?? 264;
  const max = (app?.getBoundingClientRect().width ?? window.innerWidth) - side - 360;
  return Math.max(320, Math.min(w, max));
}
try { const w = Number(localStorage.getItem(VIEWER_W)); if (w > 0) viewerWidth(viewerClamp(w)); } catch {}
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
  const move = (m) => { w = viewerClamp(right - m.clientX); viewerWidth(w); };
  const done = () => {
    edge.removeEventListener("pointermove", move);
    edge.removeEventListener("pointerup", done);
    edge.removeEventListener("pointercancel", done);
    edge.classList.remove("on");
    document.body.classList.remove("resizing");
    try { localStorage.setItem(VIEWER_W, String(Math.round(w))); } catch {}
  };
  edge.addEventListener("pointermove", move);
  edge.addEventListener("pointerup", done);
  edge.addEventListener("pointercancel", done);
});
document.addEventListener("dblclick", (e) => {
  if (!(e.target instanceof Element) || !e.target.closest(".viewer-split")) return;
  viewerWidth(null);
  try { localStorage.removeItem(VIEWER_W); } catch {}
});
window.addEventListener("resize", () => {
  const w = parseFloat(document.documentElement.style.getPropertyValue("--viewer-w"));
  if (w > 0) viewerWidth(viewerClamp(w));
});

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
    frontendNode = null;
    remoteReady = false;
    void restoreFocused();
    later();
  } else if (action === "draft") {
    ui = App.restore(ui, session.sel, value);
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
  let box = document.getElementById("view-connection");
  if (!box) {
    box = document.createElement("aside");box.id = "view-connection";box.setAttribute("role", "status");
    box.style.cssText = "position:fixed;bottom:0;left:0;right:0;z-index:30;display:flex;flex-wrap:wrap;gap:8px;padding:8px;background:var(--panel);border-top:1px solid var(--line)";
    const text = document.createElement("span");text.id = "view-connection-text";box.append(text);
    for (const [label, action] of [["Retry now", retryNow]]) {
      const button = document.createElement("button");button.type = "button";button.textContent = label;button.addEventListener("click", action);box.append(button);
    }
    document.body.append(box);
  }
  box.hidden = !message;
  box.style.display = message ? "flex" : "none";
  document.getElementById("view-connection-text").textContent = message;
  // The page shrinks above the bar instead of sliding under it.
  document.documentElement.style.setProperty("--status-h", message ? box.offsetHeight + "px" : "0px");
}

// Reconnecting
// ------------
// A dropped view connection is tried again by itself: a jittered, growing
// delay (reconnect.js), cut short when the network comes back or the page is
// looked at again; a hub that is starting says how long with Retry-After.
// While it is down the last view stays on screen (actions wait, the draft is
// kept) and the bar says so only if the gap lasts past QUIET_MS.
const thinBackoff = new Backoff(), thinRetry = new Retry();
let downSince = 0, downNote = "", downTimer = 0;
function downStatus(note) {
  downNote = note;
  if (!downSince) downSince = Date.now();
  clearTimeout(downTimer);
  const wait = Math.max(0, downSince + QUIET_MS - Date.now());
  downTimer = setTimeout(() => { if (downSince) showViewStatus(waiting(downNote)); }, wait);
}
function upStatus() {
  downSince = 0;clearTimeout(downTimer);downTimer = 0;
  thinBackoff.reset();thinRetry.cancel();
  showViewStatus("");
}
function thinReconnect() {
  if (!pairingAccepted || viewMode === "compatibility" || thinRetry.pending) return;
  downStatus(downNote || "Reconnecting… Your draft is kept.");
  thinRetry.schedule(thinBackoff.next(), async () => {
    if (viewMode === "compatibility") return;
    const hub = await probeHub(window.fetch.bind(window));
    if (viewMode === "compatibility") return;
    if (hub.state === "up") { void startThin(true);return; }
    if (hub.state === "starting") { thinBackoff.retryAfter(hub.retryAfterMs);downStatus(`${hub.note || "Hub restarting…"} Your draft is kept.`); }
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
  else void startThin();
}

function resultPanel(title) {
  document.getElementById("thin-result")?.remove();
  const shade = document.createElement("div");shade.id = "thin-result";shade.className = "shade";
  const panel = document.createElement("section");panel.className = "picker";
  panel.setAttribute("role", "dialog");panel.setAttribute("aria-modal", "true");panel.setAttribute("aria-label", title);
  const before = document.activeElement;
  const close = () => { shade.remove();root.inert = false;before?.focus?.();desktopBrowser.sync(); };
  const heading = document.createElement("div");heading.className = "tools";
  const label = document.createElement("span");label.textContent = title;heading.append(label);
  const button = document.createElement("button");button.type = "button";button.textContent = "×";button.setAttribute("aria-label", "Close");button.addEventListener("click", close);heading.append(button);
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
  button.focus();desktopBrowser.sync();return { panel, close };
}
function showThinSearch(search) {
  const { panel, close } = resultPanel("Search");
  const query = document.createElement("input");query.type = "search";query.value = search.query;query.setAttribute("aria-label", "Search conversations");
  query.addEventListener("keydown", e => {
    if (e.key === "Enter" && !e.isComposing) { e.preventDefault();thin?.search(query.value); }
  });panel.append(query);
  const rows = document.createElement("div");rows.style.cssText = "overflow:auto;min-height:0;display:flex;flex-direction:column";
  for (const row of search.rows) {
    const button = document.createElement("button");button.type = "button";button.className = "row";
    button.textContent = row.text + (row.partial ? " …" : "");
    button.addEventListener("click", () => { close();dispatch("select", row.thread); });rows.append(button);
  }
  panel.append(rows);
  if (search.hasMore && search.cursor) {
    const more = document.createElement("button");more.type = "button";more.textContent = "More results";
    more.addEventListener("click", () => thin?.search(search.query, search.cursor));panel.append(more);
  }
  query.focus();
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
function stopThin(keepView = false) {
  checkpointFocused();
  thinRetry.cancel();
  thinContinuations.clear();
  thinAttempt++;clearInterval(thinExpiry);clearTimeout(thinGreetingTimer);clearTimeout(thinDraftTimer);clearTimeout(thinFieldTimer);
  const previous = thin;thin = null;
  const s = thinSocket;thinSocket = null;
  if (socket === s) socket = null;
  if (s) { s.onclose = null;s.onmessage = null;try { s.close(); } catch {} }
  if (previous) previous.disconnect();
  remoteReady = false;desktopBrowser.hide();Plot2d.reset();Solid.reset();
  if (!keepView) frontendNode = null;
}
async function startThin(again = false) {
  if (!pairingAccepted) return;
  if (viewMode === "compatibility") return;
  stopThin(again || downSince > 0);const attempt = thinAttempt;
  viewMode = "discovering";
  if (downSince) downStatus(downNote);
  else showViewStatus("Connecting…");
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
  let ended = false;
  const current = () => !ended && attempt === thinAttempt && viewMode === "view-v1";
  const fail = message => {
    if (!current()) return;
    checkpointFocused();remoteReady = false;frontendNode = null;desktopBrowser.hide();Plot2d.reset();Solid.reset();
    showViewStatus(message);void restoreFocused(true);later();
  };
  const disconnect = () => {
    if (!current()) return;
    ended = true;
    checkpointFocused();remoteReady = false;desktopBrowser.hide();Plot2d.reset();Solid.reset();
    clearTimeout(thinGreetingTimer);clearInterval(thinExpiry);clearTimeout(thinDraftTimer);clearTimeout(thinFieldTimer);
    thin?.disconnect();if (socket === s) socket = null;
    if (!frontendNode) void restoreFocused(true);
    later();thinReconnect();
  };
  try {
    const saved = localStorage.getItem("backplane-view-request-id");
    const next = saved === null ? 2 ** 47 : /^(0|[1-9][0-9]*)$/.test(saved) ? Number(saved) : NaN;
    if (!Number.isSafeInteger(next) || next < 2 ** 47 || next >= 2 ** 48 - 1) throw Error("Invalid view counter");
    thin = new ThinHost({
      hub: location.origin, cid, now: () => performance.now(), session: currentSession,
      target: presentationTarget,
      counter: value => localStorage.setItem("backplane-view-request-id", String(value)),
      send: request => {
        if (!current() || s.readyState !== 1) throw Error("View connection closed");
        trace("send", { id: request.id, method: request.m });
        s.send(fromList(App.wire_out(JSON.stringify(request))));
      },
      choice: choosePresentation, prepare: (session, input, action) => preservePresentationRoute(
        action === "send" || action === "send-alt" ? input : session,
        Wire.plain(App.presentation_prepare(presentationJson(session), presentationJson(input), action))),
      selection: (target, session) => {
        thinMailDraft.clear();thinContinuations.clear();
        if (target.kind === "thread") { ui = App.presentation_select(ui, target.id);void restoreFocused(); }
        else { focusedRequestedKey = null;focusedView.invalidate(); }
      },
      local: localPresentation,
      sessionChanged: (session, detail) => {
        thinSession = session;
        if (detail?.remote) {
          // Only live response choices cross this boundary. State and applied
          // cursor stay in the fresh unsynced recipient, never in a picture.
          ui = App.unsync(App.presentation_handoff(ui, presentationJson(session)));
          document.documentElement.lang = App.lang(ui);
        }
        later();
      },
      ack: (session, reply) => preservePresentationRoute(session, Wire.plain(App.presentation_reply(presentationJson(session), presentationJson(reply)))),
      decode: shown => App.presentation_node(presentationJson(shown)),
      command: command => presentationCommand(command),
      anchor: () => { const tl = document.getElementById("timeline"), a = tl && anchorOf(tl);return { id: a?.id || "", offset: Math.max(0, Math.round(a ? tl.getBoundingClientRect().top - a.off : 0)) }; },
      draftRefresh: refresh => { clearTimeout(thinDraftTimer);thinDraftTimer = setTimeout(() => { if (current()) refresh(); }, 75); },
      fieldRefresh: refresh => { clearTimeout(thinFieldTimer);thinFieldTimer = setTimeout(() => { if (current()) refresh(); }, 75); },
      fieldsChanged: (name, text) => { if (name === "mrreply") thinMailDraft.edit(text);later(); },
      operationScope: (action, value) => action === "mail-send" ? thinMailDraft.scope(true) : action === "bfield" && value.startsWith("mrreply\u001f") ? thinMailDraft.scope() : undefined,
      blocked: () => showViewStatus("Wait for the current conversation to connect."),
      warning: message => showViewStatus(message),
      search: showThinSearch, message: showThinMessage,
      view: (node, detail) => {
        if (!current()) return;
        const field = mailReplyField(detail.shown);
        const accepted = field === undefined ? undefined : thinMailDraft.matches(detail.mailDraftContext, detail.epoch) ? thin.fieldValue("mrreply", field) : field;
        thinMailDraft.bind(detail.mailDraftContext, detail.epoch, accepted);
        thinContinuations.bind(location.origin, detail);
        focusedView.remote(detail.epoch);remoteEpoch = detail.epoch;frontendNode = node;
        trace("thin-view", { entries: detail.window?.ids?.length || 0 });later();
      },
      state: state => {
        if (!current()) return;
        remoteReady = state.remoteReady === true;
        if (!remoteReady) { frontendNode = null;desktopBrowser.hide(); }
        if (state.phase === "error") fail("Conversation unavailable. Your draft is kept; retry explicitly.");
        else if (remoteReady && !s.up) { s.up = true;upStatus();outboxFlush();void checkDeploy(); }
        else if (remoteReady) showViewStatus("");
        later();
      },
    }, ThinViews, next);
  } catch { disconnect();try { s.close(); } catch {}return; }
  s.onopen = () => {
    if (!current()) return;
    socket = s;heard = BigInt(Date.now());s.hb = false;
  };
  s.onmessage = event => {
    if (!current() || typeof event.data === "string") return;
    heard = BigInt(Date.now());
    const bytes = new Uint8Array(event.data);
    if (Solid.isPlot(bytes)) {
      const o = Solid.cbor(bytes), at = { bytes: bytes.length, t: performance.now() };
      const key = typeof o?.key === "string" ? o.key : "";
      if (["2|", "3|", "4|"].some(prefix => key.startsWith(prefix))) Solid.got(o, at);
      else if (o) Plot2d.got(o, at);
      return;
    }
    if (bytes.byteLength > 262144) { disconnect();try { s.close(); } catch {}return; }
    try {
      const j = Wire.cbor(bytes, KEYS, WORDS), envelope = Wire.plain(j);
      if (envelope.t === "hb") { s.hb = true;return; }
      if (envelope.t === "reply") outboxReply(envelope);
      if (envelope.t === "view") { clearTimeout(thinGreetingTimer);thin.greeting(envelope); }
      else if (!thin.receive(envelope) && envelope.t === "reply" && Number.isSafeInteger(envelope.id) && envelope.id < 2 ** 47) thin.ack(envelope);
      // A thin socket never folds log/changes/info into canonical Ui.
    } catch { disconnect();try { s.close(); } catch {} }
  };
  s.onclose = disconnect;s.onerror = disconnect;
  s.thin = true;
  s.dead = () => { disconnect();try { s.close(); } catch {} };
  // Include the HTTP Upgrade/connect phase in the greeting deadline.
  thinGreetingTimer = setTimeout(() => { disconnect();try { s.close(); } catch {} }, 4000);
  thinExpiry = setInterval(() => { if (current()) thin.expire(); }, 250);
}

// when a frame (a heartbeat included) last arrived, in ms
let heard = BigInt(Date.now()), tick = Date.now(), hiddenAt = 0;
setInterval(() => {
  const t = Date.now(), slept = t - tick > 30000;tick = t;
  if (socket && (slept || ((!socket.thin || socket.hb) && App.alive_due(heard, BigInt(t))))) socket.dead();
}, 5000);
document.addEventListener("visibilitychange", () => { if (document.visibilityState === "hidden") hiddenAt = Date.now(); }, true);
watchWake(window, document, why => {
  const away = hiddenAt ? Date.now() - hiddenAt : 0;
  if (why === "visible") hiddenAt = 0;
  // the network changed, or the page slept a while: a socket may be dead
  // without a close, so a quiet one is replaced (cheap: a bounded view)
  if (socket && (why === "online" || (why === "visible" && away > 60000) || why === "pageshow")) {
    if (Date.now() - Number(heard) > 3000) socket.dead();
  }
  if (thinRetry.pending) { thinBackoff.reset();thinRetry.wake(); }
});

setInterval(() => {
  ui = App.tick(ui, now());
  later();
}, 30000);

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
window.addEventListener("pagehide", checkpointFocused);

// A deploy: the hub serves a new web client. An open page reloads into it at
// a quiet moment (drafts live in localStorage), so it never runs stale.
const RUNNING = bundleOf(import.meta.url);
let lastKey = 0, updating = false;
document.addEventListener("keydown", () => { lastKey = Date.now(); }, true);
async function checkDeploy() {
  if (!RUNNING || updating) return;
  let html = "";
  try { const r = await fetch("/", { cache: "no-store", credentials: "same-origin" }); if (r.ok) html = await r.text(); } catch { return; }
  if (!deployed(RUNNING, html)) return;
  updating = true;trace("deploy", { running: RUNNING });
  const quiet = () => {
    const busy = document.getElementById("thin-result") || document.querySelector(".shade, .picker, [aria-modal=true]");
    if (document.visibilityState === "hidden" || (Date.now() - lastKey > 3000 && !busy)) {
      try { checkpointFocused(); } catch {}
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
