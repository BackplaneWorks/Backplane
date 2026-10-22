// The uberbot's fyi digest with automation (mail.on) off: a thread the
// uberbot launched finishes turns, and their reports are filed as fyi mail.
// The next minute tick files one digest (act mail listing them), which wakes
// the uberbot in a private run; a later report within 10 min makes no second
// digest. The person-facing chat gets nothing. Stand-in Claude only.
//   bun test/tools/uber_digest_e2e.ts HUB WIREDIR   (about 3 minutes)
// HUB: the native server (build/...) or the JS one (scripts/build-js.sh).
import { appendFileSync, chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const TEXT = "[fyi digest]";
async function agent() {
  const args = process.argv.slice(3);
  const config = JSON.parse(args[args.indexOf("--mcp-config") + 1]);
  const url = config.mcpServers.backplane.url;
  const session = `uberdigest-${process.pid}`;
  const log = (o: object) => appendFileSync(process.env.UBER_MAIL_LOG!, JSON.stringify({ pid: process.pid, ...o }) + "\n");
  log({ args });
  const emit = (o: any) => console.log(JSON.stringify({ session_id: session, uuid: crypto.randomUUID(), ...o }));
  emit({ type: "system", subtype: "init", tools: [], mcp_servers: [], model: "sonnet", claude_code_version: "2.1.286", capabilities: ["msg_lifecycle_v1"] });
  let call = 0;
  const rpc = async (method: string, params: object) => {
    const response = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: ++call, method, params }) });
    const text = await response.text(); log({ method, reply: text.slice(0, 4000) }); return text;
  };
  let chain = Promise.resolve();
  createInterface({ input: process.stdin }).on("line", (line) => {
    let o: any; try { o = JSON.parse(line); } catch { return; }
    if (o.type !== "user") return;
    const text = (Array.isArray(o.message?.content) ? o.message.content : [o.message?.content ?? ""]).map((x: any) => typeof x === "string" ? x : x.text ?? "").join(" ");
    chain = chain.then(async () => {
      log({ prompt: text, tools: await rpc("tools/list", {}) });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "queued" });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "started" });
      const reply = text.includes("REPORT_ONE") ? "Built the board, report one" : text.includes("REPORT_TWO") ? "DRC clean, report two" : "Noted";
      emit({ type: "assistant", message: { id: `m${process.pid}-${call}`, role: "assistant", content: [{ type: "text", text: reply }] }, parent_tool_use_id: null });
      emit({ type: "result", subtype: "success", is_error: false, num_turns: 1, result: reply, usage: {} });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "completed" });
    }).catch((e) => { log({ error: String(e) }); process.exit(1); });
  });
}
if (process.argv[2] === "--agent") { await agent(); } else {
  const bin = resolve(process.argv[2] ?? "build/uber-digest-server");
  const wire = resolve(process.argv[3] ?? "build/uber-digest-wire");
  for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), "bp-uberdigest-"));
  const home = join(root, "home"), fake = join(root, "bin"), proj = join(root, "project"), bot = join(root, "bot"), other = join(root, "other");
  for (const d of [home, fake, proj, bot, other]) mkdirSync(d);
  const shell = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${shell(process.execPath)} ${shell(resolve(import.meta.path))} --agent "$@"\n`);
  chmodSync(join(fake, "claude"), 0o755);
  for (const n of ["codex", "grok", "muse", "agy", "opencode"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
  const event = ($: string, fields: object) => ({ $, ...fields, at: 1 });
  const thread = (id: string, project: string, worktree: string) => event("ThreadCreated", { id, project, title: id, env: "local", branch: "", worktree, provider: "claude", model: "sonnet" });
  const logPath = join(home, "events.jsonl");
  // Bub (b, chat foh) is the uberbot; Other (o, chat oth) is a second bot; mail.on is off
  writeFileSync(logPath, [event("ProjectCreated", { id: "p", title: "Test", root: proj }), thread("worker", "p", proj), thread("foh", "", bot), thread("oth", "", other),
    event("BotSet", { id: "b", name: "Bub", thread: "foh", look: 0, persona: "" }), event("BotSet", { id: "o", name: "Other", thread: "oth", look: 0, persona: "" }),
    event("ThreadLaunched", { id: "worker", by: "foh", report: true }), event("SettingSet", { key: "uber.bot", value: "b" }), event("SettingSet", { key: "restart.continue", value: "off" })].map((e) => JSON.stringify(e)).join("\n") + "\n");
  const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const port = listener.port; listener.stop(true);
  const agentLog = join(root, "agents.jsonl");
  writeFileSync(agentLog, "");
  const proc = Bun.spawn([bin, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], { env: { ...process.env, HOME: root, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: fake, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_TELEMETRY: "off", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", UBER_MAIL_LOG: agentLog, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "deploy-lock") }, stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")) });
  const until = async (f: () => boolean, ms = 15000) => { const deadline = Date.now() + ms; while (Date.now() < deadline) { if (f()) return true; await sleep(50); } return false; };
  let ws: WebSocket | undefined;
  let failures = 0;
  const check = (ok: boolean, label: string) => { console.log(`${ok ? "ok" : "FAIL"} ${label}`); if (!ok) failures++; };
  try {
    let ready = false;
    for (let i = 0; i < 400; i++) { try { ready = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (ready) break; await sleep(50); }
    if (!ready) throw new Error("test hub failed to start");
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
    const events = () => readFileSync(logPath, "utf8").trim().split("\n").map((x) => { try { return JSON.parse(x); } catch { return {}; } });
    const agents = () => readFileSync(agentLog, "utf8").trim().split("\n").map((x) => { try { return JSON.parse(x); } catch { return {}; } });
    const fyi = () => events().filter((e) => e.$ === "MailPosted" && e.to === "bot:b" && e.want === "fyi");
    const digests = () => events().filter((e) => e.$ === "MailPosted" && e.to === "bot:b" && e.kind === "digest");
    ws.send(W.encode(JSON.stringify({ id: 1, m: "turn.start", p: { thread: "worker", text: "REPORT_ONE", msg: "person-1" } })));
    check(await until(() => fyi().length >= 1, 30000), "the worker's report is filed as fyi mail to the uberbot");
    check(!events().some((e) => e.$ === "RunSet"), "fyi mail alone starts no run");
    check(await until(() => digests().length === 1, 90000), "the next minute tick files one digest");
    const d = digests()[0];
    check(d?.want === "act", "the digest is act mail");
    check(await until(() => events().some((e) => e.$ === "RunSet" && e.bot === "b"), 10000), "the digest wakes the uberbot in a private run");
    check(await until(() => agents().some((e: any) => typeof e.prompt === "string" && e.prompt.includes(TEXT) && e.prompt.includes("report one")), 10000), "its turn lists the report with its text");
    ws.send(W.encode(JSON.stringify({ id: 2, m: "turn.start", p: { thread: "worker", text: "REPORT_TWO", msg: "person-2" } })));
    check(await until(() => fyi().length >= 2, 30000), "a second report is filed as fyi");
    await sleep(75000);
    check(digests().length === 1, "no second digest within 10 minutes");
    const es = events();
    check(!es.some((e) => e.$ === "MessagePosted" && e.thread === "foh"), "nothing is posted in the person-facing chat");
    check(!es.some((e) => e.$ === "RunSet" && e.bot !== "b"), "no other bot is woken");
    check(!es.some((e) => e.$ === "SettingSet" && e.key === "mail.on"), "automation stays off");
  } finally {
    ws?.close();
    // Only the exact hub PID started above.
    if (proc.exitCode === null) proc.kill("SIGTERM");
    await proc.exited;
    if (failures || process.env.KEEP) { console.log(`artifacts ${root}`); console.log(readFileSync(join(root, "stderr"), "utf8").slice(-3000)); console.log(readFileSync(agentLog, "utf8").slice(-5000)); console.log(readFileSync(logPath, "utf8").slice(-4000)); } else rmSync(root, { recursive: true, force: true });
  }
  process.exit(failures ? 1 : 0);
}
