// Bounded read-only viewer requests on the existing authenticated socket.
// IDs occupy a separate range below ThinViews' 2^47 base. A reply is consumed
// only by its owner; reconnect/timeout cannot apply it to another source.
export class ViewerRpc {
  constructor(send, { timeoutMs = 15000 } = {}) { this.send = send; this.timeoutMs = timeoutMs; this.next = 2 ** 46; this.pending = new Map(); this.generation = 0; }
  async components(thread, { path = "", signal } = {}) {
    const generation = this.generation, started = performance.now();
    const source = path ? { thread, path } : { thread };
    let data = await this.request("kicad.components", source, "components", this.timeoutMs, signal);
    if (signal?.aborted) throw new DOMException("Viewer read canceled", "AbortError");
    if (generation !== this.generation) throw Error("Connection changed; component metadata must be refreshed.");
    // Older hubs still return one complete response. New hubs page large
    // saved boards; no partial page becomes selection or rendering authority.
    if (!data.page) return data;
    const first = data, rows = [], ids = new Set(), identity = ["schema", "source", "schematic", "sourceRevision", "schematicRevision", "fingerprint", "thicknessMm", "componentEncoding", "componentCount"];
    if (data.schema !== 1 || !data.source || !data.sourceRevision || typeof data.schematicRevision !== "string") throw Error("Invalid saved component page identity.");
    const total = data.page.total;
    if (!Number.isSafeInteger(total) || total < 0 || total > 4096) throw Error("Saved component count exceeds the preview limit.");
    const fragmented = data.componentEncoding === "field-fragments-v1", count = fragmented ? data.componentCount : total;
    if ((data.componentEncoding !== undefined && !fragmented) || !Number.isSafeInteger(count) || count < 0 || count > total) throw Error("Invalid saved component encoding or count.");
    let offset = 0, bytes = 0, partial = null;
    const append = c => {
      if (!c || typeof c.id !== "string" || !c.id || ids.has(c.id)) throw Error("Ambiguous saved component identity across pages.");
      ids.add(c.id); rows.push(c);
    };
    for (let page = 0; page < 256; page++) {
      if (generation !== this.generation) throw Error("Connection changed; component metadata must be refreshed.");
      if (identity.some(key => data[key] !== first[key])) throw Error("Saved CAD changed between component pages. Refresh after the save finishes.");
      bytes += new TextEncoder().encode(JSON.stringify(data)).length;
      if (bytes > 8 * 1024 * 1024) throw Error("Saved component metadata exceeds the 8 MiB preview limit.");
      const p = data.page, part = data.components;
      if (!p || p.offset !== offset || p.total !== total || !Array.isArray(part) || part.length > 32 || offset + part.length > total) throw Error("Invalid saved component page sequence.");
      for (const c of part) {
        if (fragmented && c?.fragment) {
          const f = c.fragment;
          if (typeof c.id !== "string" || !c.id || ids.has(c.id) || (partial && partial.id !== c.id) ||
              !Number.isSafeInteger(f.total) || f.total < 1 || f.total > 4096 || !Number.isSafeInteger(f.index) || f.index < 0 || f.index >= f.total ||
              f.complete !== (f.index === f.total - 1) || !f.fields || typeof f.fields !== "object" || Array.isArray(f.fields) || Object.keys(f.fields).length !== 1) throw Error("Invalid saved component field fragment.");
          if (!partial) partial = { id: c.id, fields: Object.create(null), next: 0, total: f.total };
          if (f.index !== partial.next++ || f.total !== partial.total) throw Error("Invalid saved component field sequence.");
          for (const [key, value] of Object.entries(f.fields)) {
            if (["__proto__", "constructor", "prototype", "fragment"].includes(key) || Object.hasOwn(partial.fields, key)) throw Error("Duplicate or invalid saved component field.");
            partial.fields[key] = value;
          }
          if (f.complete) {
            if (partial.fields.id !== partial.id) throw Error("Saved component fragment identity changed.");
            append({ ...partial.fields }); partial = null;
          }
        } else {
          if (partial) throw Error("Saved component field fragments are incomplete.");
          append(c);
        }
      }
      offset += part.length;
      if (p.next === null) {
        if (p.complete !== true || offset !== total || partial || rows.length !== count) throw Error("Saved component pages are incomplete.");
        const result = { ...first, components: rows, page: { offset: 0, next: null, total: count, complete: true } };
        delete result.componentEncoding; delete result.componentCount;
        return result;
      }
      if (!part.length || p.complete !== false || p.next !== offset || offset >= total) throw Error("Invalid saved component page cursor.");
      if (performance.now() - started > 120000) throw Error("Saved component read exceeded its time limit. Retry after the save finishes.");
      data = await this.request("kicad.components", { ...source, offset, sourceRevision: first.sourceRevision, schematicRevision: first.schematicRevision }, "components", this.timeoutMs, signal);
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
    const data = envelope.result?.[call.field];
    if (envelope.ok === false || envelope.result?.error || data?.error || !data) call.reject(new Error(envelope.result?.error || data?.error || envelope.error || "Saved model data is unavailable."));
    else call.resolve(data);
    return true;
  }
  reset() {
    this.generation++;
    for (const call of this.pending.values()) { call.cleanup(); call.reject(new Error("Connection changed; component metadata must be refreshed.")); }
    this.pending.clear();
  }
}
