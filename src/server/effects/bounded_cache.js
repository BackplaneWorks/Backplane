// Shared, weighted LRU for derived primitive metadata. The two independent
// orders are bounded Maps: reads promote LRU, writes renew age. Replacements
// remove their old age node; there is no growing expiry queue or timer list.
// Limits come from cacheio.bend. Never retain model objects or history here.
function cache_ready(budget, total_bytes) {
  const {entries, bytes, age_ms} = budget;
  for (const n of [entries, bytes, age_ms, total_bytes]) {
    if (!Number.isSafeInteger(n) || n <= 0 || n > 0xffffffff) throw Error('Invalid cache budget');
  }
  const policy = {entries,bytes,age_ms,total_bytes};
  if (globalThis.BACKPLANE_CACHES) {
    for (const key of Object.keys(policy)) if (globalThis.BACKPLANE_CACHES.policy[key] !== policy[key]) throw Error('Cache policy changed during process lifetime');
    return {$: CID(Unit)};
  }
  const globalLRU = new Map(), slots = [];
  let charged = 0, clock = 0, serial = 0;
  const now = () => (clock = Math.max(clock, Math.floor(performance.now())));
  const count = (st, k) => {st[k] = Math.min(Number.MAX_SAFE_INTEGER, st[k] + 1);};
  const owned = s => Buffer.from(s, 'utf16le').toString('utf16le');
  function remove(c, key, reason) {
    const node = c.rows.get(key);
    if (!node) return false;
    c.rows.delete(key);c.age.delete(key);globalLRU.delete(node);
    c.charged -= node.cost;charged -= node.cost;
    if (reason) count(c.counters, reason);
    return true;
  }
  function expire(c, at) {
    for (const [key, node] of c.age) {
      if (node.until > at) break;
      remove(c, key, 'expired');
    }
  }
  function touch(c, node) {
    c.rows.delete(node.key);c.rows.set(node.key, node);
    globalLRU.delete(node);globalLRU.set(node, c);
  }
  function open(name) {
    const c = {name, rows:new Map(), age:new Map(), charged:0,
      counters:{hits:0,misses:0,evictions:0,expired:0,rejected:0}};
    const api = {
      get(key) {
        expire(c, now());
        const node = c.rows.get(key);
        if (!node) {count(c.counters, 'misses');return undefined;}
        count(c.counters, 'hits');touch(c, node);return node.value;
      },
      set(key, value) {
        const at = now();expire(c, at);
        // Copy strings once so a small substring cannot retain a giant parent.
        if (typeof key !== 'string' || typeof value !== 'string') {
          if (typeof key === 'string') remove(c, key);
          count(c.counters, 'rejected');return false;
        }
        const cost = 256 + 2 * key.length + 2 * value.length;
        if (!Number.isSafeInteger(cost) || cost > bytes || cost > total_bytes) {
          // A refused replacement must not leave a stale value available.
          remove(c, key);count(c.counters, 'rejected');return false;
        }
        const previous = c.rows.get(key), birth = previous ? previous.birth : ++serial;
        remove(c, key);
        while (c.rows.size >= entries || c.charged + cost > bytes) remove(c, c.rows.keys().next().value, 'evictions');
        if (charged + cost > total_bytes) for (const s of slots) expire(s.c, at);
        while (charged + cost > total_bytes) {
          const [node, owner] = globalLRU.entries().next().value;
          remove(owner, node.key, 'evictions');
        }
        const node = {key:owned(key),value:owned(value),cost,until:at + age_ms,birth};
        c.rows.set(node.key, node);c.age.set(node.key, node);globalLRU.set(node, c);
        c.charged += cost;charged += cost;return true;
      },
      delete(key) {return remove(c, key);},
      // Stable insertion order for callers whose result order is observable.
      entries(accept) {
        expire(c, now());const found=[];
        for (const node of c.rows.values()) if (!accept || accept(node.key,node.value)) found.push(node);
        return found.sort((a,b)=>a.birth-b.birth).map(n=>[n.key,n.value]);
      },
      clear() {for (const key of c.rows.keys()) remove(c, key);},
      stats() {expire(c, now());return {name,entries:c.rows.size,chargedBytes:c.charged,
        maxEntries:entries,maxBytes:bytes,maxAgeMs:age_ms,...c.counters};},
    };
    slots.push({c,api});
  }
  open('browser.seen');open('browser.lease');
  const metrics = () => {const rows = slots.map(s=>s.api.stats());return {chargedBytes:charged,maxBytes:total_bytes,slots:rows};};
  globalThis.BACKPLANE_CACHES = Object.freeze({
    policy:Object.freeze(policy),
    slot(id) {return Number.isInteger(id) ? slots[id]?.api : undefined;},
    // Expire entries before reporting the aggregate charge.
    stats:metrics,metrics,
  });
  return {$: CID(Unit)};
}
function cache_get(slot, key) {
  const value = globalThis.BACKPLANE_CACHES?.slot(slot)?.get(key);
  return value === undefined ? {$: CID(None)} : {$: CID(Some),value};
}
function cache_put(slot, key, value) {return !!globalThis.BACKPLANE_CACHES?.slot(slot)?.set(key,value);}
function cache_remove(slot, key) {globalThis.BACKPLANE_CACHES?.slot(slot)?.delete(key);return {$: CID(Unit)};}
function cache_stats() {return JSON.stringify(globalThis.BACKPLANE_CACHES?.metrics() || {chargedBytes:0,maxBytes:0,slots:[]});}
io_eff(CID(CacheFx.ready),cache_ready);
io_eff(CID(CacheFx.get),cache_get);
io_eff(CID(CacheFx.put),cache_put);
io_eff(CID(CacheFx.remove),cache_remove);
io_eff(CID(CacheFx.stats),cache_stats);
