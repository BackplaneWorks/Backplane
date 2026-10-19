// An event log that cannot be read must never become an empty history.
// Both native entry names, isolated homes and ports, generated histories,
// permission repair, and a sparse >4 GiB file (never read into this process).
//
//   bun test/tools/logread_e2e.ts [BINARY] [WIREDIR] [READER_FIXTURE]
// READER_FIXTURE is test/native/log_read.bend built as a native executable.
import { chmodSync, closeSync, copyFileSync, existsSync, ftruncateSync, mkdirSync, mkdtempSync, openSync, readFileSync, readSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync, writeSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binary = "build/backplane", wire = "build/wire", reader] = process.argv.slice(2);
for (const file of readdirSync(wire).filter(file => file.endsWith(".js"))) (0, eval)(readFileSync(join(wire, file), "utf8"));
const W = (globalThis as any).Wire;
const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-logread-e2e-")));
const prefix = join(root, "prefix"), fake = join(root, "bin");
const children = new Set<ReturnType<typeof Bun.spawn>>();
let failures = 0, checks = 0, serial = 0;
function check(name: string, ok: boolean, got?: unknown) {
  checks++;
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failures++;
}
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
async function within<T>(promise: Promise<T>, ms: number): Promise<T | undefined> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([promise, new Promise<undefined>(resolve => { timer = setTimeout(() => resolve(undefined), ms); })]);
  } finally { if (timer !== undefined) clearTimeout(timer); }
}
async function stop(proc: ReturnType<typeof Bun.spawn>): Promise<boolean> {
  if (proc.exitCode === null) proc.kill("SIGTERM");
  let code = await within(proc.exited, 3000);
  if (code === undefined) { proc.kill("SIGKILL"); code = await within(proc.exited, 2000); }
  if (code !== undefined) children.delete(proc);
  return code !== undefined;
}
function freePort(): number {
  for (let n = 0; n < 16; n++) {
    const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    const port = listener.port; listener.stop(true);
    if (port >= 3799) return port;
  }
  throw new Error("no spare test port above 3798");
}
async function listening(port: number): Promise<boolean> {
  try {
    const socket = await Bun.connect({ hostname: "127.0.0.1", port, socket: { data() {} } });
    socket.end(); return true;
  } catch { return false; }
}
function environment(home: string) {
  const user = join(home, "user"); mkdirSync(user, { recursive: true });
  // Deliberately do not inherit provider credentials or integration overrides.
  return {
    HOME: user, SHELL: "/bin/sh", PATH: `${fake}:/usr/bin:/bin:/usr/sbin:/sbin`, TMPDIR: root,
    DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_HOME: home, BACKPLANE_AGENTS: fake,
    XDG_CONFIG_HOME: join(home, "config"), XDG_DATA_HOME: join(home, "data"), XDG_CACHE_HOME: join(home, "cache"), XDG_STATE_HOME: join(home, "state"),
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_ADOPT: "0",
    BACKPLANE_LOCK_DIR: join(home, "locks"), BACKPLANE_DEPLOY_LOCKS: join(home, "deploy-locks"),
    BP_LOGREAD_PROBES: join(home, "provider-probes"), BP_LOGREAD_LAUNCHES: join(home, "provider-launches"),
    HTTP_PROXY: "http://127.0.0.1:1", HTTPS_PROXY: "http://127.0.0.1:1", ALL_PROXY: "http://127.0.0.1:1", NO_PROXY: "127.0.0.1,localhost",
  };
}
type Hub = {
  proc: ReturnType<typeof Bun.spawn>; home: string; port: number;
  output: Promise<[string, string]>; opened: number;
};
function start(name: string, home: string): Hub {
  const port = freePort();
  const proc = Bun.spawn([join(prefix, name), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    cwd: root, env: environment(home), stdin: "ignore", stdout: "pipe", stderr: "pipe",
  });
  children.add(proc);
  return { proc, home, port, output: Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text()]), opened: Date.now() };
}
async function hello(h: Hub): Promise<any | undefined> {
  try {
    const response = await fetch(`http://127.0.0.1:${h.port}/hello`, { signal: AbortSignal.timeout(500) });
    if (response.ok) return JSON.parse(W.decode(new Uint8Array(await response.arrayBuffer())));
  } catch {}
}
async function ready(h: Hub): Promise<any | undefined> {
  const end = Date.now() + 20000;
  while (Date.now() < end && h.proc.exitCode === null) {
    const value = await hello(h);
    if (value?.backplane) return value;
    await sleep(50);
  }
}
async function mcp(h: Hub, tool: string, args: object): Promise<any> {
  const response = await fetch(`http://127.0.0.1:${h.port}/mcp/t1`, {
    method: "POST", headers: { "content-type": "application/json", accept: "application/json" }, signal: AbortSignal.timeout(10000),
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: tool, arguments: args } }),
  });
  const result: any = await response.json();
  if (!response.ok || result.error || result.result?.isError) throw new Error(`MCP ${tool}: ${JSON.stringify(result)}`);
  return JSON.parse(result.result.content[0].text);
}
function homeFor(name: string, label: string): string {
  const home = join(root, name, `${++serial}-${label}`); mkdirSync(home, { recursive: true }); return home;
}
const title = "Replayed café 工作 🛠️";
const userText = "Keep the original measurements: 12.5 mm — Δx.";
const answerText = "Saved the original history. 完了 ✓";
function history(home: string): Buffer {
  const project = join(home, "project"); mkdirSync(project, { recursive: true });
  // These are Change.encode's exact on-disk object shapes (codec.bend),
  // also used by deploy_main_e2e.ts. No user histories or external fixtures.
  const event = ($: string, fields: object, at: number) => ({ $, ...fields, at });
  return Buffer.from([
    event("ProjectCreated", { id: "p1", title: "Original project", root: project }, 1),
    event("ThreadCreated", { id: "t1", project: "p1", title, env: "local", branch: "", worktree: "", provider: "claude", model: "" }, 2),
    event("MessagePosted", { thread: "t1", msg: "original-user", role: "user", text: userText }, 3),
    event("MessagePosted", { thread: "t1", msg: "original-answer", role: "assistant", text: answerText }, 4),
    event("TurnChanged", { thread: "t1", state: "completed" }, 5),
    event("SettingSet", { key: "restart.continue", value: "off" }, 6),
  ].map(event => JSON.stringify(event)).join("\n") + "\n");
}
async function replayed(h: Hub, label: string) {
  const list = await mcp(h, "thread_list", {});
  check(`${label} replays the original thread metadata`, Array.isArray(list.threads) && list.threads.some((thread: any) => thread.id === "t1" && thread.title === title && thread.project === "p1"), list);
  const timeline = await mcp(h, "thread_read", { threadId: "t1", limit: 20 });
  const messages = (timeline.entries ?? []).filter((entry: any) => entry.text === userText || entry.text === answerText);
  check(`${label} replays both original non-ASCII messages once and in order`, messages.length === 2 && messages[0].text === userText && messages[1].text === answerText, timeline);
}
const launches = (home: string) => existsSync(join(home, "provider-launches")) ? readFileSync(join(home, "provider-launches"), "utf8") : "";
async function healthy(name: string, label: string, content: "missing" | "empty" | "valid" | "torn") {
  const home = homeFor(name, label), path = join(home, "events.jsonl");
  const original = history(home);
  const bytes = content === "torn" ? Buffer.concat([original, Buffer.from('{"$":"ProjectRem')]) : content === "valid" ? original : Buffer.alloc(0);
  if (content !== "missing") writeFileSync(path, bytes);
  const h = start(name, home);
  try {
    const info = await ready(h);
    check(`${name} ${label} starts and serves hub metadata`, !!info?.backplane, info ?? await within(h.output, 100));
    if (!info) return;
    if (content === "valid" || content === "torn") await replayed(h, `${name} ${label}`);
    check(`${name} ${label} preserves the existing log prefix`, readFileSync(path).subarray(0, bytes.length).equals(bytes));
    if (content === "torn") {
      // A new commit must start after the torn line; the malformed fragment
      // remains ignored, and the new change survives another real restart.
      const changed = await fetch(`http://127.0.0.1:${h.port}/mcp/t1`, {
        method: "POST", headers: { "content-type": "application/json", accept: "application/json" }, signal: AbortSignal.timeout(10000),
        body: JSON.stringify({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "thread_update", arguments: { threadId: "t1", title: "After torn tail" } } }),
      });
      const response: any = await changed.json();
      check(`${name} torn tail accepts the next whole event`, changed.ok && !response.error && !response.result?.isError, response);
      const tail = readFileSync(path).subarray(bytes.length).toString("utf8");
      check(`${name} torn tail separates the fragment before appending`, tail.startsWith("\n") && tail.split("\n").some(line => { try { const event = JSON.parse(line); return event.$ === "ThreadRenamed" && event.id === "t1" && event.title === "After torn tail"; } catch { return false; } }), tail);
    }
    check(`${name} ${label} invokes no provider turn`, launches(home) === "", launches(home));
  } finally { check(`${name} ${label} captured hub stops`, await stop(h.proc)); }
  if (content === "torn") {
    const again = start(name, home);
    try {
      check(`${name} repaired torn tail starts again`, !!await ready(again));
      const list = await mcp(again, "thread_list", {});
      check(`${name} next event after a torn tail replays on restart`, list.threads?.some((thread: any) => thread.id === "t1" && thread.title === "After torn tail") === true, list);
      const timeline = await mcp(again, "thread_read", { threadId: "t1", limit: 20 });
      check(`${name} torn-tail restart retains both original messages`, timeline.entries?.filter((entry: any) => entry.text === userText || entry.text === answerText).length === 2, timeline);
    } finally { check(`${name} torn-tail restarted hub stops`, await stop(again.proc)); }
  }
}
async function refuses(name: string, label: string, home: string, reason: RegExp) {
  const path = join(home, "events.jsonl"), h = start(name, home);
  let bound = false;
  const end = Date.now() + 12000;
  while (Date.now() < end && h.proc.exitCode === null) {
    bound ||= await listening(h.port);
    if (bound) break;
    await sleep(25);
  }
  const exited = h.proc.exitCode !== null, code = h.proc.exitCode, elapsed = Date.now() - h.opened;
  check(`${name} ${label} exits nonzero before serving`, exited && code !== 0 && !bound, { exited, code, bound, elapsed });
  check(`${name} ${label} captured process stops`, await stop(h.proc));
  const [stdout, stderr] = await within(h.output, 2000) ?? ["", "output pipe did not close"];
  check(`${name} ${label} names the event log and why it cannot be read`, stderr.includes(path) && reason.test(stderr), { stdout, stderr });
  check(`${name} ${label} opens no listener`, !bound && !await listening(h.port));
  check(`${name} ${label} invokes no provider turn`, launches(home) === "", launches(home));
  return elapsed;
}
async function failureCases(name: string) {
  const home = homeFor(name, "write-only"), path = join(home, "events.jsonl"), bytes = history(home);
  writeFileSync(path, bytes); chmodSync(path, 0o200);
  try {
    // CI runs as an ordinary user. An elevated account can read mode 0200
    // and cannot meaningfully exercise this regression.
    let denied = false;
    try { const fd = openSync(path, "r"); closeSync(fd); } catch { denied = true; }
    check(`${name} permission fixture actually denies reads`, denied, { uid: process.getuid?.(), mode: statSync(path).mode & 0o777 });
    if (denied) await refuses(name, "write-only log", home, /permission denied|operation not permitted|EACCES/i);
    check(`${name} failed read preserves the log permissions`, (statSync(path).mode & 0o777) === 0o200);
  } finally { chmodSync(path, 0o600); }
  check(`${name} failed read leaves every original log byte unchanged`, readFileSync(path).equals(bytes));
  const repaired = start(name, home);
  try {
    const info = await ready(repaired);
    check(`${name} repairing read permission lets the hub start`, !!info?.backplane);
    if (info) await replayed(repaired, `${name} repaired permission`);
  } finally { check(`${name} permission-repaired captured hub stops`, await stop(repaired.proc)); }

  const sparseHome = homeFor(name, "oversized"), sparsePath = join(sparseHome, "events.jsonl"), head = history(sparseHome);
  const last = Buffer.from("original sparse log boundary\n"), size = 0x100000000 + head.length;
  const fd = openSync(sparsePath, "w");
  try { writeSync(fd, head, 0, head.length, 0); ftruncateSync(fd, size); writeSync(fd, last, 0, last.length, size - last.length); }
  finally { closeSync(fd); }
  const elapsed = await refuses(name, "oversized log", sparseHome, /too (?:big|large)|exceeds|limit|4294967295|4 GiB|4 GB/i);
  check(`${name} oversized log fails promptly without reading 4 GiB`, elapsed < 12000, elapsed);
  check(`${name} oversized read preserves the exact log size`, statSync(sparsePath).size === size, statSync(sparsePath).size);
  const readFd = openSync(sparsePath, "r"), actualHead = Buffer.alloc(head.length), actualLast = Buffer.alloc(last.length);
  try { readSync(readFd, actualHead, 0, actualHead.length, 0); readSync(readFd, actualLast, 0, actualLast.length, size - last.length); }
  finally { closeSync(readFd); }
  check(`${name} oversized read preserves both original boundaries`, actualHead.equals(head) && actualLast.equals(last));

  const directoryHome = homeFor(name, "directory"), directory = join(directoryHome, "events.jsonl");
  mkdirSync(directory); writeFileSync(join(directory, "keep"), "original directory contents\n");
  await refuses(name, "directory log", directoryHome, /directory|not a regular file|EISDIR/i);
  check(`${name} refused directory keeps its contents`, statSync(directory).isDirectory() && readFileSync(join(directory, "keep"), "utf8") === "original directory contents\n");
}

