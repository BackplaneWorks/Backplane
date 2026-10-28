import { ThinViews, compactSession, compactRoute, viewTarget, readPresentation, writePresentation, utf8Bytes } from '../web/thin.js';
import { validateThinResult, MessageParts } from '../web/thin-results.js';
import { validatePresentation } from '../web/startup.js';

// Native adapter only. ThinViews owns every wire identity, request id,
// target/revision fence and command-batch validation. Bend owns intent and
// rendering. No canonical State, cursor or history is loaded here.
const copy = value => JSON.parse(JSON.stringify(value));
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const presentation = method => ['view.open', 'view.page', 'view.update', 'view.action'].includes(method);
const same = (a, b) => a?.kind === b?.kind && a?.id === b?.id;
const counter = n => Number.isSafeInteger(n) && n >= 0 && n < 2 ** 47;
const ordinaryKey = (hub, id) => JSON.stringify([hub, id]);
const rawThread = (hub, qualified) => qualified.startsWith(hub + '|') ? qualified.slice(hub.length + 1) : null;
const plainObject = value => object(value) && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const sidebarLists = ['threads', 'snoozed', 'settled', 'archived'];
const sidebarProjectKeys = ['settledCount', 'archivedCount', ...sidebarLists.map(k => k + 'Page')];
const exactKeys = (value, keys) => plainObject(value) && Reflect.ownKeys(value).length === keys.length && keys.every(k => Object.hasOwn(value, k));
const safeCount = value => Number.isSafeInteger(value) && value >= 0;
const sidebarString = (value, max) => typeof value === 'string' && value.length > 0 && utf8Bytes(value) <= max;
const qualifiedSidebarID = (value, hub) => sidebarString(value, 2049) && value.startsWith(hub + '|') && value.length > hub.length + 1;
const sidebarStable = value => JSON.stringify(value, (_, v) => plainObject(v) ? Object.fromEntries(Object.keys(v).sort().map(k => [k, v[k]])) : v);
const hasSidebarMetadata = screen => object(screen) && (Object.hasOwn(screen, 'nativeSidebar') || Object.hasOwn(screen, 'sidebarPages') ||
  (Array.isArray(screen.projects) && screen.projects.some(p => object(p) && sidebarProjectKeys.some(k => Object.hasOwn(p, k)))));

function sidebarControl(control, hub) {
  return control === null || exactKeys(control, ['label', 'action', 'value']) &&
    sidebarString(control.label, 1024) && control.action === 'native-sidebar-page' &&
    typeof control.value === 'string' && control.value.startsWith(hub + '|') &&
    sidebarString(control.value.slice(hub.length + 1), 2048);
}

// These checks concern only a negotiated display extension. Tokens remain
// opaque; the server authenticates their canonical membership and signature.
function sidebarRange(page, rows, hub, selected, projects = false) {
  if (!Array.isArray(rows)) throw Error('Invalid sidebar rows');
  const ids = rows.map(row => row?.id);
  if (ids.some(id => !qualifiedSidebarID(id, hub)) || new Set(ids).size !== ids.length) throw Error('Invalid sidebar identities');
  if (page === null) {
    if (rows.length) throw Error('Unpaged sidebar rows');
    return { ordinary: 0, extra: 0 };
  }
  if (!exactKeys(page, ['version', 'total', 'offset', 'limit', 'selectedOutside', 'previous', 'next']) || page.version !== 1 ||
      !safeCount(page.total) || page.total === 0 || !safeCount(page.offset) || page.offset >= page.total ||
      !safeCount(page.limit) || page.limit < 1 || page.limit > (projects ? 8 : 48) ||
      typeof page.selectedOutside !== 'boolean' || !sidebarControl(page.previous, hub) || !sidebarControl(page.next, hub))
    throw Error('Invalid sidebar page');
  const ordinary = Math.min(page.limit, page.total - page.offset), extra = page.selectedOutside ? 1 : 0;
  if (rows.length !== ordinary + extra || (page.previous !== null) !== (page.offset > 0) ||
      (page.next !== null) !== (ordinary < page.total - page.offset) ||
      (extra && (!selected || (!projects && !ids.includes(selected))))) throw Error('Invalid sidebar range');
  return { ordinary, extra };
}

function sidebarPicture(screen, hub, selected) {
  if (!Array.isArray(screen.sidebarPages) || screen.sidebarPages.length !== 1) throw Error('Invalid sidebar owners');
  const descriptor = screen.sidebarPages[0];
  if (!exactKeys(descriptor, ['version', 'hub', 'projects', 'active', 'legacy']) || descriptor.version !== 1 || descriptor.hub !== hub)
    throw Error('Invalid sidebar owner');
  const projects = screen.projects, active = screen.active, legacy = screen.legacy;
  const projectRange = sidebarRange(descriptor.projects, projects, hub, selected, true);
  let ordinary = 0, extras = 0;
  const allIDs = new Set(), projectShape = [];
  const countRows = (page, rows) => {
    const range = sidebarRange(page, rows, hub, selected);
    ordinary += range.ordinary; extras += range.extra;
    for (const row of rows) {
      if (allIDs.has(row.id)) throw Error('Duplicate sidebar row');
      allIDs.add(row.id);
    }
  };
  let selectedProject = null;
  for (const project of projects) {
    if (!sidebarProjectKeys.every(k => Object.hasOwn(project, k)) ||
        !safeCount(project.settledCount) || !safeCount(project.archivedCount) ||
        typeof project.open !== 'boolean' || typeof project.archOpen !== 'boolean') throw Error('Invalid sidebar counts');
    if (!project.open && (project.settledPage !== null || project.settled?.length !== 0) ||
        !project.archOpen && (project.archivedPage !== null || project.archived?.length !== 0)) throw Error('Closed sidebar rows');
    if (project.archivedCount > 0 && !qualifiedSidebarID(project.value, hub)) throw Error('Missing archive control');
    for (const list of sidebarLists) {
      countRows(project[list + 'Page'], project[list]);
      if (project[list].some(row => row.id === selected)) selectedProject = project.id;
    }
    // Shelf counts describe source roots; expanded descendant totals can differ.
    if (project.settledCount === 0 && project.settled.length || project.archivedCount === 0 && project.archived.length)
      throw Error('Sidebar count mismatch');
    projectShape.push([project.id, project.open, project.archOpen, project.settledCount, project.archivedCount,
      ...sidebarLists.map(list => [project[list].map(row => row.id), project[list + 'Page']])]);
  }
  countRows(descriptor.active, active); countRows(descriptor.legacy, legacy);
  if (ordinary > 48 || extras > 1 || projectRange.ordinary > 8 || projectRange.extra > 1 ||
      (projectRange.extra && selectedProject && !projects.some(project => project.id === selectedProject))) throw Error('Sidebar budget exceeded');
  // If the selected thread lives in a closed shelf, its project membership is
  // known only to the authoritative planner. Do not invent a row to prove it.
  return { descriptor, projects: projectShape, active: active.map(row => row.id), legacy: legacy.map(row => row.id) };
}

export function validateNativeSidebar307265(result, input, offered = false) {
  try {
    const shown = result.shown, overlay = result.overlay?.j;
    const metadata = hasSidebarMetadata(shown) || hasSidebarMetadata(overlay);
    if (!metadata) return true; // Older hubs keep their existing arrays.
    if (!offered || !object(shown) || !object(overlay) || shown.nativeSidebar !== 1 || overlay.nativeSidebar !== 1)
      return false;
    const selected = typeof shown.sel === 'string' && shown.sel ? (shown.sel.startsWith(input.hub + '|') ? shown.sel : input.hub + '|' + shown.sel) : '';
    const a = sidebarPicture(shown, input.hub, selected), b = sidebarPicture(overlay, input.hub, selected);
    return sidebarStable(a) === sidebarStable(b);
  } catch { return false; }
}

