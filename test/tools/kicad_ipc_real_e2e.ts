// KiCad over IPC against a REAL Backplane KiCad runtime, through the hub, as an agent would use it
// (core/kserve.bend, server/kserve.bend, tools/kipc): a hub on a free port and a temp home, and only the
// hub's MCP tools kicad_ipc / kicad_ipc_status.
//
//   1  health ok for bp.4; kicad_ipc_status: a ready server, a -bp.4 version, commands
//   2  board: OpenDocument, GetItems footprints; counts of footprints, pads, 3D models
//   3  edit + save + reload: a footprint moved +1 mm (the FULL FootprintInstance), pads and models unchanged
//   4  bp.4: a partial UpdateItems preserves pads, models, graphics, layer, rotation and lock through reload
//   5  netlist import: export the schematic's netlist, ImportNetlist, save, reload: the 3D models are still there
//   6  bp.4: PlaceSymbolFromLibrary and CreateItems from a library id without bulk preload, saved with pins
//   7  rollback: BeginCommit, an edit, EndCommit CMA_DROP leaves the board unchanged
//   8  the server's argv: starts with <runtime>/bin/kicad-cli, has --parent-pid and --idle-exit
//
//   bun test/tools/kicad_ipc_real_e2e.ts BINARY WIREDIR RUNTIME_ROOT
// BINARY: a hub (build/backplane-serve); WIREDIR: bend test/wire/index.html -o build/wire; RUNTIME_ROOT: an unpacked
// Backplane KiCad (read-only: the demo is copied). Without a runtime it prints "skipped: no runtime" and exits 0.
// Needs libnng (the runtime's own lib/ is used) and `cd tools/kipc && bun install`.
// The hub is a Bun.spawn child that is stopped at the end; its KiCad server ends with it (--parent-pid).
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const [binary = "build/backplane-serve", wire = "build/wire", runtimeArg = ""] = process.argv.slice(2);
if (!runtimeArg || !existsSync(join(runtimeArg, "bin/kicad-cli"))) {
  console.log("skipped: no runtime");
  process.exit(0);
}
const runtime = resolve(runtimeArg);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const repo = resolve(".");
let failed = false;
function check(name: string, ok: boolean, got?: unknown) {
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(got)?.slice(0, 900)}`}`);
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
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p === 3787 ? freePort() : p;
}

const root = mkdtempSync(join(tmpdir(), "bp-kicad-real-e2e-"));
const bun = process.execPath;
const fake = join(root, "fake-bin");
mkdirSync(fake);
for (const name of ["claude", "codex", "grok", "muse", "agy", "curl"]) {
  writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, name), 0o755);
}
writeFileSync(join(fake, "backplane-kipc"), `#!/bin/sh\nexec "${bun}" "${repo}/tools/kipc/src/main.ts" "$@"\n`);
chmodSync(join(fake, "backplane-kipc"), 0o755);

// the demo, copied (one project in the folder)
const proj = join(root, "projects", "ecc83");
mkdirSync(proj, { recursive: true });
for (const f of ["ecc83-pp.kicad_pcb", "ecc83-pp.kicad_pro", "ecc83-pp.kicad_sch", "ecc83-pp.kicad_sym", "footprints.pretty", "fp-lib-table", "sym-lib-table", "3d_shapes"])
  cpSync(join(runtime, "share/kicad/demos/ecc83", f), join(proj, f), { recursive: true });
const pcb = join(proj, "ecc83-pp.kicad_pcb"), sch = join(proj, "ecc83-pp.kicad_sch");

const home = join(root, "home");
mkdirSync(home, { recursive: true });
const skills = join(home, ".cache/backplane/kistack/ecf16c119c7ed610f5f251c479b03884b8d98b09/skills/pcb");
mkdirSync(skills, { recursive: true });
writeFileSync(join(skills, "SKILL.md"), "---\nname: fixture\ndescription: test fixture\n---\n");
const xdg = join(root, "xdg");
mkdirSync(xdg, { recursive: true });
const port = freePort();
const hub = Bun.spawn([resolve(binary), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: {
    ...process.env, HOME: home, XDG_CACHE_HOME: join(home, ".cache"), XDG_CONFIG_HOME: join(home, ".config"), XDG_DATA_HOME: join(home, ".local/share"), XDG_RUNTIME_DIR: xdg,
    BACKPLANE_AGENTS: fake, PATH: `${fake}:${dirname(bun)}:/usr/bin:/bin`, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_HOME: home, BACKPLANE_KICAD: runtime, BACKPLANE_KICAD_CLI: "",
    BACKPLANE_PEERS: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_LOCK_DIR: join(home, "locks"),
  },
  stdout: Bun.file(join(home, "stdout.log")), stderr: Bun.file(join(home, "stderr.log")),
});
let ws: WebSocket | undefined;

