// Perf load: the native app (hub + window) on a copy of the real event log,
// with stand-in agents streaming, driven through phases; each phase is
// measured from the app's own trace (BACKPLANE_TRACE: every span, exact) and
// its /debug/perf.json readings, and checked against the targets.
//
//   bun test/tools/perf_load.ts [--bin build/backplane] [--log ~/.backplane-bend/events.jsonl]
//     [--port 3799] [--display :77] [--xvfb PATH] [--agents 10] [--secs 20]
//     [--phases normal,idle,bg,fg,type,open,plane,mail] [--out results.json] [--keep] [--shots DIR] [--hold]
//
// See docs/perf-bench.md. Nothing here touches the live home: the log is
// copied (devices, hooks, peers, routines, push and deploy settings stripped,
// the live home's path rewritten to the copy's, restart.continue off), the
// app runs with HOME, its lock dir and deploy locks in a temp folder, a
// stand-in `claude` first on PATH, and only the PIDs started here are
// stopped.
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync, openSync, readSync, closeSync, statSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { standinScript as claude } from "./perf_standin";
import { completeOpens, FixtureBots, streamWindow } from "./perf_guard";
import { basename, dirname, join, resolve } from "node:path";
import { performanceVerdict } from "./perf_gate";
import { candidateBinding, fileSha } from "../../scripts/perf-verify";
import { verifyPerformanceBundle, type PerformanceBundle } from "./perf_evidence";

const argv = process.argv.slice(2);
if (argv.includes("--help")) {
  console.log("bun test/tools/perf_load.ts --bin BUILD --log SNAPSHOT [--agents 10] [--secs 20] [--phases normal,idle,bg,fg,type,open,plane,mail] [--out results.json] [--shots DIR] [--diagnostic] [--keep]");
  process.exit(0);
}
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  if (i < 0) return d;
  if (!argv[i + 1] || argv[i + 1].startsWith("--")) throw new Error(`--${name} needs a value`);
  return argv[i + 1];
};
const here = resolve(import.meta.dir, "../..");
const bin = resolve(opt("bin", join(here, "build/backplane")));
const liveHome = join(homedir(), ".backplane-bend");
const mailReuseHome = opt("mail-reuse-home", "");
if (mailReuseHome && (!resolve(mailReuseHome).startsWith(join(tmpdir(), "bp-perfload-")) || !existsSync(join(mailReuseHome, "mail")))) throw new Error("--mail-reuse-home requires a retained isolated perf-load home with mail bodies");
const logSrc = resolve(opt("log", mailReuseHome ? join(mailReuseHome, "events.jsonl") : join(liveHome, "events.jsonl")));
const port = Number(opt("port", "3799"));
const display = opt("display", ":77");
const nAgents = Number(opt("agents", "10"));
const secs = Number(opt("secs", "20"));
const outPath = opt("out", "");
const receiptPath=opt("build-receipt", "");
const evidencePath=opt("evidence", "");
const keep = argv.includes("--keep");
const diagnostic = argv.includes("--diagnostic");
if (diagnostic && argv.includes("--fail-on-target")) throw new Error("--diagnostic conflicts with --fail-on-target");
const shots = opt("shots", ""); // a folder: a screenshot at the end of each phase (ImageMagick's import)
const phasesWanted = opt("phases", "normal,idle,bg,fg,type,open,scroll,plane").split(",");
const xpoke = resolve(opt("xpoke", join(here, "build/xpoke")));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const say = (s: string) => console.error(`[perf_load] ${s}`);

// Safety first
// ------------
if(!diagnostic && (!receiptPath || !evidencePath || !keep)) throw new Error("strict acceptance requires --build-receipt, --evidence and --keep");
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error("--port must be 1024..65535");
if (!Number.isInteger(nAgents) || nAgents < 1 || nAgents > 26) throw new Error("--agents must be 1..26");
if (!Number.isFinite(secs) || secs <= 0) throw new Error("--secs must be positive");
const knownPhases = ["normal", "idle", "bg", "fg", "type", "open", "scroll", "plane", "mail"];
if (phasesWanted.some((p) => !knownPhases.includes(p))) throw new Error("unknown --phases value");
if (port === 3787) throw new Error("3787 is the live hub's port");
if (!existsSync(bin)) throw new Error(`no binary at ${bin}`);
if (realpathSync(bin).startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("refusing to run the installed binary");
if (!existsSync(xpoke)) throw new Error(`no xpoke at ${xpoke} (cc -O2 test/tools/xpoke.c -o build/xpoke -lX11)`);
try {
  await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) });
  throw new Error(`something already answers on port ${port}`);
} catch (e: any) {
  if (String(e.message).startsWith("something")) throw e;
}

const root = mkdtempSync(join(tmpdir(), "bp-perfload-"));
const home = join(root, "home");
const user = join(root, "user");
const fake = join(root, "bin");
const locks = join(root, "locks");
const proj = join(root, mailReuseHome ? "ui-replay-project" : "proj");
for (const d of [home, user, fake, locks, proj]) mkdirSync(d, { recursive: true });
if (shots) mkdirSync(shots, { recursive: true });
const trace = join(root, "trace.json");
const fakeLog = join(root, "agents.log");

