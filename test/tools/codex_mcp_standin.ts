// A stand-in `codex exec --json` that calls Backplane's MCP endpoint while
// its turn runs (for ride_e2e.ts). The prompt is the last argument; the
// endpoint comes from `-c mcp_servers.backplane.url="..."`. Everything it
// sees goes to $RIDE_LOG as one JSON line per event:
//   {"ev":"start","prompt":...}             each process
//   {"ev":"result","n":i,"text":...}        each tool result's text
//   {"ev":"last"}                           after its last tool call
// A prompt with "ride-long" calls thread_list RIDE_CALLS times (12) every
// RIDE_GAP_MS (700), waits RIDE_TAIL_MS (2500), then ends its turn; any
// prompt with "ride-quiet" calls nothing for RIDE_QUIET_MS (20000), then
// ends its turn; "ride-cut:<thread>" opens an MCP tool call, thread_wait on
// that thread, logs how it ended ({"ev":"cut",...}) and then hangs (a
// restart cuts it); any other prompt ends its turn at once. SIGTERM is
// logged ({"ev":"term"}).
import { appendFileSync } from "node:fs";

const argv = process.argv.slice(2);
const log = (o: object) => appendFileSync(process.env.RIDE_LOG!, JSON.stringify(o) + "\n");
const say = (o: object) => process.stdout.write(JSON.stringify(o) + "\n");
const prompt = argv.at(-1) ?? "";
let url = "";
for (let i = 0; i < argv.length; i++) {
  const m = argv[i] === "-c" ? /^mcp_servers\.backplane\.url="(.*)"$/.exec(argv[i + 1] ?? "") : null;
  if (m) url = m[1];
}
if (argv[0] === "--version") { console.log("codex-cli 0.0.0"); process.exit(0); }
log({ ev: "start", prompt, pid: process.pid, args: argv.slice(0, -1) });
process.on("SIGTERM", () => { log({ ev: "term", pid: process.pid }); process.exit(143); });
say({ type: "thread.started", thread_id: "cx-ride" });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
if (prompt.includes("ride-long")) {
  const calls = Number(process.env.RIDE_CALLS ?? 12), gap = Number(process.env.RIDE_GAP_MS ?? 700);
  for (let n = 1; n <= calls; n++) {
    await sleep(gap);
    let text = "";
    try {
      const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: n, method: "tools/call", params: { name: "thread_list", arguments: {} } }) });
      const o: any = await r.json();
      text = (o?.result?.content ?? []).map((c: any) => String(c?.text ?? "")).join("\n");
    } catch (e) { text = "ERROR " + e; }
    log({ ev: "result", n, text });
    say({ type: "item.completed", item: { id: `c${n}`, type: "agent_message", text: `call ${n}` } });
  }
  log({ ev: "last" });
  await sleep(Number(process.env.RIDE_TAIL_MS ?? 2500));
} else if (/ride-cut:(\S+)/.test(prompt)) {
  const target = /ride-cut:(\S+)/.exec(prompt)![1];
  say({ type: "item.started", item: { id: "m1", type: "mcp_tool_call", server: "backplane", tool: "thread_wait", status: "in_progress" } });
  let how = "";
  try {
    const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "thread_wait", arguments: { threadId: target, timeoutMs: 600000 } } }) });
    how = "answered " + (await r.text()).slice(0, 200);
  } catch (e) { how = "error " + e; }
  log({ ev: "cut", pid: process.pid, how, at: Date.now() });
  await sleep(600000);
} else if (prompt.includes("ride-quiet")) {
  await sleep(Number(process.env.RIDE_QUIET_MS ?? 20000));
}
say({ type: "item.completed", item: { id: "done", type: "agent_message", text: "DONE" } });
say({ type: "turn.completed", usage: {} });
