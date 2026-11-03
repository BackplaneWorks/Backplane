// Disk backs the bounded, read-only transition cache. Records are partitioned
// by authenticated pairing identity and hub history epoch; none grants request
// authority, restores drafts, or changes the hub's unread/selection state.
const size = value => new TextEncoder().encode(JSON.stringify(value)).length;
export function recentPartition(scope) {
  return scope?.origin && /^[a-f0-9]{64}$/.test(scope.identity || '') && scope.epoch && scope.bundle
    ? JSON.stringify([1, scope.origin, scope.identity, scope.language, scope.epoch, scope.bundle]) : '';
}
export function pictureOnly(node) {
  if (node.type === 'text') return node;
  const field = ['textarea', 'input', 'select', 'option'].includes(node.tag);
  return { ...node, attrs: node.attrs.filter(a => !field || !['value', 'checked', 'selected'].includes(a.name)),
    kids: node.tag === 'textarea' ? [] : node.kids.map(pictureOnly) };
}
export class RecentViews {
  constructor({indexedDB, validate, limit = 8, bytes = 2 * 1024 * 1024, deadline = 1000, now = Date.now} = {}) {
    Object.assign(this, {indexedDB, validate, limit, bytes, deadline, now});this.pending = new Map();
  }
  open() {
    if (this.opened) return this.opened;
    return this.opened = new Promise(resolve => {
      let done = false;
      const finish = db => {if (done) {db?.close();return;} done = true;clearTimeout(timer);resolve(db);};
      const timer = setTimeout(() => finish(null), this.deadline);
      try {
        const request = this.indexedDB.open('backplane-recent-views-v1', 1);
        request.onupgradeneeded = () => request.result.createObjectStore('pictures', {keyPath:'key'});
        request.onsuccess = () => finish(request.result);
        request.onerror = request.onblocked = () => finish(null);
      } catch {finish(null);}
    });
  }
  valid(record, partition) {
    try {
      return record?.partition === partition && recentPartition(record.scope) === partition && record.key === partition + '\n' + record.viewKey
        && typeof record.viewKey === 'string' && typeof record.scope.selected === 'string' && record.scope.selected.length > 0
        && Number.isFinite(record.savedAt) && record.savedAt <= this.now() + 60000 && this.now() - record.savedAt < 24 * 3600000
        && size(record) <= this.bytes && this.validate(record.shown, record.scope) === true;
    } catch {return false;}
  }
  async load(scope) {
    const partition = recentPartition(scope);
    if (!partition) return [];
    const db = await this.open();
    if (!db) return [];
    return new Promise(resolve => {
      let done = false;
      const finish = rows => {if (!done) {done = true;clearTimeout(timer);resolve(rows);}};
      const timer = setTimeout(() => finish([]), this.deadline);
      try {
        const request = db.transaction('pictures').objectStore('pictures').getAll();
        request.onerror = () => finish([]);
        request.onsuccess = () => {
          let bytes = 0;
          finish(request.result.filter(row => this.valid(row, partition)).sort((a,b) => b.savedAt-a.savedAt).filter((row,i) => i < this.limit && (bytes += size(row)) <= this.bytes));
        };
      } catch {finish([]);}
    });
  }
  schedule(scope, viewKey, shown) {
    this.pending.delete(viewKey);this.pending.set(viewKey, {scope, shown});
    while (this.pending.size > this.limit) this.pending.delete(this.pending.keys().next().value);
    if (!this.scheduled) this.scheduled = setTimeout(() => this.flush(), 250);
  }
  flush() {
    this.scheduled = null;
    const item = this.pending.entries().next().value;
    if (!item) return;
    const [key, {scope, shown}] = item;this.pending.delete(key);
    void this.save(scope, key, shown).catch(() => {});
    if (this.pending.size) this.scheduled = setTimeout(() => this.flush(), 16);
  }
  async save(scope, viewKey, shown) {
    const partition = recentPartition(scope);
    if (!partition) return false;
    const record = {partition, viewKey, key:partition+'\n'+viewKey, scope, shown:pictureOnly(shown), savedAt:this.now()};
    if (!this.valid(record, partition)) return false;
    const db = await this.open();
    if (!db) return false;
    return new Promise(resolve => {
      try {
        const tx = db.transaction('pictures', 'readwrite'), store = tx.objectStore('pictures');
        store.put(record);
        // Bound the whole database, including previous owners/epochs/bundles.
        const request = store.getAll();
        request.onsuccess = () => {
          let bytes = 0;
          request.result.sort((a,b) => b.savedAt-a.savedAt).forEach((row,i) => {if (i >= this.limit || (bytes += size(row)) > this.bytes) store.delete(row.key);});
        };
        tx.oncomplete = () => resolve(true);tx.onerror = tx.onabort = () => resolve(false);
      } catch {resolve(false);}
    });
  }
}
