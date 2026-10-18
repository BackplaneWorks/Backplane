// Host protocol, target identity, page close, command validation and EOF lifetime.
import { existsSync } from "node:fs";
import { join } from "node:path";
import { check, until, jsonProcess, privateDisplay } from "./harness.ts";
if (!process.env.BACKPLANE_CEF || !existsSync(process.env.BACKPLANE_CEF)) {
  console.log("SKIP CEF host: set BACKPLANE_CEF"); process.exit(0);
}
const d = await privateDisplay();
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true);
const h = jsonProcess([process.env.BACKPLANE_CEF, "--parent", String(d.parent), "--port", String(port), "--cache", join(d.root, "cef")], d.env);
async function command(line: string, event: string, key?: string) {
  const before = h.events.length; h.write(line);
  return until(() => h.events.slice(before).find((v) => v.event === event && (key === undefined || key === v.key)), line);
}
try {
  await until(() => h.events.find((e) => e.event === "init"), "init");
  await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/json/list`)).ok; } catch { return false; } }, "CDP");
  check("no startup browser without --url", (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).length === 0);
  for (const line of ["nonsense", "create", "create a.b about:blank", "create a", "hide extra", "shield 2", "quit extra", "show a 0 0 0 1", ""]) {
    const err = await command(line, "error"); check(`rejects malformed ${JSON.stringify(line)}`, typeof err.cmd === "string" && "key" in err && err.t > 0, err);
  }
  const a = await command("create a data:text/html,<title>A title with spaces</title>", "created", "a");
  check("created includes command, key, XID, target and timestamp", a.cmd === "create" && a.xid > 0 && a.target && a.t > 0, a);
  await command("create a about:blank", "error", "a");
  const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  check("created target matches /json/list", list.length === 1 && list[0].id === a.target, list);
  const geometry = await d.x(`info ${a.xid}`);
  check("create child is unmapped with default size", geometry.parent === d.parent && !geometry.mapped && geometry.w === 1280 && geometry.h === 800, geometry);
  await command("show a 10 20 640 400", "ok", "a");
  check("show mapping", (await d.x(`info ${a.xid}`)).mapped);
  await command("hide", "ok");
  await command("size a 500 300", "ok", "a");
  check("size preserves hidden state", !(await d.x(`info ${a.xid}`)).mapped);
  const closed = await command("close a", "closed", "a");
  check("command close reason", closed.cmd === "close" && closed.why === "command");
  await command("create b about:blank", "created", "b");
  check("host survives last browser close", h.proc.exitCode === null);
  // Close via the page (window.close) over a direct native Bun WebSocket.
  const b = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json())[0];
  const ws = new WebSocket(b.webSocketDebuggerUrl);
  await new Promise<void>((r, j) => { ws.onopen = () => r(); ws.onerror = j; });
  ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression: "window.close()" } }));
  const pageClose = await until(() => h.events.find((e) => e.event === "closed" && e.key === "b"), "page close");
  check("window.close has page reason", pageClose.why === "page"); ws.close();
  await command("create c about:blank", "created", "c");
  const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  const browserWs = new WebSocket(version.webSocketDebuggerUrl);
  await new Promise<void>((r, j) => { browserWs.onopen = () => r(); browserWs.onerror = j; });
  let closeReply: any = null;
  browserWs.onmessage = (e) => { const v = JSON.parse(String(e.data)); if (v.id === 1) closeReply = v; };
  browserWs.send(JSON.stringify({ id: 1, method: "Browser.close" }));
  await until(() => closeReply || h.events.find((e) => e.event === "closed" && e.key === "c"), "CDP Browser.close response");
  console.log("Browser.close response", JSON.stringify(closeReply));
  browserWs.close();
  await command("create d about:blank", "created", "d");
  check("CDP Browser.close leaves host running", h.proc.exitCode === null);
  h.proc.stdin.end();
  check("EOF closes remaining browsers and exits zero", await h.proc.exited === 0);
  console.log("all host protocol checks passed");
} finally {
  if (h.proc.exitCode === null) { h.proc.stdin.end(); await h.proc.exited; }
  await d.cleanup();
}
