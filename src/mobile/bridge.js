// Backplane phone bridge.
//
// Loaded into JavaScriptCore (iOS) or a JavaScriptSandbox isolate
// (Android). It only moves data: the hub's CBOR frames in (as base64), the
// screen JSON and commands out, as strings. The native app owns the socket and the views;
// app.bend decides everything (AGENTS.md: the UI is dumb).
//
// Frames are decoded here rather than by Cbor.decode in Bend: neither
// engine has a JIT in an app, and a 900 KB snapshot took 8 s in Bend
// against 0.5 s here. The result is the same Json tree (cbor.bend is the
// wire format; its dictionaries come from there).

import App from "./app.bend";
import { BubMoveObserver } from "../web/bub-move.js";
const bubMoves = new BubMoveObserver((before, after) => plain(App.bub_move(JSON.stringify(before), JSON.stringify(after))));
const bubLive = new Set();
let bubMove = null;
function bubMovingScreen() {
  const screen = JSON.parse(screenText());
  if (!screen) return null;
  for (const item of screen.bubHomes || []) {
    if (!(thin.thin(item.hub) ? thin.state(item.hub).ready : bubLive.has(item.hub))) continue;
    const move = bubMoves.observe(item.hub, item.home);
    if (move) bubMove = { ...move, expires: Date.now() + move.duration };
  }
  // Retain for its full lifetime even if a native host coalesces a frame.
  screen.bubMove = bubMove && bubMove.expires > Date.now() ? bubMove : null;
  return screen;
}
import { cbor as decode, is, kind, learnJson } from "../web/wire.js";
import { Recent, foregroundPending, presentationUpdates } from "./recent.js";
import { NativeThinHost307265 } from "./thin-native-host-307265.js";
import { compactRoute, VIEW_LIMITS } from "../web/thin.js";

// the Json constructors' tags, as this bundle names them (wire.js)
learnJson(App.json_tags());

function* each(list) {
  for (let xs = list; xs && is(xs, "Con"); xs = xs.tail) yield xs.head;
}

const deviceLocal = new Map();
let nativeLocale = App.native_locale("auto", "", "");
function refreshNativeLocale() {
  const tag = App.native_locale(deviceLocal.get("lang") || "auto", deviceLocal.get("lang-os") || "", deviceLocal.get("lang-preview") || "");
  if (nativeLocale !== tag) {
    nativeLocale = tag; half = null; focusOf = null;
    // A retained Recent screen also belongs to this device's language.
    if (lastPresentation) {
      const shown = JSON.parse(lastPresentation);
      shown.locale = nativeLocale;
      lastPresentation = JSON.stringify(shown);
    }
  }
}
function restoreLocal() { for (const [key, value] of deviceLocal) hubs = App.local(hubs, key, value); }

const secs = (n) => BigInt(Math.floor(Number(n)));

const listed = (list) => Array.from(each(list));
const KEYS = listed(App.cbor_keys());
const WORDS = listed(App.cbor_words());

// base64 to bytes (neither engine has atob)
const B64 = new Int16Array(128).fill(-1);
for (let i = 0; i < 64; i += 1) B64["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".charCodeAt(i)] = i;
function bytes(b64) {
  const out = new Uint8Array((b64.length * 3) >> 2);
  let n = 0, acc = 0, bits = 0;
  for (let i = 0; i < b64.length; i += 1) {
    const c = b64.charCodeAt(i);
    const v = c < 128 ? B64[c] : -1;
    if (v < 0) continue;
    acc = ((acc << 6) | v) & 0xffffff;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[n++] = (acc >> bits) & 255;
    }
  }
  return out.subarray(0, n);
}

// The client state as flat text and back, with no recursion: Bend lists
// are chains of cells ({head, tail}) tens of thousands deep, and
// JSON.parse with a reviver recursed once per cell, overflowed the
// engine's stack on a real log and lost the kept state at every launch.
// Every object is a node in one array, named by its index; in a node a
// value [i] is node i, ["<digits>"] a natural (as BigInts were written),
// anything else itself.
// Shared parts are written once.
function flat(root) {
  const ids = new Map();
  const nodes = [];
  const ref = (v) => {
    if (typeof v === "bigint") return [v.toString()];
    if (v === null || typeof v !== "object") return v;
    let i = ids.get(v);
    if (i === undefined) {
      i = nodes.length;
      ids.set(v, i);
      nodes.push(v);
    }
    return [i];
  };
  const top = ref(root);
  const out = [];
  for (let k = 0; k < nodes.length; k += 1) {
    const v = nodes[k];
    if (Array.isArray(v)) out.push(v.map(ref));
    else {
      const o = {};
      for (const key in v) {
        const x = v[key];
        if (x !== undefined && typeof x !== "function") o[key] = ref(x);
      }
      out.push(o);
    }
  }
  return JSON.stringify({ flat: 1, top, nodes: out });
}

