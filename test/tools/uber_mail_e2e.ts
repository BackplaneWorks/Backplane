// Mail to the uberbot with automation (mail.on) off: a thread's thread_send
// to the uberbot's chat starts a private uberbot run within seconds whose
// first turn carries the text; the person-facing chat gets nothing and no
// other bot is woken. Stand-in Claude only; a throwaway home and port.
//   bun test/tools/uber_mail_e2e.ts HUB WIREDIR
// HUB: the native server (build/...) or the JS one (scripts/build-js.sh).
import { appendFileSync, chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const TEXT = "gate 4 decision needed: ship the JS hub today? UBER_MAIL_E2E";
async function agent() {
  const args = process.argv.slice(3);
  const config = JSON.parse(args[args.indexOf("--mcp-config") + 1]);
  const url = config.mcpServers.backplane.url;
  const session = `ubermail-${process.pid}`;
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
      if (text.includes("SEND_TO_UBER")) await rpc("tools/call", { name: "thread_send", arguments: { threadId: "foh", text: TEXT } });
      if (text.includes(TEXT)) await rpc("tools/call", { name: "mail_list", arguments: {} });
      emit({ type: "assistant", message: { id: `m${process.pid}-${call}`, role: "assistant", content: [{ type: "text", text: "Done" }] }, parent_tool_use_id: null });
      emit({ type: "result", subtype: "success", is_error: false, num_turns: 1, result: "Done", usage: {} });
      emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "completed" });
    }).catch((e) => { log({ error: String(e) }); process.exit(1); });
  });
}
if (process.argv[2] === "--agent") { await agent(); } else {
  const bin = resolve(process.argv[2] ?? "build/uber-mail-server");
  const wire = resolve(process.argv[3] ?? "build/uber-mail-wire");
  for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), "bp-ubermail-"));
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
    event("SettingSet", { key: "uber.bot", value: "b" }), event("SettingSet", { key: "restart.continue", value: "off" })].map((e) => JSON.stringify(e)).join("\n") + "\n");
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
    ws.send(W.encode(JSON.stringify({ id: 1, m: "turn.start", p: { thread: "worker", text: "SEND_TO_UBER", msg: "person-start" } })));
    const events = () => readFileSync(logPath, "utf8").trim().split("\n").map((x) => { try { return JSON.parse(x); } catch { return {}; } });
    const agents = () => readFileSync(agentLog, "utf8").trim().split("\n").map((x) => { try { return JSON.parse(x); } catch { return {}; } });
    check(await until(() => agents().some((e: any) => e.method === "tools/call" && e.reply?.includes("sent")), 30000), "the worker's thread_send to the uberbot is accepted");
    const sent = Date.now();
    check(await until(() => events().some((e) => e.$ === "MailPosted" && e.to === "bot:b" && e.want === "act")), "it is filed as act mail to the uberbot");
    check(await until(() => events().some((e) => e.$ === "RunSet" && e.bot === "b"), 10000), "a private uberbot run starts");
    const turned = await until(() => agents().some((e: any) => typeof e.prompt === "string" && e.prompt.includes(TEXT)), 10000);
    const took = Date.now() - sent;
    check(turned && took < 8000, `an uberbot turn carries the text within seconds (${took} ms)`);
    await sleep(1000);
    const es = events(), run = es.find((e) => e.$ === "RunSet" && e.bot === "b");
    check(!!run && es.some((e) => e.$ === "MessagePosted" && e.thread === run.thread && e.text.includes(TEXT)), "the run's first message has the full text");
    check(!es.some((e) => e.$ === "MessagePosted" && e.thread === "foh"), "nothing is posted in the person-facing chat");
    check(!es.some((e) => e.$ === "RunSet" && e.bot !== "b"), "no other bot is woken");
    check(!es.some((e) => e.$ === "SettingSet" && e.key === "mail.on"), "automation stays off");
    const first = agents().find((e: any) => typeof e.prompt === "string" && e.prompt.includes(TEXT));
    check(!!first && first.tools.includes('"mail_list"') && first.tools.includes('"mail_read"'), "the uberbot's run lists mail_list and mail_read");
    check(agents().some((e: any) => e.method === "tools/call" && e.pid === first?.pid && e.reply?.includes("conv") && !e.reply.includes('"isError":true')), "mail_list answers the uberbot's run");
    const worker = agents().find((e: any) => typeof e.prompt === "string" && e.prompt.includes("SEND_TO_UBER"));
    check(!!worker && !worker.tools.includes('"mail_list"'), "an ordinary thread lists no mail tools");
  } finally {
    ws?.close();
    // Only the exact hub PID started above.
    if (proc.exitCode === null) proc.kill("SIGTERM");
    await proc.exited;
    if (failures || process.env.KEEP) { console.log(`artifacts ${root}`); console.log(readFileSync(join(root, "stderr"), "utf8").slice(-3000)); console.log(readFileSync(agentLog, "utf8").slice(-5000)); console.log(readFileSync(logPath, "utf8").slice(-4000)); } else rmSync(root, { recursive: true, force: true });
  }
  process.exit(failures ? 1 : 0);
}
