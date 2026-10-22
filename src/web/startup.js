// Optional early-view transport. Bend owns choices, actions, rendering and
// ordinary RPCs; this owns request lifetime and the ordered replay boundary.
// The existing Bend Reply writer emits numeric ids. Keep these control ids
// in a reserved upper range, disjoint from ordinary presentation RPC ids.
let nextControlId = 2 ** 48 - 1;
export function validatePresentation(result) {
  if (!result.session || typeof result.session.sel !== 'string' || !result.session.drafts || Array.isArray(result.session.drafts)
    || !Number.isSafeInteger(result.session.rid) || result.session.rid < 0 || result.session.rid >= 2 ** 48 - 8192) throw Error('Invalid presentation session');
  const strings = (object, keys) => keys.every(key => typeof object[key] === 'string');
  for (const c of result.cmds) {
    if (!c || typeof c !== 'object') throw Error('Invalid presentation command');
    const fields = { send:['text'], copy:['text'], focus:['id'], scroll:[], connect:['url'], keep:['thread','text'],
      notify:['thread','title','body','key'], later:['action','value'], jump:['id'] }[c.type];
    if (!fields || !strings(c, fields) || c.type === 'later' && (!Number.isSafeInteger(c.ms) || c.ms < 0)) throw Error('Invalid presentation command');
    if (c.type === 'send') {
      const rpc = JSON.parse(c.text);
      if (!rpc || typeof rpc.m !== 'string' || !['string','number'].includes(typeof rpc.id) || !rpc.p || typeof rpc.p !== 'object') throw Error('Invalid presentation RPC');
    }
  }
  let remaining = 20000;
  const node = (n, depth) => {
    if (!n || --remaining < 0 || depth >= 64) throw Error('Invalid presentation tree');
    if (n.type === 'text' && typeof n.text === 'string') return;
    if (n.type !== 'element' || typeof n.tag !== 'string' || !/^[a-z][a-z0-9-]*$/.test(n.tag) || ['script','base','meta','link','object','embed'].includes(n.tag)
      || typeof n.key !== 'string' || !Array.isArray(n.attrs) || n.attrs.length > 256 || !Array.isArray(n.kids)) throw Error('Invalid presentation element');
    for (const a of n.attrs) {
      if (typeof a?.event === 'string' && a.event && typeof a.action === 'string') continue;
      // Bend's sidebar memo uses an inert selection flag named "on".
      const memoSelection = n.tag === 'bp-memo' && a?.name === 'on' && (a.value === '' || a.value === '1');
      if (!a || typeof a.name !== 'string' || !/^[a-zA-Z_:][\w:.-]*$/.test(a.name) || (/^on/i.test(a.name) && !memoSelection) || typeof a.value !== 'string') throw Error('Invalid presentation attribute');
    }
    for (const child of n.kids) node(child, depth + 1);
  };
  node(result.view, 0);
}
export class FrontendStartup {
  // Bound: a bare reference to window.setTimeout called as this.setTimer(...)
  // throws "Illegal invocation" in Chromium, which stopped compatibility sync.
  constructor(api, { setTimer = (f, ms) => setTimeout(f, ms), clearTimer = id => clearTimeout(id) } = {}) {
    this.api = api;
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.owned = new Set();
    this.revision = 0;
    this.session = api.session();
    // Disjoint ordinary request ids, within Bend's immediate Nat domain.
    const rid = this.session.rid, namespace = Date.now();
    this.invalidCounter = !Number.isSafeInteger(rid) || rid < 0 || rid >= 2 ** 48 - 8192
      || !Number.isSafeInteger(namespace) || namespace < 0 || namespace >= 2 ** 48 - 8192;
    if (!this.invalidCounter) this.session.rid = Math.max(rid, namespace);
    this.waiting = [];
    this.choices = [];
    this.calls = new Map();
    this.phase = 'waiting';
    this.restoreDone = false;
    this.hasView = false;
    this.closed = false;
  }
  opened() {
    if (this.invalidCounter) { this.legacy('unsafe presentation counter'); return; }
    this.helloTimer = this.setTimer(() => this.legacy('missing frontend greeting'), 2000);
  }
  inherit(pending) {
    if (!pending) return;
    this.session = pending.session;
    this.waiting.push(...pending.operations);
    this.choices.push(...(pending.choices || []));
    for (const operation of this.waiting) this.revision = Math.max(this.revision, operation.revision);
    for (const choice of this.choices) this.revision = Math.max(this.revision, choice.revision);
    this.changed = !!pending.changed || this.waiting.length > 0 || this.choices.length > 0;
  }
  ready() {
    this.restoreDone = true;
    this.history();
  }
  greeting(header) {
    if (this.closed || this.phase !== 'waiting') return;
    if (this.invalidCounter) { this.legacy('unsafe presentation counter'); return; }
    if (header?.protocol !== 1 || typeof header.generation !== 'string' || !/^\d{1,10}$/.test(header.generation)) {
      this.legacy('invalid frontend greeting');
      return;
    }
    this.clearTimer(this.helloTimer);
    this.generation = header.generation;
    this.phase = 'presentation';
    if (!this.waiting.length) this.waiting.push({ action: '', value: '', input: this.session, revision: ++this.revision });
    this.pump();
    this.history();
  }
  history() {
    // Never wait for a screen. The cursor is read only after the cache loop
    // has fully stopped, including on a failed partial restore.
    if (this.closed || !this.restoreDone || !this.generation || this.historySent) return;
    const cursor = this.api.cursor();
    if (!Number.isSafeInteger(cursor.since) || cursor.since < 0 || cursor.since > 4294967295 || typeof cursor.origin !== 'string') {
      this.legacy('invalid applied cursor');
      return;
    }
    const id = this.id();
    if (id === null) return;
    this.historySent = true;
    this.calls.set(id, { history: true });
    try { this.api.send({ id, m: 'frontend.history', p: { generation: this.generation, ...cursor } }); }
    catch { this.legacy('history send failed'); return; }
    this.historyTimer = this.setTimer(() => this.legacy('canonical history did not start'), 5000);
  }
  id() {
    if (nextControlId <= 2 ** 48 - 8192) { this.legacy('startup control ids exhausted'); return null; }
    const id = nextControlId--;
    this.owned.add(id);
    return id;
  }
  owns(id) { return Number.isSafeInteger(id) && this.owned.has(id); }
  request(action, value) {
    if (this.closed) return false;
    if (this.invalidCounter) return this.api.blocked(action), true;
    if (!this.api.allowed(action)) return this.api.blocked(action), true;
    const revision = ++this.revision;
    this.changed = true;
    this.session = this.api.choice(this.session, action, value);
    this.choices.push({ action, value, revision });
    this.api.local?.(action, value, this.session);
    if (action === 'select') { this.hasView = false; this.api.hide(); }
    if (this.flight?.operation.action === '') {
      this.calls.delete(this.flight.id);
      this.clearTimer(this.screenTimer);
      this.flight = null;
    }
    this.waiting = this.waiting.filter(op => op.action !== '');
    this.clearTimer(this.draftTimer);
    // Native editing is immediate; refresh the normal Bend view after a
    // pause, without one full view request per keystroke.
    if (action === 'draft') {
      this.draftTimer = this.setTimer(() => {
        if (this.closed || this.phase !== 'presentation') return;
        this.waiting.push({ action: '', value: '', input: this.session, revision: this.revision });
        this.pump();
      }, 75);
      return true;
    }
    this.waiting.push({ action, value, input: this.session, revision });
    this.pump();
    return true;
  }
  pump() {
    if (this.closed || this.phase !== 'presentation' || !this.generation || this.flight || !this.waiting.length) return;
    const id = this.id();
    if (id === null) return;
    const operation = this.waiting.shift();
    const call = this.flight = { id, operation };
    this.calls.set(id, call);
    try {
      this.api.send({ id, m: 'frontend.screen', p: { cid: this.api.cid(), generation: this.generation,
        session: this.api.prepare(this.session, operation.input, operation.action), action: operation.action,
        value: operation.value, revision: operation.revision } });
    } catch { this.screenFailed(call); return; }
    this.screenTimer = this.setTimer(() => this.screenFailed(call), 2000);
  }
  screenFailed(call) {
    if (this.closed || this.flight !== call) return;
    this.calls.delete(call.id);
    this.clearTimer(this.screenTimer);
    this.clearTimer(this.draftTimer);
    this.flight = null;
    if (call.operation.action) this.waiting.unshift(call.operation);
    // The renderer is pure: unanswered operations have issued no RPC.
    // Retain ordinary intentions for canonical handoff, never retry here.
    this.phase = 'history';
    this.hasView = false;
    this.api.hide();
    this.history();
  }
  reply(envelope, opaque) {
    if (!this.owns(envelope?.id)) return false;
    const call = this.calls.get(envelope.id);
    if (this.closed || !call) return true; // includes retired late replies
    this.calls.delete(envelope.id);
    if (call.history) {
      if (envelope.ok !== true) this.legacy('history request refused');
      return true;
    }
    if (call !== this.flight) return true;
    const result = envelope.result;
    if (envelope.ok !== true || result?.frontend !== 1 || result.web !== 1 || result.revision !== call.operation.revision || !result.session || !Array.isArray(result.cmds)) {
      this.screenFailed(call);
      return true;
    }
    let node;
    try { node = this.api.decode(result, opaque); }
    catch { this.screenFailed(call); return true; }
    this.clearTimer(this.screenTimer);
    this.flight = null;
    this.session = result.session;
    for (const ack of this.acks || []) this.session = this.api.ack(this.session, ack);
    this.acks = [];
    const localUpdates = [];
    for (const choice of this.choices) if (choice.revision > call.operation.revision) {
      this.session = this.api.choice(this.session, choice.action, choice.value);
      localUpdates.push({ choice, session: this.session });
    }
    this.choices = this.choices.filter(c => c.revision > call.operation.revision);
    // Consume commands for the matching operation once even if newer input
    // overtook its view. View freshness and command lifetime are separate.
    try {
      for (const command of result.cmds) this.api.command(command);
      // In particular a Send's old Keep("") must not erase a newer draft
      // already written locally while that response was travelling.
      for (const { choice, session } of localUpdates) this.api.local?.(choice.action, choice.value, session);
    }
    catch {
      // Consumption already happened. A local command error must not make
      // a previously dispatched Send eligible for canonical replay.
      this.phase = 'history';
      this.hasView = false;
      this.api.hide();
      this.history();
      return true;
    }
    if (call.operation.revision === this.revision) {
      this.hasView = true;
      this.api.view(node);
    }
    this.pump();
    return true;
  }
  ack(envelope) {
    if (this.closed || !this.generation) return;
    this.session = this.api.ack(this.session, envelope);
    if (this.flight) (this.acks ||= []).push(envelope);
  }
  canonicalFrame() {
    this.clearTimer(this.helloTimer);
    this.clearTimer(this.historyTimer);
    // An old hub ignores frontend=1 and starts its ordinary replay directly.
    if (!this.generation) this.phase = 'legacy';
  }
  end() {
    if (this.closed) return;
    const { operations } = this.pending();
    const session = this.session;
    this.close(); // retire every presentation id BEFORE canonical adoption
    if (this.generation || this.changed) this.api.adopt(session);
    for (const operation of operations) {
      this.api.perform(operation);
      for (const choice of operation.laterChoices) this.api.adopt(this.api.choice(this.api.session(), choice.action, choice.value));
    }
  }
  legacy(reason) {
    if (this.closed) return;
    const pending = this.pending();
    this.close();
    this.api.legacy(reason, pending);
  }
  pending() {
    return { session: this.session, changed: !!this.changed || this.hasView,
      choices: this.choices,
      operations: [...(this.flight ? [this.flight.operation] : []), ...this.waiting].filter(op => op.action)
        .map(op => ({ ...op, laterChoices: this.choices.filter(c => c.revision > op.revision) })) };
  }
  close() {
    this.closed = true;
    this.hasView = false;
    this.clearTimer(this.helloTimer);
    this.clearTimer(this.historyTimer);
    this.clearTimer(this.screenTimer);
    this.clearTimer(this.draftTimer);
    this.calls.clear();
    this.api.hide();
  }
}
