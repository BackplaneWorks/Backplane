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
    try {
      const r = await fetch(e.request);
      // a hub that is starting (building its store) answers 503 with a note:
      // a page that says so and loads again, not the raw answer
      if (page && r.status === 503) return starting(r);
      if (r.ok) {
        c.put(e.request, r.clone());
        if (page) e.waitUntil(r.clone().text().then((html) => prune(c, html)).catch(() => {}));
      }
      return r;
    } catch {
      return (await c.match(e.request)) ?? (page ? await c.match("/") : null) ?? Response.error();
    }
  }));
});

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

// The waiting page for a hub that is starting: its note ("building store
// 42%" reads "Building the database… 42%"), and a reload after Retry-After
// (the address, a pairing link's token included, is kept).
async function starting(r) {
  let note = "";
  try { const o = JSON.parse((await r.clone().text()).slice(0, 4096)); note = typeof o?.note === "string" ? o.note.slice(0, 200) : ""; } catch {}
  const m = /^building store\s+(\d{1,3})%$/i.exec(note.trim());
  const text = m ? `Building the database… ${Math.min(100, Number(m[1]))}%` : note ? `Backplane is starting: ${note}` : "Backplane is starting…";
  const after = Math.max(1, Math.min(30, Number(r.headers.get("retry-after")) || 2));
  const esc = (t) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<meta http-equiv="refresh" content="${after}"><title>Backplane</title>
<style>html,body{margin:0;height:100%;font:15px/1.5 system-ui,sans-serif;background:#fafaf8;color:#1b1d21}@media (prefers-color-scheme:dark){html,body{background:#111410;color:#e6e6e3}}
main{display:flex;height:100%;align-items:center;justify-content:center;flex-direction:column;gap:6px}p{margin:0}.dim{opacity:.6;font-size:13px}</style></head>
<body><main><p>${esc(text)}</p><p class="dim">This page loads again by itself.</p></main></body></html>`;
  return new Response(html, { status: 503, headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store", "retry-after": String(after) } });
}
