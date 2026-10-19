// End to end: a bot's project_create (docs/bots.md) on a headless hub. The
// call waits on an approval card; declining makes nothing, accepting makes
// the folder and the project, and an existing project's folder answers
// that project. Prints "ok ..." / "FAIL ..." lines.
//
//   bun test/tools/newproj_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire).
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
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
async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// no real agents: every provider exits at once
const root = mkdtempSync(join(tmpdir(), "bp-newproj-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const user = join(root, "user");
mkdirSync(user);
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: user, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: process.env.E2E_LOG ? "inherit" : "ignore",
});

try {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) seen.push(c);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let n = 0;
  const rpc = async (m: string, p: object) => {
    const id = ++n;
    ws.send(W.encode(JSON.stringify({ id, m, p })));
    return await until(10000, () => replies.get(id));
  };
  const change = (tag: string, f: (c: any) => boolean = () => true) => until(8000, () => seen.find((c) => c.$ === tag && f(c)));

  const bot = await rpc("bots.create", { name: "miso" });
  const set = await change("BotSet", (c) => c.id === bot?.bot);
  const thread: string = set?.thread ?? "";
  check("a bot to call from", !!thread, bot);

  const call = (th: string, args: object) =>
    fetch(`http://127.0.0.1:${port}/mcp/${th}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 9, method: "tools/call", params: { name: "project_create", arguments: args } }),
    }).then((r) => r.json());
  const text = (r: any) => String(r?.result?.content?.[0]?.text ?? "");

  const tools = await fetch(`http://127.0.0.1:${port}/mcp/${thread}`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
  }).then((r) => r.json());
  check("a bot sees project_create", JSON.stringify(tools).includes("\"project_create\""));

  // declined: nothing is made
  const no = call(thread, { path: "fan-no" });
  const ask1 = await change("AskOpened", (c) => c.tool === "project_create" && String(c.detail).includes("fan-no"));
  check("it opens an approval card first", !!ask1 && ask1.kind === "approval", ask1);
  check("nothing is made while it waits", !existsSync(join(user, "fan-no")) && !seen.some((c) => c.$ === "ProjectCreated" && c.id !== "scratch"));
  await rpc("ask.answer", { id: ask1?.id, answer: "decline" });
  const r1 = await no;
  check("declined, the bot is told", text(r1).includes("declined") && r1?.result?.isError === true, r1);
  await sleep(300);
  check("declined, no folder and no project", !existsSync(join(user, "fan-no")) && !seen.some((c) => c.$ === "ProjectCreated" && c.id !== "scratch"));

  // accepted: the folder and the project
  const yes = call(thread, { path: "boards/fan", name: "Fan" });
  const ask2 = await change("AskOpened", (c) => c.tool === "project_create" && String(c.detail).includes("boards/fan"));
  check("each call asks again", !!ask2 && ask2.id !== ask1?.id, ask2);
  await rpc("ask.answer", { id: ask2?.id, answer: "accept" });
  const r2 = await yes;
  const made = await change("ProjectCreated", (c) => c.title === "Fan");
  check("accepted, the project is made", !!made && made.root === join(user, "boards/fan"), [made, r2]);
  check("accepted, the folder is made", existsSync(join(user, "boards/fan")));
  check("accepted, the bot gets its id", text(r2).includes(made?.id ?? "?") && text(r2).includes("created"), r2);

  // the same folder again answers that project
  const again = call(thread, { path: join(user, "boards/fan") });
  const ask3 = await change("AskOpened", (c) => c.tool === "project_create" && c.id !== ask2?.id && c.id !== ask1?.id);
  await rpc("ask.answer", { id: ask3?.id, answer: "accept" });
  const r3 = await again;
  check("a folder that is a project answers it", text(r3).includes("already a project") && text(r3).includes(made?.id ?? "?"), r3);
  check("and makes no second project", seen.filter((c) => c.$ === "ProjectCreated" && c.id !== "scratch").length === 1);

  // not a bot
  const r4 = await call("t-nobody", { path: "x" });
  check("only bots have it", text(r4).includes("only bots"), r4);
  ws.close();
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
