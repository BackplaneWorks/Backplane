// Imported before playwright-core (main.ts's first import, which then loads
// playwright-core dynamically). When running from source, make playwright-core
// pick the ws client from bunws.ts when cef mode asks (see there). A runtime
// onLoad plugin cannot do it: Bun then loads coreBundle.js as an async module
// that playwright-core's index.js cannot require. Instead the require cache
// entry of lib/utilsBundle.js (whose `ws` is a read-only getter) is replaced
// by a proxy answering `ws` with BunWs-or-stock. The compiled binary has the
// source transform built in (build.ts).
import { createRequire } from "node:module";
import { wsChooser } from "./bunws.ts";

const req = createRequire(import.meta.url);
try {
  const path = req.resolve("playwright-core/lib/utilsBundle.js");
  const real = req(path);
  const choose = wsChooser(real.ws);
  req.cache[path]!.exports = new Proxy(real, { get: (t, k) => (k === "ws" ? choose : (t as any)[k]) });
} catch {
  // bundled (the compiled binary): nothing to patch
}
