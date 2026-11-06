// Raw draft bytes expire after an hour and share a 32 MiB browser budget.
// Only the current pairing can restore them through the hub's admission path.
export const MAX_AGE = 60 * 60 * 1000, MAX_BYTES = 32 * 1024 * 1024;
// Every wait here is bounded. A browser once left IndexedDB opens unanswered
// for a whole desktop session (2026-11-06): the store's first request never
// settled, nor anything queued behind it, and Send waited for it forever.
// STORE_WAIT bounds the open and each transaction; SEND_WAIT is how long Send
// waits for a saved batch before it goes without what was not restored.
export const STORE_WAIT = 4000, SEND_WAIT = 5000;
export function retained(record, identity, now) {
  return !!identity && record.identity === identity && Number.isFinite(record.at)
    && record.at <= now && now - record.at < MAX_AGE;
}
export class DraftAttachments {
  constructor({ identity = () => '', now = Date.now } = {}) {
    this.identity = identity;this.now = now;
    this.started = new Map();
    this.restoring = new Map();
    this.queue = Promise.resolve();
    this.db = new Promise((resolve, reject) => {
      const request = indexedDB.open('backplane-draft-attachments', 2);
      // An open still unanswered fails like a blocked one; if it lands later
      // its connection is closed rather than kept by a store that gave up.
      let late = false;
      const timer = setTimeout(() => { late = true;reject(Error('Attachment store did not open')); }, STORE_WAIT);
      request.onupgradeneeded = () => {
        // Old unscoped bytes cannot be assigned to the current pairing.
        if (request.result.objectStoreNames.contains('threads')) request.result.deleteObjectStore('threads');
        const files = request.result.createObjectStore('files', { keyPath: 'key' });
        files.createIndex('thread', 'thread');
      };
      request.onsuccess = () => {
        clearTimeout(timer);
        if (late) { request.result.close();return; }
        request.result.onversionchange = () => request.result.close();
        resolve(request.result);
      };
      request.onblocked = () => { clearTimeout(timer);reject(Error('Attachment store is open in another page')); };
      request.onerror = () => { clearTimeout(timer);reject(request.error); };
    });
    this.db.catch(() => {});
    void this.prune().catch(() => {});
  }
  transaction(update) {
    const next = this.queue.then(async () => {
      const db = await this.db;
      return new Promise((resolve, reject) => {
        const tx = db.transaction('files', 'readwrite');
        update(tx.objectStore('files'));
        // One that never settles would hold every later one behind it.
        const timer = setTimeout(() => { try { tx.abort(); } catch {} reject(Error('Attachment store did not answer')); }, STORE_WAIT);
        tx.oncomplete = () => { clearTimeout(timer);resolve(); };
        tx.onerror = () => { clearTimeout(timer);reject(tx.error); };
        tx.onabort = () => { clearTimeout(timer);reject(tx.error); };
      });
    });
    this.queue = next.catch(() => {});
    return next;
  }
  change(thread, update) {
    return this.transaction(store => {
      const request = store.index('thread').getAll(thread);
      request.onsuccess = () => {
        const original = request.result, saved = original.filter(r => retained(r, this.identity(), this.now()));
        const records = update(saved);
        for (const record of original) if (!records.includes(record)) store.delete(record.key);
        // An unchanged File is never written again when another is picked.
        for (const record of records) if (!original.includes(record)) store.put({ ...record, thread,
          key: JSON.stringify([record.identity, thread, record.path]) });
      };
    });
  }
  async remember(thread, path, file, previous = '', project = '') {
    const identity = this.identity();
    if (!identity || file.size > MAX_BYTES) throw Error('Attachment retention unavailable');
    await this.change(thread, records => {
      const original = records.find(r => r.path === previous);
      if (previous && !original) return records;
      return [...records.filter(r => r.path !== path && r.path !== previous),
        { path, file, identity, project: original?.project || project, at: original?.at ?? this.now() }];
    });
    await this.prune();
  }
  forget(thread, paths = null) {
    return this.change(thread, records => paths ? records.filter(r => !paths.includes(r.path)) : []);
  }
  clear() {
    this.started.clear();
    return this.transaction(store => store.clear());
  }
  prune(project = '') {
    return this.transaction(store => {
      const request = store.getAll();
      request.onsuccess = () => {
        let bytes = 0;
        const records = request.result.sort((a, b) => b.at - a.at);
        for (const record of records) {
          if (!retained(record, this.identity(), this.now()) || project && record.project === project
              || (bytes += record.file.size) > MAX_BYTES) store.delete(record.key);
        }
      };
    });
  }
  // Send waits while a batch restores, for at most SEND_WAIT; then the batch
  // is stalled and the page abandons it (Send goes without it).
  pending(thread) {
    const restore = this.restoring.get(thread);
    return !!restore && this.now() - restore.at < SEND_WAIT;
  }
  stalled(thread) { return this.restoring.has(thread) && !this.pending(thread); }
  // The actor that gave up keeps its start: it does not begin the batch again
  // at the next Send; a later view (another actor) restores what is left.
  abandon(thread) { this.restoring.delete(thread); }
  restores(thread, actor) { return this.restoring.get(thread)?.actor === actor; }
  async sync(thread, paths, upload, current, actor = '') {
    if (!thread || thread.startsWith('@') || this.unavailable) return;
    if (this.started.get(thread) === actor || this.restores(thread, actor)) return;
    this.started.set(thread, actor);
    const restore = { actor, at: this.now() };
    this.restoring.set(thread, restore);
    const identity = this.identity();
    try {
      let records;
      await this.change(thread, saved => { records = saved; return saved; });
      for (const record of records) {
        if (this.restoring.get(thread) !== restore) return;
        if (!current() || this.identity() !== identity) { if (this.started.get(thread) === actor) this.started.delete(thread); return; }
        // A refused upload is not tried again at every Send by this actor:
        // each try held Send while it ran, and the next view retries it.
        if (!paths.includes(record.path) && !await upload(record.file, record.path)) return;
      }
    } catch {
      this.unavailable = true;
      if (this.started.get(thread) === actor) this.started.delete(thread);
    } finally {
      if (this.restoring.get(thread) === restore) this.restoring.delete(thread);
    }
  }
}
