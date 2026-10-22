import { strict as assert } from "node:assert";
import { pairBrowser } from "../../src/web/auth.js";
import { refreshCacheIdentity } from "../../src/web/cache-identity.js";
const secret = "0123456789abcdef0123456789abcdef";
function fixture(hash = "", legacy: string | null = null, ok = true) {
  const removed: string[] = [], requests: any[] = [], replaced: string[] = [];
  return { removed, requests, replaced, args: {
    location: { hash, pathname: "/", search: "?lang=ja" },
    history: { replaceState: (_a: any, _b: any, url: string) => replaced.push(url) },
    storage: { getItem: () => legacy, removeItem: (key: string) => removed.push(key) },
    fetch: async (url: string, options: any) => { requests.push({ url, options }); return { ok }; },
  }};
}
const fresh = fixture(`#token=${secret}`);
assert.equal(await pairBrowser(fresh.args), true);
assert.deepEqual(fresh.replaced, ["/?lang=ja"]);
assert.equal(fresh.requests[0].url, "/auth/pair");
assert.equal(fresh.requests[0].options.headers.Authorization, `Bearer ${secret}`);
assert.equal(fresh.requests[0].options.credentials, "same-origin");
assert.deepEqual(fresh.removed, ["backplane-token"]);
const migrate = fixture("", secret);
assert.equal(await pairBrowser(migrate.args), true);
assert.deepEqual(migrate.removed, ["backplane-token"]);
const failed = fixture("", secret, false);
assert.equal(await pairBrowser(failed.args), false);
assert.deepEqual(failed.removed, []);
const malformed = fixture("#token=short");
assert.equal(await pairBrowser(malformed.args), false);
assert.equal(malformed.requests.length, 0);
const bot = fixture("#bot=Bub");
assert.equal(await pairBrowser(bot.args), true);
assert.equal(bot.replaced.length, 0);
assert.equal(bot.requests.length, 0);
const host = await Bun.file("src/web/host.js").text();
const identity = await Bun.file("src/web/cache-identity.js").text();
assert(!host.includes('q.set("token"'));
assert(!host.includes('setItem("backplane-token"'));
assert(host.indexOf("const pairingReady") < host.indexOf("App.wanted"));

// bp_cache_v1 is a public picture partition, never the HttpOnly bp_token credential.
// Permit only the host's refresh adapter and the helper's fixed cookie name.
function assertCacheCookieBoundary(hostSource: string, identitySource: string) {
  const adapter = "writeCookie: value => { document.cookie = value; }";
  const cookieWrite = /\bdocument\s*(?:\.\s*cookie|\[\s*["']cookie["']\s*\])\s*(?:[+&|?]*=|\+\+|--)/;
  assert.equal(hostSource.split(adapter).length - 1, 1, "only the cache refresh adapter may write cookies");
  assert.doesNotMatch(hostSource.replace(adapter, ""), cookieWrite);
  assert.doesNotMatch(identitySource, cookieWrite);
  assert.equal(identitySource.match(/\bwriteCookie\b/g)?.length, 2, "one injected sink and one fixed-name call");
  assert.match(identitySource, /\bwriteCookie\(`bp_cache_v1=\$\{key\};/);
}
assertCacheCookieBoundary(host, identity);
assert.throws(() => assertCacheCookieBoundary(host + '\ndocument.cookie = "bp_token=secret";', identity));
assert.throws(() => assertCacheCookieBoundary(host + '\ndocument["cookie"] = "token=secret";', identity));
assert.throws(() => assertCacheCookieBoundary(host, identity.replace("`bp_cache_v1=", "`bp_token=")));
assert.throws(() => assertCacheCookieBoundary(host, identity + '\nwriteCookie("bp_token=secret");'));

async function cacheRefresh(status: number, body: object, protocol = "https:") {
  let cookie = "";
  const writes: string[] = [];
  const result = await refreshCacheIdentity({
    fetch: async (url: string, options: any) => {
      assert.match(url, /^\/auth\/cache\?bpProbe=[0-9a-f]{32}$/);
      assert.equal(options.credentials, "same-origin");
      return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
    },
    readCookie: () => cookie,
    writeCookie: (value: string) => {
      assert.match(value, /^bp_cache_v1=(?:[0-9a-f]{64})?; Path=\/; SameSite=Strict; Max-Age=(?:0|31536000)(?:; Secure)?$/);
      assert(!value.includes(secret), "pairing material must never enter a JS-written cookie");
      writes.push(value);cookie = value.split(";")[0];
    },
    protocol,
  });
  return { result, writes };
}
const cacheKey = "c".repeat(64);
for (const protocol of ["http:", "https:"]) {
  const cache = await cacheRefresh(200, { version: 1, cacheKey }, protocol);
  assert.equal(cache.result.state, "verified");
  assert.deepEqual(cache.writes, [`bp_cache_v1=${cacheKey}; Path=/; SameSite=Strict; Max-Age=31536000${protocol === "https:" ? "; Secure" : ""}`]);
}
for (const status of [401, 403]) {
  const cache = await cacheRefresh(status, {});
  assert.equal(cache.result.state, "denied");
  assert.deepEqual(cache.writes, ["bp_cache_v1=; Path=/; SameSite=Strict; Max-Age=0; Secure"]);
}
for (const body of [{ version: 1, cacheKey: secret }, { version: 1, cacheKey, token: secret }]) {
  const cache = await cacheRefresh(200, body);
  assert.equal(cache.result.state, "invalid");
  assert.deepEqual(cache.writes, ["bp_cache_v1=; Path=/; SameSite=Strict; Max-Age=0; Secure"]);
}
console.log("browser pairing and legacy migration pass");
