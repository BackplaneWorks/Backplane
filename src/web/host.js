import { installMessageImages, messageImages } from "./message-images.js";
installMessageImages();
// Backplane browser host.
//
// This file only moves data: JSON <-> Bend values, a keyed DOM patch of the
// Node tree app.bend's view returns, event delegation back to app.bend's
// act, and the socket. No product decision is made here (AGENTS.md: the UI
// is dumb).

import App from "./app.bend";
import { phoneForm, phoneRequest, installPhoneShell } from "./phone-shell.js";
// a phone gets the phone arrangement (phone-shell.js, view.bend's View.phone)
const PHONE = phoneForm();
const phoneShell = installPhoneShell({ on: PHONE, dispatch: (action, value) => dispatch(action, value) });
import { SecretSaveHost } from "./secret-save.js";
import { starAttention } from "./star-attention.js";
const starAttentionSync = starAttention();
import { sizeComposer, sizeComposerControls } from "./composer-size.js";
import { applyComposerDraft } from "./composer-draft.js";
import { VoiceMic, hubHere } from "./voice-mic.js";
import { homeDraftStorage } from "./home-drafts.js";
const localStorage = homeDraftStorage(window.localStorage, window.backplaneDesktop?.drafts);
import { installProjectSplitters } from "./project-splitters.js";
const projectSplitters = installProjectSplitters({ storage: localStorage, scope: () => thin?.hub || location.origin });
import { editStart } from "./sent-claims.js";
import Planner from "./timeline-plan.bend";
import History from "./timeline-history.bend";
import { TimelineScroller } from "./timeline-scroller.js";
import { EntryHistory } from "./entry-history.js";
import { installImageActions } from "./image-actions.js";
import { installShotMissing, shotsMissing } from "./shot-missing.js";
import { copyText, copyButton } from "./text-copy.js";
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
import { installPhoneThread, phoneKeepsReturn, phoneClickAction } from "./phone-thread.js";
import { installImprovements } from "./improvements.js";
import { ImprovementsRpc } from "./improvements-rpc.js";
import { improvementOrigin, improvementContext } from "./improvements-context.js";
import { installSidebarKeys } from "./sidebar-keys.js";
installSidebarKeys();
import { installFloatingTerminal } from "./floating-terminal.js";
installUsageTabs();
import { installWorkspaceMenus } from "./workspace-menus.js";
import { installBoardCatalog } from "./board-catalog.js";
installWorkspaceMenus(document, () => composerKeeper.claim(true));
const boardCatalog = installBoardCatalog(document, () => currentSession().sel, (action, value) => dispatch(action, value));
import { installSubagentInspector } from "./subagent-inspector.js";
installSubagentInspector();
import { installViewerFeedback } from "./viewer-feedback.js";
const viewerFeedback = installViewerFeedback();
import { installWorkspaceHints } from "./workspace-hints.js";
// the keyboard kept in the open thread's composer (workspace-keys.js) and
// Esc's choice of what to put away (workspace.bend's WS.esc.next); the
// probe goes off its plots at once, as its chip's x does through the hub
const composerKeeper = installComposerFocus({ phone: () => composerPhone() });
installPhoneThread({ list: el => timelineList(el), latest: () => holdBottom(), newest: () => thin?.atNewest?.() !== false });
// (a key's result drawn in its own handler too, as a mouse press's is)
installWorkspaceKeys({ dispatch: (action, value, timer) => { const done = dispatch(action, value, timer);renderNow();return done; }, escNext: (term, probe, viewer) => App.esc_next(term, probe, viewer), composer: composerKeeper,
  clearProbe: () => { Plot2d.setProbe("");Solid.setProbe("");dispatch("probe", ""); } });
installWorkspaceHints();
installFloatingTerminal({ dispatch });
import { installPhoneOther, titleHits } from "./phone-other.js";
// (imported, so the bundle ships it hashed and index.html's link to it
// names that file: the home-screen icon, a notification's)
import touchIcon from "./apple-touch-icon.png";
// the home-screen app's icons (the phone's manifest names them)
import icon192 from "./icon-192.png";
import icon512 from "./icon-512.png";
import iconMask from "./icon-maskable-512.png";
const PHONE_TITLES = () => document.documentElement.classList.contains("phone");
const phoneOther = installPhoneOther({ dispatch: (action, value) => dispatch(action, value), touchIcon,
  appIcons: [{ src: icon192, size: 192 }, { src: icon512, size: 512 }, { src: iconMask, size: 512, purpose: "maskable" }] });
// inside the phone apps: their bridge (window.backplaneApp, docs/phone-app-shell.md)
import { installAppShell } from "./app-shell.js";
const appShell = installAppShell({ dispatch: (action, value) => dispatch(action, value), selected: () => currentSession().sel,
  attach: (files) => { const el = document.querySelector(".app.phone [data-attach]");if (el) void uploadFiles(el.getAttribute("data-attach"), files); } });
import { ConnectionLog } from "./connection-log.js";
import { frameSummary } from "./frame-summary.js";
import { pairBrowser } from "./auth.js";
import { installPairingUI } from "./pairing-ui.js";
import { pairingInput } from "./pairing-link.js";
let pairingCause = "";
const pairingUI = installPairingUI({document, location, fetch: window.fetch.bind(window), onPaired: () => location.assign("/")});

// Consume the secret before the generic deeplink handler clears the fragment.
let pendingInvitation = null;
try { const p = pairingInput(location.href, location.origin);if (p.kind !== "token") { pendingInvitation = p;history.replaceState(null, "", location.pathname + location.search); } } catch {}
const pairingReady = pendingInvitation ? Promise.resolve(false) : pairBrowser({ location, history, storage: localStorage, fetch: window.fetch.bind(window), report: cause => { pairingCause = cause;if (cause !== "invalid_link") appShell.pairingFailed(cause); } }).catch(() => false);
if (!document.cookie.split(";").some(c => c.trim().startsWith("bp_cache_v1=")) && !pendingInvitation) pairingUI.show();

import * as Solid from "./solid.js";
import * as ViewerTools from "./viewer-tools.js";
import { ViewerRpc } from "./viewer-rpc.js";
import * as Plot2d from "./plot2d.js";
import { installDesktopBrowser } from "./desktop-browser.js";
import { restoreCache } from "./replay.js";
import { FocusedCache, FocusedView, captureFocused, cacheIdentity, mergeFocused, scopeKey } from "./focused-cache.js";
import { refreshCacheIdentity } from "./cache-identity.js";
import { discoverViews } from "./thin-discovery.js";
import { Backoff, Retry, watchWake, wakeAction, directReconnect, probeHub, bundleOf, styleOf, deployed, QUIET_MS } from "./reconnect.js";
import { ThinHost, preservePresentationRoute, presentationTarget, refusedFallback } from "./thin-host.js";
import { compactRoute } from "./thin.js";
import { TerminalOutputPane } from "./terminal-output.js";
import { ModelFeedback } from "./model-feedback.js";
import { SubsFeedback } from "./subs-feedback.js";
import { PressFeedback } from "./press-feedback.js";
import { FoldFeedback } from "./fold-feedback.js";
import { ReviewPosition } from "./review-position.js";
const reviewPosition = new ReviewPosition(localStorage);
const modelFeedback = new ModelFeedback(() => later());
const subsFeedback = new SubsFeedback(() => later());
// (what presses opened is kept per hub, pairing, bundle and language)
const pressFeedback = new PressFeedback(() => later(), 64, { storage: localStorage,
  scope: () => { try { return JSON.stringify([location.origin, cacheIdentity(document.cookie), import.meta.url, App.lang(ui)]); } catch { return ""; } } });
const foldFeedback = new FoldFeedback(() => later());
// the session the hub's picture is drawn over: the model menu and the
// Subagents rows as clicked, ahead of the hub's answer
const feedbackSession = () => subsFeedback.project(modelFeedback.project(currentSession()));
function pendingPresentation(node) {
  const pending = thin?.pendingSends(currentSession().sel);
  return pending?.length ? App.presentation_pending(node, presentationJson(pending), App.lang(ui)) : node;
}
// The Subagents rows shown or hidden in the click's own task: their head and
// section patched with the overlay at once (a whole render waits for the
// next frame and draws everything), the page drawn whole after as usual
function subsNow() {
  const session = presentationJson(feedbackSession());
  for (const el of document.querySelectorAll("section.subs, .subs-head")) if (el.__v) patch(el, App.presentation_overlay(el.__v, session));
}
import { BodyWindows } from "./body-windows.js";
import { ThinContinuations } from "./thin-continuations.js";
import { previewTextFiles, previewMarkdownFiles } from "./text-preview.js";
import { DraftAttachments, SEND_WAIT } from "./draft-attachments.js";
import { ThinMailDraft, mailReplyField } from "./mail-drafts.js";
import { ThinViews, viewCapability } from "./thin.js";
import * as Wire from "./wire.js";
import { openDraw } from "./draw.js";
import { ViewTransition, withTheme } from "./view-transition.js";
import { RecentViews } from "./recent-views.js";
import { ViewPrefetch } from "./view-prefetch.js";
import { validatePresentation } from "./startup.js";
import { T, is, learnJson, learnDom } from "./wire.js";
import { softFocus, ASKED_TO_TYPE } from "./touch-focus.js";

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
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", getComputedStyle(document.documentElement).getPropertyValue("--bg").trim());
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