// The log, copied and sanitized
// -----------------------------
// Dropped: devices and push, hooks, peers, wakes and routines (they would
// start turns), locks (grants start turns), watched work (it runs commands
// again). Delegated tasks still open are closed at the end (a restart
// cancels them and wakes their parents). Every path under the user's home
// is rewritten into the temp folder, so no thread's folder is a real one;
// git is a stub too, so no repository is ever written.
const dropTypes = new Set(["DeviceRegistered", "DeviceDropped", "HookSet", "PeerSet", "WakeSet", "RoutineSet", "RoutinePlan", "LockChanged"]);
const elsewhere = join(root, "elsewhere");
const dropSetting = /^(push|voice|deploy|google|hook|apns|token|secret)/i;
type Th = { id: string; project: string; title: string; msgs: number; gone: boolean };
const threads = new Map<string, Th>();
const botThreads = new Set<string>();
const fixtureBots = new FixtureBots();
const botById = fixtureBots.threads;
const turns = new Map<string, any>();
const removedProjects = new Set<string>();
const roomById = new Map<string, { id: string; name: string; members: string }>();
const kept: string[] = [];
let dropped = 0;
const openTasks = new Set<string>();
const botIds = new Set<string>();
const mailFixtureByName = new Map<string, { id: string; thread: string }>();
const raw = readFileSync(logSrc, "utf8");
const boundAtStart=receiptPath?candidateBinding(here,bin,logSrc,receiptPath,"linux-native"):null;
const buildReceipt=receiptPath?JSON.parse(readFileSync(receiptPath,"utf8")):null;
const logSha256 = new Bun.CryptoHasher("sha256").update(raw).digest("hex");
for (const line of raw.split("\n")) {
  if (!line) continue;
  const ty = /^\{"\$":"([A-Za-z]+)"/.exec(line)?.[1] ?? "";
  if (dropTypes.has(ty) || /devicetoken|webhook|apns/i.test(line)) { dropped++; continue; }
  if (ty === "WorkSet" && !line.includes('"kind":"bg"')) { dropped++; continue; }
  if (ty === "TurnChanged") { const o = JSON.parse(line); turns.set(o.thread, o.state); }
  if (ty === "TaskDelegated") openTasks.add(/"task":"([^"]*)"/.exec(line)?.[1] ?? /"id":"([^"]*)"/.exec(line)?.[1] ?? "");
  if (ty === "TaskDelivered") openTasks.delete(/"task":"([^"]*)"/.exec(line)?.[1] ?? "");
  if (["BotSet", "BotDeleted", "UberSet", "SettingSet"].includes(ty)) fixtureBots.apply(JSON.parse(line));
  if (ty === "SettingSet") {
    const k = /"key":"([^"]*)"/.exec(line)?.[1] ?? "";
    if (dropSetting.test(k)) { dropped++; continue; }
  }
  const localLine = mailReuseHome ? line.split(resolve(mailReuseHome)).join(home).split(dirname(resolve(mailReuseHome))).join(root) : line;
  kept.push(localLine.split(liveHome).join(home).split(homedir() + "/").join(elsewhere + "/"));
  if (ty === "ThreadCreated" || ty === "MessagePosted" || ty === "ThreadDeleted" || ty === "ThreadArchived" || ty === "BotSet" || ty === "BotDeleted" || ty === "ProjectRemoved" || ty === "RoomSet" || ty === "RoomDeleted" || ty === "ThreadRenamed") {
    let o: any;
    try { o = JSON.parse(line); } catch { continue; }
    if (ty === "ThreadCreated") threads.set(o.id, { id: o.id, project: o.project, title: o.title, msgs: 0, gone: false });
    else if (ty === "MessagePosted") { const t = threads.get(o.thread); if (t) t.msgs++; }
    else if (ty === "ThreadDeleted" || ty === "ThreadArchived") { const t = threads.get(o.id ?? o.thread); if (t) t.gone = true; }
    else if (ty === "ThreadRenamed") { const t = threads.get(o.id); if (t) t.title = o.title; }
    else if (ty === "BotSet") { if (o.name?.startsWith("Mailperf")) mailFixtureByName.set(o.name, { id: o.id, thread: o.thread }); if (o.thread) botThreads.add(o.thread); botIds.add(o.id); }
    else if (ty === "ProjectRemoved") removedProjects.add(o.id);
    else if (ty === "RoomSet") roomById.set(o.id, { id: o.id, name: o.name, members: o.members ?? "" });
    else if (ty === "RoomDeleted") roomById.delete(o.id);
  }
}
const now = Math.floor(Date.now() / 1000);
const rooms = [...roomById.values()];
// real threads, longest first: the long one the window shows, and the ones phase "open" opens
const real = [...threads.values()].filter((t) => !t.gone && !removedProjects.has(t.project) && !botThreads.has(t.id)).sort((a, b) => b.msgs - a.msgs);
const longT = real[0];
if (!longT) throw new Error("--log has no available non-bot thread for the load scenario");
const openTs = real.slice(1, 11);
if (phasesWanted.includes("open") && openTs.length !== 10) throw new Error("--log needs eleven available non-bot threads for ten verified opens");
// A reused fixture already contains the prior driver's load-thread titles.
// Rename those only in the new test copy so palette selection is unambiguous.
if (mailReuseHome) for (const t of threads.values()) {
  if (/^zqrun[a-z] stream$/.test(t.title)) kept.push(JSON.stringify({ $: "ThreadRenamed", id: t.id, title: `retained load ${t.id}`, at: now }));
}
// unique titles the palette (ctrl+k) finds them by
kept.push(JSON.stringify({ $: "ThreadRenamed", id: longT.id, title: "zqlong thread", at: now }));
openTs.forEach((t, i) => kept.push(JSON.stringify({ $: "ThreadRenamed", id: t.id, title: `zqopen${String.fromCharCode(97 + i)} thread`, at: now })));
// A synthetic log need not have completed onboarding. The load harness
// drives the product, not the modal first-run tour.
kept.push(JSON.stringify({ $: "SettingSet", key: "onboard.done", value: "1", at: now }));
for (const k of openTasks) if (k) {
  kept.push(JSON.stringify({ $: "TaskStateSet", task: k, state: "cancelled", summary: "", at: now }));
  kept.push(JSON.stringify({ $: "TaskDelivered", task: k, at: now }));
}
kept.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: now }));
// The person's preferred provider may change between captures. The load
// protocol always comes from the stand-in, including copied bot threads.
kept.push(JSON.stringify({ $: "SettingSet", key: "agent.provider", value: "claude", at: now }));
for (const t of threads.values()) {
  if (t.gone) continue;
  kept.push(JSON.stringify({ $: "ProviderSet", id: t.id, provider: "claude", model: "", at: now }));
  if (turns.get(t.id) === "running") kept.push(JSON.stringify({ $: "TurnChanged", thread: t.id, state: "interrupted", at: now }));
}
writeFileSync(join(home, "events.jsonl"), kept.join("\n") + "\n");
if (mailReuseHome) cpSync(join(mailReuseHome, "mail"), join(home, "mail"), { recursive: true });
// a bot's turns run in its home folder (<home>/bots/<id>): there, empty
for (const b of botIds) mkdirSync(join(home, "bots", b), { recursive: true });
const since = kept.length;
say(`log: ${kept.length} lines kept, ${dropped} dropped; long thread ${longT.id} (${longT.msgs} messages); ${rooms.length} rooms, ${botThreads.size} bots`);

// The stand-in agent
// ------------------
// Claude's stream-json. On each user line: init, then every 100 ms FAKE_PER
// text deltas, every FAKE_CYCLE ticks the finished text, a Bash tool call and
// its result. A message holding "zqsub" also starts a subagent (an Agent call
// with task_started, its own lines under parent_tool_use_id) and a
// background shell, with a task_progress for each every tick. "zqshort"
// streams 8 s (bots woken by room posts). It ends when its hub is gone.
writeFileSync(join(fake, "claude"), claude);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "agy", "tailscale", "notify-send", "gh", "git", "kicad-cli", "freecad", "xdg-open", "gio"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
writeFileSync(fakeLog, "");

