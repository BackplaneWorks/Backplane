// Boat-only integration: real Git linkage, the hub's launch RPC and MCP
// viewer/IPC routing. GUI and IPC subprocesses are recording stand-ins.
// bun test/tools/kicad_project_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const [binary = "build/backplane-serve", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), "bp-kicad-main-"));
const home = join(root, "home"), fake = join(root, "bin"), runtime = join(root, "runtime"), project = join(root, "main");
for (const folder of [home, fake, join(runtime, "bin"), project]) mkdirSync(folder, { recursive: true });
const opened = join(root, "opened"), ipc = join(root, "ipc");
function command(path: string, text: string) { writeFileSync(path, text); chmodSync(path, 0o755); }
for (const name of ["claude", "codex", "grok", "muse", "agy", "curl"]) command(join(fake, name), "#!/bin/sh\nexit 1\n");
command(join(runtime, "bin/kicad-cli"), "#!/bin/sh\n[ \"$1\" = version ] || exit 2\necho 10.99.0-bp.4\n");
command(join(runtime, "bin/kicad"), "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$TEST_OPENED\"\n");
writeFileSync(join(runtime, "manifest.json"), JSON.stringify({ version: "10.99.0-bp.4", sourceCommit: "fixture", apiVersion: 2,
  sourceRepository: "https://github.com/i2cjak/Backplane_KiCad" }));
// Record the helper boundary and answer the startup handshake. No real
// KiCad, network socket, GUI or provider account is used by this fixture.
command(join(fake, "backplane-kipc"), `#!${process.execPath}
import { appendFileSync } from "node:fs";
import { createInterface } from "node:readline";
for await (const line of createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  appendFileSync(process.env.TEST_IPC, JSON.stringify(request) + "\\n");
  const message = request.type?.endsWith("GetVersion") ? { version: { full_version: "10.99.0-bp.4" } } : { received: request.message ?? {} };
  console.log(JSON.stringify({ id: request.id, ok: true, type: request.type + "Response", message }));
  if (request.op === "spawn") {
    const socket = request.argv[request.argv.indexOf("--socket") + 1];
    console.log(JSON.stringify({ event: "line", key: request.key, text: "KiCad API server listening at " + socket }));
  }
}
`);
function git(cwd: string, ...args: string[]) {
  const result = Bun.spawnSync(["git", "-C", cwd, ...args], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode) throw Error(result.stderr.toString());
}
function design(folder: string) {
  mkdirSync(folder, { recursive: true });
  writeFileSync(join(folder, "board.kicad_pro"), "{}");
  writeFileSync(join(folder, "board.kicad_pcb"), "(kicad_pcb)");
  writeFileSync(join(folder, "board.kicad_sch"), "(kicad_sch)");
}
git(project, "init", "-q"); design(project);
writeFileSync(join(project, "package.json"), "{}"); // Software policy stays independent of #944.
git(project, "add", "."); git(project, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture");
let worktree = "";
const listen = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const port = listen.port; listen.stop(true);
if (port === 3787) throw Error("refuse live port");
const proc = Bun.spawn([resolve(binary), "--foreground", "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: home, XDG_CACHE_HOME: join(home, ".cache"), PATH: `${fake}:${dirname(process.execPath)}:/usr/bin:/bin`,
    BACKPLANE_HOME: home, BACKPLANE_AGENTS: fake, BACKPLANE_KICAD: runtime, BACKPLANE_KICAD_CLI: "", BACKPLANE_PEERS: "",
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_LOCK_DIR: join(home, "locks"),
    TEST_OPENED: opened, TEST_IPC: ipc, DISPLAY: "", WAYLAND_DISPLAY: "" },
  stdout: Bun.file(join(home, "stdout.log")), stderr: Bun.file(join(home, "stderr.log")) });
