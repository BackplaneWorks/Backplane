// Terminals in the chat end to end (core/emb.bend): a headless hub, a
// stand-in `claude` that writes down what it is sent. An agent opens a
// terminal (terminal_open), reads it, the person types into it
// (term.input) and asks for its screen (term.snap), it restarts
// (term.restart), and at the person's next message the agent is told what
// it shows and the terminal stops. Prints "ok ..." / "FAIL ...".
//
//   bun test/tools/emb_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-emb-"));
const fake = join(root, "bin");
const proj = join(root, "proj");
const said = join(root, "said.jsonl");
mkdirSync(fake);
mkdirSync(proj);
writeFileSync(join(fake, "claude"), `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      printf '%s\\n' "$line" >> ${said}
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":[],"model":"fake","permissionMode":"default"}'
      printf '%s\\n' '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"ok"}]},"session_id":"s1"}'
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"s1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || !more ? "" : ": " + JSON.stringify(more).slice(0, 500)}`);
  if (!ok) fail++;
};
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};

const home = join(root, "home");
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", SHELL: "/bin/sh", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 300; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const seen: any[] = [];
  const msgs: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (!o) return;
    msgs.push(o);
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let n = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++n, m, p }))); return n; };
  const mcp = async (thread: string, name: string, args: object) => {
    const r = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
    });
    const t = await r.text();
    const line = t.split("\n").find((l) => l.startsWith("{") || l.startsWith("data: {")) ?? t;
    const o = JSON.parse(line.replace(/^data: /, ""));
    return { err: !!o.result?.isError, text: o.result?.content?.[0]?.text ?? JSON.stringify(o) };
  };
  const bytes = (s: string) => [...new TextEncoder().encode(s)];
  const text = (b: number[]) => new TextDecoder().decode(new Uint8Array(b));

  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: pc.id, title: "Tools" });
  const th = (await until(10000, () => seen.find((c) => c.$ === "ThreadCreated")))!.id;

  // the agent opens one
  const open = await mcp(th, "terminal_open", { command: "printf 'hello from %s\\n' \"$(basename \"$PWD\")\"; read x; echo got:$x; sleep 60", title: "Echo", cols: 40, rows: 6 });
  const id = open.err ? "" : JSON.parse(open.text).id;
  check(!!id, "terminal_open answers an id", open);
  const key = `x:${th}:${id}`;
  const act = await until(5000, () => seen.find((c) => c.$ === "ActivityLogged" && c.act === id));
  check(act?.tone === "term" && act?.summary.startsWith("Echo\n40x6\n"), "it is an entry of the thread", act);
  const live = await until(5000, () => msgs.find((o) => o.t === "term" && o.thread === key && text(o.b).includes("hello")));
  check(!!live, "its output reaches the clients under its key");

  let r = await mcp(th, "terminal_read", { id });
  check(!r.err && r.text.includes('state="running"') && r.text.includes("hello from proj"), "terminal_read: running, in the thread's folder", r);
  check((await mcp(th, "terminal_read", { id: "nope" })).err, "terminal_read of another id fails");

  // a client asks for its screen
  const m0 = msgs.length;
  send("term.snap", { key });
  const snap = await until(5000, () => msgs.slice(m0).find((o) => o.t === "term" && o.thread === key && o.reset));
  check(JSON.stringify(snap?.reset) === "[40,6]" && text(snap?.b ?? []).includes("hello from proj"), "term.snap: a reset of its size and its screen", snap);

  // the person types
  send("term.input", { thread: key, b: bytes("abc\r") });
  await sleep(800);
  r = await mcp(th, "terminal_read", { id });
  check(r.text.includes("got:abc"), "typed keys reach the program", r);

  // the agent types too
  await mcp(th, "terminal_send", { id, text: "ignored\r" });

  // a restart starts it over and every client starts its screen over
  const m1 = msgs.length;
  send("term.restart", { thread: th, id });
  const reset = await until(5000, () => msgs.slice(m1).find((o) => o.t === "term" && o.thread === key && o.reset));
  check(!!reset, "term.restart starts every client's screen over");
  const setAt = await until(5000, () => seen.find((c) => c.$ === "SettingSet" && c.key === `emb.at:${key}`));
  check(!!setAt, "term.restart marks it live from now");
  await sleep(800);
  r = await mcp(th, "terminal_read", { id });
  check(r.text.includes('state="running"') && r.text.includes("hello from proj") && !r.text.includes("got:abc"), "the restarted program runs afresh", r);
  send("term.input", { thread: key, b: bytes("xyz\r") });
  await sleep(800);

  // a script read to its end
  const sc = await mcp(th, "terminal_open", { command: "echo one; sleep 1; echo two", title: "Script" });
  const sid = JSON.parse(sc.text).id;
  r = await mcp(th, "terminal_read", { id: sid, waitMs: 5000 });
  check(r.text.includes('state="done 0"') && r.text.includes("two"), "terminal_read waitMs waits for the command to end", r);
  await sleep(1500);
  await mcp(th, "terminal_send", { id: sid, text: "echo still-here\r" });
  await sleep(1000);
  r = await mcp(th, "terminal_read", { id: sid });
  check(r.text.includes('state="done 0"') && r.text.includes("still-here"), "after its command the terminal is a shell", r);

  // pinned ones keep running past the person's message
  const pin = await mcp(th, "terminal_open", { command: "sleep 60", title: "Pinned" });
  const pid = JSON.parse(pin.text).id;
  send("setting.set", { key: `emb.pin:x:${th}:${pid}`, value: "1" });
  await until(5000, () => seen.find((c) => c.$ === "SettingSet" && c.key === `emb.pin:x:${th}:${pid}`));

  // the person writes: the agent hears what the terminals show
  send("turn.start", { thread: th, text: "what did I type?" });
  await until(15000, () => existsSync(said) && readFileSync(said, "utf8").includes("what did I type"));
  const line = existsSync(said) ? readFileSync(said, "utf8").trim().split("\n").pop()! : "";
  const sent = line ? JSON.parse(line).message.content[0].text : "";
  check(sent.startsWith("what did I type?") && sent.includes("<terminals>") && sent.includes("got:xyz") && sent.includes(`id="${id}"`), "the next message carries what the terminals show", sent);
  await sleep(1000);
  r = await mcp(th, "terminal_read", { id });
  check(r.text.includes('state="exited'), "a terminal not pinned stops at the person's message", r);
  r = await mcp(th, "terminal_read", { id: pid });
  check(r.text.includes('state="running"'), "a pinned one keeps running", r);

  // a second message tells nothing new
  send("turn.start", { thread: th, text: "again" });
  await until(15000, () => readFileSync(said, "utf8").includes('"again'));
  const again = JSON.parse(readFileSync(said, "utf8").trim().split("\n").pop()!).message.content[0].text;
  check(!again.includes(`id="${id}"`), "what the agent has seen is not told again", again);

  // its last screen outlives the program
  check(existsSync(join(home, "terms", key)), "the last screen is kept on disk");
  await mcp(th, "terminal_close", { id: pid });
  await sleep(800);
  r = await mcp(th, "terminal_read", { id: pid });
  check(r.text.includes('state="exited'), "terminal_close stops it", r);
} catch (e) {
  check(false, "run", String(e));
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(fail ? 1 : 0);