// CBOR, the hub's wire (src/core/cbor.bend)
// ------------------------------------------
const cborSrc = readFileSync(join(here, "src/core/cbor.bend"), "utf8");
const dict = (name: string) => JSON.parse(new RegExp(`def Cbor\\.${name}\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])\\n`).exec(cborSrc)![1]) as string[];
const KEYS = dict("keys"), WORDS = dict("words");
const te = new TextEncoder(), td = new TextDecoder();
function cborEnc(v: any, out: number[]) {
  const head = (mb: number, n: number) => {
    if (n < 24) out.push(mb | n);
    else if (n < 256) out.push(mb | 24, n);
    else if (n < 65536) out.push(mb | 25, n >> 8, n & 255);
    else out.push(mb | 26, (n >>> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255);
  };
  const str = (s: string) => { const b = te.encode(s); head(0x60, b.length); for (const x of b) out.push(x); };
  if (v === null || v === undefined) out.push(0xf6);
  else if (v === true) out.push(0xf5);
  else if (v === false) out.push(0xf4);
  else if (typeof v === "number") {
    if (Number.isInteger(v) && v >= 0 && v < 2 ** 32) head(0x00, v);
    else if (Number.isInteger(v) && v < 0 && -1 - v < 2 ** 32) head(0x20, -1 - v);
    else { out.push(0xc7); str(String(v)); }
  } else if (typeof v === "string") str(v);
  else if (Array.isArray(v)) { head(0x80, v.length); for (const x of v) cborEnc(x, out); }
  else {
    const ks = Object.keys(v);
    head(0xa0, ks.length);
    for (const k of ks) { const i = KEYS.indexOf(k); if (i >= 0) head(0x00, i); else str(k); cborEnc(v[k], out); }
  }
}
function cborDec(b: Uint8Array): any {
  let p = 0;
  const arg = (ai: number) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[p++];
    if (ai === 25) { const v = (b[p] << 8) | b[p + 1]; p += 2; return v; }
    if (ai === 26) { const v = ((b[p] << 24) >>> 0) + (b[p + 1] << 16) + (b[p + 2] << 8) + b[p + 3]; p += 4; return v; }
    throw new Error("cbor arg");
  };
  const item = (key: boolean): any => {
    const ib = b[p++], mt = ib >> 5, ai = ib & 31;
    if (mt === 7) return ai === 21 ? true : ai === 20 ? false : null;
    if (mt === 6) {
      const tag = arg(ai);
      if (tag === 6) { const i = item(false); return WORDS[i]; }
      const t = item(false);
      return Number(t);
    }
    const n = arg(ai);
    if (mt === 0) return key ? KEYS[n] : n;
    if (mt === 1) return -1 - n;
    if (mt === 2 || mt === 3) { const s = td.decode(b.subarray(p, p + n)); p += n; return s; }
    if (mt === 4) { const a = []; for (let i = 0; i < n; i++) a.push(item(false)); return a; }
    const o: any = {};
    for (let i = 0; i < n; i++) { const k = item(true); o[k] = item(false); }
    return o;
  };
  return item(false);
}

// The app
// -------
const ownPids: { pid: number; what: string; kill: () => void; exited: Promise<any> }[] = [];
let xvfbPid = 0;
const xsock = `/tmp/.X11-unix/X${display.replace(/^:/, "").split(".")[0]}`;
if (argv.includes("--exclusive-display") && existsSync(xsock)) throw new Error(`exclusive X display ${display} already exists; refusing to share another job's X server`);
if (!existsSync(xsock)) {
  const xv = opt("xvfb", ["/tmp/xvfb-root/usr/bin/Xvfb", "/usr/bin/Xvfb", join(homedir(), ".local/bin/Xvfb")].find((p) => existsSync(p)) ?? "");
  if (!xv) throw new Error("no Xvfb (see docs/perf-bench.md)");
  const x = Bun.spawn([xv, display, "-screen", "0", "1600x1000x24", "-nolisten", "tcp"], { stdout: "ignore", stderr: Bun.file(join(root, "xvfb.log")) });
  xvfbPid = x.pid;
  ownPids.push({ pid: x.pid, what: "Xvfb", kill: () => x.kill(), exited: x.exited });
  for (let i = 0; i < 50 && !existsSync(xsock); i++) await sleep(100);
  say(`Xvfb ${display} pid ${x.pid}`);
} else say(`using the X server already on ${display}`);

const env: Record<string, string> = {
  BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/local/bin:/usr/bin:/bin`,
  HOME: user, USER: process.env.USER ?? "h", LANG: process.env.LANG ?? "C.UTF-8",
  DISPLAY: display, WAYLAND_DISPLAY: "", DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
  BACKPLANE_TRACE: trace, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_LOCK_DIR: locks,
  BACKPLANE_DEPLOY_LOCKS: `${locks}/bp-bend.lock:${locks}/bp-wt-build.lock`, BACKPLANE_APNS_URL: "http://127.0.0.1:9",
  BACKPLANE_HOOK_URL: "", BACKPLANE_GOOGLE_API: "http://127.0.0.1:9", BACKPLANE_GOOGLE_OAUTH: "http://127.0.0.1:9",
  FAKE_LOG: fakeLog, FAKE_SECS: "900", FAKE_PER: "3", FAKE_CYCLE: "30",
};
const t0Wall = Date.now();
const app = Bun.spawn([bin, "--home", home, "--port", String(port), "--no-tailscale"], {
  env, cwd: root, stdout: Bun.file(join(root, "app.log")), stderr: Bun.file(join(root, "app.err")),
});
const observedExeSha=fileSha(`/proc/${app.pid}/exe`);
if(boundAtStart && observedExeSha!==boundAtStart.binary_sha256) { app.kill(); await app.exited; throw new Error("running executable differs from candidate binary"); }
ownPids.push({ pid: app.pid, what: "backplane", kill: () => app.kill(), exited: app.exited });
say(`app pid ${app.pid}, home ${home}`);

async function cleanup() {
  for (const p of ownPids.reverse()) {
    try { p.kill(); } catch {}
    await Promise.race([p.exited, sleep(5000)]);
  }
}
process.on("SIGINT", async () => { await cleanup(); process.exit(130); });
const bail = async (e: any) => { console.error(e); await cleanup(); process.exit(1); };
process.on("uncaughtException", bail);
process.on("unhandledRejection", bail);

const pj = async (path: string) => (await fetch(`http://127.0.0.1:${port}${path}`)).json();
let ready = false;
for (let i = 0; i < 1800; i++) {
  if (app.exitCode !== null) break;
  try { if ((await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(2000) })).status === 200) { ready = true; break; } } catch {}
  await sleep(100);
}
if (!ready) { await cleanup(); throw new Error(`app did not start; see ${root}/app.err`); }
say(`hub up after ${((Date.now() - t0Wall) / 1000).toFixed(1)} s`);

// no real agent may run: every child named claude/codex/grok/muse must be ours
function children(pid: number): number[] {
  try { return readFileSync(`/proc/${pid}/task/${pid}/children`, "utf8").trim().split(/\s+/).filter(Boolean).map(Number); } catch { return []; }
}
function strayAgents(): string[] {
  const bad: string[] = [];
  const walk = (pid: number, depth: number) => {
    for (const c of children(pid)) {
      let cmd = "";
      try { cmd = readFileSync(`/proc/${c}/cmdline`, "utf8").replace(/\0/g, " "); } catch {}
      if (/\b(claude|codex|grok|muse|agy)\b/.test(cmd) && !cmd.includes(fake)) bad.push(`${c}: ${cmd.slice(0, 120)}`);
      if (depth < 3) walk(c, depth + 1);
    }
  };
  walk(app.pid, 0);
  return bad;
}
await sleep(3000);
{
  const bad = strayAgents();
  if (bad.length) { await cleanup(); throw new Error(`real agents under the test hub:\n${bad.join("\n")}`); }
  const started = readFileSync(fakeLog, "utf8").trim();
  if (started) say(`stand-ins started by the log itself (resumed turns?):\n${started}`);
}