function sidebarPreservedMap(actual, captured, normalized = {}, known = {}) {
  return plainObject(actual) && plainObject(captured) &&
    Object.entries(captured).every(([key, value]) => Object.hasOwn(actual, key) && actual[key] === value) &&
    Object.entries(actual).every(([key, value]) => Object.hasOwn(captured, key) ? value === captured[key] :
      Object.hasOwn(normalized, key) ? value === normalized[key] :
        Object.hasOwn(known, key) && value === known[key]);
}

// A sidebar read does not inherit the mutation/redirect permissions of other
// view.action replies. Run this inside ThinViews' atomic validation callback,
// before it installs a picture or returns any command dispatch descriptors.
export function validateNativeSidebarAction307265(result, input, known = {}) {
  if (input.action !== 'native-sidebar-page') return true;
  try {
    const session = result.session, captured = input.session;
    if (!Array.isArray(result.cmds) || result.cmds.length || Object.hasOwn(result, 'nextTarget') ||
        result.remoteReady !== true || !plainObject(session) || !plainObject(captured) ||
        Object.keys(session).some(key => !['sel', 'drafts', 'open', 'adding', 'panel', 'rid', 'wantNew', 'error', 'note', 'info', 'outbox', 'route'].includes(key)) ||
        session.sel !== captured.sel || session.rid !== captured.rid || session.adding !== captured.adding ||
        session.panel !== captured.panel || session.wantNew !== captured.wantNew ||
        sidebarStable(session.route) !== sidebarStable(captured.route) || sidebarStable(session.open) !== sidebarStable(captured.open) ||
        !plainObject(session.outbox) || !Object.entries(session.outbox).every(([key, value]) =>
          typeof value === 'string' && Object.hasOwn(known.outbox || {}, key) && known.outbox[key] === value)) return false;
    // P.Session.input/Ui.online and the actor selector add these scratch
    // values. Selector values are derived from the captured target/route;
    // an arbitrary new @btab or @bot is not an allowed normalization.
    const target = input.target, selectors = { '@presentation': '', '@bot': target?.kind === 'bot' ? target.id : '',
      '@room': target?.kind === 'room' ? target.id : '', '@remote': '', '@btab': captured.route?.tab || '' };
    if (!sidebarPreservedMap(session.drafts, captured.drafts,
      { '@boot': '', '@sync': '1', '@mech.at': '', '@solid.at': '', '@desk': '', ...selectors }, known.drafts) ||
        !sidebarPreservedMap(session.info, captured.info, {}, known.info)) return false;
    const qualified = id => typeof id !== 'string' ? null : !id ? '' : id.startsWith(input.hub + '|') ? id : input.hub + '|' + id;
    const selected = qualified(captured.sel);
    for (const screen of [result.shown, result.overlay?.j]) {
      if (!object(screen) || screen.nativeSidebar !== 1 || qualified(screen.sel) !== selected ||
          (screen.thread != null && qualified(screen.thread.id) !== selected) ||
          (input.target?.kind === 'bot' && screen.bot != null && qualified(screen.bot.id) !== qualified(input.target.id))) return false;
    }
    return true;
  } catch { return false; }
}
// Ordinary transport side channels are not presentation authority. Never
// retain, copy or log their values: pair/pair.qr can contain credentials.
export function inertAncillary307265(envelope) {
  if (!plainObject(envelope)) return false;
  const keys = Reflect.ownKeys(envelope);
  if (envelope.t === 'hb') return keys.length === 1 && keys[0] === 't';
  // Server source revision is a plot side channel, not a presentation or
  // geometry frame. Do not retain it or let it affect a pending deadline.
  if (envelope.t === 'plotrevision') return keys.length === 3 &&
    keys.includes('t') && keys.includes('key') && keys.includes('revision') &&
    typeof envelope.key === 'string' && /^[0-4]\|[^|]+\|/.test(envelope.key) &&
    typeof envelope.revision === 'string' && envelope.revision.length > 0 &&
    utf8Bytes(JSON.stringify(envelope)) <= 65536;
  // a 3D model's note (server plot.bend's Pv.note: parts with no model) to
  // the phones watching it: not a view frame. Taken for a stray frame it
  // failed the view once a model loaded, and every tap after was dropped.
  if (envelope.t === 'plotnote') return keys.every(k => k === 't' || k === 'key' || k === 'text') &&
    typeof envelope.key === 'string' && typeof envelope.text === 'string' && utf8Bytes(JSON.stringify(envelope)) <= 65536;
  if (envelope.t !== 'info' || keys.length !== 2 || !keys.includes('t') || !keys.includes('info') || !plainObject(envelope.info)) return false;
  const info = Reflect.ownKeys(envelope.info);
  return info.length === 2 && info.includes('pair') && info.includes('pair.qr') &&
    typeof envelope.info.pair === 'string' && typeof envelope.info['pair.qr'] === 'string' && utf8Bytes(JSON.stringify(envelope)) <= 262144;
}

// Validate native structures without asking the web VDOM decoder to accept
// them. Total reply bytes are already capped by ThinViews before callbacks.
export function validateNativeResult307265(result, input) {
  try {
    validatePresentation({ session: result.session, view: { type: 'text', text: '' }, cmds: result.cmds });
    if (!counter(result.session.rid)) return false;
    if (result.cmds.some(c => (c.type === 'keep' && rawThread(input.hub, c.thread) === null) ||
        (c.type === 'later' && rawThread(input.hub, c.value) === null))) return false;
    if (result.session.route !== undefined && !compactRoute(result.session.route, result.nextTarget || input.target)) return false;
    // A navigation answers the person's action, or the update that follows
    // its ordinary request (a new thread or fork: thread.create's answer,
    // then view.update names the thread). Refusing the second failed the
    // view: "New thread" waited for good and nothing applied after it.
    if (result.nextTarget) return viewTarget(result.nextTarget) && !same(result.nextTarget, input.target) &&
      (typeof input.action === 'string' || typeof input.viewRevision === 'string') &&
      result.remoteReady === false && !Object.prototype.hasOwnProperty.call(result, 'shown');
    const w = result.window, str = (s, limit = 1024) => typeof s === 'string' && utf8Bytes(s) <= limit;
    if (!object(w) || !Array.isArray(w.ids) || w.ids.length > 32 || w.ids.some(id => !str(id) || !id) || new Set(w.ids).size !== w.ids.length ||
        !str(w.older) || typeof w.hasMore !== 'boolean' || (w.hasMore && !w.older) || !object(w.anchor) || !str(w.anchor.id) ||
        !Number.isInteger(w.anchor.offset) || w.anchor.offset < 0 || w.anchor.offset > 4294967295 || !Array.isArray(w.partial) || w.partial.length > 33 ||
        w.partial.some(p => !object(p) || !str(p.id) || !p.id || !str(p.cursor) || !p.cursor || p.reason !== 'byte-limit' ||
          !str(p.version, 32) || !/^(0|[1-9][0-9]*)$/.test(p.version) || p.hasMore !== true || p.complete !== false) ||
        new Set(w.partial.map(p => p.id)).size !== w.partial.length) return false;
    if (result.remoteReady !== true || !object(result.shown) || result.shown.hub !== input.hub || typeof result.shown.sel !== 'string' ||
        !object(result.overlay) || result.overlay.key !== input.hub || typeof result.overlay.name !== 'string' || !object(result.overlay.j)) return false;
    if (typeof result.shown.online !== 'boolean' || ['version','error','note','empty'].some(k => typeof result.shown[k] !== 'string') ||
        ['hubs','found','projects','bots','rooms'].some(k => !Array.isArray(result.shown[k]))) return false;
    let remaining = 20000;
    const stack = [[result.shown, 0], [result.overlay.j, 0]];
    while (stack.length) {
      const [value, depth] = stack.pop();
      if (--remaining < 0 || depth > 64) return false;
      if (value === null || typeof value === 'boolean' || typeof value === 'string') continue;
      if (typeof value === 'number') { if (!Number.isFinite(value)) return false; continue; }
      if (typeof value !== 'object') return false;
      for (const item of Object.values(value)) stack.push([item, depth + 1]);
    }
    // Normal link/fold headers may double 32 logical entries; live adds one.
    for (const shown of [result.shown, result.overlay.j]) {
      if (shown.thread != null && (!object(shown.thread) || typeof shown.thread.id !== 'string' ||
          !Array.isArray(shown.thread.entries) || shown.thread.entries.length > 65 ||
          shown.thread.entries.some(e => !object(e) || typeof e.id !== 'string' || typeof e.kind !== 'string' || typeof e.text !== 'string' ||
            (e.blocks != null && !Array.isArray(e.blocks)) || (e.partial != null && (!object(e.partial) || typeof e.partial.version !== 'string' ||
              typeof e.partial.cursor !== 'string' || typeof e.partial.hasMore !== 'boolean' || typeof e.partial.complete !== 'boolean'))))) return false;
      const synthetic = shown.thread?.entries.filter(e => e.id.startsWith('@live:')) || [];
      const owner = shown.thread ? rawThread(input.hub, shown.thread.id) : null;
      if (synthetic.length > 1 || (synthetic.length && (owner === null || synthetic.some(e => e.id !== '@live:' + owner)))) return false;
    }
    return true;
  } catch { return false; }
}

