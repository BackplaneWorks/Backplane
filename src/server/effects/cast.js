// The JS twin of cast.c: which clients show which browser page
// (src/server/castio.bend). A client showing a page's Browser tab says so
// (browser.seen) about every second; the hub sends it every frame of that
// page, and every other client only the frames marked "all". Kept by
// client and page with the monotonic time.
const bp_cast = { seen: new Map(), leases: new Map() };

// how long a client counts as showing a page after it said so
const BP_CAST_HOLD = 3000;

function bp_cast_prune(now) {
  for (const [key, c] of bp_cast.seen) {
    if (now - c.at > BP_CAST_HOLD) bp_cast.seen.delete(key);
  }
}

// client shows page now; answers whether the helper's lease is due again
// (none renewed for the page in the last second)
function cast_seen(client, page) {
  const now = Math.floor(performance.now());
  bp_cast_prune(now);
  const key = (client >>> 0) + "\n" + page;
  const c = bp_cast.seen.get(key);
  if (c) c.at = now;
  else bp_cast.seen.set(key, { client: client >>> 0, page, at: now });
  const l = bp_cast.leases.get(page);
  let due = false;
  if (l === undefined) {
    bp_cast.leases.set(page, now);
    due = true;
  } else if (now - l >= 1000) {
    bp_cast.leases.set(page, now);
    due = true;
  }
  return due;
}

// the clients that showed page in the last few seconds
function cast_to(page) {
  bp_cast_prune(Math.floor(performance.now()));
  let xs = { $: "Nil" };
  const cs = [...bp_cast.seen.values()].filter((c) => c.page === page);
  for (let i = cs.length - 1; i >= 0; i--) xs = { $: "Con", head: cs[i].client, tail: xs };
  return xs;
}

io_eff(CID(Cast.seen), cast_seen);
io_eff(CID(Cast.to), cast_to);
