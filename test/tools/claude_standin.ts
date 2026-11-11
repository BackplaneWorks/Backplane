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
//   (any mode)   a user line with priority "now" while a command runs
//                interrupts it and runs next, ahead of the queue
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
//                steps, the first D done, the next in progress;
//                "mcp:N" runs N Backplane MCP calls (viewer_show);
//                "replies:N" posts N short replies, each after a Bash call;
//                "hold:MS" runs a last Bash call MS before its result;
//                "agents:N" runs N background subagents (Agent calls)
//                that finish once the text is posted;
//                "agentsteps:N" gives each one N tool/text activity rows
//                (each tool call followed by its result);
//                "subwork:MS" keeps them working MS more, a step each
//                every BP_AGENT_STEP_MS (1500), then each reports;
//                "fgagent:MS" runs a foreground subagent (the turn waits
//                on its Agent call) for MS, stepping the same way;
//                "ask:" asks a question (AskUserQuestion), "approve:" a
//                Bash approval, both through permission_prompt, waiting;
//                "edit:" moves the board's first footprint 2.54 mm;
//                "write:" adds a line to README.md and writes NOTES-<tag>.md;
//                "delegate:" calls delegate_task (a plain child);
//                "mcpcall:{json}" makes one Backplane MCP call, {"name":..,"arguments":..}, and says its answer;
//                "fail:" starts, then fails the turn (an API error result)
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
//   mcpcalls     the first command makes each call of BP_MCP_CALLS (JSON: [{name, arguments}]) of the
//                hub's MCP tools in turn (a buy thread's order_quote, say), then ends; later commands as plain, answered as a buy
//                thread's agent would ("The quote is on the Order card…"), not echoed
//   deploy       the first command calls the hub's deploy_restart (the MCP
//                URL, BP_DEPLOY_ARGS, JSON, as its arguments), then ends
//   buyflow      a buy thread's agent, scripted by what each command says: BP_BUY_SCRIPT (JSON: [{when, shell?, calls?, say?}]),
//                the first entry whose `when` (a regex) matches the command's text runs: its `shell` (sh -c, in the thread's
//                folder; its output logged), then each of its `calls` of the hub's MCP tools, then it says `say`; no match: a short
//                answer ("{{out}}" in a call's arguments is the shell's last output line; a call with `if` runs only when the
//                call before answered something that regex matches). Keyed by text, not by turn, so a respawned process goes on where the thread is (test/order/buy_e2e.py)
// BP_STANDIN_IMAGES=<dir>: each image block received is written there (0, 1, ...), and
// logged as "IMAGE <path> <media type> <source type>" and "TEXT <the message's text>".
// BP_STANDIN_USAGE=1: results carry token usage and a cost (a counted turn).
// Every line read is appended to $FAKE_LOG ("IN <line>"), and "START pid".
// BP_EOF_FINISH=1: at the end of stdin the turn in flight is finished
// before the process exits, as Claude Code does.
// BP_STANDIN_MODELS=<file>: an `initialize` control request (the hub's
// model probe) is answered with that file's {"models": [...]}, as Claude
// Code's /model list (test/fixtures/claude/models-long.json); unset, it
// goes unanswered and the hub falls back to its own short list.
//
//   test/tools/claude_standin.ts is run through a `claude` wrapper:
//   #!/bin/sh
//   exec bun /path/to/claude_standin.ts "$@"
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const args = process.argv.slice(2);
// `claude --version` (the hub's probe for Settings › Agents) answers at once
if (args[0] === "--version") {
  process.stdout.write("2.1.286 (Claude Code)\n");
  process.exit(0);
}
// BP_STANDIN_SUMMARY=1: a one-shot (the hub's writing model: a title, a `/give` summary, a side question; `--no-session-persistence`) reads its text on stdin and
// answers one result, "Stand-in summary: " and the first request it read, so a handoff's summary can be told from the thread's own words
if (process.env.BP_STANDIN_SUMMARY === "1" && args.includes("--no-session-persistence")) {
  let input = "";
  for await (const c of process.stdin) input += c;
  const asked = /User: ([^\n]*)/.exec(input)?.[1] ?? "";
  process.stdout.write(JSON.stringify({ type: "result", subtype: "success", is_error: false, result: `Stand-in summary: ${asked.slice(0, 100)}`, session_id: "one", uuid: crypto.randomUUID() }) + "\n");
  process.exit(0);
}
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
const stoppedTasks = new Set<string>();
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
  if (firstOne && mode === "mcpcalls") {
    await sleep(Number(process.env.BP_MCP_DELAY_MS ?? 0)); // a run may look away first (a need raised on a thread the window is not watching)
    // BP_MCP_CALLS: a JSON list of {name, arguments}, each a real call of the hub's MCP tool in turn, each shown as a tool row
    for (const t of JSON.parse(process.env.BP_MCP_CALLS ?? "[]") as { name: string; arguments: object }[]) {
      let got = "";
      try {
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: t.name, arguments: t.arguments } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`MCPCALL ${t.name} ${got.slice(0, 400)}`);
    }
    say("ASKED");
    result("ASKED");
    lc(c.uuid, "completed");
    return;
  }
  if (mode === "buyflow") {
    type Step = { when: string; shell?: string; calls?: { name: string; arguments: object; if?: string }[]; say?: string };
    const step = (JSON.parse(process.env.BP_BUY_SCRIPT ?? "[]") as Step[]).find((s) => new RegExp(s.when).test(c.text));
    log(`BUYFLOW ${step ? step.when : "-"} ${c.text.slice(0, 120).replace(/\n/g, " ")}`);
    let last = ""; // the shell's last line of output: "{{out}}" in a call's arguments (the folder it exported, say)
    if (step?.shell) {
      const out = await new Promise<string>((res) => {
        const p = spawn("sh", ["-c", step.shell!], { cwd: process.cwd() }); let o = "", e = "";
        p.stdout.on("data", (d) => (o += d)); p.stderr.on("data", (d) => (e += d)); p.on("close", (code) => { last = o.trim().split("\n").pop() ?? ""; res(`exit ${code}: ${(o + e).slice(-600)}`); });
      });
      log(`SHELL ${out.replace(/\n/g, " | ")}`);
    }
    let prev = "";
    for (const t of step?.calls ?? []) {
      if (t.if && !new RegExp(t.if).test(prev)) { log(`MCPSKIP ${t.name}`); continue; } // only when the call before answered so (a gate that passed, say)
      let got = "";
      try {
        const args = JSON.parse(JSON.stringify(t.arguments).replaceAll("{{out}}", last));
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: t.name, arguments: args } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`MCPCALL ${t.name} ${got.slice(0, 400)}`);
      prev = got;
    }
    const reply = step?.say ?? "Noted.";
    say(reply);
    result(reply);
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
      if (stoppedTasks.has(t.task)) return;
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
    // "ask:" / "approve:": a question (AskUserQuestion) or a tool approval
    // (Bash) through the hub's permission_prompt, waiting for the answer
    for (const [kw, tool, input] of [
      ["ask:", "AskUserQuestion", { questions: [{ question: `Which connector for ${tag}?`, header: "Connector", multiSelect: false,
        options: [{ label: "USB-C", description: "reversible" }, { label: "Micro-B", description: "cheaper" }] }] }],
      ["approve:", "Bash", { command: `rm -rf build/ && kicad-cli pcb export gerbers board.kicad_pcb # ${tag}` }],
    ] as const) {
      if (!c.text.includes(kw) || !mcpUrl) continue;
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: tool, input }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      let got = "";
      try {
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "permission_prompt", arguments: { tool_name: tool, input, tool_use_id: id } } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`PERMIT ${tool} ${got.slice(0, 300)}`);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: got.includes("allow") ? `answered: ${got.slice(0, 200)}` : "denied", is_error: !got.includes("allow") }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "mcpcall:{json}": one Backplane MCP tool call, {"name":"preview_evaluate","arguments":{...}}, its answer in the tool result
    const mc = /mcpcall:(\{.*\})/.exec(c.text);
    let mcGot: string | null = null; // the call made here: the reply below says it, and does not make it a second time
    if (mc && mcpUrl) {
      const call = JSON.parse(mc[1]) as { name: string; arguments: unknown };
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: `mcp__backplane__${call.name}`, input: call.arguments }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      let got = "";
      try {
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: call.name, arguments: call.arguments } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`MCPCALL ${call.name} ${got.slice(0, 600)}`);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: `MCPRESULT ${got.slice(0, 1500)}` }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      mcGot = got;
    }
    // "edit:": an Edit call that moves the board's first footprint 2.54 mm right
    if (c.text.includes("edit:")) {
      const fs = await import("node:fs");
      const f = fs.readdirSync(process.cwd()).find((n) => n.endsWith(".kicad_pcb"));
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Edit", input: { file_path: `${process.cwd()}/${f}`, old_string: "(at", new_string: "(at" } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      if (f) {
        const s = fs.readFileSync(f, "utf8");
        const m = /\(footprint [^\n]*\n(?:[^\n]*\n){0,6}?\s*\(at ([\d.-]+) ([\d.-]+)/.exec(s);
        if (m) {
          const at = m.index + m[0].lastIndexOf("(at ");
          const old = `(at ${m[1]} ${m[2]}`;
          fs.writeFileSync(f, s.slice(0, at) + `(at ${(Number(m[1]) + 2.54).toFixed(2)} ${m[2]}` + s.slice(at + old.length));
        }
      }
      await sleep(ms);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: f ? `edited ${f}` : "no board" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "write:": a Write call that adds a line to README.md (a tracked file) and writes NOTES-<tag>.md (a new one), in the turn's folder
    if (c.text.includes("write:")) {
      const fs = await import("node:fs");
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Write", input: { file_path: `${process.cwd()}/NOTES-${tag}.md`, content: `notes ${tag}\n` } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      fs.appendFileSync("README.md", `a line from ${tag}\n`);
      fs.writeFileSync(`NOTES-${tag}.md`, `notes ${tag}\n`);
      await sleep(ms);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: `wrote README.md and NOTES-${tag}.md` }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "delegate:": hands a child task to another thread (delegate_task)
    if (c.text.includes("delegate:") && mcpUrl) {
      const id = `toolu_${rnd()}`;
      const args = { prompt: `mode:plain check the footprints for ${tag}`, provider: "claude", title: `Footprint check ${tag}` };
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "mcp__backplane__delegate_task", input: args }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      let got = "";
      try {
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "delegate_task", arguments: args } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`DELEGATED ${got.slice(0, 300)}`);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: got.slice(0, 400) }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "mcpcall:{json}": one Backplane MCP call, {"name":"thread_launch","arguments":{..}} (a thread elsewhere, a room...), its answer posted as the reply
    if (c.text.includes("mcpcall:") && mcpUrl && mcGot !== null) { const got = mcGot; say(`MCPCALL ${JSON.parse(mc![1]).name} ${got.slice(0, 600)}`); }
    else if (c.text.includes("mcpcall:") && mcpUrl) {
      const id = `toolu_${rnd()}`;
      let call: { name: string; arguments: unknown } = { name: "", arguments: {} };
      try { call = JSON.parse(c.text.slice(c.text.indexOf("mcpcall:") + 8)); } catch (e) { log(`MCPCALL bad json ${e}`); }
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: `mcp__backplane__${call.name}`, input: call.arguments }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      let got = "";
      try {
        const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: call.name, arguments: call.arguments } }) });
        got = await r.text();
      } catch (e) { got = String(e); }
      log(`MCPCALL ${call.name} ${got.slice(0, 600)}`);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: got.slice(0, 600) }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      say(`MCPCALL ${call.name} ${got.slice(0, 600)}`);
    }
    // "fail:": streams a little, then the API fails the turn (an error result)
    if (c.text.includes("fail:")) {
      emit({ type: "stream_event", event: { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      emit({ type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: `Starting on ${tag}… ` } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(ms * 5);
      say("API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}");
      result("", true);
      lc(c.uuid, "completed");
      return;
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
    // "mcp:N": N calls of a Backplane MCP tool, each answered
    for (let k = 1; k <= Number(/mcp:(\d+)/.exec(c.text)?.[1] ?? 0); k++) {
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "mcp__backplane__viewer_show", input: { path: "board.kicad_pcb" } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(ms);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "shown" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "replies:N": N short replies in the turn, each after a Bash call
    for (let k = 1; k <= Number(/replies:(\d+)/.exec(c.text)?.[1] ?? 0); k++) {
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Bash", input: { command: `make check${k} # ${tag}` } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(ms);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "ok" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "text", text: `Reply ${k} of ${tag}: check ${k} passed.` }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(ms);
    }
    // "hold:MS": a last Bash call that runs MS before its result
    const hold = Number(/hold:(\d+)/.exec(c.text)?.[1] ?? 0);
    if (hold > 0) {
      const id = `toolu_${rnd()}`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id, name: "Bash", input: { command: `kicad-cli pcb drc board.kicad_pcb\n# ${tag}` } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      await sleep(hold);
      emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "0 violations" }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "agents:N": N subagents in the background (task_started), ended after the text
    const agents = Array.from({ length: Number(/agents:(\d+)/.exec(c.text)?.[1] ?? 0) }, (_, k) => {
      // as Claude Code 2.1.286 says it: the Agent call's input, task_started's
      // subagent_type and prompt (every other one an Explore on haiku)
      const task = `a${process.pid}x${rnd()}`, tool = `toolu_${task}`, description = `Subagent ${k + 1} for ${tag}`;
      const subagent_type = k % 2 ? "Explore" : "general-purpose", prompt = `Look into ${tag}: read the board files and report what you find.\nKeep it short.`;
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id: tool, name: "Agent", input: { description, prompt, subagent_type, ...(k % 2 ? { model: "haiku" } : {}), run_in_background: true } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      emit({ type: "system", subtype: "task_started", task_id: task, tool_use_id: tool, description, subagent_type, prompt, task_type: "local_agent", is_backgrounded: true, session_id: session });
      emit({ type: "user", message: { role: "user", content: [{ tool_use_id: tool, type: "tool_result", content: `Async agent launched: ${task}` }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      return { task, tool };
    });
    const agentSteps = Number(/agentsteps:(\d+)/.exec(c.text)?.[1] ?? 0);
    // a subagent's own step, as Claude Code says it: its tool call (and its result, a "user" line under the same parent) or its text
    const subStep = (parent: string, step: number, what: string) => {
      const id = `toolu_${rnd()}`;
      const content = step % 2
        ? [{ type: "tool_use", id, name: "Bash", input: { command: `inspect ${what} step ${step}` } }]
        : [{ type: "text", text: `Checked ${what} step ${step}` }];
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, content }, parent_tool_use_id: parent, session_id: session });
      if (step % 2) emit({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: `step ${step}: 3 footprints, 1 without a model`, is_error: step % 6 === 5 }] }, parent_tool_use_id: parent, session_id: session, uuid: rnd() });
    };
    // "fgagent:MS": a foreground subagent (an Agent call the turn waits on, no task_started) that works MS, a step every BP_AGENT_STEP_MS (1500), then answers
    const fgHold = Number(/fgagent:(\d+)/.exec(c.text)?.[1] ?? 0);
    if (fgHold > 0) {
      const tool = `toolu_fg${rnd()}`, gap = Number(process.env.BP_AGENT_STEP_MS ?? 1500);
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, type: "message", role: "assistant", model: "claude-haiku-4-5-20251001",
        content: [{ type: "tool_use", id: tool, name: "Agent", input: { description: `Foreground look at ${tag}`, prompt: `Check ${tag} by hand and answer when done.`, subagent_type: "Explore" } }], usage: { input_tokens: 10, output_tokens: 5 } }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
      const end = Date.now() + fgHold;
      for (let step = 1; Date.now() < end; step++) { subStep(tool, step, `${tag} (fg)`); if (await waitOrInterrupt(gap)) break; }
      emit({ type: "assistant", message: { id: `msg_${rnd()}`, content: [{ type: "text", text: `Foreground check of ${tag}: all footprints have models.` }] }, parent_tool_use_id: tool, session_id: session });
      emit({ type: "user", message: { role: "user", content: [{ tool_use_id: tool, type: "tool_result", content: [{ type: "text", text: `Foreground check of ${tag}: all footprints have models.` }] }] }, parent_tool_use_id: null, session_id: session, uuid: rnd() });
    }
    // "subwork:MS": the background subagents keep working MS after their agentsteps, a step each every BP_AGENT_STEP_MS (1500), then report
    const subWork = Number(/subwork:(\d+)/.exec(c.text)?.[1] ?? 0);
    for (const a of agents) {
      for (let step = 1; step <= agentSteps; step++) {
        subStep(a.tool, step, tag);
        emit({ type: "system", subtype: "task_progress", task_id: a.task, tool_use_id: a.tool,
          description: `Checking ${tag} step ${step}`, usage: { total_tokens: 1200 * step, tool_uses: Math.ceil(step / 2), duration_ms: step * ms }, last_tool_name: step % 2 ? "Bash" : undefined, session_id: session });
        await sleep(ms);
      }
    }
    if (subWork > 0 && agents.length) {
      const end = Date.now() + subWork, gap = Number(process.env.BP_AGENT_STEP_MS ?? 1500);
      for (let step = agentSteps + 1; Date.now() < end; step++) {
        for (const a of agents) {
          subStep(a.tool, step, tag);
          emit({ type: "system", subtype: "task_progress", task_id: a.task, tool_use_id: a.tool, description: `Checking ${tag} step ${step}`, usage: { total_tokens: 1200 * step, tool_uses: Math.ceil(step / 2), duration_ms: step * gap }, session_id: session });
        }
        if (await waitOrInterrupt(gap)) break;
      }
      for (const a of agents) emit({ type: "assistant", message: { id: `msg_${rnd()}`, content: [{ type: "text", text: `Report on ${tag}: one footprint lacks a 3D model.` }] }, parent_tool_use_id: a.tool, session_id: session });
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
    for (const a of agents)
      emit({ type: "system", subtype: "task_notification", task_id: a.task, tool_use_id: a.tool, status: "completed", summary: `${a.task} done`,
        usage: { total_tokens: 1200 * agentSteps + 800, tool_uses: Math.ceil(agentSteps / 2), duration_ms: 5000 }, session_id: session });
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
  // mcpcalls (a buy thread's agent): Backplane's notes about the quote get an agent's short answer, not an echo the screenshots would show
  const reply = mode === "mcpcalls" ? "The quote is on the Order card: Buy now there when you are ready." : c.text.includes("two") ? "TWO" : c.text.includes("one") ? "ONE" : `REPLY ${c.text.slice(0, 40)}`;
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

let imagesSeen = 0;
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
  // stop_task: one background task stops (its row says "stopped"), the rest and the turn go on
  if (o.type === "control_request" && o.request?.subtype === "stop_task") {
    const task = String(o.request.task_id ?? "");
    emit({ type: "control_response", response: { subtype: "success", request_id: o.request_id, response: {} } });
    emit({ type: "system", subtype: "task_updated", task_id: task, patch: { status: "stopped" }, session_id: session });
    stoppedTasks.add(task); if (wake?.task === task) { wake = null; wakeDue = false; }
    log(`STOPTASK ${task}`);
    return;
  }
  if (o.type === "control_request" && o.request?.subtype === "initialize" && process.env.BP_STANDIN_MODELS) {
    const models = JSON.parse(readFileSync(process.env.BP_STANDIN_MODELS, "utf8")).models;
    emit({ type: "control_response", response: { subtype: "success", request_id: o.request_id, response: { models } } });
    return;
  }
  if (o.type !== "user") return;
  const text = (o.message?.content ?? []).map((b: any) => (typeof b === "string" ? b : b.text ?? "")).join(" ");
  if (process.env.BP_STANDIN_IMAGES) for (const b of o.message?.content ?? []) if (b?.type === "image") {
    const at = `${process.env.BP_STANDIN_IMAGES}/${imagesSeen++}`;
    writeFileSync(at, Buffer.from(b.source?.data ?? "", "base64"));
    log(`IMAGE ${at} ${b.source?.media_type} ${b.source?.type}`);
    log(`TEXT ${JSON.stringify(text)}`);
  }
  if (!mode) mode = /mode:([\w-]+)/.exec(text)?.[1] ?? "plain";
  const uuid = o.uuid ?? rnd();
  log(`USER ${uuid} ${text.slice(0, 80)}`);
  lc(uuid, "queued");
  // priority "now" (a person's message into a running turn): as Claude Code does, the running
  // command is interrupted (its partial text kept, the turn ends cancelled) and this one runs next
  if (o.priority === "now" && busy) {
    log(`NOW ${uuid}`);
    interrupted = true;
    queue.unshift({ uuid, text });
    return;
  }
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