export class NativeThinHost307265 {
  constructor(api, next) {
    this.api = api;
    this.states = new Map(); this.paired = new Set(); this.current = '';
    this.pending = new Map(); this.ordinary = new Map(); this.commands = []; this.sync = [];
    this.commandOwners = new WeakMap();
    // Opted in only by a native engine that acknowledges delivered screens.
    this.publicationAck = false;
    // Offered only by an engine whose native views implement shelf counts
    // and paging controls. Keep each offer tied to its exact request; enabling
    // support cannot authorize metadata on an earlier in-flight response.
    this.nativeSidebar = false;
    this.sidebarOffers = new WeakSet();
    this.sidebarCaptures = new WeakMap();
    this.projectSearch = null;
    // The outbox: a message sent is kept until its hub answers. A drop
    // before the answer (or a send while the socket is down, which the app
    // cannot deliver) leaves it here; each view connection that presents
    // sends it again. The hub stores a client-named message once (Hub.dup),
    // so a resend never posts twice; a refusal puts the text back in its
    // draft. Kept for a day; keyed by the message's own id.
    this.outbox = new Map();
    this.client = new ThinViews({
      now: api.now,
      // Renderer hints are device-owned and never select another hub or owner.
      send: (hub, request) => {
        const sidebar = this.nativeSidebar && presentation(request.m);
        const input = this.client.calls.get(request.id).input;
        if (sidebar) this.sidebarOffers.add(input);
        if (request.p.action === 'native-sidebar-page') {
          const session = this.states.get(hub).session;
          // Optional actor-owned fields may be echoed, never changed. This
          // private baseline is not sent on the wire or persisted.
          this.sidebarCaptures.set(input, { drafts: copy(session.drafts), info: copy(session.info), outbox: copy(session.outbox || {}) });
        }
        return { type: 'request', hub, request: { ...request,
          p: { ...request.p, ...(request.p.session ? { session: this.requestSession(request.p.session, request.m, request.p.target) } : {}),
            nativeRenderer: api.renderer(), ...(sidebar ? { nativeSidebar: 1 } : {}) } } };
      },
      command: (command, input) => ({ type: 'command', command, input }),
      validateResult: (result, input) => this.validate(result, input),
    }, next);
  }

  state(hub) {
    let value = this.states.get(hub);
    if (!value) {
      const session = copy(this.api.session(hub));
      if (!counter(session.rid)) throw Error('Invalid ordinary request counter');
      session.rid = Math.max(session.rid, this.api.now());
      const drafts = new Map(Object.entries(this.api.drafts?.(hub) || {}).map(([thread, text]) => [thread, { revision: 0, text }]));
      session.drafts = { ...session.drafts, ...Object.fromEntries([...drafts].map(([thread, d]) => [thread, d.text])) };
      value = { hub, session, target: this.api.target(session), anchor: { id: '', offset: 0 },
        mode: '', epoch: '', greeted: false, ready: false, presentedRevision: 0, shown: null, window: null,
        // history pages (as the web's thin-host): the cursor shown ('' the
        // newest), the cursors that led here, the one page on its way, and
        // pages a send overtook (their replies are dropped)
        cursor: '', back: [], paging: null, overtaken: new Set(),
        revision: 0, drafts, queue: [], active: null, messages: new MessageParts(), findRevision: 0, findCloseOnce: false, registrations: new Map(),
        shownTarget: null, deferredNavigation: null, attachmentEnds: [], projectQuery: null, projectApplied: null, followup: null };
      this.states.set(hub, value);
    }
    return value;
  }

  hubs(keys) {
    const next = new Set(keys);
    for (const key of new Set([...this.paired, ...this.states.keys(), ...[...this.outbox.values()].map(o => o.hub)]))
      if (!next.has(key)) this.forgetHub(key);
    this.paired = next;
    if (!this.paired.has(this.current)) this.current = keys[0] || '';
  }

  // A pairing change is an identity boundary, unlike a network reconnect.
  // Never carry pending sends or local presentation state to a new pairing at
  // the same address. No credential is passed into this adapter.
  forgetHub(hub) {
    this.disconnect(hub);
    this.states.delete(hub);
    this.paired.delete(hub);
    for (const [msg, item] of this.outbox) if (item.hub === hub) this.outbox.delete(msg);
    this.commands = this.commands.filter(c => this.commandOwners.get(c) !== hub);
    this.sync = this.sync.filter(s => s.hub !== hub);
    if (this.current === hub) this.current = '';
  }

  reset() {
    for (const key of this.states.keys()) this.client.disconnect(key);
    this.states.clear(); this.paired.clear(); this.current = '';
    this.projectSearch = null;
    this.pending.clear(); this.ordinary.clear(); this.commands = []; this.sync = [];
    this.outbox.clear();
  }

  enqueue(hub, ...commands) {
    for (const command of commands) this.commandOwners.set(command, hub);
    this.commands.push(...commands);
  }

  outboxKeep(hub, rpc) {
    const msg = rpc?.p?.msg;
    if (rpc?.m !== 'turn.start' || typeof msg !== 'string' || !msg || this.outbox.size >= 64) return;
    this.outbox.set(msg, { hub, rpc: copy(rpc), at: this.api.now() });
  }

  // a hub's answer to a message in the outbox; true when it was one
  outboxReply(hub, envelope) {
    for (const [msg, o] of this.outbox) if (o.hub === hub && o.rpc.id === envelope.id) {
      this.outbox.delete(msg);
      if (envelope.ok === false) {
        const thread = String(o.rpc.p.thread || ''), s = this.states.get(hub);
        if (thread && s && !s.session.drafts[thread]) {
          this.enqueue(hub, { type: 'keep', thread: hub + '|' + thread, text: String(o.rpc.p.text || '') });
          s.session.drafts = { ...s.session.drafts, [thread]: String(o.rpc.p.text || '') };
        }
      }
      return true;
    }
    return false;
  }

