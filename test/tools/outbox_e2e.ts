// Shared-client reconnect ordering, followed through a real native hub.
// All histories, providers, homes and command bytes are generated here.
//
//   bend test/fixtures/outbox/index.html -o build/outbox-client
//   bun test/tools/outbox_e2e.ts [BINARY] [CLIENTDIR] [BRIDGE]
//   bun test/tools/outbox_e2e.ts --client-only [CLIENTDIR] [BRIDGE]
//
// BRIDGE optionally exercises the actual phone save/load/reconnect path.
import { appendFileSync, chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

// A deterministic Claude stream-json stand-in. It performs no work and
// finishes every command, including a follow-up sent while it is running.
if (process.argv[2] === "--agent") {
  if (process.argv.includes("--version")) { console.log("2.1.286"); process.exit(0); }
  const session = `outbox-${process.pid}`;
  const emit = (o: object) => console.log(JSON.stringify(o));
  const reader = createInterface({ input: process.stdin });
  for await (const line of reader) {
    let o: any;
    try { o = JSON.parse(line); } catch { continue; }
    if (o.type === "control_request") {
      if (o.request?.subtype === "initialize") emit({ type: "control_response", request_id: o.request_id, response: { subtype: "success", response: { models: [] } } });
      continue;
    }
    if (o.type !== "user") continue;
    appendFileSync(process.env.OUTBOX_AGENT_LOG!, JSON.stringify({ pid: process.pid, uuid: o.uuid }) + "\n");
    emit({ type: "system", subtype: "init", session_id: session, claude_code_version: "2.1.286", capabilities: ["msg_lifecycle_v1"] });
    emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "queued", session_id: session, uuid: crypto.randomUUID() });
    emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "started", session_id: session, uuid: crypto.randomUUID() });
    emit({ type: "result", subtype: "success", is_error: false, result: "done", session_id: session, uuid: crypto.randomUUID() });
    emit({ type: "command_lifecycle", command_uuid: o.uuid, state: "completed", session_id: session, uuid: crypto.randomUUID() });
  }
  process.exit(0);
}

const args = process.argv.slice(2);
const clientOnly = args[0] === "--client-only";
if (clientOnly) args.shift();
const binary = resolve(clientOnly ? "build/backplane" : (args.shift() ?? "build/backplane"));
const client = resolve(args.shift() ?? "build/outbox-client");
const bridge = args.shift();
// Keep compiler-generated declarations local; only the fixture exports
// belong on globalThis (a generated name can coincide with a JS global).
for (const file of readdirSync(client).filter(f => f.endsWith(".js")).sort()) new Function(readFileSync(join(client, file), "utf8"))();
const W = (globalThis as any).Wire;
const F = (globalThis as any).OutboxFixture;
if (!W || !F) throw new Error("build test/fixtures/outbox/index.html first");

let failed = 0;
let checks = 0;
function check(label: string, ok: boolean, detail?: unknown) {
  checks++;
  console.log(`${ok ? "ok" : "FAIL"} ${label}${ok ? "" : `: ${JSON.stringify(detail)?.slice(0, 500)}`}`);
  if (!ok) failed++;
}
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const turnTexts = (xs: string[]) => xs.filter(text => JSON.parse(text).m === "turn.start");
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(ms: number, probe: () => T | Promise<T>): Promise<T | undefined> {
  const end = Date.now() + ms;
  do {
    const value = await probe();
    if (value) return value;
    await sleep(25);
  } while (Date.now() < end);
  return undefined;
}

const scenarios = [9, 99].map(start => {
  const thread = `t${start}`;
  const data = F.scenario(start, thread);
  const original = turnTexts(data.original);
  const resend = turnTexts(data.resend);
  check(`shared ${start}: original request IDs cross a digit boundary`, same(original.map((x: string) => JSON.parse(x).id), [start, start + 1]), original);
  check(`shared ${start}: reconnect preserves exact request bytes and order`, same(resend, original), resend);
  check(`shared ${start}: repeated reconnect preserves exact request bytes and order`, same(turnTexts(data.again), original), data.again);
  check(`shared ${start}: pending messages retain send order`, same(data.pending, [`first ${thread}`, `second ${thread}`]), data.pending);
  check(`shared ${start}: acknowledging first retains only second`, same(turnTexts(data.afterFirstAck), original.slice(1)), data.afterFirstAck);
  check(`shared ${start}: acknowledging second retains only first`, same(turnTexts(data.afterSecondAck), original.slice(0, 1)), data.afterSecondAck);
  check(`shared ${start}: out-of-order acknowledgements empty the outbox`, data.afterBoth.length === 0, data.afterBoth);
  return { start, thread, original, resend };
});

