// The hub's Rust transport (kipc-probe) and the Bun helper against one real headless KiCad each, through the same NDJSON:
// the same commands, the JSON compared, latency timed at the NDJSON boundary, rounds interleaved. Writes only under WORK, whose
// HOME/XDG_*/KICAD_CONFIG_HOME are temp folders; reads the runtime. Servers end by Shutdown or --parent-pid: this sends no signal.
//   bun test/gpui/files/kipc_real.ts WORK RUNTIME PROBE BUN_MAIN [ROUNDS]     (results: WORK/results.json)
import { spawn } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createInterface } from "node:readline";

const [work, rt, probe, bunMain, rounds = "2"] = process.argv.slice(2);
const desc = `${rt}/share/backplane/api.desc`, lib = `${rt}/lib`, cli = `${rt}/bin/kicad-cli`;
const home = `${work}/home`;
const env: Record<string, string> = { ...process.env as Record<string, string>, HOME: home, XDG_CONFIG_HOME: `${home}/.config`, XDG_CACHE_HOME: `${home}/.cache`, XDG_DATA_HOME: `${home}/.local/share`, XDG_STATE_HOME: `${home}/.local/state`, KICAD_CONFIG_HOME: `${home}/.config/kicad`, DISPLAY: "" };
const cc = "kiapi.common.commands", bc = "kiapi.board.commands";
const pct = (xs: number[], p: number) => { const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };

class Link {
  next = 1; wait = new Map<number, (o: any) => void>(); events: any[] = []; onEvent: ((o: any) => void) | null = null; child: any;
  constructor(argv: string[]) {
    this.child = spawn(argv[0], argv.slice(1), { env, cwd: "/", stdio: ["pipe", "pipe", "inherit"] });
    createInterface({ input: this.child.stdout }).on("line", (l) => {
      const o = JSON.parse(l);
      if (o.event) { this.events.push(o); this.onEvent?.(o); return; }
      const r = this.wait.get(o.id); this.wait.delete(o.id); r?.(o);
    });
  }
  send(o: object): Promise<any> { const id = this.next++; return new Promise((res) => { this.wait.set(id, res); this.child.stdin.write(JSON.stringify({ ...o, id }) + "\n"); }); }
  until(test: (o: any) => boolean, ms: number): Promise<any> {
    const hit = this.events.find(test); if (hit) return Promise.resolve(hit);
    return new Promise((res, rej) => { const t = setTimeout(() => rej(new Error("no event in time: " + JSON.stringify(this.events.slice(-3)))), ms); this.onEvent = (o) => { if (test(o)) { clearTimeout(t); this.onEvent = null; res(o); } }; });
  }
}

