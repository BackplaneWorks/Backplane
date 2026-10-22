// The web client's reconnect pieces (src/web/reconnect.js): backoff with
// jitter and a cap, Retry-After from a starting hub, one pending retry that
// a wake runs at once, and telling a deploy from the bundle names.
//   bun test/tools/reconnect_test.ts
import { Backoff, Retry, retryAfterMs, bundleOf, bundleIn, deployed, probeHub, startingText, CAP_MS, RETRY_AFTER_CAP_MS } from "../../src/web/reconnect.js";
import { pairBrowser } from "../../src/web/auth.js";

let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };

// backoff: equal jitter between half and all of an exponential step, capped
{
  const lo = new Backoff({ random: () => 0 }), hi = new Backoff({ random: () => 0.999999 });
  const a = Array.from({ length: 10 }, () => lo.next()), b = Array.from({ length: 10 }, () => hi.next());
  check(a[0] === 125 && b[0] === 250, `first delay 125..250 (${a[0]}, ${b[0]})`);
  check(a.every((x, i) => i === 0 || x >= a[i - 1]), "delays never shrink");
  check(Math.max(...b) === CAP_MS && Math.max(...a) === CAP_MS / 2, `capped at ${CAP_MS} (${Math.max(...b)})`);
  check(a.every(x => x > 0), "never zero: a crowd never returns at once");
  lo.reset();check(lo.next() === 125, "reset starts over");
  const r = new Backoff({ random: () => 0 });r.retryAfter(8000);
  check(r.next() === 8000 && r.next() === 250, "Retry-After is honoured once, then the schedule goes on");
  r.retryAfter(10 ** 9);check(r.next() === RETRY_AFTER_CAP_MS, "Retry-After is capped");
  // spread: 1000 clients' first retry after a restart covers the window
  const firsts = Array.from({ length: 1000 }, () => new Backoff().next());
  check(Math.min(...firsts) >= 125 && Math.max(...firsts) <= 250 && new Set(firsts).size > 50, "first retries spread over 125..250 ms");
}
check(retryAfterMs("3") === 3000 && retryAfterMs("") === 0 && retryAfterMs(null) === 0 && retryAfterMs("x") === 0, "Retry-After seconds");
check(retryAfterMs(new Date(10_000).toUTCString(), 4_000) === 6000, "Retry-After as a date");

// retry: one pending at a time; wake runs it now; cancel drops it
{
  const timers = new Map<number, () => void>();let n = 0;
  const r = new Retry({ setTimer: (f: () => void) => { timers.set(++n, f);return n; }, clearTimer: (id: number) => timers.delete(id) });
  let runs = 0;
  r.schedule(1000, () => runs++);r.schedule(1000, () => runs++);
  check(timers.size === 1 && r.pending, "a second schedule replaces the first");
  r.wake();check(runs === 1 && !r.pending && timers.size === 0, "wake runs it once, now");
  r.wake();check(runs === 1, "wake with nothing pending does nothing");
  r.schedule(1000, () => runs++);r.cancel();check(!r.pending && timers.size === 0 && runs === 1, "cancel drops it");
  r.schedule(1000, () => runs++);[...timers.values()][0]();check(runs === 2 && !r.pending, "the timer runs it");
}

// deploys: the running chunk against the one index.html names now
const index = (c: string) => `<link rel="stylesheet" crossorigin href="./chunk-eqswhk46.css"><script type="module" crossorigin src="./${c}"></script>`;
check(bundleOf("http://h:3787/chunk-rzyzmezk.js") === "chunk-rzyzmezk.js", "bundle of the running module");
check(bundleOf("http://h:3787/host.js") === "", "an unbundled dev page has no bundle (never reloads)");
check(bundleIn(index("chunk-rzyzmezk.js")) === "chunk-rzyzmezk.js", "bundle named by index.html");
check(!deployed("chunk-rzyzmezk.js", index("chunk-rzyzmezk.js")), "same build: no deploy");
check(deployed("chunk-rzyzmezk.js", index("chunk-new12345.js")), "new chunk: a deploy");
check(!deployed("chunk-rzyzmezk.js", "<html>offline</html>") && !deployed("", index("chunk-x.js")), "no evidence: no deploy");

// the hub's state from /hello
const res = (status: number, h: Record<string, string> = {}) => async () => ({ ok: status < 300, status, headers: new Headers(h) });
check((await probeHub(res(200) as any)).state === "up", "200: up");
const st = await probeHub(res(503, { "retry-after": "2" }) as any);
check(st.state === "starting" && st.retryAfterMs === 2000, "503 + Retry-After: starting, 2 s");
check((await probeHub((async () => { throw new TypeError("refused"); }) as any)).state === "down", "refused: down");
check((await probeHub((() => new Promise(() => {})) as any, { timeoutMs: 50 })).state === "down", "no answer: down");

