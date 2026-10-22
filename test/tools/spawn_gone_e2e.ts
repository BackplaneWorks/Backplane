// A thread whose folder is gone when its agent starts (a removed worktree:
// posix_spawn's chdir answers ENOENT, which read as a missing program): the
// turn fails and both the thread's activity and the hub's log name the
// folder. A headless hub with a stand-in `claude`, an empty home.
//
//   bun test/tools/spawn_gone_e2e.ts BINARY WIREDIR
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || more === "" ? "" : ": " + JSON.stringify(more).slice(0, 300)}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); }
};

const root = mkdtempSync(join(tmpdir(), "bp-gone-"));
const fake = join(root, "bin"), home = join(root, "home"), proj = join(root, "proj");
mkdirSync(fake); mkdirSync(proj);
writeFileSync(join(fake, "claude"), "#!/bin/sh\nexec cat >/dev/null\n");
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const port = (() => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; })();
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, BACKPLANE_TELEMETRY: "off" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")),
});
try {
  for (let i = 0; i < 300; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(100); }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => { try { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); if (o.t !== "live") for (const c of o.items ?? []) seen.push(c); } catch {} };
  await new Promise((r) => (ws.onopen = r));
  let n = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated" && c.root === proj));
  send("thread.create", { project: pc.id, title: "gone", env: "local", provider: "claude" });
  const th = (await until(10000, () => seen.find((c) => c.$ === "ThreadCreated" && c.title === "gone")) as any).id;
  // the folder goes before the agent starts
  rmSync(proj, { recursive: true, force: true });
  send("turn.start", { thread: th, text: "hi" });
  const failed = await until(15000, () => seen.find((c) => c.$ === "TurnChanged" && c.thread === th && /fail/i.test(c.state)));
  check(!!failed, "the turn fails");
  const act = seen.find((c) => c.$ === "ActivityLogged" && c.thread === th && c.kind === "spawn");
  check(!!act && String(act.summary).includes(`its folder ${proj} is gone`), "the thread's activity names the missing folder", act?.summary);
  await sleep(300);
  check(readFileSync(join(root, "hub.err"), "utf8").includes(`could not start agent: its folder ${proj} is gone`), "the hub's log names it too");
} catch (e) {
  check(false, "run", String(e));
} finally {
  hub.kill();
  await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `spawn_gone: ${fail} failed` : "spawn_gone: ok");
process.exit(fail ? 1 : 0);
