// End to end: a window on another machine's hub (core/rview.bend, docs/bots.md
// "A window on another hub"). One headless hub owns a project holding a
// KiCad board (the ecc83 demo), a git commit of it, and things that must
// never leave it (.env, .git, a symlink out). A stand-in window, a plain
// HTTP client of the hub, lists the thread's viewer files (GET /view/ver),
// holds on the digest until a file changes (the long poll) and is woken by
// the change, fetches a file's bytes (/view/get) and a file of a commit
// (/view/git), and is refused ../, absolute and hidden paths, a symlink
// out, a thread the hub does not have, a git directory that is no history,
// a commit that is an option, and a client the hub does not let in (no
// token). Prints "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/remote_view_e2e.ts [BINARY] [WIREDIR] [RVW_SYNC]
//
// BINARY defaults to build/backplane (build/backplane-serve works too);
// WIREDIR to build/wire; RVW_SYNC (build/rvw_sync, from test/native/rvw_sync.bend
// with scripts/build-app.sh) is the window's own fetching code run against the hub:
// when it is built the test also checks that it fills a cache folder, byte for
// byte, follows a change with one long poll and reads a version of a commit.
// Needs KiCad's ecc83 demo.
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync, existsSync, copyFileSync, symlinkSync, appendFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

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