function unflat(text) {
  const d = JSON.parse(text);
  if (!d || d.flat !== 1) throw new Error("state");
  const nodes = d.nodes;
  // a natural kept as a BigInt comes back as a Number: Bend's own form of a
  // Nat inside (scripts/js-unmarshal.py hands the state across as it is)
  const val = (x) => (Array.isArray(x) ? (typeof x[0] === "number" ? nodes[x[0]] : Number(x[0])) : x);
  for (const n of nodes) {
    if (Array.isArray(n)) for (let i = 0; i < n.length; i += 1) n[i] = val(n[i]);
    else for (const key in n) n[key] = val(n[key]);
  }
  return val(d.top);
}

let hubs = null;
// the drafts kept on this phone, {"<hub>|<thread>": text}
let kept = {};
let clientID = "";
let previewCur = "";
let lastPresentation = null;
let lastPresentationHub = "";
const booting = new Set();
let syncUpdates = [];
const mutationReplies = new Map();
const transportEpoch = new Map();

const foregroundHub = () => previewCur || thin.current || App.current(hubs);
const waitingForForeground = () => foregroundPending(booting, foregroundHub());
const foregroundPresentation = () => lastPresentationHub === foregroundHub() ? lastPresentation : null;

function field(j, key) {
  if (!is(j, "Obj")) return null;
  for (let fs = j.fields; is(fs, "Field"); fs = fs.tail) if (fs.key === key) return fs.value;
  return null;
}
// Optional wire fields are absent on older hubs and valid one-frame joins.
function fieldValue(j, key) {
  const value = field(j, key);
  return value === null ? null : plain(value);
}
const sendWire = (hub, message) => ({ type: "send", hub, data: App.wire(JSON.stringify(message)) });
const readingPages = new Map();
function reconcileSession(session, key) {
  for (const [name, value] of deviceLocal) session = plain(App.session_local(JSON.stringify(session), name, value));
  for (const [thread, page] of readingPages) if (App.session_target(hubs, key, "select", thread) === key) {
    session = plain(App.session_reading(JSON.stringify(session), thread, page.seen, BigInt(page.count), page.away));
  }
  return session;
}
const recent = new Recent({
  now: () => Date.now(), cid: () => clientID,
  session: key => reconcileSession(plain(App.session(hubs, key)), key),
  reconcile: reconcileSession,
  prepare: (session, input, action) => plain(App.session_prepare(JSON.stringify(session), JSON.stringify(input), action)),
  draft: (session, text, key) => ({
    session: plain(App.session_draft(JSON.stringify(session), text)),
    cmds: session.sel ? [{ type: "keep", thread: key + "|" + session.sel, text }] : [],
  }),
  failed: (key, input, action) => plain(App.session_failed(key, JSON.stringify(input), action)),
  issued: (key, input, action) => plain(App.session_issued(key, JSON.stringify(input), action)),
  reply: (session, envelope) => plain(App.session_reply(JSON.stringify(session), envelope.bend)),
  command: c => {
    if (c.type !== "send") return c;
    const request = JSON.parse(c.text);
    if (request.m === "turn.start" || request.m === "thread.modes") mutationReplies.set(request.id, c.hub);
    return { type: "send", hub: c.hub, data: App.wire(c.text) };
  },
  perform: (key, operation) => {
    const cur = previewCur || App.current(hubs);
    const session = plain(App.session_prepare(JSON.stringify(plain(App.session(hubs, key))), JSON.stringify(operation.input), operation.action));
    hubs = App.session_adopt(hubs, key, JSON.stringify(session), key);
    const result = App.act(hubs, operation.action, operation.value);
    hubs = result.hubs;
    const cmds = commandList(result.cmds, JSON.parse(App.alerts(result)));
    hubs = App.session_adopt(hubs, key, JSON.stringify(plain(App.session(hubs, key))), cur);
    return cmds;
  },
  send: sendWire,
  adopt: (key, session) => {
    hubs = App.session_adopt(hubs, key, JSON.stringify(session), previewCur || App.current(hubs));
    half = null; focusOf = null;
  },
});

