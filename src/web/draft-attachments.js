// Raw draft bytes expire after an hour and share a 32 MiB browser budget.
// Only the current pairing can restore them through the hub's admission path.
export const MAX_AGE = 60 * 60 * 1000, MAX_BYTES = 32 * 1024 * 1024;
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
      request.onupgradeneeded = () => {
        // Old unscoped bytes cannot be assigned to the current pairing.
        if (request.result.objectStoreNames.contains('threads')) request.result.deleteObjectStore('threads');
        const files = request.result.createObjectStore('files', { keyPath: 'key' });
        files.createIndex('thread', 'thread');
      };
      request.onsuccess = () => {
        request.result.onversionchange = () => request.result.close();
        resolve(request.result);
      };
      request.onblocked = () => reject(Error('Attachment store is open in another page'));
      request.onerror = () => reject(request.error);
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
        tx.oncomplete = resolve;
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
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
  pending(thread) { return this.restoring.has(thread); }
  async sync(thread, paths, upload, current, actor = '') {
    if (!thread || thread.startsWith('@') || this.unavailable) return;
    if (this.started.get(thread) === actor || this.restoring.get(thread) === actor) return;
    this.started.set(thread, actor);
    this.restoring.set(thread, actor);
    const identity = this.identity();
    try {
      let records;
      await this.change(thread, saved => { records = saved; return saved; });
      for (const record of records) {
        if (!current() || this.identity() !== identity) { if (this.started.get(thread) === actor) this.started.delete(thread); return; }
        if (!paths.includes(record.path) && !await upload(record.file, record.path)) {
          if (this.started.get(thread) === actor) this.started.delete(thread);
          return;
        }
      }
    } catch {
      this.unavailable = true;
      if (this.started.get(thread) === actor) this.started.delete(thread);
    } finally {
      if (this.restoring.get(thread) === actor) this.restoring.delete(thread);
    }
  }
}
