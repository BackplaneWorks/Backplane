// The JS twin of cast.c: which clients show which browser page
// (src/server/castio.bend). A client showing a page's Browser tab says so
// (browser.seen) about every second; the hub sends it every frame of that
// page, and every other client only the frames marked "all". Kept by
// client and page with the monotonic time. cacheio.bend initializes the shared
// bounded LRU before these raw effects run. Its age includes the endpoint
// at exactly three seconds, matching the prior `age > 3000` expiry.

// client shows page now; answers whether the helper's lease is due again
// (none renewed for the page in the last second)
function cast_seen(client, page) {
  const now = Math.floor(performance.now());
  const seen = globalThis.BACKPLANE_CACHES.slot(0), leases = globalThis.BACKPLANE_CACHES.slot(1);
  const key = (client >>> 0) + "\n" + page;
  seen.set(key, String(now));
  const l = leases.get(page);
  let due = false;
  if (l === undefined) {
    leases.set(page, String(now));
    due = true;
  } else if (now - Number(l) >= 1000) {
    leases.set(page, String(now));
    due = true;
  }
  return due;
}

// the clients that showed page in the last few seconds
function cast_to(page) {
  let xs = { $: "Nil" };
  const cs = globalThis.BACKPLANE_CACHES.slot(0).entries(key => key.slice(key.indexOf('\n') + 1) === page);
  for (let i = cs.length - 1; i >= 0; i--) xs = { $: "Con", head: Number(cs[i][0].slice(0, cs[i][0].indexOf('\n'))), tail: xs };
  return xs;
}

function cast_drop(client) {
  const seen = globalThis.BACKPLANE_CACHES.slot(0), prefix = (client >>> 0) + '\n';
  for (const [key] of seen.entries(key => key.startsWith(prefix))) seen.delete(key);
  return {$: CID(Unit)};
}

io_eff(CID(CastFx.seen), cast_seen);
io_eff(CID(CastFx.to), cast_to);
io_eff(CID(CastFx.drop), cast_drop);
