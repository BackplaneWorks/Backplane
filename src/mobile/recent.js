// Transport for the optional presentation session. Decisions and commands
// come from Bend; this only serializes requests, rejects obsolete replies,
// and hands transient choices back after the ordered log finishes.
export class Recent {
  constructor(api) {
    this.api = api;
    this.links = new Map();
    this.calls = new Map();
    this.next = 4_000_000_000_000;
    this.revision = 0;
  }
  active(key) { return this.links.has(key); }
  pending() { return this.links.size > 0; }
  open(key, generation) {
    const abandoned = this.abort(key, 'connection changed');
    const link = { key, generation, session: this.api.session(key), waiting: [], flight: null,
      history: false, complete: false, shown: null, drafts: new Map(), revision: ++this.revision };
    // Separate command ids from the canonical client's in-flight requests.
    link.session.rid = Math.max(Number(link.session.rid || 0), Date.now() * 1000);
    this.links.set(key, link);
    link.waiting.push({ action: '', value: '', revision: link.revision, input: link.session });
    return [...(abandoned?.cmds || []), ...this.pump(link)];
  }
  close(key) {
    const old = this.links.get(key);
    if (!old) return;
    if (old.flight) this.calls.delete(old.flight.id);
    this.links.delete(key);
  }
  abort(key, reason) {
    const link = this.links.get(key);
    if (!link) return null;
    const operations = [...(link.flight ? [link.flight.operation] : []), ...link.waiting];
    const cmds = operations.flatMap(op => this.api.failed?.(key, op.input, op.action) || []);
    this.close(key);
    return { key, failed: reason, cmds };
  }
  ack(key, envelope) {
    const link = this.links.get(key);
    if (!link || !this.api.reply) return;
    link.session = this.api.reply(link.session, envelope);
    if (link.flight) (link.acks ||= []).push(envelope);
  }
  refresh(key) {
    const link = this.links.get(key);
    if (!link || link.waiting.some(op => op.action === '')) return [];
    return this.request(key, '', '');
  }
  request(key, action, value) {
    const link = this.links.get(key);
    if (!link) return null;
    // A refresh has no commands. A person action need not wait for its
    // presentation response; the old id cannot issue anything afterward.
    if (action && link.flight?.operation.action === '') {
      this.calls.delete(link.flight.id);
      link.flight = null;
      link.waiting = link.waiting.filter(op => op.action !== '');
    }
    link.revision = ++this.revision;
    link.waiting.push({ action, value, revision: link.revision, input: link.session });
    return this.pump(link);
  }
  draft(key, text) {
    const link = this.links.get(key);
    if (!link) return null;
    const change = this.api.draft(link.session, text, link.key);
    link.session = change.session;
    // A draft typed while a response was travelling takes precedence over
    // that older response. Its own next action captures it below.
    link.drafts.set(link.session.sel, { thread: link.session.sel, text, revision: ++this.revision });
    return change.cmds;
  }
  pump(link) {
    if (link.flight || !link.waiting.length) return [];
    const operation = link.waiting.shift();
    const id = ++this.next;
    link.flight = { id, operation, began: this.api.now() };
    this.calls.set(id, link);
    return [this.api.send(link.key, { id, m: 'mobile.screen', p: {
      key: link.key, cid: this.api.cid(), generation: link.generation,
      session: this.api.prepare(link.session, operation.input, operation.action), action: operation.action, value: operation.value,
    } })];
  }
  reply(key, envelope) {
    const link = this.calls.get(Number(envelope.id));
    if (!link || link.key !== key || this.links.get(key) !== link) return null;
    this.calls.delete(Number(envelope.id));
    const flight = link.flight;
    link.flight = null;
    if (envelope.ok !== true || envelope.result?.mobile !== 1) {
      // Include this operation: it never produced its ordinary RPC.
      link.waiting.unshift(flight.operation);
      return this.abort(key, String(envelope.error || 'mobile presentation unavailable'));
    }
    const result = envelope.result;
    link.session = this.api.reconcile ? this.api.reconcile(result.session, key) : result.session;
    for (const ack of link.acks || []) link.session = this.api.reply(link.session, ack);
    link.acks = [];
    let newDraftCommands = [];
    for (const draft of link.drafts.values()) if (draft.revision > flight.operation.revision) {
      const change = this.api.draft(link.session, draft.text, link.key, draft.thread);
      link.session = change.session;
      newDraftCommands.push(...change.cmds);
    }
    link.shown = result.shown;
    const cmds = (result.cmds || []).map(c => this.api.command(c));
    cmds.push(...(this.api.issued?.(key, flight.operation.input, flight.operation.action) || []));
    // An old send's clear and the newer unsent draft are applied together.
    cmds.push(...newDraftCommands);
    if (!link.history) {
      link.history = true;
      cmds.push(this.api.send(key, { id: ++this.next, m: 'mobile.history', p: { generation: link.generation } }));
    }
    cmds.push(...this.pump(link));
    const current = flight.operation.revision === link.revision;
    if (link.complete && !link.flight && !link.waiting.length) {
      this.api.adopt(key, link.session);
      this.close(key);
      return { cmds, live: true, key, current };
    }
    return { cmds, key, current, history: true };
  }
  end(key) {
    const link = this.links.get(key);
    if (!link) return [];
    const operations = [...(link.flight ? [link.flight.operation] : []), ...link.waiting];
    // The renderer is pure: an unanswered presentation has not issued
    // its ordinary commands. Retire its id before executing those actions
    // against the now-complete canonical client, in the original order.
    this.close(key);
    this.api.adopt(key, link.session);
    const cmds = [];
    for (const operation of operations) {
      if (!operation.action) continue;
      cmds.push(...this.api.perform(key, operation));
      cmds.push(...(this.api.issued?.(key, operation.input, operation.action) || []));
      for (const draft of link.drafts.values()) if (draft.revision > operation.revision) {
        const change = this.api.draft(this.api.session(key), draft.text, key, draft.thread);
        this.api.adopt(key, change.session);
        cmds.push(...change.cmds);
      }
    }
    return cmds;
  }
  expire() {
    const failures = [];
    for (const [key, link] of this.links) {
      if (link.flight && this.api.now() - link.flight.began > 5000) {
        failures.push(this.abort(key, 'mobile presentation timed out'));
      }
    }
    return failures;
  }
  overlays() { return [...this.links.values()].filter(l => l.shown).map(l => l.shown); }
}
