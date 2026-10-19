// Run with BACKPLANE_BROWSER pointing at the installed Flatpak helper wrapper.
// Host XIDs and CDP endpoints cross the sandbox; namespace-local PIDs do not.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { check, until, jsonProcess, privateDisplay } from "../../tools/cef/test/harness.ts";

if (!process.env.BACKPLANE_BROWSER || !process.env.BACKPLANE_CEF) throw new Error("Flatpak helper and CEF runtime are required");
const d = await privateDisplay();
const helper = jsonProcess([process.env.BACKPLANE_BROWSER, "--profile", join(d.root, "profile"), "--jpeg-dir", join(d.root, "shots"), "--frames", join(d.root, "frame.rgb")], d.env);
const { call, proc, events } = helper;
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  if (new URL(req.url).pathname === "/file") return new Response("sandbox download\n", { headers: { "content-disposition": 'attachment; filename="cef.txt"' } });
  return new Response('<!doctype html><title>Flatpak CEF</title><h1>Sandbox page</h1><input id=q aria-label=Search><button id=b onclick="q.value+=\' clicked\'">Go</button><script>window.ticks=0;function animate(){ticks++;document.body.style.backgroundColor=`hsl(${ticks%360},60%,40%)`;requestAnimationFrame(animate)};requestAnimationFrame(animate)</script>', { headers: { "content-type": "text/html" } });
} });
const url = `http://127.0.0.1:${server.port}`;
const endpointGone = async (port: number) => {
  try { await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(500) }); return false; }
  catch { return true; }
};
try {
  const started = await call("engine", { engine: "cef", parent: d.parent, display: d.env.DISPLAY });
  check("Flatpak CEF starts CDP", started.engine === "cef" && started.port > 0, started);
  await call("open", { page: "a", url });
  await call("open", { page: "b", url: `${url}/other` });
  await call("show", { page: "a", x: 10, y: 20, w: 640, h: 400 });
  let children = (await d.x("children")).children;
  const child = children.find((v: any) => v.mapped && !v.inputOnly);
  check("CEF embeds in the host X11 parent at exact geometry", child?.x === 10 && child.y === 20 && child.w === 640 && child.h === 400, children);
  check("CEF creates no top-level browser window", (await d.x("tops")).chromium === 0);
  check("CEF snapshot reads the rendered page", (await call("snapshot", { page: "a" })).title === "Flatpak CEF");
  await call("type", { page: "a", selector: "#q", text: "Flatpak" });
  await call("click", { page: "a", selector: "#b" });
  check("CEF CDP input reaches the embedded page", await call("evaluate", { page: "a", expression: "q.value === 'Flatpak clicked'" }));
  await call("focus", { page: "a" });
  await call("evaluate", { page: "a", expression: "q.value='';q.focus()" });
  await d.x("key a");
  await until(() => call("evaluate", { page: "a", expression: "q.value === 'a'" }), "native X11 keyboard input");
  check("CEF receives native X11 input through the sandbox", true);
  await call("shield", { page: "a", on: true });
  const before = events.length;
  await d.x("click 40 110 1");
  await until(() => events.slice(before).some((v: any) => v.event === "shield-click"), "native input shield event");
  await call("shield", { page: "a", on: false });
  await call("hide");
  check("hide unmaps the embedded child", (await d.x("children")).children.every((v: any) => !v.mapped));
  const firstTicks = await call("evaluate", { page: "a", expression: "ticks" });
  await call("cast", { page: "a", ms: 2000 });
  await until(() => events.filter((v: any) => v.event === "frame" && v.page === "a").length >= 3, "remote JPEG frames");
  check("hidden CEF pages keep rendering for remote viewers", (await call("evaluate", { page: "a", expression: "ticks" })) > firstTicks);
  check("CEF writes real JPEG frames", readFileSync(join(d.root, "shots/a.jpg")).subarray(0, 2).equals(Buffer.from([255, 216])));
  await call("cast", { page: "a", ms: 0 });
  await until(() => call("evaluate", { page: "a", expression: "document.hidden" }), "watcher releases rendering");
  const shot = join(d.root, "page.png");
  await call("screenshot", { page: "a", path: shot });
  check("CEF screenshot is PNG", readFileSync(shot).subarray(1, 4).toString() === "PNG");
  const dl = await call("download", { page: "a", url: `${url}/file`, dir: join(d.root, "downloads") });
  check("CEF download crosses the sandbox filesystem", readFileSync(dl.path, "utf8") === "sandbox download\n");
  await call("open", { url });
  await call("frames", { on: true, maxFps: 10 });
  check("CEF shared preview writes raw RGB", readFileSync(join(d.root, "frame.rgb")).subarray(0, 4).toString() === "BPF1");
  await call("frames", { on: false });
  await call("close", { page: "a" });
  check("closing one owner preserves another", (await call("snapshot", { page: "b" })).title === "Flatpak CEF");
  await call("engine", { engine: "chrome" });
  check("CEF to Chrome keeps the active URL", (await call("snapshot", { page: "b" })).title === "Flatpak CEF");
  await until(() => endpointGone(started.port), "old CEF endpoint closes");
  check("engine switch removes old native children", (await d.x("children")).children.length === 0);
  const again = await call("engine", { engine: "cef", parent: d.parent, display: d.env.DISPLAY });
  check("Chrome to CEF keeps the active URL", (await call("snapshot", { page: "b" })).title === "Flatpak CEF");
  await call("close");
  check("Flatpak helper exits cleanly", await proc.exited === 0);
  await until(() => endpointGone(again.port), "final CEF endpoint closes");
  check("helper shutdown removes native children", (await d.x("children")).children.length === 0);
  console.log("all Flatpak CEF acceptance checks passed");
} finally {
  if (proc.exitCode === null) { proc.stdin.end(); await proc.exited; }
  server.stop(true);
  await d.cleanup();
}
