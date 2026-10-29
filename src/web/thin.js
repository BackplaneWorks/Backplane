import { terminalInputReply } from './terminal-input.js';
import { modelMenuReply } from './model-menu-input.js';
import { sidebarOffer, sidebarPicture } from './sidebar-region.js';

// Bounded view transport shared by the web and native bridge. Product actions
// and rendering remain in Bend. No canonical replay is initiated here.
export const VIEW_VERSION = 1;
export const VIEW_LIMITS = Object.freeze({ replyBytes: 262144, entries: 32, recent: 24,
  searchResults: 25, searchCandidates: 128, chunkBytes: 65536 });
export const VIEW_ID_BASE = 2 ** 47;
const MAX_ID = 2 ** 48 - 1;
const MAX_REVISION = 2 ** 32 - 1;
const METHODS = new Set(['view.open', 'view.page', 'view.search', 'view.update', 'view.action', 'view.message', 'view.cancel']);
const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
const record = o => o !== null && typeof o === 'object' && !Array.isArray(o);
export function utf8Bytes(text) {
  let n = 0;
  for (const c of text) { const x = c.codePointAt(0); n += x < 128 ? 1 : x < 2048 ? 2 : x < 65536 ? 3 : 4; }
  return n;
}
const bounded = (s, n) => typeof s === 'string' && utf8Bytes(s) <= n;
const integer = (n, max = MAX_REVISION) => Number.isSafeInteger(n) && n >= 0 && n <= max;
const sameTarget = (a, b) => record(a) && record(b) && a.kind === b.kind && a.id === b.id;
const revision = s => bounded(s, 32) && /^(0|[1-9][0-9]*)$/.test(s);
// Revisions are decimal protocol strings, potentially larger than JS integers.
const coversRevision = (a, b) => revision(a) && revision(b) && (a.length > b.length || a.length === b.length && a >= b);
function coverChanged(link, shownRevision) {
  if (link.changedRevision !== null && coversRevision(shownRevision, link.changedRevision)) {
    link.changedRevision = null;
    if (!link.readInvalidated) link.invalidated = false;
  } else if (link.changedRevision !== null) link.invalidated = true;
}
export function viewTarget(t) {
  return record(t) && Object.keys(t).length === 2 && Object.keys(t).every(k => k === 'kind' || k === 'id') && ['thread', 'bot', 'room', 'recent'].includes(t.kind) &&
    bounded(t.id, 1024) && (t.kind === 'recent' ? t.id === '' : t.id.length > 0);
}
const snapshot = o => JSON.parse(JSON.stringify(o));
export function viewCapability(o) {
  return record(o) && o.version === 1 && record(o.views) && o.views.version === 1 &&
    o.views.websocketQuery === 'view=1' && o.views.compatibility === 'explicit-only' &&
    Array.isArray(o.views.renderers) && o.views.renderers.includes('web') && o.views.renderers.includes('mobile');
}
export function viewGreeting(o) {
  return record(o) && o.t === 'view' && o.version === 1 && ['web', 'mobile'].includes(o.renderer) &&
    typeof o.generation === 'string' && /^(0|[1-9][0-9]{0,9})$/.test(o.generation) &&
    Number(o.generation) <= MAX_REVISION && bounded(o.epoch, 1024) && o.epoch.length > 0 &&
    record(o.limits) && Object.entries(VIEW_LIMITS).every(([k, n]) => integer(o.limits[k]) && o.limits[k] > 0 && o.limits[k] <= n);
}
function optionsMap(o, boolean) {
  if (!record(o) || Object.keys(o).length > 128) return null;
  const out = {};
  for (const [k, v] of Object.entries(o)) {
    if (!bounded(k, 1024) || (boolean ? typeof v !== 'boolean' : !bounded(v, 1024))) return null;
    if (!boolean && !k.startsWith('fold:') && !k.startsWith('subs:')) return null;
    // Object.defineProperty also treats __proto__ as data, never a prototype.
    Object.defineProperty(out, k, { value: v, enumerable: true, writable: true, configurable: true });
  }
  return out;
}
const BOT_TABS = new Set(['chat','mail','status','space','browser','tasks','memory','routines','hooks','settings']);
export function compactRoute(r, target = null) {
  return record(r) && Object.keys(r).length === 3 && Object.keys(r).every(k => ['kind','bot','tab'].includes(k)) && r.kind === 'bot-tab' &&
    bounded(r.bot, 1024) && r.bot.length > 0 && BOT_TABS.has(r.tab) &&
    (!target || (target.kind === 'bot' && target.id === r.bot))
    ? { kind: 'bot-tab', bot: r.bot, tab: r.tab } : null;
}
export function compactSession(s, target = null) {
  if (!record(s) || Object.keys(s).some(k => !['sel', 'open', 'adding', 'panel', 'info', 'route'].includes(k))) return null;
  if (!bounded(s.sel, 1024) || typeof s.adding !== 'boolean' || typeof s.panel !== 'boolean') return null;
  const open = optionsMap(s.open, true), info = optionsMap(s.info, false);
  const route = own(s, 'route') ? compactRoute(s.route, target) : null;
  if (own(s, 'route') && !route) return null;
  return open && info ? { sel: s.sel, open, adding: s.adding, panel: s.panel, info, ...(route ? { route } : {}) } : null;
}
export function readPresentation(text) {
  if (typeof text !== 'string' || utf8Bytes(text) > 65536) return null;
  let v; try { v = JSON.parse(text); } catch { return null; }
  if (!record(v) || Object.keys(v).some(k => !['version', 'current', 'hubs'].includes(k)) ||
      v.version !== 1 || !bounded(v.current, 1024) || !Array.isArray(v.hubs) || v.hubs.length > 16) return null;
  const hubs = [], seen = new Set();
  for (const h of v.hubs) {
    if (!record(h) || Object.keys(h).some(k => !['hub', 'target', 'session', 'anchor'].includes(k)) ||
        !bounded(h.hub, 1024) || !h.hub || seen.has(h.hub) || !viewTarget(h.target)) return null;
    const session = compactSession(h.session, h.target), a = h.anchor;
    if (!session || !record(a) || Object.keys(a).length !== 2 || !bounded(a.id, 1024) || !integer(a.offset)) return null;
    seen.add(h.hub); hubs.push({ hub: h.hub, target: snapshot(h.target), session, anchor: { id: a.id, offset: a.offset } });
  }
  if (v.current && !seen.has(v.current)) return null;
  return { version: 1, current: v.current, hubs };
}
export function writePresentation(value) {
  let text; try { text = JSON.stringify(value); } catch { return null; }
  const checked = readPresentation(text);
  return checked ? JSON.stringify(checked) : null;
}
// Validate every command before returning ANY dispatch descriptor. The
// renderer-specific view validator also runs before callbacks, atomically.
export function viewCommand(c, hub) {
  if (!record(c) || !bounded(c.type, 32)) return false;
  if (c.type === 'send') {
    if ((own(c, 'hub') && c.hub !== hub) || !bounded(c.text, 131072)) return false;
    let p; try { p = JSON.parse(c.text); } catch { return false; }
    return record(p) && integer(p.id, VIEW_ID_BASE - 1) && p.id > 0 && bounded(p.m, 128) &&
      !p.m.startsWith('view.') && p.m !== 'mobile.history' && p.m !== 'frontend.history';
  }
  if (c.type === 'keep') return bounded(c.thread, 2048) && bounded(c.text, 131072);
  if (c.type === 'focus' || c.type === 'jump') return bounded(c.id, 2048);
  if (c.type === 'scroll') return true;
  if (c.type === 'copy') return bounded(c.text, 131072);
  if (c.type === 'later') return integer(c.ms) && bounded(c.action, 128) && bounded(c.value, 2048);
  if (c.type === 'connect') return bounded(c.url, 8192) && (c.url === '' || /^(?:https?|wss?):\/\/[^\s\x00-\x1f\x7f]+$/.test(c.url));
  if (c.type === 'notify') return (!own(c, 'hub') || c.hub === hub) && bounded(c.thread, 2048) && bounded(c.title, 4096) && bounded(c.body, 65536) && bounded(c.key, 2048);
  return false;
}
export class ThinViews {
  constructor(api, next = VIEW_ID_BASE + Date.now()) {
    this.api = api; this.next = next; this.links = new Map(); this.calls = new Map();
  }
  allocate() {
    if (!integer(this.next, MAX_ID - 1) || this.next < VIEW_ID_BASE) throw new Error('view request counter exhausted');
    return ++this.next;
  }
  connect(hub, greeting) {
    if (!bounded(hub, 1024) || !hub || !viewGreeting(greeting)) return { compatibilityRequired: true, cmds: [] };
    const abandoned = this.disconnect(hub, 'connection changed');
    const link = { hub, greeting: snapshot(greeting), revision: 0, pictureRevision: 0, terminalOwner: '', terminalOpen: false, viewId: '', target: null, shown: null, viewRevision: '', updatePending: false, updateId: null, invalidated: false, changedRevision: null, readInvalidated: false };
    this.links.set(hub, link);
    return { connected: true, abandoned, cmds: [] };
  }
  disconnect(hub, reason = 'disconnected') {
    const abandoned = [];
    for (const [id, call] of this.calls) if (call.link.hub === hub) {
      abandoned.push({ id, method: call.method, target: call.target, reason, retry: false, issued: false }); this.calls.delete(id);
    }
    this.links.delete(hub); return abandoned;
  }
  select(hub, target, session, cid) {
    const l = this.links.get(hub);
    if (!l || !viewTarget(target)) return { failed: 'invalid view selection', cmds: [] };
    if (session && own(session, 'route') && !compactRoute(session.route, target)) return { failed: 'invalid view route', cmds: [] };
    if (l.revision >= MAX_REVISION) return { failed: 'view revision exhausted', cmds: [] };
    l.target = snapshot(target); l.accountBinding = ''; l.viewId = ''; l.viewRevision = ''; l.updatePending = false; l.updateId = null; l.invalidated = false; l.changedRevision = null; l.readInvalidated = false;l.sidebar = null;l.terminalOwner = '';l.terminalOpen = false;
    return this.request(hub, 'view.open', { session, cid });
  }
  request(hub, method, args = {}) {
    const l = this.links.get(hub);
    if (!l || !l.target || !METHODS.has(method)) return { failed: 'view unavailable', cmds: [] };
    if (method !== 'view.open' && !l.viewId) return { failed: 'view not ready', cmds: [] };
    if (method === 'view.update' && l.updatePending) { l.invalidated = true; l.readInvalidated = true; return { coalesced: true, cmds: [] }; }
    let count = 0;
    for (const c of this.calls.values()) if (c.link === l) count++;
    if (count >= 8 || this.calls.size >= 128) return { failed: 'pending request limit', cmds: [] };
    const previousPicture = l.pictureRevision;
    if (method !== 'view.cancel') {
      if (l.revision >= MAX_REVISION) return { failed: 'view revision exhausted', cmds: [] };
      l.revision++;
      l.pictureRevision = l.revision;
    }
    // Callers supply page/search/chunk requests only after a real gesture or
    // explicit query. Invalidations cannot generate those requests.
    let extra;
    try { extra = snapshot(args); } catch { return { failed: 'invalid request', cmds: [] }; }
    if (!record(extra) || utf8Bytes(JSON.stringify(extra)) > VIEW_LIMITS.replyBytes) return { failed: 'request limit', cmds: [] };
    // Compact input cannot change the selected picture. Keep the wire counter
    // monotonic without starving a canonical read during continuous typing.
    // This is an intent only: a validated full fallback becomes a barrier below.
    const terminalInputIntent = method === 'view.action' && l.greeting.renderer === 'web' && !!l.viewId && !!l.shown &&
      extra.terminalInput === true && ['term-key','term-paste'].includes(extra.action) &&
      extra.session?.open?.term === true && l.terminalOpen && bounded(extra.terminalOwner, 1024) &&
      extra.terminalOwner.length > 0 && extra.terminalOwner === l.terminalOwner;
    // Negotiated body queries publish text through their own source fence.
    // They cannot replace a picture or retire an in-flight canonical refresh.
    const bodyReadIntent = method === 'view.message' && extra.bodyPage === 1 &&
      l.greeting.renderer === 'web' && this.api.bodyPages === true;
    if (terminalInputIntent || bodyReadIntent) l.pictureRevision = previousPicture;
    let id; try { id = this.allocate(); } catch (e) { return { failed: e.message, cmds: [] }; }
    const p = { ...extra, version: 1, hub, generation: l.greeting.generation, epoch: l.greeting.epoch,
      requestRevision: l.revision, target: snapshot(l.target), viewId: l.viewId };
    if (l.greeting.renderer === 'web') { p.accountProfiles = 1; p.accountBinding = l.accountBinding || ''; }
    if (l.greeting.renderer === 'web' && this.api.bodyPages === true) p.bodyPages = 1;
    const regionEnabled = this.api.sidebarRegions === true && l.greeting.renderer === 'web'
      && ['view.open','view.page','view.update','view.action'].includes(method);
    if (regionEnabled) p.sidebarRegion = sidebarOffer(l.sidebar);
    const call = { id, link: l, method, target: snapshot(l.target), revision: l.revision, viewId: l.viewId,
      began: this.api.now(), input: snapshot(p), terminalInputIntent, sidebarBase: regionEnabled ? l.sidebar : null, explicitRead: method === 'view.update' && (l.readInvalidated || l.changedRevision === null) };
    if (method === 'view.update') { l.updatePending = true; l.updateId = id; l.invalidated = false; l.readInvalidated = false; }
    this.calls.set(id, call);
    return { id, cmds: [this.api.send(hub, { id, m: method, p })] };
  }
  changed(hub, e, session, anchor, defer = false) {
    const l = this.links.get(hub);
    if (!l || !record(e) || e.t !== 'view.changed' || e.version !== 1 || e.generation !== l.greeting.generation ||
        e.epoch !== l.greeting.epoch || e.viewId !== l.viewId || !sameTarget(e.target, l.target) ||
        !revision(e.viewRevision) || utf8Bytes(JSON.stringify(e)) > 1024) return { ignored: true, cmds: [] };
    if (coversRevision(l.viewRevision, e.viewRevision)) return { ignored: true, cmds: [] };
    if (!coversRevision(l.changedRevision, e.viewRevision)) l.changedRevision = e.viewRevision;
    if (defer || l.updatePending) { l.invalidated = true; return { coalesced: true, cmds: [] }; }
    return this.request(hub, 'view.update', { session, anchor, viewRevision: l.viewRevision });
  }
  retireUpdate(call) {
    const l = this.links.get(call.link.hub);
    if (call.method !== 'view.update' || l !== call.link || l.updateId !== call.id) return false;
    l.updatePending = false; l.updateId = null;
    // A newer auxiliary query can retire display authority without consuming
    // the selected view's invalidation. Preserve that outstanding refresh.
    if (l.viewId === call.viewId && sameTarget(l.target, call.target) && l.pictureRevision !== call.revision) {
      l.invalidated = true;
      if (call.explicitRead) l.readInvalidated = true;
    }
    return true;
  }
  reply(hub, e) {
    if (!record(e) || !integer(e.id, MAX_ID) || e.id <= VIEW_ID_BASE) return null;
    const c = this.calls.get(e.id), l = this.links.get(hub);
    if (!c || c.link !== l || c.link.hub !== hub) return null;
    this.calls.delete(e.id);
    let current = (c.terminalInputIntent || c.method === 'view.cancel' ? l.revision : l.pictureRevision) === c.revision && sameTarget(l.target, c.target);
    this.retireUpdate(c);
    if (e.ok !== true) return { failed: record(e.error) ? e.error.code : 'UNAVAILABLE', id: e.id, method: c.method, retry: false, cmds: [] };
    let r = e.result;
    if (!record(r) || utf8Bytes(JSON.stringify(r)) > l.greeting.limits.replyBytes || r.version !== 1 ||
        r.hub !== hub || r.generation !== l.greeting.generation || r.epoch !== l.greeting.epoch ||
        r.requestRevision !== c.revision || !sameTarget(r.target, c.target) ||
        !bounded(r.viewId, 128) || !r.viewId || (c.method !== 'view.open' && r.viewId !== c.viewId) ||
        !bounded(r.viewRevision, 32) || !/^(0|[1-9][0-9]*)$/.test(r.viewRevision) || r.renderer !== l.greeting.renderer ||
        !Array.isArray(r.cmds) || (c.method !== 'view.action' && r.cmds.length !== 0)) {
      return { failed: 'invalid view reply', id: e.id, method: c.method, retry: false, cmds: [] };
    }
    // Commands are not display authority: an old action's one matching reply
    // still settles that action, without replacing a newer selected view.
    if (r.cmds.length > 32 || r.cmds.some(x => !viewCommand(x, hub))) return { failed: 'invalid view command', id: e.id, method: c.method, retry: false, cmds: [] };
    const inputOnly = Object.hasOwn(r, 'terminalInput') || Object.hasOwn(r, 'modelMenuInput');
    if (inputOnly && (c.method !== 'view.action' || l.greeting.renderer !== 'web' ||
        !(Object.hasOwn(r, 'modelMenuInput') ? modelMenuReply(r, c.input) : terminalInputReply(r, c.input)))) {
      return { failed: 'invalid compact action reply', id: e.id, method: c.method, retry: false, cmds: [] };
    }
    let sidebar;
    if (Object.hasOwn(r, 'sidebarRegion')) {
      try {
        sidebar = sidebarPicture(r, c.input, c.sidebarBase);r = sidebar.result;
        // A compressed reply grants no larger logical presentation budget.
        if (utf8Bytes(JSON.stringify(r)) > l.greeting.limits.replyBytes) throw Error('Expanded view limit');
      } catch { return { failed: 'invalid sidebar region', id: e.id, method: c.method, retry: false, cmds: [] }; }
    }
    if (this.api.validateResult && this.api.validateResult(r, c.input) !== true) return { failed: 'invalid view presentation', id: e.id, method: c.method, retry: false, cmds: [] };
    if (c.terminalInputIntent && inputOnly) current = current && l.terminalOpen && l.terminalOwner === c.input.terminalOwner;
    // Only a fully validated, ready full fallback may promote an exempt intent
    // into picture authority. Its original ordinal cannot overtake navigation.
    if (c.terminalInputIntent && !inputOnly && record(r.shown) && r.remoteReady === true &&
        l.viewId === c.viewId && sameTarget(l.target, c.target)) {
      l.pictureRevision = Math.max(l.pictureRevision, c.revision);
      current = l.pictureRevision === c.revision;
    }
    const cmds = c.method === 'view.action' ? r.cmds.map(x => this.api.command(snapshot(x), c.input)) : [];
    const presentation = ['view.open', 'view.page', 'view.update', 'view.action'].includes(c.method);
    const displayed = current && presentation && record(r.shown) && r.remoteReady === true;
    if (displayed) {
      l.viewId = r.viewId; l.viewRevision = r.viewRevision; l.shown = r.shown;
      if (l.greeting.renderer === 'web') l.accountBinding = typeof r.accountBinding === 'string' ? r.accountBinding : '';
      l.terminalOpen = r.session?.open?.term === true;
      l.terminalOwner = bounded(r.session?.drafts?.['@terminal.owner'], 1024) ? r.session.drafts['@terminal.owner'] : '';
      l.sidebar = sidebar?.cache || null;
      // A full matching action picture already includes older canonical
      // invalidations. Compact ACKs and auxiliary replies cover no picture.
      coverChanged(l, r.viewRevision);
    } else if (!current && c.method === 'view.update' && record(r.shown) && r.remoteReady === true &&
        l.viewId === c.viewId && sameTarget(l.target, c.target)) {
      if (!coversRevision(l.changedRevision, r.viewRevision)) l.changedRevision = r.viewRevision;
      coverChanged(l, l.viewRevision);
    }
    return { id: e.id, method: c.method, current, inputOnly, remoteReady: displayed || current && inputOnly && record(l.shown),
      shown: displayed ? r.shown : null, session: current && presentation ? r.session : null,
      window: displayed ? r.window : null, result: r, cmds,
      refreshDue: (current || (l.viewId === c.viewId && sameTarget(l.target, c.target))) && !l.updatePending && l.invalidated };
  }
  expire(timeoutMs = 5000) {
    const failed = [];
    for (const [id, c] of this.calls) if (this.api.now() - c.began >= timeoutMs) {
      this.calls.delete(id);
      this.retireUpdate(c);
      failed.push({ id, hub: c.link.hub, method: c.method, target: c.target, retry: false, issued: false, cmds: [] });
    }
    return failed;
  }
}