  // a presented view connection: what waits in the outbox goes again
  outboxFlush(hub) {
    const now = this.api.now();
    for (const [msg, o] of this.outbox) {
      if (o.hub !== hub) continue;
      if (now - o.at > 86400000) { this.outbox.delete(msg); continue; }
      this.enqueue(hub, this.api.command(hub, { type: 'send', text: JSON.stringify(o.rpc) }, {}, false));
    }
  }


  connection(hub, epoch, mode) {
    if (!this.paired.has(hub)) return;
    if (this.projectSearch && !this.projectSearch.ended) this.projectSearchEnd(this.projectSearch.scope);
    this.disconnect(hub);
    const s = this.state(hub);
    s.mode = mode === 'view-v1' ? 'view-v1' : mode === undefined ? 'compatibility' : 'blocked';
    s.epoch = String(epoch); s.greeted = false; s.ready = false; s.flushed = false;
  }

  thin(hub) { return this.states.get(hub)?.mode === 'view-v1'; }

  inert(hub, envelope, epoch) {
    const s = this.states.get(hub);
    return this.paired.has(hub) && s?.mode === 'view-v1' && s.epoch === String(epoch) && inertAncillary307265(envelope);
  }

  requestSession(session, method, target) {
    const drafts = {};
    for (const key of ['@lang', '@lang.os', '@lang.tag', '@lang.preview'])
      if (typeof session.drafts?.[key] === 'string' && utf8Bytes(session.drafts[key]) <= 128) drafts[key] = session.drafts[key];
    if (method === 'view.action' && session.sel && Object.hasOwn(session.drafts || {}, session.sel) && typeof session.drafts[session.sel] === 'string')
      Object.defineProperty(drafts, session.sel, { value: session.drafts[session.sel], enumerable: true });
    const choices = (value, accept) => {
      const out = {}; let count = 0;
      for (const [key, item] of Object.entries(object(value) ? value : {})) {
        if (count >= 128) break;
        if (utf8Bytes(key) <= 1024 && accept(key, item)) {
          Object.defineProperty(out, key, { value: item, enumerable: true }); count++;
        }
      }
      return out;
    };
    const route = compactRoute(session.route, target);
    return { sel: session.sel, rid: session.rid, drafts, outbox: {},
      open: choices(session.open, (_key, item) => typeof item === 'boolean'),
      info: choices(session.info, (key, item) => (key.startsWith('fold:') || key.startsWith('subs:')) && typeof item === 'string' && utf8Bytes(item) <= 1024),
      adding: session.adding === true, panel: session.panel === true, wantNew: session.wantNew === true,
      ...(route ? { route } : {}) };
  }

  disconnect(hub) {
    if (this.projectSearch?.owners.has(hub)) this.projectSearchEnd(this.projectSearch.scope);
    const s = this.states.get(hub);
    this.client.disconnect(hub);
    for (const [id, call] of this.pending) if (call.hub === hub) this.pending.delete(id);
    for (const [id, call] of this.ordinary) if (call.hub === hub) this.ordinary.delete(id);
    if (s) {
      s.ready = false; s.greeted = false; s.active = null; s.queue = []; s.messages.clear();
      s.deferredNavigation = null; s.attachmentEnds = []; s.projectQuery = null; s.projectApplied = null; s.shownTarget = null; s.followup = null;
      s.registrations.clear();
    }
  }

  fail(hub, reason) {
    if (this.projectSearch?.owners.has(hub)) this.projectSearchEnd(this.projectSearch.scope);
    const s = this.states.get(hub);
    if (s) { s.ready = false; s.queue = []; s.deferredNavigation = null; s.attachmentEnds = []; s.projectQuery = null; s.followup = null; }
    this.sync.push({ hub, phase: 'failed', mode: 'view-v1', detail: String(reason), remoteReady: false });
  }

  issue(s, result, extra = {}) {
    if (!result || result.failed) { this.fail(s.hub, result?.failed || 'View unavailable'); return false; }
    if (!result.id) return true; // Coalesced update: no transition or request.
    const call = this.client.calls.get(result.id);
    if (!call) { this.fail(s.hub, 'Missing shared request'); return false; }
    this.pending.set(result.id, { hub: s.hub, epoch: s.epoch, revision: s.revision, input: copy(call.input), acks: [], ...extra });
    if (presentation(call.method)) {
      s.followup = null;
      s.ready = false;
      this.sync.push({ hub: s.hub, phase: 'presenting', detail: 'view-v1', mode: 'view-v1',
        requestRevision: call.revision, remoteReady: false });
    }
    // No adapter callback above dispatches. Validate every descriptor before
    // materializing the first send, then publish the batch with the sync fence.
    if (result.cmds.some(c => c.type !== 'request' || c.hub !== s.hub)) {
      this.fail(s.hub, 'Invalid request adapter'); return false;
    }
    const descriptors = result.cmds.map(c => this.api.send(c.hub, c.request));
    this.enqueue(s.hub, ...descriptors);
    return true;
  }