try {
  await until(() => existsSync(join(home, "token")), 25000);
  for (let i = 0; ; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {}
    if (i > 400) throw new Error(`hub did not listen: ${readFileSync(join(home, "stderr.log"), "utf8")}`);
    await sleep(50);
  }
  const seen: any[] = [];
  const items: any[] = [];
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    seen.push(o);
    for (const c of o.items ?? []) items.push(c);
  };
  await new Promise<void>((r, reject) => { ws!.onopen = () => r(); ws!.onerror = reject; });
  let n = 0;
  const send = (m: string, p: any = {}) => ws!.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const info = () => [...seen].reverse().find((o) => o.t === "info" || o.t === "log")?.info ?? {};
  const made = (kind: string) => items.filter((c) => c.$ === kind);
  send("project.add", { path: proj });
  const pr = await until(() => made("ProjectCreated")[0]);
  send("thread.create", { project: pr!.id, title: "t" });
  const th = (await until(() => made("ThreadCreated")[0]))!.id as string;
  async function tool(name: string, args: any) {
    const r = await fetch(`http://127.0.0.1:${port}/mcp/${th}?token=${token}`, {
      method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
    });
    const j = JSON.parse(await r.text());
    const text = j.result?.content?.[0]?.text ?? "";
    let body: any = null;
    try { body = JSON.parse(text); } catch {}
    return { error: !!j.result?.isError, text, body, msg: body?.message };
  }
  const ipc = (path: string, type: string, message: any = {}) => tool("kicad_ipc", { path, type, message });
  const C = "kiapi.common.commands.", B = "kiapi.board.commands.", S = "kiapi.schematic.commands.";
  const T = "type.googleapis.com/";

  // 1 health and status
  const ok = await until(() => info()["kicad.status"] === "ok", 90000);
  check("1 health ok for the runtime", !!ok, info());
  check("1 health version ends in -bp.4", String(info()["kicad.fork"] ?? info()["kicad.version"] ?? "").endsWith("-bp.4"), info());
  const st = await tool("kicad_ipc_status", { path: pcb });
  check("1 status: a ready server", !st.error && String(st.body?.server).includes("ready for ecc83-pp"), st.text.slice(0, 400));
  check("1 status: version ends in -bp.4", String(st.body?.version).endsWith("-bp.4"), st.body?.version);
  const cmds = JSON.stringify(st.body?.capabilities ?? {});
  check("1 status: a commands list", cmds.length > 50 && cmds.includes("GetItems"), cmds.slice(0, 300));
  check("1 runtime advertises both bp.4 fix capabilities", cmds.includes("footprint-partial-update") && cmds.includes("symbol-from-libid"), cmds);

  // 2 board
  const open = await ipc(pcb, C + "OpenDocument", { type: "DOCTYPE_PCB", path: pcb });
  let doc = open.msg?.document;
  check("2 OpenDocument (PCB)", !open.error && !!doc, open.text.slice(0, 300));
  const header = () => ({ document: doc });
  const getFps = async () => (await ipc(pcb, C + "GetItems", { header: header(), types: ["KOT_PCB_FOOTPRINT"] })).msg?.items ?? [];
  const tally = (fps: any[]) => {
    let pads = 0, models = 0, kicad10 = 0, graphics = 0;
    for (const f of fps) for (const x of f.definition?.items ?? []) {
      const t = String(x["@type"]);
      if (t.endsWith("board.types.Pad")) pads++;
      if (t.endsWith("Footprint3DModel")) { models++; if (String(x.filename).includes("${KICAD10_3DMODEL_DIR}")) kicad10++; }
      else if (!t.endsWith("board.types.Pad")) graphics++;
    }
    return { fps: fps.length, pads, models, kicad10, graphics };
  };
  const reload = async () => {
    await ipc(pcb, C + "SaveDocument", { document: doc });
    await ipc(pcb, C + "CloseDocument", { document: doc });
    doc = (await ipc(pcb, C + "OpenDocument", { type: "DOCTYPE_PCB", path: pcb })).msg?.document;
    return getFps();
  };
  let fps = await getFps();
  const before = tally(fps);
  console.log(`ecc83 before: footprints=${before.fps} pads=${before.pads} models=${before.models} (with KICAD10_3DMODEL_DIR: ${before.kicad10})`);
  check("2 footprints, pads and 3D models are present", before.fps > 0 && before.pads > 0 && before.models > 0, before);

  // 3 edit, save, reload
  const f0 = fps[0];
  const id0 = f0.id.value;
  const x0 = BigInt(f0.position.x_nm);
  const bc = await ipc(pcb, C + "BeginCommit", { header: header() });
  const upd = await ipc(pcb, C + "UpdateItems", { header: header(), items: [{ ...f0, orientation: { value_degrees: 90 }, locked: "LS_LOCKED", position: { ...f0.position, x_nm: String(x0 + 1000000n) } }] });
  const end = await ipc(pcb, C + "EndCommit", { header: header(), id: bc.msg?.id, action: "CMA_COMMIT", message: "move +1 mm" });
  check("3 the full-instance update is accepted", !bc.error && !upd.error && !end.error, [bc.text, upd.text.slice(0, 200), end.text]);
  fps = await reload();
  const after = tally(fps);
  const moved = fps.find((f: any) => f.id.value === id0);
  console.log(`ecc83 after move: footprints=${after.fps} pads=${after.pads} models=${after.models}`);
  check("3 the footprint is at the new position after reload", !!moved && BigInt(moved.position.x_nm) === x0 + 1000000n, [x0, moved?.position]);
  check("3 pads and 3D models are unchanged", after.pads === before.pads && after.models === before.models && after.fps === before.fps, [before, after]);

  // 4 the bp.4 fix: omit definition, layer, orientation and lock.
  const bcPartial = await ipc(pcb, C + "BeginCommit", { header: header() });
  const partialX = BigInt(moved.position.x_nm) + 500000n;
  const bare = await ipc(pcb, C + "UpdateItems", { header: header(), items: [{ "@type": T + "kiapi.board.types.FootprintInstance", id: f0.id, position: { x_nm: String(partialX), y_nm: moved.position.y_nm } }] });
  const endPartial = await ipc(pcb, C + "EndCommit", { header: header(), id: bcPartial.msg?.id, action: "CMA_COMMIT", message: "partial footprint move" });
  check("4 bp.4 accepts a partial footprint update", !bcPartial.error && !bare.error && bare.msg?.updated_items?.[0]?.status?.code === "ISC_OK" && !endPartial.error, bare.text);
  const partialFps = await reload();
  const still = tally(partialFps);
  const partial = partialFps.find((f: any) => f.id.value === id0);
  check("4 partial move persists after save and reload", !!partial && BigInt(partial.position.x_nm) === partialX, partial?.position);
  check("4 pads, graphics and 3D models survive the partial update", still.pads === before.pads && still.models === before.models && still.graphics === after.graphics && still.fps === before.fps, [before, after, still]);
  check("4 omitted layer, rotation and lock are preserved", partial?.layer === moved.layer && partial?.orientation?.value_degrees === 90 && partial?.locked === "LS_LOCKED", [moved.layer, partial?.layer, partial?.orientation, partial?.locked]);

  // 7 rollback (before the netlist import, which changes the board for good)
  const base = JSON.stringify((await getFps()).find((f: any) => f.id.value === id0).position);
  const bc2 = await ipc(pcb, C + "BeginCommit", { header: header() });
  await ipc(pcb, C + "UpdateItems", { header: header(), items: [{ ...moved, position: { x_nm: "1", y_nm: "1" } }] });
  const drop = await ipc(pcb, C + "EndCommit", { header: header(), id: bc2.msg?.id, action: "CMA_DROP" });
  const rolled = JSON.stringify((await getFps()).find((f: any) => f.id.value === id0).position);
  check("7 CMA_DROP leaves the board unchanged", !drop.error && rolled === base, [base, rolled]);

  // 5 netlist import
  const sdoc = (await ipc(sch, C + "OpenDocument", { type: "DOCTYPE_SCHEMATIC", path: sch })).msg?.document;
  const net = join(proj, "e2e.net");
  const ex = await ipc(sch, "kiapi.schematic.jobs.RunSchematicJobExportNetlist", { job_settings: { document: sdoc, output_path: net }, format: "SNF_KICAD_SEXPR" });
  const imp = ex.error ? ex : await ipc(pcb, B + "ImportNetlist", { board: doc, netlist_path: net, match_mode: "NMM_REFERENCE", update_footprints: true });
  if (imp.error && /unimplemented|not available|unhandled|no schema/i.test(imp.text)) {
    console.log(`skipped: netlist import (${imp.text.slice(0, 200)})`);
  } else {
    check("5 ImportNetlist ran without errors", !imp.error && (imp.msg?.error_count ?? 0) === 0, imp.text.slice(0, 400));
    const t5 = tally(await reload());
    console.log(`ecc83 after netlist import: footprints=${t5.fps} pads=${t5.pads} models=${t5.models} (with KICAD10_3DMODEL_DIR: ${t5.kicad10})`);
    check("5 3D models are still there after import + save + reload", t5.models >= before.models && t5.pads === before.pads, [before, t5]);
    check("5 models with ${KICAD10_3DMODEL_DIR} paths are present", t5.kicad10 > 0, t5);
  }

  // 6 symbol
  const sdoc2 = (await ipc(sch, C + "OpenDocument", { type: "DOCTYPE_SCHEMATIC", path: sch })).msg?.document ?? sdoc;
  const getSyms = async () => (await ipc(sch, C + "GetItems", { header: { document: sdoc2 }, types: ["KOT_SCH_SYMBOL"] })).msg?.items ?? [];
  const symsBefore = (await getSyms()).length;
  // No LoadAllLibraries: bp.4 resolves the requested library on demand.
  const placed = await ipc(sch, S + "PlaceSymbolFromLibrary", { header: { document: sdoc2 }, lib_id: { library_nickname: "Device", entry_name: "R" }, position: { x_nm: "50000000", y_nm: "250000000" } });
  check("6 PlaceSymbolFromLibrary loads Device on demand", !placed.error && !!placed.msg?.item?.id, placed.text.slice(0, 300));
  const placedId = placed.msg?.item?.id?.value;
  const createdId = crypto.randomUUID();
  const symbolBegin = await ipc(sch, C + "BeginCommit", { header: { document: sdoc2 } });
  const created = await ipc(sch, C + "CreateItems", { header: { document: sdoc2 }, items: [{ "@type": T + "kiapi.schematic.types.SchematicSymbolInstance", id: { value: createdId }, position: { x_nm: "55000000", y_nm: "250000000" }, definition: { id: { library_nickname: "Connector_Generic", entry_name: "Conn_01x02" } } }] });
  const symbolEnd = await ipc(sch, C + "EndCommit", { header: { document: sdoc2 }, id: symbolBegin.msg?.id, action: "CMA_COMMIT", message: "create symbol from library id" });
  check("6 CreateItems accepts a library-id-only definition", !symbolBegin.error && !created.error && created.msg?.created_items?.[0]?.status?.code === "ISC_OK" && !symbolEnd.error, created.text.slice(0, 500));
  await ipc(sch, C + "SaveDocument", { document: sdoc2 });
  await ipc(sch, C + "CloseDocument", { document: sdoc2 });
  const sdoc3 = (await ipc(sch, C + "OpenDocument", { type: "DOCTYPE_SCHEMATIC", path: sch })).msg?.document;
  const syms = (await ipc(sch, C + "GetItems", { header: { document: sdoc3 }, types: ["KOT_SCH_SYMBOL"] })).msg?.items ?? [];
  const pinCount = (id: string) => (syms.find((s: any) => s.id?.value === id)?.definition?.items ?? []).filter((c: any) => String(c.item?.["@type"]).endsWith("Pin")).length;
  console.log(`schematic symbols: ${symsBefore} -> ${syms.length}; placed pins=${pinCount(placedId)}, created pins=${pinCount(createdId)}`);
  check("6 both symbols persist with two pins each", syms.length === symsBefore + 2 && pinCount(placedId) === 2 && pinCount(createdId) === 2, [syms.length, symsBefore, pinCount(placedId), pinCount(createdId)]);

  // 8 argv
  const st2 = await tool("kicad_ipc_status", { path: pcb });
  const argv = String(st2.body?.argv ?? "");
  check("8 argv starts with the runtime's launcher", argv.startsWith(`${runtime}/bin/kicad-cli api-server `) && !argv.includes("kicad-cli.real"), argv);
  check("8 argv has --parent-pid and --idle-exit", argv.includes(" --parent-pid ") && argv.includes(" --idle-exit "), argv);
} catch (e) {
  check("e2e completed", false, String((e as Error)?.stack ?? e));
} finally {
  try { ws?.close(); } catch {}
  // the hub is our child; the KiCad server follows it (--parent-pid)
  if (hub.exitCode === null) hub.kill("SIGTERM");
  await Promise.race([hub.exited, sleep(10000)]);
  if (failed) console.log(`fixtures and hub logs retained at ${root}`);
  else rmSync(root, { recursive: true, force: true });
  process.exit(failed ? 1 : 0);
}