async function session(name: string, argv: string[], round: number) {
  const dir = `${work}/r${round}${name[0]}`; rmSync(dir, { recursive: true, force: true }); mkdirSync(`${dir}/s`, { recursive: true });
  for (const f of ["kicad_pro", "kicad_pcb", "kicad_sch"]) cpSync(`${work}/proj/ecc83-pp.${f}`, `${dir}/ecc83-pp.${f}`);
  const sock = `${dir}/s/api.sock`, pro = `${dir}/ecc83-pp.kicad_pro`;
  const l = new Link(argv);
  const t0 = performance.now();
  const started = await l.send({ op: "spawn", key: "p", argv: [cli, "api-server", pro, "--socket", sock, "--parent-pid", String(process.pid), "--idle-exit", "300"] });
  const ready = await l.until((o) => o.event === "line" && o.text?.startsWith("KiCad API server listening at "), 60000);
  const readyMs = performance.now() - t0;
  const at = ready.text.slice("KiCad API server listening at ".length).replace(/^ipc:\/\//, "");
  const ask = (type: string, message: object = {}) => l.send({ sock: at, type, message, desc, lib, timeout: 30000 });
  const out: Record<string, unknown> = { started, sameSocket: at === sock };
  let first = await ask(`${cc}.Ping`); for (let i = 0; i < 100 && !first.ok; i++) { await Bun.sleep(100); first = await ask(`${cc}.Ping`); }
  out.ping = first; out.version = await ask(`${cc}.GetVersion`); out.caps = await ask("kiapi.backplane.GetCapabilities");
  const opened = await ask(`${cc}.OpenDocument`, { type: "DOCTYPE_PCB", path: `${dir}/ecc83-pp.kicad_pcb` }); out.opened = opened;
  out.docs = await ask(`${cc}.GetOpenDocuments`, { type: "DOCTYPE_PCB" });
  const document = opened.message?.document ?? { type: "DOCTYPE_PCB" };
  const header = { document };
  for (const t of ["KOT_PCB_TRACE", "KOT_PCB_ARC", "KOT_PCB_VIA", "KOT_PCB_FOOTPRINT", "KOT_PCB_PAD", "KOT_PCB_ZONE", "KOT_PCB_SHAPE", "KOT_PCB_TEXT"]) out[t] = await ask(`${cc}.GetItems`, { header, types: [t] });
  for (const c of ["GetNets", "GetBoardStackup", "GetBoardEnabledLayers", "GetBoardOrigin", "GetGraphicsDefaults"]) out[c] = await ask(`${bc}.${c}`, { board: document });
  out.titleBlock = await ask(`${cc}.GetTitleBlockInfo`, { document });
  // what KiCad and the codec refuse
  out.refused = await ask(`${cc}.GetSelection`, { header }); out.unknown = await ask("kiapi.board.commands.Nope"); out.badShape = await ask(`${cc}.GetItems`, { header: 5 }); out.noType = await l.send({ sock: at, desc });
  // an edit on this private copy, dropped: a real write path, with Any, enums, 64-bit integers and a commit
  const track = (out.KOT_PCB_TRACE as any).message?.items?.[0];
  const begin = await ask(`${cc}.BeginCommit`, { header });
  const widened = track ? { ...track, width: { value_nm: String(Number(track.width.value_nm) + 1000) } } : null;
  const update = widened ? await ask(`${cc}.UpdateItems`, { header, items: [widened] }) : null;
  const end = await ask(`${cc}.EndCommit`, { header, id: begin.message?.id, action: "CMA_DROP" });
  const after = await ask(`${cc}.GetItems`, { header, types: ["KOT_PCB_TRACE"] });
  out.edit = { begun: begin.ok, updateOk: update?.ok, updated: update?.message?.updated_items?.[0]?.item ?? update?.message, end: end.ok, dropped: JSON.stringify(after.message) === JSON.stringify((out.KOT_PCB_TRACE as any).message) };
  const time = async (type: string, m: object, n: number) => { const xs: number[] = []; for (let i = 0; i < n; i++) { const a = performance.now(); const r = await ask(type, m); xs.push(performance.now() - a); if (!r.ok) throw new Error(JSON.stringify(r)); } return xs; };
  const lat = { Ping: await time(`${cc}.Ping`, {}, 500), GetVersion: await time(`${cc}.GetVersion`, {}, 200), GetItemsFootprints: await time(`${cc}.GetItems`, { header, types: ["KOT_PCB_FOOTPRINT"] }, 40) };
  const size = JSON.stringify(out.KOT_PCB_FOOTPRINT).length;
  const rss = Object.fromEntries(readFileSync(`/proc/${l.child.pid}/status`, "utf8").split("\n").filter((x) => /^Vm(RSS|HWM):/.test(x)).map((x) => [x.split(":")[0], parseInt(x.split(":")[1])])); // kB, the transport process alone
  const shut = await ask("kiapi.backplane.Shutdown");
  const exit = await l.until((o) => o.event === "exit", 30000);
  const closed = await l.send({ op: "close", sock: at });
  l.child.stdin.end();
  // the board's own id is made afresh at each load (two runs of one transport differ by it alone): the only value normalised
  const board = (out.KOT_PCB_TRACE as any).message?.items?.[0]?.parent?.value ?? "<none>";
  return { name, readyMs, out: JSON.parse(JSON.stringify(out).replaceAll(dir, "<dir>").replaceAll(board, "<board>")), lat, size, shut, exit, closed, rss };
}

function diff(a: any, b: any, at = "", acc: string[] = []) {
  if (acc.length > 20) return acc;
  if (typeof a !== typeof b || Array.isArray(a) !== Array.isArray(b) || a === null || b === null) { if (a !== b) acc.push(`${at}: ${JSON.stringify(a)?.slice(0, 120)} | ${JSON.stringify(b)?.slice(0, 120)}`); return acc; }
  if (typeof a !== "object") { if (a !== b) acc.push(`${at}: ${JSON.stringify(a)?.slice(0, 160)} | ${JSON.stringify(b)?.slice(0, 160)}`); return acc; }
  const ka = Object.keys(a), kb = Object.keys(b);
  if (Array.isArray(a) && a.length !== b.length) acc.push(`${at} length: ${a.length} | ${b.length}`);
  if (!Array.isArray(a) && ka.join() !== kb.join()) acc.push(`${at} keys: ${ka.join()} | ${kb.join()}`);
  for (const k of ka) if (k in b) diff(a[k], b[k], `${at}/${k}`, acc);
  return acc;
}

const res: any = { rust: [], bun: [] };
for (let r = 0; r < Number(rounds); r++) {
  res.rust.push(await session("rust", [probe, "--desc", desc], r));
  res.bun.push(await session("bun", [process.execPath, bunMain, "--lib", lib, "--desc", desc], r));
}
const summary: any = { rounds: Number(rounds), ready: {}, lat: {} };
for (const k of ["rust", "bun"]) {
  summary.ready[k] = res[k].map((s: any) => Math.round(s.readyMs));
  for (const c of Object.keys(res[k][0].lat)) { const xs = res[k].flatMap((s: any) => s.lat[c]); (summary.lat[c] ??= {})[k] = { n: xs.length, median: +pct(xs, 0.5).toFixed(3), p95: +pct(xs, 0.95).toFixed(3) }; }
}
const a = res.rust[0], b = res.bun[0];
summary.footprintsJsonBytes = a.size; summary.transportKb = { rust: res.rust.map((s: any) => s.rss), bun: res.bun.map((s: any) => s.rss) };
summary.exit = { rust: a.exit, bun: b.exit }; summary.shutdown = { rust: a.shut, bun: b.shut }; summary.closed = { rust: a.closed, bun: b.closed };
summary.diffs = diff(a.out, b.out);
summary.rawPing = a.out.ping; summary.rawVersion = a.out.version; summary.edit = { rust: a.out.edit, bun: b.out.edit };
summary.opened = a.out.opened; summary.counts = Object.fromEntries(Object.entries(a.out).filter(([k]) => k.startsWith("KOT_")).map(([k, v]: any) => [k, [v.ok, v.message?.items?.length, b.out[k].message?.items?.length, v.error]]));
summary.others = Object.fromEntries(["GetNets", "GetBoardStackup", "GetBoardEnabledLayers", "GetBoardOrigin", "GetGraphicsDefaults", "titleBlock", "refused", "unknown", "badShape", "noType"].map((k) => [k, { ok: a.out[k].ok, status: a.out[k].status, error: a.out[k].error?.slice(0, 120) }]));
writeFileSync(`${work}/results.json`, JSON.stringify({ summary, rust: res.rust.map((s: any) => s.out), bun: res.bun.map((s: any) => s.out) }));
writeFileSync(`${work}/summary.json`, JSON.stringify(summary, null, 1));
console.log(JSON.stringify(summary, null, 1));
process.exit(summary.diffs.length ? 1 : 0);