// The re-render harness (test/tools/rerender): a page whose window has
// __bpRender (set before it loads) counts what each render does. Off otherwise.
const RR = globalThis.__bpRender || null;
// (a 3D tab on the page once: solid.js makes the page's first graphics
// context ahead, when idle)
let glWarmAsked = false;
function create(v) {
  RR?.n("dom.create");
  if (is(v, "Txt")) return document.createTextNode(v.text);
  // the 3D workbench that left the page last, when the hub draws that very
  // one again (viewer-tools.js: same model, thread and board), comes back
  // with its drawn model, toolbar and graphics contexts, patched to v
  const kept = v.key === "solid-workbench" ? ViewerTools.kept(nodeAttr(v, "data-key"), nodeAttr(v, "data-scope"), nodeAttr(v, "data-board-key")) : null;
  if (kept && kept.__v?.tag === v.tag) { patch(kept, v); ViewerTools.restyle(kept); return kept; }
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

// an IME composition in the composer (its listeners are by the input
// listener below): the composer's text is not written meanwhile
let composerComposing = false;
function setAttrs(el, oldAttrs, attrs) {
  const seen = new Set();
  // Read the new attributes, not an old/recycled element's field owner.
  let field = null, catalogSource = null, catalogScope = "", serverOpen = false;
  for (const a of each(attrs)) if (is(a, "At")) {
    if (a.name === "data-bfield") field = a.value;
    if (a.name === "data-board-catalog") catalogSource = a.value;
    if (a.name === "data-scope") catalogScope = a.value;
    if (a.name === "open") serverOpen = true;
  }
  const catalogOpen = catalogSource === null ? null : boardCatalog.renderOpen(el, catalogSource, catalogScope, serverOpen);
  for (const a of each(attrs)) {
    if (is(a, "At")) {
      seen.add(a.name);
      if (a.name === "open" && catalogOpen !== null) {
        if (catalogOpen) el.setAttribute("open", "");else el.removeAttribute("open");
      } else if (a.name === "value") {
        const value = field !== null && viewMode === "view-v1" && frontendNode ? thin?.fieldValue(field, thinMailDraft.fieldValue(field, a.value)) ?? a.value : a.value;
        // A hub refresh can arrive between typing and the blur that saves a
        // to-do. Keep that focused edit until its change event captures it.
        const editingTodo = el === document.activeElement && el.getAttribute("data-on-change") === "todo-text" &&
          [...each(oldAttrs)].some(old => is(old, "At") && old.name === "value" && old.value !== el.value);
        const editingCatalog = el.hasAttribute("data-board-query") && (el === document.activeElement || el.dataset.composing === "1");
        if (!editingTodo && !editingCatalog && el.value !== value && !(composerComposing && el.id === "composer")) el.value = value;
      } else if (el.getAttribute(a.name) !== a.value) {
        el.setAttribute(a.name, a.value);
        if (a.name === "data-cat") el.src = catUri(a.value);
        else if (a.name === "data-wheel") wheelHook(el);
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
  if (catalogOpen !== null) {
    if (catalogOpen) el.setAttribute("open", "");else el.removeAttribute("open");
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
    RR?.n("memo.hit");
    return m.v;
  }
  RR?.n("memo.make");
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
// whether two vnodes draw the same: a timeline row drawn again unchanged
// keeps its old vnode (timeline-scroller.js), so nothing under it is
// patched or measured. Shared pieces (bp-memo rows expand to the very
// object they made last time) answer at once.
function sameNode(a, b) {
  if (a === b) return true;
  if (!a || !b || a.$ !== b.$) return false;
  if (is(a, "Txt")) return a.text === b.text;
  if (a.tag !== b.tag || a.key !== b.key) return false;
  let x = a.attrs, y = b.attrs;
  for (; is(x, "Con") && is(y, "Con"); x = x.tail, y = y.tail) {
    const p = x.head, q = y.head;
    if (p !== q && (p.$ !== q.$ || p.name !== q.name || p.value !== q.value || p.event !== q.event || p.action !== q.action)) return false;
  }
  if (is(x, "Con") || is(y, "Con")) return false;
  x = a.kids; y = b.kids;
  for (; is(x, "Con") && is(y, "Con"); x = x.tail, y = y.tail) if (!sameNode(x.head, y.head)) return false;
  return !is(x, "Con") && !is(y, "Con");
}
function same(a, b) {
  return a.$ === b.$ && (is(a, "Txt") || (a.tag === b.tag && patchKey(a) === patchKey(b)));
}

// the timeline's plan (timeline-plan.bend). Its input is built as Bend's
// Json in JavaScript, and each plan's tree is kept by the plain object it
// became, so the next request's previous plan (that very object, never
// changed) is not built again.
const planTrees = new WeakMap();
function timelinePlan(input) {
  const tree = Planner.timeline_plan(Wire.json({ ...input, bounded: true, bottomAlign: true, overscan: historyOverscan(input.view) }, planTrees));
  const plan = Wire.plain(tree);
  if (plan && typeof plan === "object") planTrees.set(plan, tree);
  return plan;
}

// make el (whose last vnode is el.__v) match v
function patch(el, v) {
  RR?.n("dom.patch");
  const old = el.__v;
  setAttrs(el, old.attrs, v.attrs);
  // The desktop bridge owns this label's live text. Reapplying the
  // initial placeholder on every timeline update makes it flicker.
  if (el.__desktopBrowserStatus) { el.__v = v; return; }
  if (el.id === "timeline") {
    let scroller = timelineLists.get(el);
    if (!scroller) {
      scroller = new TimelineScroller(el, {
        plan: timelinePlan, create, patch,
        key: (node, i) => History.display_key_at(node, BigInt(i)),
        floating: node => node.key === "hold-pill",
        same: sameNode,
        onChange: timelineChanged,
        onScroll: timelineScrolled,
      });
      timelineLists.set(el, scroller);
    }
    const u0 = RR ? pnow() : 0, rows = [...each(v.kids)].map(expand), u1 = RR ? pnow() : 0;
    scroller.update(rows, { follow: entryHistory.active(historyFillOwner()) || !(thin && thin.atNewest?.() === false), defer: true });
    if (RR) { RR.t("patch.tl.rows", u1 - u0);RR.t("patch.tl.update", pnow() - u1); }
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
if (PHONE) ui = App.phone_form(ui, "phone");
if (window.backplaneDesktop) ui = App.desktop(ui);
if (window.backplaneDesktop?.browser) ui = App.desktop_browser(ui);
const desktopBrowser = installDesktopBrowser({bridge:window.backplaneDesktop?.browser,
  isReady:()=>(canonicalBoot||remoteReady)&&socket?.readyState===1,
  onTakeover:owner=>dispatch("tk-takeover",owner)});
const forcedLang = new URLSearchParams(location.search).get("lang");
if (forcedLang === "ja" || forcedLang === "en") ui = App.lang_preview(ui);
ui = App.act(ui, "lang", forcedLang === "ja" || forcedLang === "en" ? forcedLang : (localStorage.getItem("backplane-lang") || "auto")).ui;
ui = App.act(ui, "lang-os", navigator.languages?.[0] || navigator.language || "").ui;
pageLang(App.lang(ui));
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
  sizeComposer(document.getElementById("composer"));
  sizeComposerControls();
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
      foot.style.minHeight = "min(" + saved.height + "px, var(--composer-button))";composerFootHeld = true;
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
window.addEventListener("pagehide", () => reviewPosition.capture());
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
let navigationRestoreRoute = null;
try {
  const saved = localStorage.getItem("backplane-navigation");
  if (saved) {
    ui = App.navigation_restore(ui, saved);
    const choice = JSON.parse(saved);
    navigationRestoreRoute = compactRoute({ kind: "bot-tab", bot: choice.bot, tab: choice.tab });
  }
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
    localStorage.setItem("backplane-navigation", App.navigation_keep(presentationJson(session)));
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
  socket.send(fromList(App.wire_out_json(Wire.json(request))));
});
const improvementsRpc = new ImprovementsRpc(request => {
  if (!socket || socket.readyState !== 1) throw Error("Connect to the hub, then retry Send.");
  socket.send(fromList(App.wire_out_json(Wire.json(request))));
});
const thinContinuations = new ThinContinuations();
const bodyWindows = new BodyWindows({renderBody: node => App.make(node), request: (part, cursor, tail) => thin?.body(part, cursor, tail), changed: () => later()});
let frontend = null, frontendNode = null, wantFrontend = true, fallbackIntent = null;
let viewMode = "discovering", remoteReady = false, remoteEpoch = "", thinSession = null;
let pairingAccepted = false;
// (24 pictures: every destination the sidebar shows is prefetched, warmDestinations)
const transitionViews = new ViewTransition({ limit: 24, store: new RecentViews({ indexedDB, limit: 24, bytes: 6 * 1024 * 1024, validate: (shown, scope) => {
  try { validatePresentation({ session: {sel:scope.selected, rid:0, drafts:{}}, view:shown, cmds:[] });return true; } catch {return false;}
} }) });
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
const COARSE_POINTER = typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;
let thin = null, thinSocket = null, thinAttempt = 0, thinExpiry = 0, thinGreetingTimer = 0, thinDraftTimer = 0, thinFieldTimer = 0;
let paletteAsked = -Infinity;
let thinReading = null, resumedConversation = null;
const readingIdentity = () => JSON.stringify([location.origin, cacheIdentity(document.cookie)]);
function keepThinReading() {
  const state = thin?.readingState();
  if (state) thinReading = { identity: readingIdentity(), state, history: historyNode && !thin.pendingNewest && thin.paging?.direction !== "newest" ? {
    target: targetKey(thin.target), conversation: thin.conversation, node: historyNode,
    gone: { older: [...historyGone.older], newer: [...historyGone.newer] }, direction: historyDirection,
    held: thin.held && { pages: thin.held.pages.map(p => ({ ...p, ids: p.ids.slice(), back: p.back.slice() })) },
  } : null };
}
function takeReadingHistory(target, session) {
  const saved = thinReading?.history;
  if (!saved) return null;
  thinReading.history = null; // one socket replacement, never another selection
  return thinReading.identity === readingIdentity() && thinReading.state.hub === thin.hub && thinReading.state.epoch === thin.epoch
    && saved.target === targetKey(target) && saved.conversation === session.sel ? saved : null;
}
// (the re-render harness asks whether more of the thread lies beyond the
// rows held, in a direction: a wheel that moves nothing there is a stall)
if (RR) RR.ahead = dir => !!thin?.hasAhead?.(dir); // the rows held, not the hub's last page
const terminalOutput = new TerminalOutputPane({ document, patch, current: () => {
  const link = thin?.client.links.get(location.origin), session = currentSession();
  return remoteReady && link ? { generation: link.greeting.generation, epoch: link.greeting.epoch, viewId: link.viewId, target: link.target, owner: session.drafts?.['@terminal.owner'], open: session.open?.term === true } : null;
} });
const focusedStore = new FocusedCache({ indexedDB, report: (state, reason) => trace("focused-cache", { state, reason }) });
function currentSession() {
  return thinSession || frontend?.session || fallbackIntent?.session || { ...Wire.plain(App.presentation(ui)), ...(navigationRestoreRoute ? { route: navigationRestoreRoute } : {}) };
}
function focusedScope() {
  return { origin: location.origin, cid, identity: cacheIdentity(document.cookie), bundle: import.meta.url,
    language: App.lang(ui), selected: currentSession().sel };
}
function transitionScope(target = presentationTarget(currentSession())) {
  return { origin: location.origin, identity: cacheIdentity(document.cookie), language: App.lang(ui) + (PHONE ? "/phone" : ""),
    epoch: thin?.epoch || remoteEpoch, bundle: import.meta.url, target, selected:currentSession().sel, settings:currentSession().open?.settings===true, route: currentSession().route || null };
}
let lastInput = 0;
for (const name of ["pointerdown", "keydown", "wheel"]) addEventListener(name, () => { lastInput = performance.now(); }, { capture: true, passive: true });
// the rail's viewer tabs read ahead for the thread on show (the press each
// tab sends, and the view.kind it opens)
const VIEWER_OPENS = [["fm", "", "files"], ["diff", "", "diff"], ["view-kind", "board", "board"], ["view-kind", "schematic", "schematic"],
  ["view-kind", "3d", "3d"], ["view-kind", "browser", "browser"], ["rv-page", "bom", "review"]].map(([action, value, kind]) => ({ action, value, kind }));
const previewViews = new ViewPrefetch({
  hub:location.origin, scope:()=>transitionScope(),
  has:target=>!!transitionViews.destination(transitionScope(), target),
  // (and the person has not pressed, typed or scrolled for two seconds: the hub
  // answers one request at a time, and a prefetch ahead of a click delays it)
  idle:soon=>remoteReady && !document.hidden && !thin?.active && !thin?.paging && (soon || performance.now() - lastInput > 2000),
  socket:()=>new WebSocket(location.origin.replace(/^http/, "ws") + "/ws?view=1&frontend=1"),
  encode:request=>fromList(App.wire_out_json(Wire.json(PHONE ? {...request,p:{...request.p,...phoneRequest(PHONE)}} : request))),
  decode:data=>Wire.plain(Wire.cbor(new Uint8Array(data), KEYS, WORDS)),
  picture:(shown, session, target, scope)=>{
    transitionViews.put({...scope,target,selected:session.sel,route:session.route||null,settings:false},shown);
    // (its messages whole: a first visit draws them so before their pages come)
    if (target.kind === "thread") bodyWindows.wholes(pictureNode(shown));
  },
  // (a viewer of the thread on show, as the hub draws it open)
  viewer:(shown, target, open)=>{
    if (targetKey(target) !== shownFor) return;
    pressFeedback.ahead(pictureNode(shown), targetKey(presentationTarget(currentSession())), open.action, open.value, open.kind);
  },
  // (the thread on show with its runs open: only what they hold is kept)
  folds:(shown, target, window)=>{
    if (targetKey(target) !== shownFor) return;
    foldFeedback.seen(pictureNode(shown));
    if (window && remoteEpoch) thinContinuations.learn(location.origin, { epoch: remoteEpoch, target, window });
  },
});
// The palette's chosen row, when it is a thread, is read ahead once the
// list has stayed 200 ms (its Enter then draws the thread's picture with
// the palette's answer, not a round trip after it): a thread far from the
// sidebar has no other picture.
let paletteAheadFor = null, paletteAheadTimer = 0;
function paletteAhead() {
  const id = document.querySelector("#workspace-commands .workspace-command.on.thread[data-thread]")?.getAttribute("data-thread") || null;
  if (id === paletteAheadFor) return;
  paletteAheadFor = id;clearTimeout(paletteAheadTimer);
  if (id) paletteAheadTimer = setTimeout(() => { if (paletteAheadFor === id) previewViews.enqueue({ kind: "thread", id }, true, false, true); }, 200);
}
// Every destination the sidebar shows is fetched ahead (view-prefetch.js),
// so a click on one draws its picture at once instead of waiting a round
// trip. A row drawn differently since its picture was fetched (new activity,
// a title, a state) has a picture that is behind: it is fetched again.
const warmRows = new Map();
function warmDestinations() {
  const current = targetKey(thin?.target);
  for (const el of [...document.querySelectorAll('[data-on-click="select"],[data-on-click="bot"],[data-on-click="proj-home"]')].filter(el=>el.getAttribute("data-value")).slice(0,24)) {
    const target={kind:el.getAttribute("data-on-click")==="bot"?"bot":el.getAttribute("data-on-click")==="proj-home"?"project":"thread",id:el.getAttribute("data-value")};
    const key = targetKey(target), row = el.className.replace(/(^|\s)on(?=\s|$)/g, "") + "\u0000" + el.textContent, seen = warmRows.get(key);
    warmRows.set(key, row);
    // (the picture of the one on show is the hub's own, kept as it comes; a
    // thread's is fetched again once per page load: one kept from an earlier
    // load has its long messages cut to previews, a fetched one has them
    // whole, which its first visit then draws until their pages come)
    if (key !== current) previewViews.enqueue(target, false, seen === undefined ? target.kind === "thread" : seen !== row);
  }
}
for (const name of ["pointerover", "touchstart"]) document.addEventListener(name, event=>{
  const el=event.target.closest?.('[data-on-click="select"],[data-on-click="bot"],[data-on-click="proj-home"]');
  if (!el) return;
  void transitionViews.warm(transitionScope());
  const target={kind:el.getAttribute("data-on-click")==="bot"?"bot":el.getAttribute("data-on-click")==="proj-home"?"project":"thread",id:el.getAttribute("data-value")};
  if (name==="pointerover") previewViews.enqueue(target,true);
  warmPicture(target);
}, {passive:true});
const targetKey = target => target ? target.kind + ":" + target.id : "";
// A retained picture as Bend's nodes, made once per picture and theme (a
// whole view: made at the click it was most of the click's own time), and
// made ahead, with its messages' Markdown, while the person points at the
// row (warmPicture): the click then draws it from what is ready.
const pictureNodes = new WeakMap();
function pictureNode(shown) {
  const kept = pictureNodes.get(shown);
  if (kept?.theme === themeNow) return kept.node;
  const node = App.presentation_node(presentationJson(themed(shown)));
  if (shown && typeof shown === "object") pictureNodes.set(shown, { theme: themeNow, node });
  return node;
}
const idleSoon = fn => typeof requestIdleCallback === "function" ? requestIdleCallback(fn, { timeout: 300 }) : setTimeout(fn, 50);
let warming = null;
function warmPicture(target) {
  const shown = transitionViews.destination(transitionScope(), target)?.shown;
  if (!shown || warming === shown || pictureNodes.get(shown)?.theme === themeNow) return;
  warming = shown;
  idleSoon(() => {
    warming = null;
    const node = pictureNode(shown), stack = [node];
    // (each message's piece made now: the click's render finds it made)
    while (stack.length) {
      const v = stack.pop();
      if (!is(v, "El")) continue;
      if (v.tag === "bp-memo") { expand(v);continue; }
      for (const k of each(v.kids)) stack.push(k);
    }
  });
}
// A retained picture of another destination is drawn with the sidebar on
// screen (the hub's last view's): the picture's own was drawn when it was
// fetched, with the sidebar's default shelves and trees, so projects and
// rows jumped at the click and back when the hub's view came. The row
// picked is marked (style.css's data-picked) until that view lands.
let liveNode = null;
// (a project head's press, drawn ahead of its click: headPress below)
let headPreview = null, headPress = null, headClick = null;
const sidebarOf = n => nodeIn(n, v => v.tag === "aside" && v.key === "sidebar");
const liveSided = new WeakMap();
function withLiveSidebar(picture) {
  if (!liveNode || transitionFor === shownFor) return picture;
  // (only while the row picked is on screen in it, or under the shut Older
  // shelf, which then shows its project: else the picture's own sidebar,
  // which shows that row, is the one the hub's view will bring)
  // (a project's home: while its section is on screen)
  const older = transitionFor.startsWith("thread:") && !document.querySelector(`aside.sidebar .thread[data-value="${CSS.escape(transitionFor.slice(7))}"]`);
  if (transitionFor.startsWith("project:") ? !document.querySelector(`aside.sidebar section[data-drag-to="${CSS.escape(transitionFor.slice(8))}"]`)
    : !transitionFor.startsWith("thread:")) return picture;
  const kept = liveSided.get(picture);
  if (kept?.live === liveNode) return kept.out;
  let side = sidebarOf(liveNode);
  const theirs = sidebarOf(picture);
  if (older) side = side && theirs ? olderFrom(side, theirs, transitionFor.slice(7)) : null;
  if (!side) { liveSided.set(picture, { live: liveNode, out: picture });return picture; }
  let out = picture;
  if (side && theirs && side !== theirs) {
    const moved = sidebarRegrouped(side, theirs);
    const walk = n => {
      if (n === theirs) return moved;
      if (!is(n, "El") || n.tag === "bp-memo") return n;
      const ks = [...each(n.kids)], mapped = ks.map(walk);
      if (mapped.every((k, i) => k === ks[i])) return n;
      let kids = { $: "Nil" };
      for (let i = mapped.length - 1; i >= 0; i--) kids = { $: "Con", head: mapped[i], tail: kids };
      return { ...n, kids };
    };
    out = walk(picture);
  }
  liveSided.set(picture, { live: liveNode, out });
  return out;
}
// A thread picked from under the shut Older shelf (a palette jump, a
// link): shut, the shelf shows just the open thread's project ("Older 7 ·
// 1 shown"). The live sidebar takes the picture's shut shelf and the
// projects it shows (the picture is drawn with that thread open), in place
// of its own; null when either has it open or the picture's lacks the row.
function olderFrom(side, theirs, id) {
  const live = sectionsIn(side), pic = sectionsIn(theirs);
  if (!live || !pic) return null;
  const shelf = ks => ks.findIndex(k => is(k, "El") && k.tag === "button" && classOf(k).includes("older"));
  const block = (ks, i) => { let j = i + 1;while (j < ks.length && is(ks[j], "El") && ks[j].tag === "section") j++;return j; };
  const iL = shelf(live.ks), iP = shelf(pic.ks);
  if (iL < 0 || iP < 0 || classOf(live.ks[iL]).includes("open") || classOf(pic.ks[iP]).includes("open")) return null;
  const jL = block(live.ks, iL), jP = block(pic.ks, iP), shown = pic.ks.slice(iP + 1, jP);
  if (!shown.some(n => JSON.stringify(n).includes(JSON.stringify(id)))) return null;
  const kids = [...live.ks.slice(0, iL), pic.ks[iP], ...shown, ...live.ks.slice(jL)];
  const swap = n => {
    if (n === live.at) { let l = { $: "Nil" };for (let i = kids.length - 1; i >= 0; i--) l = { $: "Con", head: kids[i], tail: l };return { ...n, kids: l }; }
    if (!is(n, "El") || n.tag === "bp-memo") return n;
    const ks = [...each(n.kids)], mapped = ks.map(swap);
    if (mapped.every((k, i) => k === ks[i])) return n;
    let l = { $: "Nil" };for (let i = mapped.length - 1; i >= 0; i--) l = { $: "Con", head: mapped[i], tail: l };
    return { ...n, kids: l };
  };
  return swap(side);
}
// The open thread's project keeps its place among the live ones (it never
// sinks under the reader: client.bend's Side.groups.at) only while that
// thread is open: picking a thread in another project sinks it to the
// quiet ones, and raises the picked thread's own. The picture's sidebar,
// drawn with the picked thread open, says which of the two move: they are
// moved in the live sidebar at the press (each placed after the section it
// follows in the picture), every other section left where it is, so the
// hub's view finds them in place. Only those two: the picture's sidebar may
// be older than the live one for every other project.
const sectionsIn = side => {
  const at = nodeIn(side, v => [...each(v.kids)].some(k => is(k, "El") && k.tag === "section" && k.key && classOf(k).includes("project")));
  return at ? { at, ks: [...each(at.kids)] } : null;
};
const classOf = n => (nodeAttr(n, "class") || "").split(/\s+/);
const withNodeAttr = (n, name, value) => {
  const as = [...each(n.attrs)];
  let l = { $: "Nil" };for (let i = as.length - 1; i >= 0; i--) l = { $: "Con", head: is(as[i], "At") && as[i].name === name ? { ...as[i], value } : as[i], tail: l };
  return { ...n, attrs: l };
};
function sidebarRegrouped(side, theirs) {
  const projectOf = id => id && document.querySelector(`aside.sidebar .thread[data-value="${CSS.escape(id)}"]`)?.closest("section")?.getAttribute("data-drag-to");
  // (a project's home holds nothing in place: only the left thread's moves)
  const pa = shownFor.startsWith("thread:") ? projectOf(shownFor.slice(7)) : "", pt = transitionFor.startsWith("thread:") ? projectOf(transitionFor.slice(7)) : "";
  const live = sectionsIn(side), pic = sectionsIn(theirs);
  if (!live || !pic || pa === pt) return side;
  const picBy = new Map(pic.ks.filter(k => is(k, "El") && k.tag === "section" && k.key).map(k => [k.key, k]));
  const quiet = n => classOf(n).includes("quiet");
  const move = new Set([pa, pt].filter(id => id && live.ks.some(k => k.key === id) && picBy.has(id)
    && quiet(live.ks.find(k => k.key === id)) !== quiet(picBy.get(id))));
  if (!move.size) return side;
  let kids = live.ks.filter(k => !move.has(k.key));
  const order = pic.ks.map(k => k.key);
  for (const id of order.filter(id => move.has(id))) {
    const own = live.ks.find(k => k.key === id), cls = nodeAttr(picBy.get(id), "class");
    const node = withNodeAttr(own, "class", cls);
    // after the nearest section before it in the picture that is in place
    let after = -1;
    for (let i = order.indexOf(id) - 1; i >= 0 && after < 0; i--) after = kids.findIndex(k => k.key === order[i] && !move.has(order[i]));
    if (after < 0 && order.indexOf(id) > 0) return side;
    kids = [...kids.slice(0, after + 1), node, ...kids.slice(after + 1)];
  }
  const swap = n => {
    if (n === live.at) { let l = { $: "Nil" };for (let i = kids.length - 1; i >= 0; i--) l = { $: "Con", head: kids[i], tail: l };return { ...n, kids: l }; }
    if (!is(n, "El") || n.tag === "bp-memo") return n;
    const ks = [...each(n.kids)], mapped = ks.map(swap);
    if (mapped.every((k, i) => k === ks[i])) return n;
    let l = { $: "Nil" };for (let i = mapped.length - 1; i >= 0; i--) l = { $: "Con", head: mapped[i], tail: l };
    return { ...n, kids: l };
  };
  return swap(side);
}
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
  transitionNode = shown ? pictureNode(shown) : transitionNode;
  return !!shown;
}
const focusedView = new FocusedView({
  cache: focusedStore, scope: focusedScope,
  // (a kept picture put away; with none on show nothing changes)
  hide: () => { if (!focusedNode) return;focusedNode = null;later(); },
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
function refreshFocusedIdentity() {
  clearTimeout(attachmentIdentityRetry);attachmentIdentityRetry = 0;
  const generation = ++attachmentIdentityGeneration;
  identityUnverified();
  return attachmentIdentityCheck = refreshFocusedIdentityFor(generation);
}
// (a retry keeps the time the pairing check began: Send's wait is bounded from it)
function identityUnverified() {
  if (!attachmentIdentityPending) attachmentIdentitySince = Date.now();
  attachmentIdentityPending = true;attachmentIdentity = "";
}
async function refreshFocusedIdentityFor(generation) {
  const before = cacheIdentity(document.cookie);
  const result = await refreshCacheIdentity({ fetch: window.fetch.bind(window), readCookie: () => document.cookie,
    writeCookie: value => { document.cookie = value; }, protocol: location.protocol });
  if (generation !== attachmentIdentityGeneration) return;
  trace("cache-identity", { state: result.state });
  attachmentIdentity = result.state === "verified" ? result.identity : "";
  if (result.state === "verified") pairingUI.accepted();
  if (result.state === "denied") {
    attachmentIdentityPending = false;
    void draftAttachments.clear().catch(() => {});
    stopThin(false, "auth-denied");viewMode = "read-only";focusedView.invalidate();
    pairingUI.show({repair: !!before, cause: before ? "pairing_required" : ""});later();return;
  }
  if (before !== cacheIdentity(document.cookie) || result.state === "denied" || result.state === "invalid") {
    void draftAttachments.prune().catch(() => {});
    previewViews.stop();transitionViews.clear();warmRows.clear();transitionNode = null;transitionFor = "";focusedView.invalidate();
    if (!canonicalBoot && !remoteReady) void restoreFocused();
    else saveFocusedSoon();
  }
  if (attachmentIdentity) {
    await draftAttachments.prune().catch(() => {});
    if (generation !== attachmentIdentityGeneration) return;
    restoreDraftAttachments(currentSession());
  }
  attachmentIdentityPending = result.state === "offline" || result.state === "superseded";
  enterImprovements();
  if (attachmentIdentityPending) attachmentIdentityRetry = setTimeout(() => {
    if (socket?.readyState === 1) void refreshFocusedIdentity();
  }, 1500);
}

let cacheReady = false;
let queued = false;
let scroll = false;
let focus = null;
let paletteSearch = ""; // a palette query to search conversations for once the palette is closed
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
const entryHistory = new EntryHistory();
let held = false, away = "", holdPending = null;
function timelineList(tl = document.getElementById("timeline")) { return tl && timelineLists.get(tl); }
function historyClear() { entryHistory.cancel();resumedConversation = null;historyNode = null;historyDirection = "";historyOlderDemand = 0;historyFillScope = "";historyFillMore(null, false);historyPageState(null);historyGone.older.clear();historyGone.newer.clear();thin?.heldReset?.(); }
// Rows let go past the cap (thin-host.js's heldTrim): their display keys,
// by the side they went from, and the height they had, kept in their place
// as a gap (timeline-history.bend's gaps) so the scrollbar and the reader's
// place stay put. A page that brings them back takes its rows out of the gap.
// (keys: an entry's row is keyed by its id, a run of tool calls it heads
// by "fold:<id>"; a row never measured adds nothing)
const historyGone = { older: new Set(), newer: new Set() };
function historyGap(side) {
  const gone = historyGone[side];
  if (!gone.size) return 0;
  const sizes = activeTimeline?.sizes || {};
  let px = 0;
  for (const id of gone) px += sizes[id] > 0 ? sizes[id] : sizes["fold:" + id] > 0 ? sizes["fold:" + id] : 0;
  return Math.round(px);
}
function historyLetGo(ids, side) {
  if (!historyNode || !ids.length) return;
  historyNode = History.drop(historyNode, ids.join("\n"));
  for (const id of ids) historyGone[side].add(id);
}
// a page's entries are on the page again: out of the gaps (O(page))
function historyBack(ids) {
  if (!historyGone.older.size && !historyGone.newer.size) return;
  for (const id of ids || []) { historyGone.older.delete(id);historyGone.newer.delete(id); }
}
function historyGaps(node) {
  const older = historyGap("older"), newer = historyGap("newer");
  return older || newer ? History.gaps(node, String(older), String(newer)) : node;
}
// A page on its way says nothing, however long it takes: it is asked for
// well before the reader can reach the edge of the rows drawn (historyRunway),
// and a reader who gets there stays on the last row drawn until it comes.
// A line saying "loading" while scrolling is what the person must never
// see. A failed page still says so, with Retry.
function historyPageState(state) {
  historyStatus = state?.phase === "error" ? state : null;
  if (!historyStatus) { document.getElementById("history-status")?.remove();return; }
  historyPageShow(historyStatus);
}
function historyPageShow(state) {
  let el = document.getElementById("history-status");
  if (!el) { el = document.createElement("div");el.id = "history-status";document.body.appendChild(el); }
  el.setAttribute("role", "status");
  el.style.cssText = "position:fixed;z-index:12;max-width:calc(100vw - 32px);background:var(--panel);color:var(--text);border:1px solid var(--line);padding:4px 8px;font-size:12px";
  const box = document.getElementById("timeline")?.getBoundingClientRect();
  el.style.top = Math.max(0, box?.top || 0) + "px";el.style.right = Math.max(8, innerWidth - (box?.right || innerWidth) + 8) + "px";
  el.replaceChildren(document.createTextNode("Messages couldn’t load. "));
  earlierAsked = laterAsked = false;
  const button = document.createElement("button");button.textContent = "Retry";
  button.onclick = () => thin?.retryPage();
  el.appendChild(button);
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
  if (focusedNode && !frontendNode) return;
  const thread = now || App.sel(ui), controller = thin, target = thin?.target;
  // Paging/reconnect can establish the controller's first unread boundary
  // independently of this DOM latch. A local empty latch alone is not an
  // acknowledgement that the genuinely newest viewport has been reached.
  const marked = !!thin?.awayOf?.(thread);
  if (now === away && (!thin || (now ? marked : !marked))) return;
  if (holdPending?.thread === thread && holdPending.now === now && holdPending.controller === controller && holdPending.target === target) return;
  away = now;
  const pending = holdPending = { thread, now, controller, target };
  queueMicrotask(() => {
    if (holdPending !== pending) return;
    holdPending = null;
    // Old picture callbacks never consume a newly selected bot/thread, even
    // when two bot tabs resolve to the same underlying conversation.
    if (thin !== controller || thin?.target !== target || App.sel(ui) !== thread || controller && !controller.ready) return;
    dispatch(now ? "hold-away" : "hold-end", thread);
  });
}
function timelineScrolled(list, delta) {
  if (list !== activeTimeline || !delta) return;
  entryHistory.cancel();
  historyFillMore(null, false);
  historyDirection = delta < 0 ? "older" : "newer";
  historyOlderDemand = delta < 0 ? 2 : 0;
}
function timelineChanged(list) {
  if (list !== activeTimeline || focusedNode && !frontendNode) return;
  if (!canonicalBoot && (!remoteReady || !frontendNode)) return;
  if (transitionPosition) return;
  // Initial fill temporarily reads older source pages. It grants no reading
  // acknowledgement until the merged newest source is painted again.
  if (entryHistory.active(historyFillOwner())) { historyCheckSoon();return; }
  // The trail moves when newest is requested; the old viewport is still
  // unread until that request's content has actually been presented.
  held = !list.atEnd() || !!thin && (thin.atNewest?.() === false || thin.paging?.direction === "newest");
  holdSay(held ? App.sel(ui) : "");
  historyCheckSoon();
  bodyWindows.observe(list.el, remoteReady && !!frontendNode);
}
function holdMark() {
  const source = timelineList()?.el?.getAttribute("data-reading-seen");
  if (source) return source;
  return timelineList()?.rows.map(historyRow).filter(id => id && !id.startsWith("@") && !id.startsWith("control:")).at(-1) || "";
}
function holdBottom() {
  entryHistory.cancel();
  if (thin && (thin.paging || thin.atNewest?.() === false)) { thin.newest();return; }
  const list = timelineList();
  // The current source page needs no network reply to retire the old
  // upward gesture. Otherwise the next history frame pages back again
  // immediately after the new-output button reaches the bottom.
  historyDirection = "";historyOlderDemand = 0;historyFillMore(null, false);
  list?.scrollToEnd();
  // An already-bottom command need not emit a DOM scroll event. Reconcile
  // its accepted geometry too, rather than leaving a stale badge clickable.
  if (list) timelineChanged(list);
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
// <html lang>: written only when it changes. Writing the same value is still
// a :lang() invalidation of every element (each hub view wrote it: ~1,500
// style invalidations a view on a thread)
function pageLang(lang) {
  if (document.documentElement.lang !== lang) document.documentElement.lang = lang;
}
// <html>'s data attributes (style.css reads them): written only when they
// change, so a render that changes nothing there invalidates no style
function htmlData(name, value) {
  const d = document.documentElement.dataset, v = String(value);
  if (d[name] !== v) d[name] = v;
}
function render() {
  rendering = true;
  try { renderPage(); } finally { rendering = false; }
}
function renderPage() {
  reviewPosition.capture();
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
  const t0 = PERF || RR ? pnow() : 0;
  // (a press not yet answered: what it opened, drawn into the hub's picture
  // before the overlay, so the draft and the pieces in it are the page's own;
  // press-feedback.js)
  if (frontendNode) pressFeedback.seen(frontendNode, targetKey(presentationTarget(currentSession())));
  let page = headPreview && frontendNode ? App.presentation_overlay(headPreview.node, presentationJson(currentSession()))
    : frontendNode ? App.presentation_overlay(historyGaps(pressFeedback.project(frontendNode)), presentationJson(feedbackSession()))
    : transitionNode ? App.presentation_overlay(pressFeedback.project(withLiveSidebar(transitionNode)), presentationJson(currentSession())) : focusedNode
    || (canonicalBoot ? App.view(ui) : App.presentation_node(presentationJson(themed(Wire.plain(App.presentation_view(App.presentation_wait(ui)))))));
  // Saved transition/startup pictures contain the server's unprojected partial
  // footer. Clean it before their first paint, without borrowing live readers
  // or request authority from the currently selected conversation.
  const tc = RR ? pnow() : 0;
  if (RR) RR.t("render.overlay", tc - t0);
  // a run of tool calls opened or shut at the click (fold-feedback.js),
  // before the continuations: its rows get the same controls the hub's do
  if (frontendNode) foldFeedback.seen(frontendNode);
  if (frontendNode || transitionNode) page = foldFeedback.project(page);
  // (a retained picture of a thread read before: its long messages drawn
  // with the pages read for them then, as the hub's view will draw them)
  const heldBodies = !frontendNode && transitionNode ? bodyWindows.heldParts(transitionFor) : null;
  if (frontendNode || transitionNode || focusedNode) page = thinContinuations.project(page,
    node => node.tag === "bp-memo" && (frontendNode ? bodyWindows.parts : heldBodies?.parts)?.get(node.key)?.automatic === true && !(heldBodies && node.key.startsWith("@live:")) ? node : expand(node), !!frontendNode,
    frontendNode ? foldFeedback.restored() : null);
  const tb = RR ? pnow() : 0;
  if (frontendNode) page = bodyWindows.project(page);
  else if (heldBodies) page = bodyWindows.projectHeld(page, transitionFor);
  if (frontendNode && remoteReady || transitionNode && transitionFor === targetKey(presentationTarget(currentSession()))) page = pendingPresentation(page);
  const t1 = PERF || RR ? pnow() : 0;
  if (RR) { RR.t("render.continuations", tb - tc);RR.t("render.bodies", t1 - tb); }
  patch(root, { $: T.El, tag: "div", key: "", attrs: { $: "Nil" }, kids: { $: "Con", head: page, tail: { $: "Nil" } } });
  starAttentionSync(root);
  bubMoveSync();
  paletteCaughtUp();
  paletteAhead();
  // (a picture of another thread drawn with the live sidebar: its row marked)
  if (transitionNode && !frontendNode && transitionFor !== shownFor && transitionFor.startsWith("thread:")) {
    const row = document.querySelector(`aside.sidebar .thread[data-value="${CSS.escape(transitionFor.slice(7))}"]`);
    if (row) pickRow(row);
  }
  terminalOutput.apply(true);
  phoneOther.rendered();
  appShell.rendered();
  if (PERF) PERF.render.push([t1 - t0, pnow() - t1]);
  if (RR) { RR.n("render.full");RR.t("render.view", t1 - t0);RR.t("render.patch", pnow() - t1); }
  if (frontendNode || canonicalBoot && App.synced(ui)) themeSeen(root.querySelector(".app"));
  if (canonicalBoot) {
    htmlData("backplaneReady", "true");
    htmlData("backplaneSeq", App.seq(ui));
  }
  htmlData("backplaneMode", viewMode);
  // (style.css sizes the tools rail by it: an html:has() there restyled the
  // whole page on every change inside .app)
  htmlData("workspace", root.querySelector(".app > .workspace-thread") ? "thread" : "");
  htmlData("backplaneRemoteReady", String(remoteReady));
  htmlData("backplaneCached", String(!!(focusedNode || transitionNode) && !frontendNode));
  // what style.css quiets: a picture kept for another target than the one
  // picked (true, then quiet past QUIET_MS), and the page before the hub's
  // first view (its frame only, nothing half-filled)
  const heldFor = !frontendNode && !!transitionNode && transitionFor !== targetKey(presentationTarget(currentSession()));
  htmlData("backplaneHeld", !heldFor ? "false" : heldAt && performance.now() - heldAt >= QUIET_MS ? "quiet" : "true");
  htmlData("backplaneWait", String(!frontendNode && !transitionNode && !focusedNode && !canonicalBoot));
  trace("dom", { seq: App.seq(ui) });
  if (PERF?.trace) requestAnimationFrame(() => trace("paint", { seq: App.seq(ui) }));
  mailRestore(mail);
  reviewPosition.mount(document.querySelector(".review-body"));
  desktopBrowser.sync();
  memoSweep();
  const tl2 = document.getElementById("timeline");
  const list = timelineList(tl2);
  if (activeTimeline && activeTimeline !== list) activeTimeline.destroy();
  activeTimeline = list || null;
  // (one plan for the patch's rows and this render's command: timeline-scroller.js)
  let next = null;
  if (transitionPosition && list) { next = { mode: "restore", bookmark: { ...transitionPosition } };transitionPosition = null; }
  else if (scroll && list) next = { mode: "bottom" };
  list?.refresh(next);
  // A live bot/mailroom update may change only the floating pill. Unchanged
  // virtual rows/geometry cause neither a plan nor a scroll callback; still
  // reconcile the accepted newest picture with the controller's read mark.
  if (list) timelineChanged(list);
  const main = viewMode === "view-v1" ? document.querySelector("main") : null;
  if (main && main.getAttribute("aria-busy") !== String(!remoteReady)) main.setAttribute("aria-busy", String(!remoteReady));
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
    } else if (thin?.hasAhead?.("older") && searchJump.pages < 40) {
      if (!searchJump.asked) { searchJump.asked = true; requestAnimationFrame(() => { if (searchJump) { searchJump.asked = false; if (thin.ahead("older")) searchJump.pages++; } }); }
    } else if (thin?.ready) searchJump = null;
  }
  searchLight(tl2);
  panes();
  secretSaves.paint();
  projectSplitters.sync();
  // (a viewer pane drawn at a press is painted first: a 3D or board canvas
  // starting up in the same frame held its first picture back ~500 ms)
  // (its toolbar, though, is built in the press's frame: built with the
  // canvas, it filled in two frames late and the stage shrank under it)
  // and its model's last picture, when it has one (solid.js)
  // (a board or schematic draws in the press's own frame, from the plot the
  // page holds: only 3D's set-up waits, else the pane comes up a frame
  // before its drawing)
  if (pressFeedback.active?.kind === "viewer") { mountTools(true); Solid.picture(document.getElementById("solid")); Plot2d.mount([...document.querySelectorAll("canvas.plot2d")]); requestAnimationFrame(() => requestAnimationFrame(mountViewers)); }
  else mountViewers();
  if (!glWarmAsked && document.querySelector('[data-on-click="view-kind"][data-value="3d"]')) { glWarmAsked = true; Solid.warm(); }
  ViewerTools.chipInView();
  ViewerTools.focusToggle();
  Plot2d.inspectGone();
  attachmentPreviews();
  previewTextFiles();
  previewMarkdownFiles((text, node) => create(App.md(text, App.lang(ui), node.dataset.mdRoot || "", node.dataset.mdPath || "")));
  composerGeometry();
  earlierAsked = false;
  laterAsked = false;
  // a page in: the next one, if the reader is still within reach of the edge
  if (tl2 && thin?.ready) historyCheckSoon();
  // A page of entries the timeline does not draw (todo steps, a fold's
  // calls) moves nothing and, at the very top, no scroll comes to ask for
  // the next: still near the top after a render, it is asked for

  if (paletteSearch && !document.querySelector(".workspace-palette")) { const query = paletteSearch;paletteSearch = "";dispatch("thin-search-open", query); }
  if (focus) {
    const el = document.getElementById(focus);
    // (on a touch screen only a search just opened takes the keyboard by
    // itself; the rest wait for a tap on the field: touch-focus.js)
    const took = softFocus(el, { asked: ASKED_TO_TYPE.has(focus) });
    // a rename opens with the old title selected: typing replaces it ("New
    // thread" was kept in front of what was typed), an arrow key keeps it
    if (took && focus === "thread-title") el?.select?.();
    focus = null;
  }
  refocusGive();
  if (reclaim && remoteReady) { reclaim = false;composerKeeper.claim(); } else composerKeeper.check();
  tourSync();
  settingsSync();
  phoneShell.rendered();
  saveFocusedSoon();
}

// (drawnOnly: a new workbench on the page only; a kept one comes back with
// its canvas, in mountViewers)
function mountTools(drawnOnly = false) {
  const root = document.getElementById("solid-workbench");
  if (!drawnOnly || root && !ViewerTools.isKept(root)) ViewerTools.mount(root, Solid, { components: (thread, options) => viewerRpc.components(thread, options), asset: (thread, path, kind, revision, options) => viewerRpc.renderAsset(thread, path, kind, revision, options), plot: Plot2d });
}
function mountViewers() {
  Solid.mount(document.getElementById("solid"));
  mountTools();
  Plot2d.mount([...document.querySelectorAll("canvas.plot2d")]);
}

// Live updates and page landings redraw only the regions their delta wrote
// into (core/landing.bend, laws land_*). A sidebar activity change must not
// overlay, project and reconcile the selected timeline. Whole replies and
// changes outside these regions (including session changes) draw the page.
const jsonClass = (n, c) => Array.isArray(n?.attrs) && n.attrs.some(a => a?.name === "class" && (" " + a.value + " ").includes(" " + c + " "));
const nodeAttr = (v, name) => { for (const a of each(v.attrs)) if (is(a, "At") && a.name === name) return a.value; return null; };
const LAND_REGIONS = [
  { name: "sidebar", json: n => n?.tag === "aside" && n.key === "sidebar",
    node: v => v.tag === "aside" && v.key === "sidebar", el: () => document.querySelector("aside.sidebar") },
  { name: "timeline", json: n => Array.isArray(n?.attrs) && n.attrs.some(a => a?.name === "id" && a.value === "timeline"),
    node: v => nodeAttr(v, "id") === "timeline", el: () => document.getElementById("timeline") },
  { name: "topbar", json: n => n?.tag === "header" && jsonClass(n, "topbar"),
    node: v => v.tag === "header" && (" " + (nodeAttr(v, "class") || "") + " ").includes(" topbar "), el: () => document.querySelector("header.topbar") },
  // a keystroke's echo (the draft, its send button, the composer's popup)
  // redraws the composer alone, not the page
  { name: "composer", json: n => n?.tag === "div" && jsonClass(n, "composer"),
    node: v => v.tag === "div" && (" " + (nodeAttr(v, "class") || "") + " ").includes(" composer "), el: () => document.querySelector(".thread-view .composer") },
];
const landPaths = new Map();
// a region's path in a reply's body: the last one first (regions seldom move)
function landPathIn(shown, region) {
  const last = landPaths.get(region.name);
  if (last) {
    let n = shown;
    for (let i = 0; i < last.length && n; i += 2) n = n.kids?.[last[i + 1]];
    if (region.json(n)) return last;
  }
  const stack = [[shown, []]];
  while (stack.length) {
    const [n, path] = stack.pop();
    if (region.json(n)) { landPaths.set(region.name, path);return path; }
    if (Array.isArray(n?.kids)) for (let i = n.kids.length - 1; i >= 0; i--) stack.push([n.kids[i], [...path, "kids", i]]);
  }
  return null;
}
// "page", or the names of the regions to redraw, space-separated
function historyRegion(detail, projection) {
  const landing = detail.page && ["older", "newer"].includes(projection);
  const update = !detail.page && projection === "update" && !detail.historyReset && !detail.position;
  if (!(landing || update) || !Array.isArray(detail.changed) || detail.changed.some(p => !p.length) || searchJump || jump || scroll) return "page";
  // (the session's drafts are the page's own, each keystroke's echo: the
  // composer draws them, a region of its own; every echo drew the page)
  const changed = detail.changed.filter(p => !(p[0] === "session" && p[1] === "drafts"));
  const lines = [];
  for (const r of LAND_REGIONS) { const p = landPathIn(detail.shown, r);if (p) lines.push([r.name, "shown", ...p].join("\t")); }
  if (!lines.length) return "page";
  const region = History.landing(lines.join("\n"), changed.map(p => p.map(String).join("\t")).join("\n"));
  if (RR) RR.n("land.region." + (region.trim() || "none"));
  return region;
}
let landQueued = null;
function laterRegions(names) {
  if (queued) return;
  const first = !landQueued;
  landQueued = new Set([...(landQueued || []), ...names]);
  // A landing's two halves in two frames: the reply's own task (decode,
  // delta, merge) ends this one, and its regions are drawn in the next, so
  // a page that came just before a frame never puts both in it.
  if (first) requestAnimationFrame(() => requestAnimationFrame(renderRegions));
}
function nodeIn(node, test) {
  const stack = [node];
  while (stack.length) {
    const v = stack.pop();
    if (!is(v, "El")) continue;
    if (test(v)) return v;
    for (const k of each(v.kids)) stack.push(k);
  }
  return null;
}
// The timeline's overlay row by row, each kept by the row it was made from
// while the draft and the model picker stay as they were (all the overlay
// reads, presentation.bend's Node.overlay): a landing overlays only the
// page's new rows, and the rest stay the very same objects.
let overlayKept = new WeakMap(), overlayFor = "";
function overlayRows(timeline, session) {
  const s = currentSession(), key = JSON.stringify([s.sel || "", s.drafts?.[s.sel] ?? "", s.open?.models === true]);
  if (key !== overlayFor) { overlayKept = new WeakMap();overlayFor = key; }
  const shell = App.presentation_overlay({ ...timeline, kids: { $: "Nil" } }, session), rows = [];
  for (const row of each(timeline.kids)) {
    let out = overlayKept.get(row);
    if (!out) { out = App.presentation_overlay(row, session);if (row && typeof row === "object") overlayKept.set(row, out); }
    rows.push(out);
  }
  let kids = { $: "Nil" };
  for (let i = rows.length - 1; i >= 0; i--) kids = { $: "Con", head: rows[i], tail: kids };
  return { ...shell, kids };
}
function renderRegions() {
  const names = landQueued || new Set();landQueued = null;
  if (queued || !frontendNode) return;
  pressFeedback.seen(frontendNode, targetKey(presentationTarget(currentSession())));foldFeedback.seen(frontendNode);
  // (a region drawn at a press lives in the page's picture: the page is drawn)
  if (pressFeedback.showing || foldFeedback.showing) { later();return; }
  const parts = [];
  for (const r of LAND_REGIONS) if (names.has(r.name)) {
    const el = r.el(), hub = nodeIn(frontendNode, r.node);
    // nothing to patch in place: the page is drawn
    if (!el?.__v || !hub || !same(el.__v, hub)) { if (RR) RR.n("land.bail." + r.name);later();return; }
    parts.push([r, el, hub]);
  }
  const t0 = RR ? pnow() : 0, session = presentationJson(feedbackSession());
  for (const [r, el, hub] of parts) {
    const a0 = RR ? pnow() : 0;
    let v = r.name === "timeline" ? overlayRows(historyGaps(hub), session) : App.presentation_overlay(hub, session);
    if (r.name === "timeline") v = pendingPresentation(v);
    const a1 = RR ? pnow() : 0;
    v = thinContinuations.project(v, node => node.tag === "bp-memo" && bodyWindows.parts.get(node.key)?.automatic === true ? node : expand(node), true);
    v = bodyWindows.project(v);
    const a2 = RR ? pnow() : 0;
    patch(el, v);
    if (RR) { RR.t("reg." + r.name + ".overlay", a1 - a0);RR.t("reg." + r.name + ".project", a2 - a1);RR.t("reg." + r.name + ".patch", pnow() - a2); }
  }
  if (names.has("sidebar")) bubMoveSync();
  if (names.has("composer")) { attachmentPreviews();composerGeometry(); }
  if (names.has("timeline")) {
    const tl = document.getElementById("timeline"), list = timelineList(tl);
    if (activeTimeline && activeTimeline !== list) activeTimeline.destroy();
    activeTimeline = list || null;
    list?.refresh();
    if (list) timelineChanged(list);
    bodyWindows.observe(tl, remoteReady && !!frontendNode);
    if (historyStatus) historyPageState(historyStatus);
    searchLight(tl);
    earlierAsked = false;
    laterAsked = false;
    if (thin?.ready) historyCheckSoon();
  }
  saveFocusedSoon();
  if (RR) { RR.n("render.regions");RR.t("render.regions", pnow() - t0); }
}

let queuedFrame = 0, rendering = false;
function later() {
  if (!queued) {
    queued = true;
    queuedFrame = requestAnimationFrame(render);
  }
}
// A press's picture drawn in its own handler, not at the next frame: what
// it shows is ready (the press layer's, a retained picture), and waiting
// for the frame lost one (M: DOM at ~27 ms, pixels ~30, for a press at 0).
function renderNow() {
  if (!queued || rendering) return;
  cancelAnimationFrame(queuedFrame);
  render();
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
    const t0 = PERF || RR ? pnow() : 0;
    patch(c, App.composer(ui));
    attachmentPreviews();
    const a = document.getElementById("asks");
    if (a && a.__v) patch(a, App.asks(ui));
    if (PERF) PERF.part.push(pnow() - t0);
    if (RR) { RR.n("render.part");RR.t("render.part", pnow() - t0); }
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
  void copyText(text).catch(error => showViewStatus(error.message));
}

function run(cmds) {
  for (const c of each(cmds)) {
    if (is(c, "Send")) {
      // These commands exist only after the shared confirmation accepted.
      // Clearing at submission also covers a deletion whose reply is lost.
      try {
        const request = JSON.parse(c.text);
        if (request.m === "thread.delete") void draftAttachments.forget(request.p.thread).catch(() => {});
        if (request.m === "project.remove") void draftAttachments.prune(request.p.project).catch(() => {});
      } catch {}
      outboxKeep(c.text);
      if (socket && socket.readyState === 1) {
        if (PERF?.trace) { const request = JSON.parse(c.text); trace("send", { id: request.id, method: request.m, queuedBytes: socket.bufferedAmount }); }
        socket.send(fromList(App.wire_out(c.text)));
      }
    } else if (is(c, "Copy")) {
      copy(c.text);
    } else if (is(c, "FocusField")) {
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
      applyComposerDraft(document, c.thread, c.text, composerComposing);
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
  if (!(canonicalBoot || remoteReady) || socket?.readyState !== 1) return;
  const focused = document.visibilityState === "visible" && document.hasFocus();
  const r = App.desk(ui, focused, now());
  ui = r.ui;
  if (is(r.cmds, "Con")) {
    const drafts = Wire.plain(App.presentation(ui)).drafts;
    deskState = {"@desk":drafts["@desk"] || "", "@desk.at":drafts["@desk.at"] || ""};
  }
  run(r.cmds);
}

// A view response cannot know the focus receipt this renderer just sent.
// Keep that receipt across adoption, rather than reporting again every frame.
let deskState = {"@desk":"", "@desk.at":""};
function keepDesk(session) { return {...session, drafts:{...session.drafts, ...deskState}}; }

for (const ev of ["focus", "blur"]) window.addEventListener(ev, deskCheck);
document.addEventListener("visibilitychange", deskCheck);
for (const ev of ["pointerdown", "keydown", "wheel"]) document.addEventListener(ev, deskCheck, { capture: true, passive: true });

// the browser asks the person once, on a first click or key (the only time
// browsers let a page ask)
function askNotify() {
  // (a phone asks from Settings → Threads instead, phone-other.js)
  if (PHONE_TITLES()) return;
  if ("Notification" in window && Notification.permission === "default") Notification.requestPermission().catch(() => {});
}
for (const ev of ["pointerdown", "keydown"]) document.addEventListener(ev, askNotify, { once: true, capture: true });

function notify(c) {
  // (inside the phone apps the app posts it: app-shell.js)
  if (appShell.notify(c)) return;
  if (!("Notification" in window) || Notification.permission !== "granted") return;
  // (a phone's browser shows a page's notifications only through its
  // service worker, Android Chrome's constructor throws; sw.js's
  // notificationclick sends the tap back as "bp-open")
  const viaWorker = () => navigator.serviceWorker?.ready?.then(r => r.showNotification(c.title, { body: c.body, tag: c.key || c.thread, icon: document.querySelector("link[rel=apple-touch-icon]")?.href || "./apple-touch-icon.png", data: { thread: c.thread || "" } })).catch(() => {});
  if (PHONE_TITLES() && navigator.serviceWorker?.controller) { viaWorker();return; }
  try {
    const n = new Notification(c.title, { body: c.body, tag: c.key || c.thread, icon: document.querySelector("link[rel=apple-touch-icon]")?.href || "./apple-touch-icon.png" });
    n.onclick = () => {
      window.focus();
      if (c.thread) dispatch("select", c.thread);
      n.close();
    };
  } catch { viaWorker(); }
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
  // (a hub that asks for a shown thread's card itself says so in its
  // greeting: the look would only cost it another render and a round trip)
  if (thin.client?.links?.get(location.origin)?.greeting?.fabSelf === true) return;
  const now = Date.now(), last = fabLooked.get(target.id) || 0;
  if (now - last < 20000) return;
  fabLooked.set(target.id, now);
  setTimeout(() => { if (thin?.ready) thin.request("fab-look", ""); }, 0);
}

// The secure field is read only at Save. Keep metadata until the matching
// hub ACK, never the value, body, a view action or a retry outbox entry.
const secretSaves = new SecretSaveHost({
  build: packed => App.secret_req(packed),
  scope: () => JSON.stringify([presentationTarget(currentSession()), currentSession().sel, currentSession().open?.settings === true]),
  binding: () => thin?.client.links.get(location.origin)?.accountBinding || "",
  send: rpc => {
    if (!socket || socket.readyState !== 1) throw Error("Disconnected");
    socket.send(fromList(App.wire_out_json(Wire.json(rpc))));
  },
  dispatch,
  note: text => { const note = document.querySelector(".sbox-note");if (note) note.textContent = text; },
});
function secretSave(packed) { return secretSaves.save(packed); }
setInterval(() => secretSaves.expire(), 250);

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
let keyReplacing = "";
document.addEventListener("input", event => {
  if (event.target.matches?.('[data-sealed="key"]')) keyReplacing = "";
});
function sealedFields(form) {
  return [...document.querySelectorAll("[data-sealed]")].filter(f => f.getAttribute("data-sealed") === form);
}
function sealedSave(form) {
  const fields = sealedFields(form);
  let r;
  try { r = JSON.parse(App.sealed_req([form, ...fields.map(f => f.value ?? "")].join("\u001f"))); } catch { return true; }
  if (r.error) return dispatch("sealed-err", form + "\u001f" + r.error);
  if (form === "key") {
    const warning = App.key_replace(currentSession().drafts?.["@keys"] || "", r.name, document.documentElement.lang);
    if (warning && keyReplacing !== r.name) {
      keyReplacing = r.name;
      return dispatch("sealed-err", form + "\u001f" + warning);
    }
    keyReplacing = "";
  }
  if (!socket || socket.readyState !== 1) return dispatch("sealed-err", form + "\u001f" + "Not connected to the hub");
  if ([...sealedWait.values()].some(w => w.form === form)) return true;
  let rpc;
  try { rpc = JSON.parse(r.rpc); } catch { return true; }
  r = null;
  const id = sealedNext++;
  rpc.id = id;
  sealedWait.set(id, { form, timer: setTimeout(() => sealedDone(id, false, "The hub did not answer"), 15000) });
  socket.send(fromList(App.wire_out_json(Wire.json(rpc))));
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

// Dictation from this page's microphone when the hub is elsewhere
// (voice-mic.js): the mic button's press starts recording at once and tells
// Bend so ("client"); a second press sends what is left with the end mark.
const voiceMic = new VoiceMic();
function voiceMicPress(value) {
  if (hubHere() || !navigator.mediaDevices?.getUserMedia) return value;
  if (voiceMic.active) { voiceMic.stop(); return value; }
  voiceMic.start().catch(err => console.warn("dictation: no microphone", err));
  return "client";
}
// The page owns its draft: dictation's words are placed into it here, by
// the same Bend def as everywhere (client.bend's Voice.place); what that
// keeps between messages is kept per thread.
const voicePlace = new Map();
function voiceWords(envelope) {
  if (envelope.t !== "voice" || !["words", "live", "idle"].includes(envelope.ev) || !envelope.thread) return;
  const th = envelope.thread, session = currentSession();
  const field = document.getElementById("composer");
  const owner = document.querySelector("main.thread-view[data-thread]")?.getAttribute("data-thread");
  const here = session?.sel === th && field && owner === th;
  let draft = "";
  try { draft = here ? field.value : (session?.drafts?.[th] ?? localStorage.getItem(DRAFT + th) ?? ""); } catch {}
  const r = Wire.plain(App.voice_place(Wire.json(voicePlace.get(th) || {}), draft, Wire.json(envelope)));
  voicePlace.set(th, r.keep || {});
  if (typeof r.draft !== "string" || r.draft === draft) return;
  if (thin) thin.draftAt(th, r.draft);
  else if (here) dispatch("draft", r.draft);
  else try { if (r.draft) localStorage.setItem(DRAFT + th, r.draft); } catch {}
}
function voiceMicHeard(envelope) {
  voiceWords(envelope);
  if (envelope.t === "reply" && typeof envelope.audio === "string" && envelope.audio) voiceMic.address(envelope.audio);
  if (envelope.t === "reply" && envelope.engine !== undefined && !envelope.audio && voiceMic.active) voiceMic.gone();
  if (envelope.t === "voice" && (envelope.ev === "idle" || envelope.ev === "error")) voiceMic.gone();
}

function dispatch(action, value, timer) {
  headPreview = null;
  if (action === "devices-open" || action === "settings-page" && value === "devices") { void pairingUI.devices();return true; }
  if (action === "voice" && !timer) value = voiceMicPress(value);
  if (action === "fork") reviewPosition.fork(value || currentSession().sel);
  if (action === "secret-save") return secretSave(value);
  if (action === "sealed") return sealedSave(value);
  // Remove also discards an unsaved key kept only in this page.
  if (action === "voice-unkey") for (const field of sealedFields("vkey")) field.value = "";
  if (["send", "send-alt", "detach"].includes(action) && viewMode !== "compatibility") {
    restoreDraftAttachments(currentSession());
    // (each note stays up through the view's own status updates, which
    // cleared it within 0.1 s; the wait's note goes once Send goes)
    if (socket?.readyState === 1 && attachmentsHoldSend(currentSession().sel)) { holdViewNote(App.host_message(ui, "attachment-restoring"), SEND_WAIT);return true; }
    if (viewNote && viewNote === App.host_message(ui, "attachment-restoring")) { clearTimeout(viewNoteTimer);viewNote = "";showViewStatus(""); }
  }
  if (["send", "send-alt"].includes(action)) { historyClear();if (socket?.readyState === 1) dropUnsentBack(); }
  if (action === "side-toggle" || action === "rail-toggle") return foldToggle(action === "side-toggle" ? "side" : "rail");
  if (action === "select" || action === "bot") { composerKeeper.selecting(action === "select" ? value : "");drawerShut(); }
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
    // (value: a query the command palette could not match: the palette
    // closes first, and the search opens running it once it is gone, render)
    if (value && viewMode === "view-v1" && thin?.ready && document.querySelector(".workspace-palette")) { paletteSearch = value;dispatch("ws-palette-close", "");return true; }
    if (viewMode === "view-v1" && thin?.ready) showThinSearch({ query: value || "", rows: [], cursor: "", hasMore: false });
    else showViewStatus("Search requires the current conversation connection.");
    return true;
  }
  // the thread menu's "Search in thread" (value: the thread)
  if (action === "thread-find") {
    document.querySelectorAll("details.thread-actions[open]").forEach(d => d.removeAttribute("open"));
    if (viewMode === "view-v1" && thin?.ready) showThinSearch({ query: "", rows: [], cursor: "", hasMore: false }, value || currentSession().sel);
    else showViewStatus("Search requires the current conversation connection.");
    return true;
  }
  if (viewMode !== "compatibility") {
    if ((action === "send" || action === "send-alt") && (!socket || socket.readyState !== 1)) {
      offlineSend = true;
      showViewStatus(App.host_message(ui, "offline-send"));
      return true;
    }
    if (action === "select" || action === "draft") {
      if (thin) return thin.request(action, value);
      thinSession = choosePresentation(currentSession(), action, value);
      localPresentation(action, value, thinSession);
      return true;
    }
    // An explicit destination does not depend on the previous picture
    // succeeding. ThinHost validates the live greeting and opens the bot.
    if (action === "bot" && thin?.greeted && socket?.readyState === 1) return thin.request(action, value);
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
      const menu = layer ? modelFeedback.choice(action, value, layer.style.display !== "none") : undefined;
      const subs = subsFeedback.choice(action, value);
      if (subs) subsNow();
      // what the press opened the last time, drawn at once (press-feedback.js;
      // Commands: its field takes the keys in this frame)
      // (Settings put away: the thread under it, from its retained picture)
      const back = action === "flag" && value === "settings" && document.querySelector("main.settings-view") ? transitionViews.destination(transitionScope(), presentationTarget(currentSession())) : null;
      const press = pressFeedback.choice(action, value, targetKey(presentationTarget(currentSession())), back ? pictureNode(back.shown) : null);
      if (pressFeedback.active?.kind === "palette" && pressFeedback.active.node) focus = "workspace-query";
      // (a run of tool calls opened or shut at the click: fold-feedback.js)
      const fold = subs ? undefined : foldFeedback.choice(action, value);
      const settled = menu || subs || press || fold ? accepted => { menu?.(accepted);subs?.(accepted);press?.(accepted);fold?.(accepted); } : undefined;
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
  const attachmentThread = App.sel(ui);
  const attachmentPaths = ["send", "send-alt", "detach"].includes(action)
    ? draftAttachmentPaths(Wire.plain(App.presentation(ui)), attachmentThread) : [];
  const r = App.act(ui, action, value);
  if (PERF) PERF.act.push([pnow() - t0, action]);
  ui = r.ui;
  if (attachmentPaths.length) {
    const kept = draftAttachmentPaths(Wire.plain(App.presentation(ui)), attachmentThread);
    void draftAttachments.forget(attachmentThread, attachmentPaths.filter(path => !kept.includes(path))).catch(() => {});
  }
  if (action === "select") try { localStorage.setItem("backplane-selected", value); } catch {}
  mailCheckpoint();
  if (action === "lang" || action === "lang-os") pageLang(App.lang(ui));
  if (action === "subs-shut" || action === "subs-open") {
    try {
      const sep = value.indexOf("\t");
      const thread = sep < 0 ? value : value.slice(0, sep);
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
// a phone's tap card (plot2d.js) on a part: its saved component, read
// once per board version (kicad.components, as the 3D inspector reads it)
const partDetails = new Map();
Plot2d.onDetails((scope, key, ref) => {
  const path = key.split("|").slice(2).join("|");
  if (!scope) return null;
  const at = `${scope}|${path}|${Plot2d.version(key)}`;
  if (!partDetails.has(at)) {
    if (partDetails.size > 8) partDetails.delete(partDetails.keys().next().value);
    partDetails.set(at, viewerRpc.components(scope, { path }).catch(() => { partDetails.delete(at); return null; }));
  }
  return partDetails.get(at).then((data) => data?.components?.find((c) => c.reference === ref) || null);
});
Plot2d.onShow((kind) => dispatch("view-kind", kind));
// a phone's tap card (plot2d.js): "Mention in chat" adds the piece to the
// thread's draft and goes back to the thread
Plot2d.onMention((text) => {
  const session = currentSession(), th = session?.sel;
  if (!th) return;
  const field = document.getElementById("composer");
  let draft = "";
  try { draft = field ? field.value : (session?.drafts?.[th] ?? localStorage.getItem(DRAFT + th) ?? ""); } catch {}
  const next = draft + (draft && !/\s$/.test(draft) ? " " : "") + text + " ";
  if (thin) thin.draftAt(th, next); else dispatch("draft", next);
  dispatch("panel", "");
});
// a tap that probes a part or a net (probe.js) tells the hub, which keeps it
// (client.bend's Probe); the 3D view finds parts on the board's plot
Plot2d.onProbe((value) => { Solid.setProbe(value); dispatch("probe", value); });
Solid.onProbe((value) => { Plot2d.setProbe(value); dispatch("probe", value); const solid = document.querySelector("canvas#solid"); Plot2d.inspectPart(value.startsWith("c\t") ? value.slice(2).split("\t")[0] : "", solid?.dataset.scope || "", solid?.dataset.key ? `0|${solid.dataset.key.slice(2)}` : ""); });
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
const draftAttachments = new DraftAttachments({ identity: () => cacheIdentity(document.cookie) });
setInterval(() => { void draftAttachments.prune().catch(() => {}); }, 60000);
let attachmentController, attachmentView, attachmentActor, attachmentSelection;
let attachmentIdentity = "", attachmentIdentityPending = true, attachmentIdentitySince = Date.now(), attachmentIdentityGeneration = 0, attachmentIdentityRetry = 0, attachmentIdentityCheck = Promise.resolve();
// Send and file removal wait for the view, the pairing check and a saved
// batch's restore, but the last two for at most SEND_WAIT each: a pairing
// check or a restore that never finished (an IndexedDB the browser never
// opened, a hub that never answered) refused every Send. Past the wait,
// Send goes without: an unverified pairing restores nothing (its files stay
// saved for a verified one), and a stalled batch is abandoned, said once.
function attachmentsHoldSend(thread) {
  if (!thin?.ready && !thin?.opening) return true;
  if (attachmentIdentityPending && Date.now() - attachmentIdentitySince < SEND_WAIT) return true;
  if (draftAttachments.pending(thread)) return true;
  if (draftAttachments.stalled(thread)) {
    draftAttachments.abandon(thread);trace("attachment-restore", { state: "abandoned" });
    holdViewNote(App.host_message(ui, "attachment-unrestored"), 8000);
  }
  return false;
}
function draftAttachmentPaths(session, thread) {
  return (session.drafts?.["@att:" + thread] || "").split("\n").filter(Boolean);
}
function restoreDraftAttachments(session) {
  const thread = session.sel, controller = thin, view = controller?.client.links.get(controller.hub)?.viewId;
  if (!controller?.ready || !attachmentIdentity || attachmentIdentity !== cacheIdentity(document.cookie) || session.drafts?.["@att.up:" + thread]) return;
  if (attachmentController !== controller || attachmentView !== view) {
    attachmentController = controller;attachmentView = view;attachmentActor = {};
  }
  attachmentSelection = thread;
  if (draftAttachments.started.get(thread) === attachmentActor) return;
  // (an abandoned batch's upload under way stops before it reaches the draft)
  const actor = attachmentActor, scope = { ...uploadScope(), live: () => draftAttachments.restores(thread, actor) };
  void draftAttachments.sync(thread, draftAttachmentPaths(session, thread),
    (file, previous) => upload("attach", file, scope, previous),
    () => thin === scope.controller && thin.ready && currentSession().sel === thread
      && thin.epoch === scope.epoch && thin.client.links.get(thin.hub)?.viewId === scope.viewId, actor).catch(() => {});
}
function uploadScope() {
  const controller = viewMode === "compatibility" ? null : thin;
  return { mode: viewMode, controller,
    thread: viewMode === "compatibility" ? App.sel(ui) : currentSession().sel,
    target: controller?.target, epoch: controller?.epoch,
    viewId: controller?.client.links.get(controller.hub)?.viewId,
    identity: cacheIdentity(document.cookie), project: document.querySelector("main.thread-view[data-project]")?.dataset.project || "" };
}
async function readDiagnostics() {
  const response = await fetch('/debug/diagnostics.json', { credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw Error('Hub diagnostics are unavailable. Uncheck diagnostics to continue.');
  const text = await response.text();
  if (new TextEncoder().encode(text).length > 1048576) throw Error('Hub diagnostics exceeded the export limit.');
  return JSON.parse(text);
}
let improvementsVersion = "unknown";
const improvements = installImprovements({
  rpc: improvementsRpc,
  reportBody: (...args) => App.issue_report(...args),
  reportTarget: (saved, legacy) => App.issue_target(saved, legacy),
  reportPrompt: (...args) => App.improvements_prompt(...args),
  reportTemplate: (...args) => App.improvements_template(...args),
  previewBody: body => Wire.plain(App.improvements_preview_body(body)),
  appVersion: async () => window.backplaneDesktop?.version ? (await window.backplaneDesktop.version()).service : improvementsVersion,
  diagnostics: {
    enabled: () => connectionLog.enabled(),
    setEnabled: async on => {
      const results = await Promise.allSettled([Promise.resolve().then(() => connectionLog.setEnabled(on)),
        window.backplaneDesktop?.diagnostics ? window.backplaneDesktop.diagnostics(on ? "on" : "off") : Promise.resolve()]);
      if (results.some(r => r.status === "rejected")) throw Error("Collection stopped for this page where possible; could not persist every diagnostics preference. Retry the control before restarting.");
    },
    clear: async () => { connectionLog.clear();if (window.backplaneDesktop?.diagnostics) await window.backplaneDesktop.diagnostics("clear"); },
    hubStatus: async () => (await improvementsRpc.request("diagnostics.status", {})),
    controlHub: async action => { const status = await improvementsRpc.request("diagnostics.control", { action });if (!status.ok) throw Error("Could not save hub diagnostics preference. " + status.error);return status; },
    snapshot: async () => {
      let hub = null, desktop = [];
      if (remoteReady) hub = await readDiagnostics();
      if (window.backplaneDesktop?.diagnostics) desktop = (await window.backplaneDesktop.diagnostics("read")).records || [];
      return { connections: [...desktop, ...connectionLog.snapshot()], hub };
    },
  },
  ready: () => remoteReady && !!attachmentIdentity && !attachmentIdentityPending,
  identity: () => attachmentIdentity,
  context: async () => {
    const origin = improvementOrigin({ session: currentSession(), doc: document });
    const result = await improvementsRpc.request("improvements.context", { sourceThread: origin.thread });
    improvementsVersion = result.appVersion || "unknown";
    return improvementContext(origin, result);
  },
  beforeCapture: async () => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    await desktopBrowser.forCapture?.();
  },
  overlayChanged: () => desktopBrowser.sync(),
  select: async thread => { if (!dispatch("select", thread)) throw Error("The request was sent. Open the new thread in the sidebar."); },
});
// Native handoff keeps user-authored text out of request/query logs. Consume
// once and remove it before any navigation, capture or report dispatch.
let pendingImprovementEntry = null;
function enterImprovements() {
  if (!pendingImprovementEntry || !remoteReady || !attachmentIdentity || attachmentIdentityPending) return;
  const entry = pendingImprovementEntry;pendingImprovementEntry = null;void improvements.open(entry.target, entry.seed);
}
const issueFragment = location.hash.startsWith('#improvements-issue=') ? location.hash.slice(20) : null;
const internalImprovements = location.hash === '#improvements-internal' || new URLSearchParams(location.search).get('improvements') === 'internal';
if (issueFragment !== null || internalImprovements) {
  const clean = new URL(location.href);clean.hash = '';clean.searchParams.delete('improvements');history.replaceState(history.state, '', clean.href);
  let seed;
  if (issueFragment !== null) try {
    if (issueFragment.length <= 360000) { const value = JSON.parse(decodeURIComponent(issueFragment));if (typeof value.description === 'string' && value.description.length <= 120000) seed = value.description; }
  } catch {}
  const wanted = internalImprovements ? 'internal' : 'issue';
  // Trigger after host initialization. This creates no thread or model job.
  pendingImprovementEntry = { target: wanted, seed };
  queueMicrotask(enterImprovements);
}
async function uploadFiles(action, files, scope = uploadScope()) {
  if (!remoteReady && viewMode !== "compatibility" || viewMode === "compatibility" && socket?.readyState !== 1) {
    offlineAttach = true;
    showViewStatus(App.host_message(ui, "offline-attachment"));
    return;
  }
  for (const file of files) await upload(action, file, scope);
}
async function upload(action, file, scope = uploadScope(), previous = "") {
  await attachmentIdentityCheck;
  const key = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
  const name = file.name || "pasted." + ((file.type || "").split("/")[1] || "bin");
  const { controller: uploadController, thread } = scope;
  const current = () => cacheIdentity(document.cookie) === scope.identity && viewMode === scope.mode && (!scope.live || scope.live()) && (uploadController
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
  } catch {
    answer = JSON.stringify({ uiError: "upload.network", thread, upload: true });
  }
  if (!current()) return;
  // A primitive or malformed body cannot complete an upload. Keep the
  // failure tied to its original actor so the pending indicator clears.
  let result;try { result = JSON.parse(answer); } catch { result = null; }
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    result = { uiError: "upload.failed", thread, upload: true };
    answer = JSON.stringify(result);
  }
  if (viewMode !== "compatibility" && result.attached && result.thread === thread) {
    try {
      if (!attachmentIdentity || attachmentIdentity !== scope.identity) throw Error("Attachment identity unverified");
      await draftAttachments.remember(thread, result.attached, file, previous, scope.project);
    } catch {
      showViewStatus(App.host_message(ui, "attachment-reload"));
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

// A project's head drags (to reorder projects), so its name (the project's
// home) and its badge (the colour and icon picker) act at the click. What
// they open is drawn at the mouse press all the same, as a preview with
// nothing sent: the home's retained picture (with the sidebar on screen),
// the picker as the hub drew it for the same press. The release sends the
// action (the hub's answer then draws the same in place); a drag starting
// puts the page back as it was, and nothing is sent.
const HEAD_PRESS = 'aside.sidebar .project-head [data-on-click="proj-home"], aside.sidebar .project-head [data-on-click="look"]';
function headUndo() {
  if (!headPress) return;
  const p = headPress;headPress = null;
  if (p.look) pressFeedback.drop();
  if (headPreview || p.look) { headPreview = null;later();renderNow(); }
}
document.addEventListener("pointerdown", e => {
  if (!e.isTrusted || e.button !== 0 || e.pointerType !== "mouse" || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
  const el = e.target.closest?.(HEAD_PRESS), value = el?.getAttribute("data-value");
  if (!el || !value || !frontendNode || !remoteReady) return;
  if (el.getAttribute("data-on-click") === "proj-home") {
    // (the picture the click's selection draws: its scope names the home as selected)
    const shown = transitionViews.get({ ...transitionScope({ kind: "project", id: value }), selected: "project-home-" + value, settings: false, route: null });
    if (!shown || shownFor === "project:" + value) return;
    // (drawn as the click will draw it: with the sidebar on screen)
    const before = transitionFor;transitionFor = "project:" + value;
    try { headPreview = { node: withLiveSidebar(pictureNode(shown)) }; } finally { transitionFor = before; }
    headPress = { action: "proj-home", value };
  } else {
    headPress = { action: "look", value, look: true };
    pressFeedback.choice("look", value, targetKey(presentationTarget(currentSession())));
    if (!pressFeedback.showing) { headPress = null;return; }
  }
  later();renderNow();
}, true);
// A release with no drag is the click: its action goes then (what the press
// drew may lie under the pointer, the picker's shade: the browser's own
// click lands there, and is swallowed); dispatch takes over from the preview.
document.addEventListener("dragstart", headUndo, true);
document.addEventListener("pointercancel", headUndo, true);
document.addEventListener("pointerup", e => {
  const p = headPress;
  if (!p || e.button !== 0) return;
  headPress = null;headClick = { at: e.timeStamp, x: e.clientX, y: e.clientY };
  dispatch(p.action, p.value);later();renderNow();
}, true);
document.addEventListener("click", e => {
  const c = headClick;headClick = null;
  if (c && e.timeStamp - c.at < 1000 && Math.abs(e.clientX - c.x) < 8 && Math.abs(e.clientY - c.y) < 8) { e.preventDefault();e.stopImmediatePropagation(); }
}, true);
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
  shotsMissing(document);
  messageImages(document);
}
installShotMissing(document);

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

// The composer's edits while a send's acknowledgement is on its way
// (thin-host.js, sent-claims.js): where each one starts, so text the send
// took comes off the field only if the person left it alone, and IME
// compositions, after which what is due comes off
document.addEventListener("beforeinput", (e) => {
  const el = e.target;
  if (el?.id === "composer") thin?.touched?.(editStart(e.inputType, el.value, el.selectionStart, el.selectionEnd));
}, true);
document.addEventListener("compositionstart", (e) => {
  const el = e.target;
  if (el?.id !== "composer") return;
  composerComposing = true;
  thin?.touched?.(Math.min(el.selectionStart, el.selectionEnd));
}, true);
document.addEventListener("compositionend", (e) => {
  if (e.target?.id !== "composer") return;
  composerComposing = false;
  // after the composition's last input event has saved the draft
  setTimeout(() => { if (!composerComposing) thin?.composed?.(); }, 0);
}, true);

// text typed into a key field without a key event (a phone keyboard)
document.addEventListener("input", (e) => {
  const el = e.target.closest?.("[data-text]");
  if (!el || !el.value) return;
  const v = el.value;
  el.value = "";
  dispatch(el.getAttribute("data-text"), forKey(el) + v);
});

// Commands narrowed at each key to the rows on screen that hold what is
// typed, until the hub's list for that query lands (its ranking and its
// rows from beyond the 24 shown are the hub's): the hub took 75 ms + a
// render + the round trip before anything changed.
function narrowPalette(text) {
  const q = text.trim().toLocaleLowerCase(), rows = [...document.querySelectorAll("#workspace-commands .workspace-command")];
  const hides = rows.map(row => !!q && !row.textContent.toLocaleLowerCase().includes(q));
  // (none of them holds it: the list stays as it is for the hub's, never empty)
  if (hides.every(Boolean)) return;
  rows.forEach((row, i) => { if (hides[i] !== row.hasAttribute("data-narrowed")) row.toggleAttribute("data-narrowed", hides[i]); });
}
function paletteCaughtUp() {
  const field = document.getElementById("workspace-query");
  if (!field?.__v || !document.querySelector("#workspace-commands [data-narrowed]")) return;
  if ((nodeAttr(field.__v, "value") ?? "") !== field.value) return;
  for (const row of document.querySelectorAll("#workspace-commands [data-narrowed]")) row.removeAttribute("data-narrowed");
}
// A thread or bot in the sidebar is picked at the mouse press, not at its
// release: a person holds a click 60-120 ms, and the switch (its picture
// drawn from the client's cache) is under way meanwhile. So are the viewer
// rail's tabs, the sidebar's shelves and the composer's pickers (as a
// native select or a browser's tab does). Its click then does nothing more. Touch keeps the tap (a press
// may start a scroll); project heads, which drag, keep the click.
const PRESS_PICKS = ['aside.sidebar [data-on-click="select"]', 'aside.sidebar [data-on-click="bot"]',
  '.workspace-bar button[data-on-click="view-kind"]', '.workspace-bar button[data-on-click="fm"]', '.workspace-bar button[data-on-click="diff"]',
  '.workspace-bar button[data-on-click="mech"]', '.workspace-bar button[data-on-click="rv-page"]',
  'button.model-chip[data-on-click="flag"]', 'button.tok-chip[data-on-click="flag"]', 'button.account-chip[data-on-click="acct-open"]',
  'button.gear[data-on-click="flag"]', 'button[data-on-click="ws-palette-open"]', 'button.find-open[data-on-click="thin-search-open"]',
  // (the Files tab's folders, its path and Up: a folder seen before is drawn at the press)
  'aside#viewer .fm-list button[data-on-click="fm-cd"]', 'aside#viewer .fm-bar button[data-on-click="fm-cd"]', 'aside#viewer .fm-bar button[data-on-click="fm-up"]',
  // (a run of tool calls' head, the sidebar's shelves and Settings'
  // categories, as disclosures do)
  '#timeline button.fold[data-on-click="fold"]', 'aside.sidebar button.shelf[data-on-click="toggle-settled"]', 'aside.sidebar button.shelf[data-on-click="side-settled"]',
  'aside.sidebar button.shelf[data-on-click="side-older"]', 'aside.sidebar button.shelf[data-on-click="thread-history"]', '#settings-content [data-on-click="settings-page"]', '.settings-path [data-on-click="settings-page"]', '.settings-side [data-on-click="settings-page"]', '.topbar .crumbs [data-on-click="settings-page"]'].join(", ");
let pressedPick = null, pressedMenu = null, pickedTimer = 0;
// A thread row pressed with no retained picture of it draws nothing new
// until the hub's view: the row at least is marked the chosen one at once
// (style.css), until that view (or 5 s) puts the hub's own mark back.
function markPicked(el) {
  if (el.getAttribute("data-on-click") !== "select" || !el.classList.contains("thread")) return;
  if (transitionViews.destination(transitionScope(), { kind: "thread", id: el.getAttribute("data-value") })) return;
  pickRow(el);
}
function pickRow(el) {
  if (el.hasAttribute("data-picked") && pickedTimer) return;
  unmarkPicked();
  el.setAttribute("data-picked", "");htmlData("picking", "1");
  pickedTimer = setTimeout(unmarkPicked, 5000);
}
function unmarkPicked() {
  if (!pickedTimer) return;
  clearTimeout(pickedTimer);pickedTimer = 0;
  for (const row of document.querySelectorAll("[data-picked]")) row.removeAttribute("data-picked");
  htmlData("picking", "");
}
document.addEventListener("pointerdown", e => {
  if (!e.isTrusted || e.button !== 0 || e.pointerType !== "mouse" || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
  // a menu (.local-menu: thread ▾, Views, the viewer's pickers) opens or
  // shuts at the press too; its click's own toggle is then held back
  const summary = e.target.closest?.(".local-menu > summary");
  if (summary) { const menu = summary.parentElement;menu.open = !menu.open;pressedMenu = { menu, at: e.timeStamp };return; }
  const el = e.target.closest?.(PRESS_PICKS);
  if (!el || el.disabled || el.closest('[draggable="true"]') || el.closest("aside.sidebar") && !el.getAttribute("data-value") && !el.classList.contains("shelf")) return;
  pressedPick = { action: el.getAttribute("data-on-click"), value: el.getAttribute("data-value"), at: e.timeStamp, x: e.clientX, y: e.clientY };
  // (Search's field takes the keyboard now: the press's own focus would take it back)
  if (pressedPick.action === "thin-search-open") e.preventDefault();
  markPicked(el);
  dispatch(phoneClickAction(document, el, el.getAttribute("data-on-click")), valueOf(el));
  renderNow();
}, true);
document.addEventListener("click", e => {
  const p = pressedMenu;pressedMenu = null;
  if (p && e.target.closest?.(".local-menu > summary")?.parentElement === p.menu && e.timeStamp - p.at < 2000) e.preventDefault();
}, true);
for (const ev of EVENTS) {
  document.addEventListener(ev, (e) => {
    const el = e.target.closest?.(`[data-on-${ev}]`);
    if (ev === "click" && pressedPick) {
      const p = pressedPick;pressedPick = null;
      // (the same row, drawn again or not since the press; or whatever is
      // there now: the hub's view may have landed before the click did, and
      // a tab that opened the viewer is by then the one that shuts it)
      if (e.timeStamp - p.at < 2000 && (el && el.getAttribute("data-on-click") === p.action && el.getAttribute("data-value") === p.value || e.detail > 0 && Math.abs(e.clientX - p.x) < 8 && Math.abs(e.clientY - p.y) < 8)) return;
    }
    if (!el) return;
    const action = el.getAttribute(`data-on-${ev}`);
    if (ev === "click" && action === "copy") {
      e.preventDefault();
      // Copy payloads are literal: a shell comment starting with # is text,
      // never valueOf's field-reference syntax. No hub action is needed.
      void copyButton(el, el.getAttribute("data-value") ?? "");
      return;
    }
    // a row's menu opens where the pointer was (view.bend's View.rmenu
    // reads these)
    // a file link (dom.bend's Dom.file) goes to the hub, not to its href
    // The optional star suggestion is a real external link. Keep the
    // browser's user-gesture navigation while dispatch saves its decision.
    if (ev === "click" && el.tagName === "A" && action !== "star-open") e.preventDefault();
    if (ev === "contextmenu") {
      e.preventDefault();
      document.documentElement.style.setProperty("--mx", Math.min(e.clientX, innerWidth - 216) + "px");
      document.documentElement.style.setProperty("--my", e.clientY + "px");
    }
    if (ev === "input" && el.id === "workspace-query") narrowPalette(el.value);
    dispatch(ev === "click" ? phoneClickAction(document, el, action) : action, ev === "input" ? inputValue(el) : valueOf(el));
    // (a row's menu, at the right press, in this frame; so is what a tap
    // on what a mouse acts on at the press picks (the phone's list rows too),
    // and a send's bubble)
    if (ev === "contextmenu" || ev === "click" && (e.pointerType === "touch" && (el.matches(PRESS_PICKS) || action === "select" || action === "bot") || action === "send")) renderNow();
    // A mouse send or stop leaves the next keystroke in the message box.
    if (ev === "click" && el.closest(".composer") && ["send", "send-alt", "interrupt"].includes(action)) composerKeeper.activate(e);
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
// A [data-wheel] element (a browser frame, a terminal) takes the wheel
// itself. Its listener is its own (setAttrs hooks it), never the
// document's: a non-passive wheel listener on the document makes every
// wheel on the page wait for the main thread before it may scroll (the
// timeline's wheel→frame went from about 10 to 30-50 ms).
const wheelHooked = new WeakSet();
function wheelHook(el) {
  if (wheelHooked.has(el)) return;
  wheelHooked.add(el);
  el.addEventListener("wheel", (e) => {
    if (!el.hasAttribute("data-wheel") || e.target.closest?.("[data-wheel]") !== el) return;
    e.preventDefault();
    frameFlush(el);
    dispatch(el.getAttribute("data-wheel"), String(Math.round(Math.max(-600, Math.min(600, e.deltaY)))));
  }, { passive: false });
}
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
    // Clipboard chords stay native; forwarding Ctrl+V quotes the shell's
    // next key instead of letting the paste event carry the clipboard.
    if (e.ctrlKey && !e.altKey && (e.key.toLowerCase() === "v" || e.shiftKey && e.key.toLowerCase() === "c")) return;
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
  if (!action || phoneKeepsReturn(document, el, action, e)) return;
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
  // The tour drops keypress below, which also cancels Enter's native
  // button click. Press it here once; Space still clicks on release.
  if (onButton && e.key === "Enter") {
    e.preventDefault();
    if (!e.repeat) document.activeElement.click();
    return;
  }
  if (onButton && e.key === " ") {
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
let laterAsked = false, historyFrame = false, historyDirection = "", historyTouchY = null, historyFillScope = "", historyOlderDemand = 0;
// How far from the edge of the rows drawn the next page is asked for: two
// screens, or the distance the reader covers at their speed (the last
// quarter second's scrolling) in twice the time a page has been taking,
// up to twelve screens. A fast wheel asks early enough that the page is in
// before the edge comes.
let historyMoves = [];
function historyMoved(dy) {
  const now = performance.now();
  historyMoves.push([now, Math.abs(dy)]);
  while (historyMoves.length && now - historyMoves[0][0] > 250) historyMoves.shift();
}
// px ahead of the reader the timeline mounts rows (the planner's overscan
// input): what a quarter second at their speed covers, up to three screens,
// so a fast wheel that overtakes a busy frame still finds rows drawn
function historyOverscan(view) {
  const now = performance.now();
  let moved = 0;
  for (const m of historyMoves) if (now - m[0] <= 250) moved += m[1];
  return Math.round(Math.min((view || 800) * 3, moved));
}
function historyRunway(tl) {
  const now = performance.now(), recent = historyMoves.filter(m => now - m[0] <= 250);
  const speed = recent.reduce((a, m) => a + m[1], 0) / 250;
  return Math.min(tl.clientHeight * 12, Math.max(tl.clientHeight * 2, speed * 2 * Math.max(250, thin?.pageMs || 500)));
}
// A reader faster than three screens of runway gets two older pages on
// their way at once (thin-host.js's ahead {two}): one page at a time cannot
// keep up with a fast wheel, however early it is asked for.
function historyFillOwner() {
  const link = thin?.client.links.get(thin.hub);
  if (!thin?.ready || !link?.viewId) return "";
  return JSON.stringify([readingIdentity(), thin.hub, link.greeting.generation, link.greeting.epoch, link.viewId, thin.selectionRevision, thin.target, thin.session.sel, thin.historyEpoch]);
}
function historyHasMessage(tl) {
  const box = tl.getBoundingClientRect();
  for (const row of tl.querySelectorAll('.msg[data-id]:not(.live):not(.working):not(.sending)')) {
    const rect = row.getBoundingClientRect();
    if (!row.dataset.id.startsWith("@live:") && row.textContent.trim() && rect.height > 0 && rect.bottom > box.top && rect.top < box.bottom) return true;
  }
  return false;
}
// A short tool-only page may have no native scroll range. Its explicit
// control is another navigation gesture; rendering/live updates never grant
// an older-page demand or walk the conversation in the background.
function historyFillMore(tl, shown) {
  let button = document.getElementById("history-fill-more");
  if (!shown) { button?.remove();return; }
  if (!button) {
    const owner = historyFillScope;
    button = document.createElement("button");button.id = "history-fill-more";button.type = "button";
    button.textContent = "Load earlier messages";button.setAttribute("aria-controls", "timeline");
    button.onclick = () => {
      if (owner !== historyFillOwner() || !thin?.ready || thin.paging || thin.pageError || !thin.hasAhead("older")) return;
      historyDirection = "older";historyOlderDemand = 2;earlierAsked = false;historyCheckSoon();
    };
    document.body.appendChild(button);
  }
  const box = tl.getBoundingClientRect();
  button.style.cssText = "position:fixed;z-index:12;max-width:calc(100vw - 32px);background:var(--panel);color:var(--text);border:1px solid var(--line);padding:6px 10px;min-height:36px";
  button.style.top = Math.max(0, box.top + 8) + "px";button.style.left = Math.max(8, box.left + 8) + "px";
}
function earlierCheck(tl) {
  if (!tl || tl.id !== "timeline" || !thin?.ready || thin.pageError) { historyFillMore(tl, false);return; }
  const owner = historyFillOwner();
  if (!owner) { historyFillMore(tl, false);return; }
  if (!historyFillScope) historyFillScope = owner;
  else if (owner !== historyFillScope) { historyFillScope = owner;historyOlderDemand = 0;historyFillMore(tl, false);return; }
  if (entryHistory.active(owner)) {
    const usable = tl.scrollHeight > tl.clientHeight + 10 && historyHasMessage(tl);
    if (entryHistory.check(owner, thin, usable)) { historyFillMore(tl, false);return; }
    // The accepted newest page has landed; reconcile following and the read
    // boundary even if its geometry did not produce another scroll event.
    const list = timelineList(tl);if (list) timelineChanged(list);
  } else entryHistory.cancel();
  // (a timeline with no box, e.g. behind the phone's full-screen viewer, has
  // no range either: its fixed button would sit over that screen's header)
  historyFillMore(tl, thin.hasAhead("older") && tl.clientHeight > 0 && (tl.scrollHeight <= tl.clientHeight + 10 || !historyHasMessage(tl)));
  if (historyDirection !== "older" || historyOlderDemand <= 0) return;
  const gap = historyGap("older"), runway = historyRunway(tl), two = historyOlderDemand > 1 && runway > tl.clientHeight * 3;
  if (thin.paging ? !two || !!thin.pagingNext : earlierAsked) return;
  if (!thin.hasAhead("older") || tl.scrollTop - gap > runway * (gap ? 2 : 1)) return;
  const asked = thin.ahead("older", { two });
  if (asked) historyOlderDemand = Math.max(0, historyOlderDemand - (two ? 2 : 1));
  earlierAsked = asked || earlierAsked;
}
function laterCheck(tl) {
  if (historyDirection !== "newer" || laterAsked || !tl || !thin?.ready || thin.paging || thin.pageError || !thin.hasAhead("newer")) return;
  const gap = historyGap("newer");
  if (tl.scrollHeight - tl.scrollTop - tl.clientHeight - gap > historyRunway(tl) * (gap ? 2 : 1)) return;
  laterAsked = thin.ahead("newer");
}
// (the re-render harness, test/tools/rerender: why a wheel at the edge did or did not ask)
if (RR) RR.paging = () => {
  const tl = document.getElementById("timeline");
  return { ready: !!thin?.ready, paging: !!thin?.paging, second: !!thin?.pagingNext, pageError: thin?.pageError || null, pending: !!thin?.pageActionPending?.(),
    earlierAsked, laterAsked, direction: historyDirection, older: !!thin?.hasAhead?.("older"), newer: !!thin?.hasAhead?.("newer"),
    top: tl?.scrollTop ?? -1, gap: historyGap("older"), runway: tl ? Math.round(historyRunway(tl)) : 0, status: document.getElementById("history-status")?.textContent || "" };
};
function historyCheckSoon() {
  if (historyFrame) return;
  historyFrame = true;
  requestAnimationFrame(() => {
    historyFrame = false;
    const tl = document.getElementById("timeline");
    earlierCheck(tl);laterCheck(tl);
  });
}
// The rows a gap stands for are asked for well before it (twice the
// distance); a reader faster still stops at its edge, as at the end, and
// never sees into it. (Capture: before the timeline's own plan reads it.)
document.addEventListener("scroll", e => {
  const tl = e.target;
  if (tl?.id !== "timeline" || !historyGone.older.size && !historyGone.newer.size) return;
  const older = historyGap("older"), newer = historyGap("newer");
  if (older && tl.scrollTop < older) tl.scrollTop = older;
  else if (newer && tl.scrollTop + tl.clientHeight > tl.scrollHeight - newer) tl.scrollTop = Math.max(older, tl.scrollHeight - newer - tl.clientHeight);
  historyCheckSoon();
}, true);
document.addEventListener("wheel", e => {
  if (!e.isTrusted || e.ctrlKey || !e.target.closest?.("#timeline")) return;
  const tl = document.getElementById("timeline");
  if (e.deltaY) { entryHistory.cancel();historyFillMore(tl, false); }
  historyDirection = e.deltaY < 0 ? "older" : e.deltaY > 0 ? "newer" : historyDirection;
  if (e.deltaY) historyOlderDemand = e.deltaY < 0 ? 2 : 0;
  historyMoved(e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? tl?.clientHeight || 800 : 1));
  if (e.deltaY < 0) earlierCheck(tl);
  else if (e.deltaY > 0) laterCheck(tl);
}, { capture: true, passive: true });

// A native thumb drag may start before it changes scrollTop. Retire entry
// work at the press, including a tool expansion, so it cannot take the next
// viewport back to the end while the person starts reading.
document.addEventListener("pointerdown", e => {
  if (e.isTrusted && e.target.closest?.("#timeline")) entryHistory.cancel();
}, { capture: true, passive: true });

for (const event of ["touchstart", "touchmove", "keydown"]) document.addEventListener(event, e => {
  if (!e.isTrusted || !e.target.closest?.("#timeline") && !(event === "keydown" && e.target === document.body)) return;
  if (event !== "keydown" || ["PageUp", "Home", "ArrowUp", "PageDown", "End", "ArrowDown", " "].includes(e.key)) entryHistory.cancel();
  if (event === "touchstart") historyTouchY = e.touches[0]?.clientY ?? null;
  else if (event === "touchmove") {
    const y = e.touches[0]?.clientY;
    if (typeof y === "number" && typeof historyTouchY === "number") { if (y !== historyTouchY) { historyFillMore(null, false); }historyDirection = y > historyTouchY ? "older" : y < historyTouchY ? "newer" : historyDirection;if (y !== historyTouchY) historyOlderDemand = y > historyTouchY ? 2 : 0;historyMoved(y - historyTouchY); }
    historyTouchY = y;
  } else if (["PageUp", "Home", "ArrowUp"].includes(e.key)) { historyFillMore(null, false);historyDirection = "older";historyOlderDemand = 2; }
  else if (["PageDown", "End", "ArrowDown", " "].includes(e.key)) { historyFillMore(null, false);historyDirection = "newer";historyOlderDemand = 0; }
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
  if (which === "side" && document.documentElement.dataset.fold) {
    sideKept = !sideKept;
    panes();
    return true;
  }
  const on = !document.documentElement.dataset[which];
  foldSet(which, on);
  try { localStorage.setItem(FOLDS[which][0], on ? "1" : "0"); } catch {}
  // a sidebar opened by hand while the viewer is open stays open
  if (which === "side") sideKept = !on;
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
const viewerKey = () => {
  const base = splitOn() ? VIEWER_W_SPLIT : VIEWER_W;
  const project = document.querySelector("#viewer[data-split-project]")?.dataset.splitProject;
  return project ? base + ":" + JSON.stringify([thin?.hub || location.origin, project]) : base;
};
function viewerWidth(w, s, side = document.documentElement.dataset.fold ? s.strip : s.side) {
  const root = document.documentElement;
  if (w == null) { root.style.removeProperty("--viewer-w"); return; }
  root.style.setProperty("--viewer-w", `${Math.round(w)}px`);
  const edge = document.querySelector(".viewer-split");
  if (edge) {
    const at = drag => Number(App.panes_viewer(panesN(s.w), panesN(side), panesN(s.strip), panesN(s.rail), panesN(drag), splitOn(), true));
    for (const [name, value] of [["aria-valuemin", at(1)], ["aria-valuemax", at(s.w)], ["aria-valuenow", Math.round(w)]]) {
      if (edge.getAttribute(name) !== String(value)) edge.setAttribute(name, String(value));
    }
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
  // A home embeds the viewer inside main. Reserve no third column or
  // automatic sidebar fold for a second viewer beside that main area.
  if (document.querySelector(".app > main.project-home")) {
    sideKept = false; drawerSel = null;
    delete root.dataset.fold;
    root.style.removeProperty("--viewer-w");
    return;
  }
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
  viewerWidth(Number(App.panes_viewer(panesN(s.w), panesN(s.side), panesN(s.strip), panesN(s.rail), panesN(viewerDrag()), splitOn(), sideKept)), s);
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
    viewerWidth(w, s, side);
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
  } else if (e.key === "End" || e.key === "ArrowLeft" || e.key === "ArrowRight") {
    e.preventDefault();
    const s = panesSize(), side = document.documentElement.dataset.fold ? s.strip : s.side;
    const delta = (e.shiftKey ? 64 : 16) * (e.key === "ArrowLeft" ? 1 : -1);
    const w = Number(App.panes_viewer(panesN(s.w), panesN(side), panesN(s.strip), panesN(s.rail),
      panesN(e.key === "End" ? s.w : Math.max(1, edge.parentElement.getBoundingClientRect().width + delta)), splitOn(), true));
    viewerWidth(w, s, side);
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
// the session or an input as Bend's Json, built in JavaScript (wire.js's
// json(): what Json.parse gives for its JSON text, without the text)
const presentationJson = value => Wire.json(value);
const choosePresentation = (session, action, value) => {
  const chosen = Wire.plain(App.presentation_choice(action, presentationJson(session), value));
  return action === "draft" ? preservePresentationRoute(session, chosen) : chosen;
};
// Until the ordinary send is accepted, keep its captured text in the saved draft too.
// The visible field is already empty, but a reload before the acknowledgement
// must recover that text. Subsequent typing is saved alongside it.
function persistDraft(thread, text) {
  const pending = thin?.pendingSends(thread) || [];
  const saved = [...pending.filter(p => !p.accepted).map(p => p.text), text].filter(Boolean).join("\n\n");
  try {
    if (saved) localStorage.setItem(DRAFT + thread, saved);
    else localStorage.removeItem(DRAFT + thread);
  } catch {}
}
function localPresentation(action, value, session) {
  if (action === "select" || action === "bot") {
    ui = App.presentation_select(ui, session.sel);
    try { localStorage.setItem("backplane-selected", session.sel); } catch {}
    holdPicture();
    remoteReady = false;
    if (!restoreTransition(presentationTarget(session))) void restoreFocused();
    later();
  } else if (action === "draft") {
    applyComposerDraft(document, session.sel, value, composerComposing);
    ui = App.restore(ui, session.sel, value);
    // the hub's page is redrawn with the draft (app.bend's
    // presentation_overlay: the send button and the slash commands follow
    // what is typed even while the hub is busy with a streaming turn). The
    // textarea already shows each key, so a plain one waits until typing
    // pauses: redrawing a long thread's whole page per key is what made
    // typing lag, on phones most. A key that empties or fills the draft, or
    // types in a word that starts a popup (/ $ @ > % #), redraws at once.
    if (frontendNode) paintDraft(session.sel, value);
    persistDraft(session.sel, value);
  }
}
// A phone's timeline never sits scrolled sideways: style.css hides its
// sideways overflow, and this undoes a scroll something did anyway (iOS
// bringing a focused field into view scrolls hidden overflow too).
const PHONE_WIDTH = typeof matchMedia === "function" ? matchMedia("(max-width: 760px)") : null;
document.addEventListener("scroll", e => {
  const t = e.target;
  if (t && t.id === "timeline" && t.scrollLeft !== 0 && PHONE_WIDTH?.matches) t.scrollLeft = 0;
}, true);
let draftPainted = { sel: "", text: "" }, draftPaintTimer = 0;
// whether the draft's last word (after its last whitespace) starts a popup
// (/ $ @ > % #): /(^|\s)[\/$@>%#]\S*$/, looking at that word alone (the
// regex tried every position of a pasted draft, twice a key)
const draftPopup = text => {
  let i = text.length;
  while (i > 0 && !/\s/.test(text[i - 1])) i--;
  return i < text.length && "/$@>%#".includes(text[i]);
};
function paintDraft(sel, text) {
  const was = draftPainted.sel === sel ? draftPainted.text : null;
  draftPainted = { sel, text };
  clearTimeout(draftPaintTimer);
  if (was === null || !was !== !text || draftPopup(text) || draftPopup(was)) paintComposer();
  else draftPaintTimer = setTimeout(paintComposer, 160);
}
// A draft changes the composer alone (presentation.bend's Node.overlay: the
// textarea's value; view.bend's View.local: the composer's popups, keys and
// send button), so a draft's paint patches that element, made from the hub's
// own composer (View.local compares the draft with the one the hub drew),
// and not the whole page. Anything else still goes through render().
const hubComposers = new WeakMap();
const composerAttr = (v, name) => { for (const a of each(v.attrs)) if (is(a, "At") && a.name === name) return a.value; return null; };
// Ordinary composers use View.local; the home keeps its own Start thread
// controls and shares Node.overlay's local textarea update.
const isComposer = v => is(v, "El") && (composerAttr(v, "data-home-composer") === "1" || composerAttr(v, "class") === "composer" && !!composerAttr(v, "data-lang"));
function hubComposer(node) {
  if (hubComposers.has(node)) return hubComposers.get(node);
  let found = null;
  const stack = [node];
  while (stack.length && !found) {
    const v = stack.pop();
    if (!is(v, "El")) continue;
    if (isComposer(v)) found = v;
    else for (const k of each(v.kids)) stack.push(k);
  }
  hubComposers.set(node, found);
  return found;
}
let composerPainting = false;
function paintComposer() {
  if (queued || composerPainting) return;
  composerPainting = true;
  requestAnimationFrame(() => {
    composerPainting = false;
    if (queued) return;
    const hub = frontendNode && hubComposer(frontendNode);
    const el = hub && [...document.querySelectorAll(".composer[data-lang],.composer[data-home-composer]")].find(e => e.__v && isComposer(e.__v) && e.__v.key === hub.key && e.__v.tag === hub.tag);
    if (!el) { later();return; }
    const t0 = RR ? pnow() : 0;
    patch(el, App.presentation_overlay(hub, presentationJson(feedbackSession())));
    attachmentPreviews();
    composerGeometry();
    if (RR) { RR.n("render.part");RR.t("render.part", pnow() - t0); }
  });
}
function adoptPresentation(session) { ui = App.presentation_handoff(ui, presentationJson(session)); }
function performPresentation(operation) {
  const r = App.presentation_perform(ui, presentationJson(operation.input), operation.action, operation.value);
  ui = r.ui;
  run(r.cmds);
}
function presentationCommand(command) {
  const tag = { send: "Send", copy: "Copy", focus: "FocusField", scroll: "ScrollEnd", connect: "Connect",
    keep: "Keep", notify: "Notify", later: "Later", jump: "Jump" }[command.type];
  if (!tag) throw Error("Unknown presentation command");
  run({ $: "Con", head: { ...command, $: tag }, tail: { $: "Nil" } });
}

// Shared thin transport -> platform IO and the existing normal Bend renderer.
// Neither this path nor its error/retry state touches canonical history.
function showViewStatus(message) {
  clearTimeout(connectingTimer);connectingTimer = 0;
  document.getElementById("view-retry")?.remove();
  let box = document.getElementById("view-connection");
  if (!box) {
    box = document.createElement("aside");box.id = "view-connection";box.setAttribute("role", "status");
    box.style.cssText = "position:fixed;bottom:0;left:0;right:0;z-index:30;display:flex;flex-wrap:wrap;gap:8px;padding:8px;background:var(--panel);border-top:1px solid var(--line)";
    const text = document.createElement("span");text.id = "view-connection-text";box.append(text);
    document.body.append(box);
  }
  // (each written only when it changes: most calls clear a bar already clear)
  if (box.hidden !== !message) box.hidden = !message;
  const display = message ? "flex" : "none", text = document.getElementById("view-connection-text");
  if (box.style.display !== display) box.style.display = display;
  if (text.textContent !== message) text.textContent = message;
  // The page shrinks above the bar instead of sliding under it.
  const room = message ? box.offsetHeight + "px" : "0px";
  if (document.documentElement.style.getPropertyValue("--status-h") !== room) document.documentElement.style.setProperty("--status-h", room);
}


// A view the hub refused (thin-host.js's refuse): a search result or a
// reload that named something it has no view of. The page goes back to
// the thread on show before, else the project list (refusedFallback), says
// so for a while, and keeps nothing of the refused one: the selection
// saved for a reload is the one gone back to.
function refusedView(target, reason) {
  if (target.kind === "thread" && reason === "NOT_FOUND") void draftAttachments.forget(target.id).catch(() => {});
  composerKeeper.navigationFailed();
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
// How long a connect step may take: the base on a first try (a hub that is
// down shows at once), and on a phone twice that after each failed retry, up
// to 4x, so a link whose answers take seconds (weak signal, a busy relay)
// still connects; success or a wake starts over.
const patience = () => PHONE_TITLES() ? Math.min(4, 2 ** Math.max(0, thinBackoff.tries - 1)) : 1;
// a view request's deadline (thin-host.js), and the view failures in a row
// that slow the next attempt (viewRecover)
const VIEW_TIMEOUT_MS = 30000, VIEW_CALM_MS = 120000;
let viewFailures = 0, viewFailedAt = 0;
let downSince = 0, downNote = "", downTimer = 0, offlineSend = false, offlineAttach = false;
// when the try at the hub under way began (a probe, or a connect up to its
// first view), 0 when none is (onWake's wakeAction)
let tryingSince = 0;
// this page has had a view from this hub, and the direct sockets failed
// before their greeting since (directReconnect)
let viewsEver = false, directFails = 0;
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
// The down state is a small note in the corner (reconnectNote): it takes no
// room from the page, nothing in it is pressed, and a link back within
// DOWN_QUIET_MS (a heavy view that held the page, a hub restart) shows
// nothing at all. The page reconnects by itself, however long it takes.
const DOWN_QUIET_MS = 4000;
function reconnectNote(text) {
  let el = document.getElementById("reconnect-note");
  if (!text) { el?.remove();return; }
  if (!el) {
    el = document.createElement("div");el.id = "reconnect-note";el.setAttribute("role", "status");el.setAttribute("aria-live", "polite");
    el.style.cssText = "position:fixed;right:10px;bottom:10px;z-index:30;max-width:min(420px,calc(100vw - 20px));padding:3px 8px;font-size:12px;opacity:.85;pointer-events:none;background:var(--panel);color:var(--muted,var(--text));border:1px solid var(--line)";
    document.body.append(el);
  }
  el.textContent = text;
}
function downStatus(note) {
  downNote = note;
  if (!downSince) downSince = Date.now();
  clearTimeout(downTimer);
  const wait = Math.max(0, downSince + DOWN_QUIET_MS - Date.now());
  downTimer = setTimeout(() => { if (downSince) { reconnectNote(waiting(offlineSend ? App.host_message(ui, "offline-send") : offlineAttach ? App.host_message(ui, "offline-attachment") : downNote));connDot(true); } }, wait);
}
// A Send pressed while the link was down went nowhere: once it is back the
// bar says so for a while (the "Not sent" note would just vanish, leaving
// the text in the composer looking sent), until the next Send.
let unsentBack = "";
function holdViewNote(text, ms) {
  clearTimeout(viewNoteTimer);
  viewNote = text;
  viewNoteTimer = setTimeout(() => { viewNote = "";unsentBack = "";if (remoteReady) showViewStatus(""); }, ms);
  showViewStatus(viewNote);
}
function dropUnsentBack() {
  if (!unsentBack || viewNote !== unsentBack) return;
  clearTimeout(viewNoteTimer);viewNote = "";unsentBack = "";showViewStatus("");
}
function upStatus() {
  const unsent = offlineSend;
  downSince = 0;downNote = "";tryingSince = 0;clearTimeout(downTimer);downTimer = 0;offlineSend = false;offlineAttach = false;
  thinBackoff.reset();thinRetry.cancel();
  reconnectNote("");showViewStatus("");connDot(false);
  if (unsent) { unsentBack = App.host_message(ui, "offline-send-back");holdViewNote(unsentBack, 20000); }
}
function thinReconnect() {
  if (!pairingAccepted || viewMode === "compatibility" || thinRetry.pending) return;
  downStatus(downNote || "Reconnecting… Your draft is kept.");
  const attempt = thinAttempt;tryingSince = 0;
  thinRetry.schedule(thinBackoff.next(), async () => {
    if (viewMode === "compatibility" || attempt !== thinAttempt) return;
    tryingSince = Date.now();
    if (directReconnect({ again: true, viewsEver, directFails })) { void startThin(true, "reconnect");return; }
    const hub = await probeHub(window.fetch.bind(window), { timeoutMs: 2000 * patience() });
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
  if (!["turn.start", "project.start"].includes(r?.m) || typeof msg !== "string" || !msg) return;
  const thread = r.m === "project.start" ? "project-home-" + r.p.project : String(r.p.thread || "");
  outbox.set(msg, { msg, id: r.id, text, thread, body: String(r.p.text || ""), at: Date.now() });
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
  if (thin?.ordinary.get(o.id)?.preview) { /* The captured send restores alongside newer typing in ThinHost. */ }
  else if (thin && session.sel === o.thread && !session.drafts?.[o.thread]) thin.request("draft", o.body);
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
// (thread: the thread menu's "Search in thread", thread-find: only that
// thread's results, the hub's own first page of them (Search.here in
// core/thin.bend) and then its "More results" pages, those of other
// threads left out; a page with none of its own asks the next, a few)
function showThinSearch(search, thread = "") {
  thinFind = null;
  const { panel, close } = resultPanel(thread ? "Search in thread" : "Search threads", "search-threads");
  const query = document.createElement("input");query.type = "text";query.value = search.query;
  const where = thread ? "Search this thread" : "Search every thread", empty = thread ? "Type a word said in this thread." : "Type a word said in any thread.";
  query.placeholder = where;query.autocomplete = "off";
  query.setAttribute("aria-label", where);query.setAttribute("role", "combobox");
  query.setAttribute("aria-controls", "find-rows");query.setAttribute("aria-expanded", "true");
  const note = document.createElement("div");note.className = "find-note";note.setAttribute("role", "status");
  const rows = document.createElement("div");rows.id = "find-rows";rows.className = "find-rows";rows.setAttribute("role", "listbox");
  const foot = document.createElement("footer");foot.textContent = "↑↓ choose · Enter open · Esc close";
  panel.append(query, note, rows, foot);
  // (a phone: threads whose titles match, above what was said in them)
  const titles = PHONE_TITLES() && !thread ? document.createElement("div") : null;
  if (titles) { titles.className = "find-titles";rows.before(titles); }
  // (the sidebar's rows at once, then the hub's answer's titles, every
  // thread's: Search.titles in server/thin-view.bend)
  const showTitles = (hub = []) => {
    if (!titles) return;
    titles.replaceChildren();
    const seen = new Set(), hits = [];
    for (const hit of [...titleHits(document, query.value), ...hub]) if (hit && typeof hit.id === "string" && !seen.has(hit.id) && hits.length < 8) { seen.add(hit.id);hits.push(hit); }
    for (const hit of hits) {
      const b = document.createElement("button");b.type = "button";b.className = "find-title";
      const t = document.createElement("span");t.className = "find-thread";t.textContent = hit.title;b.append(t);
      if (hit.project) { const p = document.createElement("span");p.className = "find-project";p.textContent = hit.project;b.append(p); }
      b.addEventListener("click", () => { close();thinFind = null;dispatch("select", hit.id);later(); });
      titles.append(b);
    }
  };
  // asked: the search on its way (its query and cursor); next: what to ask
  // once it is answered; shown: the query whose results are on screen
  const f = { asked: null, next: null, shown: null, hits: [], at: -1, group: null, timer: 0, more: null, skipped: 0 };
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
    if (!text.trim()) { f.next = null;f.shown = null;f.hits = [];f.at = -1;f.more = null;rows.replaceChildren();note.textContent = empty;return; }
    // one at a time: an answer lost is given up after a while
    if (f.asked && Date.now() - f.asked.at < 8000) { f.next = { text, cursor };return; }
    f.next = null;f.asked = { text, cursor, at: Date.now() };
    if (!cursor) note.textContent = "Searching…";
    if (!thin?.search(text, cursor)) { f.asked = null;note.textContent = "Search needs the hub: not connected."; }
  };
  const draw = (search, more) => {
    if (!more) { rows.replaceChildren();f.hits = [];f.at = -1;f.group = null;f.skipped = 0; }
    f.more?.remove();f.more = null;
    const had = f.hits.length;
    for (const row of search.rows || []) {
      if (thread && row.thread !== thread) continue;
      if (row.thread !== f.group && !thread) { rows.append(thinFindHead(row));f.group = row.thread; }
      const el = thinFindRow(row, f.hits.length), n = f.hits.length;
      el.addEventListener("click", () => open(row));
      el.addEventListener("mousemove", () => { if (f.at !== n) choose(n); });
      rows.append(el);f.hits.push({ row, el });
    }
    if (thread && f.hits.length === had && search.hasMore && search.cursor && f.skipped < 4) { f.skipped++;f.shown = search.query;ask(search.query, search.cursor);return; }
    if (search.hasMore && search.cursor) {
      const button = document.createElement("button");button.type = "button";button.className = "find-more";button.textContent = "More results";
      button.addEventListener("click", () => { ask(search.query, search.cursor);softFocus(query); });rows.append(button);f.more = button;
    }
    f.shown = search.query;
    if (!more && Array.isArray(search.titles)) showTitles(search.titles);
    note.textContent = f.hits.length || titles?.childElementCount ? "" : thread ? `Nothing in this thread matches “${search.query.trim()}”.` : `Nothing matches “${search.query.trim()}”.`;
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
  // The first key of a burst asks at once (two letters or more); the keys
  // after it wait for a 120 ms pause (ask keeps one search on its way and
  // the newest behind it). Every query waited 180 ms first.
  query.addEventListener("input", () => {
    showTitles();
    const burst = !!f.timer;
    clearTimeout(f.timer);f.timer = 0;
    if (!query.value.trim()) { ask("", "");return; }
    if (!burst && !f.asked && query.value.trim().length >= 2) { ask(query.value, "");f.timer = setTimeout(() => { f.timer = 0; }, 120);return; }
    f.timer = setTimeout(() => { f.timer = 0;ask(query.value, ""); }, 120);
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
  showTitles();
  if (search.rows?.length) draw(search, false);
  else if (search.query) ask(search.query, "");
  else note.textContent = empty;
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
  keepThinReading();
  const previous = thin;thin = null;
  const s = thinSocket;thinSocket = null;
  if (socket === s) socket = null;
  if (s) { s.connectionDiagnostic?.initiate(reason);s.onmessage = null;try { s.close(); } catch {} }
  if (previous) previous.disconnect();
  remoteReady = false;desktopBrowser.hide();ViewerTools.reset();viewerRpc.reset();improvementsRpc.reset();
  // A reconnect (keepView) keeps the boards, schematics and 3D models drawn:
  // the new connection's first plot of each is sent whole, and the same
  // model's bytes are not decoded again (solid-assets.js), so a drop while a
  // heavy view loads neither blanks the viewer nor starts its load over.
  if (!keepView) { Plot2d.reset();Solid.reset();frontendNode = null; }
}
async function startThin(again = false, reason = "initial") {
  if (!pairingAccepted) return;
  if (viewMode === "compatibility") return;
  stopThin(again || downSince > 0, reason);const attempt = thinAttempt;
  viewMode = "discovering";tryingSince = Date.now();
  if (downSince) downStatus(downNote);
  else connectingStatus();
  later();
  const direct = directReconnect({ again: again || downSince > 0, viewsEver, directFails });
  const capability = direct ? { supported: true } : await discoverViews({ fetch: window.fetch.bind(window), validate: viewCapability, timeoutMs: 2000 * patience() });
  if (attempt !== thinAttempt || viewMode === "compatibility") return;
  if (capability.unauthorized) {
    stopThin(false, "auth-denied");viewMode = "read-only";focusedView.invalidate();
    const repair = !!cacheIdentity(document.cookie);
    pairingUI.show({repair, cause: repair ? "pairing_required" : ""});
    void refreshFocusedIdentity();later();return;
  }
  // A hub that served views a moment ago and answers this reconnect with
  // anything but its capabilities (a captive portal's page, a proxy's error,
  // a body cut off as the network went) is not a hub without views: try
  // again, rather than settle for read-only with the down note left up.
  if (!capability.supported && (capability.offline || again || downSince > 0)) {
    viewMode = "offline";later();thinReconnect();return;
  }
  if (!capability.supported) {
    viewMode = "read-only";showViewStatus("This hub needs an update to work with this page. Saved conversation is read-only.");later();return;
  }
  viewMode = "view-v1";viewsEver = true;
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const s = new WebSocket(`${proto}//${location.host}/ws?view=1&frontend=1&enc=cbor&since=0&origin=`);
  thinSocket = s;s.binaryType = "arraybuffer";
  const diagnostic = s.connectionDiagnostic = connectionLog.connection();
  diagnostic.note("created", { cause: reason });
  let ended = false;
  const current = () => !ended && attempt === thinAttempt && viewMode === "view-v1";
  const disconnect = event => {
    if (!current()) return;
    // a socket gone before its first view: the next reconnects ask first
    // once this has happened DIRECT_TRIES times (directReconnect)
    if (direct && !s.up) directFails++;
    previewViews.stop();
    if (typeof event === "string") diagnostic.initiate(event);
    trace("thin-disconnect", { reason: typeof event === "string" ? event : event?.type || "connection-ended", code: event?.code });
    ended = true;
    clearTimeout(attachmentIdentityRetry);attachmentIdentityRetry = 0;
    identityUnverified();attachmentIdentityGeneration++;
    refocusKeep();
    keepSettingsRecovery();
    bubMoves.disconnect(location.origin);
    terminalOutput.clear();
    checkpointFocused();remoteReady = false;desktopBrowser.hide();ViewerTools.reset();viewerRpc.reset();improvementsRpc.reset();
    clearTimeout(thinGreetingTimer);clearInterval(thinExpiry);clearTimeout(thinDraftTimer);clearTimeout(thinFieldTimer);
    keepThinReading();bodyWindows.disconnect();thin?.disconnect();if (socket === s) socket = null;
    secretSaves.disconnect();
    if (!frontendNode) void restoreFocused(true);
    later();thinReconnect();
  };
  // A failed presentation does not invalidate its transport. Preserve the
  // socket/greeting/heartbeat so another thread can open immediately. Real
  // disconnects, missing greetings and heartbeat failures have their own
  // transport handlers below; never turn a renderer refusal into one.
  const viewRecover = reason => {
    if (!current()) return;
    previewViews.stop();downNote = "";showViewStatus("");thin?.recover();
  };

  try {
    const saved = localStorage.getItem("backplane-view-request-id");
    const next = saved === null ? 2 ** 47 : /^(0|[1-9][0-9]*)$/.test(saved) ? Number(saved) : NaN;
    if (!Number.isSafeInteger(next) || next < 2 ** 47 || next >= 2 ** 48 - 1) throw Error("Invalid view counter");
    thin = new ThinHost({
      hub: location.origin, cid, bodyPages: true, boardCatalog: true, now: () => performance.now(), session: currentSession, viewTimeoutMs: VIEW_TIMEOUT_MS,
      readingState: thinReading?.identity === readingIdentity() ? thinReading.state : null,
      target: presentationTarget,
      counter: value => localStorage.setItem("backplane-view-request-id", String(value)),
      send: request => {
        if (!current() || s.readyState !== 1) throw Error("View connection closed");
        trace("send", { id: request.id, method: request.m });
        phoneReplyWatch(s, request);
        RR?.sent?.(request.id, request.m === "view.action" && typeof request.p?.action === "string" ? request.m + ":" + request.p.action : request.m);
        // the session's tree goes into the request as it is (known), not built again from its plain copy
        const tree = request.p?.session ? App.viewer_input(ui, presentationJson(currentSession().info || {}), presentationJson(request.p.session)) : null;
        const session = tree ? Wire.plain(tree) : null, known = new WeakMap();
        if (session && typeof session === "object") known.set(session, tree);
        s.send(fromList(App.wire_out_json(Wire.json({ ...request, p: { ...request.p, ...(session ? { session } : {}), workspaceLayout: "rail-v1", workspaceChrome: "top-v1", terminalOutput: "pane-v1", plotStamp: "v1", ...phoneRequest(PHONE) } }, known))));
      },
      choice: choosePresentation, prepare: (session, input, action) => preservePresentationRoute(
        action === "send" || action === "send-alt" ? input : session,
        Wire.plain(App.presentation_prepare(presentationJson(session), presentationJson(input), action))),
      navigation: (target, session) => {
        const cached = transitionViews.destination(transitionScope(), target);
        const selected = cached?.scope.selected;
        const next = choosePresentation(session, "select", selected || "");
        return { ...next, route: cached?.scope.route || {kind:"bot-tab", bot:target.id, tab:"chat"} };
      },
      selection: (target, session) => {
        composerKeeper.selecting(target.kind === "project" ? "project-home-" + target.id : target.kind === "thread" ? target.id : "");
        const saved = takeReadingHistory(target, session);
        historyClear();
        if (saved) {
          historyNode = saved.node;historyDirection = saved.direction;resumedConversation = saved.conversation;
          for (const side of ["older", "newer"]) for (const id of saved.gone[side]) historyGone[side].add(id);
        }
        modelFeedback.clear();subsFeedback.clear();pressFeedback.drop();foldFeedback.drop();
        thinMailDraft.clear();thinContinuations.clear();bodyWindows.disconnect();
        if (target.kind === "thread") { ui = App.presentation_select(ui, target.id);void restoreFocused(); }
        else { focusedRequestedKey = null;focusedView.invalidate(); }
        if (saved) transitionPosition = null;
        else restoreTransition(target);
        const scope = transitionScope(target), controller = thin, revision = controller?.selectionRevision;
        void transitionViews.warm(scope).then(() => {
          if (thin !== controller || thin?.selectionRevision !== revision || remoteReady || targetKey(thin?.target) !== targetKey(target)) return;
          if (target.kind === "bot" && !currentSession().sel) {
            const cached = transitionViews.destination(scope, target);
            if (cached) {
              const session = { ...choosePresentation(currentSession(), "select", cached.scope.selected), route:cached.scope.route };
              thin.adoptLocalSession(session);localPresentation("bot", target.id, session);
            }
          }
          if (restoreTransition(target)) later();
        });
        return saved?.held;
      },
      local: localPresentation,
      previewSend: text => presentationTarget(currentSession()).kind !== "project" && App.presentation_send_preview(text),
      sendFeedback: (started, thread, text) => { persistDraft(thread, text);if (started) scroll = true;later(); },
      composing: thread => composerComposing && document.querySelector("main.thread-view[data-thread]")?.getAttribute("data-thread") === thread,
      modelMenuReady: () => {
        // A picker whose cursor moved (data-draft) opens and shuts through
        // the hub, so its picture never shows a cursor dropped since.
        const layer = document.querySelector('[data-local-models="1"]');
        return !!layer && !document.querySelector('.account-picker') && !layer.hasAttribute('data-draft') && !layer.querySelector('input')?.value;
      },
      sessionChanged: (session, detail) => {
        const sc0 = RR ? pnow() : 0;
        const selectedChanged = thinSession?.sel !== session.sel;
        thinSession = session;
        if (detail?.remote) {
          reviewPosition.accepted(session);
          // Only live response choices cross this boundary. State and applied
          // cursor stay in the fresh unsynced recipient, never in a picture.
          ui = App.unsync(App.presentation_handoff(ui, presentationJson(keepDesk(session))));
          pageLang(App.lang(ui));
          keepViewer();
          // A hub action can open a thread too (Ctrl+N, Fork, the palette).
          // Keep the accepted selection as well as explicit sidebar clicks.
          try { localStorage.setItem("backplane-selected", session.sel || ""); } catch {}
        }
        if (selectedChanged) deskCheck();
        // (a page's reply: its view decides what is drawn, historyRegion)
        // (a reply that changed nothing on show draws nothing: thin-host's same)
        if (!detail?.inputOnly && !detail?.paged && !detail?.unchanged && !detail?.presenting) later();
        if (RR) RR.t("land.session", pnow() - sc0);
      },
      ack: (session, reply) => preservePresentationRoute(session, Wire.plain(App.presentation_reply(presentationJson(session), presentationJson(reply)))),
      decode: shown => {
        const d0 = RR ? pnow() : 0, json = presentationJson(shown), d1 = RR ? pnow() : 0;
        const node = App.presentation_node(json);
        if (RR) { RR.t("land.json", d1 - d0);RR.t("land.node", pnow() - d1); }
        return node;
      },
      terminal: (node, frame) => terminalOutput.receive(node, frame),
      terminalLifetime: () => document.querySelector('.floating-terminal .term-pane')?.getAttribute('data-terminal-incarnation') || '',
      command: command => presentationCommand(command),
      anchor: () => { const tl = document.getElementById("timeline"), a = tl && anchorOf(tl);return { id: a?.id || "", offset: Math.max(0, Math.round(a ? tl.getBoundingClientRect().top - a.off : 0)) }; },
      readingMark: () => holdMark(),
      pagePosition: () => {
        const list = timelineList();
        return list ? { position: { ...list.position(), atEnd: thin.atNewest() && list.atEnd() }, adjacent: "" } : null;
      },
      // Give an immediately following user action priority over an unsent
      // notification read. Timer is bounded and works while the page is hidden.
      deferRefresh: refresh => setTimeout(() => { if (current()) refresh(); }, 16),
      // (a touch screen's keys wait a little longer for the hub's echo, so
      // its page is not patched under every key while the person types)
      draftRefresh: refresh => { clearTimeout(thinDraftTimer);thinDraftTimer = setTimeout(() => { if (current()) refresh(); }, COARSE_POINTER ? 200 : 75); },
      // (the palette's results: the first key of a burst asks at once, the
      // rest of it 75 ms after its last key, as Search's)
      fieldRefresh: (refresh, name) => {
        clearTimeout(thinFieldTimer);thinFieldTimer = setTimeout(() => { if (current()) refresh(); }, 75);
        const now = performance.now();
        if (name === "wspalette" && now - paletteAsked >= 75 && current()) refresh();
        if (name === "wspalette") paletteAsked = now;
      },
      fieldsChanged: (name, text) => { if (name === "mrreply") thinMailDraft.edit(text);later(); },
      operationScope: (action, value) => action === "mail-send" ? thinMailDraft.scope(true) : action === "bfield" && value.startsWith("mrreply\u001f") ? thinMailDraft.scope() : undefined,
      blocked: () => showViewStatus("Wait for the current conversation to connect."),
      viewerChoice: choice => viewerFeedback.choice(choice),
      warning: message => showViewStatus(message),
      // a Send the dropped link never took is back in the field: "Not sent"
      // now, and once connected again that it is still there to send
      // (the words a Send pressed while offline gets)
      sendLost: () => { if (remoteReady) return; offlineSend = true; showViewStatus(App.host_message(ui, "offline-send")); },
      pageState: historyPageState,
      letGo: historyLetGo,
      refused: refusedView,
      search: thinSearchAnswer, searchFailed: thinSearchFailed, message: showThinMessage, body: (message, input) => bodyWindows.accept(message, input), bodyFailed: (input, reason) => bodyWindows.failed(input, reason),
      view: (node, detail) => {
        if (!current()) return;
        const vw0 = RR ? pnow() : 0;
        RR?.n("view.apply." + (detail.page?.direction || "update"));
        if (detail.page?.direction === "newest") { entryHistory.cancel();scroll = true; }
        if (detail.historyReset) { historyClear();transitionPosition = { top: 0, atEnd: false }; }
        // A bot may resolve to a new source conversation on this connection.
        // Its former timeline never supplies history to that new conversation.
        if (resumedConversation !== null) {
          if (resumedConversation !== currentSession().sel) historyClear();
          resumedConversation = null;
        }
        const previous = historyNode || node;
        const projection = historyNode ? detail.page?.direction || "update" : "reset";
        if (RR && historyNode && projection === "older") RR.n("land.block." + History.why(previous, node, projection));
        const p0 = RR ? pnow() : 0;
        // historyNode is a merge's own result, prepared already and marked with
        // where its last non-entry row is: an older page is added as a block,
        // the held rows past its overlap shared (timeline-history.bend's
        // project_block), anything else merged whole
        node = historyNode && projection !== "reset" && projection !== "newest" ? History.project_block(previous, node, projection) : History.project(previous, node, projection);
        if (RR) RR.t("land.project." + projection, pnow() - p0);
        historyNode = node;
        // (timeline-history.bend's project keeps no row from before a reset)
        if (projection === "reset" || projection === "newest") {
          // An older-reading gesture cannot page us back immediately after
          // Send or the new-output button has presented the newest replies.
          historyDirection = "";historyOlderDemand = 0;historyFillMore(null, false);
          historyGone.older.clear();historyGone.newer.clear();thin?.heldReset?.();
        } else historyBack(detail.window?.ids);
        if (detail.entry) entryHistory.arm(historyFillOwner(), detail.window?.remaining, detail.window?.contextMessages);
        terminalOutput.rebind();
        const field = mailReplyField(detail.shown);
        const accepted = field === undefined ? undefined : thinMailDraft.matches(detail.mailDraftContext, detail.epoch) ? thin.fieldValue("mrreply", field) : field;
        thinMailDraft.bind(detail.mailDraftContext, detail.epoch, accepted);
        thinContinuations.bind(location.origin, detail);
        bodyWindows.bind(location.origin, detail, detail.accountBinding || "", node, held, targetKey(detail.target));
        if (detail.position) transitionPosition = detail.position;
        else if (!remoteReady && !detail.historyReset) transitionPosition = transitionViews.position(transitionScope(detail.target)) || {atEnd:true};
        // (a pressed row's own mark goes once a view of another target lands)
        if (typeof unmarkPicked === "function" && targetKey(detail.target) !== shownFor) unmarkPicked();
        liveNode = node;
        focusedView.remote(detail.epoch);remoteEpoch = detail.epoch;frontendNode = node;transitionNode = null;transitionFor = "";heldAt = 0;
        shownFor = targetKey(detail.target);shownTarget = detail.target;transitionViews.put(transitionScope(detail.target), detail.shown);
        void transitionViews.warm(transitionScope(detail.target)).then(warmDestinations);
        // (its shut runs of tool calls read ahead: a first open draws at the click)
        if (detail.target?.kind === "thread") previewViews.enqueueFolds(detail.target, foldFeedback.unseen(node));
        // (and its viewers, each as the hub draws it open: a first open of
        // each draws at the click too; not on a phone, where data costs)
        if (detail.target?.kind === "thread" && !PHONE) previewViews.enqueueViewers(detail.target, VIEWER_OPENS);
        trace("thin-view", { entries: detail.window?.ids?.length || 0 });
        const region = historyRegion(detail, projection);
        if (region === "page") later();else laterRegions(region.split(" ").filter(Boolean));
        if (RR) RR.t("land.view", pnow() - vw0);
        fabLook(detail.target);
      },
      state: state => {
        if (!current()) return;
        const becameReady = !remoteReady && state.remoteReady === true;
        remoteReady = state.remoteReady === true;
        if (becameReady) deskCheck();
        if (remoteReady && (attachmentController !== thin || attachmentView !== thin?.client.links.get(thin.hub)?.viewId || attachmentSelection !== currentSession().sel)) restoreDraftAttachments(currentSession());
        if (!remoteReady) {
          modelFeedback.clear();subsFeedback.clear();pressFeedback.drop();foldFeedback.drop();
          // Keep the last validated tree as a read-only picture. The local Ui
          // has no canonical bot/catalog state in view mode. Falling back to
          // it here removed the sidebar and cats after a refusal or disconnect.
          // A selection's callback replaces it only with a retained picture of
          // the thread picked; else it stays until the next view comes.
          holdPicture();desktopBrowser.hide();
        }
        if (state.phase === "error") {
          composerKeeper.navigationFailed();
          diagnostic.note("view-error", { cause: /timed out/.test(state.reason || "") ? "view-timeout" : "view-retry" });
          trace("thin-error", { reason: state.reason });
          // Never a page to unfreeze by hand: the view is asked for again on
          // a fresh view on the same connection (viewRecover), with the last
          // validated picture and drafts kept meanwhile.
          viewRecover(state.reason);
        }
        else if (remoteReady && !s.up) { diagnostic.note("ready");s.up = true;directFails = 0;upStatus();outboxFlush();void checkDeploy();enterImprovements(); }
        else if (remoteReady) showViewStatus("");
        if (remoteReady && viewNote) showViewStatus(viewNote);
        // An accepted picture's view callback owns its full or region paint.
        // Session/state notifications must not promote it to a full redraw.
        if (!(state.painted && !becameReady && remoteReady && state.phase === "presented")) later();
      },
    }, ThinViews, next);
  } catch { disconnect("client-setup");try { s.close(); } catch {}return; }
  s.onopen = () => {
    if (!current()) return;
    ui = App.desk_reset(ui);
    deskState = {"@desk":"", "@desk.at":""};
    socket = s;heard = BigInt(Date.now());s.hb = false;diagnostic.note("open");
    void refreshFocusedIdentity();
  };
  s.onmessage = event => {
    if (!current() || typeof event.data === "string") return;
    heard = BigInt(Date.now());
    const bytes = new Uint8Array(event.data);
    diagnostic.frame(bytes.byteLength);
    if (Solid.isPlot(bytes)) {
      plotHeard = Date.now();
      const o = Solid.cbor(bytes), at = { bytes: bytes.length, t: performance.now() };
      if (o && typeof o.none === "string") o.none = App.plot_error(ui, o.none);
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
      const d0 = pnow();
      const j = Wire.cbor(bytes, KEYS, WORDS), envelope = Wire.plain(j);
      connectionLog.measure("frame.decode", pnow() - d0);
      RR?.got?.(envelope, bytes.byteLength, pnow() - d0);
      if (envelope.t === "hb") { s.hb = true;diagnostic.heartbeat();return; }
      if (envelope.t === "notify") { notify(envelope);return; }
      if (viewerRpc.receive(envelope) || improvementsRpc.receive(envelope)) return;
      if (envelope.t === "plotrevision") { ViewerTools.sourceRevision(envelope.key, envelope.revision); return; }
      if (envelope.t === "plotstamp") { ViewerTools.sourceStamp(envelope.key, envelope.revision); return; }
      voiceMicHeard(envelope);
      if (secretSaves.receive(envelope)) return;
      if (envelope.t === "reply" && sealedReply(envelope)) return;
      if (envelope.t === "reply") outboxReply(envelope);
      if (envelope.t === "view") { diagnostic.greeting(envelope);clearTimeout(thinGreetingTimer);thin.greeting(envelope); }
      else {
        const r0 = RR ? pnow() : 0;
        if (!thin.receive(envelope) && envelope.t === "reply" && Number.isSafeInteger(envelope.id) && envelope.id < 2 ** 47) thin.ack(envelope);
        if (RR && envelope.t === "reply") RR.t("land.receive", pnow() - r0);
      }
      // A thin socket never folds log/changes/info into canonical Ui.
    } catch (error) { diagnostic.note("error", { cause: "decode-error", errorKind: error?.name });disconnect("decode-error");try { s.close(); } catch {} }
  };
  // Capture the actual CloseEvent even after a local replacement retired it.
  s.onclose = event => { diagnostic.closed(event);disconnect(event); };
  s.onerror = event => { diagnostic.note("error", { cause: "transport-error" });disconnect(event); };
  s.thin = true;
  s.dead = reason => { disconnect(reason || "heartbeat-expired");try { s.close(); } catch {} };
  // Include the HTTP Upgrade/connect phase in the greeting deadline.
  thinGreetingTimer = setTimeout(() => { disconnect("greeting-timeout");try { s.close(); } catch {} }, 4000 * patience());
  thinExpiry = setInterval(() => { if (current()) thin.expire(); }, 250);
}

// when a frame (a heartbeat included) last arrived, in ms
let heard = BigInt(Date.now()), tick = Date.now(), hiddenAt = 0, heldUntil = 0;
setInterval(() => {
  const t = Date.now(), gap = t - tick;tick = t;
  // A tick far past its time with the machine awake means this page's main
  // thread was held (a heavy board, schematic or 3D model): the frames that
  // came meanwhile still wait behind this timer, so `heard` is stale and the
  // link is fine. It gets one heartbeat period to show a frame before it is
  // judged; a tick minutes late is a sleep, judged at once.
  const slept = gap > 120000;
  if (!slept && gap > 7500) heldUntil = t + 16000;
  if (!slept && t < heldUntil) return;
  if (socket && (slept || !socket.thin || socket.hb) && App.alive_due(heard, BigInt(t))) socket.dead("heartbeat-expired");
}, 5000);
document.addEventListener("visibilitychange", () => { if (document.visibilityState === "hidden") hiddenAt = Date.now(); }, true);
// A phone that slept or changed networks often keeps a socket that is dead
// without a close, and the lease above takes up to 40 s to say so while
// the page looks live. So on a phone a wake asks the hub for a view, and a
// link that brings nothing at all within 6 s is replaced (cheap: a bounded
// view; "Reconnecting" shows only if that takes a while too).
// (Both stand down while a viewer is drawing: its pictures can be megabytes,
// minutes on a slow link, and no frame shows until all of one is in; the
// lease above still judges such a link.)
let wakeProbe = 0, plotHeard = 0;
const viewerBusy = () => Date.now() - plotHeard < 60000;
function phoneWakeProbe() {
  if (!PHONE_TITLES() || !socket || wakeProbe || viewerBusy()) return;
  const asked = BigInt(Date.now()), s = socket;
  if (remoteReady) thin?.update();
  wakeProbe = setTimeout(() => {
    wakeProbe = 0;
    if (socket === s && heard < asked && Date.now() >= heldUntil && !viewerBusy()) s.dead("wake-quiet");
  }, 6000);
}
// The same holds for a Send or an answer: the hub answers it at once with a
// small frame, so a link that brings nothing within 12 s of the oldest one
// not answered since is dead, not slow (a real history on a slow phone
// answered every request within 5.5 s). (Not other requests: their answer
// can be a viewer's picture.)
const REPLY_WATCHED = new Set(["send", "send-alt", "answer"]);
let replyWatch = 0, replyFrom = 0n;
function phoneReplyWatch(s, request) {
  if (!PHONE_TITLES() || s !== socket || request.m !== "view.action" || !REPLY_WATCHED.has(request.p?.action) || viewerBusy()) return;
  if (replyWatch && heard < replyFrom) return;
  clearTimeout(replyWatch);replyFrom = BigInt(Date.now());
  replyWatch = setTimeout(() => {
    replyWatch = 0;
    if (socket === s && heard < replyFrom && Date.now() >= heldUntil && !viewerBusy()) s.dead("reply-quiet");
  }, 12000);
}
const onWake = why => {
  const away = hiddenAt ? Date.now() - hiddenAt : 0;
  if (why === "visible") hiddenAt = 0;
  if (why === "online" || why === "network" || why === "pageshow" || (why === "visible" && away > 5000)) phoneWakeProbe();
  // the network changed, or the page slept a while: a socket may be dead
  // without a close, so a quiet one is replaced (cheap: a bounded view)
  if (socket && (why === "online" || (why === "visible" && away > 60000) || why === "pageshow")) {
    // Heartbeats are 15 seconds apart. A healthy idle socket is often quiet
    // for more than 3 seconds; use the same 40-second lease as the timer.
    if (App.alive_due(heard, BigInt(Date.now()))) socket.dead("wake-expired");
  }
  // the link is down: a retry waiting runs now, and a try started while the
  // page was away (its timeouts stretched by the failures meanwhile) is
  // replaced by a fresh one with a first try's patience (reconnect.js)
  const action = !pairingAccepted || viewMode === "compatibility" || viewMode === "read-only" ? "none"
    : wakeAction({ down: downSince > 0 && !remoteReady, pending: thinRetry.pending, since: tryingSince, now: Date.now() });
  if (action !== "none") connectionLog.record("wake", { cause: "wake-" + action });
  if (action !== "none") directFails = 0;
  if (action === "retry") { thinBackoff.reset();thinRetry.wake(); }
  else if (action === "restart") { thinBackoff.reset();thinRetry.cancel();void startThin(true, "wake"); }
};
watchWake(window, document, onWake);
// Android's Chrome says when the phone moves between Wi-Fi and mobile data
navigator.connection?.addEventListener?.("change", () => { if (PHONE_TITLES()) onWake("network"); });

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
    viewMode = "read-only";focusedView.invalidate();pairingUI.show({cause: pendingInvitation ? "" : pairingCause || "pairing_required", pending: pendingInvitation});return;
  }
  if (location.pathname === "/devices") void pairingUI.devices();
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
  navigator.serviceWorker.addEventListener("message", e => { if (e.data?.t === "bp-open" && typeof e.data.thread === "string" && e.data.thread) dispatch("select", e.data.thread); });
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
