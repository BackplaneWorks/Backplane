// A stand-in `claude` for end-to-end tests: Claude Code 2.1.286's
// stream-json, shaped like the recordings in test/fixtures/claude/
// (bg-wake.jsonl, stop-steer.jsonl), keyed to the uuids the hub sends. No
// model runs. Each user line's uuid gets command_lifecycle queued, started
// and completed (or cancelled), its result before its completion; commands
// queue and run one at a time; an interrupt answers {still_queued: []} and
// cancels the running command and the queued ones.
//
// The mode is BP_STANDIN_MODE, or "mode:<name>" in the first message:
//   wake         the first command starts a background shell that ends
//                BP_BG_MS later (2500); Claude then wakes by itself
//                (bg-wake.jsonl l.17-23)
//   bg           the same; messages after the first take BP_WORK_MS
//   stale-first  the live repro: on the first message, a whole stale
//                notification turn runs before the command starts
//   stop         a command works until it is interrupted (20 s at most)
//   pair         each command works BP_WORK_MS (1500), so a second sent
//                at once waits queued (stop-steer.jsonl l.20-33)
//   noLC         an old CLI: no lifecycles, init says 2.0.0
//   exit-held    a child holds stdout open (30 s), the process exits 3
//   result-exit  a result, then exit with a child holding stdout
//   tail-nonl    an old CLI whose last line, its result, has no newline,
//                then exit with a child holding stdout
//   exit-now     the process exits (3) as the first command starts
//   bg-exit      the first command starts a background shell, answers,
//                and the process exits (0) 600 ms later, the shell with it
//   oom          the first command allocates 20 MB blocks (600 MB at most,
//                touched) until the kernel ends it; BP_ALLOC_MB (600)
//   plain        each command: init, an answer, its result
//   stream       each command streams BP_STREAM_N (40) text deltas
//                "<tag>0 <tag>1 ..." (tag: the message's last word),
//                BP_STREAM_MS (150) apart, then posts them whole (each
//                followed by BP_STREAM_SEP, " "; "\n\n" makes lines);
//                "tools:N" in the message runs N Bash calls first;
//                "todos:N/D" sets a to-do list first (TodoWrite): N
//                steps, the first D done, the next in progress
//   delegate     the first command calls the hub's delegate_task (its MCP
//                URL from --mcp-config) for a child "mode:plain child
//                work", then ends; later commands as plain
//   launch       the same with thread_launch (BP_LAUNCH_ARGS, JSON, merged
//                into its arguments), holding the turn BP_LAUNCH_HOLD_MS
//                (0) before it ends; later commands as plain
//   toolhold     the first command starts a Bash tool call and holds it
//                BP_TOOL_MS (6000) before its result, then answers
//   toolgap      the first command runs two tool calls in a row with no text
//                between, each answered at once, and sleeps BP_GAP_MS (30000)
//                between them, outside any tool call, then answers
//   lostend      each command answers and has its result, but its lifecycle
//                never completes (the line lost); the process stays, silent
//   deploy       the first command calls the hub's deploy_restart (the MCP
//                URL, BP_DEPLOY_ARGS, JSON, as its arguments), then ends
// BP_STANDIN_USAGE=1: results carry token usage and a cost (a counted turn).
// Every line read is appended to $FAKE_LOG ("IN <line>"), and "START pid".
// BP_EOF_FINISH=1: at the end of stdin the turn in flight is finished
// before the process exits, as Claude Code does.
//
//   test/tools/claude_standin.ts is run through a `claude` wrapper:
//   #!/bin/sh
//   exec bun /path/to/claude_standin.ts "$@"
import { appendFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const args = process.argv.slice(2);
const at = args.indexOf("--resume");
const session = at >= 0 ? args[at + 1] : `sess-${process.pid}-${Date.now()}`;
const logf = process.env.FAKE_LOG ?? "";
const log = (s: string) => {
  if (logf) appendFileSync(logf, s + "\n");
};
log(`START ${process.pid} ${args.join(" ").slice(0, 200).replace(/\n/g, " ")}`);
log(`ARGV ${JSON.stringify(args)}`);
const workMs = Number(process.env.BP_WORK_MS ?? 1500);
const bgMs = Number(process.env.BP_BG_MS ?? 2500);
let mode = process.env.BP_STANDIN_MODE ?? "";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const rnd = () => crypto.randomUUID();
const mcpAt = args.indexOf("--mcp-config");
const mcpUrl = (() => {
  try { return JSON.parse(args[mcpAt + 1]).mcpServers.backplane.url as string; } catch { return ""; }
})();
if (mcpUrl) log(`MCP ${process.pid} ${mcpUrl}`);

// (with BP_EOF_FINISH, like Claude Code, a stdout nobody reads any more
// is no reason to stop)
if (process.env.BP_EOF_FINISH === "1") process.stdout.on("error", () => {});
const emit = (o: object) => process.stdout.write(JSON.stringify(o) + "\n");
const lc = (u: string, state: string) => {
  if (mode !== "noLC" && mode !== "tail-nonl") emit({ type: "command_lifecycle", command_uuid: u, state, uuid: rnd(), session_id: session });
};
const init = () =>
  emit({ type: "system", subtype: "init", cwd: process.cwd(), session_id: session, tools: ["Bash"], mcp_servers: [], model: "claude-haiku-4-5-20251001",
    permissionMode: "bypassPermissions", apiKeySource: "none", claude_code_version: mode === "noLC" || mode === "tail-nonl" ? "2.0.0" : "2.1.286",
    capabilities: mode === "noLC" ? [] : ["interrupt_receipt_v1", "interrupt_cancel_queued_v1", "msg_lifecycle_v1"], uuid: rnd() });
let msgN = 0;
const say = (text: string) => {
  const id = `msg_${process.pid}_${++msgN}`;
  emit({ type: "assistant", message: { id, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001", content: [{ type: "thinking", thinking: "", signature: "x" }], usage: { input_tokens: 10, output_tokens: 1 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
  emit({ type: "assistant", message: { id, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001", content: [{ type: "text", text }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
};
const result = (text: string, err = false) =>
  emit(err
    ? { type: "result", subtype: "error_during_execution", duration_ms: 10, is_error: true, num_turns: 1, session_id: session, total_cost_usd: 0, usage: {}, modelUsage: {}, uuid: rnd() }
    : { type: "result", subtype: "success", is_error: false, duration_ms: 10, num_turns: 1, result: text, session_id: session,
        // BP_STANDIN_USAGE=1: a counted turn, as Claude's results carry (token stats)
        ...(process.env.BP_STANDIN_USAGE === "1"
          ? { total_cost_usd: 0.0123, usage: { input_tokens: 1200, cache_creation_input_tokens: 300, cache_read_input_tokens: 4000, output_tokens: 450 } }
          : { total_cost_usd: 0, usage: {} }),
        modelUsage: { "claude-haiku-4-5-20251001": { inputTokens: 10, outputTokens: 5, contextWindow: 200000 } }, uuid: rnd() });

// background tasks (Bash run_in_background)
let bgN = 0;
const bgStart = () => {
  const task = `b${process.pid}x${++bgN}`;
  const tool = `toolu_${task}`;
  emit({ type: "assistant", message: { id: `msg_bg_${task}`, role: "assistant", content: [{ type: "tool_use", id: tool, name: "Bash", input: { command: "sleep 8; echo done-bg", run_in_background: true } }], usage: { input_tokens: 1 } }, parent_tool_use_id: null, session_id: session });
  emit({ type: "system", subtype: "background_tasks_changed", tasks: [{ task_id: task, task_type: "local_bash", description: "sleep 8; echo done-bg" }], session_id: session });
  emit({ type: "system", subtype: "task_started", task_id: task, tool_use_id: tool, description: "sleep 8; echo done-bg", is_backgrounded: true, task_type: "local_bash", session_id: session });
  emit({ type: "user", message: { role: "user", content: [{ tool_use_id: tool, type: "tool_result", content: `Command running in background with ID: ${task}.` }] }, parent_tool_use_id: null, session_id: session });
  return { task, tool };
};

type Cmd = { uuid: string; text: string };
const queue: Cmd[] = [];
let busy = false;
let interrupted = false;
let wake: { task: string; tool: string } | null = null;
let wakeDue = false;
let first = true;

const waitOrInterrupt = async (ms: number) => {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (interrupted) return true;
    await sleep(20);
  }
  return interrupted;
};

async function wakeTurn() {
  const w = wake!;
  wake = null;
  wakeDue = false;
  busy = true;
  emit({ type: "system", subtype: "background_tasks_changed", tasks: [], session_id: session });
  emit({ type: "system", subtype: "task_updated", task_id: w.task, patch: { status: "completed", end_time: Date.now() }, session_id: session });
  emit({ type: "system", subtype: "task_notification", task_id: w.task, tool_use_id: w.tool, status: "completed", output_file: `/tmp/standin/${w.task}.output`,
    summary: 'Background command "sleep 8; echo done-bg" completed (exit code 0)', session_id: session });
  init();
  await sleep(300);
  say("FINISHED");
  result("FINISHED");
  log("WOKE");
  busy = false;
}

async function run(c: Cmd, firstOne: boolean) {
  interrupted = false;
  if (firstOne && mode === "stale-first") {
    emit({ type: "system", subtype: "task_notification", task_id: "old1", tool_use_id: "toolu_old1", status: "completed", summary: "An old background command completed", session_id: session });
    init();
    say("STALE");
    result("STALE");
    log("STALE");
    await sleep(2000);
  }
  lc(c.uuid, "started");
  init();
  if (mode === "lostend") {
    // the command's answer and result, but its lifecycle never completes
    // (as when that line is lost): the process lives on, silent
    say("LOST END");
    result("LOST END");
    log("LOSTEND");
    return;
  }
  if (firstOne && mode === "hold-child") {
    // a shell or build the agent started, in the agent's scope, holding nothing open
    const ch = spawn("sleep", ["60"], { stdio: "ignore", detached: true });
    ch.unref();
    log(`CHILD ${ch.pid}`);
  }
  if (firstOne && mode === "exit-now") {
    log(`EXIT ${process.pid}`);
    process.stderr.write("exit-now: the stand-in quits here (api key=hunter2)\n");
    process.exit(3);
  }
  if (firstOne && mode === "bg-exit") {
    bgStart();
    say("STARTED");
    result("STARTED");
    lc(c.uuid, "completed");
    log(`EXIT ${process.pid}`);
    setTimeout(() => process.exit(0), 600);
    return;
  }
  if (firstOne && mode === "tail-nonl") {
    say("TAIL");
    process.stdout.write(JSON.stringify({ type: "result", subtype: "success", is_error: false, result: "TAIL", session_id: session, uuid: rnd() }));
    spawn("sleep", ["30"], { stdio: ["ignore", "inherit", "ignore"], detached: true }).unref();
    log(`EXIT ${process.pid}`);
    await sleep(100);
    process.exit(0);
  }
  if (firstOne && (mode === "exit-held" || mode === "result-exit")) {
    if (mode === "result-exit") {
      say("BYE");
      result("BYE");
      lc(c.uuid, "completed");
    }
    spawn("sleep", ["30"], { stdio: ["ignore", "inherit", "ignore"], detached: true }).unref();
    log(`EXIT ${process.pid}`);
    await sleep(100);
    process.exit(mode === "exit-held" ? 3 : 0);
  }
  if (firstOne && mode === "oom") {
    log(`ALLOC ${process.pid}`);
    const keep: Buffer[] = [];
    for (let i = 0; i < Number(process.env.BP_ALLOC_MB ?? 600) / 20; i++) {
      keep.push(Buffer.alloc(20 * 1024 * 1024, 1));
      await sleep(20);
    }
    log(`SURVIVED ${keep.length}`);
    say("ALIVE");
    result("ALIVE");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && mode === "delegate") {
    let task = "";
    try {
      const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "delegate_task", arguments: { prompt: process.env.BP_CHILD_PROMPT ?? "mode:plain child work", provider: "claude" } } }) });
      task = await r.text();
    } catch (e) { task = String(e); }
    log(`DELEGATED ${task.slice(0, 300)}`);
    say("DELEGATED");
    result("DELEGATED");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && mode === "toolhold") {
    const id = `toolu_${rnd()}`;
    emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
      content: [{ type: "tool_use", id, name: "Bash", input: { command: "make all" } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    log("TOOL started");
    await sleep(Number(process.env.BP_TOOL_MS ?? 6000));
    emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "built" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    log("TOOL ended");
    say("BUILT");
    result("BUILT");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && mode === "toolgap") {
    const call = async (n: number) => {
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Bash", input: { command: `make step${n}` } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      log(`GAP tool ${n} started`);
      await sleep(300);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "ok" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      log(`GAP tool ${n} ended`);
    };
    await call(1);
    await sleep(Number(process.env.BP_GAP_MS ?? 30000));
    log("GAP over");
    await call(2);
    say("GAPPED");
    result("GAPPED");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && mode === "deploy") {
    let got = "";
    try {
      const extra = JSON.parse(process.env.BP_DEPLOY_ARGS ?? "{}");
      const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "deploy_restart", arguments: extra } }) });
      got = await r.text();
    } catch (e) { got = String(e); }
    log(`DEPLOY ${got.slice(0, 400)}`);
    say("SCHEDULED");
    result("SCHEDULED");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && mode === "launch") {
    let got = "";
    try {
      const extra = JSON.parse(process.env.BP_LAUNCH_ARGS ?? "{}");
      const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "thread_launch", arguments: { prompt: process.env.BP_CHILD_PROMPT ?? "mode:plain child work", provider: "claude", title: "launched child", ...extra } } }) });
      got = await r.text();
    } catch (e) { got = String(e); }
    log(`LAUNCHED ${got.slice(0, 400)}`);
    await sleep(Number(process.env.BP_LAUNCH_HOLD_MS ?? 0));
    say("LAUNCHED");
    result("LAUNCHED");
    lc(c.uuid, "completed");
    return;
  }
  if (firstOne && (mode === "wake" || mode === "bg")) {
    const t = bgStart();
    say("STARTED");
    result("STARTED");
    lc(c.uuid, "completed");
    setTimeout(() => {
      wake = t;
      wakeDue = true;
      pump();
    }, bgMs);
    return;
  }
  if (mode === "stream") {
    const tag = c.text.trim().split(/\s+/).pop() ?? "w";
    const n = Number(process.env.BP_STREAM_N ?? 40);
    const ms = Number(process.env.BP_STREAM_MS ?? 150);
    // "tools:N" in the message: N Bash calls, each answered, before the text
    const tools = Number(/tools:(\d+)/.exec(c.text)?.[1] ?? 0);
    for (let k = 1; k <= tools; k++) {
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Bash", input: { command: `grep -n ${tag} board.kicad_pcb # ${k}` } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(ms);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: `${k}: (footprint "${tag}" (layer "F.Cu"))` }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "todos:N/D": the agent's to-do list, N steps with the first D done
    const todo = /todos:(\d+)\/(\d+)/.exec(c.text);
    if (todo) {
      const n = Number(todo[1]), d = Number(todo[2]), id = `toolu_${rnd()}`;
      const todos = Array.from({ length: n }, (_, k) => ({ content: `Step ${k + 1} of ${tag}`, activeForm: `Doing step ${k + 1}`,
        status: k < d ? "completed" : k === d ? "in_progress" : "pending" }));
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "TodoWrite", input: { todos } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "Todos have been modified successfully." }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    let full = "";
    emit({ type: "stream_event", event: { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    for (let i = 0; i < n; i++) {
      const piece = `${tag}${i}${process.env.BP_STREAM_SEP ?? " "}`;
      full += piece;
      emit({ type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: piece } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      if (await waitOrInterrupt(ms)) break;
    }
    log(`STREAMED ${tag}`);
    say(full);
    result(full);
    lc(c.uuid, "completed");
    return;
  }
  const stopped = await waitOrInterrupt(mode === "stop" ? 20000 : mode === "plain" ? 200 : workMs);
  if (stopped) {
    say("1. One is the first number and the begin");
    emit({ type: "user", message: { role: "user", content: [{ type: "text", text: "[Request interrupted by user]" }] }, parent_tool_use_id: null, session_id: session });
    result("", true);
    lc(c.uuid, "cancelled");
    return;
  }
  const reply = c.text.includes("two") ? "TWO" : c.text.includes("one") ? "ONE" : `REPLY ${c.text.slice(0, 40)}`;
  say(reply);
  result(reply);
  lc(c.uuid, "completed");
}

async function pump() {
  if (busy) return;
  busy = true;
  for (;;) {
    const c = queue.shift();
    if (!c) break;
    const f = first;
    first = false;
    await run(c, f);
  }
  busy = false;
  if (wakeDue) await wakeTurn();
  if (queue.length) pump();
}

const rl = createInterface({ input: process.stdin });
rl.on("line", (line) => {
  log(`IN ${line.slice(0, 400)}`);
  let o: any;
  try { o = JSON.parse(line); } catch { return; }
  if (o.type === "control_request" && o.request?.subtype === "interrupt") {
    emit({ type: "control_response", response: { subtype: "success", request_id: o.request_id, response: { still_queued: [] } } });
    interrupted = true;
    for (const c of queue.splice(0)) lc(c.uuid, "cancelled");
    log("INT");
    return;
  }
  if (o.type !== "user") return;
  const text = (o.message?.content ?? []).map((b: any) => (typeof b === "string" ? b : b.text ?? "")).join(" ");
  if (!mode) mode = /mode:([\w-]+)/.exec(text)?.[1] ?? "plain";
  const uuid = o.uuid ?? rnd();
  log(`USER ${uuid} ${text.slice(0, 80)}`);
  lc(uuid, "queued");
  queue.push({ uuid, text });
  pump();
});
rl.on("close", () => {
  log(`EOF ${process.pid}`);
  // BP_EOF_FINISH=1: as Claude Code does, the turn in flight is finished
  // first (its lines go to a stdout nobody may read any more)
  if (process.env.BP_EOF_FINISH === "1") {
    const done = () => (busy || queue.length ? setTimeout(done, 100) : (log(`FINISHED ${process.pid}`), setTimeout(() => process.exit(0), 50)));
    done();
    return;
  }
  setTimeout(() => process.exit(0), 50);
});
process.on("SIGTERM", () => {
  log(`TERM ${process.pid}`);
  process.exit(0);
});
