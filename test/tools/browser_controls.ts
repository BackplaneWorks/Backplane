// The real native window and CEF, on a private display and an empty home.
// Run only inside bp-admit --mem 6 --cores 2. Requires build/backplane,
// build/wire and the 2a BACKPLANE_CEF runtime. No live hub or display is used.
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { check, until, privateDisplay } from "../../tools/cef/test/harness.ts";

const [binary = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
if (!process.env.BACKPLANE_CEF) throw new Error("set BACKPLANE_CEF to the 2a runtime");
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const d = await privateDisplay();
const home = join(d.root, "home"), fake = join(d.root, "bin"), proj = join(d.root, "project");
for (const dir of [home, fake, proj]) mkdirSync(dir);
console.log(`private display ${d.env.DISPLAY}; empty home ${home}`);
for (const provider of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, provider), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, provider), 0o755);
}
if (process.env.BACKPLANE_BROWSER) symlinkSync(resolve(process.env.BACKPLANE_BROWSER), join(fake, "backplane-browser"));
else {
  writeFileSync(join(fake, "backplane-browser"), `#!/bin/sh\nexec bun '${resolve("tools/browser/src/main.ts")}' "$@"\n`);
  chmodSync(join(fake, "backplane-browser"), 0o755);
}
const listen = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listen.port; listen.stop(true);
const env = { ...d.env, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", WAYLAND_DISPLAY: "", BACKPLANE_CEF_NO_GPU: "1" };
const web = process.env.BROWSER_CONTROLS_WEB;
const app = Bun.spawn([resolve(binary), "--home", home, "--port", String(port), "--no-tailscale", "--foreground", ...(web ? ["--web", resolve(web)] : [])], { env, stdout: Bun.file(join(d.root, "app.out")), stderr: Bun.file(join(d.root, "app.err")) });
let win = 0, ws: WebSocket | undefined;
const info: Record<string, string> = {}, changes: any[] = [], messages: any[] = [], replies = new Map<number, any>();
let seq = 0;
async function run(cmd: string[]) {
  const p = Bun.spawn(cmd, { env, stdout: "pipe", stderr: "inherit" });
  const out = await new Response(p.stdout).text();
  if (await p.exited) throw new Error(cmd.join(" "));
  return out;
}
async function poke(...args: string[]) { await run([join(d.root, "xpoke"), ...args]); await Bun.sleep(200); }
async function select(title: string) {
  // A parent click takes focus back before opening its palette.
  await d.x("click 300 30 1"); await poke("ctrl", "k"); await poke("type", title); await poke("key", "Return"); await Bun.sleep(500);
}
async function rpc(m: string, p: object) {
  const id = ++seq; ws!.send(W.encode(JSON.stringify({ id, m, p })));
  return until(() => replies.get(id), m, 30000);
}
async function mcp(thread: string, name: string, args: object = {}) {
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const r = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: ++seq, method: "tools/call", params: { name, arguments: args } }) });
  return r.json();
}
async function picCount() {
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const result = await (await fetch(`http://127.0.0.1:${port}/debug/perf.json?token=${token}`)).json();
  return Number(result.rows.find((r: any) => r.name === "ui.msg.pic")?.n ?? 0);
}
const text = (r: any) => String(r?.result?.content?.[0]?.text ?? "");
async function children() { return (await d.x(`children ${win}`)).children.filter((c: any) => !c.inputOnly && c.w > 100); }
async function mapped() { return (await children()).find((c: any) => c.mapped); }
async function pageShown() { return until(mapped, "native page mapped", 30000); }
async function evalPage(port: number, path: string, expression: string) {
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const target = targets.find((t: any) => t.url.endsWith(path));
  check("CDP target for shared page exists", target);
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise<void>((r, j) => { socket.onopen = () => r(); socket.onerror = j; });
  try {
    const reply = new Promise<any>((r, j) => {
      const timer = setTimeout(() => j(new Error("CDP evaluate timeout")), 10000);
      socket.onmessage = e => { const o = JSON.parse(String(e.data)); if (o.id === 1) { clearTimeout(timer); r(o.result?.result?.value); } };
    });
    socket.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
    return await reply;
  } finally { socket.close(); }
}
function processTree(pid: number): number[] {
  try {
    const kids = readFileSync(`/proc/${pid}/task/${pid}/children`, "utf8").trim().split(/\s+/).filter(Boolean).map(Number);
    return [pid, ...kids.flatMap(processTree)];
  } catch { return []; }
}
const shotDir = process.env.CEF_SHOTS ?? resolve("build/browser-controls-shots");
mkdirSync(shotDir, { recursive: true });
const logDir = process.env.CEF_LOGS ?? resolve("build/browser-controls-window");
mkdirSync(logDir, { recursive: true });
// one pixel of the screen as rrggbb (ImageMagick), and a loose colour match
let pxN = 0;
async function px(x: number, y: number) {
  const path = join(d.root, `px${++pxN}.png`);
  await run(["import", "-display", d.env.DISPLAY!, "-window", "root", path]);
  const m = /#([0-9A-Fa-f]{6})/.exec(await run(["convert", path, "-crop", `1x1+${Math.round(x)}+${Math.round(y)}`, "+repage", "txt:-"]));
  return m ? m[1].toLowerCase() : "";
}
const near = (a: string, b: string, tol = 24) => a.length === 6 && [0, 2, 4].every(i => Math.abs(parseInt(a.slice(i, i + 2), 16) - parseInt(b.slice(i, i + 2), 16)) <= tol);
async function shot(name: string) {
  const path = join(shotDir, name + ".png"); await run(["import", "-display", d.env.DISPLAY!, "-window", "root", path]); console.log("screenshot " + path);
}
const site = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  const name = new URL(req.url).pathname;
  return new Response(`<!doctype html><title>${name}</title><body style="margin:0;background:#eef4dd;color:#182018;font:24px monospace"><h1 style="margin:20px">Shared ${name}</h1><input id="field" aria-label="Shared input" style="margin:20px;width:340px;height:44px;font:24px monospace"><button id="go" style="height:50px" onclick="document.body.style.background='#f5b25e';document.querySelector('h1').textContent='Clicked '+(++window.count)">Go</button><a id="popup" target="_blank" href="/popup">Popup</a><div style="height:1200px"></div><script>window.count=0</script>`, { headers: { "content-type": "text/html" } });
} });
// Controls are exercised with real parent-window input, not a browser RPC.
const tabs = (page: string) => JSON.parse(info[`browser.tabs.${page}`] ?? '{"tabs":[]}');
async function active(page: string, path: string) {
  return until(() => tabs(page).url?.endsWith(path), `active URL ${path}`, 30000);
}
async function address(url: string, y: number) {
  await d.x(`click 1100 ${y} 1`);
  await poke("ctrl", "a"); await poke("type", url); await shot("typed-address"); await poke("key", "Return");
}
async function webControls(id: string) {
  const { chromium } = await import("../../tools/browser/node_modules/playwright-core/index.mjs");
  const { findChromium } = await import("../../tools/browser/src/chromium.ts");
  const browser = await chromium.launch({executablePath: findChromium() ?? undefined, headless:true});
  try {
    const page = await browser.newPage({viewport:{width:1440,height:900}});
    await page.goto(`http://127.0.0.1:${port}/`);
    await page.locator(`[data-on-click="select"][data-value="${id}"]`).first().click();
    await page.locator('[data-on-click="panel"]').first().click();
    await page.locator('[data-on-click="view-kind"][data-value="browser"]').click();
    const address = page.locator('.browser-address input');
    await address.fill(`127.0.0.1:${site.port}/web-first`); await address.press('Enter');
    await active(id, '/web-first');
    const first = tabs(id).tabs.find((t: any) => t.active).id;
    await page.locator('[data-on-click="br-new"]').click();
    await until(() => tabs(id).tabs.length === 2, 'web new tab');
    await address.fill(`http://127.0.0.1:${site.port}/web-second`);
    await page.locator('[data-on-click="br-go"]').click(); await active(id, '/web-second');
    await page.locator(`[data-on-click="br-select"][data-value="${first}"]`).click();
    await active(id, '/web-first');
    check('web controls open own sites and switch actual tabs', true);
    const second = tabs(id).tabs.find((t: any) => t.id !== first).id;
    await page.locator(`[data-on-click="br-close"][data-value="${second}"]`).click();
    await until(() => tabs(id).tabs.length === 1, 'web close tab');
    for (const width of [1440,390]) {
      await page.setViewportSize({width,height:900});
      await until(async () => await address.isVisible(), 'responsive address visible');
      const box = await page.locator('.browser-address').boundingBox();
      check(`web address fits ${width}px viewport`, box && box.x >= 0 && box.x + box.width <= width + 1, box);
      await page.screenshot({path:join(shotDir, `web-${width}.png`)});
    }
  } finally { await browser.close(); }
}

