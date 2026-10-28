// A deploy waits for Backplane MCP calls in flight, end to end
// (core/deploy.bend's Open.mcp; server.bend's Mcp.ask.raw). A headless hub
// installed in a temp prefix, with the stand-in `codex`
// (test/tools/codex_mcp_standin.ts) and the stand-in `claude`:
//   - thread Z (Claude) streams a turn of about 20 s;
//   - thread C (Codex) sits in an MCP tool call, thread_wait on Z;
//   - thread D (Claude) calls deploy_restart with a staged package.
// The restart waits while C's call is open (deploy_status lists C), and
// goes once its answer is written; D's own deploy call never holds it.
//
//   bun test/tools/deploy_mcp_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
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
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); }
};
const alive = (pid: number) => {
  try { process.kill(pid, 0); } catch { return false; }
  try { return !/^\d+ \(.*\) Z/.test(readFileSync(`/proc/${pid}/stat`, "utf8")); } catch { return false; }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };

const root = mkdtempSync(join(tmpdir(), "bp-deploy-mcp-"));
const fake = join(root, "bin"), clog = join(root, "claude.log"), xlog = join(root, "codex.log"), home = join(root, "home"), proj = join(root, "proj"), prefix = join(root, "prefix");
for (const d of [fake, home, proj, prefix, join(prefix, "web")]) mkdirSync(d);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
writeFileSync(join(fake, "codex"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/codex_mcp_standin.ts")} "$@"\n`);
for (const n of ["grok", "muse"]) writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
for (const n of ["claude", "codex", "grok", "muse"]) chmodSync(join(fake, n), 0o755);
writeFileSync(join(home, "events.jsonl"), "");
copyFileSync(resolve(bin), join(prefix, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);
writeFileSync(join(prefix, "LICENSE"), "orig");
const stage = join(root, "stage");
mkdirSync(join(stage, "web"), { recursive: true });
copyFileSync(resolve(bin), join(stage, "backplane"));
chmodSync(join(stage, "backplane"), 0o755);
writeFileSync(join(stage, "LICENSE"), "new");
const events = () => read(xlog).split("\n").filter(Boolean).map((l) => JSON.parse(l));

const PORT = freePort();
if (PORT === 3787) throw new Error("refusing the live port");
const spawnHub = (n: number) => Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    FAKE_LOG: clog, RIDE_LOG: xlog, BP_EOF_FINISH: "1", BP_STREAM_N: "200", BP_STREAM_MS: "100", BP_DEPLOY_ARGS: JSON.stringify({ stage, note: "deploy mcp e2e" }),
    BACKPLANE_WAKE_TICK_MS: "2000", BACKPLANE_DEPLOY_LOCKS: join(root, "none.lock"), BACKPLANE_LOCK_DIR: join(root, "locks"),
    BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, `hub${n}.err`)),
});
let proc = spawnHub(1);
mkdirSync(join(root, "locks"));
const up = async () => {
  for (let i = 0; i < 200; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) return true; } catch {} await sleep(100); }
  return false;
};
const seen: any[] = [];
const connect = async () => {
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};
try {
  check(await up(), "the hub is up");
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string, provider: string) => {
    await sleep(800);
    c.send("thread.create", { project: pc.id, title, env: "local", provider });
    return (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
  };
  const Z = await made("sleeper", "claude");
  const C = await made("waiter", "codex");
  const D = await made("deployer", "claude");
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  c.send("turn.start", { thread: Z, text: "mode:stream go w", msg: "z-1" });
  await until(15000, () => turns(Z).includes("running") ? true : undefined);
  c.send("turn.start", { thread: C, text: `ride-cut:${Z} go`, msg: "c-1" });
  check(!!(await until(15000, () => events().find((e) => e.ev === "start"))), "C's codex turn runs and calls thread_wait");
  await sleep(1500);
  c.send("turn.start", { thread: D, text: "mode:deploy go", msg: "d-1" });
  const asked = await until(15000, () => seen.find((x) => x.$ === "SettingSet" && x.key === "deploy.wait" && x.value) ? true : undefined);
  check(!!asked, "the deploy is asked for");
  await sleep(4000);
  check(read(join(prefix, "LICENSE")) === "orig", "it waits while C's MCP call is open");
  const tok = read(join(home, "token")).trim();
  const st = await (await fetch(`http://127.0.0.1:${PORT}/mcp/deploy?token=${tok}`, { method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "deploy_status", arguments: {} } }) })).text();
  check(st.includes(C), `deploy_status lists C's call (${st.slice(0, 300)})`);
  let swapAt = 0;
  const swapped = await until(90000, () => (read(join(prefix, "LICENSE")) === "new" ? (swapAt = Date.now(), true) : undefined));
  check(!!swapped, "the restart goes once the call returned");
  const cut = events().find((e) => e.ev === "cut");
  check(!!cut && String(cut.how).startsWith("answered"), `C's call was answered, not cut (${String(cut?.how ?? "").slice(0, 120)})`);
  check(!!cut && cut.at <= swapAt, `and before the swap (${cut ? swapAt - cut.at : "?"} ms)`);
  check(turns(Z).includes("completed"), "Z's turn had ended first");
  await sleep(1500);
  check(await up(), "the hub is up again after its restart");
} finally {
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
  await proc.exited;
  for (const e of events()) if (e.ev === "start") { try { process.kill(Number(e.pid), "SIGKILL"); } catch {} }
  for (const m of read(clog).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
