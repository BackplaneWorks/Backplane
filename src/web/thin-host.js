import { validatePresentation } from './startup.js';
import { validateThinResult, MessageParts } from './thin-results.js';
import { compactRoute, utf8Bytes } from './thin.js';
import { viewerChoice } from './viewer-feedback.js';
import { terminalInputRequest } from './terminal-input.js';
import { terminalOutputFrame } from './terminal-output.js';
import { modelMenuRequest } from './model-menu-input.js';
import { SentClaims } from './sent-claims.js';

// how many entries the page holds before it lets the far end go: about
// forty screens: fewer and the reader overtakes the pages fetched again,
// more and every scroll frame costs more (timeline-scroller.js)
export const HELD_IDS = 600;
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
// A draft too long for every view request (core/draftbound.bend's twin):
// over 64 KB a request carries its first and last 16384 code points, joined
// by " … " when the cut middle held a space or line break and "…" when not,
// and marks it "<bytes> <hash>" as "@draft.bound". Send carries the whole
// text within a message's bound (131072 bytes); over it the hub refuses and
// says why. This page keeps (and saves) the full text either way.
const fnv = text => { let h = 0x811c9dc5;for (let i = 0; i < text.length; i++) { h ^= text.charCodeAt(i);h = Math.imul(h, 0x01000193) >>> 0; } return h.toString(16); };
export function boundedDraft(text, action = '', keep = 16384, whole = 65536, send = 131072) {
  if (typeof text !== 'string' || text.length <= keep) return null;
  const bytes = utf8Bytes(text);
  if (bytes <= whole || ['send', 'send-alt'].includes(action) && bytes <= send) return null;
  const cps = Array.from(text);
  if (cps.length <= 2 * keep) return null;
  const middle = cps.slice(keep, cps.length - keep).join('');
  return { text: cps.slice(0, keep).join('') + (/[ \n\t\r]/.test(middle) ? ' … ' : '…') + cps.slice(cps.length - keep).join(''),
    marker: bytes + ' ' + fnv(text) };
}
// The page's own mark of its selected draft's size, which its composer
// reads to say a draft is over a message's bound (core/draftbound.bend's
// Bound.say): set on each edit and selection, so it is never stale. Only
// the bytes: the hash rides on the wire's mark alone.
function localBound(session) {
  const text = object(session?.drafts) && typeof session.sel === 'string' ? session.drafts[session.sel] : null;
  const bytes = typeof text === 'string' && text.length > 21845 ? utf8Bytes(text) : 0;
  const mark = bytes > 65536 ? String(bytes) : '';
  if ((session.drafts?.['@draft.bound'] ?? '') === mark) return session;
  return { ...session, drafts: { ...session.drafts, '@draft.bound': mark } };
}
// Reference suggestions are projected by the hub from this selected draft.
// Every view request keeps the same current-thread-only privacy boundary.
// bound: as it goes on the wire (the draft bounded for the action); the
// callers' own copy keeps the whole draft (what the view delta puts back).
function requestSession(session, action, target, bound = false) {
  const drafts = {}, local = object(session.drafts) ? session.drafts : {};
  for (const key of ['@lang', '@lang.os', '@lang.tag', '@lang.preview', '@desktop.browser']) {
    if (typeof local[key] === 'string' && utf8Bytes(local[key]) <= 128) drafts[key] = local[key];
  }
  if (session.sel && !session.sel.startsWith('@') && Object.hasOwn(local, session.sel) && typeof local[session.sel] === 'string') {
    const b = bound ? boundedDraft(local[session.sel], action) : null;
    Object.defineProperty(drafts, session.sel, { value: b ? b.text : local[session.sel], enumerable: true });
    if (b) drafts['@draft.bound'] = b.marker;
  }
  // the selected thread's viewer and its sheets: a fresh view (a reload)
  // starts from them (thin-actor.bend's Actor.seed.key); a running one
  // keeps its own
  const viewer = session.sel ? ['panel:', 'view.kind:', 'view.path:'].map(p => p + session.sel) : [];
  for (const key of session.sel ? ['@sch:' + session.sel, '@split.sheet:' + session.sel] : []) {
    if (typeof local[key] === 'string' && utf8Bytes(local[key]) <= 1024) Object.defineProperty(drafts, key, { value: local[key], enumerable: true });
  }
  // an intent edit's borrowed conversation draft and its mark (client.bend's
  // Intent.kept): a fresh actor (a reload, a reconnect) gives it back on Save
  for (const key of session.sel ? ['@intent.draft:' + session.sel, '@intent.edit:' + session.sel] : []) {
    if (typeof local[key] === 'string' && utf8Bytes(local[key]) <= 131072) Object.defineProperty(drafts, key, { value: local[key], enumerable: true });
  }
  const route = compactRoute(session.route, target);
  return { sel: session.sel, rid: session.rid, drafts, outbox: {},
    open: choices(session.open, (_key, item) => typeof item === 'boolean'),
    info: choices(session.info, (key, item) => (key.startsWith('fold:') || key.startsWith('subs:') || viewer.includes(key)) && typeof item === 'string' && utf8Bytes(item) <= 1024),
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
// Where a refused view (ThinHost.refuse) leaves the page, as a selection:
// the thread on show before, unless that is the one refused or was refused
// itself, else the project list (''); null once the project list itself
// was refused (the page then says the conversation is unavailable).
// refused: the keys ("kind:id") of the views refused so far.
export function refusedFallback(target, shown, refused) {
  const key = t => t ? t.kind + ':' + t.id : '';
  if (!target || target.kind === 'recent' || refused.has('recent:')) return null;
  return shown?.kind === 'thread' && shown.id && key(shown) !== key(target) && !refused.has(key(shown)) ? shown.id : '';
}

// Renderer session/draft and effect adapter. Views is the shared ThinViews;
// it alone validates transport identity, generations, targets and RPC ids.
export class ThinHost {
  constructor(api, Views, next) {
    this.api = api;this.hub = api.hub;this.ready = false;this.window = null;this.cursor = '';this.back = [];this.paging = null;this.pagingNext = null;this.overtaken = new Set();
    this.session = clone(api.session());
    this.validatePresentation = api.validatePresentation || validatePresentation;
    const rid = this.session.rid;
    if (!Number.isSafeInteger(rid) || rid < 0 || rid >= 2 ** 47) throw Error('Invalid ordinary request counter');
    this.session.rid = Math.max(rid, Date.now());
    this.viewerRevision = 0;this.localRevision = 0;this.selectionRevision = 0;this.drafts = new Map();this.draftChanges = new Map();this.fields = new Map();this.pending = new Map();this.actions = [];this.active = null;
    this.terminalBytes = 0;this.sent = new SentClaims();
    this.messages = new MessageParts();this.ordinary = new Map();this.refreshScheduled = null;this.pageError = null;
    this.pagePositions = new Map();this.pagePositionBytes = 0;this.restorePage = null;this.conversation = '';
    // The pages whose rows the page holds (timeline-history.bend keeps every
    // row drawn since the view was last reset), oldest first, each its trail
    // (cursor, back), the cursor older than it and its entry ids: the first
    // is the top edge, the last the bottom one (an empty trail: the newest
    // page). A scroll within them asks the hub for nothing (ahead). Past
    // HELD_IDS entries the far end's pages are let go (heldTrim; host.js
    // keeps their height). null: taken from the next view shown.
    this.held = null;this.pageMs = 500;
    // the reader away from the end: the thread and the newest entry its page
    // showed then (host.js's hold), which the hub's "N new" pill counts from
    this.awayMark = null;
    // a moving action's Focus and Jump, for the view it moves to (handOver)
    this.forView = null;
    // a view asked for and on its way (select): a press meanwhile waits for it
    this.opening = false;
    this.client = new Views({ now: api.now,
      sidebarRegions: (api.renderer || 'web') === 'web',
      // the hub's keyed view delta (view-delta.js); a page that turns it
      // off (api.viewDelta === false) gets whole views
      viewDelta: (api.renderer || 'web') === 'web' && api.viewDelta !== false,
      bodyPages: api.bodyPages === true,
      send: (_hub, request) => ({ type: 'request', request: request.p.session ? { ...request,
        p: { ...request.p, session: requestSession(request.p.session, request.m === 'view.action' ? request.p.action : '', request.p.target, true), away: this.awayOf(request.p.session.sel) } } : request }),
      // what the request limit measures: the request as it goes on the wire
      shape: (p, method) => p.session ? { ...p, session: requestSession(p.session, method === 'view.action' ? p.action : '', this.target, true) } : p,
      command: (command, input) => ({ type: 'command', command, input }),
      validateResult: api.validateResult || validateThinResult,
    }, next);
  }
  flush(result, metadata = {}) {
    if (!result || result.failed) { this.fail(result?.failed || 'View unavailable');return false; }
    if (result.id) {
      const request = result.cmds.find(c => c.type === 'request')?.request;
      this.pending.set(result.id, { viewerRevision: this.viewerRevision, revision: this.localRevision, target: request?.p.target, wireRevision: request?.p.requestRevision,
        pictureAuthority: request?.m !== 'view.cancel' && this.client.calls?.get(result.id)?.terminalInputIntent !== true, input: request?.p, acks: [], ...metadata });
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
    // A new view may have a fresh actor. Its response alone owns the upload
    // paths; locally retained bytes are admitted again by the upload helper.
    const drafts = { ...this.session.drafts };
    for (const key of Object.keys(drafts)) if (key.startsWith('@att:') || key.startsWith('@att.up:')) delete drafts[key];
    this.session = { ...this.session, drafts };
    if (this.forView && (this.forView.target.kind !== target?.kind || this.forView.target.id !== target?.id)) this.forView = null;
    this.rememberPage();
    this.restorePage = this.pagePositions.get(JSON.stringify([this.hub, this.epoch, target])) || null;
    this.retireTerminal('Queued terminal input was cancelled when the view changed.');
    this.refreshScheduled = null;
    this.viewerRevision++;this.api.viewerChoice?.(null);
    this.selectionRevision++;
    // Unstarted intentions from another view never acquire the new target.
    // The already travelling action keeps its independent original lifetime.
    for (const operation of this.actions) operation.settled?.(false);
    this.actions = [];this.terminalBytes = 0;this.fields.clear();
    this.messages.clear();this.target = target;
    this.ready = false;this.window = null;this.cursor = '';this.back = [];this.paging = null;this.pagingNext = null;this.pageError = null;this.awayMark = null;this.held = null;this.api.pageState?.(null);this.api.state({ phase: 'loading', remoteReady: false });
    this.api.selection?.(target, this.session);
    const sent = this.flush(this.client.select(this.hub, target, requestSession(this.session, false, target), this.api.cid), { selection: true });
    this.opening = sent;
    return sent;
  }
  rememberPage() {
    if (!this.ready || !this.target || this.paging?.direction === 'restore') return;
    const position = this.api.pagePosition?.();
    if (!position) return;
    const base = this.paging?.undo || this;
    // Bound bookmark preparation too; visiting deep history never copies an
    // arbitrarily large active trail into each conversation's retained state.
    if (base.back.length > 512) return;
    let cursor = base.cursor, back = base.back.slice();
    // The reader can still be in the outgoing visual bridge. Restore the
    // adjacent issued page that actually contains that visible message.
    if (position.adjacent === 'newer' && back.length) cursor = back.pop();
    else if (position.adjacent === 'older' && this.window?.older) { back.push(cursor);cursor = this.window.older; }
    const saved = { cursor, back, position: position.position, conversation: this.conversation };
    const bytes = utf8Bytes(JSON.stringify(saved)), key = JSON.stringify([this.hub, this.epoch, this.target]);
    const old = this.pagePositions.get(key);
    if (old) { this.pagePositionBytes -= old.bytes;this.pagePositions.delete(key); }
    if (bytes > 65536) return;
    this.pagePositions.set(key, { ...saved, bytes });this.pagePositionBytes += bytes;
    while (this.pagePositions.size > 8 || this.pagePositionBytes > 65536) {
      const oldest = this.pagePositions.keys().next().value;this.pagePositionBytes -= this.pagePositions.get(oldest).bytes;this.pagePositions.delete(oldest);
    }
  }
  // Every view request names the shown thread's away mark ("" at the end),
  // so the hub draws the pill over the newest page and any other.
  awayOf(thread) {
    return this.awayMark?.thread === thread ? this.awayMark.mark : '';
  }
  // The reader left the end (mark: the newest entry shown) or came back
  // (''): the view is asked for again, with the pill or without it.
  away(thread, mark) {
    const was = this.awayMark?.thread === thread ? this.awayMark.mark : '';
    this.awayMark = mark ? { thread, mark } : this.awayMark?.thread === thread ? null : this.awayMark;
    if (was !== mark && thread === this.session.sel) this.update();
  }
  terminalScope() {
    const link = this.client.links.get(this.hub);
    return JSON.stringify([link?.greeting.generation, link?.greeting.epoch, link?.viewId, this.target?.kind, this.target?.id,
      this.session.drafts?.['@terminal.owner'], this.api.terminalLifetime?.() || '']);
  }
  operationScope(action, value) {
    return action === 'term-key' || action === 'term-paste' ? this.terminalScope() : this.api.operationScope?.(action, value);
  }
  retireTerminal(reason) {
    let retired = false;
    this.actions = this.actions.filter(op => { if (!op.terminal) return true;op.settled?.(false);retired = true;return false; });
    this.terminalBytes = 0;
    if (retired) this.api.warning?.(reason);
  }
  request(action, value, settled) {
    const terminalInput = action === 'term-key' || action === 'term-paste';
    const retire = ['select','bot','bot-tab','uber','uber-bot','remote','ws-term-hide','term-toggle','ws-term-here'].includes(action) || action === 'ws-go' && value === 'term';
    if (retire) this.retireTerminal('Queued terminal input was cancelled when the view changed.');
    if (action === 'draft' || action === 'select') {
      this.localRevision++;
      this.session = this.api.choice(this.session, action, value);
      this.session = localBound(this.session);
      if (action === 'draft') {
        const draft = { revision: this.localRevision, text: value };
        this.drafts.set(this.session.sel, draft);this.draftChanges.set(this.session.sel, draft);
        this.sent.edited(this.session.sel, value);
      }
      // (a draft's key paints through local's own pacing, host.js's
      // paintDraft: not a whole page here as well)
      this.api.sessionChanged(this.session, action === 'draft' ? { inputOnly: true } : undefined);
      this.api.local(action, value, this.session);
      if (action === 'select') {
        if (this.greeted) this.select(this.api.target(this.session));
      } else {
        this.api.draftRefresh?.(() => {
          const link = this.client.links.get(this.hub);
          if (link) link.readInvalidated = true;
          this.update();
        });
      }
      return true;
    }
    // A press while the selected thread's view is on its way (a click right
    // after picking a thread) is for that view: it waits in the queue and
    // goes once the view is on show (pump). Only a terminal's keys, bound
    // to the view they were typed in, are refused, as is anything while no
    // view is coming (a failed or closed connection).
    if (!this.ready && !(this.opening && !terminalInput)) { settled?.(false);this.api.blocked();return true; }
    // a message sent from an earlier page shows the newest (the hub's
    // Hub.thin.act moves the page there too)
    // A page still on its way is dropped when it comes: shown after the
    // newest, it put the reader back on the old page and the message just
    // sent was nowhere to be seen.
    if (action === 'send' || action === 'send-alt') { for (const p of [this.paging, this.pagingNext]) if (p) this.overtaken.add(p.id);this.cursor = '';this.back = [];this.paging = null;this.pagingNext = null;this.held = null;this.pageError = null;this.api.pageState?.(null); }
    if (action === 'bfield') {
      const split = value.indexOf('\u001f');
      if (split <= 0) { settled?.(false);this.api.blocked();return false; }
      const name = value.slice(0, split), tail = this.actions.at(-1);
      // Only adjacent unsent edits coalesce. A Send is a hard ordering fence:
      // later text never changes the value captured before that Send.
      if (!(tail?.action === 'bfield' && tail.field === name) && this.actions.filter(op => !op.terminal).length >= 7) { settled?.(false);this.api.blocked();return false; }
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
    const viewer = viewerChoice(action, value);
    if (viewer) {
      const tail = this.actions.at(-1);
      if (tail?.viewer) { this.actions.pop();tail.settled?.(false); }
    }
    const normalQueued = this.actions.filter(op => !op.terminal).length;
    if (!terminalInput && normalQueued >= 7) { settled?.(false);this.api.blocked();return true; }
    if (viewer) { this.viewerRevision++;this.api.viewerChoice?.({ action, value }); }
    if (action === 'send' || action === 'send-alt' || action === 'mail-send') {
      const active = this.pending.get(this.active)?.operation;
      const draftRevision = action === 'mail-send' ? this.localRevision : this.drafts.get(this.session.sel)?.revision || 0;
      const sameSend = op => action === 'mail-send' ? op.action === action : ['send','send-alt'].includes(op.action);
      if ([active, ...this.actions].some(op => op && sameSend(op) && op.input.sel === this.session.sel && op.revision >= draftRevision)) {
        settled?.(false);this.api.blocked();return true;
      }
    }
    const scope = this.operationScope(action, value);
    if (terminalInput) {
      const owner = this.session.drafts?.['@terminal.owner'];
      if (!owner || this.session.open?.term !== true) { settled?.(false);this.api.warning?.('Terminal input was not sent: the terminal is closed.');return true; }
      // Input preparation uses the current authoritative session. Capture only
      // its intended terminal and scope; never clone a large chat draft per key.
      const input = { sel: this.session.sel, drafts: { '@terminal.owner': owner }, open: { term: true } };
      const bytes = utf8Bytes(value) + utf8Bytes(scope) + utf8Bytes(JSON.stringify(input));
      if (this.terminalBytes + bytes > 131072 || this.actions.filter(op => op.terminal).length >= 1024) {
        settled?.(false);this.api.warning?.('Terminal input was not sent: its queue is full. Wait for it to catch up.');return true;
      }
      this.terminalBytes += bytes;
      this.actions.push({ action, value, input, terminal: true, bytes, scope, revision: this.localRevision, settled });
    } else {
      const urgent = retire || action === 'stop' || action === 'interrupt';
      const operation = { action, value, viewer, viewerRevision: this.viewerRevision, input: clone(this.session), revision: this.localRevision, settled, scope, urgent };
      // what this send takes from the composer (sent-claims.js)
      if (action === 'send' || action === 'send-alt') operation.claim = this.sent.capture(this.session.sel, operation.input.drafts?.[this.session.sel]);
      // Stop and explicit close/switch controls remain responsive during input.
      if (urgent) this.actions.unshift(operation);
      else this.actions.push(operation);
    }
    this.pump(true);return true;
  }
  fieldValue(name, fallback) {
    const field = this.fields.get(name);
    return field && field.scope === this.api.operationScope?.('bfield', name + '\u001f') ? field.text : fallback;
  }
  draftChanged(thread, text) {
    this.draftChanges.set(thread, { revision: ++this.localRevision, text });
  }
  // The composer's edit events (host.js): an edit starting at `start` in
  // the selected thread's field, and the end of an IME composition, after
  // which a sent text due to come off the field does.
  touched(start) { this.sent.touch(this.session.sel, start); }
  composed() { this.cutSent(this.session.sel, this.sent.settle(this.session.sel)); }
  // An accepted send's text comes off the front of the draft the person
  // typed on with; what they typed after it stays as it is.
  cutSent(thread, cut) {
    const text = this.session.drafts[thread];
    if (!cut || typeof text !== 'string' || !text.startsWith(cut)) return;
    const rest = text.slice(cut.length);
    this.session.drafts = { ...this.session.drafts, [thread]: rest };
    this.drafts.set(thread, { ...this.drafts.get(thread), text: rest });
    this.draftChanged(thread, rest);
    this.api.local('draft', rest, { ...this.session, sel: thread });
    this.api.sessionChanged(this.session);
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
    // A page on its way is shown first (its reply pumps again): an action
    // sent meanwhile made it stale, and the refresh that drew it instead
    // drew no bridge from the page on screen. Stop and switches do not wait.
    if (this.paging && !this.actions[0].urgent) return;
    if (this.actions[0].action === 'bfield' && !flushFields) return;
    const operation = this.actions.shift();
    if (operation.terminal) this.terminalBytes -= operation.bytes;
    if (operation.scope !== this.operationScope(operation.action, operation.value)) {
      operation.settled?.(false);this.api.warning?.('Conversation changed before the action was sent');this.pump(flushFields);return;
    }
    const input = this.api.prepare(this.session, operation.input, operation.action);
    const result = this.client.request(this.hub, 'view.action', { session: requestSession(input, true, this.target), action: operation.action, value: operation.value,
      ...terminalInputRequest(operation.action, operation.input),
      ...modelMenuRequest(operation.action, operation.value, this.api.modelMenuReady?.() === true) });
    this.active = result.id || null;
    this.flush(result, { viewerRevision: operation.viewerRevision ?? this.viewerRevision, revision: operation.revision, operation, selection: this.selectionRevision });
  }
  // Scrolling pages: each older page overlaps the one before by half
  // (thin-view.bend's Page.mid), and the cursors that led here are kept so
  // scrolling back down retraces them to the newest ('' : no cursor). One
  // page request at a time: the next is taken from the page its reply
  // shows (a request made from a page still on its way would skip or
  // repeat one, and out of order replies left the view on an old page
  // while the trail said newest, so new messages never showed). A page
  // that fails puts the trail back as it was.
  pageTo(cursor, undo, direction, position, second = false) {
    const result = this.client.request(this.hub, 'view.page', { cursor, anchor: this.api.anchor(), session: requestSession(this.session, false, this.target) });
    if (!this.flush(result)) { this.cursor = undo.cursor;this.back = undo.back;return false; }
    const page = { id: result.id, undo, direction, before: this.window, position, destination: { cursor, back: this.back.slice() }, at: this.api.now?.() ?? Date.now(), olderNext: '' };
    this.pageError = null;
    if (second) this.pagingNext = page;else this.paging = page;
    this.api.pageState?.({ phase: 'loading', direction });
    return true;
  }
  page() {
    if (!this.ready || !this.window?.older || this.window.older === this.cursor || this.paging || this.pageActionPending()) return false;
    const undo = { cursor: this.cursor, back: this.back.slice() };
    this.back.push(this.cursor);this.cursor = this.window.older;
    return this.pageTo(this.cursor, undo, 'older');
  }
  newer() {
    if (!this.ready || !this.back.length || this.paging || this.pageActionPending()) return false;
    const undo = { cursor: this.cursor, back: this.back.slice() };
    this.cursor = this.back.pop();
    return this.pageTo(this.cursor, undo, 'newer');
  }
  // The page past the edge of the rows held, in the direction the reader
  // goes (host.js asks once they near it): older than the oldest page held,
  // newer than the newest; at the newest page's rows with the hub still on
  // an earlier one, the newest page itself, merged (not the reset newest()
  // makes), so new messages show and the rows above stay. A scroll inside
  // the rows held asks nothing: they are drawn already.
  //
  // Two older pages may be on their way while the reader scrolls fast
  // ({two: true}): a window that names its older page's own older cursor
  // (olderNext, the hub's thin-view.bend) lets the one after the page
  // asked for be asked for at once, and each one in turn, as it comes,
  // names the next (thin.js draws a run of pages in order).
  ahead(direction, { two = false } = {}) {
    if (!this.ready || this.pageActionPending()) return false;
    if (this.paging) return direction === 'older' && two ? this.aheadSecond() : false;
    const held = this.held || this.heldHere();
    if (direction === 'older') {
      const top = held.pages[0];
      if (!top.older || top.older === top.cursor) return false;
      if (!this.pageJump(top.older, [...top.back, top.cursor], 'older')) return false;
      this.paging.olderNext = top.olderNext || '';
      if (two) this.aheadSecond();
      return true;
    }
    const bottom = held.pages.at(-1);
    if (bottom.back.length) return this.pageJump(bottom.back.at(-1), bottom.back.slice(0, -1), 'newer');
    return this.atNewest() ? false : this.pageJump('', [], 'newer');
  }
  // whether ahead(direction) has a page to ask for
  hasAhead(direction) {
    if (!this.ready) return false;
    const held = this.held || this.heldHere(), top = held.pages[0], bottom = held.pages.at(-1);
    return direction === 'older' ? !!top.older && top.older !== top.cursor : bottom.back.length > 0 || !this.atNewest();
  }
  // the page after the older one on its way, when its window named it
  aheadSecond() {
    const first = this.paging;
    if (!first || this.pagingNext || first.direction !== 'older' || !first.olderNext || first.olderNext === first.destination.cursor) return false;
    return this.pageJump(first.olderNext, [...first.destination.back, first.destination.cursor], 'older', true);
  }
  pageJump(cursor, back, direction, second = false) {
    const undo = { cursor: this.cursor, back: this.back.slice() };
    this.cursor = cursor;this.back = back;
    return this.pageTo(cursor, undo, direction, undefined, second);
  }
  // a page as held: the trail its own request took (paged.destination; a
  // second page on its way has moved this.cursor on already)
  heldPage(at = this) {
    return { cursor: at.cursor, back: at.back.slice(), older: this.window?.older || '', olderNext: this.window?.olderNext || '', ids: (this.window?.ids || []).slice() };
  }
  heldHere() { return { pages: [this.heldPage()] }; }
  // the rows held are only the page on show (host.js reset its history)
  heldReset() { this.held = this.ready ? this.heldHere() : null; }
  heldShown(paged) {
    const page = this.heldPage(paged?.destination || this), same = p => p.cursor === page.cursor && p.back.length === page.back.length;
    if (!this.held || !paged || paged.direction === 'restore' || paged.direction === 'newest') { this.held = this.heldHere();return; }
    const pages = this.held.pages.filter(p => !same(p));
    if (paged.direction === 'older') pages.unshift(page);else pages.push(page);
    this.held = { pages };
    this.heldTrim(paged.direction);
  }
  // A view of a page held (an update after a change on the hub): its rows
  // and its older cursor as they are now.
  heldUpdate() {
    const at = this.held.pages.findIndex(p => p.cursor === this.cursor && p.back.length === this.back.length);
    if (at >= 0) this.held.pages[at] = this.heldPage();
  }
  // More than HELD_IDS entries held: the pages at the far end from the way
  // the reader goes are let go, never the two nearest; host.js takes their
  // rows off the page (api.letGo) and keeps their height in their place.
  heldTrim(direction) {
    const pages = this.held.pages, count = () => new Set(pages.flatMap(p => p.ids)).size;
    while (pages.length > 2 && count() > HELD_IDS) {
      const gone = direction === 'older' ? pages.pop() : pages.shift();
      const kept = new Set(pages.flatMap(p => p.ids));
      this.api.letGo?.(gone.ids.filter(id => !kept.has(id)), direction === 'older' ? 'newer' : 'older');
    }
  }
  // Straight to the newest page (the "new" pill on an earlier page): the
  // trail is let go, as a send does.
  newest() {
    if (!this.ready || this.atNewest() || this.paging || this.pageActionPending()) return false;
    const undo = { cursor: this.cursor, back: this.back.slice() };
    this.cursor = '';this.back = [];
    return this.pageTo('', undo, 'newest');
  }
  pageFailed(page, reason) {
    this.cursor = page.undo.cursor;this.back = page.undo.back;
    if (page.direction === 'restore' && reason === 'STALE_CURSOR') {
      const key = JSON.stringify([this.hub, this.epoch, this.target]), old = this.pagePositions.get(key);
      if (old) { this.pagePositionBytes -= old.bytes;this.pagePositions.delete(key); }
      // A rejected saved cursor grants no authority. Select fresh once, rather
      // than disabling a valid conversation or retrying an expired bookmark.
      this.ready = false;this.select(this.target);return;
    }
    this.pageError = { direction: page.direction, reason, ...(page.direction === 'restore' ? { destination: page.destination, position: page.position } : {}) };
    this.api.pageState?.({ phase: 'error', ...this.pageError });
    if (!this.api.pageState) this.api.warning?.('Messages could not load. Scroll again to retry; your position is kept.');
  }
  retryPage() {
    if (this.pageError?.direction === 'restore') {
      const { destination, position } = this.pageError, undo = { cursor: this.cursor, back: this.back.slice() };
      this.cursor = destination.cursor;this.back = destination.back.slice();
      return this.pageTo(this.cursor, undo, 'restore', position);
    }
    return this.pageError?.direction === 'newer' ? this.newer() : this.pageError?.direction === 'newest' ? this.newest() : this.pageError ? this.page() : false;
  }
  atNewest() { return this.back.length === 0; }
  search(query, cursor = '') {
    if (!this.ready) return false;
    return this.flush(this.client.request(this.hub, 'view.search', { query, cursor }));
  }
  body(part, cursor, tail = false) {
    if (!this.ready || this.active || this.paging || !part || part.bodyPage !== 1 || typeof cursor !== 'string' || !cursor) return false;
    return this.flush(this.client.request(this.hub, 'view.message', { entryId: part.id, cursor, bodyPage: 1, bodyVersion: part.version, bodyThread: part.thread, bodyTail: tail }), { bodyRead: true, pictureAuthority: false });
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
  uploadPending() {
    return ['attach-up', 'attach-end'].includes(this.pending.get(this.active)?.operation?.action);
  }
  botTabPending() {
    return this.pending.get(this.active)?.operation?.action === 'bot-tab' || this.actions.some(op => op.action === 'bot-tab');
  }
  viewerPending() {
    // A panel toggle is ordered once, just like a destination click. Refresh
    // must adopt its open/closed result before snapshotting the old panel.
    // Keep it out of viewerChoice: consecutive toggles must never coalesce.
    const changesViewer = op => !!op?.viewer || op?.action === 'panel';
    return changesViewer(this.pending.get(this.active)?.operation) || this.actions.some(changesViewer);
  }
  terminalPending() {
    const changesWorkspace = op => op && (op.action.startsWith('ws-') ||
      ['term-toggle', 'term-key', 'term-paste'].includes(op.action) ||
      op.action === 'bfield' && op.value.startsWith('wspalette\u001f'));
    return changesWorkspace(this.pending.get(this.active)?.operation) || this.actions.some(changesWorkspace);
  }
  composerPending() {
    const changesMenu = op => op && ['skill', 'skill-move', 'skill-close'].includes(op.action);
    return changesMenu(this.pending.get(this.active)?.operation) || this.actions.some(changesMenu);
  }
  threadChildPending() {
    return this.pending.get(this.active)?.operation?.action === 'thread-child' || this.actions.some(op => op.action === 'thread-child');
  }
  refreshPending() {
    // Every queued/issued action owns its one presentation result. A read
    // completing during an action must not send a newer picture request with
    // the pre-click flags, even for an otherwise side-effect-free menu toggle.
    return !!this.pending.get(this.active)?.operation || this.actions.length > 0;
  }
  pageActionPending() {
    // The automatic ordering-card lookup changes no presentation choice.
    // A wheel page may overtake its picture; its original validated commands
    // still settle once. All actions that can change the reader's view keep
    // the existing ordering fence, as do native and nonempty requests.
    const blocks = op => !!op && !((this.api.renderer || 'web') === 'web' && op.action === 'fab-look' && op.value === '');
    return blocks(this.pending.get(this.active)?.operation) || this.actions.some(blocks);
  }
  refreshSoon() {
    if (!this.api.deferRefresh) return this.update();
    const link = this.client.links.get(this.hub);
    if (!this.ready || !link || this.refreshScheduled) return false;
    const ticket = { selection: this.selectionRevision, viewId: link.viewId, link };
    this.refreshScheduled = ticket;
    this.api.deferRefresh(() => {
      if (this.refreshScheduled !== ticket) return;
      this.refreshScheduled = null;
      if (!this.ready || ticket.selection !== this.selectionRevision ||
          this.client.links.get(this.hub) !== ticket.link || ticket.link.viewId !== ticket.viewId) return;
      // An intervening action may already cover the notification. Otherwise
      // update retains every existing active/queued-action ownership guard.
      if (ticket.link.invalidated || ticket.link.readInvalidated) this.update();
    });
    return true;
  }
  update() {
    if (!this.ready) return false;
    const link = this.client.links.get(this.hub);
    // Keep upload receipts and Settings navigation visible before refreshing.
    // A newer read otherwise retires a same-target navigation reply, making
    // the person repeat the click. Refresh with its accepted session instead.
    if (this.paging || this.refreshPending() || this.composerPending() || this.threadChildPending() || this.terminalPending() || this.viewerPending() || this.botTabPending()) { link.invalidated = true;return true; }
    return this.flush(this.client.request(this.hub, 'view.update', { session: requestSession(this.session, false, this.target), anchor: this.api.anchor(), viewRevision: link.viewRevision }));
  }
  receive(envelope) {
    if (envelope?.t === 'view.terminal') {
      const link = this.client.links.get(this.hub);
      if (!this.ready || !terminalOutputFrame(envelope, link, this.session)) return true;
      try {
        this.validatePresentation({ session: this.session, view: envelope.pane, cmds: [] });
        const shown = this.api.terminal?.(this.api.decode(envelope.pane), envelope);
        if (shown === false) this.update(); // missing/replaced pane: normal bounded snapshot fallback
      } catch { /* Invalid display fragments have no effects or log credit. */ }
      return true;
    }
    if (envelope?.t === 'view.changed') {
      if (this.ready) {
        const result = this.client.changed(this.hub, envelope, requestSession(this.session, false, this.target), this.api.anchor(),
          !!this.api.deferRefresh || !!this.paging || this.refreshPending() || this.composerPending() || this.threadChildPending() || this.terminalPending() || this.viewerPending() || this.botTabPending());
        this.flush(result);
        if (this.api.deferRefresh && result.coalesced) this.refreshSoon();
      }
      return true;
    }
    if (envelope?.t !== 'reply') return false;
    const metadata = this.pending.get(envelope.id);
    const result = this.client.reply(this.hub, envelope);
    if (!result) return false;
    this.pending.delete(envelope.id);
    if (this.active === envelope.id) this.active = null;
    let paged = null, dropped = false;
    if (this.paging?.id === envelope.id) { paged = this.paging;this.paging = null; }
    else if (this.pagingNext?.id === envelope.id) { paged = this.pagingNext;this.pagingNext = null; }
    // the first of two pages back: the second is now the one on its way,
    // named the next by this one's window; a failed first lets it go too
    if (paged && this.pagingNext) {
      if (result.failed) { this.overtaken.add(this.pagingNext.id);this.pagingNext = null;dropped = true; }
      else { this.paging = this.pagingNext;this.pagingNext = null;this.paging.olderNext = result.window?.olderNext || ''; }
    }
    if (this.overtaken.delete(envelope.id)) { this.pump(true);return true; }
    if (paged && result.failed) {
      this.cursor = paged.undo.cursor;this.back = paged.undo.back;
      if (dropped || this.isCurrent(metadata)) { this.pageFailed(paged, result.failed);this.pump(true);return true; }
    }
    if (paged && !result.failed && !result.current) {
      // A later action can retire a page's picture after the hub applied its
      // cursor. Keep the trail aligned with that hub; refresh after the action.
      this.client.links.get(this.hub).invalidated = true;this.api.pageState?.(null);
    }
    if (result.failed) {
      if (metadata?.bodyRead) { this.api.bodyFailed?.(metadata.input, result.failed);this.pump(true);return true; }
      metadata?.operation?.settled?.(false);
      // a search refused (a query the hub does not take) is the search's
      // to say, not the view's: the view stays
      if (result.method === 'view.search') { this.api.searchFailed?.(result.failed);this.pump(true);return true; }
      if (this.refuse(metadata, result.failed, this.isCurrent(metadata))) return true;
      if (this.abortField(metadata) || this.isCurrent(metadata)) this.fail(result.failed);
      else this.api.warning?.(result.failed);
      this.pump(true);return true;
    }
    const r = result.result;
    if (['view.search', 'view.message', 'view.cancel'].includes(result.method)) {
      // a search answered after an update of the same view (a thread
      // streaming elsewhere) is still the search asked; another view's is not
      if (result.method === 'view.search') { if (result.current || this.sameTarget(metadata)) this.api.search?.(r.search); }
      else if (result.current || metadata?.bodyRead && this.sameTarget(metadata)) {
        if (result.method === 'view.message' && metadata?.bodyRead) this.api.body?.(r.message, metadata.input);
        else if (result.method === 'view.message') {
          try { this.api.message?.(this.messages.accept(r.message, metadata.input.cursor)); }
          catch { this.api.warning?.('Message continuation is unavailable; reopen it explicitly'); }
        } else this.api.cancelled?.(r.cancelled);
      }
      if (metadata?.bodyRead && !result.current && !this.sameTarget(metadata)) this.api.bodyFailed?.(metadata.input, 'overtaken');
      if (result.refreshDue) this.refreshSoon();
      return true; // query results neither replace shown nor establish readiness
    }
    // Validate the entire normal renderer tree and every ordinary command
    // before the first effect. The shared callbacks above remain pure.
    let node;
    try {
      this.validatePresentation({ session: r.session, view: r.nextTarget || result.inputOnly ? { type: 'text', text: '' } : r.shown, cmds: r.cmds });
      if (r.session.rid >= 2 ** 47) throw Error('Ordinary counter overlaps view namespace');
      if (!r.nextTarget && !result.inputOnly && (metadata?.viewerRevision ?? 0) === this.viewerRevision) node = this.api.decode(r.shown, r, metadata?.input);
    } catch {
      metadata?.operation?.settled?.(false);
      if (paged && result.current) this.pageFailed(paged, 'Invalid rendered view');
      else if (this.refuse(metadata, 'Invalid rendered view', result.current)) return true;
      else if (this.abortField(metadata) || result.current) this.fail('Invalid rendered view');
      else this.api.warning?.('Old action reply rejected');
      this.pump(true);return true;
    }
    // Background reads may retire this reply's picture, but cannot cancel the
    // person's navigation. Only an actual selection change retires its target.
    const navigation = !!r.nextTarget && result.method === 'view.action' && metadata?.operation && metadata.selection === this.selectionRevision;
    let responseSession = r.session;
    for (const ack of metadata?.acks || []) responseSession = this.api.ack(responseSession, ack);
    this.session.rid = Math.max(this.session.rid, responseSession.rid);
    if (result.current || navigation) {
      const drafts = { ...responseSession.drafts, ...this.session.drafts };
      if (result.method === 'view.action') for (const [key, text] of Object.entries(responseSession.drafts)) {
        // a draft the request carried bounded is this page's alone: what
        // the hub made of the bounded text is never adopted (or saved)
        if (key === responseSession.sel && metadata?.input?.session?.drafts?.['@draft.bound']) continue;
        if (key === responseSession.sel || key.startsWith('@')) Object.defineProperty(drafts, key, { value: text, enumerable: true, writable: true, configurable: true });
      }
      // each thread's viewer open or shut ("panel:<t>") is this page's own,
      // set as it leaves the thread (presentation.bend's Session.select);
      // the hub's view says only the selected one's, as "panel"
      const info = { ...this.session.info, ...responseSession.info };
      for (const [key, value] of Object.entries(this.session.info || {})) if (key.startsWith('panel:')) info[key] = value;
      this.session = { ...this.session, ...responseSession, rid: this.session.rid, drafts,
        info, outbox: { ...this.session.outbox, ...responseSession.outbox } };
      for (const [thread, draft] of this.draftChanges) if (draft.revision > (metadata?.revision ?? -1)) {
        this.session.drafts = { ...this.session.drafts, [thread]: draft.text };
      }
    }
    if (this.ordinary.size + result.cmds.filter(c => c.command.type === 'send').length > 128) { this.fail('Ordinary reply backlog limit');return true; }
    // Original actions settle once even when navigation overtook their view.
    // Only their own Keep may clear a draft, and never a newer edit.
    // An action that moves the page (nextTarget) and asks for a Focus or a
    // Jump means them for where it goes: they wait for that view (handOver).
    const moving = (result.current || navigation) && r.nextTarget, held = [];
    for (const item of result.cmds) {
      if (item.type !== 'command') { this.fail('Invalid command adapter');return true; }
      if (moving && (item.command.type === 'focus' || item.command.type === 'jump')) { held.push(item);continue; }
      const c = item.command, thread = this.api.threadKey?.(c.thread) ?? c.thread;
      const newer = this.drafts.get(thread)?.revision > (metadata?.revision ?? -1);
      if (c.type === 'keep' && newer) {
        const operation = metadata?.operation, claim = operation?.claim, sent = operation?.input.drafts?.[thread];
        // A send consumes the text it took, even if the next keystrokes
        // arrived before its Keep: off the field only while no edit touched
        // that text (sent-claims.js), never by what the new text starts with.
        if (c.text === '' && claim?.thread === thread && sent) {
          // Another Enter may already have captured that same text while
          // this send was on its way. It must send only what followed.
          for (const queued of this.actions) if (queued.claim?.over.includes(claim) && queued.input.drafts?.[thread]?.startsWith(sent)) {
            queued.input.drafts[thread] = queued.input.drafts[thread].slice(sent.length);
          }
          this.cutSent(thread, this.sent.accept(claim, this.api.composing?.(thread) === true));
        }
        continue;
      }
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
    // (a send whose text is not due to come off the field is done with it)
    if (fieldOperation?.claim && !fieldOperation.claim.due) this.sent.drop(fieldOperation.claim);
    if (fieldOperation?.action === 'bfield') {
      const field = this.fields.get(fieldOperation.field);
      if (field?.revision === fieldOperation.revision) field.acknowledged = true;
    }
    if (result.current) for (const [name, field] of this.fields) {
      if (field.acknowledged && field.revision <= (metadata?.revision ?? -1)) this.fields.delete(name);
    }
    this.api.sessionChanged(this.session, { remote: true, inputOnly: result.inputOnly });
    metadata?.operation?.settled?.(result.current && result.remoteReady && !r.nextTarget);
    if (moving) { this.forView = held.length ? { target: r.nextTarget, cmds: held } : null;this.select(r.nextTarget);return true; }
    if (result.current && result.remoteReady && result.inputOnly) {
      // Input ACKs settle the existing queue without redrawing the conversation
      // or consuming a pending output invalidation. Output still owns its view.
    } else if (result.current && result.remoteReady) {
      const partial = new Map(result.window.partial.map(row => [row.id, row]));
      for (const previous of this.window?.partial || []) {
        const current = partial.get(previous.id);
        if (!current || current.version !== previous.version) this.messages.reset(previous.id);
      }
      this.ready = true;this.opening = false;this.window = result.window;
      let position = paged?.position;
      if (result.method === 'view.open' && this.restorePage) {
        const saved = this.restorePage;this.restorePage = null;position = saved.position;
        if (saved.conversation !== r.session.sel) position = undefined;
        else if (saved.cursor) {
          this.cursor = saved.cursor;this.back = saved.back.slice();
          this.pageTo(this.cursor, { cursor: '', back: [] }, 'restore', position);return true;
        }
      }
      this.conversation = r.session.sel;
      if (paged) { this.pageError = null;this.api.pageState?.(null);this.pageMs = 0.7 * this.pageMs + 0.3 * Math.max(0, (this.api.now?.() ?? Date.now()) - paged.at); }
      if (paged || !this.held) this.heldShown(paged);
      else this.heldUpdate();
      if ((metadata?.viewerRevision ?? 0) === this.viewerRevision) this.api.view(node, { epoch: this.epoch, generation: r.generation, viewId: r.viewId, accountBinding: r.accountBinding,
        target: r.target, window: result.window, shown: r.shown, mailDraftContext: r.mailDraftContext,
        page: paged && { direction: paged.direction, before: paged.before }, position });
      if ((metadata?.viewerRevision ?? 0) === this.viewerRevision) this.api.viewerChoice?.(null);
      this.api.state({ phase: 'presented', remoteReady: true, requestRevision: r.requestRevision, viewRevision: r.viewRevision, viewId: r.viewId });
      this.handOver();
    } else if (result.current && !this.refuse(metadata, 'Current view is not ready', true)) this.fail('Current view is not ready');
    // Send queued navigation before a background read can snapshot its old route.
    this.pump(true);
    if (result.refreshDue || paged && !result.current) this.refreshSoon();
    return true;
  }
  // The view a moving action went to is on show: its Focus and Jump now
  // find what they name (the intent over the thread, an entry in it).
  handOver() {
    const due = this.forView;
    if (!due || due.target.kind !== this.target?.kind || due.target.id !== this.target?.id) return;
    this.forView = null;
    for (const item of due.cmds) this.api.command(item.command, item.input, true);
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
    let current = false, pageExpired = false;
    for (const call of expired) {
      const metadata = this.pending.get(call.id);
      if (metadata?.bodyRead) { this.api.bodyFailed?.(metadata.input, 'timeout');this.pending.delete(call.id);continue; }
      if (this.pagingNext?.id === call.id) { pageExpired = true;this.pagingNext = null;this.pending.delete(call.id);continue; }
      if (this.paging?.id === call.id) {
        pageExpired = true;
        const page = this.paging;this.paging = null;
        if (this.pagingNext) { this.overtaken.add(this.pagingNext.id);this.pagingNext = null; }
        if (this.isCurrent(metadata)) this.pageFailed(page, 'History request timed out');
        else { this.cursor = page.undo.cursor;this.back = page.undo.back;this.api.pageState?.(null); }
        this.pending.delete(call.id);continue;
      }
      current = this.abortField(metadata) || this.isCurrent(metadata) || current;
      metadata?.operation?.settled?.(false);
      this.pending.delete(call.id);
      if (this.active === call.id) this.active = null;
    }
    if (current) { this.actions = [];this.fail('View request timed out; actions were not resent'); }
    else if (!pageExpired) this.api.warning?.('Old request timed out; action was not resent');
    this.pump();
  }
  abortField(metadata) {
    const target = this.client.links.get(this.hub)?.target;
    if (metadata?.operation?.action !== 'bfield' || target?.kind !== metadata.target?.kind || target?.id !== metadata.target?.id) return false;
    for (const operation of this.actions) operation.settled?.(false);
    this.actions = [];return true;
  }
  sameTarget(metadata) {
    const link = this.client.links.get(this.hub);
    return !!metadata && !!link && metadata.target?.kind === link.target?.kind && metadata.target?.id === link.target?.id;
  }
  isCurrent(metadata) {
    const link = this.client.links.get(this.hub);
    return !!metadata && !!link && metadata.wireRevision === (metadata.pictureAuthority ? link.pictureRevision : link.revision) && metadata.target?.kind === link.target?.kind && metadata.target?.id === link.target?.id;
  }
  // The view a selection asked for, refused by the hub (STALE_VIEW: it has
  // no such thread, as a search result in a subagent's log once named) or
  // not one the page can show, is not asked for again, nor kept: the page
  // goes where it can (api.refused selects it) rather than stopping on
  // "Conversation unavailable", which every reload asked for again. Too
  // big a view (LIMIT) still says so.
  refuse(metadata, reason, current) {
    if (metadata?.selection !== true || reason === 'LIMIT' || !current || !this.api.refused) return false;
    this.api.pageState?.(null);this.forView = null;this.opening = false;this.restorePage = null;
    for (const operation of this.actions) operation.settled?.(false);
    this.actions = [];this.ready = false;
    return this.api.refused(metadata.target, reason) === true;
  }
  fail(reason) {
    this.api.pageState?.(null);this.forView = null;this.opening = false;
    this.retireTerminal('Queued terminal input was not sent because the connection failed.');
    this.refreshScheduled = null;
    this.api.viewerChoice?.(null);
    for (const metadata of this.pending.values()) metadata.operation?.settled?.(false);
    for (const operation of this.actions) operation.settled?.(false);
    this.ready = false;this.api.state({ phase: 'error', remoteReady: false, reason });
  }
  disconnect() {
    this.api.pageState?.(null);this.forView = null;this.opening = false;
    this.retireTerminal('Queued terminal input was not sent because the connection closed.');
    this.refreshScheduled = null;
    this.api.viewerChoice?.(null);
    for (const metadata of this.pending.values()) metadata.operation?.settled?.(false);
    for (const operation of this.actions) operation.settled?.(false);
    this.client.disconnect(this.hub);this.pending.clear();this.actions = [];this.active = null;
    this.ready = false;this.api.state({ phase: 'disconnected', remoteReady: false });
  }
}