// The real phone bridge keeps the client's Map, writes it flat, restores
// it after a fresh start, and routes replayed commands to their hub.
if (bridge) {
  (0, eval)(readFileSync(resolve(bridge), "utf8"));
  const B = (globalThis as any).Backplane;
  const key = "127.0.0.1:3799";
  const frame = {
    t: "log", since: 0, origin: "outbox-phone-origin", info: {}, items: [
      { $: "ProjectCreated", id: "p-phone", title: "Example", root: "/tmp/outbox-phone-example", at: 1 },
      { $: "ThreadCreated", id: "t-phone", project: "p-phone", title: "Offline", env: "local", provider: "claude", at: 2 },
    ],
  };
  const encoded = Buffer.from(W.encode(JSON.stringify(frame))).toString("base64");
  const sends = (value: string) => (JSON.parse(value).cmds ?? []).filter((c: any) => c.type === "send").map((c: any) => ({ hub: c.hub, text: W.decode(new Uint8Array(Buffer.from(c.data, "base64"))) })).filter((c: any) => JSON.parse(c.text).m === "turn.start");
  B.start("outbox-phone", "{}");
  B.hubs([key]);
  B.recv(key, encoded, false, false);
  B.act("select", `${key}|t-phone`);
  const original: { hub: string, text: string }[] = [];
  for (let i = 0; i < 105; i++) {
    B.quiet("draft", `offline ${i}`);
    original.push(...sends(B.act("send", "")));
  }
  check("phone: offline sends cross both digit boundaries", original.length === 105 && [9, 10, 99, 100].every(id => original.some(c => JSON.parse(c.text).id === id)), original.map(c => JSON.parse(c.text).id));
  const kept = B.save();
  B.start("outbox-phone", "{}");
  check("phone: kept state loads", !!B.load(kept));
  const replay = sends(B.recv(key, encoded, false, false));
  check("phone: save/load reconnect preserves every request byte and hub in send order", same(replay, original), replay.map(c => JSON.parse(c.text).id));
  check("phone: another reconnect keeps the same send order", same(sends(B.recv(key, encoded, false, false)), original));
}

