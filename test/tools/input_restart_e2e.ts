// A turn's input outlives a hub restart mid-turn and the minute ticks that
// prune inputs of threads at rest (model.bend's Input.prune): a Claude turn
// runs, the hub is killed, the restart carries the turn on, two ticks pass,
// then Claude hits its usage limit and the fallback starts Codex with the
// interrupted request (hub.bend's Fallback.continue). Real isolated hub,
// stand-in CLIs, no real model calls. Takes about three minutes.
//
//   bun test/tools/input_restart_e2e.ts [BINARY] [WIREDIR]
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const emit = (o: object) => console.log(JSON.stringify(o));
const log = (o: object) => appendFileSync(process.env.INPUT_AGENT_LOG!, JSON.stringify(o) + "\n");
const args = process.argv.slice(3);

if (process.argv[2] === "--claude") {
  // every user line opens the turn; it ends only at its usage limit, once
  // the GO file exists
  if (args.includes("--version")) { console.log("2.1.286"); process.exit(0); }
  const rl = createInterface({ input: process.stdin });
  rl.on("close", () => process.exit(0));
  rl.on("line", (line) => {
    const o = JSON.parse(line);
    if (o.type === "control_request") {
      if (o.request?.subtype === "initialize") emit({ type: "control_response", response: { response: { models: [{ value: "sonnet", displayName: "Sonnet" }] } } });
      return;
    }
    if (o.type !== "user") return;
    const text = typeof o.message?.content === "string" ? o.message.content : (o.message?.content ?? []).map((b: any) => b.text ?? "").join("\n");
    log({ provider: "claude", prompt: text, args });
    emit({ type: "system", subtype: "init", session_id: "sess-r", claude_code_version: "2.1.286", capabilities: ["msg_lifecycle_v1"] });
    emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "queued" });
    emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "started" });
    emit({ type: "assistant", uuid: "a1", parent_tool_use_id: null, message: { id: "a1", role: "assistant", content: [{ type: "text", text: "working on it" }] } });
    const t = setInterval(() => {
      if (!existsSync(process.env.INPUT_GO!)) return;
      clearInterval(t);
      const reset = Math.floor(Date.now() / 1000) + 3600;
      emit({ type: "rate_limit_event", rate_limit_info: { status: "rejected", rateLimitType: "seven_day", resetsAt: reset } });
      emit({ type: "result", subtype: "error_during_execution", is_error: true, errors: ["Weekly limit reached"], resets_at: reset, session_id: "sess-r" });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "completed" });
    }, 200);
  });
} else if (process.argv[2] === "--codex") {
  if (args.includes("--version")) { console.log("codex-cli 0.100.0"); process.exit(0); }
  log({ provider: "codex", prompt: args.at(-1)!, args });
  emit({ type: "thread.started", thread_id: `fake-codex-${process.pid}` });
  emit({ type: "item.completed", item: { id: `answer-${process.pid}`, type: "agent_message", text: "Done." } });
  emit({ type: "turn.completed", usage: { input_tokens: 10, output_tokens: 5 } });
} else {
  const [binArg = "build/backplane", wireArg = "build/wire"] = process.argv.slice(2);
  const bin = resolve(binArg), wire = resolve(wireArg);
  for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), "bp-input-restart-"));
  const home = join(root, "home"), fake = join(root, "bin"), proj = join(root, "project");
  for (const p of [home, fake, proj, join(root, ".codex")]) mkdirSync(p);
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  for (const p of ["claude", "codex"]) {
    writeFileSync(join(fake, p), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve(import.meta.path))} --${p} "$@"\n`);
    chmodSync(join(fake, p), 0o755);
  }
  for (const p of ["grok", "muse", "agy", "gh", "tailscale", "kicad-cli", "freecad"]) { writeFileSync(join(fake, p), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, p), 0o755); }
  writeFileSync(join(root, ".codex/models_cache.json"), JSON.stringify({ models: [{ slug: "test-next" }] }));
  writeFileSync(join(home, "models.cache"), JSON.stringify({ claude: ["sonnet"], codex: ["test-next"], grok: [], muse: [], antigravity: [] }) + "\nFake installed providers\n");
  const ev = ($: string, f: object) => JSON.stringify({ $, ...f, at: 1 });
  const eventsPath = join(home, "events.jsonl"), agentLog = join(root, "agents.jsonl"), go = join(root, "GO");
  writeFileSync(agentLog, "");
  writeFileSync(eventsPath, [ev("ProjectCreated", { id: "p", title: "Input test", root: proj }),
    ev("ThreadCreated", { id: "w", project: "p", title: "w", env: "local", branch: "", worktree: proj, provider: "claude", model: "sonnet" }),
    ev("SettingSet", { key: "fallback.chain", value: "claude/sonnet,codex/test-next" }), ev("SettingSet", { key: "fallback.on", value: "on" }),
    ev("SettingSet", { key: "text.provider", value: "codex" }), ev("SettingSet", { key: "text.model", value: "test-next" })].join("\n") + "\n");
  const env = { ...process.env, HOME: root, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1",
    BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_KISTACK_SYNC: "0",
    INPUT_AGENT_LOG: agentLog, INPUT_GO: go, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "deploy-locks") };
  const events = () => readFileSync(eventsPath, "utf8").trim().split("\n").filter(Boolean).map((x) => JSON.parse(x));
  const launches = () => readFileSync(agentLog, "utf8").trim().split("\n").filter(Boolean).map((x) => JSON.parse(x));
  const until = async (label: string, f: () => boolean, ms = 30000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) { if (f()) return; await sleep(100); }
    throw new Error(`timeout: ${label}`);
  };
  let fail = 0;
  const check = (ok: unknown, what: string) => { console.log(`${ok ? "ok" : "FAIL"} ${what}`); if (!ok) fail++; };
  const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
  const hub = async () => {
    const port = freePort();
    const proc = Bun.spawn([bin, "--headless", "--foreground", "--home", home, "--port", String(port), "--no-tailscale"], { env, stdout: Bun.file(join(root, `stdout-${port}`)), stderr: Bun.file(join(root, `stderr-${port}`)) });
    let ready = false;
    for (let i = 0; i < 300 && !ready; i++) { try { ready = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (!ready) await sleep(100); }
    if (!ready) throw new Error("hub not ready");
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    ws.binaryType = "arraybuffer";
    await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
    let id = 0;
    return { proc, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))), kill: async () => { ws.close(); proc.kill(9); await proc.exited; } };
  };
  const inputGone = (from: number) => events().slice(from).some((e) => e.$ === "SettingSet" && e.key === "fallback.input.w" && e.value === "");
  try {
    let h = await hub();
    h.send("turn.start", { thread: "w", text: "ORIGINAL_REQUEST_42: build the thing", msg: "original" });
    await until("claude got the request", () => launches().some((l) => l.provider === "claude" && l.prompt.includes("ORIGINAL_REQUEST_42")));
    await until("input recorded", () => events().some((e) => e.$ === "SettingSet" && e.key === "fallback.input.w" && e.value.includes("ORIGINAL_REQUEST_42")));
    await sleep(500);
    await h.kill();
    const mark = events().length, n0 = launches().length;
    h = await hub();
    await until("the restart carried the turn on", () => launches().slice(n0).some((l) => l.provider === "claude"), 20000);
    // two of the minute ticks that prune inputs of threads at rest
    await sleep(130000);
    check(!inputGone(mark), "a running thread's input survives the restart and the prune ticks");
    writeFileSync(go, "");
    await until("codex started by the fallback", () => launches().some((l) => l.provider === "codex"), 30000);
    const codex = launches().find((l) => l.provider === "codex")!;
    const marker = "Original request (context, not a new task):\n";
    const at = codex.prompt.indexOf(marker);
    check(at >= 0 && codex.prompt.slice(at + marker.length).trim().length > 0, "the fallback's prompt carries the interrupted input");
    check(codex.prompt.includes("ORIGINAL_REQUEST_42"), "the original request reaches the next provider");
    await until("the turn ends", () => events().slice(mark).some((e) => e.$ === "TurnChanged" && e.thread === "w" && e.state === "completed"), 30000);
    await sleep(65000);
    check(inputGone(mark), "once at rest, the input is pruned");
    await h.kill();
  } finally {
    if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
  }
  console.log(fail ? `${fail} failed` : "all ok");
  process.exit(fail ? 1 : 0);
}