// The hub's socket (a client like the web's, joined at the log's head)
// ---------------------------------------------------------------------
const items: any[] = [];
const deltas = new Map<string, number>();
const deltaTimes = new Map<string, number[]>();
let deskThreads = new Set<string>();
const openEvidence: { title: string; thread: string; verified: boolean }[] = [];
const traffic = (): Record<string, number> => Object.fromEntries(deltas);
const replies = new Map<number, any>();
const replyWaiters = new Map<number, (reply: any) => void>();
let wsMsgs = 0, wsBytes = 0;
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?since=${since}`);
ws.binaryType = "arraybuffer";
ws.onmessage = (e) => {
  const u = new Uint8Array(e.data as ArrayBuffer);
  wsMsgs++; wsBytes += u.length;
  let o: any;
  try { o = cborDec(u); } catch { return; }
  if (!o || typeof o !== "object") return;
  if (o.t === "delta" && typeof o.thread === "string" && o.text) {
    deltas.set(o.thread, (deltas.get(o.thread) ?? 0) + 1);
    const times = deltaTimes.get(o.thread) ?? [];
    times.push(performance.now()); deltaTimes.set(o.thread, times);
  }
  if (o.t === "info" && typeof o.info?.desk === "string") deskThreads = new Set(o.info.desk.split("\n").filter(Boolean));
  if (o.t === "reply") {
    const id = Number(o.id);
    replies.set(id, o);
    replyWaiters.get(id)?.(o);
    replyWaiters.delete(id);
  }
  for (const c of Array.isArray(o.items) ? o.items : []) {
    if (typeof c === "string") { try { items.push(JSON.parse(c)); } catch {} } else items.push(c);
  }
};
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let rid = 0;
const send = (m: string, p: object) => {
  const i = ++rid;
  const out: number[] = [];
  cborEnc({ id: i, m, p }, out);
  ws.send(new Uint8Array(out));
  return i;
};
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
};
// Read only the appended suffix. Polling must not split the retained 62 MB
// fixture repeatedly while waiting for a test-created event.
const logStartByte = statSync(join(home, "events.jsonl")).size;
const logged = () => {
  const path = join(home, "events.jsonl");
  const size = statSync(path).size;
  if (size < logStartByte) throw new Error("test event log unexpectedly truncated");
  const fd = openSync(path, "r");
  let text: string;
  try {
    const suffix = Buffer.alloc(size - logStartByte);
    const n = readSync(fd, suffix, 0, suffix.length, logStartByte);
    text = suffix.subarray(0, n).toString("utf8");
  } finally { closeSync(fd); }
  return text.split("\n").filter(Boolean).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
};
send("project.add", { path: proj });
const pc = await until(20000, () => logged().find((c: any) => c.$ === "ProjectCreated" && c.root === proj) ?? logged().find((c: any) => c.$ === "ProjectCreated"));
if (!pc) { await cleanup(); throw new Error("no project"); }
for (let i = 0; i < nAgents; i++) send("thread.create", { project: pc.id, title: `zqrun${String.fromCharCode(97 + i)} stream`, env: "local", provider: "claude" });
const ours = await until(30000, () => {
  const t = logged().filter((c: any) => c.$ === "ThreadCreated" && c.project === pc.id && /^zqrun/.test(c.title ?? ""));
  return t.length >= nAgents ? t : undefined;
});
if (!ours) { await cleanup(); throw new Error("threads not created"); }
// a project made afresh starts with a thread of its own: ours are the zqrun ones
say(`project ${pc.id}, ${ours.length} threads`);

// Mailroom phase. No direct ConvSet/MailPosted fixtures: every seeded
// body goes through the real mail.on filing path, and both list and read
// RPCs must prove the expected data exists before native timing starts.
const mailBots: string[] = [];
const mailConvs = Number(opt("mail-convs", "5000"));
const mailMails = Number(opt("mail-mails", "50000"));
if (!Number.isInteger(mailConvs) || mailConvs < 10 || !Number.isInteger(mailMails) || mailMails < mailConvs) throw new Error("mail fixture needs >=10 conversations and >=1 mail each");
async function rpc(m: string, p: object) {
  const id = send(m, p);
  const r: any = replies.get(id) ?? await new Promise((resolve) => {
    const timer = setTimeout(() => { replyWaiters.delete(id); resolve(undefined); }, 60000);
    replyWaiters.set(id, (reply) => { clearTimeout(timer); resolve(reply); });
  });
  replies.delete(id);
  if (!r || r.ok === false || r.error) throw new Error(`${m} failed: ${JSON.stringify(r)}`);
  return r.result;
}
async function seedMail() {
  if (!phasesWanted.includes("mail")) return;
  await rpc("setting.set", { key: "mail.on", value: "on" });
  for (let i = 0; i < 6; i++) {
    if (!mailReuseHome) await rpc("bots.create", { name: `Mailperf${i}`, provider: "claude", model: "", persona: "", uber: i === 0 });
    const b = mailReuseHome ? mailFixtureByName.get(`Mailperf${i}`) : await until(10000, () => items.find((x) => x.$ === "BotSet" && x.name === `Mailperf${i}`));
    if (!b) throw new Error(`mail bot ${i} not filed`);
    mailBots.push(b.id); botById.set(b.id, b.thread); botThreads.add(b.thread);
  }
  await rpc("bots.uber", { bot: mailBots[0] });
  fixtureBots.uber = mailBots[0];
  for (let i = 0; !mailReuseHome && i < mailMails; i += 32) {
    await Promise.all(Array.from({ length: Math.min(32, mailMails - i) }, (_, k) => {
      const j = i + k, c = j % mailConvs;
      return rpc("mail.send", { to: `bot:${mailBots[c % 6]}`, conv: `perf-mail-${String(c).padStart(5, "0")}`, subject: `${c >= mailConvs - 10 ? "needle" : "Mail"} conversation ${c}`, text: `Filed body ${j}: inspect the board and report the result.`, want: "fyi" });
    }));
    if (i % 3200 === 0) say(`mail filing ${i}/${mailMails}`);
  }
  const pages = await Promise.all(mailBots.map((bot) => rpc("mail.list", { bot, n: 10 })));
  const count = pages.reduce((n, page) => n + Number(page?.total ?? 0), 0);
  if (count < mailConvs) throw new Error(`mail fixture incomplete: ${count}/${mailConvs}`);
  const read = await rpc("mail.read", { conv: `perf-mail-${String(mailConvs - 1).padStart(5, "0")}`, n: 50 });
  if (!read?.mails?.length) throw new Error("mail bodies missing after filing");
  say(`mail fixture ${mailReuseHome ? "reused from retained filing" : "filed"}: ${mailConvs} conversations, ${mailMails} bodies, six bots`);
}

// Window driving
// --------------
const poke = async (...a: string[]) => {
  const p = Bun.spawn([xpoke, ...a], { env: { DISPLAY: display }, stdout: "ignore", stderr: "ignore" });
  if (await p.exited !== 0) throw new Error(`xpoke failed: ${a.join(" ")}`);
};
const W = 1400, H = 900;
const MAINX = 264 + (W - 264) / 2, MIDY = H / 2;
// a thread by its (unique) title: ctrl+k, the title, Return. Answers the
// trace window of the Return alone (the open).
async function openByTitle(title: string): Promise<[number, number]> {
  const expected = title === "zqlong" ? longT.id
    : /^zqopen[a-j]$/.test(title) ? openTs[title.charCodeAt(title.length - 1) - 97]?.id
    : ours.find((t: any) => t.title.startsWith(title))?.id;
  if (!expected) throw new Error(`no expected thread for ${title}`);
  await poke("ctrl", "k");
  await sleep(250);
  await poke("type", title);
  await sleep(600);
  const a = await upMs();
  await poke("key", "Return");
  await sleep(700);
  const b = await upMs();
  // The native client reports its selection through the existing desk.look
  // info path; no test-only writes or new application instrumentation.
  const verified = Boolean(await until(2000, () => deskThreads.has(expected)));
  openEvidence.push({ title, thread: expected, verified });
  if (!verified) throw new Error(`thread open not observed: ${title} (${expected}); desk=${[...deskThreads]}`);
  return [a, b];
}
// the composer has the keyboard after a thread opens (Tour.focus); a click at the bottom makes sure
const composer = async () => { await poke("click", String(MAINX), String(H - 50)); await sleep(150); };

// Readings
// --------
// proc.up_ms is the trace's clock (perf.c's perf_now_us / 1000)
type Reading = { up: number; rows: Map<string, any>; wall: number; cpu: number; rss: number };
const cpuTicks = () => {
  try { const f = readFileSync(`/proc/${app.pid}/stat`, "utf8").split(") ")[1].split(" "); return Number(f[11]) + Number(f[12]); } catch { return NaN; }
};
const rssKb = () => { try { return Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${app.pid}/status`, "utf8"))![1]); } catch { return NaN; } };
async function reading(): Promise<Reading> {
  const j = await pj("/debug/perf.json");
  const rows = new Map<string, any>((j.rows ?? []).map((r: any) => [r.name, r]));
  return { up: Number(rows.get("proc.up_ms")?.last ?? 0), rows, wall: Date.now(), cpu: cpuTicks(), rss: rssKb() };
}
const upMs = async () => (await reading()).up;
let rssMax = 0;
const rssTimer = setInterval(() => { rssMax = Math.max(rssMax, rssKb()); }, 500);

