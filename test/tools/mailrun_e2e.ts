// Isolated mailrun integration. A stand-in Claude calls thread_send,
// then the spawned run decides and escalates through real MCP HTTP.
// MAILRUN_FALLBACK_E2E=1 models provider-side tool rejection before MCP:
// the run can only return its final failure report. The hub must surface it.
// No copied user log, registrations, tokens, peers or live hub access.
import { appendFileSync, chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function agent() {
  const args = process.argv.slice(3);
  const config = JSON.parse(args[args.indexOf("--mcp-config") + 1]);
  const url = config.mcpServers.backplane.url;
  const session = `mailrun-${process.pid}`;
  appendFileSync(process.env.MAILRUN_AGENT_LOG!, JSON.stringify({ args, pid: process.pid }) + "\n");
  const emit = (o: any) => console.log(JSON.stringify({ session_id: session, uuid: crypto.randomUUID(), ...o }));
  emit({ type: "system", subtype: "init", tools: [], mcp_servers: [], model: "sonnet", claude_code_version: "2.1.286", capabilities: ["msg_lifecycle_v1"] });
  let call = 0;
  const tool = async (name: string, a: object) => {
    const response = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: ++call, method: "tools/call", params: { name, arguments: a } }) });
    const text = await response.text();
    appendFileSync(process.env.MAILRUN_AGENT_LOG!, JSON.stringify({ name, reply: text }) + "\n");
    if (!response.ok || text.includes('"isError":true')) throw new Error(text);
  };
  let chain = Promise.resolve();
  createInterface({ input: process.stdin }).on("line", (line) => {
    let o: any; try { o = JSON.parse(line); } catch { return; }
    if (o.type !== "user") return;
    const text = (Array.isArray(o.message?.content) ? o.message.content : [o.message?.content ?? ""]).map((x: any) => typeof x === "string" ? x : x.text ?? "").join(" ");
    chain = chain.then(async () => {
      appendFileSync(process.env.MAILRUN_AGENT_LOG!, JSON.stringify({ pid: process.pid, prompt: text }) + "\n");
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "queued" });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "started" });
      if (text.includes("MAILRUN_E2E_SEND")) await tool("thread_send", { threadId: "foh", text: "Please choose a board colour" });
      const denied = text.includes("<charter>") && process.env.MAILRUN_FALLBACK_E2E === "1";
      if (denied) appendFileSync(process.env.MAILRUN_AGENT_LOG!, JSON.stringify({ pid: process.pid, deniedBeforeMcp: ["decide", "escalate", "mail"] }) + "\n");
      if (text.includes("<charter>") && !denied) {
        await tool("decide", { text: "Use blue", reason: "Known preference" });
        await tool("escalate", { to: "bot", text: "Confirm blue" });
      }
      const final = denied ? "Automatic approval rejected decide, escalate and mail; policy never. Nothing was recorded or sent." : "Done";
      emit({ type: "assistant", message: { id: `m${process.pid}-${call}`, role: "assistant", content: [{ type: "text", text: final }] }, parent_tool_use_id: null });
      emit({ type: "result", subtype: "success", is_error: false, num_turns: 1, result: final, usage: {} });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "completed" });
    }).catch((e) => { appendFileSync(process.env.MAILRUN_AGENT_LOG!, String(e) + "\n"); process.exit(1); });
  });
}
if (process.argv[2] === "--agent") { await agent(); } else {
  const bin = resolve(process.argv[2] ?? "build/mailrun-server");
  const wire = resolve(process.argv[3] ?? "build/mailrun-wire");
  for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), "bp-mailrun-"));
  const home = join(root, "home"), fake = join(root, "bin"), proj = join(root, "project"), bot = join(root, "bot");
  for (const d of [home, fake, proj, bot]) mkdirSync(d);
  const shell = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${shell(process.execPath)} ${shell(resolve(import.meta.path))} --agent "$@"\n`);
  chmodSync(join(fake, "claude"), 0o755);
  for (const n of ["codex", "grok", "muse", "agy"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
  const event = ($: string, fields: object) => ({ $, ...fields, at: 1 });
  const thread = (id: string, project: string, worktree: string) => event("ThreadCreated", { id, project, title: id, env: "local", branch: "", worktree, provider: "claude", model: "sonnet" });
  const logPath = join(home, "events.jsonl");
  writeFileSync(logPath, [event("ProjectCreated", { id: "p", title: "Test", root: proj }), thread("worker", "p", proj), thread("foh", "", bot), event("BotSet", { id: "b", name: "Beta", thread: "foh", look: 0, persona: "" }), event("ThreadLaunched", { id: "worker", by: "foh", report: false }), event("SettingSet", { key: "mail.on", value: "on" }), event("SettingSet", { key: "restart.continue", value: "off" })].map(JSON.stringify).join("\n") + "\n");
  const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const port = listener.port; listener.stop(true);
  const agentLog = join(root, "agents.jsonl");
  writeFileSync(agentLog, "");
  const proc = Bun.spawn([bin, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], { env: { ...process.env, HOME: root, PATH: `${fake}:${process.env.PATH}`, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", MAILRUN_AGENT_LOG: agentLog, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "deploy-lock") }, stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")) });
  const until = async (f: () => boolean, ms = 15000) => { const deadline = Date.now() + ms; while (Date.now() < deadline) { if (f()) return true; await sleep(50); } return false; };
  let ws: WebSocket | undefined;
  let failures = 0;
  const check = (ok: boolean, label: string) => { console.log(`${ok ? "ok" : "FAIL"} ${label}`); if (!ok) failures++; };
  try {
    let ready = false;
    for (let i = 0; i < 200; i++) { try { ready = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (ready) break; await sleep(50); }
    if (!ready) throw new Error("test hub failed to start");
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
    ws.send(W.encode(JSON.stringify({ id: 1, m: "turn.start", p: { thread: "worker", text: "MAILRUN_E2E_SEND", msg: "person-start" } })));
    const events = () => readFileSync(logPath, "utf8").trim().split("\n").map((x) => JSON.parse(x));
    check(await until(() => events().some((e) => e.$ === "RunSet"), 30000), "thread_send spawns a run");
    const fallback = process.env.MAILRUN_FALLBACK_E2E === "1";
    check(await until(() => events().some((e) => e.$ === "ActivityLogged" && e.kind === "escalation")), fallback ? "hub surfaces unresolved run without an agent tool call" : "run escalates through MCP");
    if (!fallback) await until(() => events().some((e) => e.$ === "DecisionLogged"));
    await sleep(800);
    const es = events(), run = es.find((e) => e.$ === "RunSet");
    check(!!run && es.some((e) => e.$ === "MessagePosted" && e.thread === run.thread && e.text.includes("<charter>") && e.text.includes("Please choose a board colour")), "first turn contains brief and triggering mail");
    check(es.filter((e) => e.$ === "ActivityLogged" && e.thread === "foh" && e.kind === "escalation").length === 1, "FOH gets exactly one escalation row");
    const inbound = es.filter((e) => e.$ === "MessagePosted" && e.thread === "foh" && e.role !== "assistant");
    check(inbound.length === 1 && inbound[0].role === "system" && inbound[0].text.startsWith("[mailroom escalation from "), "FOH contains only the escalation inbound line");
    if (fallback) {
      check(!es.some((e) => e.$ === "DecisionLogged" && e.run === run?.thread), "denied decision is not claimed as committed");
      check(inbound[0]?.text.includes("policy never") && inbound[0]?.text.includes("Actionable input:"), "owner receives actual failure and pending input identity");
      check(es.some((e) => e.$ === "SettingSet" && e.key === `run.forwarded.${run?.thread}`), "fallback disposition is durable");
    } else check(es.some((e) => e.$ === "DecisionLogged" && e.run === run?.thread && e.text === "Use blue"), "run decision durably logged");
    const agentRows = readFileSync(agentLog, "utf8").trim().split("\n").map((x) => { try { return JSON.parse(x); } catch { return {}; } });
    const first = agentRows.find((e: any) => e.prompt?.includes("<charter>"));
    const launch = agentRows.find((e: any) => e.pid === first?.pid && e.args);
    check(!!launch && !launch.args.includes("--resume") && launch.args.includes("--permission-prompt-tool"), "fresh agent session uses permission prompt");
  } finally {
    ws?.close();
    // Only the exact hub PID started above; no process-tree signalling.
    if (proc.exitCode === null) proc.kill("SIGTERM");
    await proc.exited;
    if (failures || process.env.KEEP) { console.log(`artifacts ${root}`); console.log(readFileSync(join(root, "stderr"), "utf8").slice(-2000)); console.log(readFileSync(agentLog, "utf8").slice(-5000)); console.log(readFileSync(logPath, "utf8").slice(-4000)); } else rmSync(root, { recursive: true, force: true });
  }
  process.exit(failures ? 1 : 0);
}
