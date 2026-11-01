// Local-only draft and screenshot. Scoped to the authenticated hub identity;
// screenshots never enter URLs, WebSocket frames or localStorage.
export class ImprovementsStore {
  constructor(indexedDB = globalThis.indexedDB) { this.indexedDB = indexedDB; }
  async db() {
    return this.opened ??= new Promise((resolve, reject) => {
      const r = this.indexedDB.open('backplane-improvements', 1);
      r.onupgradeneeded = () => r.result.createObjectStore('drafts');
      r.onsuccess = () => resolve(r.result);r.onerror = () => reject(Error('Could not keep this draft on this device.'));
    });
  }
  async transaction(scope, value, write = false) {
    const db = await this.db();
    return new Promise((resolve, reject) => {
      const tx = db.transaction('drafts', write ? 'readwrite' : 'readonly'), store = tx.objectStore('drafts');
      const r = write ? value === null ? store.delete(scope) : store.put(value, scope) : store.get(scope);
      tx.oncomplete = () => resolve(r.result);tx.onabort = tx.onerror = () => reject(Error('Could not keep this draft on this device.'));
    });
  }
  read(scope) { return this.transaction(scope); }
  write(scope, value) { return this.transaction(scope, value, true); }
}
