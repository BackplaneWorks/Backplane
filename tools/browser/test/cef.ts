// Source and compiled helper acceptance. Every process/profile belongs to this run.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { check, until, jsonProcess, privateDisplay } from "../../cef/test/harness.ts";
if (!process.env.BACKPLANE_CEF || !existsSync(process.env.BACKPLANE_CEF)) {
  console.log("SKIP CEF: set BACKPLANE_CEF to a built backplane-cef"); process.exit(0);
}
const d = await privateDisplay();
const flags = ["--profile", join(d.root, "profile"), "--jpeg-dir", join(d.root, "shots"), "--frames", join(d.root, "frame.rgb")];
const helper = jsonProcess(process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER, ...flags] : ["bun", new URL("../src/main.ts", import.meta.url).pathname, ...flags], d.env);
const { call, events, proc } = helper;
const page = (title: string) => `data:text/html,${encodeURIComponent(`<!doctype html><title>${title}</title><h1>${title}</h1><input id=q aria-label=Search><button id=btn>Go</button><a id=pop target=_blank href="${serverUrl}/popup">Popup</a><script>window.clicks=0;window.wheels=0;document.addEventListener('click',()=>clicks++);document.addEventListener('wheel',()=>wheels++);let i=0;setInterval(()=>btn.textContent='Go '+ ++i,70)</script>`)}`;
const server = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch(req) {
  if (new URL(req.url).pathname === "/file") return new Response("cef download\n", { headers: { "content-disposition": 'attachment; filename="cef.txt"' } });
  return new Response('<!doctype html><title>Popup</title><h1>Popup</h1><input aria-label=Popup>', { headers: { "content-type": "text/html" } });
} });
const serverUrl = `http://127.0.0.1:${server.port}`;
let hostPid = 0;
try {
  const e = await call("engine", { engine: "cef", parent: d.parent, display: d.env.DISPLAY });
  check("engine cef starts", e.engine === "cef" && e.hostPid > 0, e); hostPid = e.hostPid;
  await Promise.all([
    call("open", { page: "a", url: page("Owner A") }),
    call("show", { page: "a", x: 10, y: 10, w: 600, h: 400 }),
  ]);
  const openingChild = (await d.x("children")).children.find((k: any) => k.mapped && !k.inputOnly);
  check("concurrent open preserves the shown pane viewport", openingChild?.w === 600 && openingChild.h === 400, openingChild);
  await call("hide");
  const b = await call("open", { page: "b", url: page("Owner B") });
  for (const [owner, title] of [["a", "Owner A"], ["b", "Owner B"]]) {
    const s = await call("snapshot", { page: owner }); check(`${owner} owns its snapshot`, s.title === title && s.outline.includes(title), s);
  }
  const targets = await (await fetch(`http://127.0.0.1:${e.port}/json/list`)).json();
  check("exactly two page targets", targets.filter((p: any) => p.type === "page").length === 2, targets);
  const tops = await d.x("tops"); check("no top-level Chrome window", tops.chromium === 0, tops);
  await call("show", { page: "a", x: 10, y: 10, w: 600, h: 400 });
  let kids = (await d.x("children")).children;
  let visible = kids.filter((k: any) => k.mapped);
  check("a mapped at exact geometry; b hidden", visible.length === 1 && visible[0].x === 10 && visible[0].y === 10 && visible[0].w === 600 && visible[0].h === 400, kids);
  const aXid = visible[0].xid;
  await call("show", { page: "b", x: 20, y: 20, w: 700, h: 450 });
  kids = (await d.x("children")).children; visible = kids.filter((k: any) => k.mapped);
  check("show b swaps children", visible.length === 1 && visible[0].xid !== aXid && visible[0].w === 700 && !kids.find((k: any) => k.xid === aXid).mapped, kids);
  await call("hide");
  check("helper hide unmaps every browser", (await d.x("children")).children.every((k: any) => !k.mapped));
  await call("show", { page: "b", x: 20, y: 20, w: 700, h: 450 });
  await call("resize", { page: "a", width: 500, height: 350 });
  const hidden = await d.x(`info ${aXid}`);
  check("hidden resize preserves unmapped state", !hidden.mapped && hidden.w === 500 && hidden.h === 350, hidden);
  const aStatus = await call("status", { page: "a" }); check("status reports real viewport", aStatus.width === 500 && aStatus.height === 350, aStatus);

  await call("record", { page: "a", on: true });
  await call("click", { page: "a", selector: "#btn" });
  await call("type", { page: "a", selector: "#q", text: "CEF recording" });
  await call("press", { page: "a", key: "Tab" });
  await until(() => events.find((v) => v.event === "record" && v.page === "a" && v.step.do === "type" && v.step.value === "CEF recording"), "recorder type");
  check("CDP context binding and init script record click/type", events.some((v) => v.event === "record" && v.step.do === "click"));
  await call("record", { page: "a", on: false });
  for (const owner of ["a", "b"]) {
    const before = events.filter((v) => v.event === "frame" && v.page === owner).length;
    await call("cast", { page: owner, ms: 2000 });
    await until(() => events.filter((v) => v.event === "frame" && v.page === owner).length > before + 2, `JPEG cast ${owner}`);
    check(`${owner} ${owner === "a" ? "hidden" : "shown"} cast produces JPEG`, readFileSync(join(d.root, "shots", `${owner}.jpg`)).subarray(0, 2).equals(Buffer.from([255, 216])));
  }
  await call("record", { page: "a", on: true });
  await call("click", { page: "a", selector: "#pop" });
  const tabs = await until(async () => { const t = await call("tabs", { page: "a" }); return t.tabs.length === 2 && t.active === 1 && t.tabs[1].title === "Popup" ? t : null; }, "popup adoption");
  check("popup is active owner tab", tabs.tabs.length === 2);
  await call("type", { page: "a", selector: "input", text: "Popup recording" });
  await call("press", { page: "a", key: "Tab" });
  await until(() => events.find((v) => v.event === "record" && v.page === "a" && v.step.value === "Popup recording"), "popup recording");
  check("popup inherits owner recording", true);
  await call("record", { page: "a", on: false });
  const afterPop = await (await fetch(`http://127.0.0.1:${e.port}/json/list`)).json();
  check("popup adds exactly one child", afterPop.filter((p: any) => p.type === "page").length === 3 && (await d.x("children")).children.length === 4 && (await d.x("tops")).chromium === 0);
  await call("tabs", { page: "a", close: 1 });
  const explicit = await call("tabs", { page: "a", new: "data:text/html,<h1>Explicit tab</h1>" });
  check("tabs new uses embedded child", explicit.tabs.length === 2 && explicit.active === 1 && (await d.x("tops")).chromium === 0);
  await call("tabs", { page: "a", close: 1 });
  await call("show", { page: "a", x: 10, y: 10, w: 600, h: 400 });
  await call("evaluate", { page: "a", expression: "clicks=0; wheels=0" });
  await call("shield", { on: true });
  await call("click", { page: "a", selector: "#btn" });
  check("agent CDP input works through shield", await call("evaluate", { page: "a", expression: "clicks === 1" }));
  await call("evaluate", { page: "a", expression: "clicks=0" });
  const n = events.length;
  await d.x("click 40 110 1"); await d.x("click 40 110 5");
  await until(() => events.slice(n).find((v) => v.event === "shield-click" && v.page === "a"), "shield-click event");
  await Bun.sleep(100);
  check("shield blocks pointer and wheel", await call("evaluate", { page: "a", expression: "clicks === 0 && wheels === 0" }));
  const shieldClick = events.slice(n).find((v) => v.event === "shield-click");
  check("shield coordinates are page CSS px", shieldClick.x === 30 && shieldClick.y === 100, shieldClick);
  await call("shield", { on: false });
  await d.x("click 40 110 1"); await d.x("click 40 110 5");
  await until(() => call("evaluate", { page: "a", expression: "clicks > 0 && wheels > 0" }), "unshielded input");
  check("unshielded pointer and wheel reach page", true);

  const focusStart = events.length;
  const box = await call("evaluate", { page: "a", expression: "(()=>{q.value='';const r=q.getBoundingClientRect();return {x:r.x+8,y:r.y+8}})()" });
  await d.x(`click ${Math.round(10 + box.x)} ${Math.round(10 + box.y)} 1`);
  await until(async () => (await d.x("focus")).focus !== d.parent, "click transfers X focus from embedding parent");
  await d.x("key a");
  await until(() => call("evaluate", { page: "a", expression: "q.value === 'a'" }), "click takes child keyboard focus");
  check("real desktop click transfers X keyboard focus", true);
  check("native focus event names the owner", events.slice(focusStart).some((v) => v.event === "focus" && v.page === "a"));
  await call("evaluate", { page: "a", expression: "q.value='';q.focus()" });
  await call("focus", { page: "a" }); await d.x("key b");
  await until(() => call("evaluate", { page: "a", expression: "q.value === 'b'" }), "focus op keyboard");
  check("focus op accepts XTest keyboard input", true);
  const pdf = await rawError("pdf", { page: "a", path: join(d.root, "out.pdf") });
  check("CEF pdf has clear error", pdf.includes("pdf needs the headless engine"), pdf);
  const dl = await call("download", { page: "a", url: `${serverUrl}/file`, dir: join(d.root, "dl") });
  check("CEF download saves suggested file", dl.suggestedFilename === "cef.txt" && dl.bytes === 13 && readFileSync(dl.path, "utf8") === "cef download\n", dl);
  console.log("download result", JSON.stringify(dl));
  await call("open", { url: "data:text/html,<h1>Shared RGB</h1>", width: 320, height: 240 });
  await call("frames", { on: true, maxFps: 10 });
  const raw = readFileSync(join(d.root, "frame.rgb"));
  check("CEF shared raw RGB frames", raw.subarray(0, 4).toString() === "BPF1" && raw.readUInt32LE(4) === 320 && raw.readUInt32LE(8) === 240 && raw.length === 16 + 320 * 240 * 3);
  await call("frames", { on: false }); await call("close", { page: "" });
  await call("close", { page: "a" });
  check("closing a keeps b and host alive", (await call("pages")).pages.length === 1 && (await call("status", { page: "b" })).hostPid === hostPid);
  await call("close", { page: "b" });
  check("last browser close keeps host running", (await call("status")).hostPid === hostPid && (await call("pages")).pages.length === 0);
  await call("open", { page: "b", url: page("Reopened") });
  check("new page after last close uses same host", (await call("status", { page: "b" })).hostPid === hostPid);
  await call("engine", { engine: "chrome" });
  check("switch to Chrome reopens active URLs", (await call("snapshot", { page: "b" })).title === "Reopened");
  check("old CEF host exited after switch", !existsSync(`/proc/${hostPid}`));
  const again = await call("engine", { engine: "cef", parent: d.parent, display: d.env.DISPLAY }); hostPid = again.hostPid;
  check("switch back reopens active URLs", (await call("snapshot", { page: "b" })).title === "Reopened");
  await call("close");
  check("helper close exits zero", await proc.exited === 0);
  check("helper close waits for host exit", !existsSync(`/proc/${hostPid}`));
  console.log("all CEF acceptance checks passed");
} finally {
  if (proc.exitCode === null) { proc.stdin.end(); await proc.exited; }
  server.stop(true); await d.cleanup();
}
async function rawError(op: string, args: any) { try { await call(op, args); return ""; } catch (e) { return String(e); } }
