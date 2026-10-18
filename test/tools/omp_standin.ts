// A stand-in for oh-my-pi's ACP server (`omp acp --approval-mode <mode>`), for
// test/tools/omp_e2e.ts. No model runs; the replies are oh-my-pi's shapes:
//   initialize         the recorded capabilities (test/fixtures/omp/turn.jsonl, line 1)
//   session/new        the recorded sessionId and configOptions (line 2)
//   session/load      the same reply when it resumes a session (test/fixtures/omp/resume.jsonl, line 8)
//   set_config_option  acks with the updated configOptions
//   session/prompt     "ECHO: <text>" as a chunk. The words after Backplane's
//                      instructions that say "write" make a write tool call
//                      (note.txt, "hello"). In always-ask, "ACP permission"
//                      sends a session/request_permission with only
//                      allow_always/reject_always; its Allow must be cancelled.
//                      Otherwise, with form elicitation, an elicitation/create
//                      asks first: Approve writes, anything else fails the call.
//                      Without the form, the call is refused. In yolo it goes
//                      straight through. Then end_turn.
// The form mirrors test/fixtures/omp/form.jsonl, line 64. Every request goes to
// $OMP_LOG.
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// the hub's own requests and replies, on our stdin
interface Rpc {
  id?: number;
  method?: string;
  params?: {
    sessionId?: string;
    cwd?: string;
    configId?: string;
    value?: string;
    prompt?: { text?: string }[];
    clientCapabilities?: { elicitation?: { form?: boolean } };
  };
  result?: { action?: string; content?: { value?: string }; outcome?: { outcome?: string; optionId?: string } };
}

const log = process.env.OMP_LOG ?? "/dev/null";
const mode = process.argv[process.argv.indexOf("--approval-mode") + 1] ?? "always-ask";
const say = (o: object) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...o }) + "\n");
const note = (s: string) => appendFileSync(log, s + "\n");
const fixture = readFileSync(join(import.meta.dir, "../fixtures/omp/turn.jsonl"), "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l) as { id?: number; result?: unknown });
const init = fixture.find((o) => o.id === 1)?.result ?? {};
const opened = fixture.find((o) => o.id === 2)?.result as { sessionId: string; configOptions: { id: string; currentValue: string }[] };
const sid = opened.sessionId as string;
const options = new Map<string, { cwd: string; configOptions: { id: string; currentValue: string }[] }>();
let form = false;
let nextId = 0;
const waiting = new Map<number, (reply: Rpc["result"]) => void>();

note(`START args=${process.argv.slice(2).join(" ")}`);

const update = (session: string, u: object) => say({ method: "session/update", params: { sessionId: session, update: u } });
const finish = (session: string, id: string, ok: boolean, text: string) =>
  update(session, {
    sessionUpdate: "tool_call_update",
    toolCallId: id,
    status: ok ? "completed" : "failed",
    rawOutput: { content: [{ type: "text", text }], details: {} },
    content: [{ type: "content", content: { type: "text", text } }],
  });

function doWrite(session: string, id: string) {
  writeFileSync(join(options.get(session)?.cwd ?? "", "note.txt"), "hello");
  note("WROTE note.txt");
  finish(session, id, true, "Wrote note.txt");
}
async function requestPermission(session: string, id: string) {
  const rid = nextId++;
  const { promise, resolve } = Promise.withResolvers<Rpc["result"]>();
  waiting.set(rid, resolve);
  say({
    id: rid,
    method: "session/request_permission",
    params: {
      sessionId: session,
      toolCall: { toolCallId: id, title: "Write note.txt", kind: "execute", rawInput: { command: "write note.txt" } },
      options: [{ optionId: "always", kind: "allow_always" }, { optionId: "never", kind: "reject_always" }],
    },
  });
  const reply = await promise;
  note("PERMISSION " + JSON.stringify(reply));
  if (reply?.outcome?.outcome === "selected" && reply.outcome.optionId === "always") doWrite(session, id);
  else finish(session, id, false, "Tool call denied by user: write");
}

async function prompt(id: number, session: string, text: string) {
  note(`PROMPT ${JSON.stringify(text.slice(0, 80))} mode=${mode}`);
  // a fresh session's prompt opens with Backplane's instructions, which say
  // "write": only the words after them count
  const close = "</backplane_instructions>";
  const at = text.lastIndexOf(close);
  const asked = (at < 0 ? text : text.slice(at + close.length)).trim();
  update(session, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "ECHO: " + asked.slice(-40) } });
  if (asked.includes("write")) {
    const tool = "W" + id;
    update(session, { sessionUpdate: "tool_call", toolCallId: tool, title: "Writing note.txt", kind: "edit", status: "pending", rawInput: { path: "note.txt", content: "hello" } });
    if (mode === "yolo") {
      doWrite(session, tool);
    } else if (asked.includes("ACP permission")) {
      await requestPermission(session, tool);
    } else if (!form) {
      note("REFUSED no form capability");
      finish(session, tool, false, "Tool call denied by user: write");
    } else {
      const rid = nextId++;
      const { promise, resolve } = Promise.withResolvers<Rpc["result"]>();
      waiting.set(rid, resolve);
      say({
        id: rid,
        method: "elicitation/create",
        params: {
          mode: "form",
          sessionId: session,
          message: "Allow tool: write\nPath: note.txt\nContent:\nhello",
          requestedSchema: { type: "object", properties: { value: { type: "string", enum: ["Approve", "Deny"] } }, required: ["value"] },
        },
      });
      const reply = await promise;
      note("ANSWER " + JSON.stringify(reply));
      if (reply?.action === "accept" && reply.content?.value === "Approve") doWrite(session, tool);
      else finish(session, tool, false, "Tool call denied by user: write");
    }
  }
  say({ id, result: { stopReason: "end_turn" } });
}

function handle(o: Rpc) {
  if (!o.method) {
    // the answer to an elicitation we sent
    const w = o.id === undefined ? undefined : waiting.get(o.id);
    if (w) { waiting.delete(o.id ?? -1); w(o.result); }
    return;
  }
  note(`REQ ${o.method}`);
  switch (o.method) {
    case "initialize":
      form = !!o.params?.clientCapabilities?.elicitation?.form;
      return say({ id: o.id, result: init });
    case "session/load":
    case "session/new":
      options.set(sid, { cwd: o.params?.cwd ?? "", configOptions: structuredClone(opened.configOptions) });
      return say({ id: o.id, result: opened });
    case "session/set_config_option": {
      const s = options.get(o.params?.sessionId ?? "");
      const opt = s?.configOptions.find((x) => x.id === o.params?.configId);
      if (!s || !opt) return say({ id: o.id, error: { code: -32602, message: "unknown config option" } });
      opt.currentValue = o.params?.value ?? "";
      note(`CONFIG ${o.params?.configId}=${o.params?.value}`);
      return say({ id: o.id, result: { configOptions: s.configOptions } });
    }
    case "session/prompt":
      return void prompt(o.id ?? 0, o.params?.sessionId ?? sid, (o.params?.prompt ?? []).map((b) => b.text ?? "").join(""));
    default:
      if (o.id !== undefined) say({ id: o.id, error: { code: -32601, message: "not supported" } });
  }
}

let buf = "";
process.stdin.on("data", (d) => {
  buf += d.toString();
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (line.trim()) { try { handle(JSON.parse(line) as Rpc); } catch (e) { note("BAD " + line); } }
  }
});
process.stdin.on("end", () => note("EOF"));