type Phase = { name: string; what: string; a: Reading; b: Reading; rssMax: number; trafficBefore: Record<string, number>; trafficAfter: Record<string, number>; streamStart: number; streamEnd: number; marks?: [number, number][]; notes?: string[]; mailReads?: number[]; mailTraffic?: number; mailSeed?: object };
const phases: Phase[] = [];
async function phase(name: string, what: string, body: (p: Phase) => Promise<void>) {
  if (!phasesWanted.includes(name)) return;
  say(`phase ${name}: ${what}`);
  rssMax = 0;
  const p: Phase = { name, what, a: await reading(), b: null as any, rssMax: 0, trafficBefore: traffic(), trafficAfter: {}, streamStart: performance.now(), streamEnd: 0, marks: [], notes: [] };
  await body(p);
  p.b = await reading();
  p.trafficAfter = traffic();
  p.streamEnd = performance.now();
  const stray = strayAgents();
  if (stray.length) throw new Error(`invalid workload: late real provider process: ${stray.join("; ")}`);
  if (shots) {
    const sh = Bun.spawn(["import", "-display", display, "-window", "root", join(shots, `${name}.png`)], { stdout: "ignore", stderr: "ignore" });
    if (await sh.exited !== 0) throw new Error(`screenshot failed for ${name}`);
  }
  p.rssMax = Math.max(rssMax, p.b.rss);
  phases.push(p);
}

// Streaming
// ---------
let streaming = false;
async function startStreams() {
  if (streaming) return;
  streaming = true;
  ours.forEach((t: any, i: number) => send("turn.start", { thread: t.id, text: `stream for the perf bench ${i % 5 < 3 ? "zqsub" : ""}` }));
  const ready = await until(30000, () => ours.every((t: any) => (deltas.get(t.id) ?? 0) >= 2));
  if (!ready) {
    throw new Error(`invalid workload: not all ${nAgents} agents streamed parsed deltas; ${JSON.stringify(traffic())}; see ${root}/agents.log and app.err`);
  }
  say(`workload ready: all ${nAgents} agents delivered streaming deltas`);
}
// bots woken in rooms: the person posts in each room with bots every few seconds
const roomBots = (r: { members: string }) => r.members.split("\n").map((id) => botById.get(id)).filter((id): id is string => Boolean(id));
const botRooms = rooms.filter((r) => roomBots(r).length > 0);
const postedRooms = new Map<string, { at: number; bots: string[] }>();
let posting: Timer | null = null;
let posts = 0;
const startPosts = () => {
  if (posting || !botRooms.length) return;
  posting = setInterval(() => {
    const r = botRooms[posts++ % botRooms.length];
    if (!postedRooms.has(r.id)) postedRooms.set(r.id, { at: performance.now(), bots: roomBots(r) });
    send("bots.post", { room: r.id, text: `zqshort a quick note for the room, post ${posts}` });
  }, 4000);
};
const stopPosts = () => { if (posting) clearInterval(posting); posting = null; };
const typeKeys = async (n: number, gapMs: number) => {
  const words = "the quick brown fox jumps over the lazy dog and keeps typing ";
  for (let i = 0; i < n; i++) { await poke("type", words[i % words.length]); await sleep(gapMs); }
  for (let i = 0; i < n; i++) { await poke("key", "BackSpace"); await sleep(gapMs / 4); }
};
const ourTitle = (i: number) => `zqrun${String.fromCharCode(97 + i)} stream`;

// --hold: set up, start streaming, then wait until <root>/stop exists (to look around by hand)
if (argv.includes("--hold")) {
  await sleep(2000);
  await poke("size", String(W), String(H));
  await startStreams();
  startPosts();
  say(`holding: DISPLAY=${display}, root ${root}; touch ${root}/stop to end`);
  while (!existsSync(join(root, "stop"))) await sleep(500);
  phasesWanted.length = 0;
}

// window settles on the long thread
await sleep(2000);
await seedMail();
await poke("size", String(W), String(H));
await poke("focus");
await sleep(500);
await openByTitle("zqlong");
await sleep(3000);

await phase("idle", "window on the long thread, nothing streaming (blink only)", async () => {
  await composer();
  await sleep(secs * 1000);
});

await startStreams();
startPosts();
await sleep(4000);
await phase("bg", `${nAgents} agents streaming (${Math.round(nAgents * 0.6)} with a subagent and a background shell), bots posting; the window on the long thread`, async () => {
  await sleep(secs * 1000);
});

await openByTitle("zqruna");
await sleep(1500);
await phase("fg", "streaming; the window shows a streaming thread with its Subagents panel", async () => {
  await sleep(secs * 1000);
});

await phase("type", "streaming; 60 keys typed into the streaming thread's composer (8/s), then deleted", async () => {
  await composer();
  await typeKeys(60, 125);
  await sleep(1000);
});

await phase("open", "streaming; 10 real threads opened one after another (ctrl+k, title, Return)", async (p) => {
  for (let i = 0; i < 10; i++) p.marks!.push(await openByTitle(`zqopen${String.fromCharCode(97 + i)}`));
});

// Continuous demand is separate from sparse typing/stream updates. The
// driver keeps a fixed schedule; late dispatch remains invalid evidence.
await phase("scroll", "continuous 60Hz wheel demand over long history while ten agents stream", async (p) => {
  await openByTitle("zqlong");
  const start=performance.now();
  const count=Math.floor(secs*60);
  for(let tick=0;tick<count;tick++) {
    const due=start+tick*(1000/60);
    await sleep(Math.max(0,due-performance.now()));
    await poke("wheel",String(MAINX),String(MIDY),Math.floor(tick/24)%2?"down":"up");
  }
  p.notes!.push("Content revision/ROI observation producer is required: wheel dispatch alone is not cadence evidence.");
});

