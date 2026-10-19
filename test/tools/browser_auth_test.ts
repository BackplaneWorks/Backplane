import { strict as assert } from "node:assert";
import { pairBrowser } from "../../src/web/auth.js";
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
assert(!host.includes('q.set("token"'));
assert(!host.includes("document.cookie ="));
assert(!host.includes('setItem("backplane-token"'));
assert(host.indexOf("const pairingReady") < host.indexOf("App.wanted"));
console.log("browser pairing and legacy migration pass");