// The thin native lane never invokes App.recv/online/tick on view frames.
// Every adapter below is pure until direct() returns the complete command batch.
const bendJson = value => App.json_parse(JSON.stringify(value));
function thinRoute(session, previous) {
  const route = compactRoute(previous.route);
  return route && session.sel === previous.sel ? { ...session, route } : session;
}
function thinIntent(key, session, target, action, value) {
  if (action === "select" || action === "draft") {
    let next = plain(App.thin_choice(JSON.stringify(session), action, value));
    if (action === "draft") next = thinRoute(next, session);
    if (next.route === null) delete next.route;
    return { kind: action === "select" ? "select" : "draft", session: next,
      target: plain(App.thin_target(JSON.stringify(next))) };
  }
  if (action === "earlier") return { kind: "page" };
  if (action === "later") return { kind: "later" };
  if (action === "find-open" && value === "search") return { kind: "find", search: { query: "", rows: [], cursor: "", hasMore: false, visited: 0 } };
  if (action === "find-close") return { kind: "find", search: null };
  if (action === "find-q") return { kind: "search", args: { query: value, cursor: "" } };
  if (["thin-search", "thin-search-more", "thin-message-more", "thin-cancel"].includes(action)) {
    let args; try { args = JSON.parse(value); } catch { return { kind: "blocked" }; }
    if (!args || typeof args !== "object" || Array.isArray(args)) return { kind: "blocked" };
    if (action === "thin-message-more" && Object.keys(args).length === 2 && typeof args.entryId === "string" && typeof args.cursor === "string")
      return { kind: "message", args };
    if (["thin-search", "thin-search-more"].includes(action) && Object.keys(args).every(k => k === "query" || k === "cursor") && typeof args.query === "string" && (args.cursor === undefined || typeof args.cursor === "string"))
      return { kind: "search", args: { query: args.query, cursor: args.cursor || "" } };
    if (action === "thin-cancel" && Object.keys(args).length === 1 && Number.isSafeInteger(args.requestId)) return { kind: "cancel", args };
    return { kind: "blocked" };
  }
  // This only captures an intended route. The actual action still runs in
  // Bend on the server, whose accepted policy decides the returned route.
  if (action === "bot-tab") {
    const route = compactRoute({ kind: "bot-tab", bot: target.id, tab: value }, target);
    if (route) session = { ...session, route };
  }
  return { kind: "action", session };
}
function thinMaterialize(result, session) {
  const window = bendJson(result.window);
  const shown = plain(App.thin_partial(bendJson(result.shown), window));
  const overlay = { ...result.overlay, j: plain(App.thin_partial(bendJson(result.overlay.j), window)) };
  return { shown, overlay: plain(App.thin_shown_draft(bendJson(overlay), JSON.stringify(session))) };
}
function thinMessage(view, message) {
  const hub = view.overlay?.key;
  const replace = screen => {
    const thread = screen?.thread;
    if (typeof hub !== "string" || !hub || !thread || thread.id !== hub + "|" + message.thread) throw Error("Wrong message owner");
    const entry = thread.entries.find(e => e.id === message.id);
    if (!entry || entry.partial?.version !== message.version) throw Error("Stale message version");
    const partial = { version: message.version, cursor: message.cursor, hasMore: message.hasMore, complete: message.complete };
    const changed = { ...entry, text: message.text, blocks: plain(App.live_nodes(message.text)), partial };
    return { ...screen, thread: { ...thread, entries: thread.entries.map(e => e.id === message.id ? changed : e) } };
  };
  return { shown: replace(view.shown), overlay: { ...view.overlay, j: replace(view.overlay.j) } };
}
const thin = new NativeThinHost307265({
  now: () => Date.now(), cid: () => clientID,
  renderer: () => deviceLocal.get("mailroom-renderer") || "",
  session: key => {
    const session = reconcileSession(plain(App.session(hubs, key)), key);
    return session;
  },
  drafts: key => Object.fromEntries(Object.entries(kept).filter(([qualified]) => qualified.startsWith(key + "|")).map(([qualified, text]) => [qualified.slice(key.length + 1), text])),
  target: session => plain(App.thin_target(JSON.stringify(session))),
  intent: thinIntent,
  focus: key => { previewCur = key; },
  firstProject: () => bubMovingScreen()?.projects?.[0]?.id || '',
  prepare: (session, input, action) => thinRoute(plain(App.session_prepare(JSON.stringify(session), JSON.stringify(input), action)), input),
  ack: (session, envelope) => thinRoute(plain(App.session_reply(JSON.stringify(session), envelope.bend || bendJson(envelope))), session),
  send: sendWire,
  command: (key, command) => {
    if (command.type === "send") return { type: "send", hub: key, data: App.wire(command.text) };
    return command;
  },
  registration: (key, session, args) => {
    const isolated = App.session_adopt(hubs, key, JSON.stringify(session), key);
    const registered = App.register_at(isolated, key, ...args);
    const cmds = [];
    for (const item of each(registered.cmds)) {
      if (!is(item.cmd, "Send") || item.hub !== key) return null;
      cmds.push({ type: "send", text: item.cmd.text });
    }
    return { session: thinRoute(plain(App.session(registered.hubs, key)), session), cmds };
  },
  materialize: thinMaterialize,
  message: thinMessage,
  find: (view, search) => {
    const project = screen => search === null ? { ...screen, find: null } : plain(App.thin_search(JSON.stringify(screen), JSON.stringify(search)));
    return { shown: project(view.shown), overlay: { ...view.overlay, j: project(view.overlay.j) } };
  },
  search: (key, result, view) => ({
    shown: plain(App.thin_search(JSON.stringify(view.shown), JSON.stringify(result))),
    overlay: { ...view.overlay, j: plain(App.thin_search(JSON.stringify({ ...view.overlay.j, hub: key }), JSON.stringify(result))) },
  }),
});

