// Riding mail, end to end (hub.bend's Ride, bothub.bend's Ride.mcp): a
// headless hub with a stand-in `codex` that calls Backplane's MCP
// endpoint through a long turn (test/tools/codex_mcp_standin.ts).
//   - five messages sent while the turn runs: the turn is never stopped or
//     started again, and the stand-in sees each one exactly once, in its
//     tool results
//   - a message sent after its last tool call: when the turn ends, the
//     next turn starts at once with it
//   - a message waiting when the hub is killed: the hub that starts next
//     ends the cut turn and starts one with it, once
//
//   bun test/tools/ride_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };

const root = mkdtempSync(join(tmpdir(), "bp-ride-"));
const fake = join(root, "bin"), log = join(root, "codex.log"), home = join(root, "home"), proj = join(root, "proj");
for (const d of [fake, home, proj]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(join(fake, "codex"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/codex_mcp_standin.ts")} "$@"\n`);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
for (const n of ["grok", "muse"]) writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
for (const n of ["codex", "claude", "grok", "muse"]) chmodSync(join(fake, n), 0o755);
const events = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l)) : []);

const port = freePort();
if (port === 3787) throw new Error("refusing the live port");
const start = () => Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    RIDE_LOG: log, FAKE_LOG: join(root, "claude.log"), BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, `hub-${Date.now()}.err`)),
});
const seen: any[] = [];
let ws: WebSocket;
let id = 0;
const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
const connect = async () => {
  let up = false;
  for (let i = 0; i < 200 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
  check(up, "the hub is up");
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
};
let proc = start();
try {
  await connect();
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  await sleep(500);
  send("thread.create", { project: pc.id, title: "ride", env: "local", provider: "codex" });
  const th = (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "ride")))?.id as string;
  const turns = () => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const acts = (kind: string) => seen.filter((x) => x.$ === "ActivityLogged" && x.thread === th && x.kind === kind);

  send("turn.start", { thread: th, text: "ride-long go", msg: "r-0" });
  await until(10000, () => events().find((e) => e.ev === "result"));
  // five messages while the turn runs (no mode: delivered like any message)
  for (let k = 1; k <= 5; k++) {
    send("turn.start", { thread: th, text: `RIDE-MSG-${k} please note`, msg: `r-${k}` });
    await sleep(900);
  }
  await until(20000, () => events().find((e) => e.ev === "last"));
  send("turn.start", { thread: th, text: "RIDE-MSG-6 after the last call", msg: "r-6" });
  await until(20000, () => events().filter((e) => e.ev === "start").length >= 2 && turns().at(-1) === "completed" && turns().length >= 4 ? true : undefined);

  const ev = events();
  const starts = ev.filter((e) => e.ev === "start");
  const results = ev.filter((e) => e.ev === "result").map((e) => String(e.text));
  check(results.length > 0 && results.every((t) => !t.startsWith("ERROR")), `the stand-in's tool calls answered (${results.length})`);
  for (let k = 1; k <= 5; k++) {
    const n = results.filter((t) => t.includes(`RIDE-MSG-${k} `)).length;
    check(n === 1, `message ${k} seen exactly once in tool results (${n})`);
  }
  check(results.some((t) => t.includes("Incoming messages")), "as an incoming messages block");
  check(!results.some((t) => t.includes("RIDE-MSG-6")), "message 6 was in no tool result");
  check(starts.length === 2, `two processes, so the long turn was never restarted (${starts.length})`);
  check(starts[0]?.prompt.includes("ride-long") && !/RIDE-MSG-[1-5]/.test(starts[0]?.prompt ?? ""), "the first started with its own message only");
  check(String(starts[1]?.prompt ?? "").includes("RIDE-MSG-6") && !/RIDE-MSG-[1-5]/.test(starts[1]?.prompt ?? ""), `the next turn started with message 6 alone (${String(starts[1]?.prompt ?? "").slice(-80)})`);
  check(acts("steer").length === 0, "no steer");
  check(JSON.stringify(turns()) === JSON.stringify(["running", "completed", "running", "completed"]), `turns: ${turns().join(",")}`);
  const rides = acts("ride").map((a) => String(a.summary));
  check(rides.some((s) => s.includes("into the running turn")) && rides.some((s) => s.includes("with a new turn")), `delivery logged (${rides.join(" | ")})`);
  const posted = seen.filter((x) => x.$ === "MessagePosted" && x.thread === th && /RIDE-MSG/.test(String(x.text))).length;
  check(posted === 6, `all six posted at once (${posted})`);

  // a message waiting when the hub is killed
  send("thread.create", { project: pc.id, title: "cut", env: "local", provider: "codex" });
  const t2 = (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "cut")))?.id as string;
  send("turn.start", { thread: t2, text: "ride-quiet go", msg: "q-0" });
  const quiet: any = await until(10000, () => events().find((e) => e.ev === "start" && String(e.prompt).includes("ride-quiet")));
  send("turn.start", { thread: t2, text: "RIDE-MSG-7 across the restart", msg: "q-1" });
  await until(5000, () => seen.find((x) => x.$ === "MessagePosted" && x.thread === t2 && String(x.text).includes("RIDE-MSG-7")));
  await sleep(500);
  proc.kill("SIGKILL");
  await proc.exited;
  try { process.kill(quiet.pid, "SIGKILL"); } catch {}
  proc = start();
  await connect();
  await until(20000, () => events().some((e) => e.ev === "start" && String(e.prompt).includes("RIDE-MSG-7")) ? true : undefined);
  await sleep(3000);
  const with7 = events().filter((e) => e.ev === "start" && String(e.prompt).includes("RIDE-MSG-7")).length;
  check(with7 === 1, `after the restart a turn started with message 7, once (${with7})`);
  const in7 = events().filter((e) => e.ev === "result" && String(e.text).includes("RIDE-MSG-7")).length;
  check(in7 === 0, `and no tool result carried it again (${in7})`);
} finally {
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
