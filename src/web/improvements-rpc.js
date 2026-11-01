// Ordinary authenticated RPC, in its own bounded ID range. No reconnect replay.
export class ImprovementsRpc {
  constructor(send, timeoutMs = 15000) { this.send = send; this.timeoutMs = timeoutMs; this.next = 2 ** 45; this.pending = new Map(); }
  request(m, p) {
    if (this.pending.size >= 2) return Promise.reject(Error('A request is still being sent.'));
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id);reject(Error('The hub did not answer. Reconnect, then press Send to retry the same request.')); }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      try { this.send({ id, m, p }); } catch (e) { clearTimeout(timer);this.pending.delete(id);reject(e); }
    });
  }
  receive(o) {
    if (o?.t !== 'reply') return false;
    const call = this.pending.get(o.id);if (!call) return false;
    clearTimeout(call.timer);this.pending.delete(o.id);
    if (o.ok === false || o.error || o.result?.error) call.reject(Error(o.result?.error || o.error || 'The hub could not accept this request. Retry Send.'));
    else call.resolve(o.result ?? o);
    return true;
  }
  reset() {
    for (const call of this.pending.values()) { clearTimeout(call.timer);call.reject(Error('Connection changed. Reconnect, then press Send to retry the same request.')); }
    this.pending.clear();
  }
}
