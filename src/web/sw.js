// Backplane's service worker: the app shell from cache, so a reload on a
// poor connection shows the page and its read-only focused picture. Hashed
// chunks never change under their name (cache first); the page itself is
// fetched fresh when the network answers, else served from cache. The
// socket and MCP never pass through here. Each fresh page names the assets
// of its build; cached assets it no longer names (an earlier deploy's) are
// deleted then, so the cache holds one build, not one per deploy.

const SHELL = "backplane-shell-v1";

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));

// a notification the page showed through here (a phone's, host.js's
// notify): a tap brings the page forward and opens its thread there
self.addEventListener("notificationclick", (e) => {
  e.notification.close();
  const thread = e.notification.data?.thread || "";
  e.waitUntil(self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((cs) => {
    const c = cs.find((w) => w.focused) || cs[0];
    if (!c) return self.clients.openWindow("/");
    c.postMessage({ t: "bp-open", thread });
    return c.focus();
  }));
});

self.addEventListener("fetch", (e) => {
  const url = new URL(e.request.url);
  if (e.request.method !== "GET" || url.origin !== location.origin) return;
  if (e.request.cache === "no-store") return;
  const page = url.pathname === "/" || url.pathname === "/index.html";
  const asset = /^\/(?:chunk-|favicon-|apple-touch-icon-)[a-zA-Z0-9_-]+\.(?:js|css|svg|png|woff2?)$/.test(url.pathname);
  // Auth, capability, usage, file and image API results are never app shell.
  // Do not turn a failed API request into a cached response or index HTML.
  if (!page && !asset) return;
  if (url.pathname.startsWith("/chunk-")) {
    e.respondWith(caches.open(SHELL).then(async (c) => {
      const hit = await c.match(e.request);
      if (hit) return hit;
      const r = await fetch(e.request);
      if (r.ok) c.put(e.request, r.clone());
      return r;
    }));
    return;
  }
  e.respondWith(caches.open(SHELL).then(async (c) => {
    const net = fetch(e.request).then((r) => {
      if (r.ok) {
        c.put(e.request, r.clone());
        if (page) e.waitUntil(r.clone().text().then((html) => prune(c, html)).catch(() => {}));
      }
      return r;
    });
    const cached = async () => (await c.match(e.request)) ?? (page ? await c.match("/") : null);
    if (page) {
      // A page on a link that does not answer (a phone's weak signal, a
      // hub address that times out) is the cached one after PAGE_WAIT_MS;
      // the fresh page is still saved for the next open, and a page that
      // runs an earlier build reloads into the new one (host.js checkDeploy).
      let timer;
      const late = new Promise((resolve) => { timer = setTimeout(resolve, PAGE_WAIT_MS, null); });
      const r = await Promise.race([net.catch(() => null), late]);
      clearTimeout(timer);
      if (r) return r;
      const hit = await cached();
      if (hit) { e.waitUntil(net.catch(() => {})); return hit; }
    }
    try { return await net; } catch { return (await cached()) ?? Response.error(); }
  }));
});

const PAGE_WAIT_MS = 2500;

const ASSET = /^\/(?:chunk-|favicon-|apple-touch-icon-)[a-zA-Z0-9_-]+\.(?:js|css|svg|png|woff2?)$/;

// Keep only the hashed assets this page's build names. A page that names
// none (an error page, a proxy's) leaves the cache alone.
async function prune(c, html) {
  const named = new Set();
  for (const m of html.matchAll(/(?:src|href)="\.?\/?((?:chunk-|favicon-|apple-touch-icon-)[a-zA-Z0-9_-]+\.[a-z0-9]+)"/g)) named.add("/" + m[1]);
  if (![...named].some((n) => n.startsWith("/chunk-"))) return;
  for (const req of await c.keys()) {
    const path = new URL(req.url).pathname;
    if (ASSET.test(path) && !named.has(path)) await c.delete(req);
  }
}
