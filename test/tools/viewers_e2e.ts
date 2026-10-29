// End to end: each thread has its own viewer and its own browser page.
// Starts a headless hub on a temporary home and a free port, makes two
// threads, and has each thread's agent (through its MCP endpoint) show a
// file and open a page. Checks that each "view" names its thread, that
// each thread drives its own page (its own URL, its own frames in
// <home>/shots/<thread>.jpg, its own "shot" messages), and that deleting a
// thread closes its page. Needs Chrome (see tools/browser/README.md).
// Prints "ok ..." / "FAIL ..." lines.
//
//   bun test/tools/viewers_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, with build/backplane-browser beside
// it (cd tools/browser && bun build.ts ../../build/backplane-browser);
// WIREDIR to build/wire (bend test/wire/index.html -o build/wire).
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const home = mkdtempSync(join(tmpdir(), "bp-viewers-e2e-"));
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
  stdout: "ignore",
  stderr: "ignore",
});
const seen: any[] = [];
const msgs: any[] = [];
let ws: WebSocket;
let n = 0;

async function until<T>(f: () => T | undefined, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(100);
  }
  return undefined;
}

const send = (m: string, p: any) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));

async function mcp(thread: string, name: string, args: any): Promise<string> {
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const r = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
  });
  return r.text();
}

try {
  if (!(await until(() => existsSync(join(home, "token")), 10000))) throw new Error("the hub did not start");
  await sleep(300);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    msgs.push(o);
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r) => (ws.onopen = r));
  send("project.add", { path: home });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: proj.id, title: "A" });
  send("thread.create", { project: proj.id, title: "B" });
  const threads = await until(() => {
    const ts = seen.filter((c) => c.$ === "ThreadCreated");
    return ts.length >= 2 ? ts : undefined;
  });
  const [a, b] = threads!.map((t: any) => t.id);

  await mcp(a, "viewer_show", { path: join(home, "a.kicad_pcb") });
  const va = await until(() => msgs.find((o) => o.t === "view" && o.path?.endsWith("a.kicad_pcb")));
  check("viewer_show names its thread", va?.thread === a, va);

  const pa = await mcp(a, "preview_open", { url: "data:text/html,<title>page-a</title><h1>A</h1>" });
  const pb = await mcp(b, "preview_open", { url: "data:text/html,<title>page-b</title><h1>B</h1>" });
  check("thread a's page opened", pa.includes("page-a"), pa);
  check("thread b's page opened", pb.includes("page-b"), pb);
  const sa = await mcp(a, "preview_status", {});
  const sb = await mcp(b, "preview_status", {});
  check("thread a keeps its own page", sa.includes("page-a") && !sa.includes("page-b"), sa);
  check("thread b keeps its own page", sb.includes("page-b") && !sb.includes("page-a"), sb);

  const vb = msgs.filter((o) => o.t === "view" && o.kind === "browser").map((o) => o.thread);
  check("opening a page brings each thread's own Browser tab forward", vb.includes(a) && vb.includes(b), vb);
  const shots = await until(() => {
    const s = new Set(msgs.filter((o) => o.t === "shot").map((o) => o.bot));
    return s.has(a) && s.has(b) ? s : undefined;
  });
  check("each thread's frames are announced under its page", !!shots, msgs.filter((o) => o.t === "shot"));
  check("each thread's frames are its own file", existsSync(join(home, "shots", a + ".jpg")) && existsSync(join(home, "shots", b + ".jpg")),
    existsSync(join(home, "shots")) ? readdirSync(join(home, "shots")) : "no shots dir");

  send("thread.delete", { thread: b });
  const gone = await until(() => !existsSync(join(home, "shots", b + ".jpg")) || undefined, 10000);
  check("deleting a thread closes its page", !!gone, readdirSync(join(home, "shots")));
  check("the other thread's page stays", existsSync(join(home, "shots", a + ".jpg")));
} catch (e) {
  check("run", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  rmSync(home, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
