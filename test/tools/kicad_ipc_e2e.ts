// KiCad over IPC through the hub, end to end (core/kserve.bend, server/kserve.bend, tools/kipc).
// A hub on a temporary home and a free port, a fake Backplane KiCad runtime
// whose `kicad-cli api-server` is a Bun stub (test/tools/kipc_stub.ts, an
// nng REP server), and the hub's MCP tools kicad_ipc / kicad_ipc_status.
//
//   lazy start     the first call starts the project's server; GetItems answers two items
//   refused        a selection command is answered by the hub, the stub never hears it
//   status         kicad_ipc_status names the server and its commands
//   eviction       a third project retires the least recently used one through Shutdown
//   crash          the server dies after each test.Crash: restarts after 1 s, 5 s, 30 s; the fourth leaves it
//                  "failed", the next call names the fallback, Retry starts it again
//   stock          a KiCad without the fork: the tool's error names the fallback, the thread gets an activity line
//   idle-exit      a server that ends itself is "stopped", not failed, and the next call starts it again
//
// Nothing here signals a process: stubs end by themselves (their parent, the hub, going away; idle-exit;
// Shutdown), and the hubs end by `timeout`.
//
//   bun test/tools/kicad_ipc_e2e.ts [BINARY] [WIREDIR]
// BINARY: a hub (default build/backplane-serve); WIREDIR: bend test/wire/index.html -o build/wire.
// Needs libnng (apt-get install libnng1) and `cd tools/kipc && bun install`.
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const [binary = "build/backplane-serve", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const repo = resolve(".");
let failed = false;
function check(name: string, ok: boolean, got?: unknown) {
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(got)?.slice(0, 600)}`}`);
  if (!ok) failed = true;
}
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(f: () => T | undefined | false, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v as T;
    await sleep(50);
  }
  return undefined;
}
function freePort() {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p === 3787 ? freePort() : p;
}

const root = mkdtempSync(join(tmpdir(), "bp-kicad-ipc-e2e-"));
const bun = process.execPath;
const fake = join(root, "fake-bin");
mkdirSync(fake);
for (const name of ["claude", "codex", "grok", "muse", "agy", "curl"]) {
  writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, name), 0o755);
}
// the IPC helper, from source
writeFileSync(join(fake, "backplane-kipc"), `#!/bin/sh\nexec "${bun}" "${repo}/tools/kipc/src/main.ts" "$@"\n`);
chmodSync(join(fake, "backplane-kipc"), 0o755);

const manifest = {
  manifestVersion: 2, sourceRepository: "https://github.com/i2cjak/Backplane_KiCad", sourceCommit: "abc", upstreamCommit: "up",
  version: "10.99.0-bp.4", kicadVersion: "10.99.0", apiVersion: 2, apiDescriptor: "share/backplane/api.desc",
  capabilities: ["api-server", "parent-pid", "idle-exit", "shutdown", "api-descriptor"],
};
function runtime(name: string, fork: boolean) {
  const dir = join(root, name);
  mkdirSync(join(dir, "bin"), { recursive: true });
  mkdirSync(join(dir, "share/backplane"), { recursive: true });
  copyFileSync(join(repo, "test/fixtures/kipc/api.desc"), join(dir, "share/backplane/api.desc"));
  const version = fork ? "10.99.0-bp.4" : "10.0.6";
  writeFileSync(join(dir, "bin/kicad-cli"), `#!/bin/sh
case "$1" in
  version) echo ${version}; exit 0;;
  api-server) [ "$2" = "--describe" ] && exit 2; exec "${bun}" "${repo}/test/tools/kipc_stub.ts" "$@";;
esac
exit 2
`);
  chmodSync(join(dir, "bin/kicad-cli"), 0o755);
  if (fork) writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  return dir;
}
function project(name: string) {
  const dir = join(root, "projects", name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "x.kicad_pro"), "{}");
  writeFileSync(join(dir, "x.kicad_pcb"), "(kicad_pcb)");
  return dir;
}

