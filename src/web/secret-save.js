// Correlate secure saves without keeping a value, request body or retry queue.
// The caller sends the returned RPC once, then drops it. Web and phones use
// the same ACK, timeout and disconnect handling; only metadata waits here.
export class SecretSave {
  constructor(build, now = () => Date.now()) {
    this.build = build; this.now = now; this.next = 2 ** 44; this.pending = new Map();
  }
  begin(packed, hub, scope, binding = "") {
    if ([...this.pending.values()].some(w => w.hub === hub && w.scope === scope)) return { busy: true };
    if (this.next >= 2 ** 45) return { error: 'Reload this page before saving another secret.' };
    try {
      const result = JSON.parse(this.build(packed));
      if (result.error) return { error: result.error };
      const rpc = JSON.parse(result.rpc);
      const id = this.next++;
      rpc.id = id;
      // Raw secure RPCs bypass Ui.rpc, which ordinarily stamps this owner.
      rpc.p.accountBinding = binding;
      this.pending.set(id, { hub, scope, until: this.now() + 15000 });
      return { id, rpc };
    } catch { return { error: 'The secret could not be sent. Paste it again and retry.' }; }
  }
  finish(id, ok, why = 'The hub refused the secret') {
    const wait = this.pending.get(id);
    if (!wait) return null;
    this.pending.delete(id);
    return { ...wait, action: ok ? 'secret-sent' : 'secret-err', value: ok ? '' : why };
  }
  receive(hub, envelope) {
    if (envelope.t !== 'reply' || this.pending.get(envelope.id)?.hub !== hub) return null;
    return this.finish(envelope.id, envelope.ok === true,
      typeof envelope.error === 'string' && envelope.error ? envelope.error : 'The hub refused the secret');
  }
  cancel(hub) {
    return [...this.pending].filter(([, w]) => w.hub === hub).map(([id]) =>
      this.finish(id, false, 'Disconnected before the hub confirmed the save. Check Secrets before retrying.'));
  }
  expire() {
    return [...this.pending].filter(([, w]) => w.until <= this.now()).map(([id]) =>
      this.finish(id, false, 'The hub did not confirm the save. Check Secrets before retrying.'));
  }
  reset() { this.pending.clear(); }
  deadline() {
    return this.pending.size ? Math.min(...[...this.pending.values()].map(w => w.until)) : null;
  }
}

// Browser adapter. The inline note also survives an offline redraw, when a
// view action cannot reach the hub. No field value is retained for a retry.
export class SecretSaveHost {
  constructor({ build, scope, binding, send, dispatch, note }) {
    this.saves = new SecretSave(build);this.scope = scope;this.binding = binding;
    this.send = send;this.dispatch = dispatch;this.note = note;this.error = null;
  }
  paint() {
    if (this.error?.scope === this.scope()) this.note(this.error.value);
  }
  finish(result) {
    if (!result || result.scope !== this.scope()) return;
    this.error = result.action === 'secret-err' ? result : null;
    this.paint();this.dispatch(result.action, result.value);
  }
  save(packed) {
    const scope = this.scope();
    let request = this.saves.begin(packed, 'web', scope, this.binding());
    packed = '';
    if (request.error) this.finish({ scope, action: 'secret-err', value: request.error });
    else if (!request.busy) {
      try { this.send(request.rpc); }
      catch { this.finish(this.saves.finish(request.id, false, 'The secret could not be sent. Reconnect and paste it again.')); }
    }
    request = null;
    return true;
  }
  receive(envelope) {
    const result = this.saves.receive('web', envelope);
    if (!result) return false;
    this.finish(result);return true;
  }
  expire() { for (const result of this.saves.expire()) this.finish(result); }
  disconnect() { for (const result of this.saves.cancel('web')) this.finish(result); }
}
