// Playwright connectOverCDP from Bun through BunWs, against a running backplane-cef:
//   bun tools/cef/bench/bunws_check.ts PORT
// needs a playwright-core copy at ./pw whose coreBundle.js reads globalThis.__bpWs:
//   ws = globalThis.__bpWs ?? require("./utilsBundle").ws;
import { BunWs } from "./bunws.ts";
(globalThis as any).__bpWs = BunWs;
const { chromium } = await import("./pw/index.mjs");
const port = process.argv[2];
const t = Date.now();
const b = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const pg = b.contexts()[0].pages()[0];
await pg.goto("data:text/html,<input><button>Go</button><p>x</p>");
await pg.locator("input").fill("hi");
console.log(JSON.stringify({ ms: Date.now() - t, pages: b.contexts()[0].pages().length, val: await pg.locator("input").inputValue(), aria: await pg.locator("body").ariaSnapshot() }));
process.exit(0);
