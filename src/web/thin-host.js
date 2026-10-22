import { validatePresentation } from './startup.js';
import { validateThinResult, MessageParts } from './thin-results.js';
import { compactRoute, utf8Bytes, popupDraft, popupReadDraft, carriesDraft } from './thin.js';

const clone = value => typeof structuredClone === 'function' ? structuredClone(value) : JSON.parse(JSON.stringify(value));
const object = value => value && typeof value === 'object' && !Array.isArray(value);
function choices(value, accept) {
  const out = {};
  for (const [key, item] of Object.entries(object(value) ? value : {})) {
    if (Object.keys(out).length >= 128) break;
    if (utf8Bytes(key) <= 1024 && accept(key, item)) Object.defineProperty(out, key, { value: item, enumerable: true });
  }
  return out;
}
export function requestSession(session, action, target, popup = false) {
  const drafts = {}, local = object(session.drafts) ? session.drafts : {};
  for (const key of ['@lang', '@lang.os', '@lang.tag', '@lang.preview']) {
    if (typeof local[key] === 'string' && utf8Bytes(local[key]) <= 128) drafts[key] = local[key];
  }
  if (action && session.sel && !session.sel.startsWith('@') && Object.hasOwn(local, session.sel) && typeof local[session.sel] === 'string') {
    Object.defineProperty(drafts, session.sel, { value: local[session.sel], enumerable: true });
  } else if (popup && session.sel && !session.sel.startsWith('@') && typeof local[session.sel] === 'string') {
    // A read carries the draft only while the composer's popup may show (or
    // must shut): the hub renders the popup from it (client.bend's Refs.ready)
    Object.defineProperty(drafts, session.sel, { value: popupReadDraft(local[session.sel]), enumerable: true });
  }
  const route = compactRoute(session.route, target);
  return { sel: session.sel, rid: session.rid, drafts, outbox: {},
    open: choices(session.open, (_key, item) => typeof item === 'boolean'),
    info: choices(session.info, (key, item) => (key.startsWith('fold:') || key.startsWith('subs:')) && typeof item === 'string' && utf8Bytes(item) <= 1024),
    adding: session.adding === true, panel: session.panel === true, wantNew: session.wantNew === true,
    ...(route ? { route } : {}) };
}

// Bend's generic presentation pack intentionally omits the thin bot route.
// Local non-navigation edits keep the validated tab the person already chose;
// an actual selection change must not borrow that route.
export function preservePresentationRoute(before, after) {
  const route = compactRoute(before?.route);
  return route && after?.sel === before.sel ? { ...after, route } : after;
}
export function presentationTarget(session) {
  const route = compactRoute(session?.route);
  return route ? { kind: 'bot', id: route.bot } : session?.sel ? { kind: 'thread', id: session.sel } : { kind: 'recent', id: '' };
}

