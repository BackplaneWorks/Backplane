// A log that cannot be written (core/diskq.bend): steps wait instead of
// being dropped, every client is told, effects that act outside the hub
// wait for the flush, and once there is room every waiting line is written
// in order and the hub boots from it.
//
//   bun test/tools/disk_full_e2e.ts BINARY WIREDIR
//
// No root is needed: the hub runs with SIGXFSZ ignored, and its file size
// limit (prlimit --fsize) is lowered to the log's size, so every append
// fails with EFBIG as it would with ENOSPC; raising it again is the disk
// with room. A stand-in claude logs each turn it is given.
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binArg, wire] = process.argv.slice(2);
if (!binArg || !wire) throw new Error("usage: disk_full_e2e.ts BINARY WIREDIR");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const bin = resolve(binArg);
let fail = 0;
const check = (ok: unknown, what: string, got?: unknown) => { console.log(`${ok ? "ok" : "FAIL"} ${what}${ok || got === undefined ? "" : ": " + JSON.stringify(got).slice(0, 300)}`); if (!ok) fail++; };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const until = async <T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> => { const end = Date.now() + ms; for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); } };
const root = mkdtempSync(join(process.env.STORE_UP_DIR ?? tmpdir(), "bp-disk-full-"));
const home = join(root, "home"), fake = join(root, "bin"), proj = join(root, "proj"), started = join(root, "started");
for (const d of [home, fake, proj]) mkdirSync(d);
// it logs a turn it was given (not the hub's probes of the CLI)
writeFileSync(join(fake, "claude"), `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      echo turn >> ${JSON.stringify(started)}
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":[],"model":"fake","permissionMode":"default"}'
      printf '%s\\n' '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"done"}]},"session_id":"s1"}'
      printf '%s\\n' '{"type":"result","is_error":false,"result":"done","session_id":"s1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const now = Math.floor(Date.now() / 1000);
const ev = ($: string, f: object) => JSON.stringify({ $, ...f, at: now });
writeFileSync(join(home, "events.jsonl"), [ev("ProjectCreated", { id: "p", title: "P", root: proj }),
  ev("ThreadCreated", { id: "t1", project: "p", title: "first", env: "local", branch: "", worktree: proj, provider: "claude", model: "" }),
  ev("SettingSet", { key: "restart.continue", value: "off" })].join("\n") + "\n");
const log = join(home, "events.jsonl");
const size = () => statSync(log).size;
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const env = { PATH: `${fake}:/usr/bin:/bin`, HOME: root, DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_PEERS: "", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_GOOGLE_BAKE: "0" };
type C = { ws: WebSocket; seen: any[]; updates: any[]; replies: Map<number, any>; n: number };
const hub = async () => {
  const port = freePort();
  // SIGXFSZ ignored: a write past the file size limit fails (EFBIG) instead of ending the hub
  const proc = Bun.spawn(["sh", "-c", 'trap "" XFSZ; exec "$@"', "sh", bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], { env, stdout: "ignore", stderr: "pipe" });
  // the hub's log is written by this test, not the hub: the file size limit
  // below would stop the hub writing a file of its own
  (async () => { const out = join(root, `hub-${port}.log`); for await (const b of proc.stderr as any) appendFileSync(out, b); })();
  let up = false;
  for (let i = 0; i < 300 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  if (!up) throw new Error("hub did not start");
  const c: C = { ws: new WebSocket(`ws://127.0.0.1:${port}/ws`), seen: [], updates: [], replies: new Map(), n: 0 };
  c.ws.binaryType = "arraybuffer";
  c.ws.onmessage = (e) => { const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))); for (const x of o.items ?? []) c.seen.push(x); if (o.t === "update") c.updates.push(o); if (o.t === "reply") c.replies.set(Number(o.id), o); };
  await new Promise((r, j) => { c.ws.onopen = r; c.ws.onerror = j; });
  const rpc = async (m: string, p: object) => { const id = ++c.n; c.ws.send(W.encode(JSON.stringify({ id, m, p }))); return await until(15000, () => c.replies.get(id)); };
  const stop = async () => { c.ws.close(); proc.kill("SIGTERM"); await proc.exited; };
  return { proc, c, rpc, stop, port };
};
const limit = (pid: number, v: string) => Bun.spawnSync(["prlimit", "--pid", String(pid), `--fsize=${v}:`]).exitCode;
const titles = () => readFileSync(log, "utf8").split("\n").filter((l) => l.includes('"ThreadRenamed"')).map((l) => JSON.parse(l).title);
try {
  let h = await hub();
  const r0 = await h.rpc("thread.rename", { thread: "t1", title: "r0" });
  check(r0?.ok && titles().join() === "r0", "a rename is written while there is room");
  // the hub's real pid is the exec'd one (sh exec keeps the pid)
  const before = size();
  check(limit(h.proc.pid, String(before)) === 0, "the hub's file size limit is the log's size (every append fails)");
  const accepted: boolean[] = [];
  for (const t of ["r1", "r2", "r3"]) accepted.push(!!(await h.rpc("thread.rename", { thread: "t1", title: t }))?.ok);
  check(accepted.every((x) => x), "renames are still taken (they wait instead of being dropped)", accepted);
  check(size() === before, "nothing reached the log meanwhile", { before, now: size() });
  check(h.c.seen.some((x) => x.$ === "ThreadRenamed" && x.title === "r3"), "clients see the waiting changes");
  const note = await until(10000, () => h.c.updates.find((u) => String(u.error ?? "").startsWith("Disk full")));
  check(note, "every client is told: Disk full, changes waiting", h.c.updates.map((u) => u.error));
  // an agent's tool call that writes is refused; one that reads is answered
  const tool = async (name: string, args: object) => (await fetch(`http://127.0.0.1:${h.port}/mcp/t1`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) })).json();
  const wrote = await tool("thread_update", { title: "via-tool" });
  check(JSON.stringify(wrote).includes("Disk full"), "a tool call that writes is refused while steps wait", wrote);
  const read = await tool("thread_list", {});
  check(!JSON.stringify(read).includes("Disk full") && !read.error, "a tool call that only reads is answered", read.error);
  const turn = await h.rpc("turn.start", { thread: "t1", text: "hello", msg: "m-df" });
  check(turn?.ok, "a turn under the soft bound is taken", turn);
  await sleep(3000);
  check(!existsSync(started), "the agent gets no turn while its step waits (an effect outside the hub)");
  // past the soft bound (5,000 waiting lines) new turns are refused
  // (50 at a time: a client that floods the hub is cut off)
  const tq = Date.now();
  for (let i = 0; i < 5000; i += 50) {
    for (let j = i; j < i + 50; j++) h.c.ws.send(W.encode(JSON.stringify({ id: 100000 + j, m: "thread.rename", p: { thread: "t1", title: `w${j}` } })));
    await until(60000, () => h.c.replies.get(100000 + i + 49));
  }
  console.log(`  5,000 renames taken while waiting in ${((Date.now() - tq) / 1000).toFixed(1)} s`);
  const refused = await h.rpc("thread.create", { project: "p", title: "refused" });
  check(refused && !refused.ok && String(refused.error).includes("Disk full"), "past the soft bound a new thread is refused, saying why", refused);
  // room again: the next flush (backoff 2 s, 4 s, ...) writes everything in order
  check(limit(h.proc.pid, "unlimited") === 0, "room again");
  const want = ["r0", "r1", "r2", "r3", ...Array.from({ length: 5000 }, (_, i) => `w${i}`)].join();
  const flushed = await until(120000, () => titles().join() === want);
  check(flushed, "every waiting line (5,000 and more) is written, in order", titles().length);
  check(await until(20000, () => h.c.updates.some((u) => u.error === "" && String(u.state ?? "").includes("saved"))), "clients are told it is saved");
  check(await until(20000, () => existsSync(started)), "the held agent start runs after the flush: the agent gets its turn");
  check(readFileSync(log, "utf8").includes('"m-df"'), "the turn's message is in the log");
  await h.stop();
  // the log and the store agree: the next boot is a hot boot with the last title
  h = await hub();
  const hl = readFileSync(join(root, `hub-${h.port}.log`), "utf8");
  check(hl.includes("hot boot from") && !hl.includes("full replay"), "the next boot is a hot boot", hl.split("\n").filter((l) => /hot boot|full replay|store/.test(l)).slice(0, 3));
  check(await until(10000, () => h.c.seen.some((x) => x.$ === "ThreadRenamed" && x.title === "w4999")), "and shows the last title");
  await h.stop();
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
