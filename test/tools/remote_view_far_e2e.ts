// End to end: a window on a hub that only MIRRORS a thread (core/rview.bend over
// core/fview.bend's cache). Beta owns the project, alpha mirrors the thread and keeps
// the files in its cache (far.view); a stand-in window of alpha asks alpha's
// /view/ver, /view/get and /view/git for the thread elsewhere and is refused what
// the owner would refuse. Setup as test/tools/viewers_far_e2e.ts, whose header follows:
// (the viewers for a thread elsewhere (core/fview.bend,
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
//   bun test/tools/remote_view_far_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (the hub's
// CBOR codec: bend test/wire/index.html -o build/wire). Needs KiCad's ecc83
// demo (scripts/fetch-cad-fixture.py; BACKPLANE_CAD_FIXTURE overrides its location).
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync, existsSync, copyFileSync, symlinkSync, appendFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHmac } from "node:crypto";

const args = process.argv.slice(2);
const [bin = "build/backplane", wire = "build/wire", sync = "build/rvw_sync"] = args.filter((a) => !a.startsWith("--"));
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

const demo = resolve(process.env.BACKPLANE_CAD_FIXTURE ?? "build/fixtures/ecc83");
if (!existsSync(`${demo}/ecc83-pp.kicad_pcb`)) {
  throw new Error("required CAD fixture missing; run python3 scripts/fetch-cad-fixture.py");
}

const root = mkdtempSync(join(tmpdir(), "bp-viewers-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
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


  // the stand-in window of alpha
  const base = `http://127.0.0.1:${pa}`;
  const q = (o: Record<string, string>) => "?" + Object.entries(o).map(([k, v]) => `${k}=${encodeURIComponent(v)}`).join("&");
  const get = (path: string, o: Record<string, string>) => fetch(`${base}${path}${q(o)}`);
  const l1 = await get("/view/ver", { thread: far, root: cache, want: "", have: "" });
  const lj: any = l1.status === 200 ? await l1.json() : await l1.text();
  const lfiles = String(lj?.result?.files ?? "").split("\n").filter(Boolean).map((l: string) => l.split("\t")[0]);
  check("alpha lists the mirrored thread's files from its cache", l1.status === 200 && lj?.result?.root === cache && lfiles.includes("ecc83-pp.kicad_pcb") && lfiles.includes("hw/second.kicad_pcb"), lj);
  check("nothing hidden is listed", !lfiles.some((f: string) => f.startsWith(".") || f.includes("/.") || f.includes("etclink") || f === "leak.kicad_pcb"), lfiles);
  const g1 = await get("/view/get", { thread: far, root: cache, path: "ecc83-pp.kicad_pcb" });
  check("a file comes from alpha's cache, byte for byte", g1.status === 200 && Buffer.from(await g1.arrayBuffer()).equals(readFileSync(join(project, "ecc83-pp.kicad_pcb"))), g1.status);
  const st = async (path: string, o: Record<string, string>) => (await get(path, o)).status;
  check("../, absolute and hidden paths are refused", (await st("/view/get", { thread: far, path: "../x" })) === 403 && (await st("/view/get", { thread: far, path: "/etc/passwd" })) === 403 && (await st("/view/get", { thread: far, path: ".env" })) === 403);
  check("a mirrored thread with no folder named is refused", (await st("/view/get", { thread: far, path: "ecc83-pp.kicad_pcb" })) === 403);
  check("a folder outside this link's cache is refused", (await st("/view/get", { thread: far, root: project, path: "ecc83-pp.kicad_pcb" })) === 403 && (await st("/view/get", { thread: far, root: join(a.home, "farfs", "other", project), path: "ecc83-pp.kicad_pcb" })) === 403 && (await st("/view/get", { thread: far, root: join(a.home, "farfs", link, "..", ".."), path: "token" })) === 403);
  check("a link with a path in it is refused", (await st("/view/get", { thread: "../x~y", root: cache, path: "a.kicad_pcb" })) === 403);
  check("a client names no folder for a thread of this hub's own", (await st("/view/get", { thread: "t-none", root: cache, path: "ecc83-pp.kicad_pcb" })) === 403);
  // a change on the owner reaches the window's list through alpha's cache
  const t6 = Date.now();
  const hold = fetch(`${base}/view/ver${q({ thread: far, root: cache, want: "", have: String(lj.result.ver) })}`).then((r) => r.status);
  await sleep(1000);
  appendFileSync(join(project, "ecc83-pp.kicad_pcb"), "\n");
  const v2 = await rpc(ca, "far.view", { thread: far, want: "", have: String(v1?.result?.ver) });
  check("alpha's cache follows the owner", !!v2?.ok, v2);
  const h2 = await Promise.race([hold, sleep(20000).then(() => "late")]);
  console.log(`  mirrored list long poll answered ${Date.now() - t6} ms after it began`);
  check("and the window's long poll is woken by it", h2 === 200, h2);
  check("the new bytes are served", Buffer.from(await (await get("/view/get", { thread: far, root: cache, path: "ecc83-pp.kicad_pcb" })).arrayBuffer()).equals(readFileSync(join(project, "ecc83-pp.kicad_pcb"))));
  // the window's own code (src/app/rvw.bend) on alpha
  if (existsSync(sync)) {
    const cacheDir = join(root, "xdg");
    const p = Bun.spawn([resolve(sync), base, far, cacheDir, "", "", cache], { stdout: "pipe", stderr: "pipe", env: { ...process.env, PATH: "/usr/bin:/bin" } });
    const text = await new Response(p.stdout).text();
    await p.exited;
    const copy = join(cacheDir, "remote", "h" + base.replace(/[^A-Za-z0-9.-]/g, "_"), cache);
    check("the window's round on a mirrored thread fills its copy", /round ok=True/.test(text) && existsSync(join(copy, "ecc83-pp.kicad_pcb")) && readFileSync(join(copy, "ecc83-pp.kicad_pcb")).equals(readFileSync(join(project, "ecc83-pp.kicad_pcb"))), text);
  }
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "remote view far e2e: FAIL" : "remote view far e2e: ok");
process.exit(failed ? 1 : 0);
