// End to end: the web client in headless Chrome against a headless hub (the JS
// constructor tags of Bend 2.0.36 are module-qualified, so this exercises wire.js,
// host.js and the view code for real). A project and a thread are made over the
// socket, a stand-in agent answers, and the page must show the sidebar, the
// thread, and a message typed in its own composer, with no console errors.
//
//   bun test/tools/web_e2e.ts [BINARY] [WIREDIR] [CHROME]
//
// BINARY defaults to build/backplane (its web client is read from web/ beside
// it: dist/ after scripts/build.sh, or bend src/web/index.html -o <dir>/web);
// WIREDIR to build/wire; CHROME to /usr/bin/google-chrome. WEB_SHOT=file.png
// saves a screenshot. Needs tools/browser's node_modules (bun install there).
// Prints "ok ..." / "FAIL ..." lines.
import { chromium } from "../../tools/browser/node_modules/playwright-core/index.mjs";
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [bin = "build/backplane", wire = "build/wire", chrome = "/usr/bin/google-chrome"] = process.argv.slice(2);
const WT = resolve(new URL("../..", import.meta.url).pathname);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = mkdtempSync(join(tmpdir(), "bp-web-e2e-"));
const fake = join(root, "bin"); mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${WT}/test/tools/claude_standin.ts "$@"\n`);
for (const n of ["codex", "grok", "muse", "omp"]) writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
for (const n of ["claude", "codex", "grok", "muse", "omp"]) chmodSync(join(fake, n), 0o755);
const user = join(root, "user"); mkdirSync(join(user, "proj"), { recursive: true });
const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const port = s.port; s.stop(true);
const proc = Bun.spawn([resolve(bin), "--home", join(root, "home"), "--port", String(port), "--no-tailscale", "--headless", "--foreground"], {
  env: { ...process.env, HOME: user, DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BP_STANDIN_MODE: "plain", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore", stderr: "ignore" });
console.log("hub pid", proc.pid, "port", port);
let fail = false; const check = (n: string, ok: boolean, g?: unknown) => { console.log(ok ? `ok ${n}` : `FAIL ${n}: ${JSON.stringify(g)?.slice(0, 300)}`); if (!ok) fail = true; };
let browser: any;
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {} await sleep(100); }
  const errors: string[] = [];
  browser = await chromium.launch({ executablePath: chrome, headless: true, args: ["--no-sandbox"] });
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  page.on("console", (m: any) => { if (m.type() === "error") errors.push("console: " + m.text()); });
  page.on("pageerror", (e: any) => errors.push("pageerror: " + e.message));
  await page.goto(`http://127.0.0.1:${port}/`);
  await sleep(2500);
  check("page loaded: body has text", (await page.innerText("body")).length > 0);
  // drive the hub from a second client
  const replies = new Map<number, any>(); const evs: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => { const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))); for (const c of o.items ?? []) evs.push(c); if (o.t === "reply") replies.set(Number(o.id), o); };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; }); await sleep(300);
  let n = 0; const rpc = async (m: string, p: object) => { const id = ++n; ws.send(W.encode(JSON.stringify({ id, m, p }))); for (let i = 0; i < 100 && !replies.has(id); i++) await sleep(100); return replies.get(id); };
  const r1 = await rpc("project.new", { path: join(user, "widget-board") });
  check("project created over rpc", r1?.ok === true, r1);
  const th = r1?.thread;
  await sleep(1500);
  const body1 = await page.innerText("body");
  check("page sidebar shows the project", /widget-board/i.test(body1), body1.slice(0, 300));
  const send = async (id: string, text: string) => { ws.send(W.encode(JSON.stringify({ id: ++n, m: "turn.start", p: { thread: id, text, msg: "m-" + n } }))); };
  await send(th, "hello stand-in agent");
  await sleep(3000);
  // open the thread in the page and send a message through the page's own composer
  await page.keyboard.press("Escape"); await sleep(500);
  await page.locator("text=hello stand-in agent").first().click().catch(() => {});
  await sleep(800);
  const ta = page.locator("textarea").first();
  if (await ta.count()) {
    await ta.fill("typed in the page");
    await ta.press("Enter");
    await sleep(3000);
  }
  const body2 = await page.innerText("body");
  check("thread shows the sent message", /hello stand-in agent/.test(body2), body2.slice(0, 400));
  check("thread shows the message typed in the page", /typed in the page/.test(body2), body2.slice(-400));
  check("thread shows an agent answer", evs.filter((c) => c.$ === "MessagePosted").length >= 2, evs.length);
  if (process.env.WEB_SHOT) await page.screenshot({ path: process.env.WEB_SHOT });
  check("no console errors", errors.length === 0, errors);
  if (errors.length) console.log(errors.join("\n"));
  ws.close();
} catch (e) { console.log("ERROR", e); fail = true; }
finally { try { await browser?.close(); } catch {} proc.kill(); rmSync(root, { recursive: true, force: true }); }
process.exit(fail ? 1 : 0);
