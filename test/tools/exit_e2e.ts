// Exit without end of stream, end to end (item 7: the server's
// Agent.watch, Agent.last; hub.bend's Exit.current). A headless hub with
// the stand-in `claude` (test/tools/claude_standin.ts), one thread a case:
//   exit-held    the agent exits (3) while a child keeps its output open:
//                the turn fails within a few seconds anyway
//   result-exit  a completed command, then the same: completed, not failed
//   tail-nonl    an old CLI's last line, its result, has no newline: it is
//                read at the exit, and the turn completes
//   exit-now     a plain exit, end of stream and the watcher racing: the
//                exit is handled once
//   replaced     a new model replaces the process: the old one's exit is
//                ignored and the new turn completes
//   cx-held      a codex process exits without ending its turn while a
//                child keeps its output open: the turn fails within a few
//                seconds anyway (every provider is supervised)
//   cx-nonl      a codex turn's last line, turn.completed, has no newline:
//                it is read at the end of the stream, the turn completes
//   cx-err       a codex process writes an error to stderr and exits 2: the
//                failure says "(exit 2)" and the error, its token dropped
// (exit-now's stand-in writes to stderr too: its failure carries that)
//
//   bun test/tools/exit_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-exit-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
writeFileSync(join(fake, "grok"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "grok"), 0o755);
writeFileSync(join(fake, "muse"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "muse"), 0o755);
writeFileSync(join(fake, "omp"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "omp"), 0o755);
// codex exec --json: the prompt is the last argument
writeFileSync(join(fake, "codex"), `#!/bin/sh
for a in "$@"; do last="$a"; done
printf '%s\\n' '{"type":"thread.started","thread_id":"cx-1"}'
case "$last" in
  *cx-held*) sleep 30 & printf '%s\\n' '{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"working"}}'; exit 3 ;;
  *cx-err*) printf '%s\\n' 'Error: the config broke (token=SECRET123)' >&2; exit 2 ;;
  *cx-nonl*) printf '%s\\n' '{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"DONE"}}'; printf '%s' '{"type":"turn.completed","usage":{}}' ;;
esac
`);
chmodSync(join(fake, "codex"), 0o755);
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
};
const lines = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []);
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
    BP_WORK_MS: "300", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: { c: any; t: number }[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push({ c, t: Date.now() });
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const changes = () => seen.map((x) => x.c);
  const turns = (th: string) => changes().filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const exits = (th: string) => changes().filter((x) => x.$ === "ActivityLogged" && x.thread === th && x.kind === "exit").length;
  send("project.add", { path: proj });
  const pc = await until(10000, () => changes().find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string, provider = "claude") => {
    const n = changes().filter((x) => x.$ === "ThreadCreated").length;
    send("thread.create", { project: pc.id, title, env: "local", provider });
    return (await until(10000, () => changes().filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };
  const exitAt = (n: number) => { const l = lines().filter((x) => x.startsWith("EXIT")); return l.length >= n ? Date.now() : undefined; };

  // exit-held
  const t1 = await made("held");
  send("turn.start", { thread: t1, text: "mode:exit-held go", msg: "x-1" });
  const at1 = await until(10000, () => exitAt(1));
  await until(8000, () => turns(t1).at(-1) === "failed" ? true : undefined);
  const failedAt = seen.find((x) => x.c.$ === "TurnChanged" && x.c.thread === t1 && x.c.state === "failed")?.t ?? Infinity;
  check(turns(t1).at(-1) === "failed" && failedAt - (at1 ?? 0) < 4500, `exit-held: fails within 4.5 s though a child holds the output (${failedAt - (at1 ?? 0)} ms)`);
  check(exits(t1) === 1, `exit-held: one exit (${exits(t1)})`);

  // result-exit (a new process: each thread has its own)
  const t2 = await made("result");
  send("turn.start", { thread: t2, text: "mode:result-exit go", msg: "x-2" });
  await until(10000, () => exitAt(2));
  await sleep(4000);
  check(turns(t2).at(-1) === "completed" && !turns(t2).includes("failed"), `result-exit: completed, not failed (${turns(t2).join(",")})`);

  // tail-nonl
  const t3 = await made("tail");
  send("turn.start", { thread: t3, text: "mode:tail-nonl go", msg: "x-3" });
  await until(10000, () => exitAt(3));
  await until(6000, () => turns(t3).at(-1) === "completed" ? true : undefined);
  await sleep(500);
  check(turns(t3).at(-1) === "completed" && !turns(t3).includes("failed"), `tail-nonl: the last line without a newline completes the turn (${turns(t3).join(",")})`);

  // exit-now
  const t4 = await made("now");
  send("turn.start", { thread: t4, text: "mode:exit-now go", msg: "x-4" });
  await until(10000, () => exitAt(4));
  await sleep(4000);
  check(turns(t4).at(-1) === "failed" && exits(t4) === 1, `exit-now: handled once (${turns(t4).join(",")}; ${exits(t4)} exit)`);
  const why4 = String(changes().find((x) => x.$ === "ActivityLogged" && x.thread === t4 && x.kind === "exit")?.summary ?? "");
  check(why4.includes("(exit 3)") && why4.includes("the stand-in quits here") && !why4.includes("hunter2"), `exit-now: the failure carries the exit code and stderr, secret dropped (${why4})`);

  // replaced
  const t5 = await made("replaced");
  send("turn.start", { thread: t5, text: "mode:plain one", msg: "x-5" });
  await until(10000, () => turns(t5).at(-1) === "completed" ? true : undefined);
  send("thread.modes", { thread: t5, model: "claude-other" });
  await sleep(300);
  send("turn.start", { thread: t5, text: "two", msg: "x-6" });
  await until(10000, () => turns(t5).filter((s) => s === "completed").length >= 2 ? true : undefined);
  await sleep(3500);
  check(turns(t5).at(-1) === "completed" && exits(t5) === 0 && !turns(t5).includes("failed"), `replaced: the old process's exit is ignored (${turns(t5).join(",")})`);

  // cx-held: a codex process exits, a child keeps its output open
  const t6 = await made("cx-held", "codex");
  const s6 = Date.now();
  send("turn.start", { thread: t6, text: "cx-held go", msg: "x-7" });
  await until(9000, () => turns(t6).at(-1) === "failed" ? true : undefined);
  check(turns(t6).at(-1) === "failed" && Date.now() - s6 < 7000, `cx-held: a codex turn fails within seconds though a child holds the output (${turns(t6).join(",")}; ${Date.now() - s6} ms)`);

  // cx-nonl: the last line has no newline
  const t7 = await made("cx-nonl", "codex");
  send("turn.start", { thread: t7, text: "cx-nonl go", msg: "x-8" });
  await until(8000, () => turns(t7).at(-1) === "completed" ? true : undefined);
  await sleep(500);
  check(turns(t7).at(-1) === "completed" && !turns(t7).includes("failed"), `cx-nonl: a codex turn's last line without a newline completes it (${turns(t7).join(",")})`);

  // cx-err: stderr and the exit code reach the failure
  const t8 = await made("cx-err", "codex");
  send("turn.start", { thread: t8, text: "cx-err go", msg: "x-9" });
  await until(8000, () => turns(t8).at(-1) === "failed" ? true : undefined);
  const why8 = String(changes().find((x) => x.$ === "ActivityLogged" && x.thread === t8 && x.tone === "error")?.summary ?? "");
  check(turns(t8).at(-1) === "failed" && why8.includes("(exit 2)") && why8.includes("the config broke") && !why8.includes("SECRET123"), `cx-err: the failure says the exit code and stderr, token dropped (${why8})`);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
