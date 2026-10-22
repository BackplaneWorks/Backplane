// store.db after a hub's own commits, against the fold. A JS hub on an
// empty home (build/store_commit_e2e, E2E_ROOT to move it) is driven over
// its WebSocket: a project, two threads, a message (a turn that holds), a
// queued message behind it, a setting (one whose value looks like an
// integer, "#42", too), a finished turn, a rename, an archive. The hub is
// stopped and HOME/store.db read with bun:sqlite:
//   - records: every row of project, thread, task, ask, queue, setting,
//     bot, room, routine, conv, mark and obj, columns in Sdb.cols order
//     (src/core/storedb.bend; an integer reads "#<n>", as the pure store
//     keeps it), equals test/native/store_dump.bend's lines for the home
//     (the log folded, SDB.Tab.dump)
//   - facts: count(line) is the number of log lines that parse, count(entry)
//     the number of MessagePosted, ActivityLogged and Reminded lines
//
//   bun test/tools/store_commit_e2e.ts HUB WIREDIR DUMP     (port 3831, E2E_PORT)
// HUB: the server as JS (bend src/server/main.bend -o hub.js, or
// scripts/build-js.sh's build/js/backplane-serve/stamped.js) or the single
// file scripts/build-js.sh makes. WIREDIR: bend test/wire/index.html -o
// build/wire. DUMP: store_dump emitted as JS (bend test/native/store_dump.bend
// -o build/store_dump.js, run with bun), the .bend itself (run with bend),
// or a binary. KEEP=1 keeps the root.
import { Database } from "bun:sqlite";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [hub, wire, dump] = process.argv.slice(2);
if (!hub || !wire || !dump) { console.log("usage: bun test/tools/store_commit_e2e.ts HUB WIREDIR DUMP"); process.exit(2); }
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3831);
if (PORT === 3787 || PORT < 3799) throw new Error(`refusing port ${PORT}`);

let fail = 0;
const ok = (name: string) => console.log(`ok ${name}`);
const bad = (name: string, got: unknown, want: unknown) => {
  console.log(`FAIL ${name}: got ${typeof got === "string" ? got : JSON.stringify(got)} want ${typeof want === "string" ? want : JSON.stringify(want)}`);
  fail++;
};
const check = (cond: boolean, name: string, got: unknown = false, want: unknown = true) => (cond ? ok(name) : bad(name, got, want));
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const run = (cmd: string[], cwd?: string) => Bun.spawnSync(cmd, { cwd, stdout: "pipe", stderr: "pipe" });

try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) { console.log(`port ${PORT} is in use`); process.exit(2); } } catch {}

const root = resolve(process.env.E2E_ROOT ?? "build/store_commit_e2e");
if (root === "/" || !root.includes("store_commit_e2e")) throw new Error(`refusing root ${root}`);
rmSync(root, { recursive: true, force: true });
const fake = join(root, "bin"), log = join(root, "claude.log"), home = join(root, "home"), proj = join(root, "proj"), web = join(root, "web");
for (const d of [fake, home, proj, web, join(root, "locks"), join(root, "tmp")]) mkdirSync(d, { recursive: true });

const exe = join(root, "backplane");
if (hub.endsWith(".js")) {
  const b = run([process.execPath, "build", "--compile", resolve(hub), "--outfile", exe]);
  if (b.exitCode !== 0) { console.log(String(b.stderr)); process.exit(1); }
} else copyFileSync(resolve(hub), exe);
chmodSync(exe, 0o755);
writeFileSync(join(web, "index.html"), "<!doctype html><title>store-commit-e2e</title>");
// the stand-ins (BACKPLANE_AGENTS): claude is test/tools/claude_standin.ts
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(join(proj, "README"), "e2e\n");

const env = {
  ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_TELEMETRY: "off", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "",
  BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "none.lock"),
  BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: join(root, "tmp"),
};
delete (env as any).BACKPLANE_HOME;
const proc = Bun.spawn([exe, "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground", "--web", web],
  { env, stdout: Bun.file(join(root, "hub.out")), stderr: Bun.file(join(root, "hub.err")) });
