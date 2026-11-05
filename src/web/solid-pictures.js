// The 3D preview as last drawn, per thread and model (solid.js's state key),
// kept across a reload in this origin's IndexedDB (never localStorage: the
// drafts live there). A view opened before its model is drawn shows the
// picture at once, at the pose and size it was taken with, and the model
// then draws over it in place. A few are kept, each small; a store that
// cannot open keeps none (the view simply waits, as before).
const DB = "backplane-solid-pictures-v1", STORE = "pictures";
export const KEEP = 4, MAX_CHARS = 400000;

export function validPicture(p) {
  return !!p && typeof p.key === "string" && !!p.key && typeof p.url === "string" && p.url.startsWith("data:image/") && p.url.length <= MAX_CHARS
    && [p.w, p.h, p.dpr].every(n => Number.isFinite(n) && n > 0) && !!p.state && typeof p.state === "object"
    && typeof p.stat === "string" && Number.isFinite(p.at);
}

export class SolidPictures {
  constructor(idb) {
    this.idb = idb; this.map = new Map();
    this.ready = this.load();
  }
  open() {
    if (this.opened) return this.opened;
    return this.opened = new Promise(resolve => {
      const timer = setTimeout(() => resolve(null), 1500);
      const done = db => { clearTimeout(timer); resolve(db); };
      try {
        const r = this.idb.open(DB, 1);
        r.onupgradeneeded = () => r.result.createObjectStore(STORE, { keyPath: "key" });
        r.onsuccess = () => done(r.result);
        r.onerror = r.onblocked = () => done(null);
      } catch { done(null); }
    });
  }
  async load() {
    const db = await this.open(); if (!db) return;
    const all = await new Promise(resolve => {
      try { const r = db.transaction(STORE).objectStore(STORE).getAll(); r.onsuccess = () => resolve(r.result || []); r.onerror = () => resolve([]); }
      catch { resolve([]); }
    });
    // oldest first; one taken meanwhile is newer than any stored
    const taken = this.map; this.map = new Map();
    for (const p of all.filter(validPicture).sort((a, b) => a.at - b.at)) this.map.set(p.key, p);
    for (const [key, p] of taken) { this.map.delete(key); this.map.set(key, p); }
    this.trim(db);
    for (const key of this.map.keys()) this.image(key);
  }
  get(key) { return this.map.get(key) || null; }
  // the picture decoded ahead (an image from a data URL paints a frame
  // after it is inserted otherwise): an <img> to put on the page as it is
  image(key) {
    const p = this.map.get(key); if (!p || typeof Image === "undefined") return null;
    if (p.image?.dataset.url !== String(p.at)) {
      const img = new Image(); img.alt = ""; img.decoding = "sync"; img.dataset.url = String(p.at); img.src = p.url;
      img.decode?.().catch(() => {});
      Object.defineProperty(p, "image", { value: img, configurable: true, enumerable: false, writable: true });
    }
    return p.image;
  }
  async put(p) {
    if (!validPicture(p)) return false;
    this.map.delete(p.key); this.map.set(p.key, p); this.image(p.key);
    const db = await this.open(); if (!db) return false;
    try { db.transaction(STORE, "readwrite").objectStore(STORE).put(p); } catch { return false; }
    this.trim(db);
    return true;
  }
  trim(db) {
    while (this.map.size > KEEP) {
      const key = this.map.keys().next().value; this.map.delete(key);
      try { db?.transaction(STORE, "readwrite").objectStore(STORE).delete(key); } catch {}
    }
  }
}
