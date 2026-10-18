import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { standinScript } from "./perf_standin";

const root = mkdtempSync(join(tmpdir(), "bp-perf-standin-"));
try {
  const child = Bun.spawn(["/bin/sh", "-c", standinScript], {
    env: { ...process.env, FAKE_LOG: join(root, "agent.log"), FAKE_SECS: "1", FAKE_PER: "3", FAKE_CYCLE: "5" },
    stdin: "pipe", stdout: "pipe", stderr: "pipe",
  });
  child.stdin.write(JSON.stringify({ type: "user", message: { role: "user", content: "zqsub stream" } }) + "\n");
  child.stdin.end();
  const [out, err, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (exit !== 0) throw new Error(`stand-in failed: ${exit}: ${err}`);
  const events = out.trim().split("\n").map((line) => JSON.parse(line));
  const deltas = events.filter((o) => o.type === "stream_event" && o.event?.type === "content_block_delta");
  if (deltas.length !== 30) throw new Error(`expected 30 deltas, got ${deltas.length}`);
  if (events.filter((o) => o.subtype === "task_started").length !== 2) throw new Error("subagent and shell were not started");
  if (events.filter((o) => o.type === "assistant").length < 4) throw new Error("no finished replies/tool calls");
  console.log(`PASS: deterministic stand-in emits ${events.length} valid JSON lines, 30 deltas, subagent/shell and completed replies`);
} finally {
  rmSync(root, { recursive: true, force: true });
}