// the Status plane: the uberbot's first tab, reached by its sidebar row (see docs/perf-bench.md)
await phase("plane", "streaming; the uberbot's Status plane on screen, wheel over it", async (p) => {
  const ok = await statusPlane(p);
  if (!ok) p.notes!.push("Status plane not reached");
  for (let i = 0; i < 6; i++) { await poke("wheel", String(MAINX), String(MIDY), i % 2 ? "up" : "down"); await sleep(400); }
  await sleep(secs * 1000 / 2);
});

await phase("mail", `All mail: filed ${mailConvs} conversations/${mailMails} mails, 40 wheels, six search keys, ten conversations; target 2 mails/s behind FOH chat`, async (p) => {
  p.mailSeed = { conversations: mailConvs, mails: mailMails, bots: mailBots.length, through_filing: true, reused_home: mailReuseHome || null };
  p.mailReads = [];
  await poke("click", "130", "74");
  await sleep(800);
  const expectedBotThread = botById.get(fixtureBots.uber);
  if (!expectedBotThread || !await until(3000, () => deskThreads.has(expectedBotThread))) throw new Error("Mailroom bot selection was not observed");
  await poke("click", "400", "62"); // explicit Mailroom, including old sidebar defaults
  await sleep(800);
  if (shots) {
    const sh = Bun.spawn(["import", "-display", display, "-window", "root", join(shots, "mail-list.png")]);
    if (await sh.exited !== 0) throw new Error("Mailroom screenshot failed");
  }
  for (let i = 0; i < 40; i++) { await poke("wheel", "400", "600", "down"); await sleep(80); }
  await poke("click", "410", opt("mail-search-y", "222"));
  for (const c of "needle") { await poke("type", c); await sleep(125); }
  await sleep(1000);
  if (shots) {
    const sh = Bun.spawn(["import", "-display", display, "-window", "root", join(shots, "mail-search.png")]);
    if (await sh.exited !== 0) throw new Error("Mail search screenshot failed");
  }
  for (let i = 0; i < 10; i++) {
    const a = (await reading()).up;
    await poke("click", "410", String(Number(opt("mail-list-y", "325")) + i * 52));
    // A separate real roundtrip records the body-read latency; input-to-
    // frame marks below remain processing timings, not body presentation.
    const t = performance.now();
    await rpc("mail.read", { conv: `perf-mail-${String(mailConvs - 1 - i).padStart(5, "0")}`, n: 50 });
    p.mailReads.push(performance.now() - t);
    await sleep(250);
    if (shots) {
      const sh = Bun.spawn(["import", "-display", display, "-window", "root", join(shots, `mail-conversation-${i}.png`)]);
      if (await sh.exited !== 0) throw new Error("Mail conversation screenshot failed");
    }
    p.marks!.push([a, (await reading()).up]);
  }
  // Click Chat (third tab after Mailroom/Status) while mailing continues.
  await poke("click", "556", "62");
  p.mailTraffic = 0;
  const end = Date.now() + Math.max(5, secs) * 1000;
  while (Date.now() < end) {
    const j = p.mailTraffic++;
    const started = performance.now();
    await rpc("mail.send", { to: `bot:${mailBots[j % 6]}`, conv: `perf-mail-${String(j % mailConvs).padStart(5, "0")}`, text: `Live mail ${j}`, want: j % 2 ? "act" : "fyi" });
    await sleep(Math.max(0, 500 - (performance.now() - started)));
  }
  if (shots) {
    const sh = Bun.spawn(["import", "-display", display, "-window", "root", join(shots, "mail-foh.png")]);
    if (await sh.exited !== 0) throw new Error("Mail FOH screenshot failed");
  }
  p.notes!.push("Mailroom tab and FOH click positions require screenshot review; body RPC roundtrips are reported separately from native input-to-frame samples.");
});

await phase("normal", "normal use, all at once: agents + subagents + bots streaming; a streaming thread with Subagents (typing, scrolling), the long thread scrolled, the Status plane", async (p) => {
  await openByTitle("zqrunb");
  await composer();
  await typeKeys(30, 125);
  for (let i = 0; i < 4; i++) { await poke("wheel", String(MAINX), String(MIDY), "up"); await sleep(250); }
  for (let i = 0; i < 4; i++) { await poke("wheel", String(MAINX), String(MIDY), "down"); await sleep(250); }
  p.marks!.push(await openByTitle("zqlong"));
  for (let i = 0; i < 8; i++) { await poke("wheel", String(MAINX), String(MIDY), "up"); await sleep(300); }
  await composer();
  await typeKeys(20, 125);
  await statusPlane(p);
  await sleep(3000);
  p.marks!.push(await openByTitle("zqrunc"));
  await composer();
  await typeKeys(20, 125);
  await sleep(2000);
});

// Report
// ------
stopPosts();
clearInterval(rssTimer);
for (const t of ours) send("turn.interrupt", { thread: t.id });
await sleep(1500);
const strays = strayAgents();
const appLogTail = existsSync(join(root, "app.err")) ? readFileSync(join(root, "app.err"), "utf8").split("\n").slice(-20).join("\n") : "";
await cleanup();