type Hub = Awaited<ReturnType<typeof start>>;
async function start(name: string, rt: string, extra: Record<string, string> = {}) {
  const home = join(root, `home-${name}`);
  mkdirSync(home, { recursive: true });
  const skills = join(home, ".cache/backplane/kistack/ecf16c119c7ed610f5f251c479b03884b8d98b09/skills/pcb");
  mkdirSync(skills, { recursive: true });
  writeFileSync(join(skills, "SKILL.md"), "---\nname: fixture\ndescription: test fixture\n---\n");
  const xdg = join(root, `xdg-${name}`);
  mkdirSync(xdg, { recursive: true });
  const log = join(home, "stub.log");
  writeFileSync(log, "");
  const port = freePort();
  // `timeout` ends the hub by itself after the test's own limit
  const proc = Bun.spawn(["timeout", "420", resolve(binary), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: {
      ...process.env, HOME: home, XDG_CACHE_HOME: join(home, ".cache"), XDG_RUNTIME_DIR: xdg, BACKPLANE_AGENTS: fake, PATH: `${fake}:${dirname(bun)}:/usr/bin:/bin`,
      DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_HOME: home, BACKPLANE_KICAD: rt, BACKPLANE_KICAD_CLI: "", BACKPLANE_PEERS: "", BACKPLANE_NO_UPDATE: "1",
      BACKPLANE_LOCK_DIR: join(home, "locks"), KIPC_DESC: join(rt, "share/backplane/api.desc"), KIPC_LOG: log, ...extra,
    },
    stdout: Bun.file(join(home, "stdout.log")), stderr: Bun.file(join(home, "stderr.log")),
  });
  await until(() => existsSync(join(home, "token")), 25000);
  for (let i = 0; ; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {}
    if (i > 200) throw new Error(`hub ${name} did not listen: ${readFileSync(join(home, "stderr.log"), "utf8")}`);
    await sleep(50);
  }
  const seen: any[] = [];
  const items: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    seen.push(o);
    for (const c of o.items ?? []) items.push(c);
  };
  await new Promise<void>((r, reject) => { ws.onopen = () => r(); ws.onerror = reject; });
  let n = 0;
  const send = (m: string, p: any = {}) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const info = () => [...seen].reverse().find((o) => o.t === "info" || o.t === "log")?.info ?? {};
  const servers = () => String(info()["kicad.ipc.servers"] ?? "").split("\n").filter(Boolean);
  const state = (p: string) => servers().find((l) => l.startsWith(p + "|"))?.split("|")[1];
  const made = (kind: string) => items.filter((c) => c.$ === kind);
  const thread = async (p: string) => {
    const np = made("ProjectCreated").length;
    send("project.add", { path: p });
    const pr = await until(() => made("ProjectCreated").length > np && made("ProjectCreated")[np]);
    const nt = made("ThreadCreated").length;
    send("thread.create", { project: pr!.id, title: "t" });
    const th = await until(() => made("ThreadCreated").length > nt && made("ThreadCreated")[nt]);
    return th!.id as string;
  };
  async function tool(th: string, name: string, args: any) {
    const r = await fetch(`http://127.0.0.1:${port}/mcp/${th}?token=${token}`, {
      method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
    });
    const j = JSON.parse(await r.text());
    const text = j.result?.content?.[0]?.text ?? "";
    let body: any = null;
    try { body = JSON.parse(text); } catch {}
    return { error: !!j.result?.isError, text, body, raw: j };
  }
  return { proc, ws, seen, items, send, info, servers, state, thread, tool, home, log };
}
const stubLog = (h: Hub) => readFileSync(h.log, "utf8").split("\n").filter(Boolean).map((l) => l.split(" "));
const count = (h: Hub, type: string) => stubLog(h).filter((l) => l[1] === type).length;