// Renderer session/draft and effect adapter. Views is the shared ThinViews;
// it alone validates transport identity, generations, targets and RPC ids.
export class ThinHost {
  constructor(api, Views, next) {
    this.api = api;this.hub = api.hub;this.ready = false;this.window = null;
    this.session = clone(api.session());
    this.validatePresentation = api.validatePresentation || validatePresentation;
    const rid = this.session.rid;
    if (!Number.isSafeInteger(rid) || rid < 0 || rid >= 2 ** 47) throw Error('Invalid ordinary request counter');
    this.session.rid = Math.max(rid, Date.now());
    this.localRevision = 0;this.drafts = new Map();this.draftChanges = new Map();this.fields = new Map();this.pending = new Map();this.actions = [];this.active = null;
    this.messages = new MessageParts();this.ordinary = new Map();
    this.client = new Views({ now: api.now,
      send: (_hub, request) => ({ type: 'request', request: request.p.session ? { ...request,
        p: { ...request.p, session: requestSession(request.p.session, request.m === 'view.action', request.p.target, carriesDraft(request)) } } : request }),
      command: (command, input) => ({ type: 'command', command, input }),
      validateResult: api.validateResult || validateThinResult,
    }, next);
  }
  flush(result, metadata = {}) {
    if (!result || result.failed) { this.fail(result?.failed || 'View unavailable');return false; }
    if (result.id) {
      const request = result.cmds.find(c => c.type === 'request')?.request;
      this.pending.set(result.id, { revision: this.localRevision, target: request?.p.target, wireRevision: request?.p.requestRevision, input: request?.p, acks: [], ...metadata });
    }
    try {
      for (const item of result.cmds || []) {
        if (item.type !== 'request') throw Error('Invalid shared transport output');
        this.api.counter(this.client.next); // durable before transmission
        if (['view.open', 'view.page', 'view.update', 'view.action'].includes(item.request.m)) this.api.presentationRequest?.(clone(item.request));
        this.api.send(item.request);
      }
    } catch { this.fail('View request could not be sent; not retried');return false; }
    return true;
  }
  greeting(header) {
    if (this.greeted || header?.renderer !== (this.api.renderer || 'web')) { this.fail('Invalid view greeting');return; }
    const result = this.client.connect(this.hub, header);
    if (result.connected !== true) { this.fail('Unsupported view greeting');return; }
    this.greeted = true;this.epoch = header.epoch;
    this.select(this.api.target(this.session));
  }
  select(target) {
    // Unstarted intentions from another view never acquire the new target.
    // The already travelling action keeps its independent original lifetime.
    for (const operation of this.actions) operation.settled?.(false);
    this.actions = [];this.fields.clear();
    this.messages.clear();this.target = target;
    this.api.selection?.(target, this.session);
    this.ready = false;this.window = null;this.api.state({ phase: 'loading', remoteReady: false });
    return this.flush(this.client.select(this.hub, target, requestSession(this.session, false, target), this.api.cid), { selection: true });
  }
  request(action, value, settled) {
    if (action === 'draft' || action === 'select') {
      this.localRevision++;
      this.session = this.api.choice(this.session, action, value);
      if (action === 'draft') {
        const draft = { revision: this.localRevision, text: value };
        this.drafts.set(this.session.sel, draft);this.draftChanges.set(this.session.sel, draft);
      }
      this.api.sessionChanged(this.session);
      this.api.local(action, value, this.session);
      if (action === 'select') {
        if (this.greeted) this.select(this.api.target(this.session));
      } else {
        this.api.draftRefresh?.(() => this.update());
      }
      return true;
    }
    if (!this.ready) { settled?.(false);this.api.blocked();return true; }
    if (action === 'bfield') {
      const split = value.indexOf('\u001f');
      if (split <= 0) { settled?.(false);this.api.blocked();return false; }
      const name = value.slice(0, split), tail = this.actions.at(-1);
      // Only adjacent unsent edits coalesce. A Send is a hard ordering fence:
      // later text never changes the value captured before that Send.
      if (!(tail?.action === 'bfield' && tail.field === name) && this.actions.length >= 7) { settled?.(false);this.api.blocked();return false; }
      const revision = ++this.localRevision;
      const scope = this.api.operationScope?.(action, value);
      this.fields.set(name, { revision, text: value.slice(split + 1), scope });
      if (tail?.action === 'bfield' && tail.field === name) {
        tail.settled?.(false);Object.assign(tail, { value, revision, settled, scope });
      } else this.actions.push({ action, value, field: name, input: clone(this.session), revision, settled, scope });
      this.api.fieldsChanged?.(name, value.slice(split + 1));
      if (this.api.fieldRefresh) this.api.fieldRefresh(() => this.pump(true));
      else this.pump(true);
      return true;
    }
    if (this.actions.length >= 7) { settled?.(false);this.api.blocked();return true; }
    if (action === 'send' || action === 'send-alt' || action === 'mail-send') {
      const active = this.pending.get(this.active)?.operation;
      const draftRevision = action === 'mail-send' ? this.localRevision : this.drafts.get(this.session.sel)?.revision || 0;
      const sameSend = op => action === 'mail-send' ? op.action === action : ['send','send-alt'].includes(op.action);
      if ([active, ...this.actions].some(op => op && sameSend(op) && op.input.sel === this.session.sel && op.revision >= draftRevision)) {
        settled?.(false);this.api.blocked();return true;
      }
    }
    this.actions.push({ action, value, input: clone(this.session), revision: this.localRevision, settled, scope: this.api.operationScope?.(action, value) });
    this.pump(true);return true;
  }
  fieldValue(name, fallback) {
    const field = this.fields.get(name);
    return field && field.scope === this.api.operationScope?.('bfield', name + '\u001f') ? field.text : fallback;
  }
  draftChanged(thread, text) {
    this.draftChanges.set(thread, { revision: ++this.localRevision, text });
  }
  perform(action, value) {
    return new Promise(resolve => {
      const timer = setTimeout(() => resolve(false), 6000);
      this.request(action, value, accepted => { clearTimeout(timer);resolve(accepted); });
    });
  }
  adoptLocalSession(session) {
    this.validatePresentation({ session, view: { type: 'text', text: '' }, cmds: [] });
    if (session.rid >= 2 ** 47) throw Error('Invalid ordinary counter');
    this.session = { ...session, rid: Math.max(session.rid, this.session.rid) };
    this.api.sessionChanged(this.session);
  }
  pump(flushFields = false) {
    if (this.active || !this.ready || !this.actions.length) return;
    if (this.actions[0].action === 'bfield' && !flushFields) return;
    const operation = this.actions.shift();
    if (operation.scope !== this.api.operationScope?.(operation.action, operation.value)) {
      operation.settled?.(false);this.api.warning?.('Conversation changed before the action was sent');this.pump(flushFields);return;
    }
    const input = this.api.prepare(this.session, operation.input, operation.action);
    const result = this.client.request(this.hub, 'view.action', { session: requestSession(input, true, this.target), action: operation.action, value: operation.value });
    this.active = result.id || null;
    this.flush(result, { revision: operation.revision, operation });
  }
  page() {
    if (!this.ready || !this.window?.older) return false;
    return this.flush(this.client.request(this.hub, 'view.page', { cursor: this.window.older, session: requestSession(this.session, false, this.target) }));
  }
  search(query, cursor = '') {
    if (!this.ready) return false;
    return this.flush(this.client.request(this.hub, 'view.search', { query, cursor }));
  }
  message(entryId, cursor) {
    if (!this.ready) return false;
    const partial = this.window?.partial?.find(p => p.id === entryId);
    if (!partial) return false;
    const expected = this.messages.next(entryId);
    // Start at the beginning to assemble the complete message, rather than
    // silently treating the bounded initial preview as a complete prefix.
    const next = expected === undefined ? cursor ?? '' : expected;
    if (expected === undefined && next !== '' && next !== partial.cursor || expected !== undefined && cursor !== undefined && cursor !== next) return false;
    return this.flush(this.client.request(this.hub, 'view.message', { entryId, cursor: next }));
  }
  messageFromStart(entryId) {
    this.messages.reset(entryId);return this.message(entryId, '');
  }
  cancel(requestId) {
    const original = this.pending.get(requestId);
    if (!original || original.operation) return false;
    return this.flush(this.client.request(this.hub, 'view.cancel', { requestId }));
  }
  // whether a read carries the selected draft: while it may open the popup,
  // and once more after, so the hub shuts it
  popupRead() {
    return popupDraft(this.session.drafts?.[this.session.sel]) || !!this.popupSent;
  }
  update() {
    if (!this.ready) return false;
    const link = this.client.links.get(this.hub);
    const result = this.client.request(this.hub, 'view.update', { session: requestSession(this.session, false, this.target, this.popupRead()), anchor: this.api.anchor(), viewRevision: link.viewRevision });
    // Coalescing must not consume the one draft that closes a popup.
    if (result.id) this.popupSent = popupDraft(this.session.drafts?.[this.session.sel]);
    return this.flush(result);
  }
  receive(envelope) {
    if (envelope?.t === 'view.changed') {
      if (this.ready) {
        const result = this.client.changed(this.hub, envelope, requestSession(this.session, false, this.target, this.popupRead()), this.api.anchor());
        if (result.id) this.popupSent = popupDraft(this.session.drafts?.[this.session.sel]);
        this.flush(result);
      }
      return true;
    }
    if (envelope?.t !== 'reply') return false;
    const metadata = this.pending.get(envelope.id);
    const result = this.client.reply(this.hub, envelope);
    if (!result) return false;
    this.pending.delete(envelope.id);
    if (this.active === envelope.id) this.active = null;
    if (result.failed) {
      metadata?.operation?.settled?.(false);
      if (this.abortField(metadata) || this.isCurrent(metadata)) this.fail(result.failed);
      else this.api.warning?.(result.failed);
      this.pump(true);return true;
    }
    const r = result.result;
    if (['view.search', 'view.message', 'view.cancel'].includes(result.method)) {
      if (result.current) {
        if (result.method === 'view.search') this.api.search?.(r.search);
        else if (result.method === 'view.message') {
          try { this.api.message?.(this.messages.accept(r.message, metadata.input.cursor)); }
          catch { this.api.warning?.('Message continuation is unavailable; reopen it explicitly'); }
        } else this.api.cancelled?.(r.cancelled);
      }
      if (result.refreshDue) this.update();
      return true; // query results neither replace shown nor establish readiness
    }
    // Validate the entire normal renderer tree and every ordinary command
    // before the first effect. The shared callbacks above remain pure.
    let node;
    try {
      this.validatePresentation({ session: r.session, view: r.nextTarget ? { type: 'text', text: '' } : r.shown, cmds: r.cmds });
      if (r.session.rid >= 2 ** 47) throw Error('Ordinary counter overlaps view namespace');
      if (!r.nextTarget) node = this.api.decode(r.shown, r, metadata?.input);
    } catch {
      metadata?.operation?.settled?.(false);
      if (this.abortField(metadata) || result.current) this.fail('Invalid rendered view');
      else this.api.warning?.('Old action reply rejected');
      this.pump(true);return true;
    }
    let responseSession = r.session;
    for (const ack of metadata?.acks || []) responseSession = this.api.ack(responseSession, ack);
    this.session.rid = Math.max(this.session.rid, responseSession.rid);
    if (result.current) {
      const drafts = { ...responseSession.drafts, ...this.session.drafts };
      if (result.method === 'view.action') for (const [key, text] of Object.entries(responseSession.drafts)) {
        if (key === responseSession.sel || key.startsWith('@')) Object.defineProperty(drafts, key, { value: text, enumerable: true, writable: true, configurable: true });
      }
      this.session = { ...this.session, ...responseSession, rid: this.session.rid, drafts,
        info: { ...this.session.info, ...responseSession.info }, outbox: { ...this.session.outbox, ...responseSession.outbox } };
      for (const [thread, draft] of this.draftChanges) if (draft.revision > (metadata?.revision ?? -1)) {
        this.session.drafts = { ...this.session.drafts, [thread]: draft.text };
      }
    }
    if (this.ordinary.size + result.cmds.filter(c => c.command.type === 'send').length > 128) { this.fail('Ordinary reply backlog limit');return true; }
    // Original actions settle once even when navigation overtook their view.
    // Only their own Keep may clear a draft, and never a newer edit.
    for (const item of result.cmds) {
      if (item.type !== 'command') { this.fail('Invalid command adapter');return true; }
      const c = item.command, thread = this.api.threadKey?.(c.thread) ?? c.thread;
      const newer = this.drafts.get(thread)?.revision > (metadata?.revision ?? -1);
      if (c.type === 'keep' && newer) continue;
      if (c.type === 'send') {
        const rpc = JSON.parse(c.text);
        this.ordinary.set(rpc.id, { revision: metadata?.revision ?? -1, thread: item.input.session?.sel || '' });
      }
      this.api.command(c, item.input, result.current);
      if (c.type === 'keep') {
        this.session.drafts = { ...this.session.drafts, [thread]: c.text };
        this.draftChanged(thread, c.text);
      }
    }
    const fieldOperation = metadata?.operation;
    if (fieldOperation?.action === 'bfield') {
      const field = this.fields.get(fieldOperation.field);
      if (field?.revision === fieldOperation.revision) field.acknowledged = true;
    }
    if (result.current) for (const [name, field] of this.fields) {
      if (field.acknowledged && field.revision <= (metadata?.revision ?? -1)) this.fields.delete(name);
    }
    this.api.sessionChanged(this.session, { remote: true });
    metadata?.operation?.settled?.(result.current && result.remoteReady && !r.nextTarget);
    if (result.current && r.nextTarget) { this.select(r.nextTarget);return true; }
    if (result.current && result.remoteReady) {
      const partial = new Map(result.window.partial.map(row => [row.id, row]));
      for (const previous of this.window?.partial || []) {
        const current = partial.get(previous.id);
        if (!current || current.version !== previous.version) this.messages.reset(previous.id);
      }
      this.ready = true;this.window = result.window;
      this.api.view(node, { epoch: this.epoch, target: r.target, window: result.window, shown: r.shown, mailDraftContext: r.mailDraftContext });
      this.api.state({ phase: 'presented', remoteReady: true, requestRevision: r.requestRevision, viewRevision: r.viewRevision, viewId: r.viewId });
    } else if (result.current) this.fail('Current view is not ready');
    if (result.refreshDue) this.update();
    this.pump(true);return true;
  }
  ack(envelope) {
    const original = this.ordinary.get(envelope.id);
    if (!original) return false;
    this.ordinary.delete(envelope.id);
    const before = this.session;
    this.session = this.api.ack(this.session, envelope);
    for (const [thread, draft] of this.drafts) if (draft.revision > original.revision) {
      this.session = { ...this.session, drafts: { ...this.session.drafts, [thread]: before.drafts[thread] || '' } };
    }
    for (const thread of new Set([...Object.keys(before.drafts), ...Object.keys(this.session.drafts)])) {
      if (this.session.drafts[thread] !== before.drafts[thread]) this.draftChanged(thread, this.session.drafts[thread] || '');
    }
    if (original.thread && this.session.drafts[original.thread] !== before.drafts[original.thread]) {
      this.api.local('draft', this.session.drafts[original.thread] || '', { ...this.session, sel: original.thread });
    }
    for (const metadata of this.pending.values()) {
      if (metadata.acks.length >= 128) { this.fail('Reply backlog limit');return; }
      metadata.acks.push(envelope);
    }
    this.api.sessionChanged(this.session, { remote: true });
    return true;
  }
  expire() {
    const expired = this.client.expire();
    if (!expired.length) return;
    let current = false;
    for (const call of expired) {
      const metadata = this.pending.get(call.id);
      current = this.abortField(metadata) || this.isCurrent(metadata) || current;
      metadata?.operation?.settled?.(false);
      this.pending.delete(call.id);
      if (this.active === call.id) this.active = null;
    }
    if (current) { this.actions = [];this.fail('View request timed out; actions were not resent'); }
    else this.api.warning?.('Old request timed out; action was not resent');
    this.pump();
  }
  abortField(metadata) {
    const target = this.client.links.get(this.hub)?.target;
    if (metadata?.operation?.action !== 'bfield' || target?.kind !== metadata.target?.kind || target?.id !== metadata.target?.id) return false;
    for (const operation of this.actions) operation.settled?.(false);
    this.actions = [];return true;
  }
  isCurrent(metadata) {
    const link = this.client.links.get(this.hub);
    return !!metadata && !!link && metadata.wireRevision === link.revision && metadata.target?.kind === link.target?.kind && metadata.target?.id === link.target?.id;
  }
  fail(reason) {
    for (const metadata of this.pending.values()) metadata.operation?.settled?.(false);
    for (const operation of this.actions) operation.settled?.(false);
    this.ready = false;this.api.state({ phase: 'error', remoteReady: false, reason });
  }
  disconnect() {
    for (const metadata of this.pending.values()) metadata.operation?.settled?.(false);
    for (const operation of this.actions) operation.settled?.(false);
    this.client.disconnect(this.hub);this.pending.clear();this.actions = [];this.active = null;
    this.ready = false;this.api.state({ phase: 'disconnected', remoteReady: false });
  }
}
