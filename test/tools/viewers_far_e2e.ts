// End to end: the viewers for a thread elsewhere (core/fview.bend,
// src/server/farview.bend, docs/bots.md "Viewers elsewhere") on two
// headless hubs that pair by themselves. Beta owns a project holding a
// KiCad board (the ecc83 demo), a second board, a part in mech/ with a
// render, and things that must never leave it (.env, .git, a symlink out).
// A client of alpha, which only mirrors beta's thread, lists the project's
// files, has the board brought into alpha's cache, follows a change of the
// board on beta with no polling of its own, gets phone plot chunks of it
// from alpha's plot actor, reads the Mech parts, and sees what an agent on
// beta shows with viewer_show. Requests for a path outside the project, a
// hidden file or a symlink out are refused by beta, also when sent signed
// as alpha. Prints "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/viewers_far_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (the hub's
// CBOR codec: bend test/wire/index.html -o build/wire). Needs KiCad's ecc83
// demo (/usr/share/kicad/demos/ecc83).
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync, existsSync, copyFileSync, symlinkSync, appendFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHmac } from "node:crypto";

const args = process.argv.slice(2);
const [bin = "build/backplane", wire = "build/wire"] = args.filter((a) => !a.startsWith("--"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const demo = "/usr/share/kicad/demos/ecc83";
if (!existsSync(`${demo}/ecc83-pp.kicad_pcb`)) {
  console.log("skip: KiCad's ecc83 demo is not installed");
  process.exit(0);
}

const root = mkdtempSync(join(tmpdir(), "bp-viewers-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
type Client = { ws: WebSocket; seen: any[]; far: any[]; replies: Map<number, any>; n: number; info: any; views: any[]; plots: any[] };

// a plot frame: a map whose first key is 1 ("t") and value "plot"
const isPlot = (b: Uint8Array) =>
  b.length > 7 && b[0] >= 0xa0 && b[0] <= 0xb7 && b[1] === 1 && b[2] === 0x64 && b[3] === 0x70 && b[4] === 0x6c && b[5] === 0x6f && b[6] === 0x74;

function spawn(h: Hub) {
  h.proc = Bun.spawn([resolve(bin), "--home", h.home, "--port", String(h.port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: h.peers, BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
    stdout: "ignore",
    stderr: "ignore",
  });
}

async function up(h: Hub) {
  for (let i = 0; i < 200; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${h.port}/hello`);
      if (r.status === 200) return;
    } catch {}
    await sleep(100);
  }
  throw new Error(`${h.name} did not start`);
}

async function connect(h: Hub): Promise<Client> {
  const c: Client = { ws: null as any, seen: [], far: [], replies: new Map(), n: 0, info: {}, views: [], plots: [] };
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const bytes = new Uint8Array(e.data as ArrayBuffer);
    if (isPlot(bytes)) { c.plots.push({ bytes: bytes.length, at: Date.now() }); return; }
    const o = JSON.parse(W.decode(bytes));
    for (const x of o.items ?? []) c.seen.push(x);
    if (o.t === "far") c.far.push({ ...o, at: Date.now() });
    if (o.t === "reply") c.replies.set(Number(o.id), o);
    if (o.t === "view") c.views.push({ ...o, at: Date.now() });
    if (o.info) c.info = { ...c.info, ...o.info };
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  c.ws = ws;
  return c;
}

async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}

async function rpc(c: Client, m: string, p: object, ms = 20000): Promise<any> {
  const id = ++c.n;
  c.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(ms, () => c.replies.get(id));
}

const farItems = (c: Client, link: string) => c.far.filter((o) => o.link === link).flatMap((o) => o.items.map((i: any) => i.c));

const pa = freePort();
const pb = freePort();
const a: Hub = { name: "alpha", home: join(root, "alpha"), port: pa, peers: `http://127.0.0.1:${pb}`, proc: null };
const b: Hub = { name: "beta", home: join(root, "beta"), port: pb, peers: `http://127.0.0.1:${pa}`, proc: null };
const clients: Client[] = [];

// the project on beta
const project = join(root, "board");
mkdirSync(join(project, "hw"), { recursive: true });
mkdirSync(join(project, "mech", "renders"), { recursive: true });
mkdirSync(join(project, ".git"));
for (const f of ["ecc83-pp.kicad_pro", "ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch"]) copyFileSync(`${demo}/${f}`, join(project, f));
copyFileSync(`${demo}/ecc83-pp.kicad_pcb`, join(project, "hw", "second.kicad_pcb"));
writeFileSync(join(project, ".env"), "SECRET=1\n");
writeFileSync(join(project, ".git", "config"), "[core]\n");
writeFileSync(join(project, "mech", "part.step"), "ISO-10303-21;\nHEADER;\nENDSEC;\nEND-ISO-10303-21;\n");
const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64");
writeFileSync(join(project, "mech", "renders", "part.iso.png"), png);
symlinkSync("/etc", join(project, "hw", "etclink"));
symlinkSync("/etc/passwd", join(project, "leak.kicad_pcb"));

const hex = (n: number) => [...crypto.getRandomValues(new Uint8Array(n))].map((x) => x.toString(16).padStart(2, "0")).join("");

try {
  spawn(a);
  spawn(b);
  await up(a);
  await up(b);
  const ca = await connect(a);
  const cb = await connect(b);
  clients.push(ca, cb);
  const knows = (c: Client, port: number) => String(c.info?.machines ?? "").includes(`127.0.0.1:${port}`);
  const tm = Date.now();
  const both = await until(130000, () => knows(ca, pb) && knows(cb, pa));
  check("each hub lists the other as the owner's", !!both, [ca.info?.machines, cb.info?.machines]);
  const peer = await until(150000, () => ca.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const peerB = await until(8000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0 && peerB?.id === link, [peer, peerB]);
  console.log(`  paired ${Date.now() - tm} ms after start`);

  // beta's project and a thread in it, mirrored on alpha
  const added = await rpc(cb, "project.add", { path: project });
  const pc = await until(5000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && c.root === project));
  const made = await rpc(cb, "thread.create", { project: pc?.id ?? added?.id, title: "REMOTE BOARD" });
  const created = await until(5000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.project === pc?.id && c.title === "REMOTE BOARD"));
  const th = created?.id as string;
  check("project and thread made on beta", !!pc && !!th && !!made?.ok, [added, made]);
  const mirrored = await until(90000, () => farItems(ca, link).find((c) => c.$ === "ThreadCreated" && c.id === th));
  check("alpha mirrors the thread", !!mirrored, farItems(ca, link).map((c) => c.$));
  const far = `${link}~${th}`;
  const cache = join(a.home, "farfs", link, project);

  // the Files tab: one folder, hidden things and symlinked folders left out
  const t0 = Date.now();
  const dir = await rpc(ca, "files.dir", { thread: far, path: "" });
  const entries = String(dir?.result?.fmEntries ?? "");
  check("files.dir on a thread elsewhere", !!dir?.ok && entries.includes("f\t") && entries.includes("ecc83-pp.kicad_pcb") && entries.includes("d\t0\thw"), dir);
  check("listing carries the thread's id here and the cache folder", dir?.result?.thread === far && dir?.result?.fmRoot === cache, dir?.result);
  check("hidden and linked things are not listed", !entries.includes(".env") && !entries.includes(".git") && !entries.includes("leak.kicad_pcb\n") || !entries.includes("etclink"), entries);
  console.log(`  files.dir ${Date.now() - t0} ms`);
  const sub = await rpc(ca, "files.dir", { thread: far, path: "hw" });
  check("a subfolder", !!sub?.ok && String(sub?.result?.fmEntries).includes("second.kicad_pcb") && sub.result.fmDir === "hw", sub);
  const up1 = await rpc(ca, "files.dir", { thread: far, path: "../.." });
  check("a folder outside the project is refused", !!up1 && !up1.ok, up1);
  const link1 = await rpc(ca, "files.dir", { thread: far, path: "hw/etclink" });
  check("a symlinked folder out is refused", !link1?.ok || !String(link1?.result?.fmEntries ?? "").includes("passwd"), link1);

  // the cache: far.view brings the project's KiCad files, nothing hidden
  const t1 = Date.now();
  const v1 = await rpc(ca, "far.view", { thread: far, want: "", have: "" });
  const tView = Date.now() - t1;
  check("far.view answers with the cache folder", !!v1?.ok && v1.result?.farView === true && v1.result?.root === cache && v1.result?.thread === far && !v1.result?.same, v1);
  console.log(`  far.view first answer ${tView} ms (${v1?.result?.changed} files)`);
  const same = (rel: string) => existsSync(join(cache, rel)) && readFileSync(join(cache, rel)).equals(readFileSync(join(project, rel)));
  check("the board, schematic and project are in alpha's cache, byte for byte", same("ecc83-pp.kicad_pcb") && same("ecc83-pp.kicad_sch") && same("ecc83-pp.kicad_pro"));
  check("every KiCad file of the project is brought (the listing's own rule), nothing hidden", existsSync(join(cache, "hw", "second.kicad_pcb")) && !existsSync(join(cache, ".env")));
  check("nothing hidden or linked came", !existsSync(join(cache, ".env")) && !existsSync(join(cache, ".git")) && !existsSync(join(cache, "leak.kicad_pcb")) && !existsSync(join(cache, "hw", "etclink")));
  check("no part files left behind", !readdirSync(cache).some((f) => f.startsWith(".farpart")), readdirSync(cache));

  // asking for a file by name
  const v2 = await rpc(ca, "far.view", { thread: far, want: "hw/second.kicad_pcb", have: "" });
  check("a named file comes, and is ready", !!v2?.ok && String(v2.result?.ready).includes("hw/second.kicad_pcb") && same("hw/second.kicad_pcb"), v2);
  // the board changes on beta: the waiting ask is answered, with the new bytes
  const t2 = Date.now();
  const waiting = rpc(ca, "far.view", { thread: far, want: "", have: v1?.result?.ver }, 40000);
  await sleep(1500);
  const before = readFileSync(join(cache, "ecc83-pp.kicad_pcb")).length;
  appendFileSync(join(project, "ecc83-pp.kicad_pcb"), "\n");
  const v3 = await waiting;
  const tChange = Date.now() - t2;
  check("a change on the owner answers the waiting ask", !!v3?.ok && !v3.result?.same && v3.result?.changed >= 1 && v3.result?.ver !== v1?.result?.ver, v3);
  check("alpha's cache has the new board", same("ecc83-pp.kicad_pcb") && readFileSync(join(cache, "ecc83-pp.kicad_pcb")).length === before + 1);
  console.log(`  board changed on beta, cache current on alpha ${tChange} ms after the ask (1.5 s of it waiting to change)`);
  const t3 = Date.now();
  const idle = await rpc(ca, "far.view", { thread: far, want: "", have: v3?.result?.ver }, 40000);
  check("an ask with nothing new is answered same, after a wait", !!idle?.ok && idle.result?.same === true && Date.now() - t3 > 15000, idle);
  console.log(`  idle ask answered after ${Date.now() - t3} ms`);

  // a phone's plot of the cached board, from alpha's plot actor
  const cp = await connect(a);
  clients.push(cp);
  const t4 = Date.now();
  cp.ws.send(W.encode(JSON.stringify({ id: 1, m: "kicad.watch", p: { kind: "board", root: cache, path: "" } })));
  const plot = await until(20000, () => cp.plots.length > 0 && cp.plots[0]);
  check("a phone gets plot chunks of the board on a machine elsewhere", !!plot && plot.bytes > 1000, plot);
  console.log(`  plot ${plot?.bytes} bytes, ${Date.now() - t4} ms from ask to arrival`);

  // Mech: the listing, with its renders brought along
  const mech = await rpc(ca, "mech.list", { thread: far });
  const lines = String(mech?.result?.mech ?? "");
  check("mech.list names the part and its render, under this thread", !!mech?.ok && lines.includes("mech/part.step") && lines.includes("mech/renders/part.iso.png") && mech.result.thread === far && mech.result.root === cache, mech);
  check("the render is in the cache before the listing arrives", same("mech/renders/part.iso.png"));
  const part = await rpc(ca, "far.view", { thread: far, want: "mech/part.step", have: "" });
  check("the part's model comes when asked for", !!part?.ok && same("mech/part.step") && String(part.result?.ready).includes("mech/part.step"), part);

  // what an agent shows with viewer_show on beta opens on alpha
  const shown = fetch(`http://127.0.0.1:${b.port}/mcp/${th}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 5, method: "tools/call", params: { name: "viewer_show", arguments: { path: join(project, "hw", "second.kicad_pcb"), kind: "board" } } }),
  }).then((r) => r.json());
  const view = await until(20000, () => ca.views.find((v) => v.thread === far));
  await shown;
  check("viewer_show on beta opens the file on alpha's clients", !!view && view.path === join(cache, "hw", "second.kicad_pcb") && view.kind === "board", view);
  check("and the file is in the cache by then", same("hw/second.kicad_pcb"));
  check("beta's own clients got it as it is", !!cb.views.find((v) => v.thread === th && v.path === join(project, "hw", "second.kicad_pcb")), cb.views);

  // a diff request and writes
  const diff = await rpc(ca, "git.diff", { thread: far, from: 0, to: -1 });
  check("git.diff is forwarded and answered (not a repository here)", !!diff && diff.ok === false && String(diff.error ?? diff.result?.text ?? "").length > 0, diff);
  const render = await rpc(ca, "mech.render", { thread: far, path: "mech/part.step" });
  check("a render (a write on the owner) is refused", !!render && !render.ok, render);
  const opened = await rpc(ca, "kicad.open", { thread: far });
  check("opening KiCad on the owner is refused", !!opened && !opened.ok, opened);
  const pdf = await rpc(ca, "pdf.page", { thread: far, path: "/etc/passwd", page: "1" });
  check("pdf.page names a path here, never the owner's", !!pdf && !String(JSON.stringify(pdf)).includes("root:"), pdf);

  // beta refuses what is not a read of the project, whoever signs it
  const secret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", link), "utf8")).secret;
  const signed = async (m: string, p: object): Promise<{ status: number; text: string }> => {
    const body = JSON.stringify({ id: 0, m, p: { ...p, n: hex(4) } });
    const ts = Math.floor(Date.now() / 1000);
    const sig = "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${ts}.${body}`).digest("hex");
    const r = await fetch(`http://127.0.0.1:${b.port}/far/rpc`, { method: "POST", body, headers: { "content-type": "application/json", "x-backplane-peer": link, "x-backplane-timestamp": String(ts), "x-backplane-signature": sig } });
    return { status: r.status, text: await r.text() };
  };
  const got = await signed("view.get", { thread: th, path: "hw/second.kicad_pcb" });
  check("beta serves a project file to its owner's machine", got.status === 200 && got.text.length > 1000, got.status);
  for (const [name, p] of [["../../etc/passwd", "../../etc/passwd"], ["an absolute path", "/etc/passwd"], ["a hidden file", ".env"], ["a file in a hidden folder", ".git/config"], ["a symlink out", "leak.kicad_pcb"], ["a file through a symlinked folder", "hw/etclink/passwd"]] as const) {
    const r = await signed("view.get", { thread: th, path: p });
    check(`beta refuses ${name}`, r.status !== 200 || !r.text.includes("root:"), r);
    check(`  (and sends no bytes of it)`, !r.text.includes("SECRET") && !r.text.includes("root:") && !r.text.includes("[core]"), r.text.slice(0, 80));
  }
  const ver = await signed("view.ver", { thread: th, want: ".env\nhw/second.kicad_pcb", have: "x" });
  check("view.ver with a hidden file in want is refused", ver.status === 403, ver);
  const ver2 = await signed("view.ver", { thread: th, want: "hw/second.kicad_pcb\nhw/etclink/passwd\nleak.kicad_pcb", have: "" });
  check("view.ver lists no linked file", ver2.status === 200 && !ver2.text.includes("etclink") && !ver2.text.includes("leak.kicad_pcb") && ver2.text.includes("hw/second.kicad_pcb"), ver2.text.slice(0, 400));
  const w = await signed("turn.start", { thread: th, text: "hi", msg: "x", mode: "queue" });
  check("a write still goes by Fr.methods only (turn.start allowed, setting.set not)", w.status === 200, w);
  const s = await signed("setting.set", { key: "x", value: "y" });
  check("an unrelated method is refused", s.status === 403, s);
  const other = await signed("view.get", { thread: "nosuchthread", path: "ecc83-pp.kicad_pcb" });
  check("a thread that is not shared is refused", other.status === 403, other);
  const dirs = await signed("files.dir", { thread: th, path: "../.." });
  check("files.dir outside the project is refused", dirs.status === 403 || !dirs.text.includes("etc"), dirs);
  const sw = await signed("pdf.page", { thread: th, path: "/etc/passwd", page: "1" });
  check("the owner does not draw a path it was named", sw.status === 403, sw);
  const fo = await signed("file.open", { path: "/etc/passwd", how: "open" });
  check("the owner does not open a path it was named", fo.status === 403, fo);
  const fl = await signed("fs.list", { path: "/" });
  // folders are browsed on the owner by its own machines (create anywhere: the
  // project picker's machine row, Fr.async); only the owner's machines get here
  check("the owner's folders are browsed by its own machines", fl.status === 200, fl);
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "viewers far e2e: FAIL" : "viewers far e2e: ok");
process.exit(failed ? 1 : 0);