if (!clientOnly) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-outbox-e2e-")));
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  async function stop(proc: ReturnType<typeof Bun.spawn>) {
    if (proc.exitCode !== null) return;
    proc.kill("SIGTERM");
    if (!(await until(4000, () => proc.exitCode !== null))) proc.kill("SIGKILL");
    if (!(await until(4000, () => proc.exitCode !== null))) throw new Error(`captured hub ${proc.pid} did not exit`);
    await proc.exited;
  }
  function freePort() {
    for (let i = 0; i < 20; i++) {
      const server = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
      const port = server.port;
      server.stop(true);
      if (port >= 3799) return port;
    }
    throw new Error("could not reserve a test port above 3798");
  }
  try {
    for (const entry of ["backplane", "backplane-serve"]) {
      const dir = join(root, entry);
      const home = join(dir, "home"), user = join(dir, "user"), fake = join(dir, "fake"), prefix = join(dir, "prefix"), project = join(dir, "project");
      for (const d of [home, user, fake, prefix, project, join(prefix, "web"), join(user, ".config"), join(user, ".cache"), join(user, ".local", "share")]) mkdirSync(d, { recursive: true });
      for (const name of ["backplane", "backplane-serve"]) { copyFileSync(binary, join(prefix, name)); chmodSync(join(prefix, name), 0o755); }
      writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve(import.meta.path))} --agent "$@"\n`);
      chmodSync(join(fake, "claude"), 0o755);
      for (const name of ["codex", "grok", "muse", "agy", "pi", "gh", "tailscale", "curl", "wget", "ssh", "kicad-cli", "freecad", "busctl"]) {
        writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, name), 0o755);
      }
      const event = ($: string, fields: object) => ({ $, ...fields, at: 1 });
      const seeded = [event("ProjectCreated", { id: "p-outbox", title: "Example", root: project }),
        ...scenarios.map(s => event("ThreadCreated", { id: s.thread, project: "p-outbox", title: s.thread, env: "local", branch: "", worktree: "", provider: "claude", model: "" })),
        event("SettingSet", { key: "restart.continue", value: "off" })];
      const logPath = join(home, "events.jsonl");
      writeFileSync(logPath, seeded.map(e => JSON.stringify(e)).join("\n") + "\n");
      const agentLog = join(dir, "agents.jsonl");
      writeFileSync(agentLog, "");
      const env = {
        HOME: user, BACKPLANE_HOME: home, XDG_CONFIG_HOME: join(user, ".config"), XDG_CACHE_HOME: join(user, ".cache"), XDG_DATA_HOME: join(user, ".local", "share"),
        PATH: `${fake}:${dirname(process.execPath)}:/usr/bin:/bin`, TMPDIR: dir, LANG: "C.UTF-8", DISPLAY: "", WAYLAND_DISPLAY: "",
        BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
        BACKPLANE_LOCK_DIR: join(dir, "locks"), BACKPLANE_DEPLOY_LOCKS: join(dir, "deploy-locks"), OUTBOX_AGENT_LOG: agentLog,
        HTTP_PROXY: "http://127.0.0.1:1", HTTPS_PROXY: "http://127.0.0.1:1", ALL_PROXY: "http://127.0.0.1:1", NO_PROXY: "127.0.0.1,localhost",
      };
      const port = freePort();
      const proc = Bun.spawn([join(prefix, entry), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], { env, cwd: dir, stdout: "ignore", stderr: "pipe" });
      const stderr = new Response(proc.stderr).text();
      const sockets: WebSocket[] = [];
      const history = () => readFileSync(logPath, "utf8").split("\n").slice(0, -1).filter(Boolean).map(line => JSON.parse(line));
      const posted = (thread: string) => history().filter(c => c.$ === "MessagePosted" && c.thread === thread && c.role === "user");
      try {
        const up = await until(30000, async () => {
          try { return (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(500) })).ok; } catch { return false; }
        });
        check(`${entry}: isolated hub starts`, !!up, proc.exitCode);
        if (!up) throw new Error("hub did not become ready");
        const token = readFileSync(join(home, "token"), "utf8").trim();
        async function connect() {
          const frames: any[] = [], replies = new Map<number, any>();
          let decodeError: string | undefined;
          const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${encodeURIComponent(token)}&enc=cbor`);
          sockets.push(ws);
          ws.binaryType = "arraybuffer";
          ws.onmessage = e => {
            try {
              const frame = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
              frames.push(frame);
              if (frame.t === "reply") replies.set(Number(frame.id), frame);
            } catch (err) { decodeError = String(err); }
          };
          const joined = await until(10000, () => frames.some(f => f.t === "log") || decodeError);
          if (!joined || decodeError) throw new Error(`socket join failed: ${decodeError ?? ws.readyState}`);
          return { ws, replies };
        }
        for (const s of scenarios) {
          const link = await connect();
          const send = async (c: Awaited<ReturnType<typeof connect>>) => {
            for (const text of s.resend) { c.replies.delete(JSON.parse(text).id); c.ws.send(W.encode(text)); }
            return until(10000, () => s.resend.every(text => c.replies.has(JSON.parse(text).id)));
          };
          check(`${entry} ${s.start}: every replayed request is accepted`, !!(await send(link)) && s.resend.every(text => link.replies.get(JSON.parse(text).id)?.ok === true), [...link.replies.values()]);
          await until(10000, () => posted(s.thread).length >= 2);
          check(`${entry} ${s.start}: stored user messages retain send order`, same(posted(s.thread).map(c => c.text), [`first ${s.thread}`, `second ${s.thread}`]), posted(s.thread));
          check(`${entry} ${s.start}: stored user IDs equal original retry IDs`, same(posted(s.thread).map(c => c.msg), s.original.map(text => JSON.parse(text).p.msg)), posted(s.thread));
          const completed = await until(10000, () => history().some(c => c.$ === "TurnChanged" && c.thread === s.thread && c.state === "completed"));
          check(`${entry} ${s.start}: fake provider completes a turn`, !!completed, history().filter(c => c.thread === s.thread).map(c => c.$));
          link.ws.close();
          const retry = await connect();
          check(`${entry} ${s.start}: a fresh connection accepts exact retries`, !!(await send(retry)) && s.resend.every(text => retry.replies.get(JSON.parse(text).id)?.ok === true), [...retry.replies.values()]);
          check(`${entry} ${s.start}: retries append no duplicate user messages`, posted(s.thread).length === 2, posted(s.thread));
          check(`${entry} ${s.start}: the other thread receives no messages`, posted(scenarios.find(other => other.thread !== s.thread)!.thread).every(c => !String(c.text).includes(s.thread)), history().filter(c => c.$ === "MessagePosted" && c.role === "user"));
          retry.ws.close();
        }
        check(`${entry}: only stand-in providers handled the messages`, readFileSync(agentLog, "utf8").trim().length > 0);
      } catch (err) {
        check(`${entry}: integration completes`, false, String(err));
      } finally {
        for (const socket of sockets) socket.close();
        await stop(proc);
        const output = await stderr;
        if (failed && output) console.error(output.slice(-3000));
      }
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}
console.log(`${checks} assertions, ${failed} failures`);
process.exit(failed ? 1 : 0);
