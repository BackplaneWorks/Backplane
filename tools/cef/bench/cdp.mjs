// Automation on the page the person sees, over CEF's CDP endpoint, and the
// cost of a remote watcher's JPEG screencast. Node (Playwright's ws transport
// does not connect under Bun 1.3; see README):
//   node tools/cef/bench/cdp.mjs PORT STATIC_URL ANIM_URL SECS
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium } = require("../../browser/node_modules/playwright-core");
const [port, STATIC, ANIM, secs] = process.argv.slice(2);
const say = (o) => process.stdout.write(JSON.stringify(o) + "\n");
const cdp = { ok: false };
try {
  const tc = Date.now();
  const b = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  cdp.connectMs = Date.now() - tc;
  const pages = b.contexts().flatMap((x) => x.pages());
  cdp.pages = pages.length;
  const pg = pages[0];
  await pg.goto(STATIC);
  cdp.title = await pg.title();
  cdp.evaluate = await pg.evaluate(() => document.querySelectorAll("p").length);
  await pg.locator("input").fill("hello");
  cdp.filled = await pg.locator("input").inputValue();
  await pg.locator("role=button[name='Go']").click();
  cdp.aria = (await pg.locator("body").ariaSnapshot()).split("\n").length;
  cdp.screenshotBytes = (await pg.screenshot()).length;
  await pg.goto(ANIM);
  const s = await pg.context().newCDPSession(pg);
  let frames = 0;
  s.on("Page.screencastFrame", (e) => { frames++; s.send("Page.screencastFrameAck", { sessionId: e.sessionId }).catch(() => {}); });
  await s.send("Page.startScreencast", { format: "jpeg", quality: 70, maxWidth: 1280, maxHeight: 2048, everyNthFrame: 1 });
  await new Promise((r) => setTimeout(r, 1000));
  say({ casting: true });
  const f0 = frames;
  await new Promise((r) => setTimeout(r, Number(secs) * 1000));
  cdp.castFps = +((frames - f0) / Number(secs)).toFixed(1);
  await s.send("Page.stopScreencast");
  cdp.ok = true;
  // no b.close(): over CDP it closes the host's browser; exiting disconnects
} catch (e) { cdp.error = String(e).split("\n")[0]; }
// stdout to a pipe is asynchronous in Node: exit once the line is out
process.stdout.write(JSON.stringify({ cdp }) + "\n", () => process.exit(0));
