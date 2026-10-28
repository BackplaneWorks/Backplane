// End to end: an uberbot promoted before the op log existed (an UberSet in
// the log, no UberOp) is seeded once (docs/uber.md, ubersync.bend's
// Ub.init): at the hub's first tick it logs its first ops (who, each memory,
// each routine, the lease at term 1, its bind), and at the next tick none
// again. One headless hub on a temporary home; stand-in agents.
//
//   bun test/tools/uber_seed_e2e.ts [BINARY] [WIREDIR]
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane-serve", wire = "build/wire"] = process.argv.slice(2);
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

const root = mkdtempSync(join(tmpdir(), "bp-uber-seed-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

// the log of a hub that promoted its bot under the first uberbot release:
// a bot with three memories and two routines, then UberSet, and no ops
const home = join(root, "home");
mkdirSync(home);
const t0 = 1790000000;
const lines = [
  { $: "ThreadCreated", id: "t1", project: "", title: "Bub", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: t0 },
  { $: "BotSet", id: "b1", name: "Bub", thread: "t1", look: 7, persona: "chief of staff", at: t0 },
  { $: "MemorySaved", bot: "b1", key: "home", kind: "user", text: "Toronto", tags: "", at: t0 + 1 },
  { $: "MemorySaved", bot: "b1", key: "tone", kind: "pref", text: "short", tags: "", at: t0 + 2 },
  { $: "MemorySaved", bot: "b1", key: "board", kind: "fact", text: "rev C", tags: "", at: t0 + 3 },
  { $: "RoutineSet", id: "o1", bot: "b1", name: "digest", cron: "0 9 * * 1-5", prompt: "sum up", on: true, at: t0 + 4 },
  { $: "RoutineSet", id: "o2", bot: "b1", name: "weekly", cron: "0 17 * * 5", prompt: "week", on: false, at: t0 + 5 },
  { $: "UberSet", bot: "b1", uid: "u1790000010-abcd", at: t0 + 10 },
  { $: "SettingSet", key: "restart.continue", value: "off", at: t0 + 11 },
];
writeFileSync(join(home, "events.jsonl"), lines.map((l) => JSON.stringify(l)).join("\n") + "\n");

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: "seed", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
  stdout: "ignore",
  stderr: "ignore",
});

const seen: any[] = [];
async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
}
const ops = () => seen.filter((x) => x.$ === "UberOp").map((x) => ({ ...x, body: JSON.parse(x.op) }));

try {
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const x of o.items ?? []) seen.push(x);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });

  const id = await until(15000, () => seen.find((x) => x.$ === "SettingSet" && x.key === "machine.id")?.value);
  check("the hub makes its machine id", !!id, id);
  const lease = await until(90000, () => ops().find((o) => o.body.k === "lease"));
  check("within a tick it seeds the uberbot's ops", !!lease, ops().map((o) => o.body.k));
  const got = ops();
  const kinds = got.map((o) => o.body.k);
  check("under the uid it had", got.every((o) => o.uid === "u1790000010-abcd"), got.map((o) => o.uid));
  check("who, each memory, each routine, the lease and the bind",
    kinds.filter((k) => k === "who").length === 1 && kinds.filter((k) => k === "mem").length === 3 &&
    kinds.filter((k) => k === "routine").length === 2 && kinds.filter((k) => k === "lease").length === 1 && kinds.filter((k) => k === "bind").length === 1, kinds);
  check("the memories' texts", ["Toronto", "short", "rev C"].every((t) => got.some((o) => o.body.k === "mem" && o.body.text === t)), got.map((o) => o.body));
  check("the lease is this machine's, term 1", lease?.body.holder === id && lease?.body.term === 1, lease?.body);
  check("numbered 1..n from this machine", got.every((o, i) => o.origin === id && o.n === i + 1), got.map((o) => [o.origin, o.n]));
  const n = got.length;
  await sleep(75000);
  check("and not again at the next tick", ops().length === n, [n, ops().length]);
  check("nothing of the bot is lost", !seen.some((x) => ["MemoryForgot", "RoutineDeleted", "BotDeleted"].includes(x.$)), seen.map((x) => x.$));
  ws.close();
} catch (e) {
  check("no exception", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "uber seed e2e: FAIL" : "uber seed e2e: ok");
process.exit(failed ? 1 : 0);