try {
  mkdirSync(prefix); mkdirSync(fake);
  for (const name of ["backplane", "backplane-serve"]) { copyFileSync(resolve(binary), join(prefix, name)); chmodSync(join(prefix, name), 0o755); }
  // Model catalog/version probes are allowed. Every other invocation is
  // recorded as a turn and immediately fails, with no real provider fallback.
  const provider = '#!/bin/sh\nprintf "%s\\t%s\\n" "$0" "$*" >> "$BP_LOGREAD_PROBES"\ncase "$*" in\n  "--version"|"models"|"-p --input-format stream-json --output-format stream-json --verbose"|"serve --no-session-log") exit 1;;\nesac\nprintf "%s\\t%s\\n" "$0" "$*" >> "$BP_LOGREAD_LAUNCHES"\nexit 1\n';
  for (const name of ["claude", "codex", "grok", "muse", "agy"]) { writeFileSync(join(fake, name), provider); chmodSync(join(fake, name), 0o755); }
  for (const name of ["tailscale", "curl", "wget", "ssh", "gh", "kicad-cli", "freecad", "FreeCAD", "busctl", "secret-tool"]) { writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, name), 0o755); }
  for (const name of ["backplane-browser", "backplane-kipc"]) { writeFileSync(join(prefix, name), "#!/bin/sh\nexit 1\n"); chmodSync(join(prefix, name), 0o755); }
  if (reader) {
    // This fixture exists only with the strict reader. Never pass the
    // 2 GiB allocation-boundary case to an older full hub binary.
    const fixtureHome = join(root, "reader-home");
    async function probe(args: string[]) {
      const proc = Bun.spawn([resolve(reader!), ...args], { env: environment(fixtureHome), stdout: "pipe", stderr: "pipe" });
      children.add(proc);
      const output = Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
      const code = await within(proc.exited, 15000);
      check("captured reader fixture stops", await stop(proc));
      const [stdout, stderr] = await within(output, 2000) ?? ["", "output pipe did not close"];
      return { code, stdout, stderr };
    }
    const unicode = join(root, "unicode-read"), unicodeBytes = Buffer.from("café 工作 🛠️\n"); writeFileSync(unicode, unicodeBytes);
    const decoded = await probe([unicode]);
    check("native reader preserves the exact non-ASCII bytes", decoded.code === 0 && decoded.stdout.trim() === `read ${unicodeBytes.length}\n${[...unicodeBytes].join(",")}` && readFileSync(unicode).equals(unicodeBytes), decoded);
    const large = join(root, "large-read"), bytes = Buffer.alloc(8 * 1024 * 1024, 0x61); writeFileSync(large, bytes);
    const counted = await probe(["--count", large]);
    check("native reader counts an 8 MiB file without overflowing the stack", counted.code === 0 && counted.stdout.trim() === `read ${bytes.length}`, counted);
    const short = await probe(["--short-read", large]);
    check("native reader rejects a short read with the path and byte counts", short.code === 1 && short.stderr.includes(large) && /incomplete/.test(short.stderr) && /expected 4 bytes, got 3/.test(short.stderr), short);
    check("large and incomplete native reads preserve every source byte", readFileSync(large).equals(bytes));
    const capped = join(root, "two-gib-read"), capSize = 0x80000000, marker = Buffer.from("original allocation boundary\n");
    const fd = openSync(capped, "w");
    try { writeSync(fd, marker, 0, marker.length, 0); ftruncateSync(fd, capSize); writeSync(fd, marker, 0, marker.length, capSize - marker.length); }
    finally { closeSync(fd); }
    const limit = await probe(["--count", capped]);
    check("native reader refuses the 2 GiB allocation boundary before reading", limit.code === 1 && limit.stderr.includes(capped) && /limit.*2147483647/.test(limit.stderr), limit);
    const readFd = openSync(capped, "r"), head = Buffer.alloc(marker.length), tail = Buffer.alloc(marker.length);
    try { readSync(readFd, head, 0, head.length, 0); readSync(readFd, tail, 0, tail.length, capSize - tail.length); }
    finally { closeSync(readFd); }
    check("rejected 2 GiB read preserves the size and both boundaries", statSync(capped).size === capSize && head.equals(marker) && tail.equals(marker));
  }
  for (const name of ["backplane", "backplane-serve"]) {
    for (const [label, content] of [["missing log", "missing"], ["empty log", "empty"], ["valid history", "valid"], ["torn tail", "torn"]] as const) {
      try { await healthy(name, label, content); } catch (error) { check(`${name} ${label} run`, false, String(error)); }
    }
    try { await failureCases(name); } catch (error) { check(`${name} failure scenarios`, false, String(error)); }
  }
} finally {
  for (const proc of children) check("captured child stops during final cleanup", await stop(proc));
  if (children.size === 0) rmSync(root, { recursive: true, force: true });
}
console.log(`${checks} checks, ${failures} failures`);
process.exit(failures ? 1 : 0);