const hello = async () => { try { const r = await fetch(`http://127.0.0.1:${PORT}/hello`); return r.status === 200 ? true : undefined; } catch { return undefined; } };
// stop the hub we started (its PID only), then the stand-ins it ran (their
// PIDs from the stand-in's own log)
const stop = async () => {
  if (proc.exitCode === null) { try { process.kill(proc.pid, "SIGTERM"); } catch {} }
  const gone = await Promise.race([proc.exited.then(() => true), sleep(10000).then(() => false)]);
  if (!gone) { try { process.kill(proc.pid, "SIGKILL"); } catch {} await proc.exited; }
  for (const m of read(log).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
  return gone;
};

let drove = false;
try {
  check(!!(await until(30000, hello)), "hub answers /hello");
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const wait = (what: string, f: (x: any) => boolean, ms = 15000) => until(ms, () => seen.find(f)).then((x) => { check(!!x, what); return x; });

  send("project.add", { path: proj });
  const pc: any = await wait("a project is added", (x) => x.$ === "ProjectCreated" && x.root === proj);
  const made = async (title: string) => {
    send("thread.create", { project: pc?.id, title, env: "local", provider: "claude" });
    return (await wait(`thread ${title} is created`, (x) => x.$ === "ThreadCreated" && x.title === title))?.id as string;
  };
  const A = await made("alpha");
  const B = await made("beta");
  // a turn that holds (the stand-in works until interrupted), and a message
  // queued behind it
  send("turn.start", { thread: A, text: "mode:stop hold on", msg: "a-1" });
  await wait("a message runs a turn", (x) => x.$ === "TurnChanged" && x.thread === A && x.state === "running", 20000);
  send("turn.start", { thread: A, text: "later, please", msg: "a-2" });
  await wait("a message is queued", (x) => x.$ === "TurnQueued" && x.thread === A);
  send("setting.set", { key: "e2e.store", value: "one\ttwo\nthree" });
  await wait("a setting is set", (x) => x.$ === "SettingSet" && x.key === "e2e.store");
  send("setting.set", { key: "e2e.hash", value: "#42" });
  await wait("a setting that looks like an integer is set", (x) => x.$ === "SettingSet" && x.key === "e2e.hash");
  // a turn that ends, so B has a reply
  send("turn.start", { thread: B, text: "mode:plain hello", msg: "b-1" });
  await wait("B's turn ends", (x) => x.$ === "TurnChanged" && x.thread === B && x.state === "completed", 20000);
  send("thread.rename", { thread: B, title: "beta renamed" });
  await wait("a thread is renamed", (x) => x.$ === "ThreadRenamed" && x.id === B);
  send("thread.archive", { thread: B });
  await wait("a thread is archived", (x) => /Archiv/.test(String(x.$)) && (x.id === B || x.thread === B));
  await sleep(1500);
  ws.close();
  drove = true;
} finally {
  const gone = await stop();
  check(gone, "the hub stops on SIGTERM");
}

// The fold's records, from store_dump
const dumpCmd = dump.endsWith(".js") ? [process.execPath, resolve(dump), home] : dump.endsWith(".bend") ? ["bend", resolve(dump), home] : [resolve(dump), home];
const d = Bun.spawnSync(dumpCmd, { stdout: "pipe", stderr: "pipe", env: { ...process.env, BEND_NO_TELEMETRY: "1" }, timeout: 600000 });
check(d.exitCode === 0, "store_dump runs", `exit ${d.exitCode}: ${String(d.stderr).trim().slice(-300)}`, "exit 0");
const want = new Map<string, Map<string, string>>(); // table -> key -> cols
for (const l of String(d.stdout).split("\n")) {
  if (!l) continue;
  const [t, k, v] = l.split("\t");
  if (!want.has(t)) want.set(t, new Map());
  want.get(t)!.set(JSON.parse(k), JSON.parse(v));
}
console.log(`     store_dump: ${String(d.stderr).trim()}`);

// Sdb.cols (src/core/storedb.bend), key columns first (Tab.keyn)
const COLS: Record<string, string[]> = {
  project: ["id", "title", "root", "created", "removed", "json"],
  thread: ["id", "project", "title", "created", "activity", "turn", "pending", "archived", "pinned", "settled_at", "provider", "model", "session", "deleted", "json"],
  task: ["id", "parent", "child", "state", "delivered", "json"],
  ask: ["id", "thread", "open", "json"],
  queue: ["thread", "n", "json"],
  setting: ["k", "v", "cold"],
  bot: ["id", "name", "thread", "deleted", "json"],
  room: ["id", "name", "deleted", "json"],
  routine: ["id", "bot", "on_", "deleted", "json"],
  conv: ["id", "kind", "project", "last", "want", "parts", "json"],
  mark: ["k", "bot", "conv", "status", "json"],
  obj: ["domain", "key", "json"],
};
const keyn = (t: string) => (t === "obj" ? 2 : 1);
const SEP = "\u001f";
const cell = (v: unknown) => (v === null ? "<NULL>" : typeof v === "number" || typeof v === "bigint" ? `#${v}` : v instanceof Uint8Array ? `<blob ${v.length}>` : String(v));

const dbPath = join(home, "store.db");
const logText = read(join(home, "events.jsonl"));
if (!existsSync(dbPath)) bad("store.db exists", "no store.db (files: " + readdirSync(home).join(" ") + ")", dbPath);
else {
  ok("store.db exists");
  const db = new Database(dbPath, { safeIntegers: true });
  for (const [t, cols] of Object.entries(COLS)) {
    const info = db.query(`PRAGMA table_info(${t})`).all() as { name: string }[];
    const names = info.map((c) => c.name);
    if (names.join(",") !== cols.join(",")) { bad(`columns ${t}`, names, cols); continue; }
    const got = new Map<string, string>();
    for (const r of db.query(`SELECT ${cols.join(", ")} FROM ${t}`).values() as unknown[][]) {
      const cs = r.map(cell);
      got.set(cs.slice(0, keyn(t)).join(SEP), cs.join(SEP));
    }
    const w = want.get(t) ?? new Map<string, string>();
    const diffs: string[] = [];
    for (const [k, v] of w) if (got.get(k) !== v) diffs.push(k);
    for (const k of got.keys()) if (!w.has(k)) diffs.push(k);
    if (!diffs.length) ok(`records ${t} (${got.size} rows)`);
    for (const k of diffs.slice(0, 20)) bad(`records ${t} ${JSON.stringify(k)}`, got.get(k) ?? "<no row>", w.get(k) ?? "<no row>");
    if (diffs.length > 20) console.log(`     ... ${diffs.length - 20} more in ${t}`);
  }
  for (const t of want.keys()) if (!(t in COLS)) bad(`records: the dump's table ${t} is checked`, "unchecked", Object.keys(COLS));

  // facts
  let parsed = 0, entries = 0;
  for (const l of logText.split("\n").slice(0, -1)) {
    let o: any;
    try { o = JSON.parse(l); } catch { continue; }
    parsed++;
    if (o && (o.$ === "MessagePosted" || o.$ === "ActivityLogged" || o.$ === "Reminded")) entries++;
  }
  const n = (q: string) => Number((db.query(q).values()[0] as unknown[])[0]);
  const lines = n("SELECT count(*) FROM line"), ents = n("SELECT count(*) FROM entry");
  check(lines === parsed, "facts count(line) is the log's parsed lines", lines, parsed);
  check(ents === entries, "facts count(entry) is MessagePosted+ActivityLogged+Reminded", ents, entries);
  db.close();
}
check(drove, "the hub was driven to the end");
if (fail || process.env.KEEP) console.log(`kept ${root} (hub.err, home/events.jsonl, home/store.db there)`);
else rmSync(root, { recursive: true, force: true });
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
