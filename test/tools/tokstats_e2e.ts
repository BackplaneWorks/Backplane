// End to end: token stats from a real Claude turn. Starts a headless hub on
// a temporary home and a free port, runs two short turns on Claude (haiku,
// in an empty project), and checks the readings the composer's token panel
// shows (usage.bend's Ts): the last turn (turn.last), Claude's speed
// (turn.speed: the second turn's share of API time and output) and its
// models (turn.models). Needs a signed-in `claude` on PATH; it spends a few
// cents of haiku. Prints "ok ..." / "FAIL ...". With KEEP=1 the hub stays up
// (its port and token printed) to look at in a browser.
//
//   bun test/tools/tokstats_e2e.ts [BINARY] [WIREDIR]
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

const home = mkdtempSync(join(tmpdir(), "bp-tokstats-e2e-"));
const root = mkdtempSync(join(tmpdir(), "bp-tokstats-proj-"));
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--headless"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
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
  send("project.add", { path: root });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: proj.id, title: "Tokens", env: "local", provider: "claude", model: "haiku" });
  // (a new project comes with a thread of its own: take the one made here)
  const th = (await until(() => seen.find((c) => c.$ === "ThreadCreated" && c.title === "Tokens")))!.id;
  const done = (k: number) => until(() => seen.filter((c) => c.$ === "TurnChanged" && c.thread === th && c.state !== "running").length >= k ? true : undefined, 180000);

  send("turn.start", { thread: th, text: "Say hi in three words.", msg: "m-1" });
  check("the first turn ends", !!(await done(1)));
  await until(() => setting(`turn.speed.${th}`), 10000);
  const first = parts(setting(`turn.speed.${th}`));
  check("a session the hub saw start: the first turn's speed is told", first[2] > 0 && first[3] > 0, first);

  send("turn.start", { thread: th, text: "Count from 1 to 40, separated by spaces.", msg: "m-2" });
  check("the second turn ends", !!(await done(2)));
  await until(() => parts(setting(`turn.speed.${th}`))[5] > first[5] ? true : undefined, 10000);
  const sp = parts(setting(`turn.speed.${th}`));
  check("the second turn's API time is the rise", sp[2] === sp[0] - first[0] && sp[2] > 0, { first, sp });
  check("and its output too", sp[3] === sp[1] - first[1] && sp[3] > 0, { first, sp });
  check("the thread's timed totals add both turns", sp[5] === first[2] + sp[2] && sp[6] === first[3] + sp[3], { first, sp });
  const rate = (sp[3] * 1000) / sp[2];
  check(`a believable speed (${rate.toFixed(1)} tok/s)`, rate > 5 && rate < 2000, sp);
  // the last turn (turn.last) is folded from the turn's TurnUsage by every client
  const tu = seen.filter((c) => c.$ === "TurnUsage" && c.thread === th);
  check("each turn's usage is told, with its cost and model", tu.length === 2 && tu.every((c) => c.output > 0 && c.known.includes("c") && c.model.includes("haiku")), tu);
  const mu = setting(`turn.models.${th}`) ?? "";
  check("Claude's models this session", /^claude-[^|]+\|\d+\|[1-9]\d*\|/.test(mu), mu);

  send("setting.set", { key: "ui.tokstats", value: "on" });
  check("the opt-in is a shared setting", !!(await until(() => setting("ui.tokstats") === "on" ? true : undefined)));
  if (keep) {
    console.log(`hub http://127.0.0.1:${port}/#token=${readFileSync(join(home, "token"), "utf8").trim()} pid ${proc.pid} home ${home}`);
    process.exit(failed ? 1 : 0);
  }
} catch (e) {
  check("run", false, String(e));
}
proc.kill();
await proc.exited;
rmSync(home, { recursive: true, force: true });
rmSync(root, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
