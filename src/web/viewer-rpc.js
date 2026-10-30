// Bounded read-only viewer requests on the existing authenticated socket.
// IDs occupy a separate range below ThinViews' 2^47 base. A reply is consumed
// only by its owner; reconnect/timeout cannot apply it to another source.
export class ViewerRpc {
  constructor(send, { timeoutMs = 15000 } = {}) { this.send = send; this.timeoutMs = timeoutMs; this.next = 2 ** 46; this.pending = new Map(); this.generation = 0; }
  async components(thread, { signal } = {}) {
    const generation = this.generation, started = performance.now();
    let data = await this.request("kicad.components", { thread }, "components", this.timeoutMs, signal);
    if (signal?.aborted) throw new DOMException("Viewer read canceled", "AbortError");
    if (generation !== this.generation) throw Error("Connection changed; component metadata must be refreshed.");
    // Older hubs still return one complete response. New hubs page large
    // saved boards; no partial page becomes selection or rendering authority.
    if (!data.page) return data;
    const first = data, rows = [], ids = new Set(), identity = ["schema", "source", "schematic", "sourceRevision", "schematicRevision", "fingerprint", "thicknessMm"];
    if (data.schema !== 1 || !data.source || !data.sourceRevision || typeof data.schematicRevision !== "string") throw Error("Invalid saved component page identity.");
    const total = data.page.total;
    if (!Number.isSafeInteger(total) || total < 0 || total > 4096) throw Error("Saved component count exceeds the preview limit.");
    let offset = 0, bytes = 0;
    for (let page = 0; page < 256; page++) {
      if (generation !== this.generation) throw Error("Connection changed; component metadata must be refreshed.");
      if (identity.some(key => data[key] !== first[key])) throw Error("Saved CAD changed between component pages. Refresh after the save finishes.");
      bytes += new TextEncoder().encode(JSON.stringify(data)).length;
      if (bytes > 8 * 1024 * 1024) throw Error("Saved component metadata exceeds the 8 MiB preview limit.");
      const p = data.page, part = data.components;
      if (!p || p.offset !== offset || p.total !== total || !Array.isArray(part) || part.length > 32 || offset + part.length > total) throw Error("Invalid saved component page sequence.");
      for (const c of part) {
        if (!c || typeof c.id !== "string" || !c.id || ids.has(c.id)) throw Error("Ambiguous saved component identity across pages.");
        ids.add(c.id); rows.push(c);
      }
      offset += part.length;
      if (p.next === null) {
        if (p.complete !== true || offset !== total) throw Error("Saved component pages are incomplete.");
        return { ...first, components: rows, page: { offset: 0, next: null, total, complete: true } };
      }
      if (!part.length || p.complete !== false || p.next !== offset || offset >= total) throw Error("Invalid saved component page cursor.");
      if (performance.now() - started > 120000) throw Error("Saved component read exceeded its time limit. Retry after the save finishes.");
      data = await this.request("kicad.components", { thread, offset, sourceRevision: first.sourceRevision, schematicRevision: first.schematicRevision }, "components", this.timeoutMs, signal);
    }
    throw Error("Saved component metadata exceeds the page limit.");
  }
  renderAsset(thread, path, kind, revision, { signal } = {}) {
    if (!revision) return Promise.reject(new Error("Waiting for the saved model revision before preparing quality rendering."));
    return this.request("kicad.renderAsset", { thread, path, kind, revision }, "asset", 120000, signal);
  }
  request(method, params, field, timeoutMs = this.timeoutMs, signal) {
    if (signal?.aborted) return Promise.reject(new DOMException("Viewer read canceled", "AbortError"));
    if (!params.thread) return Promise.reject(new Error("No selected thread for model inspection."));
    if (this.pending.size >= 4) return Promise.reject(new Error("Viewer reads are busy; retry shortly."));
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      const cleanup = () => { clearTimeout(timer); signal?.removeEventListener("abort", abort); this.pending.delete(id); };
      const abort = () => { cleanup(); reject(new DOMException("Viewer read canceled", "AbortError")); };
      const timer = setTimeout(() => { cleanup(); reject(new Error("Saved model read timed out. Retry after the save finishes.")); }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer, field, cleanup });
      signal?.addEventListener("abort", abort, { once: true });
      try { this.send({ id, m: method, p: params }); }
      catch (error) { cleanup(); reject(error); }
    });
  }
  receive(envelope) {
    if (envelope?.t !== "reply") return false;
    const call = this.pending.get(envelope.id); if (!call) return false;
    call.cleanup();
    if (envelope.ok === false || envelope.result?.error || !envelope.result?.[call.field]) call.reject(new Error(envelope.result?.error || envelope.error || "Saved model data is unavailable."));
    else call.resolve(envelope.result[call.field]);
    return true;
  }
  reset() {
    this.generation++;
    for (const call of this.pending.values()) { call.cleanup(); call.reject(new Error("Connection changed; component metadata must be refreshed.")); }
    this.pending.clear();
  }
}