let ws: WebSocket | undefined, failed = false;
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(f: () => T | undefined, ms = 30000): Promise<T> {
  const end = Date.now() + ms;
  while (Date.now() < end) { const value = f(); if (value !== undefined) return value; await sleep(30); }
  throw Error("timeout: " + readFileSync(join(home, "stderr.log"), "utf8").slice(-4000));
}
function check(name: string, ok: boolean, value?: unknown) {
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : ": " + JSON.stringify(value)}`); if (!ok) failed = true;
}
const lines = (path: string) => existsSync(path) ? readFileSync(path, "utf8").trim().split("\n").filter(Boolean) : [];
try {
  await until(() => existsSync(join(home, "token")) ? true : undefined);
  for (let i = 0; ; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {}
    if (i > 400) throw Error("hub unavailable"); await sleep(30);
  }
  const token = readFileSync(join(home, "token"), "utf8").trim(), seen: any[] = [], changes: any[] = [];
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => { const o = JSON.parse(W.decode(new Uint8Array(e.data))); seen.push(o); changes.push(...(o.items ?? [])); };
  await new Promise<void>((r, reject) => { ws!.onopen = () => r(); ws!.onerror = reject; });
  let next = 0;
  async function rpc(m: string, p: object) { const id = ++next; ws!.send(W.encode(JSON.stringify({ id, m, p }))); return until(() => seen.find(o => o.t === "reply" && String(o.id) === String(id))); }
  async function tool(thread: string, name: string, args: object) {
    const response = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, { method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++next, method: "tools/call", params: { name, arguments: args } }) });
    const raw = await response.json(); return { raw, body: JSON.parse(raw.result.content[0].text) };
  }
  await until(() => seen.find(o => o.info?.["kicad.status"] === "ok"));
  await rpc("project.add", { path: project });
  const pid = (await until(() => changes.find(c => c.$ === "ProjectCreated" && c.root === project))).id;
  await rpc("thread.create", { project: pid, title: "linked", env: "worktree" });
  const thread = (await until(() => changes.find(c => c.$ === "ThreadCreated" && c.title === "linked"))).id;
  worktree = (await until(() => changes.find(c => c.$ === "WorktreeSet" && c.id === thread))).worktree;
  check("thread uses the linked checkout", worktree.includes("/.backplane/worktrees/") && worktree !== project, worktree);
  let response = await rpc("kicad.open", { thread });
  await until(() => lines(opened).length ? true : undefined);
  check("unmodified worktree opens main", lines(opened).at(-1) === join(project, "board.kicad_pro"), response);
  const identicalWrite = await tool(thread, "kicad_ipc", { type: "kiapi.common.commands.BeginCommit", message: {} });
  check("identical copies allow existing IPC writes", !identicalWrite.raw.result.isError, identicalWrite);
  writeFileSync(join(worktree, "board.kicad_pcb"), "(kicad_pcb modified)");
  const count = lines(opened).length;
  response = await rpc("kicad.open", { thread });
  const result = response.result ?? response;
  check("modified worktree offers the choice", result.kicadCopy === true && result.main === join(project, "board.kicad_pro") && result.copy === join(worktree, "board.kicad_pro"), response);
  check("choice launches no application", lines(opened).length === count);
  await rpc("kicad.open", { thread, copy: "main" }); await until(() => lines(opened).length === count + 1 ? true : undefined);
  check("Open main opens main", lines(opened).at(-1) === join(project, "board.kicad_pro"));
  await rpc("kicad.open", { thread, copy: "thread" }); await until(() => lines(opened).length === count + 2 ? true : undefined);
  check("explicit thread-copy choice opens the worktree", lines(opened).at(-1) === join(worktree, "board.kicad_pro"));
  let shown = await tool(thread, "viewer_show", { path: join(worktree, "board.kicad_pcb") });
  check("viewer_show canonicalizes worktree paths", shown.body.shown === join(project, "board.kicad_pcb"), shown);
  check("viewer broadcast agrees with its result", !!seen.find(o => o.t === "view" && o.thread === thread && o.path === shown.body.shown));
  shown = await tool(thread, "viewer_show", { kind: "schematic" });
  check("viewer_show defaults to main schematic", shown.body.shown === join(project, "board.kicad_sch"), shown);
  shown = await tool(thread, "viewer_show", { kind: "board", thread_copy: true });
  check("viewer accepts explicit thread copy", shown.body.shown === join(worktree, "board.kicad_pcb"), shown);
  const beforeWrite = lines(ipc).length;
  const rejected = await tool(thread, "kicad_ipc", { path: join(worktree, "board.kicad_pcb"), type: "kiapi.common.commands.SaveDocument", message: {} });
  check("divergent IPC write is refused with both paths and flags", rejected.raw.result.isError &&
    JSON.stringify(rejected.body).includes(join(project, "board.kicad_pcb")) && JSON.stringify(rejected.body).includes(join(worktree, "board.kicad_pcb")) &&
    JSON.stringify(rejected.body).includes("thread_copy:true") && JSON.stringify(rejected.body).includes("main:true"), rejected);
  check("refused write never reaches IPC helper", lines(ipc).length === beforeWrite);
  const mainWrite = await tool(thread, "kicad_ipc", { path: join(worktree, "board.kicad_pcb"), main: true, type: "kiapi.common.commands.SaveDocument", message: { path: join(worktree, "board.kicad_pcb") } });
  check("explicit main write is allowed and canonical", !mainWrite.raw.result.isError && mainWrite.body.message?.received?.path === join(project, "board.kicad_pcb"), mainWrite);
  const threadWrite = await tool(thread, "kicad_ipc", { path: join(worktree, "board.kicad_pcb"), thread_copy: true, type: "kiapi.common.commands.SaveDocument", message: { path: join(worktree, "board.kicad_pcb") } });
  check("explicit thread-copy write is allowed", !threadWrite.raw.result.isError && threadWrite.body.message?.received?.path === join(worktree, "board.kicad_pcb"), threadWrite);
  await tool(thread, "kicad_ipc_status", { path: join(worktree, "board.kicad_pro") });
  check("IPC selects main project", lines(ipc).map(s => JSON.parse(s)).some(o => o.op === "spawn" && o.key === join(project, "board.kicad_pro")), lines(ipc));
  await tool(thread, "kicad_ipc_status", { path: join(worktree, "board.kicad_pro"), thread_copy: true });
  check("IPC accepts explicit thread-copy project", lines(ipc).map(s => JSON.parse(s)).some(o => o.op === "spawn" && o.key === join(worktree, "board.kicad_pro")), lines(ipc));
  const doc = join(worktree, "board.kicad_pcb");
  const answer = await tool(thread, "kicad_ipc", { path: doc, type: "kiapi.board.commands.GetItems", message: { header: { document: { path: doc } } } });
  check("IPC command document path agrees with main server", answer.body.message?.received?.header?.document?.path === join(project, "board.kicad_pcb"), answer);
  const copyAnswer = await tool(thread, "kicad_ipc", { path: doc, thread_copy: true, type: "kiapi.board.commands.GetItems", message: { header: { document: { path: doc } } } });
  check("explicit copy preserves IPC command document path", copyAnswer.body.message?.received?.header?.document?.path === doc, copyAnswer);
  const outside = join(root, "absolute"); design(outside);
  writeFileSync(join(worktree, ".backplane.json"), JSON.stringify({ project: join(outside, "board.kicad_pro") }));
  response = await rpc("kicad.open", { thread }); await until(() => lines(opened).length === count + 3 ? true : undefined);
  check("absolute thread configuration workaround survives", lines(opened).at(-1) === join(outside, "board.kicad_pro"), response);
  shown = await tool(thread, "viewer_show", { kind: "board" });
  check("viewer agrees with absolute configuration override", shown.body.shown === join(outside, "board.kicad_pcb"), shown);
  await rpc("thread.create", { project: pid, title: "ordinary", env: "local" });
  const local = (await until(() => changes.find(c => c.$ === "ThreadCreated" && c.title === "ordinary"))).id;
  await rpc("kicad.open", { thread: local }); await until(() => lines(opened).length === count + 4 ? true : undefined);
  check("non-worktree thread opens its project unchanged", lines(opened).at(-1) === join(project, "board.kicad_pro"));
  const ordinaryWrite = await tool(local, "kicad_ipc", { type: "kiapi.common.commands.SaveDocument", message: {} });
  check("non-worktree IPC writes are unchanged", !ordinaryWrite.raw.result.isError, ordinaryWrite);

} catch (error) { console.error(error); failed = true; }
finally { ws?.close(); proc.kill(); await proc.exited; rmSync(root, { recursive: true, force: true }); }
process.exit(failed ? 1 : 0);
