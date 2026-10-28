// deploy_main end to end (core/ship.bend, hub.bend's Ship.*, server.bend's
// ShipJob.*, docs/deploy.md). A throwaway hub (temp --home, --no-tailscale,
// port 3799+) whose deploy.repo is a temp git checkout of a temp origin; its
// scripts/dev.sh is a stub that records each run, sleeps, and stages an empty
// package (so the restart it leads to swaps nothing into the prefix copy of
// the binary). It posts nothing to any hub.
//   1. t1 asks: queued, one build. t2 asks for the same commit: skipped
//      (covered by the build).
//   2. origin/main moves; t2 and the person (/mcp/deploy) ask: both queued,
//      no second build now.
//   3. the build ends: t1 and t2 are told once (commit, version); one
//      follow-up build of the newest commit, which the person is told of.
//   4. the hub restarts into the staged build: deploy_status says live.
//      Asking again: skipped (live). Two quick requests: 2 builds in all.
//   5. a build under way survives the hub being killed (SIGKILL): the next
//      hub re-adopts it and reports its outcome, not "lost".
//
//   bun test/tools/deploy_main_e2e.ts [BINARY]     (BINARY: a native build of
//   src/server/main.bend or src/app/main.bend; default build/backplane)
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane"] = process.argv.slice(2);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) { const v = await f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(200); }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const sh = (cmd: string[], cwd: string) => {
  const r = Bun.spawnSync(cmd, { cwd, stdout: "pipe", stderr: "pipe", env: { ...process.env, GIT_AUTHOR_NAME: "e2e", GIT_AUTHOR_EMAIL: "e2e@x", GIT_COMMITTER_NAME: "e2e", GIT_COMMITTER_EMAIL: "e2e@x" } });
  if (r.exitCode !== 0) throw new Error(`${cmd.join(" ")}: ${r.stderr}`);
  return r.stdout.toString().trim();
};

const free = async (p: number) => { try { await fetch(`http://127.0.0.1:${p}/hello`); return false; } catch { return true; } };
let PORT = Number(process.env.E2E_PORT ?? 3799);
while (!(await free(PORT))) PORT++;

const root = mkdtempSync(join(tmpdir(), "bp-ship-"));
const home = join(root, "home"), prefix = join(root, "prefix"), fake = join(root, "bin");
const origin = join(root, "origin.git"), src = join(root, "src"), dev = join(root, "dev");
const runs = join(root, "runs"), sleepf = join(root, "sleep"), proj = join(root, "proj");
for (const d of [home, prefix, fake, proj]) mkdirSync(d);
writeFileSync(sleepf, "5");
// the stand-in agent: a turn a report starts ends at once
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
for (const n of ["codex", "grok", "muse"]) writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
for (const n of ["claude", "codex", "grok", "muse"]) chmodSync(join(fake, n), 0o755);
copyFileSync(resolve(bin), join(prefix, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);

// origin, the hub's checkout and a clone to push from
sh(["git", "init", "-q", "--bare", "-b", "main", origin], root);
sh(["git", "clone", "-q", origin, dev], root);
mkdirSync(join(dev, "scripts"));
writeFileSync(join(dev, "scripts", "dev.sh"), `#!/bin/sh
# deploy_main e2e stub: records the run, stages an empty package
[ "$1" = stage ] || exit 3
git rev-parse HEAD >> "${runs}"
sleep "$(cat "${sleepf}")"
mkdir -p "$2"
echo "dev: built 0.0.1-e2e.$(git rev-parse --short=7 HEAD)"
`);
chmodSync(join(dev, "scripts", "dev.sh"), 0o755);
const commit = (msg: string) => {
  writeFileSync(join(dev, "f"), msg);
  sh(["git", "add", "-A"], dev); sh(["git", "commit", "-q", "-m", msg], dev); sh(["git", "push", "-q", "origin", "HEAD:main"], dev);
  return sh(["git", "rev-parse", "HEAD"], dev);
};
const c1 = commit("one");
sh(["git", "clone", "-q", origin, src], root);

const ev = (o: object) => JSON.stringify(o);
writeFileSync(join(home, "events.jsonl"), [
  ev({ $: "ProjectCreated", id: "p1", title: "p", root: proj, at: 1 }),
  ev({ $: "ThreadCreated", id: "t1", project: "p1", title: "one", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: 1 }),
  ev({ $: "ThreadCreated", id: "t2", project: "p1", title: "two", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: 1 }),
  ev({ $: "SettingSet", key: "restart.continue", value: "off", at: 1 }),
  ev({ $: "SettingSet", key: "deploy.repo", value: src, at: 1 }),
  // a harmless prefix: the e2e does not need a systemd scope
  ev({ $: "SettingSet", key: "deploy.wrap", value: "env", at: 1 }),
].join("\n") + "\n");

const log = () => read(join(home, "events.jsonl")).split("\n").filter(Boolean).map((l) => { try { return JSON.parse(l); } catch { return {}; } });
const told = (th: string, c: string) => log().filter((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).includes(`deploy_main: origin/main ${c.slice(0, 9)}`));
const ran = () => read(runs).split("\n").filter(Boolean);

type Hub = ReturnType<typeof Bun.spawn>;
const start = async (): Promise<Hub> => {
  const p = Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
      BACKPLANE_DEPLOY_LOCKS: join(root, "no.lock"), FAKE_LOG: join(root, "claude.log"), BP_WORK_MS: "100", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: Bun.file(join(root, "hub.out")), stderr: Bun.file(join(root, "hub.err")),
  });
  await until(20000, async () => { try { return (await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200; } catch { return undefined; } });
  return p;
};
const call = async (thread: string, name: string, args: object = {}) => {
  const r = await fetch(`http://127.0.0.1:${PORT}/mcp/${thread}`, { method: "POST", headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) });
  const o: any = await r.json();
  try { return JSON.parse(o.result.content[0].text); } catch { return o; }
};

