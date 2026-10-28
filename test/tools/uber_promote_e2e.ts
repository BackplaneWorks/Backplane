// End to end: making a bot the uberbot on one headless hub (docs/uber.md).
// A bot with memory, a routine and a room is promoted (bots.uber): the hub
// backs it up first (<home>/backups/uber-*: its home folder and the log
// lines that name it), then logs UberSet; nothing of the bot is lost. Then
// it is taken back, and a bot made with uber: true (the tour's "Create my
// uberbot") is promoted too. Agents are stand-ins. Prints "ok ..." /
// "FAIL ..." lines.
//
//   bun test/tools/uber_promote_e2e.ts [BINARY] [WIREDIR]
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

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-uber-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

const home = join(root, "home");
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: "uber-e2e", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});

const seen: any[] = [];
const replies = new Map<number, any>();
let n = 0;
let ws: WebSocket;

async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}

async function rpc(m: string, p: object): Promise<any> {
  const id = ++n;
  ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(15000, () => replies.get(id));
}

const change = (tag: string, f: (c: any) => boolean = () => true) => until(15000, () => seen.find((c) => c.$ === tag && f(c)));

try {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) seen.push(c);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });

  const kit = await rpc("bots.create", { name: "kit", persona: "the main one" });
  const pip = await rpc("bots.create", { name: "pip" });
  check("two bots made", kit?.ok && pip?.ok, [kit, pip]);
  await change("WorktreeSet", (c) => JSON.stringify(c).includes(`/bots/${kit.bot}`));
  const mem = await rpc("bots.remember", { bot: kit.bot, key: "likes", kind: "pref", text: "short answers", tags: "" });
  const rt = await rpc("bots.routine", { bot: kit.bot, name: "digest", cron: "0 9 * * *", prompt: "sum up", on: true });
  const room = await rpc("bots.room", { name: "ops", members: `${kit.bot}\n${pip.bot}` });
  check("memory, a routine and a room", mem?.ok && rt?.ok && room?.ok, [mem, rt, room]);

  const far = await rpc("bots.uber", { bot: "p1~b9" });
  check("a bot elsewhere is refused", far && !far.ok, far);

  const up = await rpc("bots.uber", { bot: kit.bot });
  check("bots.uber answers ok with a uid", up?.ok && typeof up.uid === "string" && up.uid.startsWith("u"), up);
  const set = await change("UberSet", (c) => c.bot === kit.bot);
  check("UberSet is logged", !!set, seen.map((c) => c.$).slice(-6));
  const backups = existsSync(join(home, "backups")) ? readdirSync(join(home, "backups")).filter((d) => d.startsWith("uber-") && d.endsWith(kit.bot)) : [];
  check("a backup folder was made", backups.length === 1, backups);
  const dir = join(home, "backups", backups[0] ?? "x");
  const lines = existsSync(join(dir, "events.jsonl")) ? readFileSync(join(dir, "events.jsonl"), "utf8").trim().split("\n") : [];
  check("the backup holds the bot's log lines", lines.some((l) => l.includes("MemorySaved")) && lines.some((l) => l.includes("RoutineSet")) && lines.some((l) => l.includes("BotSet")), lines.length);
  check("the backup holds its home folder", existsSync(join(dir, "home", ".git")));
  check("nothing was forgotten or deleted",
    !seen.some((c) => ["MemoryForgot", "RoutineDeleted", "RoomDeleted", "BotDeleted"].includes(c.$)), seen.map((c) => c.$));

  const again = await rpc("bots.uber", { bot: kit.bot });
  await sleep(500);
  check("promoting it again changes nothing", again?.ok && seen.filter((c) => c.$ === "UberSet").length === 1, again);

  const down = await rpc("bots.uber", { bot: "" });
  check("taking it back logs an empty UberSet", down?.ok && !!(await change("UberSet", (c) => c.bot === "")), down);

  const neo = await rpc("bots.create", { name: "neo", uber: true });
  check("a bot made with uber: true", neo?.ok, neo);
  check("is promoted after its backup", !!(await change("UberSet", (c) => c.bot === neo?.bot)), seen.filter((c) => c.$ === "UberSet"));
} catch (e) {
  check("no exception", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
