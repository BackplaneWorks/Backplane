// Isolated hub: a fake agent invokes MCP, two clients answer through the
// same WebSocket requests as the UI. No real account or user state is used.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();

const [binary = "build/settings-proposal/backplane-serve", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), "bp-settings-e2e-"));
const fake = join(root, "fake"), home = join(root, "home"), project = join(root, "project"), argv = join(root, "argv");
for (const p of [fake, home, project]) mkdirSync(p);
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
writeFileSync(join(fake, "claude"), `#!/bin/sh
case " $* " in *" --mcp-config "*) printf '%s\\n' "$@" >> ${quote(argv)}; cat >/dev/null;; *) printf '%s\\n' '2.1.286';; esac
`);
chmodSync(join(fake, "claude"), 0o755);
for (const p of ["codex", "grok", "muse", "agy", "gh", "tailscale"]) { writeFileSync(join(fake, p), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, p), 0o755); }
writeFileSync(join(fake, "busctl"), "#!/bin/sh\nprintf 'b false\\n'\n"); chmodSync(join(fake, "busctl"), 0o755);
writeFileSync(join(home, "models.cache"), JSON.stringify({ claude: ["sonnet"], codex: [], grok: [], muse: [], antigravity: [] }) + "\nfixture\n");
const event = ($: string, fields: object) => ({ $, ...fields, at: 1 });
const log = join(home, "events.jsonl");
writeFileSync(log, [event("ProjectCreated", { id: "p", title: "Fixture", root: project }),
  event("ThreadCreated", { id: "worker", project: "p", title: "Worker", env: "local", branch: "", worktree: project, provider: "claude", model: "sonnet" }),
  event("ThreadCreated", { id: "held", project: "p", title: "Held", env: "local", branch: "", worktree: project, provider: "claude", model: "sonnet" }),
  event("TurnChanged", { thread: "held", state: "interrupted" }), event("SettingSet", { key: "restart.continue", value: "off" }),
  event("SettingSet", { key: "fallback.on", value: "off" }), event("SettingSet", { key: "fallback.chain", value: "claude/sonnet" })].map(JSON.stringify).join("\n") + "\n");
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true);
const proc = Bun.spawn([resolve(binary), "--headless", "--foreground", "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: root, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_PEERS: "", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" }, stdout: "ignore", stderr: "ignore" });
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
const until = async <T>(label: string, f: () => T | undefined | Promise<T | undefined>, ms = 15000): Promise<T> => {
  const end = Date.now() + ms;
  while (Date.now() < end) { const v = await f(); if (v !== undefined && v !== false) return v; await sleep(50); }
  throw new Error(`timeout: ${label}`);
};
const check = (ok: unknown, label: string) => { if (!ok) throw new Error(label); console.log(`ok ${label}`); };
const events = () => readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(x => JSON.parse(x));
const values = (key: string) => events().filter(x => x.$ === "SettingSet" && x.key === key).map(x => x.value);
const sockets: WebSocket[] = [];
let seq = 0;
const send = (ws: WebSocket, m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++seq, m, p })));
let rpc = 0;
const call = async (url: string, name: string, args: object) => {
  const response = await fetch(url, { method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++rpc, method: "tools/call", params: { name, arguments: args } }) });
  return await response.json() as any;
};
try {
  await until("hub ready", async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).ok ? true : undefined; } catch { return undefined; } });
  for (let i = 0; i < 2; i++) {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}`); ws.binaryType = "arraybuffer";
    await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; }); sockets.push(ws);
  }
  send(sockets[0], "turn.start", { thread: "worker", text: "fixture", msg: "fixture-start" });
  const url = await until("fake agent MCP URL", () => existsSync(argv) ? [...readFileSync(argv, "utf8").matchAll(/http:\/\/127\.0\.0\.1:\d+\/mcp\/[^"'\s\\]+/g)].at(-1)?.[0] : undefined);
  const inspected = await call(url, "settings_inspect", {});
  check(!inspected.result?.isError && JSON.parse(inspected.result.content[0].text).on === "off", "MCP inspection returns current effective fallback");
  const secret = "SECRET-FIXTURE-NOT-A-MODEL";
  const invalid = await call(url, "settings_propose", { on: "on", chain: secret });
  check(invalid.result?.isError && !JSON.stringify(invalid).includes(secret) && !readFileSync(log, "utf8").includes(secret), "invalid secret-like input is neither echoed nor stored");
  const propose = async (on = "on") => {
    const result = await call(url, "settings_propose", { on, chain: "claude/sonnet" });
    check(!result.result?.isError, "MCP proposal accepted");
    const body = JSON.parse(result.result.content[0].text); return body.id as string;
  };
  const first = await propose();
  const card = await until("proposal persisted", () => events().find(x => x.$ === "AskOpened" && x.id === first));
  check(card.detail.includes("Hub-wide") && card.detail.includes("off → on") && card.detail.includes("active threads and bots"), "approval card records exact change, scope and consequences");
  check(values("fallback.on").at(-1) === "off", "proposal does not change settings");
  send(sockets[1], "ask.answer", { id: first, answer: "decline" });
  await until("rejection persisted", () => events().find(x => x.$ === "AskClosed" && x.id === first));
  check(values("fallback.on").at(-1) === "off", "second client rejects without changing settings");
  const second = await propose();
  send(sockets[1], "setting.set", { key: "fallback.chain", value: "" });
  await until("proposal invalidated", () => events().find(x => x.$ === "AskClosed" && x.id === second && x.answer === "stale"));
  send(sockets[1], "setting.set", { key: "fallback.chain", value: "claude/sonnet" });
  await until("setting restored", () => values("fallback.chain").length >= 3);
  send(sockets[0], "ask.answer", { id: second, answer: "accept" });
  await sleep(200);
  check(values("fallback.on").at(-1) === "off", "change and restore cannot revive approval");
  const third = await propose();
  send(sockets[1], "ask.answer", { id: third, answer: "accept" });
  await until("approval applied", () => values("fallback.on").at(-1) === "on");
  const count = values("fallback.on").length;
  send(sockets[0], "ask.answer", { id: third, answer: "accept" });
  await sleep(200);
  check(values("fallback.on").length === count, "approval cannot be replayed");
  const fourth = await propose("off"), fifth = await propose("off");
  send(sockets[0], "ask.answer", { id: fourth, answer: "accept" });
  await until("concurrent proposal invalidated", () => events().find(x => x.$ === "AskClosed" && x.id === fifth && x.answer === "stale"));
  send(sockets[1], "ask.answer", { id: fifth, answer: "accept" });
  await sleep(200);
  check(values("fallback.on").at(-1) === "off", "competing proposal becomes stale and cannot replay");
  check(!events().some(x => x.$ === "TurnChanged" && x.thread === "held" && x.state === "running"), "held thread remains stopped");
} finally {
  for (const ws of sockets) ws.close();
  proc.kill("SIGTERM"); await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