try {
  const rt = runtime("fork", true);
  const pa = project("a"), pb = project("b"), pc = project("c");
  const h = await start("main", rt, { KIPC_DIRTY: "/b/" });
  check("health ok with the 10.99 runtime", !!(await until(() => h.info()["kicad.status"] === "ok")), h.info());
  const th = await h.thread(pa);

  // lazy start
  check("no server before the first call", h.servers().length === 0 && stubLog(h).length === 0, h.servers());
  const items = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems", message: { types: ["track"] } });
  check("GetItems answers two items with KIIDs", !items.error && items.body?.message?.items?.length === 2 && items.body.message.items[0].id.value === "00000000-0000-0000-0000-000000000001", items);
  check("the stub heard Ping, GetVersion, GetItems in order", JSON.stringify(stubLog(h).map((l) => l[1])) === JSON.stringify(["kiapi.common.commands.Ping", "kiapi.common.commands.GetVersion", "kiapi.board.commands.GetItems"]), stubLog(h));
  await until(() => h.state(join(pa, "x.kicad_pro")) === "ready");
  check("Settings' info lists the ready server", h.state(join(pa, "x.kicad_pro")) === "ready", h.servers());
  const second = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pro"), type: "kiapi.board.commands.GetItems" });
  check("a .kicad_pro path reaches the same server", !second.error && count(h, "kiapi.common.commands.Ping") === 1, stubLog(h));

  // refused
  const before = stubLog(h).length;
  const refused = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.common.commands.GetSelection", message: {} });
  check("a selection command is refused with the explicit-ID advice", refused.error && refused.text.includes("not available in headless KiCad: pass explicit item IDs"), refused);
  check("the stub never heard it", stubLog(h).length === before, stubLog(h));
  const bad = await h.tool(th, "kicad_ipc", { path: "x.kicad_pcb", type: "kiapi.board.commands.GetItems" });
  check("a relative path is an error", bad.error && bad.text.includes("absolute"), bad);

  // status
  const st = await h.tool(th, "kicad_ipc_status", { path: join(pa, "x.kicad_pcb") });
  check("status names the server", !st.error && String(st.body?.server).includes("ready for x"), st);
  check("status lists the commands", JSON.stringify(st.body?.capabilities).includes("kiapi.board.commands.GetItems"), st);

  // argv: the launcher, with --parent-pid and --idle-exit (apiVersion 2)
  const argv = String(st.body?.argv ?? "");
  check("status shows the start argv with the launcher, --parent-pid and --idle-exit", argv.startsWith(`${rt}/bin/kicad-cli api-server `) && argv.includes(" --parent-pid ") && argv.includes(" --idle-exit 600"), argv);

  // the bp.3 guards: refused by the hub, the stub never hears them
  const beforeGuard = stubLog(h).length;
  const bare = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.common.commands.UpdateItems",
    message: { items: [{ "@type": "type.googleapis.com/kiapi.board.types.FootprintInstance", id: { value: "00000000-0000-0000-0000-000000000001" } }] } });
  check("a footprint update without its definition is refused", bare.error && bare.text.includes("send the full FootprintInstance from GetItems with only the fields you change"), bare);
  const hollow = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.common.commands.CreateItems",
    message: { items: [{ "@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance", lib_id: { library_nickname: "Device", entry_name: "R" } }] } });
  check("a symbol from a library id alone is refused", hollow.error && hollow.text.includes("PlaceSymbolFromLibrary"), hollow);
  const nestedId = await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.common.commands.CreateItems",
    message: { items: [{ "@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance", definition: { id: { library_nickname: "Device", entry_name: "R" } } }] } });
  check("a symbol with only definition.id is refused without the fix capability", nestedId.error && nestedId.text.includes("PlaceSymbolFromLibrary"), nestedId);
  check("the stub heard no guarded requests", stubLog(h).length === beforeGuard, stubLog(h));

  // eviction
  await sleep(1100);
  const tb = await h.thread(pb);
  const b1 = await h.tool(tb, "kicad_ipc", { path: join(pb, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("a second project has its own server", !b1.error && h.state(join(pb, "x.kicad_pro")) === "ready" && h.state(join(pa, "x.kicad_pro")) === "ready", h.servers());
  await sleep(1100);
  await h.tool(th, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  await sleep(1100);
  const tc = await h.thread(pc);
  const c1 = await h.tool(tc, "kicad_ipc", { path: join(pc, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("a third project is served", !c1.error && h.state(join(pc, "x.kicad_pro")) === "ready", c1);
  // b is the least recently used, but has unsaved changes: it refuses
  // Shutdown, is kept (never forced), and a is asked instead
  check("the LRU (b) and then a were asked to Shutdown", count(h, "kiapi.backplane.Shutdown") === 2, stubLog(h));
  check("b with unsaved changes stays up; a stopped", h.state(join(pb, "x.kicad_pro")) === "ready" && h.state(join(pa, "x.kicad_pro")) === "stopped", h.servers());

  // crash and backoff: project c
  // (the next call waits for the restart; it is sent once the hub has seen the server die)
  const crash = async () => {
    const r = await h.tool(tc, "kicad_ipc", { path: join(pc, "x.kicad_pcb"), type: "test.Crash" });
    await until(() => h.state(join(pc, "x.kicad_pro")) !== "ready", 8000);
    return r;
  };
  const t0 = Date.now();
  const k1 = await crash();
  check("the crash call itself is answered", !k1.error, k1);
  const k2 = await crash();
  const t2 = Date.now() - t0;
  check("after the first crash it restarts (about 1 s)", !k2.error, k2);
  const k3 = await crash();
  const t3 = Date.now() - t0;
  check("after the second it waits about 5 s", !k3.error && t3 - t2 >= 4000, [k3, t3 - t2]);
  const k4 = await crash();
  const t4 = Date.now() - t0;
  check("after the third it waits about 30 s", !k4.error && t4 - t3 >= 25000, [k4, t4 - t3]);
  await until(() => h.state(join(pc, "x.kicad_pro")) === "failed", 15000);
  check("the fourth crash leaves it failed", h.state(join(pc, "x.kicad_pro")) === "failed", h.servers());
  check("the failure carries a reason in info", (h.servers().find((l) => l.startsWith(join(pc, "x.kicad_pro"))) ?? "").split("|")[2]?.length > 0, h.servers());
  const failedCall = await h.tool(tc, "kicad_ipc", { path: join(pc, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("a call on a failed server names the fallback", failedCall.error && failedCall.text.includes("Fallback:"), failedCall);
  h.send("kicad.retry");
  await until(() => h.state(join(pc, "x.kicad_pro")) === "stopped", 8000);
  check("Retry forgets the failure", h.state(join(pc, "x.kicad_pro")) === "stopped", h.servers());
  const again = await h.tool(tc, "kicad_ipc", { path: join(pc, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("and the next call starts it again", !again.error && h.state(join(pc, "x.kicad_pro")) === "ready", again);

  // stock KiCad
  const stock = await start("stock", runtime("stock", false));
  await until(() => stock.info()["kicad.status"] === "stock");
  const ts = await stock.thread(pa);
  const sr = await stock.tool(ts, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("stock KiCad: the error names the fallback", sr.error && sr.text.includes("KiCad IPC unavailable: this is stock KiCad") && sr.text.includes("Fallback:"), sr);
  check("stock KiCad: no server was started", stubLog(stock).length === 0, stubLog(stock));
  const logged = await until(() => stock.items.find((c) => c.$ === "ActivityLogged" && String(c.summary).includes("KiCad: fell back to files")));
  check("the thread gets an activity line", !!logged, stock.items.slice(-3));

  // the setting off
  const offHome = await start("off", rt);
  await until(() => offHome.info()["kicad.status"] === "ok");
  offHome.send("setting.set", { key: "kicad.ipc", value: "off" });
  await sleep(300);
  const to = await offHome.thread(pa);
  const orr = await offHome.tool(to, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("IPC off: the error says so and names the fallback", orr.error && orr.text.includes("turned off") && orr.text.includes("Fallback:"), orr);

  // idle-exit
  const idle = await start("idle", rt, { KIPC_IDLE: "2" });
  await until(() => idle.info()["kicad.status"] === "ok");
  const ti = await idle.thread(pa);
  const i1 = await idle.tool(ti, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  const pidOf = (hh: Hub) => new Set(stubLog(hh).map((l) => l[0]));
  check("idle: first call served", !i1.error && pidOf(idle).size === 1, i1);
  await until(() => idle.state(join(pa, "x.kicad_pro")) === "stopped", 15000);
  check("idle-exit is stopped, not failed", idle.state(join(pa, "x.kicad_pro")) === "stopped", idle.servers());
  const i2 = await idle.tool(ti, "kicad_ipc", { path: join(pa, "x.kicad_pcb"), type: "kiapi.board.commands.GetItems" });
  check("the next call starts it again (a new process)", !i2.error && pidOf(idle).size === 2, [i2, stubLog(idle)]);
  for (const x of [h, stock, offHome, idle]) x.ws.close();
} catch (e) {
  check("e2e completed", false, String(e));
} finally {
  if (failed) console.log(`fixtures and hub logs retained at ${root}`);
  else rmSync(root, { recursive: true, force: true });
  process.exit(failed ? 1 : 0);
}
