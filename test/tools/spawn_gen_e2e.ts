// Spawn generations, end to end (item 6: hub.bend's Gen.*, the server's
// placeholders, TurnPid/SpawnFailed by generation, MCP paths
// "<thread>.<gen>"). A headless hub with stand-ins:
//   1. a codex turn (a stand-in `codex` that waits BP_SPAWN_DELAY ms before
//      it speaks) completes; one stopped while it starts, then sent again
//      at once, completes with the second process's answer and never the
//      first's
//   2. grok and muse that cannot start fail their turns (the current
//      generation's failure is heard)
//   3. a Claude thread whose process was replaced (a new model): a
//      changing tool call over the old process's MCP URL is refused as a
//      stale process, a reading one answers, and the new URL may change
//
//   bun test/tools/spawn_gen_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-gen-"));
const fake = join(root, "bin");
const log = join(root, "agents.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
// codex exec --json, one turn: the prompt is the last argument
writeFileSync(join(fake, "codex"), `#!/bin/sh
for a in "$@"; do last="$a"; done
echo "CODEX $$ $last" >> "$FAKE_LOG"
case "$last" in *slow*) sleep $(( \${BP_SPAWN_DELAY:-1500} / 1000 )).5 ;; esac
printf '%s\\n' '{"type":"thread.started","thread_id":"cx-'"$$"'"}'
printf '%s\\n' '{"type":"item.completed","item":{"id":"i'"$$"'","type":"agent_message","text":"CODEX ANSWER '"$$"'"}}'
printf '%s\\n' '{"type":"turn.completed","usage":{}}'
`);
chmodSync(join(fake, "claude"), 0o755);
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
// no grok on the path: it cannot start
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
    BP_WORK_MS: "300", BP_SPAWN_DELAY: "1500", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string, provider: string) => {
    const n = seen.filter((x) => x.$ === "ThreadCreated").length;
    send("thread.create", { project: pc.id, title, env: "local", provider });
    return (await until(10000, () => seen.filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };

  // 1. codex
  const c1 = await made("codex", "codex");
  send("turn.start", { thread: c1, text: "hello codex", msg: "g-1" });
  await until(10000, () => turns(c1).at(-1) === "completed" ? true : undefined);
  check(seen.some((x) => x.$ === "MessagePosted" && x.thread === c1 && String(x.text).startsWith("CODEX ANSWER")), `a codex turn completes (${turns(c1).join(",")})`);
  const c2 = await made("codex-stop", "codex");
  send("turn.start", { thread: c2, text: "slow first", msg: "g-2" });
  await until(5000, () => lines().some((l) => l.startsWith("CODEX") && l.includes("slow first")) ? true : undefined);
  send("turn.interrupt", { thread: c2 });
  await until(5000, () => turns(c2).at(-1) === "interrupted" ? true : undefined);
  send("turn.start", { thread: c2, text: "second now", msg: "g-3" });
  await until(10000, () => turns(c2).at(-1) === "completed" ? true : undefined);
  await sleep(2500);
  const first = lines().find((l) => l.startsWith("CODEX") && l.includes("slow first"))?.split(" ")[1];
  const second = lines().find((l) => l.startsWith("CODEX") && l.includes("second now"))?.split(" ")[1];
  const answers = seen.filter((x) => x.$ === "MessagePosted" && x.thread === c2 && String(x.text).startsWith("CODEX ANSWER")).map((x) => String(x.text));
  check(answers.includes(`CODEX ANSWER ${second}`) && !answers.includes(`CODEX ANSWER ${first}`), `stopped then sent again: the second process answers, never the first (${answers.join("|")})`);
  check(turns(c2).at(-1) === "completed", `and the thread ends completed (${turns(c2).join(",")})`);

  // 2. grok that cannot start
  const g1 = await made("grok", "grok");
  send("turn.start", { thread: g1, text: "hello grok", msg: "g-4" });
  await until(10000, () => turns(g1).at(-1) === "failed" ? true : undefined);
  check(turns(g1).at(-1) === "failed" && seen.some((x) => x.$ === "ActivityLogged" && x.thread === g1 && String(x.summary).includes("could not start grok")), "grok that cannot start fails its turn");
  const m1 = await made("muse", "muse");
  send("turn.start", { thread: m1, text: "hello muse", msg: "g-4m" });
  await until(10000, () => turns(m1).at(-1) === "failed" ? true : undefined);
  check(turns(m1).at(-1) === "failed" && seen.some((x) => x.$ === "ActivityLogged" && x.thread === m1 && String(x.summary).includes("could not start muse")), "muse that cannot start fails its turn");

  // 3. a replaced Claude process's MCP URL
  const t1 = await made("claude", "claude");
  send("turn.start", { thread: t1, text: "mode:plain one", msg: "g-5" });
  await until(10000, () => turns(t1).at(-1) === "completed" ? true : undefined);
  const url1 = lines().filter((l) => l.startsWith("MCP ")).at(-1)?.split(" ")[2] ?? "";
  send("thread.modes", { thread: t1, model: "claude-other" });
  await sleep(300);
  send("turn.start", { thread: t1, text: "two", msg: "g-6" });
  await until(10000, () => turns(t1).filter((s) => s === "completed").length >= 2 ? true : undefined);
  const url2 = lines().filter((l) => l.startsWith("MCP ")).at(-1)?.split(" ")[2] ?? "";
  check(!!url1 && !!url2 && url1 !== url2 && url1.includes(`/mcp/${t1}.`), `each Claude process has its own MCP path (${url1.replace(/\?.*/, "")} ${url2.replace(/\?.*/, "")})`);
  const call = async (url: string, name: string, args: object) => {
    const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 7, method: "tools/call", params: { name, arguments: args } }) });
    return await r.text();
  };
  const stale = await call(url1, "thread_update", { threadId: t1, title: "renamed by a stale process" });
  check(stale.includes("stale process") && !seen.some((x) => x.$ === "ThreadRenamed" && x.title === "renamed by a stale process"), `a changing call from the replaced process is refused (${stale.slice(0, 120)})`);
  const read = await call(url1, "thread_list", {});
  check(!read.includes("stale process") && read.includes(t1), "a reading call from it still answers");
  const fresh = await call(url2, "thread_update", { threadId: t1, title: "renamed by the current process" });
  await until(3000, () => seen.find((x) => x.$ === "ThreadRenamed" && x.title === "renamed by the current process"));
  check(!fresh.includes("stale process") && seen.some((x) => x.$ === "ThreadRenamed" && x.title === "renamed by the current process"), "the current process may change");
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
