// A thread whose folder is gone when its agent starts (a removed worktree:
// posix_spawn's chdir answers ENOENT, which read as a missing program): the
// turn fails and both the thread's activity and the hub's log name the
// folder. Covers Claude/Codex, ACP providers, and a project subdirectory
// absent from the worktree base branch (#43). Only stand-in agents run.
//
//   bun test/tools/spawn_gone_e2e.ts BINARY WIREDIR
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();

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
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, BACKPLANE_TELEMETRY: "off", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_KISTACK_SYNC: "0" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")),
});
try {
  for (let i = 0; i < 300; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(100); }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => { try { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); if (o.t !== "live") for (const c of o.items ?? []) seen.push(c); } catch {} };
  await new Promise((r) => (ws.onopen = r));
  let n = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  for (const provider of ["claude", "codex", "grok", "muse"]) {
    const folder = join(proj, provider + " folder 日本"); mkdirSync(folder);
    send("project.add", {path:folder});
    const pc: any = await until(10000, () => seen.find(c => c.$ === "ProjectCreated" && c.root === folder));
    send("thread.create", {project:pc.id, title:provider, env:"local", provider});
    const th = (await until(10000, () => seen.find(c => c.$ === "ThreadCreated" && c.title === provider)) as any).id;
    rmSync(folder, {recursive:true, force:true});
    send("turn.start", {thread:th, text:"hi"});
    const failed = await until(15000, () => seen.find(c => c.$ === "TurnChanged" && c.thread === th && /fail/i.test(c.state)));
    check(!!failed, `${provider}: the turn fails`);
    const act = seen.find(c => c.$ === "ActivityLogged" && c.thread === th && String(c.summary).includes("working directory does not exist:"));
    check(!!act && act.summary.includes(folder), `${provider}: activity names the missing directory`, seen.filter(c=>c.thread===th));
  }

  // A local origin's main branch lacks the project's feature-only subdir.
  const origin = join(root, "origin.git"), repo = join(root, "repo");
  const git = (cwd: string, ...args: string[]) => {
    const r = Bun.spawnSync(["git", ...args], {cwd, env:{...process.env, GIT_CONFIG_GLOBAL:"/dev/null", GIT_CONFIG_NOSYSTEM:"1"}});
    if (r.exitCode) throw new Error(new TextDecoder().decode(r.stderr));
  };
  git(root, "init", "--bare", "--initial-branch=main", origin);
  git(root, "clone", origin, repo);
  git(repo, "-c", "user.name=fixture", "-c", "user.email=fixture@example.com", "commit", "--allow-empty", "-m", "main");
  git(repo, "push", "origin", "main");
  git(repo, "checkout", "-b", "feature");
  const subdir = join(repo, "feature only"); mkdirSync(subdir);
  writeFileSync(join(subdir, "feature.txt"), "only on feature");
  git(repo, "add", ".");
  git(repo, "-c", "user.name=fixture", "-c", "user.email=fixture@example.com", "commit", "-m", "feature");
  send("project.add", {path:subdir});
  const pc: any = await until(10000, () => seen.find(c=>c.$==="ProjectCreated" && c.root===subdir));
  for (const provider of ["claude", "codex"]) {
    const title = "branch-only " + provider;
    send("thread.create", {project:pc.id, title, env:"worktree", provider});
    const th = (await until(10000, () => seen.find(c=>c.$==="ThreadCreated" && c.title===title)) as any).id;
    const wt: any = await until(20000, () => seen.find(c=>c.$==="WorktreeSet" && c.id===th));
    check(!!wt && !existsSync(wt.worktree) && wt.worktree.endsWith("/feature only"), `${provider}: worktree base lacks the project subdirectory`, wt);
    send("turn.start", {thread:th, text:"hi"});
    const act: any = await until(15000, () => seen.find(c=>c.$==="ActivityLogged" && c.thread===th && String(c.summary).includes("working directory does not exist:")));
    check(!!act && act.summary.includes(wt?.worktree), `${provider}: branch-only worktree failure names its exact directory`, act?.summary);
    check(!!seen.find(c=>c.$==="TurnChanged" && c.thread===th && /fail/i.test(c.state)), `${provider}: branch-only worktree turn fails`);
  }
  ws.close();
} catch (e) {
  check(false, "run", String(e));
} finally {
  hub.kill();
  await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `spawn_gone: ${fail} failed` : "spawn_gone: ok");
process.exit(fail ? 1 : 0);