// the service worker keeps one build's assets
{
  const src = await Bun.file(new URL("../../src/web/sw.js", import.meta.url)).text();
  const deleted: string[] = [];
  const keys = ["/chunk-rzyzmezk.js", "/chunk-old00000.js", "/chunk-eqswhk46.css", "/chunk-old11111.css", "/favicon-7jbpzm5e.svg", "/favicon-oldoldol.svg", "/"].map(p => ({ url: "http://h" + p }));
  const c = { keys: async () => keys, delete: async (r: any) => { deleted.push(new URL(r.url).pathname); } };
  const self = { addEventListener() {} };
  const prune = new Function("self", "caches", "location", src + "\nreturn prune;")(self, {}, { origin: "http://h" });
  await prune(c, index("chunk-rzyzmezk.js") + '<link rel="icon" href="./favicon-7jbpzm5e.svg">');
  check(deleted.sort().join() === "/chunk-old00000.js,/chunk-old11111.css,/favicon-oldoldol.svg", `old build's assets deleted (${deleted})`);
  deleted.length = 0;await prune(c, "<html>502 Bad Gateway</html>");
  check(deleted.length === 0, "a page naming no chunk prunes nothing");
}
// a hub that is starting (building its store): its note, as the person reads it
check(startingText({ status: "starting", note: "building store 42%" }) === "Building the database… 42%", "store build note");
check(startingText({ status: "starting", note: "replaying" }) === "Hub starting: replaying" && startingText({ status: "starting" }) === "Hub starting…", "other starting notes");
check(startingText({}) === "" && startingText(null) === "", "no note");
const starting503 = async () => ({ ok: false, status: 503, headers: new Headers({ "retry-after": "2" }), text: async () => '{"status":"starting","note":"building store 7%"}' });
const sp = await probeHub(starting503 as any);
check(sp.state === "starting" && sp.retryAfterMs === 2000 && sp.note === "Building the database… 7%", "probeHub carries the starting note");

// pairing while the hub starts: the link's token is kept, the wait says why, then it pairs
{
  const store = new Map<string, string>(), waits: string[] = [], slept: number[] = [];
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) };
  const tok = "0123456789abcdef0123456789abcdef";
  let calls = 0, keptDuring = false;
  const fetch = async () => { calls++; keptDuring ||= store.get("backplane-token") === tok; if (calls <= 2) return { ok: false, status: 503, headers: new Headers({ "retry-after": "2" }), text: async () => '{"status":"starting","note":"building store 50%"}' }; return { ok: true, status: 200, headers: new Headers() }; };
  const ok = await pairBrowser({ location: { hash: "#token=" + tok, pathname: "/", search: "" }, history: { replaceState() {} }, storage, fetch, onWait: (n: string) => waits.push(n), sleep: async (ms: number) => { slept.push(ms); } } as any);
  check(ok === true && calls === 3, "pairing retries through a starting hub and pairs");
  check(keptDuring && !store.has("backplane-token"), "the token is kept while waiting, dropped once paired");
  check(waits[0] === "Building the database… 50%" && slept.every((ms) => ms === 2000), "the wait says why and follows Retry-After");
  const refused = await pairBrowser({ location: { hash: "#token=" + tok, pathname: "/", search: "" }, history: { replaceState() {} }, storage, fetch: async () => ({ ok: false, status: 401, headers: new Headers() }), sleep: async () => {} } as any);
  check(refused === false && !store.has("backplane-token"), "a refused link fails at once and is not kept");
  let n2 = 0;
  const flaky = await pairBrowser({ location: { hash: "#token=" + tok, pathname: "/", search: "" }, history: { replaceState() {} }, storage, fetch: async () => { if (++n2 === 1) throw new TypeError("refused"); return { ok: true, status: 200, headers: new Headers() }; }, sleep: async () => {} } as any);
  check(flaky === true && n2 === 2, "a hub not up yet is asked again");
}

// the service worker's waiting page for a starting hub
{
  const src = await Bun.file(new URL("../../src/web/sw.js", import.meta.url)).text();
  const starting = new Function("self", "caches", "location", src + "\nreturn starting;")({ addEventListener() {} }, {}, { origin: "http://h" });
  const res = await starting(new Response('{"status":"starting","note":"building store 63%"}', { status: 503, headers: { "retry-after": "2" } }));
  const html = await res.text();
  check(res.status === 503 && res.headers.get("content-type")?.startsWith("text/html"), "a page, not the raw answer");
  check(html.includes("Building the database… 63%") && html.includes('http-equiv="refresh" content="2"'), "it says the progress and loads again after Retry-After");
}
if (fail) { console.log(`${fail} failed`);process.exit(1); }
console.log("all ok");
