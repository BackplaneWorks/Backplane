// The strict command line and one hub per home, end to end (core/cli.bend,
// core/onehub.bend, core/standin.bend; server.bend's Lock.*, Agents.only).
// Every run has a temp HOME and a temp --home, ports 3799+ and
// --no-tailscale: this test never touches ~/.backplane-bend or the live hub.
//   1. --help, -h, --version, --bogus, a missing value and a word: each exits
//      at once (0, 0, 0, 2, 2, 2) and starts nothing: no home made, no
//      hub.lock, nothing on the port. A bad flag says the usage on stderr.
//   2. A hub on a temp home: hub.lock says its pid and port and is flocked.
//      A second start on the same home ends non-zero before the event store
//      ("a hub already serves <home> (pid P, port N)") and binds nothing;
//      the first still answers /hello.
//   3. Stand-ins (BACKPLANE_AGENTS) never fall back to real CLIs: the
//      stand-in folder has no claude, a "real" claude is later on PATH; a
//      claude turn fails saying so, and the real one never runs.
//   4. A restart in place (deploy.restart, "restarting in place" in its
//      log) keeps the lock: same pid, still flocked, a second hub still
//      refused.
//   5. The hub killed with SIGKILL leaves hub.lock behind; it blocks nobody:
//      the next hub starts.
//
//   bun test/tools/onehub_e2e.ts [BINARY] [WIREDIR]
//   (BINARY defaults to build/backplane; SERVE=dist/backplane-serve also
//   runs step 1 and the second-hub refusal against the server binary)
//   EXPECT_SERVER=1 TEST_DISPLAY=:N also proves the server entry never
//   loads window/font libraries, including after its in-place restart.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binArg = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
const bin = resolve(binArg);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-onehub-"));
const user = join(root, "user");
mkdirSync(user);
const standins = join(root, "standins");
mkdirSync(standins);
// a codex stand-in that is there but not executable: it must not count
writeFileSync(join(standins, "codex"), "#!/bin/sh\nexit 0\n");
// the "real" CLIs, later on PATH: running one leaves a mark
const real = join(root, "real");
mkdirSync(real);
const ranReal = join(root, "real-ran");
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(real, n), `#!/bin/sh\necho "$0 $*" >> "${ranReal}"\nexit 0\n`);
  chmodSync(join(real, n), 0o755);
}
const proj = join(root, "proj");
mkdirSync(proj);

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
const listening = async (port: number) => {
  try {
    const s = await Bun.connect({ hostname: "127.0.0.1", port, socket: { data() {} } });
    s.end();
    return true;
  } catch {
    return false;
  }
};
const hello = async (port: number) => {
  try {
    return (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200;
  } catch {
    return false;
  }
};
const env = (extra: Record<string, string> = {}) => ({
  HOME: user, DISPLAY: process.env.TEST_DISPLAY ?? "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "",
  BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_DEPLOY_LOCKS: join(root, "no-such.lock"), BACKPLANE_AGENTS: standins,
  PATH: `${real}:/usr/bin:/bin`, XDG_CACHE_HOME: join(user, ".cache"), TMPDIR: root, ...extra,
});
// flock(1) on the file: true when another process holds the lock
const flocked = (path: string) => {
  const r = Bun.spawnSync(["sh", "-c", 'command -v flock >/dev/null || exit 9; flock -n "$1" true', "sh", path]);
  return r.exitCode === 9 ? undefined : r.exitCode !== 0;
};

// 1. the command line, before anything else
const quick = async (b: string, name: string) => {
  const port = 3799;
  const home = join(root, `cli-${name}`);
  const cases: [string[], number, string][] = [
    [["--home", home, "--port", String(port), "--no-tailscale", "--help"], 0, "usage:"],
    [["-h"], 0, "usage:"],
    [["--home", home, "--version"], 0, "backplane "],
    [["--home", home, "--port", String(port), "--bogus"], 2, "unknown flag --bogus"],
    [["--home"], 2, "--home needs a value"],
    [["serve"], 2, "unexpected argument serve"],
  ];
  for (const [args, code, says] of cases) {
    const t0 = Date.now();
    const r = Bun.spawnSync([b, ...args], { env: env(), timeout: 20000 });
    const out = r.stdout.toString() + r.stderr.toString();
    const ms = Date.now() - t0;
    check(r.exitCode === code && out.includes(says), `${name} ${args.at(-1)}: exit ${r.exitCode} (want ${code}), says "${says}" (${ms} ms): ${out.trim().split("\n")[0]}`);
    if (code === 2) check(r.stderr.toString().includes("usage:"), `${name} ${args.at(-1)}: the usage goes to stderr`);
    check(!existsSync(home) && !existsSync(join(user, ".backplane-bend")), `${name} ${args.at(-1)}: no home made`);
    check(!(await listening(port)), `${name} ${args.at(-1)}: nothing on port ${port}`);
  }
};
await quick(bin, "app");
if (process.env.SERVE) await quick(resolve(process.env.SERVE), "serve");

const home = join(root, "home");
const P1 = 3801, P2 = 3802, P3 = 3803;
const start = (b: string, port: number) =>
  Bun.spawn([b, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], { env: env(), stdout: "ignore", stderr: Bun.file(join(root, `hub-${port}.log`)) });
const logTail = (port: number) => { const f = join(root, `hub-${port}.log`); return existsSync(f) ? readFileSync(f, "utf8").trim().split("\n").slice(-8).join("\n") : ""; };
const serverOnly = (pid: number, when: string) => {
  if (process.env.EXPECT_SERVER !== "1") return;
  const maps = readFileSync(`/proc/${pid}/maps`, "utf8");
  check(!/libX11|libfontconfig|libfreetype/.test(maps), `${when}: server loads no window/font libraries despite DISPLAY=${process.env.TEST_DISPLAY ?? ""}`);
};
let a = start(bin, P1);
let a2: ReturnType<typeof start> | undefined;
try {
  check(!!(await until(30000, () => hello(P1))), "a hub starts on a temp home");
  serverOnly(a.pid, "startup");
  const lockText = existsSync(join(home, "hub.lock")) ? readFileSync(join(home, "hub.lock"), "utf8").trim() : "";
  check(lockText === `${a.pid} ${P1} flock`, `hub.lock says its pid and port (${lockText})`);
  const fl = flocked(join(home, "hub.lock"));
  if (fl !== undefined) check(fl, "hub.lock is flocked while the hub runs");

  // 2. a second hub on the same home
  const second = async (b: string, name: string, port: number) => {
    const t0 = Date.now();
    const r = Bun.spawnSync([b, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], { env: env(), timeout: 30000 });
    const err = r.stderr.toString();
    check(r.exitCode !== 0 && r.exitCode !== null && err.includes(`a hub already serves ${home} (pid ${a.pid}, port ${P1})`),
      `${name}: a second hub refuses (exit ${r.exitCode}, ${Date.now() - t0} ms): ${err.trim().split("\n").at(-1)}`);
    check(!(await listening(port)), `${name}: the second hub bound nothing on ${port}`);
    check(await hello(P1), `${name}: the first hub still answers /hello`);
  };
  await second(bin, "app", P2);
  if (process.env.SERVE) await second(resolve(process.env.SERVE), "serve", P2);
  const headless = Bun.spawnSync([bin, "--home", home, "--port", String(P2), "--no-tailscale", "--headless"], { env: env(), timeout: 30000 });
  check(headless.exitCode === 1 && headless.stderr.toString().includes("a hub already serves"), `--headless on a served home says so and ends 1 (exit ${headless.exitCode})`);

  // 3. stand-ins only
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${P1}/ws`);
  ws.binaryType = "arraybuffer";
  let closed = false;
  ws.onclose = () => { closed = true; };
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  const made = async (provider: string) => {
    const n = seen.filter((x) => x.$ === "ThreadCreated").length;
    send("thread.create", { project: pc?.id, title: provider, env: "local", provider });
    return (await until(10000, () => seen.filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };
  for (const provider of ["claude", "codex"]) {
    const th = await made(provider);
    send("turn.start", { thread: th, text: "hello", msg: `m-${provider}` });
    const failed = await until(20000, () => seen.find((x) => x.$ === "TurnChanged" && x.thread === th && x.state === "failed"));
    const why = seen.filter((x) => x.$ === "ActivityLogged" && x.thread === th).map((x) => String(x.summary ?? "")).join(" | ");
    check(!!failed && why.includes(`has no runnable ${provider}`), `${provider}: a turn with no stand-in fails and says so (${why.slice(0, 200)})`);
  }
  check(!existsSync(ranReal), `the real CLIs later on PATH never ran (${existsSync(ranReal) ? readFileSync(ranReal, "utf8").trim() : "none"})`);

  // 4. a restart in place keeps the lock
  send("deploy.restart", { note: "onehub e2e" });
  const went = await until(60000, () => closed || logTail(P1).includes("restarting in place"));
  check(!!went, "deploy.restart: the hub restarts in place");
  if (!went) console.log(logTail(P1));
  check(!!(await until(30000, () => hello(P1))), "the restarted hub answers /hello");
  serverOnly(a.pid, "after exec");
  const after = existsSync(join(home, "hub.lock")) ? readFileSync(join(home, "hub.lock"), "utf8").trim() : "";
  check(after === `${a.pid} ${P1} flock`, `after the restart hub.lock still names the same process (${after})`);
  const fl2 = flocked(join(home, "hub.lock"));
  if (fl2 !== undefined) check(fl2, "after the restart hub.lock is still flocked");
  await second(bin, "after restart", P2);

  // 5. SIGKILL: the lock goes with the process, the file stays and blocks no one
  a.kill(9);
  await a.exited;
  check(existsSync(join(home, "hub.lock")), "SIGKILL leaves hub.lock behind");
  const fl3 = flocked(join(home, "hub.lock"));
  if (fl3 !== undefined) check(!fl3, "and nobody holds its lock");
  a2 = start(bin, P3);
  check(!!(await until(30000, () => hello(P3))), "the next hub starts on the same home");
  const again = existsSync(join(home, "hub.lock")) ? readFileSync(join(home, "hub.lock"), "utf8").trim() : "";
  check(again === `${a2.pid} ${P3} flock`, `and the file names it (${again})`);
} finally {
  a.kill(9);
  await a.exited;
  if (a2) { a2.kill(9); await a2.exited; }
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
