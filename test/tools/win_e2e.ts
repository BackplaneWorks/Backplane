// The JS hub's agents and terminals end to end, on any platform (Windows
// included): no sh stand-ins and no bun needed on the machine, so it can be
// compiled with the hub for a Windows VM (`bun build --compile`). A
// throwaway home and a free port; the hub is stopped by the PID it was
// started with.
//   - /hello answers and a WebSocket joins
//   - an agent turn runs to completion through a stand-in `claude`
//     (test/tools/claude_standin.ts compiled to AGENTS/claude[.exe]), and a
//     second turn resumes the session
//   - a terminal (terminal_open through the hub's MCP) shows its program's
//     output, takes typed keys (term.input) and is read back
//     (terminal_read), and a client's term.snap gets its screen
// Prints "ok ..." / "FAIL ..." and exits with the number of failures.
//
//   win_e2e HUB WIREDIR AGENTS        (SHELL: the shell terminals run under)
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join, resolve } from "node:path";

const [hub, wire, agents] = process.argv.slice(2);
if (!hub || !wire || !agents) {
  console.log("usage: win_e2e HUB WIREDIR AGENTS");
  process.exit(2);
}
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || !more ? "" : ": " + JSON.stringify(more).slice(0, 600)}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-win-e2e-"));
const home = join(root, "home");
const proj = join(root, "proj");
const log = join(root, "standin.log");
mkdirSync(proj);
const port = freePort();
if (port === 3787) throw new Error("refusing the live port");
const t0 = Date.now();
const proc = Bun.spawn([resolve(hub), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  // as exit_e2e: no display, the keyring known unlocked (a turn waits on it), nothing baked
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "",
    BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_AGENTS: resolve(agents),
    FAKE_LOG: log, PATH: resolve(agents) + delimiter + (process.env.PATH ?? "") },
  stdout: "ignore",
  stderr: Bun.file(join(root, "hub.err")),
});
try {
  let up = false;
  for (let i = 0; i < 600 && !up; i++) {
    try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch {}
    if (!up) await sleep(100);
  }
  check(up, `/hello answers (${Date.now() - t0} ms)`);
  if (!up) throw new Error("no hub");
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const seen: any[] = [];
  const msgs: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (!o) return;
    msgs.push(o);
    for (const c of o.items ?? []) seen.push(c.c ?? c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  check(true, "a WebSocket joins");
  let n = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++n, m, p }))); return n; };
  const mcp = async (thread: string, name: string, args: object) => {
    const r = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
    });
    const t = await r.text();
    const line = t.split("\n").find((l) => l.startsWith("{") || l.startsWith("data: {")) ?? t;
    try {
      const o = JSON.parse(line.replace(/^data: /, ""));
      return { err: !!o.result?.isError, text: o.result?.content?.[0]?.text ?? JSON.stringify(o) };
    } catch {
      return { err: true, text: t.slice(0, 300) };
    }
  };
  const text = (b: number[]) => new TextDecoder().decode(new Uint8Array(b));
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));

  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated"));
  check(!!pc, "project.add");
  // a project added afresh starts with a thread of its own: ours is the one made after it
  await until(5000, () => seen.find((c) => c.$ === "ThreadCreated"));
  const before = seen.filter((c) => c.$ === "ThreadCreated").length;
  send("thread.create", { project: pc?.id, title: "win", env: "local", provider: "claude" });
  const th = (await until(10000, () => seen.filter((c) => c.$ === "ThreadCreated")[before]))?.id as string;
  check(!!th, "thread.create");

  // an agent turn, then a second one on the same session
  send("turn.start", { thread: th, text: "mode:plain hello", msg: "w-1" });
  const done1 = await until(30000, () => turns(th).at(-1) === "completed" ? true : undefined);
  const said = existsSync(log) ? readFileSync(log, "utf8") : "";
  check(!!done1, "an agent turn completes", { turns: turns(th), standin: said.slice(0, 300) });
  check(said.includes("START"), "the stand-in agent was started by the hub", said.slice(0, 200));
  send("turn.start", { thread: th, text: "again", msg: "w-2" });
  const done2 = await until(30000, () => turns(th).filter((s) => s === "completed").length >= 2 ? true : undefined);
  check(!!done2, "a second turn completes", turns(th));

  // a terminal the agent opens; the person types into it
  const open = await mcp(th, "terminal_open", { command: "echo hello-term; read x; echo got:$x; sleep 60", title: "Echo", cols: 60, rows: 8 });
  const id = open.err ? "" : JSON.parse(open.text).id;
  check(!!id, "terminal_open answers an id", open);
  const key = `x:${th}:${id}`;
  const live = await until(15000, () => msgs.find((o) => o.t === "term" && o.thread === key && text(o.b).includes("hello-term")));
  check(!!live, "its output reaches the clients");
  let r = await mcp(th, "terminal_read", { id });
  check(!r.err && r.text.includes("hello-term"), "terminal_read shows it", r);
  send("term.input", { thread: key, b: [...new TextEncoder().encode("abc\r")] });
  let typed = false;
  for (let i = 0; i < 50 && !typed; i++) {
    await sleep(200);
    r = await mcp(th, "terminal_read", { id });
    typed = r.text.includes("got:abc");
  }
  check(typed, "typed keys reach the program", r);
  const m0 = msgs.length;
  send("term.snap", { key });
  const snap = await until(5000, () => msgs.slice(m0).find((o) => o.t === "term" && o.thread === key && o.reset));
  check(!!snap && text(snap.b ?? []).includes("hello-term"), "term.snap: the screen", snap);
  await mcp(th, "terminal_close", { id });
  ws.close();
} catch (e) {
  check(false, "the run", String(e));
} finally {
  proc.kill();
  await proc.exited;
  if (fail > 0 && existsSync(join(root, "hub.err"))) {
    console.log("--- hub stderr (tail)");
    console.log(readFileSync(join(root, "hub.err"), "utf8").split("\n").slice(-30).join("\n"));
  }
  try { rmSync(root, { recursive: true, force: true }); } catch {}
}
console.log(fail === 0 ? "all ok" : `${fail} FAILED`);
process.exit(fail);