type Ev = { name: string; ts: number; dur: number };
const evs: Ev[] = [];
const rawEvents:any[]=[];
for (const l of readFileSync(trace, "utf8").split("\n")) {
  const s = l.replace(/^,/, "").trim();
  if (!s.startsWith("{")) continue;
  try { const o = JSON.parse(s.replace(/,$/, "")); rawEvents.push(o); if(o.ph==="X") evs.push({ name: o.name, ts: o.ts, dur: o.dur }); } catch {}
}
evs.sort((a, b) => a.ts - b.ts);
const inWin = (a: number, b: number) => evs.filter((e) => e.ts >= a * 1000 && e.ts < b * 1000);
const pctOf = (xs: number[], p: number) => {
  if (!xs.length) return NaN;
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.ceil((p / 100) * s.length) - 1)];
};
type Sum = { n: number; p50: number; p99: number; max: number; sum: number; over8: number; over16: number };
const stat = (es: Ev[], name: string | RegExp): Sum => {
  const d = es.filter((e) => (typeof name === "string" ? e.name === name : name.test(e.name))).map((e) => e.dur / 1000);
  return { n: d.length, p50: pctOf(d, 50), p99: pctOf(d, 99), max: d.length ? Math.max(...d) : NaN, sum: d.reduce((a, b) => a + b, 0), over8: d.filter((x) => x > 8).length, over16: d.filter((x) => x > 16).length };
};
const f1 = (x: number) => (Number.isFinite(x) ? (x >= 100 ? x.toFixed(0) : x.toFixed(1)) : "-");
const results: any = { bin, observed_exe_sha256: observedExeSha, candidate:boundAtStart, ci:"unavailable — local gate only", acceptance_scope:"local-exact-candidate", mail_reuse_home: mailReuseHome || null, log: logSrc, log_sha256: logSha256, log_bytes: Buffer.byteLength(raw), port, agents: nAgents, secs, rooms: botRooms.length, home: keep ? home : "", phases: [] as any[] };
const order = ["normal", "idle", "bg", "fg", "type", "open", "scroll", "plane", "mail"];
const rows: string[] = [];
const verdicts: string[] = [];
for (const name of order) {
  const p = phases.find((x) => x.name === name);
  if (!p) continue;
  const es = inWin(p.a.up, p.b.up);
  const wallS = (p.b.wall - p.a.wall) / 1000;
  const cpu = ((p.b.cpu - p.a.cpu) * 10) / (p.b.wall - p.a.wall) * 100;
  const names = ["ui.frame", "ui.memo", "ui.layout", "ui.raster", "ui.present", "ui.layout.mm.hit", "ui.layout.mm.miss", "ui.mail.body.hit", "ui.msg.blink", "ui.msg.hub", "ui.input", "ui.hub", "hub.agent_line", "hub.mcp", "hub.rpc"];
  const st: Record<string, Sum> = {};
  for (const n of names) st[n] = stat(es, n);
  st["hub.*"] = stat(es, /^hub\./);
  // the opens: ui.input inside each Return's window
  // key to frame: a ui.input (from the end of its window message to the frame)
  // plus the ui.msg.win that handled the event (it ends where ui.input starts);
  // time an event waited in the queue before that is not traced
  const k2f = (xs: Ev[]) => {
    const wins = xs.filter((e) => e.name === "ui.msg.win");
    return xs.filter((e) => e.name === "ui.input").map((e) => {
      let w: Ev | undefined;
      for (const m of wins) if (m.ts + m.dur <= e.ts + 1000 && (!w || m.ts > w.ts)) w = m;
      return (e.ts + e.dur - (w && e.ts - (w.ts + w.dur) < 1000 ? w.ts : e.ts)) / 1000;
    });
  };
  const keys = k2f(es);
  // a thread open: the Return's window message to the frame showing the thread
  const opens = (p.marks ?? []).map(([a, b]) => Math.max(...k2f(inWin(a, b))));
  const byName = new Map<string, { n: number; sum: number; max: number }>();
  for (const e of es) {
    const r = byName.get(e.name) ?? { n: 0, sum: 0, max: 0 };
    r.n++; r.sum += e.dur / 1000; r.max = Math.max(r.max, e.dur / 1000);
    byName.set(e.name, r);
  }
  const top = [...byName.entries()].sort((a, b) => b[1].sum - a[1].sum).slice(0, 25).map(([n, r]) => ({ name: n, n: r.n, sum_ms: +r.sum.toFixed(1), mean_ms: +(r.sum / r.n).toFixed(2), max_ms: +r.max.toFixed(1) }));
  const hubBusy = (st["hub.*"].sum / 1000) / wallS * 100;
  const fr = st["ui.frame"], inp = st["ui.input"], al = st["hub.agent_line"];
  const key = { n: keys.length, p50: pctOf(keys, 50), p99: pctOf(keys, 99), max: keys.length ? Math.max(...keys) : NaN };
  const streamingCounts = Object.fromEntries(ours.map((t: any) => [t.id, (p.trafficAfter[t.id] ?? 0) - (p.trafficBefore[t.id] ?? 0)]));
  const streamWindows = Object.fromEntries(ours.map((t: any) => [t.id, streamWindow(deltaTimes.get(t.id) ?? [], p.streamStart, p.streamEnd)]));
  const checks: [string, boolean | null][] = [
    ...(name !== "idle" ? [["all agents streaming", Object.values(streamWindows).every((w) => w.sustained)] as [string, boolean | null]] : []),
    ...(["open", "mail"].includes(name) ? [["all ten opens sampled", completeOpens(opens, 10)] as [string, boolean | null]] : []),
    ...(name === "mail" ? [["Mailroom and ten conversation selections visually verified", null] as [string, boolean | null]] : []),
    ...(["normal", "plane"].includes(name) ? [["Status tab observed", null] as [string, boolean | null]] : []),
    ["frame p99<=8ms", fr.n ? fr.p99 <= 8 : null],
    ["no frame>16ms", fr.n ? fr.over16 === 0 : null],
    ...(["type", "normal"].includes(name) ? [["input p99<=16ms", inp.n ? inp.p99 <= 16 : null] as [string, boolean | null]] : []),
    ...(["type", "normal"].includes(name) ? [["key>frame p99<=16ms", key.n ? key.p99 <= 16 : null] as [string, boolean | null]] : []),
    ...(name === "open" && !opens.length ? [["open<=50ms", null] as [string, boolean | null]] : []),
    ...(opens.length ? [["open<=50ms", completeOpens(opens, p.marks?.length ?? 0) && Math.max(...opens) <= 50] as [string, boolean | null]] : []),
    ...(name !== "idle" ? [["agent_line p99<=2ms", al.n ? al.p99 <= 2 : null] as [string, boolean | null]] : []),
  ];
  const verdict = checks.map(([k, v]) => `${k}:${v === null ? "n/a" : v ? "PASS" : "FAIL"}`).join(" ");
  verdicts.push(`${name.padEnd(7)} ${verdict}`);
  rows.push(
    `${name.padEnd(7)}${f1(wallS).padStart(5)}s ` +
    `${String(fr.n).padStart(5)} ${f1(fr.p50).padStart(5)}/${f1(fr.p99).padStart(5)}/${f1(fr.max).padStart(5)} ${String(fr.over8).padStart(4)}/${String(fr.over16).padStart(4)} ` +
    `${f1(st["ui.layout"].p50).padStart(5)}/${f1(st["ui.layout"].p99).padStart(5)} ${f1(st["ui.raster"].p50).padStart(5)}/${f1(st["ui.raster"].p99).padStart(5)} ` +
    `${f1(st["ui.msg.blink"].p50).padStart(5)}/${f1(st["ui.msg.blink"].p99).padStart(5)} ${f1(inp.p50).padStart(5)}/${f1(inp.p99).padStart(5)} ${f1(key.p50).padStart(5)}/${f1(key.p99).padStart(5)} ` +
    `${opens.length ? f1(Math.max(...opens)).padStart(5) : "    -"} ` +
    `${String(al.n).padStart(6)} ${f1(al.p50).padStart(4)}/${f1(al.p99).padStart(5)}/${f1(al.max).padStart(5)} ${f1(st["hub.mcp"].p99).padStart(5)} ` +
    `${f1(hubBusy).padStart(5)} ${f1(cpu).padStart(5)} ${String(Math.round(p.rssMax / 1024)).padStart(5)}`,
  );
  // This is a value counter (ph:C), not a timed span. Keep it separate
  // from stats so an empty span sample cannot conceal actual body reuse.
  const mailHitBefore = p.a.rows.get("ui.mail.body.hit");
  const mailHitAfter = p.b.rows.get("ui.mail.body.hit");
  const mailBodyCache = { frames: (mailHitAfter?.n ?? 0) - (mailHitBefore?.n ?? 0), hits: (mailHitAfter?.sum ?? 0) - (mailHitBefore?.sum ?? 0) };
  results.phases.push({ name, mail_body_cache: mailBodyCache, mail_seed: p.mailSeed, mail_read_rpc_ms: p.mailReads, mail_streamed: p.mailTraffic, what: p.what, wall_s: wallS, cpu_pct: cpu, rss_max_mb: p.rssMax / 1024, hub_busy_pct: hubBusy, opens_ms: opens, key_to_frame: key, notes: p.notes, streaming_deltas: streamingCounts, stream_windows: streamWindows, stats: st, top, checks: Object.fromEntries(checks), perf_json_before: Object.fromEntries(p.a.rows), perf_json_after: Object.fromEntries(p.b.rows) });
}
console.log(`\nperf_load: ${bin}\n  log ${logSrc} (${kept.length} lines; sha256 ${logSha256}), ${nAgents} agents, ${botRooms.length} bot rooms, ${secs} s phases, port ${port}, display ${display}`);
console.log(`  ms; frame = ui.frame n p50/p99/max >8/>16; input = ui.input; key>frame = ui.msg.win + ui.input; open = the slowest thread open, Return to frame; line = hub.agent_line n p50/p99/max; hub% = hub.* span time / wall; cpu% = whole app process`);
console.log(`phase    wall     n  frame p50/p99/max  >8/>16  layout p50/p99 raster p50/p99 blink p50/p99 input p50/p99 key>frame  open   line  p50/p99/max   mcp99  hub%  cpu%  rssMB`);
for (const r of rows) console.log(r);
console.log("\ntargets: frame p99<=8, no frame>16 from streaming (idle/bg), typing input p99<=16, thread open<=50, agent_line p99<=2");
for (const v of verdicts) console.log(v);
for (const p of results.phases) {
  console.log(`\n${p.name}: heaviest spans (total ms in phase, n, mean, max)`);
  for (const t of p.top.slice(0, 8)) console.log(`  ${t.name.padEnd(28)} ${String(t.sum_ms).padStart(9)} ${String(t.n).padStart(6)} ${String(t.mean_ms).padStart(8)} ${String(t.max_ms).padStart(8)}`);
  if (p.notes?.length) console.log(`  notes: ${p.notes.join("; ")}`);
}
if (strays.length) console.log(`\nINVALID WORKLOAD: real agents seen under the hub:\n${strays.join("\n")}`);
results.ws = { msgs: wsMsgs, bytes: wsBytes };
results.streaming_deltas = traffic();
results.bot_streaming_deltas = Object.fromEntries([...deltas].filter(([th]) => botThreads.has(th)));
results.room_streaming = Object.fromEntries([...postedRooms].map(([room, evidence]) => [room,
  Object.fromEntries(evidence.bots.map((th) => [th, (deltaTimes.get(th) ?? []).some((t) => t >= evidence.at)]))]));
