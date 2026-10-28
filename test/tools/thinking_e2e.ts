// End to end: thinking (model.bend's Think, opt-in) from a real Claude
// turn. Starts a headless hub on a temporary home and a free port, turns
// agent.thinking on, runs a short turn on Claude (haiku, in an empty
// project) that has to reason, and checks that its thought is logged as an
// act of tone "thinking" before the reply. Then turns it off and checks
// the next turn logs none. Needs a signed-in `claude` on PATH; it spends a
// cent or so of haiku. Prints "ok ..." / "FAIL ...". With KEEP=1 the hub
// stays up (its port and token printed) to look at in a browser.
//
//   bun test/tools/thinking_e2e.ts [BINARY] [WIREDIR]
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const keep = process.env.KEEP === "1";

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

async function until<T>(f: () => T | undefined, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(100);
  }
  return undefined;
}

const home = mkdtempSync(join(tmpdir(), "bp-thinking-e2e-"));
const root = mkdtempSync(join(tmpdir(), "bp-thinking-proj-"));
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--headless"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
  stdout: "ignore",
  stderr: "ignore",
});
const seen: any[] = [];
let ws: WebSocket;
let n = 0;
const send = (m: string, p: any) => { ws.send(W.encode(JSON.stringify({ id: ++n, m, p }))); return n; };
const setting = (k: string) => seen.filter((c) => c.$ === "SettingSet" && c.key === k).at(-1)?.value as string | undefined;
const parts = (v: string | undefined) => (v ?? "").split("/").map(Number);

try {
  if (!(await until(() => existsSync(join(home, "token")), 10000))) throw new Error("the hub did not start");
  await sleep(300);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r) => (ws.onopen = r));
  send("setting.set", { key: "agent.thinking", value: "on" });
  check("thinking turned on", !!(await until(() => setting("agent.thinking") === "on" ? true : undefined)));
  send("project.add", { path: root });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  // (a new project comes with a thread of its own: take the one made here)
  send("thread.create", { project: proj.id, title: "Thinking", env: "local", provider: "claude", model: "haiku" });
  const th = (await until(() => seen.find((c) => c.$ === "ThreadCreated" && c.title === "Thinking")))!.id;
  const done = (k: number) => until(() => seen.filter((c) => c.$ === "TurnChanged" && c.thread === th && c.state !== "running").length >= k ? true : undefined, 180000);
  const thoughts = () => seen.filter((c) => c.$ === "ActivityLogged" && c.thread === th && c.tone === "thinking");

  send("turn.start", { thread: th, text: "Is 1001 prime? Think it through, then answer in one line.", msg: "m-1" });
  check("the turn ends", !!(await done(1)));
  const t1 = thoughts();
  check("its thought is logged", t1.length >= 1 && t1.every((c) => c.summary.trim().length > 20 && c.act.startsWith("think:")), t1);
  const order = seen.filter((c) => c.thread === th && (c.$ === "MessagePosted" && c.role === "assistant" || c.tone === "thinking")).map((c) => c.$);
  check("before the reply", order[0] === "ActivityLogged" && order.includes("MessagePosted"), order);

  if (keep) {
    console.log(`hub http://127.0.0.1:${port}/#token=${readFileSync(join(home, "token"), "utf8").trim()} pid ${proc.pid} home ${home}`);
    process.exit(failed ? 1 : 0);
  }
  send("setting.set", { key: "agent.thinking", value: "off" });
  await until(() => setting("agent.thinking") === "off" ? true : undefined);
  send("turn.start", { thread: th, text: "Is 1003 prime? Think it through, then answer in one line.", msg: "m-2" });
  check("the next turn ends", !!(await done(2)));
  check("with thinking off, no thought is logged", thoughts().length === t1.length, thoughts().length);
} catch (e) {
  check("run", false, String(e));
}
proc.kill();
await proc.exited;
rmSync(home, { recursive: true, force: true });
rmSync(root, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
