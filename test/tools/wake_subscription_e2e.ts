// Wake subscriptions against a throwaway hub and stand-in Claude, no models.
// bun test/tools/wake_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
if (!(globalThis as any).Wire) for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
const root = mkdtempSync(join(tmpdir(), "bp-wake-"));
if (process.env.KEEP) console.log(`artifacts ${root}`);
const home = join(root, "home"), proj = join(root, "proj"), fake = join(root, "bin"), log = join(root, "claude.log");
for (const d of [home, proj, fake]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec '${process.execPath}' '${resolve("test/tools/claude_standin.ts")}' "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const socket = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = socket.port; socket.stop(true);
let fail = 0;
const check = (ok: boolean, text: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${text}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined, label = "condition"): Promise<T> => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) throw new Error(`timed out: ${label}`); await sleep(50); }
};
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_WAKE_TICK_MS: "1000", BP_WORK_MS: "200", FAKE_LOG: log, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` }, stdout: "ignore", stderr: Bun.file(join(root, "hub.log")),
});
let ws: WebSocket | undefined;
const seen: any[] = [];
try {
  for (let i = 0; i < 200; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {} await sleep(100); }
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data))); seen.push(...(o.items ?? [])); };
  await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
  let seq = 0;
  const send = (m: string, p: object) => ws!.send(W.encode(JSON.stringify({ id: ++seq, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find(x => x.$ === "ProjectCreated" && x.root === proj));
  const made = async (title: string): Promise<string> => { send("thread.create", { project: pc.id, title, env: "local", provider: "claude" }); return (await until(10000, () => seen.find(x => x.$ === "ThreadCreated" && x.title === title))).id; };
  const a = await made("Subscriber A"), b = await made("Target B");
  const turns = (th: string) => seen.filter(x => x.$ === "TurnChanged" && x.thread === th && x.state === "completed").length;
  const turn = async (th: string, text: string) => { const before = turns(th); send("turn.start", { thread: th, text, msg: `test-${++seq}` }); await until(15000, () => turns(th) > before ? true : undefined); };
  await turn(a, "mode:plain subscriber ready");
  const mcpUrl = await until(10000, () => readFileSync(log, "utf8").split("\n").find(l => l.startsWith("MCP ") && (l.includes(`/mcp/${a}?`) || l.includes(`/mcp/${a}.`)))?.split(" ").slice(2).join(" "));
  const call = async (name: string, args: object) => {
    const r = await fetch(mcpUrl, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: ++seq, method: "tools/call", params: { name, arguments: args } }) });
    const o: any = await r.json(); if (o.result?.isError || o.error) throw new Error(JSON.stringify(o));
    return JSON.parse(o.result.content[0].text);
  };
  const wakes = (id: string) => seen.filter(x => x.$ === "MessagePosted" && x.thread === a && String(x.text).startsWith(`Wake ${id}:`));
  const first = await call("wake_on", { kind: "thread", target: b });
  await turn(b, "mode:plain first target turn");
  await until(10000, () => wakes(first.id).length ? true : undefined);
  await sleep(1400);
  check(wakes(first.id).length === 1, `once:true posts exactly one wake (${wakes(first.id).length})`);
  check(seen.some(x => x.$ === "WakeDropped" && x.id === first.id), "once:true drop persists with delivery");
  check(String(wakes(first.id)[0]?.text).includes("Target B") && String(wakes(first.id)[0]?.text).includes("REPLY mode:plain first target turn"), "wake names target and last reply");
  const repeated = await call("wake_on", { kind: "thread", target: b, once: false, events: ["end", "done"], note: "review board" });
  await turn(b, "mode:plain second target turn");
  await until(10000, () => wakes(repeated.id).length === 1 ? true : undefined);
  await sleep(1400);
  await turn(b, "mode:plain third target turn");
  await until(10000, () => wakes(repeated.id).length === 2 ? true : undefined);
  check(wakes(repeated.id).length === 2, `once:false wakes for two turns, one message per turn (${wakes(repeated.id).length})`);
  check(String(wakes(repeated.id)[0]?.text).includes("end, done") && String(wakes(repeated.id)[0]?.text).includes("review board"), "one paragraph names both matching events and note");
  const listed = await call("wake_list", {});
  check(listed.wakes.length === 1 && listed.wakes[0].id === repeated.id, "wake_list shows only active subscriptions");
  await call("wake_cancel", { id: repeated.id });
  check((await call("wake_list", {})).wakes.length === 0, "wake_cancel removes subscription");
  const file = join(root, "signal.txt"); writeFileSync(file, "baseline");
  const watched = await call("wake_on", { kind: "file", target: file });
  await until(10000, () => seen.some(x => x.$ === "SettingSet" && x.key === `wake.base.${watched.id}`) ? true : undefined, "file baseline persisted");
  await sleep(1100); writeFileSync(file, "changed file, different size\n");
  await until(70000, () => wakes(watched.id).length ? true : undefined, "file changed wake");
  await sleep(2200);
  check(wakes(watched.id).length === 1, `file stat change wakes once via tick (${wakes(watched.id).length})`);
  check((await call("wake_list", {})).wakes.length === 0, "file once subscription dropped");
} finally {
  if (process.env.KEEP) {
    writeFileSync(join(root, "seen.json"), JSON.stringify(seen, null, 2));
    try { writeFileSync(join(root, "perf.txt"), await (await fetch(`http://127.0.0.1:${port}/debug/perf`, { signal: AbortSignal.timeout(2000) })).text()); } catch {}
  }
  ws?.close();
  process.kill(hub.pid, "SIGTERM"); await hub.exited;
  if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
