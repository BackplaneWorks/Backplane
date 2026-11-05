// Viewer data kept hub-wide (server/viewer-kept.bend): the last answer of a
// thread's viewer reads (a Files folder's listing, ...), per thread and
// key, for the folder the thread works in (cwd: another worktree drops
// them). A view draws what is kept at once while it reads again; derived
// data only, bounded (entries and characters, least recently used out),
// lost with the process.
const bp_kept = { threads: new Map(), chars: 0, ahead: new Map(), seq: 0 };
const BP_KEPT_CHARS = 8 * 1024 * 1024, BP_KEPT_THREADS = 256, BP_KEPT_KEYS = 64, BP_KEPT_AHEAD_MS = 15000;
function kept_size(t) { let n = 0;for (const [k, e] of t.rows) n += k.length + e.v.length;return n; }
function kept_forget(thread) {
  const t = bp_kept.threads.get(thread);
  if (!t) return;
  bp_kept.chars -= kept_size(t);bp_kept.threads.delete(thread);
}
function kept_thread(thread, cwd, make) {
  let t = bp_kept.threads.get(thread);
  if (t && t.cwd !== cwd) { kept_forget(thread);t = undefined; }
  if (!t && make) t = { cwd, rows: new Map() };
  if (t) { bp_kept.threads.delete(thread);bp_kept.threads.set(thread, t); }
  return t;
}
function kept_trim() {
  while ((bp_kept.chars > BP_KEPT_CHARS || bp_kept.threads.size > BP_KEPT_THREADS) && bp_kept.threads.size)
    kept_forget(bp_kept.threads.keys().next().value);
}
function viewerkept_put(thread, cwd, key, value) {
  if (typeof thread !== 'string' || !thread || typeof cwd !== 'string' || typeof key !== 'string' || typeof value !== 'string') return '';
  bp_kept.ahead.delete(thread + '\n' + key);
  if (key.length + value.length > BP_KEPT_CHARS / 8) { viewerkept_drop(thread, key);return ''; }
  const t = kept_thread(thread, cwd, true), old = t.rows.get(key);
  if (old) { bp_kept.chars -= key.length + old.v.length;t.rows.delete(key); }
  t.rows.set(key, { v: value, at: Date.now(), s: ++bp_kept.seq });t.seq = bp_kept.seq;bp_kept.chars += key.length + value.length;
  while (t.rows.size > BP_KEPT_KEYS) { const [k, e] = t.rows.entries().next().value;t.rows.delete(k);bp_kept.chars -= k.length + e.v.length; }
  bp_kept.threads.set(thread, t);kept_trim();
  return '';
}
function viewerkept_drop(thread, key) {
  bp_kept.ahead.delete(thread + '\n' + key);
  const t = bp_kept.threads.get(thread), e = t && t.rows.get(key);
  if (e) { t.rows.delete(key);bp_kept.chars -= key.length + e.v.length; }
  return '';
}
// what is kept for a thread working in cwd: keys and values in turn (a Bend
// list), only what changed after since (the "kept.seq:<thread>" a view was
// last given, which comes with it: a view keeps what it was given), all of it
// for a view given nothing yet or a number this process never gave
function kept_list(o) {
  let out = { $: CID(Nil) };
  for (const k of Object.keys(o).reverse()) out = { $: CID(Con), head: k, tail: { $: CID(Con), head: o[k], tail: out } };
  return out;
}
function viewerkept_object(thread, cwd, since) {
  const t = typeof thread === 'string' && thread ? kept_thread(thread, cwd, false) : undefined;
  if (!t || !t.rows.size) return {};
  let n = /^[0-9]+$/.test(since) ? Number(since) : 0;
  if (n > bp_kept.seq) n = 0;
  if (n >= t.seq) return {};
  const o = {};for (const [k, e] of t.rows) if (e.s > n) o[k] = e.v;
  o['kept.seq:' + thread] = String(t.seq);
  return o;
}
function viewerkept_info(thread, cwd, since) { return kept_list(viewerkept_object(thread, cwd, since)); }
// "1": nothing kept under key for a thread working in cwd (or kept longer
// ago than maxMs), and no read of it on its way: the caller reads it now
// (a read on its way for 15 s at most)
function viewerkept_claim(thread, cwd, key, maxMs) {
  if (typeof thread !== 'string' || !thread || typeof key !== 'string') return '';
  const t = kept_thread(thread, cwd, false), e = t && t.rows.get(key), now = Date.now(), wait = thread + '\n' + key;
  if (e && now - e.at < (Number(maxMs) || 0)) return '';
  const since = bp_kept.ahead.get(wait);
  if (since !== undefined && now - since < BP_KEPT_AHEAD_MS) return '';
  bp_kept.ahead.set(wait, now);
  if (bp_kept.ahead.size > 4096) bp_kept.ahead.delete(bp_kept.ahead.keys().next().value);
  return '1';
}
io_eff(CID(ViewerKept.put), viewerkept_put);
io_eff(CID(ViewerKept.drop), viewerkept_drop);
io_eff(CID(ViewerKept.info), viewerkept_info);
io_eff(CID(ViewerKept.claim), viewerkept_claim);