function currentDeadline() {
  const deadlines = [...recent.links.values()].filter(l => l.flight).map(l => l.flight.began + 5001);
  const deadline = thin.deadline();
  if (deadline !== null) deadlines.push(deadline);
  return deadlines.length ? Math.min(...deadlines) : null;
}

function direct(cmds, draw, updates = []) {
  const pending = thin.drain();
  cmds = cmds.concat(pending.cmds);
  updates = updates.concat(pending.sync);
  for (const c of cmds) if (c.type === "keep") {
    if (c.text) kept[c.thread] = c.text;
    else delete kept[c.thread];
  }
  const canDraw = draw && (!thin.thin(foregroundHub()) || thin.views() !== null);
  return JSON.stringify({ screen: canDraw ? bubMovingScreen() : null, cmds, sync: syncUpdates.splice(0).concat(updates), deadline: currentDeadline() });
}

function expired(followups = false) {
  thin.expire(followups);
  const cs = [];
  for (const failure of recent.expire()) {
    cs.push(...failure.cmds);
    syncUpdates.push({ hub: failure.key, phase: "failed", detail: failure.failed });
  }
  return cs;
}

// kept drafts into the clients of the hubs they belong to
function restore() {
  for (const [k, text] of Object.entries(kept)) {
    const i = k.indexOf("|");
    if (i > 0 && typeof text === "string") hubs = App.restore(hubs, k.slice(0, i), k.slice(i + 1), text);
  }
}

// every call answers {"screen": <screen>, "cmds": [...]} as one string;
// a send names the hub it goes to; alerts ride along as
// {"type": "notify", ...} commands
// a Bend JSON value (core/json.bend: Arr/Obj hold Item/Field chains ending
// in End) as a plain JS value, for the engine's own JSON.stringify
function plain(j) {
  switch (kind(j)) {
    case "Null":
      return null;
    case "Flag":
      return j.value;
    case "Num": {
      const n = Number(j.raw);
      return Number.isFinite(n) ? n : j.raw;
    }
    case "Str":
      return j.text;
    case "Arr": {
      const a = [];
      for (let c = j.items; is(c, "Item"); c = c.tail) a.push(plain(c.head));
      return a;
    }
    case "Obj": {
      const o = {};
      for (let c = j.fields; is(c, "Field"); c = c.tail) Object.defineProperty(o, c.key, { value: plain(c.value), enumerable: true, configurable: true, writable: true });
      return o;
    }
    default:
      return null;
  }
}

// The screen, in two halves (hubs.bend's Hubs.lean): the list half (every
// hub's list, the plane, the island, the tabs) is made again only when
// App.lean(hubs) is not made of the very same parts as when it was last
// made; the open thread ("thread") whenever the hubs changed at all. Bend
// values never change in place, so a part that is the same object holds the
// same value, and a pure def of the same values answers the same: a token of
// the open thread's reply makes only the thread again (80 ms with no JIT for
// the whole screen, a few for the thread).
// the values same() looks inside: the hubs, their list, each hub, its client
const OPEN = new Set(["hubs.Hubs", "Con", "Nil", "hubs.Hub", "../core/client.Ui"]);
function same(a, b) {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object" || a.$ !== b.$) return false;
  const open = OPEN.has(a.$);
  for (const k in a) {
    if (k === "$" || a[k] === b[k]) continue;
    if (!open || !same(a[k], b[k])) return false;
  }
  return true;
}