results.bot_streaming = !phasesWanted.includes("normal") || (botRooms.length > 0 && postedRooms.size === botRooms.length
  && Object.values(results.room_streaming).every((bots: any) => Object.values(bots).every((v) => v === true)));
results.open_selections = openEvidence;
results.stray_agents = strays;
results.agentsLog = readFileSync(fakeLog, "utf8").split("\n").filter((l) => l.startsWith("TURN")).length;
results.appErrTail = appLogTail;
const workloadValid = results.bot_streaming && strays.length === 0 && !results.phases.some((p: any) => p.checks["all agents streaming"] === false);
// Bind raw observations, not a copied summary verdict, to the exact build.
// Missing render/ROI producer data is intentionally left missing and fails
// strict validation; callbacks, message generations and desk.look cannot fill it.
let exactVerdict:any=null;
if(boundAtStart) {
  const after=candidateBinding(here,bin,logSrc,receiptPath,"linux-native");
  if(JSON.stringify(after)!==JSON.stringify(boundAtStart)) throw new Error("candidate changed during measurement");
  const labels:Record<string,string>={scroll:"continuous-scroll",type:"input",open:"thread-switch",bg:"background"};
  const rawBundle:PerformanceBundle={schema:1,built:{...boundAtStart,commit:buildReceipt.commit,source_sha256:buildReceipt.source_before,
    binary_sha256:buildReceipt.binary_sha256,compiler_sha256:buildReceipt.compiler_sha256},measured:boundAtStart,
    started_at:t0Wall,completed_at:Date.now(),history_bytes:Buffer.byteLength(raw),phases:phases.map(p=>({
      name:labels[p.name]??p.name,start_ms:p.a.up,end_ms:p.b.up,
      spans:inWin(p.a.up,p.b.up).map(e=>({name:e.name,at_ms:e.ts/1000,duration_ms:e.dur/1000})),
      streams:ours.map((t:any)=>({id:t.id,received_ms:(deltaTimes.get(t.id)??[])
        .filter(t=>t>=p.streamStart&&t<p.streamEnd).map(t=>p.a.up+t-p.streamStart)})),
      opens:(p.marks??[]).map((mark,i)=>({expected:p.name==="open"?openTs[i].id:"",drawn:"",duration_ms:results.phases.find((q:any)=>q.name===p.name)?.opens_ms[i]})),
    }))};
  exactVerdict=verifyPerformanceBundle(rawBundle,boundAtStart,["long-history","timeline","subagents","rooms","bots","status"]);
  if(evidencePath) {
    const rawPath=resolve(evidencePath+".raw.json");
    const retainedTrace=resolve(evidencePath+".trace.json");
    writeFileSync(rawPath,JSON.stringify(rawBundle));
    // Evidence must survive artifact upload and promotion out of the checkout;
    // retaining a pointer to an unrelated /tmp directory is insufficient.
    cpSync(trace,retainedTrace);
    writeFileSync(evidencePath,JSON.stringify({schema:1,raw:{path:basename(rawPath),sha256:fileSha(rawPath)},
      trace:{path:basename(retainedTrace),sha256:fileSha(retainedTrace)},candidate:boundAtStart},null,2));
  }
}
results.acceptance = performanceVerdict(results.phases, workloadValid, diagnostic);
if(!diagnostic) {
  if(!exactVerdict?.accepted) {
    results.acceptance.accepted=false;results.acceptance.exitCode=1;
    results.acceptance.failures.push(...(exactVerdict?.failures??["exact candidate native content/cadence evidence missing"]));
  }
}
results.exact_candidate_acceptance=exactVerdict;
if (outPath) { writeFileSync(outPath, JSON.stringify(results, null, 1)); console.log(`\nraw JSON: ${outPath}`); }
if (!keep) rmSync(root, { recursive: true, force: true });
else console.log(`kept ${root}`);
// Performance targets and missing coverage are enforced by default.
// Diagnostics remain useful while fixing failures, but cannot claim acceptance.
console.log(`\n${results.acceptance.mode}: ${results.acceptance.accepted ? "PASS" : "NOT ACCEPTED"}`);
for (const failure of results.acceptance.failures) console.log(`  ${failure}`);
process.exitCode = results.acceptance.exitCode;

// the uberbot's Status plane: its sidebar row (declared last; hoisted)
async function statusPlane(p: Phase): Promise<boolean> {
  return statusPlaneImpl(p);
}
async function statusPlaneImpl(_p: Phase): Promise<boolean> {
  // bots.bend's Bv.uber.row: the hit "uber" at (0, 52, 264, 44)
  await poke("click", "130", "74");
  await sleep(800);
  const expected = botById.get(fixtureBots.uber);
  if (!expected || !await until(2000, () => deskThreads.has(expected))) throw new Error(`uberbot selection not observed: bot=${fixtureBots.uber}, thread=${expected}; desk=${[...deskThreads]}`);
  _p.notes!.push("Uberbot selection observed through desk.look; Status tab itself is unverified (strict coverage check remains n/a).");
  return false;
}