const root = mkdtempSync(join(tmpdir(), "bp-remote-view-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

const port = freePort();
const home = join(root, "hub");
const project = join(root, "board");
mkdirSync(join(project, "hw"), { recursive: true });
for (const f of ["ecc83-pp.kicad_pro", "ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch"]) copyFileSync(`${demo}/${f}`, join(project, f));
copyFileSync(`${demo}/ecc83-pp.kicad_pcb`, join(project, "hw", "second.kicad_pcb"));
writeFileSync(join(project, ".env"), "SECRET=1\n");
writeFileSync(join(project, "mech.step"), "ISO-10303-21;\nEND-ISO-10303-21;\n");
symlinkSync("/etc", join(project, "hw", "etclink"));
symlinkSync("/etc/passwd", join(project, "leak.kicad_pcb"));
const git = (...a: string[]) => spawnSync("git", ["-C", project, "-c", "user.email=a@b", "-c", "user.name=x", ...a], { encoding: "utf8" });
git("init", "-q");
git("add", "ecc83-pp.kicad_pcb", "ecc83-pp.kicad_pro");
git("commit", "-qm", "first");
const sha = git("rev-parse", "HEAD").stdout.trim();
const committed = readFileSync(join(project, "ecc83-pp.kicad_pcb"));

const base = `http://127.0.0.1:${port}`;
let proc: ReturnType<typeof Bun.spawn> | null = null;
try {
  proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; ; i++) {
    try {
      if ((await fetch(`${base}/hello`)).status === 200) break;
    } catch {}
    if (i > 200) throw new Error("hub did not start");
    await sleep(100);
  }

  // a project and a thread, made as a client does
  const replies = new Map<number, any>();
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const x of o.items ?? []) seen.push(x);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let n = 0;
  const until = async <T>(ms: number, f: () => T | undefined | null | false) => {
    for (const end = Date.now() + ms; ; await sleep(50)) {
      const v = f();
      if (v) return v;
      if (Date.now() > end) return undefined;
    }
  };
  const rpc = async (m: string, p: object) => {
    const id = ++n;
    ws.send(W.encode(JSON.stringify({ id, m, p })));
    return await until(20000, () => replies.get(id));
  };
  const added = await rpc("project.add", { path: project });
  const pc: any = await until(5000, () => seen.find((c) => c.$ === "ProjectCreated" && c.root === project));
  await rpc("thread.create", { project: pc?.id ?? added?.id, title: "REMOTE" });
  const th: string = ((await until(5000, () => seen.find((c) => c.$ === "ThreadCreated" && c.project === pc?.id))) as any)?.id;
  check("project and thread made", !!pc && !!th, [added]);

  const q = (o: Record<string, string>) => "?" + Object.entries(o).map(([k, v]) => `${k}=${encodeURIComponent(v)}`).join("&");
  const get = (path: string, o: Record<string, string>, init: RequestInit = {}) => fetch(`${base}${path}${q(o)}`, init);
  const ver = async (want = "", have = "") => {
    const r = await get("/view/ver", { thread: th, want, have });
    return { status: r.status, body: r.status === 200 ? await r.json() : await r.text() };
  };

  // the list
  const t0 = Date.now();
  const v1 = await ver();
  const res = v1.body?.result;
  const files = String(res?.files ?? "").split("\n").filter(Boolean).map((l: string) => l.split("\t")[0]);
  console.log(`  /view/ver first answer ${Date.now() - t0} ms (${files.length} files)`);
  check("the list answers with the folder, a digest and the files", v1.status === 200 && v1.body?.ok === true && typeof res?.root === "string" && res.root.startsWith("/") && String(res?.ver).length > 0, v1);
  check("it holds the board, schematic, project and a model", ["ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch", "ecc83-pp.kicad_pro", "hw/second.kicad_pcb", "mech.step"].every((f) => files.includes(f)), files);
  check("nothing hidden or linked is listed", !files.some((f: string) => f.startsWith(".") || f.includes("/.") || f.includes("etclink") || f === "leak.kicad_pcb"), files);

  // the thread works in a folder of its own (a worktree of the project)
  const dirT: string = res.root;
  const own = (rel: string) => join(dirT, rel);

  // a file's bytes
  const t1 = Date.now();
  const g1 = await get("/view/get", { thread: th, path: "ecc83-pp.kicad_pcb" });
  const bytes = Buffer.from(await g1.arrayBuffer());
  console.log(`  /view/get ${bytes.length} bytes ${Date.now() - t1} ms`);
  check("a file comes byte for byte", g1.status === 200 && bytes.equals(readFileSync(own("ecc83-pp.kicad_pcb"))), g1.status);
  const g2 = await get("/view/get", { thread: th, path: "hw/second.kicad_pcb" });
  check("a file in a folder", g2.status === 200 && Buffer.from(await g2.arrayBuffer()).length === bytes.length);

  // the long poll
  const t2 = Date.now();
  const hold = new AbortController();
  const waiting = get("/view/ver", { thread: th, want: "", have: String(res.ver) }, { signal: hold.signal }).then((r) => r.status, () => "aborted");
  const early = await Promise.race([waiting, sleep(1500).then(() => "held")]);
  check("with the digest it holds while nothing changes", early === "held", early);
  appendFileSync(own("ecc83-pp.kicad_pcb"), "\n");
  const woke = (await Promise.race([waiting, sleep(8000).then(() => "late")])) as any;
  const wokeMs = Date.now() - t2;
  console.log(`  long poll woken ${wokeMs} ms after it began (change at 1500 ms)`);
  check("a changed file wakes it", woke === 200 && wokeMs < 7000, woke);
  const v2 = await ver();
  check("the digest moved and the board's size with it", v2.body?.result?.ver !== res.ver && String(v2.body?.result?.files).includes(`ecc83-pp.kicad_pcb\t${bytes.length + 1}\t`), v2.body?.result);
  const idle = await Promise.race([ver("", String(v2.body.result.ver)).then((r) => r.status), sleep(2500).then(() => "held")]);
  check("an unchanged list is held, no polling answer", idle === "held", idle);

  // a file asked for by name
  const v3 = await ver("hw/second.kicad_pcb", "");
  check("a named file is listed", String(v3.body?.result?.files).includes("hw/second.kicad_pcb"), v3.body?.result);

  // a file of a commit
  const top = project;
  const hp = (g: string, c: string, rel: string) => `git\t${g}\t${c}\t${top.length}\t${top}/${rel}`;
  const gc = await get("/view/git", { thread: th, path: hp(`${project}/.git`, sha, "ecc83-pp.kicad_pcb") });
  check("a file of a commit is its committed bytes", gc.status === 200 && Buffer.from(await gc.arrayBuffer()).equals(committed), gc.status);
  const gm = await get("/view/git", { thread: th, path: hp(`${project}/.git`, sha, "nothere.kicad_pcb") });
  check("a file the commit lacks is 404", gm.status === 404, gm.status);

  // refusals
  const status = async (path: string, o: Record<string, string>, init: RequestInit = {}) => (await get(path, o, init)).status;
  check("../ is refused", (await status("/view/get", { thread: th, path: "../x" })) === 403 && (await status("/view/get", { thread: th, path: "hw/../../x" })) === 403);
  check("an absolute path is refused", (await status("/view/get", { thread: th, path: "/etc/passwd" })) === 403);
  check("a hidden file is refused", (await status("/view/get", { thread: th, path: ".env" })) === 403 && (await status("/view/get", { thread: th, path: ".git/config" })) === 403);
  check("a symlink out is not sent", (await status("/view/get", { thread: th, path: "leak.kicad_pcb" })) === 404 && (await status("/view/get", { thread: th, path: "hw/etclink/passwd" })) === 404);
  check("a list naming a hidden file or ../ is refused", (await status("/view/ver", { thread: th, want: "a\n.env" })) === 403 && (await status("/view/ver", { thread: th, want: "../x" })) === 403);
  check("a thread the hub does not have is refused", (await status("/view/get", { thread: "nope", path: "ecc83-pp.kicad_pcb" })) === 403 && (await status("/view/ver", { thread: "", want: "" })) === 403);
  check("a thread of another machine this hub does not mirror is refused", (await status("/view/get", { thread: "x~y", path: "a.kicad_pcb" })) === 403);
  check("a commit that is an option is refused", (await status("/view/git", { thread: th, path: hp(`${project}/.git`, "--output=/tmp/bp-x", "ecc83-pp.kicad_pcb") })) === 403);
  check("a git directory that is no history is refused", (await status("/view/git", { thread: th, path: hp("/etc", sha, "ecc83-pp.kicad_pcb") })) === 403);
  check("a path that climbs in a commit is refused", (await status("/view/git", { thread: th, path: hp(`${project}/.git`, sha, "../../etc/passwd") })) === 403);
  check("a live file is not a version", (await status("/view/git", { thread: th, path: "ecc83-pp.kicad_pcb" })) === 403);
  check("an unknown request is nothing", (await status("/view/other", { thread: th })) !== 200);

  // a client the hub does not let in: a request that came through Tailscale
  // (its identity header) carries the pairing token or is refused
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const via = { headers: { "Tailscale-User-Login": "someone@example.com" } };
  check("no token through the tailnet: 401 on every request", (await status("/view/ver", { thread: th, want: "" }, via)) === 401 && (await status("/view/get", { thread: th, path: "ecc83-pp.kicad_pcb" }, via)) === 401 && (await status("/view/git", { thread: th, path: hp(`${project}/.git`, sha, "ecc83-pp.kicad_pcb") }, via)) === 401);
  check("a wrong token: 401", (await status("/view/get", { thread: th, path: "ecc83-pp.kicad_pcb", token: "0".repeat(token.length) }, via)) === 401);
  check("the token lets it in", (await status("/view/get", { thread: th, path: "ecc83-pp.kicad_pcb", token }, via)) === 200);
  // the window's own code (src/app/rvw.bend) against this hub
  if (existsSync(sync)) {
    const cacheDir = join(root, "xdg");
    const run = async (have: string, gitp = "") => {
      const p = Bun.spawn([resolve(sync), base, th, cacheDir, have, gitp], { stdout: "pipe", stderr: "pipe", env: { ...process.env, PATH: "/usr/bin:/bin" } });
      const text = await new Response(p.stdout).text();
      await p.exited;
      const m = /round ok=(\w+) ver=(\S*) reread=(\w+) why=(.*)/.exec(text);
      return { ok: m?.[1] === "True", ver: m?.[2] ?? "", reread: m?.[3] === "True", why: m?.[4] ?? text, text };
    };
    const copy = join(cacheDir, "remote", "h" + base.replace(/[^A-Za-z0-9.-]/g, "_"), dirT);
    const t3 = Date.now();
    const r1 = await run("");
    console.log(`  window round 1 ${Date.now() - t3} ms`);
    check("the window's first round fills its cache and says to read", r1.ok && r1.reread && r1.ver.length > 0, r1);
    const sameFile = (rel: string) => existsSync(join(copy, rel)) && readFileSync(join(copy, rel)).equals(readFileSync(own(rel)));
    check("the copies are the hub's files byte for byte", sameFile("ecc83-pp.kicad_pcb") && sameFile("ecc83-pp.kicad_sch") && sameFile("ecc83-pp.kicad_pro") && sameFile("hw/second.kicad_pcb"), readdirSync(copy));
    check("nothing hidden came", !existsSync(join(copy, ".env")) && !existsSync(join(copy, "hw", "etclink")) && !existsSync(join(copy, "leak.kicad_pcb")), readdirSync(copy));
    const t4 = Date.now();
    const pending = run(r1.ver);
    await sleep(1500);
    writeFileSync(own("ecc83-pp.kicad_sch"), readFileSync(own("ecc83-pp.kicad_sch"), "utf8") + "\n; changed\n");
    const r2 = await pending;
    console.log(`  window round 2 (change at 1500 ms) answered ${Date.now() - t4} ms after it began`);
    check("the next round waits, then brings the change and says to read", r2.ok && r2.reread && r2.ver !== r1.ver && sameFile("ecc83-pp.kicad_sch"), r2);
    const t5 = Date.now();
    const r3 = await run(r2.ver, hp(`${project}/.git`, sha, "ecc83-pp.kicad_pcb"));
    console.log(`  window round 3 (nothing changed) answered after ${Date.now() - t5} ms`);
    check("a round nothing changed in says so: same digest, no read", r3.ok && !r3.reread && r3.ver === r2.ver, r3);
    check("a version of a commit comes through the hub", r3.text.includes(`git ${committed.length}`), r3.text);
  } else {
    console.log("  (no build/rvw_sync: the window's side is not run)");
  }
  ws.close();
} catch (e) {
  console.log(`FAIL error: ${e}`);
  failed = true;
} finally {
  if (proc) proc.kill();
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "FAILED" : "all ok");
process.exit(failed ? 1 : 0);