let hub: Hub | null = null;
try {
  try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) { console.log(`port ${PORT} in use`); process.exit(2); } } catch {}
  hub = await start();
  // 1.
  const a1 = await call("t1", "deploy_main", { note: "e2e" });
  check(a1.state === "queued" && a1.commit === c1, `t1: queued c1 (${JSON.stringify(a1).slice(0, 120)})`);
  const a2 = await call("t2", "deploy_main");
  check(a2.state === "skipped" && a2.covered === "building" && a2.commit === c1, `t2, same commit: skipped, covered by the build (${a2.state} ${a2.covered})`);
  // 2.
  const c2 = commit("two");
  const a3 = await call("t2", "deploy_main");
  check(a3.state === "queued" && a3.after === c1, `t2, newer: queued after the build (${a3.state} ${a3.after})`);
  const a4 = await call("deploy", "deploy_main");
  check(a4.state === "queued", `the person: queued (${a4.state})`);
  await sleep(1000);
  check(ran().length === 1, `one build so far (${ran().length})`);
  // 3.
  await until(30000, () => told("t1", c1).length ? true : undefined);
  await sleep(1500);
  check(told("t1", c1).length === 1 && told("t2", c1).length === 1, `t1 and t2 told once (${told("t1", c1).length}, ${told("t2", c1).length})`);
  check(/built and staged \(version 0\.0\.1-e2e\./.test(String(told("t1", c1)[0]?.text)), `with the version (${String(told("t1", c1)[0]?.text).slice(0, 110)})`);
  const second = await until(30000, () => ran().length >= 2 ? true : undefined);
  check(!!second && ran()[1] === c2, `one follow-up build, of the newest (${ran().map((x) => x.slice(0, 7)).join(",")})`);
  // the restart into c1 does not wait for the follow-up: the build goes on through it
  const alerted = await until(30000, () => log().some((x) => x.$ === "SettingSet" && x.key === "alert.deploy" && String(x.value).includes(c2.slice(0, 9))) ? true : undefined);
  check(!!alerted, "the person is alerted of the follow-up");
  // 4. it restarts into c2 (BACKPLANE_DEPLOY_LOCKS free, no tool call)
  const live = await until(60000, async () => { try { const s = await call("deploy", "deploy_status"); return s?.main?.phase === "live" ? s : undefined; } catch { return undefined; } });
  check(!!live && live.main.live === c2, `after the restart c2 is live (${JSON.stringify(live?.main ?? {}).slice(0, 160)})`);
  const a5 = await call("t1", "deploy_main");
  check(a5.state === "skipped" && a5.covered === "live", `asking again: skipped, live (${a5.state} ${a5.covered})`);
  check(ran().length === 2, `two builds for five requests (${ran().length})`);
  check(told("t2", c2).length === 1, `t2 told of c2 once (${told("t2", c2).length})`);
  // 5. the hub killed during a build: the build goes on, the next hub reads its outcome
  writeFileSync(sleepf, "8");
  const c3 = commit("three");
  const a6 = await call("t1", "deploy_main");
  check(a6.state === "queued" && a6.commit === c3, `t1: queued c3 (${a6.state})`);
  await until(10000, () => log().some((x) => x.$ === "DeployBuilt" && x.state === "pid" && x.commit === c3) ? true : undefined);
  check(log().some((x) => x.$ === "DeployBuilt" && x.state === "pid" && x.commit === c3), "the build's pid is recorded");
  // the hub restarted in place (exec), so it is still the PID started here
  try { process.kill(hub.pid, "SIGKILL"); } catch {}
  await sleep(1000);
  hub = await start();
  await until(40000, () => told("t1", c3).length ? true : undefined);
  check(told("t1", c3).length === 1 && /built and staged/.test(String(told("t1", c3)[0]?.text)), `the re-adopted build is reported, not lost (${String(told("t1", c3)[0]?.text).slice(0, 90)})`);
  check(ran().filter((x) => x === c3).length === 1, "and it ran once");
} catch (e) {
  console.log("FAIL", e);
  fail++;
} finally {
  if (hub) { try { process.kill(hub.pid, "SIGKILL"); } catch {} }
  console.log(`${fail ? "FAILED" : "passed"} (home ${root})`);
  process.exit(fail ? 1 : 0);
}