  greeting(hub, header, epoch) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1' || s.epoch !== String(epoch)) return false;
    if (s.greeted || header?.renderer !== 'mobile') { this.fail(hub, 'Invalid native view greeting'); return true; }
    const result = this.client.connect(hub, header);
    if (!result.connected) { this.fail(hub, 'Unsupported native view greeting'); return true; }
    s.greeted = true;
    this.select(s, s.target, s.session);
    return true;
  }

  pageTo(s, cursor, back) {
    const undo = { cursor: s.cursor, back: s.back };
    const result = this.client.request(s.hub, 'view.page', { cursor, session: this.requestSession(s.session, 'view.page', s.target) });
    s.cursor = cursor; s.back = back;
    if (!this.issue(s, result) || !result?.id) { s.cursor = undo.cursor; s.back = undo.back; return; }
    s.paging = { id: result.id, undo };
  }

  select(s, target, session) {
    if (this.projectSearch?.owners.has(s.hub)) this.projectSearchEnd(this.projectSearch.scope);
    s.deferredNavigation = null; s.attachmentEnds = [];
    s.projectQuery = null;
    s.projectApplied = null;
    s.followup = null;
    s.target = copy(target); s.session = copy(session); s.window = null;
    s.cursor = ''; s.back = []; s.paging = null;
    s.queue = []; s.messages.clear();
    return this.issue(s, this.client.select(s.hub, s.target, this.requestSession(s.session, 'view.open', s.target), this.api.cid()));
  }

  // Atomic local admission for native read-navigation; no wire/schema change.
  navigation(hub, action, value, presentedNav = null) {
    if (!['select', 'bot', 'room', 'remote', 'uber', 'uber-bot'].includes(action) ||
        typeof value !== 'string' || utf8Bytes(value) > 1024 || !this.paired.has(hub)) return false;
    if (presentedNav !== null) {
      const s = this.states.get(hub), link = this.client.links.get(hub);
      // Post-prime continuation is authorized by the current validated host
      // presentation, never a consumed native callback or cached shown view.
      if (typeof presentedNav !== 'string' || presentedNav !== hub + '|' + s?.session.sel ||
          !s?.ready || !s.greeted || !s.shown || !same(s.target, s.shownTarget) ||
          !link || link.revision !== s.presentedRevision || !same(link.target, s.target)) return false;
    }
    return this.request(hub, action, value, false, undefined, null, true);
  }

  sidebarPage(value) {
    if (!this.nativeSidebar || typeof value !== 'string') return false;
    const separator = value.indexOf('|'), hub = value.slice(0, separator);
    const s = this.states.get(hub), link = this.client.links.get(hub);
    if (separator < 1 || !this.paired.has(hub) || !s?.ready || !s.greeted || s.mode !== 'view-v1' ||
        !s.shown || !same(s.target, s.shownTarget) || s.active || s.queue.length || !link ||
        link.revision !== s.presentedRevision || s.overlay?.j?.nativeSidebar !== 1) return false;
    const screen = s.overlay.j, pages = screen.sidebarPages.flatMap(d => [d.projects, d.active, d.legacy]);
    for (const project of screen.projects) for (const list of sidebarLists) pages.push(project[list + 'Page']);
    if (!pages.some(page => page && [page.previous, page.next].some(control => control?.value === value))) return false;
    // Only an actually offered current control may issue a read. A different
    // hub's sidebar page must not replace foreground selection or held intent.
    return this.request(hub, 'native-sidebar-page', value, true);
  }

  request(hub, action, value, keepFocus = false, viewerIntent = undefined, capture = null, admission = false) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1') return false;
    // For admission, validate without changing focus or cancelling held work.
    // Legacy request retains its original ordering of those side effects.
    let intent = admission ? this.api.intent(hub, copy(s.session), copy(s.target), action, value) : null;
    if (admission && (!intent || intent.kind !== (action === 'select' ? 'select' : 'action'))) return false;
    if (admission && intent.kind === 'action') {
      if (!s.greeted) return false;
      if (!s.ready && !(s.shown && same(s.target, s.shownTarget) && ['bot', 'uber', 'uber-bot'].includes(action))) return false;
      if (s.ready && s.queue.length >= 7) return false;
    }
    // An explicit new owner or a hub switch supersedes any held tap, even
    // one belonging to a different hub whose refresh is still in flight.
    if (!keepFocus && (this.current !== hub || ['select', 'bot', 'uber', 'uber-bot', 'room', 'remote'].includes(action)))
      for (const state of this.states.values()) { state.deferredNavigation = null; state.attachmentEnds = []; }
    if (!keepFocus) this.current = hub;
    if (['select', 'bot', 'uber', 'uber-bot', 'room', 'remote'].includes(action) && this.projectSearch) this.projectSearchEnd(this.projectSearch.scope);
    if (!admission) intent = this.api.intent(hub, copy(s.session), copy(s.target), action, value);
    if (!intent || intent.kind === 'blocked') return true;
    if (action === 'proj-find-q' && intent.kind === 'action' && typeof value === 'string' && utf8Bytes(value) <= 4096) {
      s.projectQuery = { value, target: copy(s.target), sel: s.session.sel, epoch: s.epoch };
      this.pumpProjectQuery(s);
      return true;
    }
    if (intent.kind === 'draft') {
      s.revision++;
      s.session = copy(intent.session);
      s.drafts.set(s.session.sel, { revision: s.revision, text: value });
      this.enqueue(hub, { type: 'keep', thread: hub + '|' + s.session.sel, text: value });
      return true;
    }
    if (intent.kind === 'find') {
      if (intent.search === null && s.findCloseOnce) s.findCloseOnce = false;
      else s.findRevision++;
      if (s.shown && s.overlay) {
        const view = this.api.find({ shown: s.shown, overlay: s.overlay }, intent.search);
        s.shown = view.shown; s.overlay = view.overlay;
      }
      return true;
    }
    if (intent.kind === 'select') {
      s.deferredNavigation = null; s.attachmentEnds = [];
      const unchanged = s.session.sel === intent.session.sel;
      // Bootstrap preselect of the restored owner is no-effect and preserves
      // the compact route/open choices. A real selection after greeting opens.
      if (!s.greeted && unchanged) return true;
      s.revision++; s.target = copy(intent.target); s.session = copy(intent.session);
      s.ready = false; s.window = null; s.messages.clear(); s.queue = [];
      if (s.greeted) {
        const selected = this.select(s, s.target, s.session);
        if (admission && !selected) return false;
      }
      return true;
    }
    if (s.greeted && intent.kind === 'cancel') {
      const pending = this.pending.get(intent.args.requestId);
      if (pending && pending.hub === hub && !pending.operation) this.issue(s, this.client.request(hub, 'view.cancel', intent.args));
      return true;
    }
    if (!s.greeted || !s.ready) {
      // Only an already shown owner may retain the latest read-navigation tap
      // while its current view refreshes. Never carry it into a new selection.
      const retain = s.greeted && s.shown && same(s.target, s.shownTarget) && intent.kind === 'action' &&
          ['view', 'bot-tab', 'bot', 'uber', 'uber-bot'].includes(action) && typeof value === 'string' && utf8Bytes(value) <= 1024;
      if (retain)
        s.deferredNavigation = { action, value, viewerIntent, target: copy(s.target), sel: s.session.sel, epoch: s.epoch };
      return admission ? !!retain : true;
    }
    // One page request at a time, each taken from the page shown. 'later'
    // retraces the trail back down to the newest: without it a phone that
    // paged up could never page back down (the list ended on the old page)
    if (intent.kind === 'page') {
      if (!s.paging && s.window?.hasMore && s.window.older) this.pageTo(s, s.window.older, [...s.back, s.cursor]);
      return true;
    }
    if (intent.kind === 'later') {
      if (!s.paging && s.back.length) this.pageTo(s, s.back[s.back.length - 1], s.back.slice(0, -1));
      return true;
    }
    if (intent.kind === 'search') {
      if (typeof intent.args.query !== 'string' || utf8Bytes(intent.args.query) > 256 ||
          typeof intent.args.cursor !== 'string' || utf8Bytes(intent.args.cursor) > 1024) return true;
      s.findRevision++;
      s.findCloseOnce = action === 'thin-search-more';
      this.issue(s, this.client.request(hub, 'view.search', intent.args), { findRevision: s.findRevision }); return true;
    }
    if (intent.kind === 'message') {
      const part = s.window?.partial?.find(p => p.id === intent.args.entryId);
      const expected = s.messages.next(intent.args.entryId);
      if (!part || (expected !== undefined && expected !== intent.args.cursor) ||
          (expected === undefined && intent.args.cursor !== part.cursor && intent.args.cursor !== '')) return true;
      this.issue(s, this.client.request(hub, 'view.message', { ...intent.args, cursor: expected === undefined ? '' : expected })); return true;
    }
    if (intent.kind !== 'action' || s.queue.length >= 7) return !admission;
    const thread = s.session.sel;
    if (['send', 'send-alt'].includes(action)) {
      if (s.paging) s.overtaken.add(s.paging.id);
      s.cursor = ''; s.back = []; s.paging = null;
      const draft = s.drafts.get(thread)?.revision || 0;
      const active = this.pending.get(s.active)?.operation;
      if ([active, ...s.queue].some(op => op && ['send', 'send-alt'].includes(op.action) && op.input.sel === thread && op.revision >= draft)) return true;
    }
    // Keep the latest intended route separately from display authority.
    if (intent.session) { s.revision++; s.session = copy(intent.session); }
    const input = copy(s.session);
    if (capture) input.drafts = { ...input.drafts, [thread]: capture.text };
    const operation = { action, value, viewerIntent, input, revision: capture ? capture.revision : s.revision };
    s.queue.push(operation);
    this.pump(s);
    return admission ? s.queue.includes(operation) || this.pending.get(s.active)?.operation === operation : true;
  }

  attachmentEnd(hub, thread, value) {
    const s = this.states.get(hub);
    if (!s || this.current !== hub || !this.paired.has(hub) || s.mode !== 'view-v1' || s.session.sel !== thread ||
        !s.greeted || !s.shown || !same(s.target, s.shownTarget) || typeof value !== 'string' || utf8Bytes(value) > 65536) return false;
    if (s.attachmentEnds.some(x => x.value === value) || s.queue.some(x => x.action === 'attach-end' && x.value === value) ||
        (this.pending.get(s.active)?.operation?.action === 'attach-end' && this.pending.get(s.active)?.operation?.value === value)) return true;
    if (s.attachmentEnds.length >= 10) return false;
    s.attachmentEnds.push({ epoch: s.epoch, sel: thread, target: copy(s.target), value });
    this.pumpAttachmentEnd(s);
    return true;
  }

  pumpAttachmentEnd(s) {
    if (s.active || !s.ready || this.current !== s.hub || s.queue.length >= 7) return;
    const completion = s.attachmentEnds.shift();
    if (!completion) return;
    if (completion.epoch === s.epoch && completion.sel === s.session.sel && same(completion.target, s.target))
      this.request(s.hub, 'attach-end', completion.value);
  }

  draftAt(hub, thread, text) {
    const s = this.states.get(hub);
    if (!s || !this.paired.has(hub) || s.mode !== 'view-v1') return false;
    s.revision++;
    s.drafts.set(thread, { revision: s.revision, text });
    s.session.drafts = { ...s.session.drafts, [thread]: text };
    this.enqueue(hub, { type: 'keep', thread: hub + '|' + thread, text });
    return true;
  }

  sendAt(hub, thread, action, text, currentDraft) {
    const s = this.states.get(hub);
    if (!s || this.current !== hub || !this.paired.has(hub) || s.mode !== 'view-v1' || s.session.sel !== thread ||
        !s.greeted || !s.ready || s.active || s.queue.length >= 7) return false;
    const revision = currentDraft ? s.drafts.get(thread)?.revision ?? 0 : 0;
    return this.request(hub, action, '', false, undefined, action === 'interrupt' ? null : { text, revision });
  }

  viewerAction(hub, nav, serial, value) {
    const s = this.states.get(hub);
    if (!s || !this.paired.has(hub) || s.mode !== 'view-v1' || typeof nav !== 'string' ||
        nav !== hub + '|' + s.session.sel || !Number.isSafeInteger(serial) || serial < 1 || typeof value !== 'string') return false;
    return this.request(hub, 'view', value, false, serial);
  }

  pump(s) {
    if (!s.active && s.ready && s.projectQuery) { this.pumpProjectQuery(s); return; }
    if (s.active || !s.ready || !s.queue.length) return;
    const operation = s.queue.shift();
    const session = this.api.prepare(s.session, operation.input, operation.action);
    const result = this.client.request(s.hub, 'view.action', { session: this.requestSession(session, 'view.action', s.target), action: operation.action, value: operation.value });
    s.active = result.id || null;
    this.issue(s, result, { operation, revision: operation.revision });
  }

  pumpProjectQuery(s) {
    const pending = s.projectQuery;
    if (!pending || !s.greeted || !s.ready || s.active) return;
    s.projectQuery = null;
    if (pending.epoch !== s.epoch || pending.sel !== s.session.sel || !same(pending.target, s.target)) { this.pump(s); return; }
    s.queue = s.queue.filter(op => op.projectScope === undefined);
    s.queue.unshift({ action: 'proj-find-q', value: pending.value, projectScope: pending.scope, projectEdit: pending.edit,
      input: copy(s.session), revision: s.revision });
    this.pump(s);
  }

  projectSearchEnd(scope) {
    if (!Number.isSafeInteger(scope) || scope < 1) return;
    if (!this.projectSearch || scope > this.projectSearch.scope)
      this.projectSearch = { scope, edit: -1, text: '', ended: true, owners: new Map(), go: null };
    else if (scope === this.projectSearch.scope) { this.projectSearch.ended = true; this.projectSearch.go = null; }
    for (const s of this.states.values()) {
      if (s.projectQuery?.scope <= scope) s.projectQuery = null;
      s.queue = s.queue.filter(op => op.projectScope === undefined || op.projectScope > scope);
    }
  }

  projectSearchQuery(scope, edit, text) {
    if (!Number.isSafeInteger(scope) || scope < 1 || !Number.isSafeInteger(edit) || edit < 0 ||
        typeof text !== 'string' || utf8Bytes(text) > 4096) return false;
    let q = this.projectSearch;
    if (q && (scope < q.scope || (scope === q.scope && (q.ended || edit < q.edit || (edit === q.edit && text !== q.text))))) return false;
    if (q && scope === q.scope && edit === q.edit) return true;
    if (!q || scope > q.scope) {
      if (q) this.projectSearchEnd(q.scope);
      q = { scope, edit, text, ended: false, owners: new Map(), go: null };
      for (const hub of this.paired) {
        const s = this.states.get(hub);
        if (s?.mode === 'view-v1') q.owners.set(hub, { epoch: s.epoch, sel: s.session.sel, target: copy(s.target) });
      }
      this.projectSearch = q;
    }
    q.edit = edit; q.text = text; q.go = null;
    for (const hub of this.paired) {
      const s = this.states.get(hub);
      if (s?.mode === 'view-v1' && !q.owners.has(hub))
        q.owners.set(hub, { epoch: s.epoch, sel: s.session.sel, target: copy(s.target) });
    }
    for (const [hub, owner] of q.owners) {
      const s = this.states.get(hub);
      if (!s || s.epoch !== owner.epoch || s.session.sel !== owner.sel || !same(s.target, owner.target)) { this.projectSearchEnd(scope); return false; }
      s.projectApplied = null;
      s.projectQuery = { scope, edit, value: text, ...copy(owner) };
      s.queue = s.queue.filter(op => op.projectScope === undefined);
      this.pumpProjectQuery(s);
    }
    return true;
  }

  projectSearchGo(scope, edit, text, project) {
    if (!this.projectSearchQuery(scope, edit, text)) return false;
    const q = this.projectSearch;
    if (typeof project !== 'string' || (project !== '' && ![...q.owners.keys()].some(hub => project.startsWith(hub + '|')))) return false;
    q.go = { project };
    this.pumpProjectGo();
    return true;
  }

  pumpProjectGo() {
    const q = this.projectSearch;
    if (!q || q.ended || !q.go || !q.owners.size) return;
    for (const [hub, owner] of q.owners) {
      const s = this.states.get(hub), accepted = s?.projectApplied;
      if (!s || s.epoch !== owner.epoch || s.session.sel !== owner.sel || !same(s.target, owner.target)) { this.projectSearchEnd(q.scope); return; }
      if (!s.ready || s.active || s.projectQuery || s.queue.length || accepted?.scope !== q.scope || accepted.edit !== q.edit ||
          accepted.epoch !== owner.epoch || !same(accepted.target, owner.target) || s.shown?.search?.query !== q.text) return;
    }
    const first = this.api.firstProject ? this.api.firstProject() : [...q.owners.keys()].flatMap(hub =>
      (this.states.get(hub).shown.projects || []).map(p => p.id.startsWith(hub + '|') ? p.id : hub + '|' + p.id))[0];
    const project = q.go.project || first;
    const hub = typeof project === 'string' && [...q.owners.keys()].find(key => project.startsWith(key + '|'));
    if (!hub || project !== first) { q.go = null; return; }
    const s = this.states.get(hub), raw = project.slice(hub.length + 1);
    // Never pass an earlier-prefix or missing project to Find.go: its fallback
    // can create a thread. Only the exact freshly filtered project is eligible.
    if (!s.shown.projects?.some(p => p.id === project || p.id === raw)) { q.go = null; return; }
    q.ended = true;
    q.go = null;
    for (const other of q.owners.keys()) if (other !== hub) this.request(other, 'proj-find', 'off', true);
    this.current = hub;
    this.api.focus?.(hub);
    this.request(hub, 'proj-go', raw);
  }

  validate(result, input) {
    if ('entryId' in input) {
      if (!validateThinResult(result, input)) return false;
      const thread = this.states.get(input.hub)?.shown?.thread, message = result.message;
      return thread?.id === input.hub + '|' + message.thread && thread.entries.some(e => e.id === message.id && e.partial?.version === message.version);
    }
    if ('query' in input || 'requestId' in input) return validateThinResult(result, input);
    return validateNativeSidebarAction307265(result, input, this.sidebarCaptures.get(input)) && validateNativeResult307265(result, input) &&
      validateNativeSidebar307265(result, input, this.sidebarOffers.has(input)) &&
      (!this.api.validate || this.api.validate(result, input) === true);
  }

  receive(hub, envelope, epoch) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1' || s.epoch !== String(epoch)) return false;
    if (this.inert(hub, envelope, epoch)) return true;
    if (envelope?.t === 'view') return this.greeting(hub, envelope, epoch);
    if (envelope?.t === 'view.changed') {
      if (s.greeted && this.client.links.get(hub)?.viewId) this.issue(s, this.client.changed(hub, envelope, this.requestSession(s.session, 'view.update', s.target), s.anchor,
        !!s.active || !!s.deferredNavigation || s.queue.length > 0 || !!s.followup));
      return true;
    }
    if (envelope?.t !== 'reply') {
      this.fail(hub, 'Unexpected thin frame; compatibility must be chosen explicitly'); return true;
    }
    const metadata = this.pending.get(envelope.id);
    const result = this.client.reply(hub, envelope);
    if (!result) return this.ack(hub, envelope);
    this.pending.delete(envelope.id);
    if (s.active === envelope.id) s.active = null;
    const paged = s.paging?.id === envelope.id ? s.paging : null;
    if (paged) s.paging = null;
    if (s.overtaken.delete(envelope.id)) return true;
    if (paged && result.failed) { s.cursor = paged.undo.cursor; s.back = paged.undo.back; }
    if (!metadata || metadata.epoch !== s.epoch) return true;
    if (result.failed) {
      if (presentation(result.method) && this.isCurrent(metadata)) this.fail(hub, result.failed);
      else this.pump(s);
      return true;
    }
    const r = result.result;
    if (!presentation(result.method)) {
      if (result.current) {
        if (result.method === 'view.search' && metadata.findRevision === s.findRevision) {
          const view = this.api.search(hub, r.search, { shown: s.shown, overlay: s.overlay });
          if (view) { s.shown = view.shown; s.overlay = view.overlay; }
        }
        if (result.method === 'view.message') {
          try {
            const message = s.messages.accept(r.message, metadata.input.cursor);
            const view = this.api.message({ shown: s.shown, overlay: s.overlay }, message);
            s.shown = view.shown; s.overlay = view.overlay;
          } catch { this.api.warning?.(hub, 'Message continuation is stale'); }
        }
      }
      if (result.refreshDue && s.ready && !s.active && !s.deferredNavigation && !s.queue.length) this.deferRefresh(s);
      return true;
    }
    let response = copy(r.session);
    if (metadata.operation?.action === 'native-sidebar-page') {
      // Paging cannot clear or replace local status, including a legitimate
      // update that arrived after dispatch. Preserve absence as well as values.
      for (const key of ['error', 'note']) {
        if (Object.hasOwn(s.session, key)) response[key] = s.session[key];
        else delete response[key];
      }
    }
    for (const ack of metadata.acks) response = this.api.ack(response, ack);
    if (!counter(response.rid)) { this.fail(hub, 'Invalid ordinary counter'); return true; }
    s.session.rid = Math.max(s.session.rid, response.rid);
    if (result.current) {
      const drafts = { ...response.drafts, ...s.session.drafts };
      if (result.method === 'view.action') for (const [key, text] of Object.entries(response.drafts))
        if (key === response.sel || key.startsWith('@')) Object.defineProperty(drafts, key, { value: text, enumerable: true, writable: true, configurable: true });
      s.session = { ...s.session, ...response, rid: s.session.rid, drafts,
        info: { ...s.session.info, ...response.info }, outbox: { ...s.session.outbox, ...response.outbox } };
      // New navigation is fenced by ThinViews. Preserve local drafts while
      // accepting the current server's actual route and policy fallback.
      for (const [thread, draft] of s.drafts)
        s.session.drafts = { ...s.session.drafts, [thread]: draft.text };
    }
    if (this.ordinary.size + result.cmds.filter(c => c.command?.type === 'send').length > 128) { this.fail(hub, 'Ordinary reply backlog limit'); return true; }
    // ThinViews validated the entire batch. Original actions retain their
    // lifetime after selection moves; only their captured commands settle.
    let view = null;
    if (result.current && result.remoteReady) {
      try { view = this.api.materialize(r, s.session); }
      catch { this.fail(hub, 'Invalid native materialization'); return true; }
    }
    const commands = [];
    for (const item of result.cmds) {
      const c = item.command;
      if (item.type !== 'command') { this.fail(hub, 'Invalid command descriptor'); return true; }
      const thread = c.type === 'keep' ? rawThread(hub, c.thread) : null;
      if (c.type === 'keep' && (s.drafts.get(thread)?.revision || 0) > metadata.revision) continue;
      if (c.type === 'send') {
        const rpc = JSON.parse(c.text);
        this.ordinary.set(ordinaryKey(hub, rpc.id), { hub, epoch: s.epoch, revision: metadata.revision, thread: item.input.session?.sel || '', session: copy(response) });
        this.outboxKeep(hub, rpc);
      }
      commands.push(this.api.command(hub, c, item.input, result.current));
      if (c.type === 'keep') {
        s.session.drafts = { ...s.session.drafts, [thread]: c.text };
        s.drafts.set(thread, { revision: metadata.revision, text: c.text });
      }
    }
    this.enqueue(hub, ...commands);
    if (result.current && r.nextTarget) { this.select(s, r.nextTarget, s.session); return true; }
    if (result.current && result.remoteReady) {
      for (const old of s.window?.partial || []) if (!result.window.partial.some(p => p.id === old.id && p.version === old.version)) s.messages.reset(old.id);
      s.presentedRevision = r.requestRevision;
      s.ready = true; s.window = copy(result.window); s.anchor = copy(result.window.anchor);
      for (const entry of view.shown.thread?.entries || []) {
        const old = s.shown?.thread?.entries?.find(e => e.id === entry.id);
        if (old?.partial?.version !== entry.partial?.version) s.messages.reset(entry.id);
      }
      if (view.shown?.thread) view.shown.thread.later = s.back.length;
      s.shown = view.shown; s.overlay = view.overlay; s.shownTarget = copy(s.target);
      if (metadata.operation?.projectScope !== undefined)
        s.projectApplied = { scope: metadata.operation.projectScope, edit: metadata.operation.projectEdit, epoch: s.epoch, target: copy(s.target) };
      if (!s.flushed) { s.flushed = true; this.outboxFlush(hub); }
      this.sync.push({ hub, phase: 'presented', detail: 'view-v1', mode: 'view-v1', viewId: r.viewId,
        requestRevision: r.requestRevision, viewRevision: r.viewRevision, remoteReady: true,
        ...(metadata.operation?.viewerIntent === undefined ? {} : { viewerIntent: metadata.operation.viewerIntent }) });
      for (const args of s.registrations.values()) this.sendRegistration(s, args);
      s.registrations.clear();
    } else if (result.current) this.fail(hub, 'Current view is not ready');
    this.pumpProjectQuery(s);
    if (s.ready && s.deferredNavigation) {
      const pending = s.deferredNavigation; s.deferredNavigation = null;
      if (pending.epoch === s.epoch && pending.sel === s.session.sel && same(pending.target, s.target))
        this.request(hub, pending.action, pending.value, false, pending.viewerIntent);
    }
    this.pumpAttachmentEnd(s);
    this.pump(s);
    this.pumpProjectGo();
    // Person work gets the next request; ThinViews retains invalidation until
    // a later accepted picture covers it, or a new selection retires it.
    if (result.refreshDue && s.ready) this.deferRefresh(s);
    return true;
  }

  ack(hub, envelope) {
    this.outboxReply(hub, envelope);
    const key = ordinaryKey(hub, envelope.id), original = this.ordinary.get(key), s = this.states.get(hub);
    if (!original || original.hub !== hub || original.epoch !== s?.epoch) return true;
    this.ordinary.delete(key);
    const acknowledged = this.api.ack(original.session, envelope);
    s.session.rid = Math.max(s.session.rid, acknowledged.rid);
    if (original.thread && (s.drafts.get(original.thread)?.revision || 0) <= original.revision) {
      const text = acknowledged.drafts[original.thread] || '';
      if (text !== s.session.drafts[original.thread]) this.enqueue(hub, { type: 'keep', thread: hub + '|' + original.thread, text });
      s.session.drafts = { ...s.session.drafts, [original.thread]: text };
      s.drafts.set(original.thread, { revision: original.revision, text });
    }
    // As the web's thin-host: the answer applies to the session as it is now
    // (a new thread or fork's answer selects it and ends the wait). Taking
    // only its drafts left wantNew set: a "New thread" page that waited for
    // good, and every view after it asked for that wait again.
    const before = s.session, next = this.api.ack(s.session, envelope);
    // the client's own state ('@' keys: the wait is "@want.new") from the
    // answer, the person's thread drafts as they are now
    const drafts = Object.fromEntries([...Object.entries(before.drafts || {}).filter(([k]) => !k.startsWith('@')),
      ...Object.entries(next.drafts || {}).filter(([k]) => k.startsWith('@'))]);
    s.session = { ...next, rid: Math.max(before.rid, next.rid || 0), drafts };
    if (next.sel && next.sel !== before.sel) this.select(s, this.api.target(s.session), s.session);
    else if (next.wantNew !== before.wantNew) this.update(s);
    for (const call of this.ordinary.values()) if (call.hub === hub) call.session = this.api.ack(call.session, envelope);
    for (const call of this.pending.values()) if (call.hub === hub) {
      if (call.acks.length >= 128) { this.fail(hub, 'Ordinary reply backlog limit'); break; }
      call.acks.push(envelope);
    }
    return true;
  }

  isCurrent(metadata) {
    const l = this.client.links.get(metadata.hub);
    return !!l && l.revision === metadata.input.requestRevision && same(l.target, metadata.input.target);
  }

  update(s) {
    const l = this.client.links.get(s.hub);
    if (s.greeted && l?.viewId) this.issue(s, this.client.request(s.hub, 'view.update', { session: this.requestSession(s.session, 'view.update', s.target), anchor: s.anchor, viewRevision: l.viewRevision }));
  }

  // Do not retire an accepted picture until the native worker has handed a
  // screen out. A quiet or discarded screen waits for the next real flush.
  deferRefresh(s) {
    if (!this.publicationAck) { this.update(s); return; }
    const link = this.client.links.get(s.hub);
    if (s.followup && s.followup.epoch === s.epoch && same(s.followup.target, s.target) && s.followup.revision === link?.revision) return;
    if (link && s.ready) s.followup = { epoch: s.epoch, target: copy(s.target), revision: link.revision, at: null };
  }

  published() {
    for (const s of this.states.values()) {
      const f = s.followup, link = this.client.links.get(s.hub);
      if (f && f.at === null && s.ready && s.epoch === f.epoch && same(s.target, f.target) && link?.revision === f.revision)
        f.at = this.api.now();
    }
    return this.deadline();
  }

  register(hub, args) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1') return false;
    if (!s.ready) {
      if (s.registrations.size < 16) s.registrations.set(JSON.stringify([args[0], args[2], args[3]]), args);
      return true;
    }
    this.sendRegistration(s, args); return true;
  }

  sendRegistration(s, args) {
    if (!counter(s.session.rid) || s.session.rid >= 2 ** 47 - 1) return;
    const prepared = this.api.registration(s.hub, copy(s.session), args);
    if (!prepared || !counter(prepared.session?.rid) || !Array.isArray(prepared.cmds) || prepared.cmds.length !== 1) return;
    const command = prepared.cmds[0];
    let rpc; try { rpc = JSON.parse(command.text); } catch { return; }
    if (command.type !== 'send' || rpc.m !== 'device.register' || !counter(rpc.id) || rpc.id <= 0 ||
        this.ordinary.size >= 128 || this.ordinary.has(ordinaryKey(s.hub, rpc.id))) return;
    s.session.rid = Math.max(s.session.rid, prepared.session.rid);
    this.ordinary.set(ordinaryKey(s.hub, rpc.id), { hub: s.hub, epoch: s.epoch, revision: s.revision, thread: '', session: { ...s.session, outbox: {} } });
    this.enqueue(s.hub, this.api.command(s.hub, command, {}, false));
  }

  expire(followups = false) {
    for (const call of this.client.expire()) {
      const metadata = this.pending.get(call.id), s = this.states.get(call.hub);
      this.pending.delete(call.id);
      if (s?.active === call.id) s.active = null;
      if (metadata && presentation(call.method) && this.isCurrent(metadata)) this.fail(call.hub, 'View request timed out; not resent');
      else if (s) this.pump(s);
    }
    if (followups) for (const s of this.states.values()) {
      const f = s.followup, link = this.client.links.get(s.hub);
      if (!f || f.at === null || f.at > this.api.now()) continue;
      s.followup = null;
      if (s.ready && !s.active && !s.deferredNavigation && !s.queue.length && s.epoch === f.epoch &&
          same(s.target, f.target) && link?.revision === f.revision && link.invalidated) this.update(s);
    }
  }

  deadline() {
    const calls = [...this.client.calls.values()];
    const deadlines = calls.map(c => c.began + 5000);
    for (const s of this.states.values()) if (s.followup?.at != null) deadlines.push(s.followup.at);
    return deadlines.length ? Math.min(...deadlines) : null;
  }

  savePresentation() {
    const hubs = [];
    for (const s of this.states.values()) if (this.paired.has(s.hub)) {
      const info = {};
      for (const [key, value] of Object.entries(s.session.info || {})) if (key.startsWith('fold:') || key.startsWith('subs:'))
        Object.defineProperty(info, key, { value, enumerable: true });
      const selected = { sel: s.session.sel, open: s.session.open, adding: s.session.adding, panel: s.session.panel, info };
      if (s.session.route !== undefined) selected.route = s.session.route;
      const session = compactSession(selected, s.target);
      if (!session) return '';
      hubs.push({ hub: s.hub, target: s.target, session, anchor: s.anchor });
    }
    return writePresentation({ version: 1, current: hubs.some(s => s.hub === this.current) ? this.current : '', hubs }) || '';
  }

  loadPresentation(text) {
    const saved = readPresentation(text);
    if (!saved || [...this.states.values()].some(s => s.mode)) return false;
    for (const h of saved.hubs) {
      const s = this.state(h.hub);
      if (s.mode || s.greeted) continue; // Never rewind a running connection.
      s.target = copy(h.target); s.anchor = copy(h.anchor);
      s.session = { ...s.session, ...copy(h.session) };
    }
    if (!this.paired.size || this.paired.has(saved.current)) this.current = saved.current;
    return true;
  }

  views() {
    const current = this.states.get(this.current);
    if (!current?.ready || !current.overlay) return null;
    return [...this.states.values()].filter(s => this.paired.has(s.hub) && s.overlay).map(s => s.overlay);
  }

  drain() {
    return { cmds: this.commands.splice(0), sync: this.sync.splice(0), deadline: this.deadline() };
  }
}
