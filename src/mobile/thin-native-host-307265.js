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
// a typed field's name ("bfield" is "<name>\u001f<text>"), null for none
const fieldName = value => { const i = typeof value === 'string' ? value.indexOf('\u001f') : -1; return i > 0 ? value.slice(0, i) : null; };
const rawThread = (hub, qualified) => qualified.startsWith(hub + '|') ? qualified.slice(hub.length + 1) : null;
const plainObject = value => object(value) && [Object.prototype, null].includes(Object.getPrototypeOf(value));
// Ordinary transport side channels are not presentation authority. Never
// retain, copy or log their values: pair/pair.qr can contain credentials.
export function inertAncillary307265(envelope) {
  if (!plainObject(envelope)) return false;
  const keys = Reflect.ownKeys(envelope);
  if (envelope.t === 'hb') return keys.length === 1 && keys[0] === 't';
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
      send: (hub, request) => ({ type: 'request', hub, request: { ...request,
        p: { ...request.p, ...(request.p.session ? { session: this.requestSession(request.p.session, request.m, request.p.target) } : {}), nativeRenderer: api.renderer() } } }),
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
        mode: '', epoch: '', greeted: false, ready: false, shown: null, window: null,
        // history pages (as the web's thin-host): the cursor shown ('' the
        // newest), the cursors that led here, the one page on its way, and
        // pages a send overtook (their replies are dropped)
        cursor: '', back: [], paging: null, overtaken: new Set(),
        revision: 0, drafts, queue: [], active: null, messages: new MessageParts(), findRevision: 0, findCloseOnce: false, registrations: new Map() };
      this.states.set(hub, value);
    }
    return value;
  }

  hubs(keys) {
    this.paired = new Set(keys);
    for (const key of this.states.keys()) if (!this.paired.has(key)) {
      this.disconnect(key); this.states.delete(key);
    }
    if (!this.paired.has(this.current)) this.current = keys[0] || '';
  }

  reset() {
    for (const key of this.states.keys()) this.client.disconnect(key);
    this.states.clear(); this.paired.clear(); this.current = '';
    this.pending.clear(); this.ordinary.clear(); this.commands = []; this.sync = [];
    this.outbox.clear();
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
          this.commands.push({ type: 'keep', thread: hub + '|' + thread, text: String(o.rpc.p.text || '') });
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
      this.commands.push(this.api.command(hub, { type: 'send', text: JSON.stringify(o.rpc) }, {}, false));
    }
  }


  connection(hub, epoch, mode) {
    if (!this.paired.has(hub)) return;
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
    // the device's own choices the hub draws by (its language; the thread
    // list's view, Backplane.local("view"): left out, Active never applied)
    for (const key of ['@lang', '@lang.os', '@lang.tag', '@lang.preview', '@local.view'])
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
    const s = this.states.get(hub);
    this.client.disconnect(hub);
    for (const [id, call] of this.pending) if (call.hub === hub) this.pending.delete(id);
    for (const [id, call] of this.ordinary) if (call.hub === hub) this.ordinary.delete(id);
    if (s) {
      s.ready = false; s.greeted = false; s.active = null; s.queue = []; s.messages.clear();
      s.registrations.clear();
    }
  }

  fail(hub, reason) {
    const s = this.states.get(hub);
    if (s) { s.ready = false; s.queue = []; }
    this.sync.push({ hub, phase: 'failed', mode: 'view-v1', detail: String(reason), remoteReady: false });
  }

  issue(s, result, extra = {}) {
    if (!result || result.failed) { this.fail(s.hub, result?.failed || 'View unavailable'); return false; }
    if (!result.id) return true; // Coalesced update: no transition or request.
    const call = this.client.calls.get(result.id);
    if (!call) { this.fail(s.hub, 'Missing shared request'); return false; }
    this.pending.set(result.id, { hub: s.hub, epoch: s.epoch, revision: s.revision, input: copy(call.input), acks: [], ...extra });
    if (presentation(call.method)) {
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
    this.commands.push(...descriptors);
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
    s.target = copy(target); s.session = copy(session); s.window = null;
    s.cursor = ''; s.back = []; s.paging = null;
    s.queue = []; s.messages.clear();
    return this.issue(s, this.client.select(s.hub, s.target, this.requestSession(s.session, 'view.open', s.target), this.api.cid()));
  }

  request(hub, action, value) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1') return false;
    this.current = hub;
    if (action === 'bfield' && fieldName(value) !== null) {
      if (!s.typed) s.typed = new Map();
      s.typed.set(fieldName(value), value.slice(fieldName(value).length + 1));
    }
    const intent = this.api.intent(hub, copy(s.session), copy(s.target), action, value);
    if (!intent || intent.kind === 'blocked') return true;
    if (intent.kind === 'draft') {
      s.revision++;
      s.session = copy(intent.session);
      s.drafts.set(s.session.sel, { revision: s.revision, text: value });
      this.commands.push({ type: 'keep', thread: hub + '|' + s.session.sel, text: value });
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
      const unchanged = s.session.sel === intent.session.sel;
      // Bootstrap preselect of the restored owner is no-effect and preserves
      // the compact route/open choices. A real selection after greeting opens.
      if (!s.greeted && unchanged) return true;
      s.revision++; s.target = copy(intent.target); s.session = copy(intent.session);
      s.ready = false; s.window = null; s.messages.clear(); s.queue = [];
      if (s.greeted) this.select(s, s.target, s.session);
      return true;
    }
    if (s.greeted && intent.kind === 'cancel') {
      const pending = this.pending.get(intent.args.requestId);
      if (pending && pending.hub === hub && !pending.operation) this.issue(s, this.client.request(hub, 'view.cancel', intent.args));
      return true;
    }
    // An action asked for while the page's own last action or update is on
    // its way waits its turn (pump sends it once that is answered), and a
    // search goes at once. Both were dropped: a field typed faster than the
    // hub answered kept only its first letter (an API key named "E2E_KEY"
    // saved as "E").
    if (!s.greeted || (!s.ready && !((intent.kind === 'action' || intent.kind === 'search') && this.presenting(s)))) return true;
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
      // one search on its way at a time: a query typed meanwhile waits, the
      // newest only, and goes when that one is answered. A request per key
      // ran past the hub's eight in flight, which failed the whole view:
      // nothing tapped did anything after
      if (s.searching && this.pending.has(s.searching)) { s.searchNext = intent.args; return true; }
      this.search(s, intent.args); return true;
    }
    if (intent.kind === 'message') {
      const part = s.window?.partial?.find(p => p.id === intent.args.entryId);
      const expected = s.messages.next(intent.args.entryId);
      if (!part || (expected !== undefined && expected !== intent.args.cursor) ||
          (expected === undefined && intent.args.cursor !== part.cursor && intent.args.cursor !== '')) return true;
      this.issue(s, this.client.request(hub, 'view.message', { ...intent.args, cursor: expected === undefined ? '' : expected })); return true;
    }
    if (intent.kind !== 'action') return true;
    // a field's edits not sent yet are one, its newest text (as the web's
    // thin-host): a burst of keys never fills the queue
    const tail = s.queue[s.queue.length - 1];
    if (action === 'bfield' && tail?.action === 'bfield' && fieldName(tail.value) !== null && fieldName(tail.value) === fieldName(value)) {
      if (intent.session) { s.revision++; s.session = copy(intent.session); }
      tail.value = value; tail.input = copy(s.session); tail.revision = s.revision;
      return true;
    }
    if (s.queue.length >= 7) return true;
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
    s.queue.push({ action, value, input: copy(s.session), revision: s.revision });
    this.pump(s); return true;
  }

  // what was typed last in a field of hub's forms (its "bfield" text), sent
  // or still waiting (undefined: nothing typed); forget lets it go
  typed(hub, name) { return this.states.get(hub)?.typed?.get(name); }
  forget(hub, name) { this.states.get(hub)?.typed?.delete(name); }

  search(s, args) {
    s.searchNext = null;
    const result = this.client.request(s.hub, 'view.search', args);
    s.searching = result?.id || null;
    this.issue(s, result, { findRevision: s.findRevision });
  }

  // the search on its way answered (or gone): the query typed meanwhile goes
  searched(s, id) {
    if (!s || s.searching !== id) return;
    s.searching = null;
    if (s.searchNext && s.greeted) this.search(s, s.searchNext);
  }

  // the page on show has its own request on its way (an action, an update,
  // a page): not a view still opening (select() clears the window)
  presenting(s) {
    if (!s.window) return false;
    for (const [id, call] of this.pending)
      if (call.hub === s.hub && call.epoch === s.epoch && presentation(this.client.calls.get(id)?.method)) return true;
    return false;
  }

  pump(s) {
    if (s.active || !s.ready || !s.queue.length) return;
    const operation = s.queue.shift();
    const session = this.api.prepare(s.session, operation.input, operation.action);
    const result = this.client.request(s.hub, 'view.action', { session: this.requestSession(session, 'view.action', s.target), action: operation.action, value: operation.value });
    s.active = result.id || null;
    this.issue(s, result, { operation, revision: operation.revision });
  }

  validate(result, input) {
    if ('entryId' in input) {
      if (!validateThinResult(result, input)) return false;
      const thread = this.states.get(input.hub)?.shown?.thread, message = result.message;
      return thread?.id === input.hub + '|' + message.thread && thread.entries.some(e => e.id === message.id && e.partial?.version === message.version);
    }
    if ('query' in input || 'requestId' in input) return validateThinResult(result, input);
    return validateNativeResult307265(result, input) && (!this.api.validate || this.api.validate(result, input) === true);
  }

  receive(hub, envelope, epoch) {
    const s = this.states.get(hub);
    if (!s || s.mode !== 'view-v1' || s.epoch !== String(epoch)) return false;
    if (this.inert(hub, envelope, epoch)) return true;
    if (envelope?.t === 'view') return this.greeting(hub, envelope, epoch);
    if (envelope?.t === 'view.changed') {
      if (s.greeted && this.client.links.get(hub)?.viewId) this.issue(s, this.client.changed(hub, envelope, this.requestSession(s.session, 'view.update', s.target), s.anchor));
      return true;
    }
    if (envelope?.t !== 'reply') {
      this.fail(hub, 'Unexpected thin frame; compatibility must be chosen explicitly'); return true;
    }
    const metadata = this.pending.get(envelope.id);
    const result = this.client.reply(hub, envelope);
    if (!result) return this.ack(hub, envelope);
    this.pending.delete(envelope.id);
    this.searched(s, envelope.id);
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
      if (result.refreshDue) this.update(s);
      return true;
    }
    let response = copy(r.session);
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
    this.commands.push(...commands);
    if (result.current && r.nextTarget) { this.select(s, r.nextTarget, s.session); return true; }
    if (result.current && result.remoteReady) {
      for (const old of s.window?.partial || []) if (!result.window.partial.some(p => p.id === old.id && p.version === old.version)) s.messages.reset(old.id);
      s.ready = true; s.window = copy(result.window); s.anchor = copy(result.window.anchor);
      for (const entry of view.shown.thread?.entries || []) {
        const old = s.shown?.thread?.entries?.find(e => e.id === entry.id);
        if (old?.partial?.version !== entry.partial?.version) s.messages.reset(entry.id);
      }
      if (view.shown?.thread) view.shown.thread.later = s.back.length;
      s.shown = view.shown; s.overlay = view.overlay;
      if (!s.flushed) { s.flushed = true; this.outboxFlush(hub); }
      this.sync.push({ hub, phase: 'presented', detail: 'view-v1', mode: 'view-v1', viewId: r.viewId,
        requestRevision: r.requestRevision, viewRevision: r.viewRevision, remoteReady: true });
      for (const args of s.registrations.values()) this.sendRegistration(s, args);
      s.registrations.clear();
    } else if (result.current) this.fail(hub, 'Current view is not ready');
    if (result.refreshDue) this.update(s);
    this.pump(s); return true;
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
      if (text !== s.session.drafts[original.thread]) this.commands.push({ type: 'keep', thread: hub + '|' + original.thread, text });
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
    this.commands.push(this.api.command(s.hub, command, {}, false));
  }

  expire() {
    for (const call of this.client.expire()) {
      const metadata = this.pending.get(call.id), s = this.states.get(call.hub);
      this.pending.delete(call.id);
      this.searched(s, call.id);
      if (s?.active === call.id) s.active = null;
      if (metadata && presentation(call.method) && this.isCurrent(metadata)) this.fail(call.hub, 'View request timed out; not resent');
      else if (s) this.pump(s);
    }
  }

  deadline() {
    const calls = [...this.client.calls.values()];
    return calls.length ? Math.min(...calls.map(c => c.began + 5000)) : null;
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