try {
  await run(["cc", "test/tools/xpoke.c", "-O2", "-o", join(d.root, "xpoke"), "-lX11", "-ldl"]);
  await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch { return false; } }, "app listening", 60000);
  const tree = await run(["xwininfo", "-display", d.env.DISPLAY!, "-root", "-tree"]);
  win = Number(tree.match(/(0x[0-9a-f]+) "Backplane"/)?.[1]); check("native parent exists", win, tree);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => {
    const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data)));
    messages.push(o); if (o.info) Object.assign(info, o.info);
    for (const c of o.items ?? []) changes.push(typeof c === "string" ? JSON.parse(c) : c);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise<void>((r, j) => { ws!.onopen = () => r(); ws!.onerror = j; });
  await until(() => info["browser.engine.live"] === "cef", "native engine", 60000);
  await rpc("setting.set", { key: "onboard.done", value: "1" });
  await rpc("setting.set", { key: "onboard.uber", value: "1" });
  await poke("key", "Escape");
  await rpc("project.add", { path: proj });
  const project = await until(() => changes.find(c => c.$ === "ProjectCreated" && c.root === proj) ?? changes.find(c => c.$ === "ProjectCreated" && c.id !== "scratch"), "project");
  await rpc("thread.create", { project: project.id, title: "zqbrowsercontrols", env: "local" });
  const a = await until(() => changes.find(c => c.$ === "ThreadCreated" && c.title === "zqbrowsercontrols"), "thread");
  if (web) await webControls(a.id);
  else {
  await select("zqbrowsercontrols");
  // Obtain diagnostics before the person takes over; tool calls are refused during takeover.
  const status = JSON.parse(text(await mcp(a.id, "preview_status")));
  await poke("ctrl", "k"); await poke("type", "Viewer: board"); await poke("key", "Return");
  // The regular Viewer header places Browser after Board/Schematic/3D/Mech/Files.
  await d.x("click 1198 28 1"); await Bun.sleep(500);
  await address(`127.0.0.1:${site.port}/alpha`, 150);
  await active(a.id, "/alpha");
  const ca = await pageShown();
  check("address bar opens user's own local site", ca.y >= 176, ca);
  check("toolbar takes over the same owner's page", info[`drive.${a.id}`] === "takeover", info);
  await until(() => tabs(a.id).tabs.find((t: any) => t.active)?.title === "/alpha", "title metadata");
  const first = tabs(a.id).tabs.find((t: any) => t.active);
  check("tab has a stable identity and title", first?.id && first.title === "/alpha", first);
  // Type into the actual native document; keep it while another tab is active.
  await d.x(`click ${ca.x + 160} ${ca.y + 140} 1`);
  for (const key of "retained") await d.x(`key ${key}`);
  await d.x("click 1378 110 1");
  await until(() => tabs(a.id).tabs.length === 2 && tabs(a.id).url === "about:blank", "new blank tab");
  await address(`http://127.0.0.1:${site.port}/beta`, 150);
  await active(a.id, "/beta");
  await address(`http://127.0.0.1:${site.port}/gamma`, 150); await active(a.id, "/gamma");
  await d.x("click 847 150 1"); await active(a.id, "/beta");
  await d.x("click 880 150 1"); await active(a.id, "/gamma");
  await d.x("click 915 150 1"); await active(a.id, "/gamma");
  console.log("ok  back, forward and reload preserve active tab");
  await d.x("click 846 110 1"); // scroll the tab strip back to the first tab
  await d.x("click 910 110 1"); await active(a.id, "/alpha");
  const value = await evalPage(status.port, "/alpha", "document.querySelector('#field').value");
  check("switching tabs preserves the real document draft", value === "retained", value);
  await d.x("click 1235 110 1");
  await until(() => tabs(a.id).tabs.length === 1, "close second tab");
  check("closing another tab keeps original tab identity", tabs(a.id).tabs[0].id === first.id);
  check("stale close id fails without closing a different tab", !(await rpc("bots.browser", {bot:a.id, op:"toolbar", command:"tabs", closeId:"gone-tab"})).ok);
  check("unknown owner still refused", !(await rpc("bots.browser", {bot:"invented", op:"toolbar", command:"tabs", new:true})).ok);
  for (const theme of ["dark", "light"]) { await rpc("setting.set", { key: "ui.theme", value: theme }); await Bun.sleep(300); await shot(`controls-${theme}`); }
  await rpc("setting.set", { key: "browser.engine", value: "chrome" });
  await until(() => info["browser.engine.live"] === "chrome", "Chrome fallback", 60000);
  await address(`127.0.0.1:${site.port}/fallback`, 150); await active(a.id, "/fallback");
  check("same address and tab UI works in Chrome fallback", !await mapped());
  console.log("all native browser controls checks passed");
  }
} catch (e) {
  console.error("FAIL", e, "app stderr:", readFileSync(join(d.root, "app.err"), "utf8").slice(-6000));
  try { await shot("failure"); } catch {}
  throw e;
} finally {
  ws?.close(); site.stop(true);
  try {
    if (win && app.exitCode === null) { await d.x(`close ${win}`); await until(() => app.exitCode !== null, "app closes", 30000); }
  } finally {
    for (const file of ["app.out", "app.err"]) writeFileSync(join(logDir, file), readFileSync(join(d.root, file)));
    await d.cleanup();
  }
}