// The open thread's streamed reply, as markdown nodes. A reply grows a few
// words a frame and rendering all of it again took most of a thread's
// screen (100s of ms for a long one with no JIT). The markdown parser ends
// every block at a blank line outside a fenced code block, so the text up
// to such a line renders to exactly the nodes it renders to alone (core's
// Live.at, laws live_prefix_exact*): those paragraphs are rendered once and
// kept, and only what follows the last one is rendered again. Only whole
// lines are looked at (a reply only grows; a text that does not start with
// what was looked at starts over).
const live = { seen: "", fence: false, head: 0, nodes: [] };
const SPACE = /^[\t-\r ]*$/;
const FENCE = /^[\t-\r ]*```/;
function liveNodes(text) {
  if (!text.startsWith(live.seen)) {
    live.seen = "";
    live.fence = false;
    live.head = 0;
    live.nodes = [];
  }
  let at = live.seen.length;
  let fence = live.fence;
  let head = live.head;
  for (let nl = text.indexOf("\n", at); nl >= 0; nl = text.indexOf("\n", at)) {
    const line = text.slice(at, nl);
    if (!fence && SPACE.test(line)) head = nl + 1;
    if (FENCE.test(line)) fence = !fence;
    at = nl + 1;
  }
  if (head > live.head) {
    live.nodes = live.nodes.concat(plain(App.live_nodes(text.slice(live.head, head))));
    live.head = head;
  }
  live.seen = text.slice(0, at);
  live.fence = fence;
  return head < text.length ? live.nodes.concat(plain(App.live_nodes(text.slice(head)))) : live.nodes;
}

let half = null; // { lean, head }: the list half's JSON, but its closing brace
let focusOf = null;
let focusText = "null";
function screenText() {
  const cur = foregroundHub();
  if (thin.thin(cur)) {
    const overlays = thin.views();
    if (!overlays) return "null";
    const drafts = overlays.map(overlay => plain(App.thin_shown_draft(bendJson(overlay), JSON.stringify(thin.state(overlay.key).session))));
    const screen = plain(App.thin_presentation(hubs, bendJson(drafts), cur));
    screen.locale = nativeLocale;
    // paged up into the history: how many pages lie below (the native
    // list's bottom trigger asks for "later"; the host keeps the trail)
    const back = thin.state(cur)?.back?.length || 0;
    if (screen.thread && back) screen.thread.later = back;
    return JSON.stringify(screen);
  }
  if (waitingForForeground() && !recent.links.get(cur)?.shown && foregroundPresentation()) return foregroundPresentation();
  const overlays = recent.overlays();
  if (overlays.length) {
    let array = App.json_array();
    const links = [...recent.links.values()].filter(l => l.shown);
    for (let i = links.length - 1; i >= 0; i--) array = App.json_cons(App.session_shown_draft(links[i].shown, JSON.stringify(links[i].session)), array);
    const shown = plain(App.presentation(hubs, array, cur));
    // The native host needs the same explicit locale during Recent as
    // after canonical replay. Rendered content and actions stay intact.
    shown.locale = nativeLocale;
    lastPresentation = JSON.stringify(shown);
    lastPresentationHub = cur;
    return lastPresentation;
  }
  if (waitingForForeground() && foregroundPresentation()) return foregroundPresentation();
  const lean = App.lean(hubs);
  if (half === null || !same(lean, half.lean)) {
    const o = plain(App.screen_list_in(lean, nativeLocale));
    delete o.thread;
    const t = JSON.stringify(o);
    half = { lean, head: t.slice(0, -1) };
  }
  if (focusOf === null || !same(hubs, focusOf)) {
    const th = plain(App.focus(hubs));
    if (th !== null && typeof th === "object") th.live = liveNodes(App.live_text(hubs));
    focusText = JSON.stringify(th);
    focusOf = hubs;
  }
  return half.head + (half.head === "{" ? "" : ",") + '"locale":' + JSON.stringify(nativeLocale) + ',"thread":' + focusText + "}";
}

// a frame that could change the screen came while more waited behind it
// (so it built none): the next call builds one, whatever that frame is
let owed = false;

function commandList(cmds, alerts) {
  const cs = [];
  for (const h of each(cmds)) {
    const c = h.cmd;
    if (is(c, "Send")) cs.push({ type: "send", hub: h.hub, data: App.wire(c.text) });
    else if (is(c, "Copy")) cs.push({ type: "copy", text: c.text });
    else if (is(c, "Focus")) cs.push({ type: "focus", id: c.id });
    else if (is(c, "ScrollEnd")) cs.push({ type: "scroll" });
    // a client action come due (the design history playing), back to the hub that asked
    else if (is(c, "Later")) cs.push({ type: "later", ms: Number(c.ms), action: c.action, value: h.hub + "|" + c.value });
    // show an entry of the thread (the history's way back to it)
    else if (is(c, "Jump")) cs.push({ type: "jump", id: c.id });
    // the native side writes it down at once; "" forgets it
    else if (is(c, "Keep")) {
      const key = h.hub + "|" + c.thread;
      if (c.text) kept[key] = c.text;
      else delete kept[key];
      cs.push({ type: "keep", thread: key, text: c.text });
    }
  }
  for (const a of alerts ?? []) cs.push({ type: "notify", ...a });
  return cs;
}

function out(cmds, quiet, alerts) {
  if (!quiet) owed = false;
  return direct(commandList(cmds, alerts), !quiet && !(waitingForForeground() && !recent.links.get(foregroundHub())?.shown && !foregroundPresentation()));
}

// a step's answer: the hubs after it, its commands and alerts
function step(r, quiet) {
  hubs = r.hubs;
  return out(r.cmds, quiet, JSON.parse(App.alerts(r)));
}

globalThis.Backplane = {
  // cid: this phone's id, part of every message id (a resend is stored once);
  // drafts: the drafts it kept, as JSON {"<hub>|<thread>": text}
  start(cid, drafts) {
    thin.reset();
    clientID = cid;
    hubs = App.init(secs(Date.now() / 1000), cid);
    try {
      kept = JSON.parse(drafts ?? "{}") ?? {};
    } catch {
      kept = {};
    }
    return out(null);
  },
  // the hubs paired, as their keys (host:port) in order
  hubs(keys) {
    thin.hubs(keys);
    if (previewCur && !keys.includes(previewCur)) previewCur = thin.current;
    for (const key of recent.links.keys()) if (!keys.includes(key)) recent.close(key);
    for (const key of booting) if (!keys.includes(key)) booting.delete(key);
    for (const key of transportEpoch.keys()) if (!keys.includes(key)) transportEpoch.delete(key);
    hubs = App.hubs(hubs, JSON.stringify(keys));
    restoreLocal();
    restore();
    return out(null);
  },
  // Explicit pairing retirement; ordinary socket reconnects do not call it.
  forgetHub(key) {
    thin.forgetHub(key);
    // Remove the compact Bend client as well; a new pairing at the same
    // address starts from a new client, not the previous account's choices.
    hubs = App.hubs(hubs, JSON.stringify([...thin.paired]));
    if (previewCur === key) previewCur = thin.current;
    for (const draft of Object.keys(kept)) if (draft.startsWith(key + "|")) delete kept[draft];
    recent.close(key);
    booting.delete(key);
    transportEpoch.delete(key);
    bubLive.delete(key);
    bubMoves.disconnect(key);
    syncUpdates = syncUpdates.filter(s => s.hub !== key);
    // Do not drain another hub's queued commands or status into a discarded
    // retirement reply; the next ordinary bridge call delivers those.
    return JSON.stringify({ screen: null, cmds: [], sync: [], deadline: currentDeadline() });
  },
  // a choice that belongs to this device, not the hub (the thread list's
  // "view": "projects" or "active"; "" leaves it to the hub's setting)
  local(key, value) {
    deviceLocal.set(key, value);
    refreshNativeLocale();
    hubs = App.local(hubs, key, value);
    const cmds = [];
    for (const state of thin.states.values()) if (thin.thin(state.hub)) {
      state.session = thinRoute(plain(App.session_local(JSON.stringify(state.session), key, value)), state.session);
      if (state.greeted) thin.update(state);
    }
    for (const [hub, link] of recent.links) {
      link.session = plain(App.session_local(JSON.stringify(link.session), key, value));
      cmds.push(...recent.refresh(hub));
    }
    return cmds.length ? direct(cmds, false) : out(null);
  },
  reading(thread, seen, count, away) {
    readingPages.set(thread, { seen, count, away });
    // Scroll anchoring is a local preference, never an older-page gesture.
    const thinKey = App.session_target(hubs, foregroundHub(), "select", thread);
    if (thin.thin(thinKey)) {
      const state = thin.state(thinKey);
      state.anchor = { id: String(seen), offset: 0 };
      return direct([], false);
    }
    const key = App.session_target(hubs, previewCur || App.current(hubs), "select", thread);
    const link = recent.links.get(key);
    const session = link?.session || plain(App.session(hubs, key));
    const next = plain(App.session_reading(JSON.stringify(session), thread, seen, BigInt(count), away));
    if (link) {
      link.session = next;
      return direct(recent.refresh(key), false);
    }
    hubs = App.session_adopt(hubs, key, JSON.stringify(next), App.current(hubs));
    return direct([], false);
  },
  // hub k's socket query: resume from what this app already holds
  resume(k) {
    if (thin.thin(k)) return JSON.stringify({ since: "0", origin: "" });
    return JSON.stringify({ since: App.seq(hubs, k), origin: App.origin(hubs, k) });
  },
  screen() {
    return out(null);
  },
  // The native 250 ms streaming timer. Off-screen frames owe nothing, so
  // this is a cheap no-op unless a visible state was withheld or dropped.
  flush(owe) {
    const failures = expired();
    if (failures.length || syncUpdates.length) return direct(failures, false);
    if (owe === true) owed = true;
    return out(null, !owed);
  },
  // a cat's rig (JSON text) for the key a screen names it by, "look:mood"
  cat(key) {
    const i = String(key).indexOf(":");
    return App.cat(BigInt(Number(String(key).slice(0, i)) || 0), String(key).slice(i + 1));
  },
  // a catalog text for a key in the device's language (the key itself if unknown)
  t(key) {
    return App.t(hubs, String(key));
  },
  // a binary frame from hub k, as base64
  // quiet: more frames wait behind this one, so no screen is built for it
  // (only the last of a burst is drawn: terminal echoes, streamed text)
  // (nor for streamed text the screen does not show: App.shows, that is
  // Hubs.redraw, laws hubs_redraw_*; every token of every agent at work is
  // a frame; anything else from any paired hub redraws the merged screen)
  // owe: the app dropped a screen since (a newer call was waiting), so
  // this one builds one
  recv(k, data, epochOrQuiet, quietOrOwe, owe) {
    // Older hosts still pass (key, data, quiet, owe). New iOS tags every
    // queued frame with its connection epoch before it reaches this worker.
    const tagged = typeof epochOrQuiet === "string";
    const quiet = tagged ? quietOrOwe : epochOrQuiet;
    if (!tagged) owe = quietOrOwe;
    if (tagged && transportEpoch.get(k) !== epochOrQuiet) return direct([], false);
    if (thin.thin(k) && data.length > 4 * Math.ceil((VIEW_LIMITS.replyBytes + 4096) / 3)) {
      thin.fail(k, "Thin frame exceeds wire limit");
      return direct([], false);
    }
    const o = decode(bytes(data), KEYS, WORDS);
    const type = fieldValue(o, "t");
    if (thin.thin(k)) {
      const envelope = plain(o);
      if (thin.inert(k, envelope, transportEpoch.get(k))) {
        // No drain/draw/owed work, and do not cancel or renew an existing timer.
        return JSON.stringify({ screen: null, cmds: [], sync: [], deadline: currentDeadline(), inert: true });
      }
      if (type === "reply") envelope.bend = o;
      thin.receive(k, envelope, transportEpoch.get(k));
      if (k === foregroundHub() && (quiet === true || owe === true)) owed = true;
      return direct([], quiet !== true || owe === true);
    }
    if (type === "mobile") {
      booting.add(k);
      if (!previewCur) previewCur = App.current(hubs) || k;
      return direct(recent.open(k, String(fieldValue(o, "generation"))), false, [{ hub: k, phase: "recent" }]);
    }
    if (type === "reply") {
      const envelope = plain(o);
      if (envelope.result?.mobile === 1) envelope.result.shown = field(field(o, "result"), "shown");
      const answer = recent.reply(k, envelope);
      if (answer) {
        if (answer.live) booting.delete(k);
        return direct(answer.cmds, answer.current === true && k === foregroundHub(), presentationUpdates(k, answer));
      }
      recent.ack(k, { ...envelope, bend: o });
      if (mutationReplies.get(envelope.id) === k) {
        mutationReplies.delete(envelope.id);
        const link = recent.links.get(k);
        if (link && !link.flight && !link.waiting.length) {
          return direct(recent.request(k, "", ""), false);
        }
      }
    }
    if (type === "log" && fieldValue(o, "more") === true || type === "changes" && fieldValue(o, "boot") === true) {
      if (recent.active(k)) booting.add(k);
      syncUpdates.push({ hub: k, phase: "history" });
    }
    const r = App.recv(hubs, k, o);
    // Log replacement installs server info. Renderer support belongs to this
    // device, so restore its explicit declaration after that replacement.
    if (type === "log" && deviceLocal.has("mailroom-renderer")) {
      r.hubs = App.local(r.hubs, "mailroom-renderer", deviceLocal.get("mailroom-renderer"));
    }
    const ended = type === "log" && fieldValue(o, "more") !== true || type === "changes" && fieldValue(o, "boot") === false;
    if (ended) {
      hubs = r.hubs;
      const pending = recent.end(k);
      booting.delete(k);
      bubLive.add(k);
      syncUpdates.push({ hub: k, phase: "live" });
      if (!waitingForForeground()) owed = false;
      return direct(commandList(r.cmds, JSON.parse(App.alerts(r))).concat(pending), !waitingForForeground());
    }
    const shows = App.shows(r.hubs, k, o);
    if (owe === true || (quiet === true && shows)) owed = true;
    return step(r, booting.has(k) || quiet === true || !(shows || owed));
  },
  // The whole client state as text, kept by the app when it leaves the
  // foreground: the next launch load()s it instead of folding every event
  // again (on QuickJS a few thousand took seconds), and resume() then asks
  // each hub only for what came since. The state is plain data (objects,
  // strings, booleans, BigInt naturals), written flat (flat()).
  savePresentation() { return thin.savePresentation(); },
  loadPresentation(text) {
    thin.loadPresentation(text);
    return JSON.stringify({ screen: null, cmds: [], sync: [], deadline: null });
  },
  save() {
    let saved = hubs;
    for (const [key, link] of recent.links) saved = App.session_adopt(saved, key, JSON.stringify(link.session), previewCur || App.current(saved));
    return flat(saved);
  },
  load(text) {
    try {
      hubs = App.stale(unflat(text));
      restoreLocal();
    } catch {
      return "";
    }
    return out(null);
  },
  register(platform, token, kind, thread, env, bundle) {
    const cmds = [];
    for (const key of thin.paired) {
      if (thin.thin(key)) thin.register(key, [platform, token, kind, thread, env, bundle]);
      else {
        const result = App.register_at(hubs, key, platform, token, kind, thread, env, bundle);
        hubs = result.hubs;
        cmds.push(...commandList(result.cmds, JSON.parse(App.alerts(result))));
      }
    }
    return direct(cmds, false);
  },
  registerAt(hub, platform, token, kind, thread, env, bundle) {
    if (thin.register(hub, [platform, token, kind, thread, env, bundle])) return direct([], false);
    return step(App.register_at(hubs, hub, platform, token, kind, thread, env, bundle), true);
  },
  projectSearchQuery(scope, edit, text) {
    thin.projectSearchQuery(scope, edit, text);
    return direct([], true);
  },
  projectSearchEnd(scope) {
    thin.projectSearchEnd(scope);
    return direct([], false);
  },
  projectSearchGo(scope, edit, text, project) {
    thin.projectSearchGo(scope, edit, text, project);
    return direct([], true);
  },
  act(action, value) {
    const key = App.session_target(hubs, previewCur || App.current(hubs), action, value);
    if (thin.thin(key)) {
      previewCur = key;
      thin.request(key, action, value);
      return direct([], action !== "draft");
    }
    if (recent.active(key)) {
      if (App.session_mail_action(action, value)) return direct([], false);
      previewCur = key;
      return direct(recent.request(key, action, value), false);
    }
    previewCur = key;
    return step(App.act(hubs, action, value));
  },
  // the secret box's Save: "<name>\u001f<purpose>\u001f<thread>\u001f<value>",
  // the value from the app's vault field (screen.bend's Mob.sbox), never a
  // form field. It leaves in one secret.set request to the focused hub that
  // Bend builds and checks (app.bend's secret_req): not an action, not the
  // kept state, not a draft.
  secret(packed) {
    const key = previewCur || App.current(hubs);
    let r;
    try { r = JSON.parse(App.secret_req(packed)); } catch { r = { error: "The secret could not be sent" }; }
    packed = "";
    if (r.error) return this.act("secret-err", r.error);
    const send = { type: "send", hub: key, data: App.wire(r.rpc) };
    r = null;
    const after = JSON.parse(this.act("secret-sent", ""));
    after.cmds = [send].concat(after.cmds || []);
    return JSON.stringify(after);
  },
  // an action whose screen the native side already shows (typing a draft),
  // unless it changed what the composer offers (app.bend's offer)
  quiet(action, value) {
    const key = App.session_target(hubs, previewCur || App.current(hubs), action, value);
    if (thin.thin(key)) {
      previewCur = key;
      thin.request(key, action, value);
      return direct([], action !== "draft");
    }
    if (recent.active(key)) {
      if (App.session_mail_action(action, value)) return direct([], false);
      if (action === "draft") return direct(recent.draft(key, value), false);
      return direct(recent.request(key, action, value), false);
    }
    const was = App.offer(hubs);
    const r = App.act(hubs, action, value);
    return step(r, App.offer(r.hubs) === was);
  },
  // hub k's socket opened or closed; a new connection re-asks for the
  // viewer's plot
  // quiet: no screen (the hubs marked offline after load(), before the
  // screen that follows)
  // (no screen either when the hub already was so: app.bend's online_same)
  online(k, b, quiet, epoch) {
    if (epoch && transportEpoch.get(k) !== epoch) return direct([], false);
    if (!b) { bubLive.delete(k);bubMoves.disconnect(k); }
    if (thin.thin(k)) {
      if (!b) { thin.disconnect(k); thin.fail(k, "connection closed"); }
      return direct([], false);
    }
    if (!b) {
      booting.delete(k);
      const failure = recent.abort(k, "connection closed");
      if (failure) {
        const r = App.online(hubs, k, false);
        hubs = r.hubs;
        return direct(failure.cmds, false, [{ hub: k, phase: "failed", detail: failure.failed }]);
      }
    }
    const same = App.online_same(hubs, k, !!b);
    return step(App.online(hubs, k, !!b), quiet === true || same);
  },
  tick(now) {
    const failures = expired();
    if (failures.length || syncUpdates.length) return direct(failures, false);
    if ([...thin.paired].every(key => thin.thin(key))) return direct([], false);
    return step(App.tick(hubs, secs(now)));
  },
  // for tests and benches: the screen as the bridge makes it (in halves,
  // kept) and as one whole (Hubs.screen), as JSON text
  halves() {
    return JSON.stringify({ kept: JSON.parse(screenText()), whole: plain(App.screenj(hubs)) });
  },
  // the same for a streamed reply's text: rendered a settled paragraph at a
  // time (liveNodes, as the screens do) and all at once
  liveHalves(text) {
    return JSON.stringify({ kept: liveNodes(String(text)), whole: plain(App.live_nodes(String(text))) });
  },
  reflowDefaults() { return App.reflow_defaults(); },
  reflowAlloys() { return App.reflow_alloys(); },
  reflow(profile, alloy) { return App.reflow_plan(String(profile), String(alloy)); },
  headerKeys() { return JSON.stringify({ keys: KEYS, words: WORDS }); },
  // Older native engines retain the existing immediate-refresh protocol.
  publicationAck() { thin.publicationAck = true; },
  // Engine calls this only after it hands a decoded current screen out.
  published() { return thin.published(); },
  pulse() { return direct(expired(true), false); },
  connection(key, epoch, mode) {
    bubLive.delete(key);bubMoves.disconnect(key);
    thin.connection(key, String(epoch), mode);
    const previous = recent.links.get(key);
    if (previous) hubs = App.session_adopt(hubs, key, JSON.stringify(previous.session), previewCur || App.current(hubs));
    const failure = recent.abort(key, "connection changed");
    booting.delete(key);
    transportEpoch.set(key, String(epoch));
    return direct(failure?.cmds || [], false);
  },
};
