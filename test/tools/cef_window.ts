// The real native window and CEF, on a private display and an empty home.
// Run only inside bp-admit --mem 6 --cores 2. Requires build/backplane,
// build/wire and the 2a BACKPLANE_CEF runtime. No live hub or display is used.
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
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
writeFileSync(join(fake, "backplane-browser"), `#!/bin/sh\nexec bun '${resolve("tools/browser/src/main.ts")}' "$@"\n`);
chmodSync(join(fake, "backplane-browser"), 0o755);
const listen = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listen.port; listen.stop(true);
const env = { ...d.env, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", WAYLAND_DISPLAY: "", BACKPLANE_CEF_NO_GPU: "1" };
const app = Bun.spawn([resolve(binary), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], { env, stdout: Bun.file(join(d.root, "app.out")), stderr: Bun.file(join(d.root, "app.err")) });
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
const shotDir = process.env.CEF_SHOTS ?? resolve("cef-window-shots");
mkdirSync(shotDir, { recursive: true });
const logDir = process.env.CEF_LOGS ?? resolve("build/2b-logs/window");
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
const PAGE = "eef4dd", CLICKED = "f5b25e";
async function shot(name: string) {
  const path = join(shotDir, name + ".png"); await run(["import", "-display", d.env.DISPLAY!, "-window", "root", path]); console.log("screenshot " + path);
}
const site = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  const name = new URL(req.url).pathname;
  return new Response(`<!doctype html><title>${name}</title><body style="margin:0;background:#eef4dd;color:#182018;font:24px monospace"><h1 style="margin:20px">Shared ${name}</h1><input id="field" aria-label="Shared input" style="margin:20px;width:340px;height:44px;font:24px monospace"><button id="go" style="height:50px" onclick="document.body.style.background='#f5b25e';document.querySelector('h1').textContent='Clicked '+(++window.count)">Go</button><a id="popup" target="_blank" href="/popup">Popup</a><script>window.count=0</script>`, { headers: { "content-type": "text/html" } });
} });
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
  await until(() => info["browser.engine.live"] === "chrome", "initial Chrome info");
  await rpc("setting.set", { key: "onboard.done", value: "1" });
  await rpc("setting.set", { key: "onboard.uber", value: "1" });
  await poke("key", "Escape");
  check("default is Chrome", info["browser.engine.live"] === "chrome", info);
  await rpc("project.add", { path: proj });
  const project = await until(() => changes.find(c => c.$ === "ProjectCreated" && c.path === proj) ?? changes.find(c => c.$ === "ProjectCreated" && c.id !== "scratch"), "project");
  for (const title of ["zqcefalpha", "zqcefbeta"]) await rpc("thread.create", { project: project.id, title, env: "local" });
  const a = await until(() => changes.find(c => c.$ === "ThreadCreated" && c.title === "zqcefalpha"), "thread A");
  const b = await until(() => changes.find(c => c.$ === "ThreadCreated" && c.title === "zqcefbeta"), "thread B");
  check("unknown browser owner refused", !(await rpc("bots.browser", { bot: "invented", op: "takeover" })).ok);
  await rpc("setting.set", { key: "browser.engine", value: "cef" });
  await until(() => info["browser.engine.live"] === "cef", "CEF engine live", 60000);
  // An empty Browser tab makes no page: thread B has none yet.
  await select("zqcefbeta"); await Bun.sleep(1500);
  check("an empty Browser tab creates no CEF browser", (await children()).length === 0 && !info[`browser.page.${b.id}`], await children());
  await select("zqcefalpha");
  check("agent opens its shared page", !((await mcp(a.id, "preview_open", { url: `http://127.0.0.1:${site.port}/alpha` })).result?.isError));
  let ca = await until(async () => {
    const c = await mapped();
    return c?.x === 832 && c.y === 96 && c.w === 568 && c.h === 804 ? c : null;
  }, "native page fills pane", 30000);
  check("CEF is an embedded mapped child filling the pane", ca.parent === win && ca.x === 832 && ca.y === 96 && ca.w === 568 && ca.h === 804, ca);
  console.log("xwininfo " + (await run(["xwininfo", "-display", d.env.DISPLAY!, "-id", String(ca.xid)])).trim());
  check("the page exists for every client", await until(() => info[`browser.page.${a.id}`] === "1", "browser.page key"));
  const spot = { x: ca.x + Math.floor(ca.w / 2), y: ca.y + ca.h - 60 };
  check("the page's pixels are on screen in the pane", await until(async () => near(await px(spot.x, spot.y), PAGE), "page pixels", 10000));
  for (const theme of ["dark", "light"]) { await rpc("setting.set", { key: "ui.theme", value: theme }); await Bun.sleep(500); await shot(`4a-${theme}`); }
  const nativePics = await picCount();
  check("agent types into the visible page", !((await mcp(a.id, "preview_type", { selector: "#field", text: "agent", clear: true })).result?.isError));
  check("agent clicks visible page", !((await mcp(a.id, "preview_click", { selector: "#go" })).result?.isError));
  check("agent snapshot sees same changed page", text(await mcp(a.id, "preview_snapshot")).includes("Clicked 1"));
  check("the agent's click changed the pixels on screen", await until(async () => near(await px(spot.x, spot.y), CLICKED), "clicked pixels", 10000));
  // Only this machine's window embeds or places: a tailnet client is refused,
  // and so is a place from a client that did not embed.
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const far = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`, { headers: { "tailscale-user-login": "someone@example.com" } } as any);
  far.binaryType = "arraybuffer";
  const farReplies = new Map<number, any>();
  far.onmessage = e => { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data))); if (o.t === "reply") farReplies.set(Number(o.id), o); };
  await new Promise<void>((r, j) => { far.onopen = () => r(); far.onerror = j; });
  far.send(W.encode(JSON.stringify({ id: 1, m: "browser.embed", p: { xid: win, display: d.env.DISPLAY } })));
  const farEmbed = await until(() => farReplies.get(1), "remote embed reply");
  check("an embed from a remote client is refused", farEmbed.ok === false, farEmbed);
  far.close();
  // A browser on this machine still has an Origin header; it cannot embed.
  const localWeb = new WebSocket(`ws://127.0.0.1:${port}/ws`, { headers: { Origin: `http://127.0.0.1:${port}` } } as any);
  localWeb.binaryType = "arraybuffer";
  let localReply: any;
  localWeb.onmessage = e => { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data))); if (o.t === "reply" && Number(o.id) === 1) localReply = o; };
  await new Promise<void>((r, j) => { localWeb.onopen = () => r(); localWeb.onerror = j; });
  try {
    localWeb.send(W.encode(JSON.stringify({ id: 1, m: "browser.embed", p: { xid: win, display: d.env.DISPLAY } })));
    check("a loopback web browser cannot embed", (await until(() => localReply, "loopback web embed refusal")).ok === false);
  } finally { localWeb.close(); }
  check("a place from a client that did not embed is refused", (await rpc("browser.place", { page: a.id, x: 0, y: 0, w: 100, h: 100 })).ok === false);
  check("the window's page stays placed", (await mapped())?.xid === ca.xid);
  await Bun.sleep(400);
  check("native page does not load JPEG shots", await picCount() === nativePics);
  // This separate WebSocket represents a watching web/phone client.
  // Its lease must still produce JPEG frames while the native window embeds.
  const jpg = join(home, "shots", a.id + ".jpg");
  const beforeRemote = messages.length;
  await rpc("browser.seen", { page: a.id });
  await mcp(a.id, "preview_evaluate", { expression: "document.querySelector('#go').style.color = 'red'" });
  check("a remote watcher still gets frames in CEF mode", await until(() => existsSync(jpg) && messages.slice(beforeRemote).some(o => o.t === "shot" && o.bot === a.id), "remote CEF frame", 15000));
  check("remote frame is a JPEG", readFileSync(jpg).subarray(0, 2).equals(Buffer.from([255, 216])));
  check("remote casting does not make the native window decode JPEGs", await picCount() === nativePics);
  const status = JSON.parse(text(await mcp(a.id, "preview_status")));
  await d.x(`click ${ca.x + 410} ${ca.y + 145} 1`);
  await until(() => info[`drive.${a.id}`] === "takeover", "shield click takes over");
  await until(async () => !(await d.x(`children ${win}`)).children.some((c: any) => c.inputOnly && c.mapped), "takeover removes shield");
  check("takeover click was swallowed", await evalPage(status.port, "/alpha", "window.count") === 1);
  check("agent cannot click during takeover", (await mcp(a.id, "preview_click", { selector: "#go" })).result?.isError);
  await d.x(`click ${ca.x + 40} ${ca.y + 145} 1`);
  for (const key of "native") await d.x(`key ${key}`);
  check("XTest keys type into the real page", String(await evalPage(status.port, "/alpha", "document.querySelector('#field').value")).includes("native"));
  for (const theme of ["dark", "light"]) { await rpc("setting.set", { key: "ui.theme", value: theme }); await Bun.sleep(500); await shot(`4c-${theme}`); }
  await d.x("click 1340 72 1"); // inside the thread strip's Hand back
  await until(() => info[`drive.${a.id}`] === "", "strip Hand back");
  await until(async () => (await d.x(`children ${win}`)).children.some((c: any) => c.inputOnly && c.mapped), "handback restores shield");
  check("native typing reached shared input", text(await mcp(a.id, "preview_snapshot")).includes("native"));
  // Focus returns on a parent press; palette hides the native child.
  await d.x("click 300 30 1"); check("parent takes keyboard focus back", await until(async () => (await d.x("focus")).focus === win, "parent focus restored"));
  await poke("ctrl", "k"); await until(async () => !(await mapped()), "palette hides child");
  await poke("key", "Escape"); await pageShown(); check("closing palette restores child", true);
  await poke("size", "1500", "950");
  ca = await until(async () => { const c = await mapped(); return c?.x === 882 && c.y === 96 && c.w === 618 && c.h === 854 ? c : null; }, "resize placement");
  await select("zqcefbeta"); await mcp(b.id, "preview_open", { url: `http://127.0.0.1:${site.port}/beta` });
  const cb = await until(async () => { const c = await mapped(); return c && c.xid !== ca.xid ? c : null; }, "second thread browser");
  check("second thread maps a different CEF browser", cb.xid !== ca.xid, cb);
  await select("zqcefalpha");
  check("return preserves native browser", await until(async () => (await mapped())?.xid === ca.xid, "first browser restored"));
  check("return preserves typed state", text(await mcp(a.id, "preview_snapshot")).includes("native"));
  const popup = await mcp(a.id, "preview_click", { selector: "#popup" });
  check("agent opens popup link", !popup.result?.isError, popup);
  await until(async () => { const c = await mapped(); return c && c.xid !== ca.xid ? c : null; }, "active popup maps without a placement change");
  check("agent popup is the visible owner's active page", text(await mcp(a.id, "preview_snapshot")).includes("Shared /popup"));
  await mcp(a.id, "preview_evaluate", { expression: "setTimeout(() => window.close(), 100)" });
  await until(async () => (await mapped())?.xid === ca.xid, "closing popup restores prior native tab");
  check("closing popup preserves the original page state", text(await mcp(a.id, "preview_snapshot")).includes("native"));
  const created = await rpc("bots.create", { name: "zqcefbot" });
  const bot = await until(() => changes.find(c => c.$ === "BotSet" && c.id === created.bot), "bot");
  // Filtering projects leaves the bot row at y=136, under the Bots heading.
  // Choose that row so this is the actual bot UI, rather than its plain thread.
  await poke("click", "100", "74"); await poke("type", "zzno-project-match");
  await poke("click", "100", "154");
  // Select the bot's Browser tab (Space, Browser, Tasks...) while it has no page.
  const before = (await children()).length;
  await poke("click", "520", "62"); await Bun.sleep(1500);
  check("the bot's empty Browser tab creates no CEF browser", (await children()).length === before && !info[`browser.page.${bot.id}`], await children());
  await mcp(bot.thread, "preview_open", { url: `http://127.0.0.1:${site.port}/bot` });
  await poke("click", "820", "24"); // close the duplicate right-hand Viewer pane
  let cc = await until(async () => { const c = await mapped(); return c?.x === 264 && c.w === 1236 && c.y > 100 ? c : null; }, "bot Browser tab fills main pane");
  check("bot Browser tab embeds its own native page", true);
  await d.x(`click ${cc.x + 410} ${cc.y + 145} 1`);
  await until(() => info[`drive.${bot.id}`] === "takeover", "bot shield click takes over");
  check("bot agent is refused during native takeover", (await mcp(bot.thread, "preview_click", { selector: "#go" })).result?.isError);
  await rpc("bots.browser", { bot: bot.id, op: "handback" });
  await until(() => info[`drive.${bot.id}`] === "", "bot handback");
  await until(async () => (await d.x(`children ${win}`)).children.some((c: any) => c.inputOnly && c.mapped), "bot handback restores shield");
  await rpc("bots.teach", { bot: bot.id, op: "start" });
  await until(() => info[`drive.${bot.id}`] === "teach", "bot teach");
  await Bun.sleep(750); // the recorder binding and native focus settle
  cc = await pageShown();
  await d.x(`click ${cc.x + 410} ${cc.y + 145} 1`);
  await until(() => (info[`teach.draft.${bot.id}`] ?? "").includes("click"), "native click recorded");
  check("native bot teach records person input", true);
  await rpc("bots.teach", { bot: bot.id, op: "discard" });
  // status.hostPid is local to the helper's PID namespace in Flatpak. Follow
  // our own app's descendants on the host so this also checks sandboxed CEF.
  const cefPids = processTree(app.pid).filter(pid => {
    try { return readFileSync(`/proc/${pid}/cmdline`, "utf8").split("\0")[0].endsWith("/backplane-cef"); }
    catch { return false; }
  });
  check("owned CEF process tree exists", cefPids.length > 0);
  const beforeChromePics = await picCount();
  await rpc("setting.set", { key: "browser.engine", value: "chrome" });
  await until(() => info["browser.engine.live"] === "chrome", "Chrome restored", 60000);
  check("switching to Chrome removes the owned CEF process tree", await until(() => cefPids.every(pid => !existsSync(`/proc/${pid}`)), "CEF processes exited"));
  check("switching to Chrome unmaps all CEF pages", !(await mapped()));
  await select("zqcefalpha"); await mcp(a.id, "preview_open", { url: `http://127.0.0.1:${site.port}/chrome` });
  check("agents still work with Chrome", text(await mcp(a.id, "preview_snapshot")).includes("Shared /chrome"));
  await until(() => existsSync(join(home, "shots", a.id + ".jpg")), "JPEG restored");
  await until(async () => await picCount() > beforeChromePics, "native window decodes Chrome JPEG frames");
  check("JPEG path restored in the native window", true);
  await shot("4g-chrome");
  // Exercise an ABA engine sequence while earlier host starts are still pending.
  for (const engine of ["cef", "chrome", "cef"]) await rpc("setting.set", { key: "browser.engine", value: engine });
  await until(() => info["browser.engine.live"] === "cef", "rapid engine changes settle", 60000);
  await pageShown(); await Bun.sleep(500);
  check("rapid engine toggles retain a mapped native page", !!(await mapped()));
  await rpc("setting.set", { key: "browser.engine", value: "chrome" });
  await until(() => info["browser.engine.live"] === "chrome", "final Chrome engine", 60000);
  console.log("all native CEF window checks passed");
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
    console.log("app logs " + logDir);
    await d.cleanup();
  }
}
