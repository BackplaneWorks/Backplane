// A turn whose last lifecycle line never comes (hub.bend's Stuck.*), end to
// end: a headless hub with the stand-in `claude` in mode:lostend (the
// command answers and has its result, but its lifecycle never completes;
// the process stays, silent). With BACKPLANE_STUCK_S=4 and a 2 s tick the
// turn ends as completed, with a "stuck" note, and the process is kept;
// a second message then works as usual in the same process.
//
//   bun test/tools/stuck_e2e.ts [BINARY] [WIREDIR]      (port 3794)
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3794);
if (PORT === 3787) throw new Error("refusing the live port");
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");

const root = mkdtempSync(join(tmpdir(), "bp-stuck-"));
const fake = join(root, "bin"), log = join(root, "claude.log"), home = join(root, "home"), proj = join(root, "proj");
for (const d of [fake, home, proj]) mkdirSync(d);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
writeFileSync(join(home, "events.jsonl"), "");
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
    BACKPLANE_STUCK_S: "4", BACKPLANE_WAKE_TICK_MS: "2000", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")),
});
try {
  let up = false;
  for (let i = 0; i < 200 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
  check(up, "the hub is up");
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  await sleep(1000);
  send("thread.create", { project: pc.id, title: "lost end", env: "local", provider: "claude" });
  const T = (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "lost end")))?.id as string;
  const t0 = Date.now();
  send("turn.start", { thread: T, text: "mode:lostend go", msg: "l-1" });
  await until(15000, () => (read(log).includes("LOSTEND") ? true : undefined));
  const turns = () => seen.filter((x) => x.$ === "TurnChanged" && x.thread === T).map((x) => x.state);
  await sleep(2000);
  check(turns().at(-1) === "running", `right after the lost end the turn still runs (${turns().join(",")})`);
  const ended = await until(30000, () => (turns().at(-1) === "completed" ? true : undefined));
  check(!!ended, `after the silence the turn is ended as completed (${((Date.now() - t0) / 1000).toFixed(1)} s; ${turns().join(",")})`);
  check(!!seen.find((x) => x.$ === "ActivityLogged" && x.thread === T && x.kind === "stuck"), "with a note saying why");
  const starts = () => read(log).split("\n").filter((l) => l.startsWith("MCP ")).length; // agent processes (each logs its MCP URL)
  const before = starts();
  send("turn.start", { thread: T, text: "again", msg: "l-2" });
  const again = await until(20000, () => (turns().filter((s) => s === "completed").length >= 2 ? true : undefined));
  check(!!again, "a second message has its turn, which ends the same way");
  check(before === 1 && starts() === before, `in the same process (${before} -> ${starts()})`);
} finally {
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
  await proc.exited;
  for (const m of read(log).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
