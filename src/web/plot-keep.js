// The last plot of each board or schematic this page drew, kept in the
// browser (IndexedDB, per hub: the page's own origin) so that after a reload
// or in a new window the viewer draws it at once, while the hub's plot is on
// its way; plot2d.js then puts the hub's in its place (identical chunks are
// kept as they are, so nothing fades or moves). Derived data only: the wire
// chunks as the hub sent them, bounded (8 plots, 6 MB each, 24 MB in all, the
// least recently drawn out), and a store that cannot open is simply none.
const DB = "bp-plots-v1", STORE = "plots", MAX = 8, ONE = 6 << 20, ALL = 24 << 20;
let opened = null;
function db() {
  if (opened) return opened;
  opened = new Promise((resolve) => {
    try {
      const r = indexedDB.open(DB, 1);
      r.onupgradeneeded = () => r.result.createObjectStore(STORE);
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => resolve(null);
      r.onblocked = () => resolve(null);
    } catch { resolve(null); }
  });
  return opened;
}
const done = (req) => new Promise((resolve) => { req.onsuccess = () => resolve(req.result); req.onerror = () => resolve(undefined); });

// every kept plot: key -> { raw, box, fam, thick, at, bytes }
export async function loadAll() {
  const d = typeof indexedDB === "undefined" ? null : await db();
  const out = new Map();
  if (!d) return out;
  try {
    const st = d.transaction(STORE, "readonly").objectStore(STORE);
    const [keys, values] = await Promise.all([done(st.getAllKeys()), done(st.getAll())]);
    (keys || []).forEach((k, i) => { const v = values?.[i];if (typeof k === "string" && v && Array.isArray(v.raw)) out.set(k, v); });
  } catch {}
  return out;
}

// keep one (too big: not kept, and an older one of that key goes); then
// the least recently drawn past 8 plots or 24 MB go
export async function save(key, entry) {
  const d = typeof indexedDB === "undefined" ? null : await db();
  if (!d) return;
  try {
    const big = !(entry.bytes <= ONE);
    await done(big ? d.transaction(STORE, "readwrite").objectStore(STORE).delete(key) : d.transaction(STORE, "readwrite").objectStore(STORE).put(entry, key));
    if (big) return;
    const all = await loadAll();
    const rows = [...all].map(([k, v]) => [k, v.at || 0, v.bytes || 0]).sort((a, b) => b[1] - a[1]);
    let n = 0, bytes = 0;
    const gone = [];
    for (const [k, , b] of rows) { n += 1;bytes += b;if (n > MAX || bytes > ALL) gone.push(k); }
    if (gone.length) { const st = d.transaction(STORE, "readwrite").objectStore(STORE);for (const k of gone) st.delete(k); }
  } catch {}
}
