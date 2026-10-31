// Keep the picked bytes locally. Restoring uploads them through the usual
// admission path; saved browser data never supplies attachment authority.
export class DraftAttachments {
  constructor() {
    this.started = new Map();
    this.restoring = new Map();
    this.queue = Promise.resolve();
    this.db = new Promise((resolve, reject) => {
      const request = indexedDB.open('backplane-draft-attachments', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('threads');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    this.db.catch(() => {});
  }
  change(thread, update) {
    const next = this.queue.then(async () => {
      const db = await this.db;
      return new Promise((resolve, reject) => {
        const tx = db.transaction('threads', 'readwrite'), store = tx.objectStore('threads');
        const request = store.get(thread);
        request.onsuccess = () => {
          const original = request.result || [], records = update(original);
          if (records === original) return;
          if (records.length) store.put(records, thread); else store.delete(thread);
        };
        tx.oncomplete = resolve;
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
      });
    });
    this.queue = next.catch(() => {});
    return next;
  }
  remember(thread, path, file, previous = '') {
    return this.change(thread, records => [...records.filter(r => r.path !== path && r.path !== previous), { path, file }]);
  }
  forget(thread, paths) {
    return this.change(thread, records => records.filter(r => !paths.includes(r.path)));
  }
  pending(thread) { return this.restoring.has(thread); }
  async sync(thread, paths, upload, current, actor = '') {
    if (!thread || thread.startsWith('@')) return;
    if (this.started.get(thread) === actor || this.restoring.get(thread) === actor) return;
    this.started.set(thread, actor);
    this.restoring.set(thread, actor);
    try {
      let records;
      await this.change(thread, saved => { records = saved; return saved; });
      for (const record of records) {
        if (!current()) { if (this.started.get(thread) === actor) this.started.delete(thread); return; }
        if (!paths.includes(record.path) && !await upload(record.file, record.path)) {
          if (this.started.get(thread) === actor) this.started.delete(thread);
          return;
        }
      }
    } catch {
      if (this.started.get(thread) === actor) this.started.delete(thread);
    } finally {
      if (this.restoring.get(thread) === actor) this.restoring.delete(thread);
    }
  }
}
