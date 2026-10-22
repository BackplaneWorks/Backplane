// Reconnecting to the hub: when to try again, and what wakes a wait early.
// Platform IO only; no product decisions. A hub restart (a deploy) refuses
// connections for a while, a sleeping laptop or a phone's browser drops its
// socket without a close, and many pages return at once after a restart, so
// the delay grows, is jittered, and is cut short by the network coming back
// or the page being looked at again.

export const BASE_MS = 250;
export const CAP_MS = 2000;
export const RETRY_AFTER_CAP_MS = 30000;
// A link that is back within this long never shows "Reconnecting".
export const QUIET_MS = 1200;

export class Backoff {
  constructor({ base = BASE_MS, cap = CAP_MS, random = Math.random } = {}) {
    this.base = base;this.cap = cap;this.random = random;this.tries = 0;this.after = 0;
  }
  // The next delay: an exponential step with "equal jitter" (half fixed, half
  // random), so a crowd of clients spreads out but none waits near zero.
  next() {
    const step = Math.min(this.cap, this.base * 2 ** Math.min(this.tries, 16));
    this.tries += 1;
    const jittered = Math.round(step / 2 + this.random() * step / 2);
    const after = this.after;this.after = 0;
    return Math.max(jittered, after);
  }
  // A hub that says how long it needs (503 + Retry-After while it starts).
  retryAfter(ms) {
    if (Number.isFinite(ms) && ms > 0) this.after = Math.min(RETRY_AFTER_CAP_MS, Math.round(ms));
  }
  reset() { this.tries = 0;this.after = 0; }
}

// Retry-After as sent by a hub: seconds, or an HTTP date.
export function retryAfterMs(header, now = Date.now()) {
  if (header == null || header === "") return 0;
  const s = Number(header);
  if (Number.isFinite(s)) return Math.max(0, s * 1000);
  const at = Date.parse(header);
  return Number.isFinite(at) ? Math.max(0, at - now) : 0;
}

// One pending retry at a time. `wake()` runs it now (the network came back,
// the page became visible); `cancel()` drops it (a connection is up).
export class Retry {
  constructor({ setTimer = (f, ms) => setTimeout(f, ms), clearTimer = id => clearTimeout(id), now = () => Date.now() } = {}) {
    this.setTimer = setTimer;this.clearTimer = clearTimer;this.now = now;
    this.timer = null;this.run = null;this.due = 0;
  }
  get pending() { return this.run !== null; }
  schedule(ms, run) {
    this.cancel();
    this.run = run;this.due = this.now() + ms;
    this.timer = this.setTimer(() => this.fire(), ms);
  }
  fire() {
    const run = this.run;
    this.cancel();
    if (run) run();
  }
  wake() { if (this.run) this.fire(); }
  cancel() {
    if (this.timer !== null) this.clearTimer(this.timer);
    this.timer = null;this.run = null;this.due = 0;
  }
}

// What tells a page to try at once: the network is back, the tab is shown
// again, the page came back from the back/forward cache, the window is
// focused. Returns a function that removes the listeners.
export function watchWake(target, doc, wake) {
  const shown = () => { if (doc.visibilityState !== "hidden") wake("visible"); };
  const online = () => wake("online");
  const pageshow = () => wake("pageshow");
  const focus = () => wake("focus");
  target.addEventListener("online", online);
  target.addEventListener("pageshow", pageshow);
  target.addEventListener("focus", focus);
  doc.addEventListener("visibilitychange", shown);
  return () => {
    target.removeEventListener("online", online);
    target.removeEventListener("pageshow", pageshow);
    target.removeEventListener("focus", focus);
    doc.removeEventListener("visibilitychange", shown);
  };
}

// The hub's state as /hello tells it: up, starting (503, maybe with
// Retry-After), or unreachable (refused, a timeout). Any 2xx counts as up.
export async function probeHub(fetch, { timeoutMs = 2000, now = Date.now } = {}) {
  const abort = new AbortController();let timer;
  try {
    return await Promise.race([
      (async () => {
        const r = await fetch("/hello", { cache: "no-store", signal: abort.signal });
        if (r.status >= 200 && r.status < 300) return { state: "up" };
        if (r.status === 503) return { state: "starting", retryAfterMs: retryAfterMs(r.headers?.get("retry-after"), now()), note: await startingNote(r) };
        return { state: "down" };
      })().catch(() => ({ state: "down" })),
      new Promise(resolve => { timer = setTimeout(() => { abort.abort();resolve({ state: "down" }); }, timeoutMs); }),
    ]);
  } finally { clearTimeout(timer); }
}

// What a starting hub says it is doing ({"status":"starting","note":"building
// store 42%"}), as a sentence for the person; "" when it says nothing usable.
export async function startingNote(response) {
  try {
    const o = JSON.parse((await response.text()).slice(0, 4096));
    return startingText(o);
  } catch { return ""; }
}
export function startingText(o) {
  const note = typeof o?.note === "string" ? o.note.slice(0, 200) : "";
  const m = /^building store\s+(\d{1,3})%$/i.exec(note.trim());
  if (m) return `Building the database… ${Math.min(100, Number(m[1]))}%`;
  if (o?.status === "starting") return note ? `Hub starting: ${note}` : "Hub starting…";
  return "";
}

// The bundle a page runs, and the one index.html names now: a deploy changed
// the web client when they differ. Only `chunk-<hash>.js` names count.
export function bundleOf(url) {
  const m = /\/(chunk-[A-Za-z0-9_-]+\.js)(?:[?#].*)?$/.exec(String(url || ""));
  return m ? m[1] : "";
}
export function bundleIn(html) {
  const m = /<script[^>]*\bsrc="\.?\/?(chunk-[A-Za-z0-9_-]+\.js)"/.exec(String(html || ""));
  return m ? m[1] : "";
}
export function deployed(running, html) {
  const now = bundleIn(html);
  return !!running && !!now && now !== running;
}
