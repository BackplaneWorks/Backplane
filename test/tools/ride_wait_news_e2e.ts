// A thread_wait ends early when riding mail was given to the waiting thread
// (hub.bend's Ride.note, mcp.bend's Mcp.news): the agent then reads its
// input instead of sitting out the wait. Thread B runs a long turn (a
// stand-in that streams); thread A waits on it for 60 s; 3 s in, A is given
// riding mail (its "ridden" mark, as Ride.note sets it). The wait answers
// within a few seconds, as news. A wait with nothing given runs its time.
//
//   bun test/tools/ride_wait_news_e2e.ts BINARY WIREDIR
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binArg, wire] = process.argv.slice(2);
if (!binArg || !wire) throw new Error("usage: ride_wait_news_e2e.ts BINARY WIREDIR");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
let fail = 0;
const check = (ok: unknown, what: string, got?: unknown) => { console.log(`${ok ? "ok" : "FAIL"} ${what}${ok || got === undefined ? "" : ": " + JSON.stringify(got).slice(0, 300)}`); if (!ok) fail++; };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const until = async <T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> => { const end = Date.now() + ms; for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); } };
const root = mkdtempSync(join(tmpdir(), "bp-ride-wait-"));
const home = join(root, "home"), fake = join(root, "bin"), proj = join(root, "proj");
for (const d of [home, fake, proj]) mkdirSync(d);
// a turn that streams for two minutes, then ends
writeFileSync(join(fake, "claude"), `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":[],"model":"fake","permissionMode":"default"}'
      i=0; while [ $i -lt 1200 ]; do i=$((i + 1))
        printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"working "}},"session_id":"s1"}'
        sleep 0.1; done
      printf '%s\\n' '{"type":"result","is_error":false,"result":"done","session_id":"s1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const now = Math.floor(Date.now() / 1000);
const ev = ($: string, f: object) => JSON.stringify({ $, ...f, at: now });
writeFileSync(join(home, "events.jsonl"), [ev("ProjectCreated", { id: "p", title: "P", root: proj }),
  ev("ThreadCreated", { id: "ta", project: "p", title: "waiter", env: "local", branch: "", worktree: proj, provider: "claude", model: "" }),
  ev("ThreadCreated", { id: "tb", project: "p", title: "worker", env: "local", branch: "", worktree: proj, provider: "claude", model: "" }),
  ev("SettingSet", { key: "restart.continue", value: "off" })].join("\n") + "\n");
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const port = freePort();
const proc = Bun.spawn([resolve(binArg), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"],
  { env: { PATH: `${fake}:/usr/bin:/bin`, HOME: root, DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0" }, stdout: "ignore", stderr: "ignore" });
try {
  let up = false;
  for (let i = 0; i < 300 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  if (!up) throw new Error("hub did not start");
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  const replies = new Map<number, any>(); const seen: any[] = []; let n = 0;
  ws.onmessage = (e) => { const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))); for (const x of o.items ?? []) seen.push(x); if (o.t === "reply") replies.set(Number(o.id), o); };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  const rpc = async (m: string, p: object) => { const id = ++n; ws.send(W.encode(JSON.stringify({ id, m, p }))); return await until(15000, () => replies.get(id)); };
  const started = await rpc("turn.start", { thread: "tb", text: "work for a while", msg: "m-tb" });
  check(started?.ok && await until(15000, () => seen.some((x) => x.$ === "TurnChanged" && x.thread === "tb" && x.state === "running")), "the worker's turn runs");
  const wait = async (ms: number) => {
    const t0 = Date.now();
    const r = await (await fetch(`http://127.0.0.1:${port}/mcp/ta`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "thread_wait", arguments: { threadId: "tb", timeoutMs: ms } } }) })).json();
    const inner = JSON.parse(r?.result?.content?.[0]?.text ?? "{}");
    return { secs: (Date.now() - t0) / 1000, inner };
  };
  // nothing given: the wait runs its time and says nothing of news
  const quiet = await wait(5000);
  check(quiet.secs >= 4 && !quiet.inner.done && !quiet.inner.news, "a wait with nothing given runs its time, no news", quiet);
  // given riding mail 3 s into a 60 s wait: it answers within a few seconds, as news
  const w = wait(60000);
  await sleep(3000);
  const given = await rpc("setting.set", { key: "ridden.ta", value: String(Math.floor(Date.now() / 1000)) });
  check(given?.ok, "the waiting thread is given riding mail (its ridden mark)");
  const r = await w;
  console.log(`  the wait answered after ${r.secs.toFixed(1)} s`);
  check(r.secs < 10 && r.inner.news === true, "the wait ends early, as news", r);
  check(r.inner.done === false && typeof r.inner.note === "string", "it still says the thread waited on is not done, and why it answered", r.inner);
  ws.close();
} finally {
  proc.kill("SIGKILL"); await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
