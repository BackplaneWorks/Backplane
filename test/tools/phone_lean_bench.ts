// The phone client (bridge.js) under normal use: a real log, ten agents
// streaming (some with a subagent and a background shell), measured offline
// so builds compare frame for frame.
//
//   bun test/tools/phone_lean_bench.ts record --bin build/backplane --out FRAMES.jsonl
//       [--log ~/.backplane-bend/events.jsonl] [--port 3811] [--agents 10] [--secs 20]
//     a headless hub on a sanitized copy of the log (devices, hooks, peers,
//     routines, push and deploy settings dropped, restart.continue off,
//     stand-in agents only), joined as a phone would; every frame it sends
//     the phone is written down (base64 CBOR and the ms it came), first the
//     join, then while ten threads stream. Keep the file out of the repo:
//     it holds the log's text.
//
//   BUN_JSC_useJIT=0 bun test/tools/phone_lean_bench.ts replay BRIDGE.js FRAMES.jsonl
//     the frames into bridge.js as the apps hand them over (one recv each),
//     the join first, then the live frames twice: once with the list on the
//     screen, once with a streaming thread open. Prints the cost of each kind
//     of frame, of the screens, and how busy the bridge thread was (Bend
//     time over the recording's wall time). BUN_JSC_useJIT=0 is the phones'
//     engine (no JIT); compare builds with each other.
//     --verify: after every screen, the bridge's kept halves against the
//     whole screen made from scratch (Backplane.halves); any difference is
//     printed and the run fails.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : d;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const here = resolve(import.meta.dir, "../..");

// CBOR, the hub's wire (src/core/cbor.bend)
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
      if (tag === 6) return WORDS[item(false)];
      return Number(item(false));
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

const mode = argv[0];
if (mode === "record") await record();
else if (mode === "replay") replay(argv[1], argv[2]);
else {
  console.error("usage: phone_lean_bench.ts record --bin BIN --out FRAMES | replay BRIDGE FRAMES");
  process.exit(2);
}

async function record() {
  const bin = resolve(opt("bin", join(here, "build/backplane")));
  const out = resolve(opt("out", ""));
  const port = Number(opt("port", "3811"));
  const nAgents = Number(opt("agents", "10"));
  const secs = Number(opt("secs", "20"));
  const liveHome = join(homedir(), ".backplane-bend");
  const logSrc = resolve(opt("log", join(liveHome, "events.jsonl")));
  if (port === 3787) throw new Error("3787 is the live hub's port");
  if (!out) throw new Error("--out FRAMES.jsonl");
  if (realpathSync(bin).startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("refusing to run the installed binary");
  try {
    await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) });
    throw new Error(`something already answers on port ${port}`);
  } catch (e: any) {
    if (String(e.message).startsWith("something")) throw e;
  }
  const root = mkdtempSync(join(tmpdir(), "bp-phonebench-"));
  const home = join(root, "home"), user = join(root, "user"), fake = join(root, "bin"), locks = join(root, "locks"), proj = join(root, "proj");
  for (const d of [home, user, fake, locks, proj]) mkdirSync(d, { recursive: true });
  // the log, copied and sanitized (as perf_load.ts does)
  const dropTypes = new Set(["DeviceRegistered", "DeviceDropped", "HookSet", "PeerSet", "WakeSet", "RoutineSet", "RoutinePlan", "LockChanged"]);
  const dropSetting = /^(push|voice|deploy|google|hook|apns|token|secret)/i;
  const elsewhere = join(root, "elsewhere");
  const kept: string[] = [];
  const openTasks = new Set<string>();
  const botIds = new Set<string>();
  for (const line of readFileSync(logSrc, "utf8").split("\n")) {
    if (!line) continue;
    const ty = /^\{"\$":"([A-Za-z]+)"/.exec(line)?.[1] ?? "";
    if (dropTypes.has(ty) || /devicetoken|webhook|apns/i.test(line)) continue;
    if (ty === "WorkSet" && !line.includes('"kind":"bg"')) continue;
    if (ty === "TaskDelegated") openTasks.add(/"task":"([^"]*)"/.exec(line)?.[1] ?? /"id":"([^"]*)"/.exec(line)?.[1] ?? "");
    if (ty === "TaskDelivered") openTasks.delete(/"task":"([^"]*)"/.exec(line)?.[1] ?? "");
    if (ty === "SettingSet" && dropSetting.test(/"key":"([^"]*)"/.exec(line)?.[1] ?? "")) continue;
    if (ty === "BotSet") botIds.add(/"id":"([^"]*)"/.exec(line)?.[1] ?? "");
    kept.push(line.split(liveHome).join(home).split(homedir() + "/").join(elsewhere + "/"));
  }
  const now = Math.floor(Date.now() / 1000);
  for (const k of openTasks) if (k) {
    kept.push(JSON.stringify({ $: "TaskStateSet", task: k, state: "cancelled", summary: "", at: now }));
    kept.push(JSON.stringify({ $: "TaskDelivered", task: k, at: now }));
  }
  kept.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: now }));
  writeFileSync(join(home, "events.jsonl"), kept.join("\n") + "\n");
  for (const b of botIds) if (b) mkdirSync(join(home, "bots", b), { recursive: true });
  // the stand-in agent: perf_load.ts's (text deltas, a tool call every 3 s,
  // "zqsub" adds a subagent and a background shell with progress each tick)
  const claude = `#!/bin/sh
  secs=\${FAKE_SECS:-600}; per=\${FAKE_PER:-3}; cycle=\${FAKE_CYCLE:-30}
  sid="fake-$$"; n=0
  echo "START $$ $(pwd)" >> "$FAKE_LOG"
  while IFS= read -r line; do
    case "$line" in
      *'"type":"user"'*)
        n=$((n + 1)); ticks=$((secs * 10)); sub=
        case "$line" in *zqshort*) ticks=80 ;; esac
        case "$line" in *zqsub*) sub=1 ;; esac
        echo "TURN $$ sub=$sub ticks=$ticks" >> "$FAKE_LOG"
        printf '%s\\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash","Read","Agent"],"model":"fake","permissionMode":"default"}'
        if [ -n "$sub" ]; then
          printf '%s\\n' '{"type":"assistant","message":{"id":"ma'"$$-$n"'","role":"assistant","content":[{"type":"tool_use","id":"ag'"$$-$n"'","name":"Agent","input":{"description":"Survey the footprints","prompt":"Look through the footprints and report","subagent_type":"general-purpose"}}],"usage":{"input_tokens":10}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"a'"$$-$n"'","tool_use_id":"ag'"$$-$n"'","description":"Survey the footprints","task_type":"local_agent","is_backgrounded":true,"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"b'"$$-$n"'","tool_use_id":"tb'"$$-$n"'","description":"npm run watch","task_type":"local_bash","is_backgrounded":true,"session_id":"'"$sid"'"}'
        fi
        t=0; k=0
        while [ $t -lt $ticks ]; do
          kill -0 $PPID 2>/dev/null || exit 0
          t=$((t + 1)); k=$((k + 1))
          if [ $k -eq 1 ]; then
            printf '%s\\n' '{"type":"stream_event","event":{"type":"message_start","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[],"usage":{"input_tokens":10}}},"session_id":"'"$sid"'"}'
            printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"'"$sid"'"}'
          fi
          if [ -n "$sub" ]; then
            printf '%s\\n' '{"type":"system","subtype":"task_progress","task_id":"a'"$$-$n"'","description":"Reading footprint '"$t"'","session_id":"'"$sid"'"}'
            printf '%s\\n' '{"type":"system","subtype":"task_progress","task_id":"b'"$$-$n"'","description":"rebuilt '"$t"'","session_id":"'"$sid"'"}'
            if [ $((t % 10)) -eq 5 ]; then
              printf '%s\\n' '{"type":"assistant","parent_tool_use_id":"ag'"$$-$n"'","message":{"id":"ms'"$$-$n-$t"'","role":"assistant","content":[{"type":"text","text":"Checked footprint '"$t"', pads match the datasheet."},{"type":"tool_use","id":"st'"$$-$n-$t"'","name":"Read","input":{"file_path":"/tmp/f'"$t"'.kicad_mod"}}]},"session_id":"'"$sid"'"}'
            fi
          fi
          now=$(date +%s%3N); i=0
          while [ $i -lt $per ]; do
            i=$((i + 1))
            printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"@'"$now"'@ some words of an answer about the board, "}},"session_id":"'"$sid"'"}'
          done
          if [ $k -ge $cycle ]; then
            k=0
            printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"'"$sid"'"}'
            printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[{"type":"text","text":"Some words of an answer about the board, the whole paragraph the deltas built, as Claude sends it once the block ends. It names U3 and the 3V3 rail, and says what changed in the layout and why."}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$t"'}},"session_id":"'"$sid"'"}'
            printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'b","role":"assistant","content":[{"type":"tool_use","id":"tu'"$$-$n-$t"'","name":"Bash","input":{"command":"ls -la src/core | head -'"$t"'","description":"List files"}}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$((t + 1))"'}},"session_id":"'"$sid"'"}'
            printf '%s\\n' '{"type":"user","message":{"role":"user","content":[{"tool_use_id":"tu'"$$-$n-$t"'","type":"tool_result","content":"total 42\\\\ndrwxr-xr-x  2 h h 4096 model.bend\\\\n-rw-r--r--  1 h h 1234 hub.bend","is_error":false}]},"session_id":"'"$sid"'"}'
          fi
          sleep 0.1
        done
        if [ -n "$sub" ]; then
          printf '%s\\n' '{"type":"system","subtype":"task_notification","task_id":"a'"$$-$n"'","tool_use_id":"ag'"$$-$n"'","status":"completed","session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"system","subtype":"task_notification","task_id":"b'"$$-$n"'","tool_use_id":"tb'"$$-$n"'","status":"completed","session_id":"'"$sid"'"}'
        fi
        printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
    esac
  done
  `;
  writeFileSync(join(fake, "claude"), claude);
  chmodSync(join(fake, "claude"), 0o755);
  for (const n of ["codex", "grok", "muse", "omp", "agy", "tailscale", "notify-send", "gh", "git", "kicad-cli", "freecad", "xdg-open", "gio"]) {
    writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
    chmodSync(join(fake, n), 0o755);
  }
  const fakeLog = join(root, "agents.log");
  writeFileSync(fakeLog, "");
  const app = Bun.spawn([bin, "--home", home, "--port", String(port), "--no-tailscale"], {
    env: {
      BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/local/bin:/usr/bin:/bin`, HOME: user, USER: process.env.USER ?? "h", LANG: process.env.LANG ?? "C.UTF-8",
      DISPLAY: "", WAYLAND_DISPLAY: "", DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
      BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_LOCK_DIR: locks,
      BACKPLANE_DEPLOY_LOCKS: `${locks}/bp-bend.lock:${locks}/bp-wt-build.lock`, BACKPLANE_APNS_URL: "http://127.0.0.1:9",
      BACKPLANE_HOOK_URL: "", BACKPLANE_GOOGLE_API: "http://127.0.0.1:9", BACKPLANE_GOOGLE_OAUTH: "http://127.0.0.1:9",
      FAKE_LOG: fakeLog, FAKE_SECS: String(secs + 30), FAKE_PER: "3", FAKE_CYCLE: "30",
    },
    cwd: root, stdout: Bun.file(join(root, "app.log")), stderr: Bun.file(join(root, "app.err")),
  });
  console.error(`[record] hub pid ${app.pid}, home ${home}`);
  const stop = async () => {
    try { app.kill(); } catch {}
    await Promise.race([app.exited, sleep(5000)]);
  };
  try {
    let ready = false;
    for (let i = 0; i < 1800 && app.exitCode === null; i++) {
      try { if ((await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(2000) })).status === 200) { ready = true; break; } } catch {}
      await sleep(100);
    }
    if (!ready) throw new Error(`hub did not start; see ${root}/app.err`);
    await sleep(2000);
    { const s = readFileSync(fakeLog, "utf8").trim(); if (s) console.error(`[record] stand-ins the log started itself:\n${s}`); }
    const key = `127.0.0.1:${port}`;
    const frames: { at: number; b: string }[] = [];
    const items: any[] = [];
    let last = Date.now();
    const ws = new WebSocket(`ws://${key}/ws?since=0&origin=`);
    ws.binaryType = "arraybuffer";
    ws.onmessage = (e) => {
      const u = new Uint8Array(e.data as ArrayBuffer);
      last = Date.now();
      frames.push({ at: Date.now(), b: Buffer.from(u).toString("base64") });
      let o: any;
      try { o = cborDec(u); } catch { return; }
      for (const c of Array.isArray(o?.items) ? o.items : []) {
        if (typeof c === "string") { try { items.push(JSON.parse(c)); } catch {} } else items.push(c);
      }
    };
    await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
    while (Date.now() - last < 2000 || frames.length === 0) await sleep(50);
    console.error(`[record] join: ${frames.length} frames`);
    const joinN = frames.length;
    items.length = 0;
    let rid = 0;
    const send = (m: string, p: object) => {
      const o: number[] = [];
      cborEnc({ id: ++rid, m, p }, o);
      ws.send(new Uint8Array(o));
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
    const t0 = Date.now();
    send("project.add", { path: proj });
    const pc = await until(20000, () => items.find((c) => c.$ === "ProjectCreated" && c.root === proj));
    if (!pc) throw new Error("no project");
    for (let i = 0; i < nAgents; i++) send("thread.create", { project: pc.id, title: `zqrun${String.fromCharCode(97 + i)} stream`, env: "local" });
    const ours = await until(30000, () => {
      const t = items.filter((c) => c.$ === "ThreadCreated" && c.project === pc.id && /^zqrun/.test(c.title ?? ""));
      return t.length >= nAgents ? t : undefined;
    });
    if (!ours) throw new Error("threads not created");
    ours.forEach((t: any, i: number) => send("turn.start", { thread: t.id, text: i % 3 === 0 ? "zqsub look through the footprints" : "look at the board", msg: `bench-${i}` }));
    const end = Date.now() + secs * 1000;
    while (Date.now() < end) await sleep(200);
    const fl = frames.slice(0);
    const wall = Date.now() - t0;
    writeFileSync(out, JSON.stringify({ key, threads: ours.map((t: any) => t.id), wall }) + "\n" + fl.map((f, i) => JSON.stringify({ ms: f.at - t0, live: i >= joinN, b: f.b })).join("\n") + "\n");
    console.error(`[record] ${fl.length} frames (${fl.length - joinN} live, ${wall} ms) to ${out}`);
    ws.close();
  } finally {
    await stop();
    if (!argv.includes("--keep")) rmSync(root, { recursive: true, force: true });
  }
}

function replay(bridge: string, file: string) {
  const lines = readFileSync(file, "utf8").split("\n").filter(Boolean);
  const head = JSON.parse(lines[0]);
  const all = lines.slice(1).map((l) => JSON.parse(l));
  const join_ = all.filter((f) => !f.live);
  const live = all.filter((f) => f.live);
  const key: string = head.key;
  const t0 = performance.now();
  (0, eval)(readFileSync(bridge, "utf8"));
  const B = (globalThis as any).Backplane;
  console.log(`script: ${(performance.now() - t0).toFixed(0)} ms`);
  let screen: any = null;
  let screens = 0;
  const verify = argv.includes("--verify");
  let checked = 0, wrong = 0;
  // where two JSON values differ ("" when they do not), key order aside
  const diff = (a: any, b: any, at: string): string => {
    if (a === b) return "";
    if (typeof a !== typeof b || a === null || b === null || typeof a !== "object") return at || "(top)";
    if (Array.isArray(a) !== Array.isArray(b)) return at;
    const ks = new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const k of ks) { const d = diff(a[k], b[k], at + "." + k); if (d) return d; }
    return "";
  };
  const check = (label: string) => {
    if (!verify) return;
    const h = JSON.parse(B.halves());
    checked++;
    const d = diff(h.kept, h.whole, "");
    if (d) { wrong++; if (wrong <= 5) console.log(`  MISMATCH after ${label}: ${d}`); }
  };
  const handle = (text: string) => {
    const o = JSON.parse(text);
    if (o.screen) { screen = o.screen; screens++; }
    return text.length;
  };
  const time = (f: () => string) => {
    const t = performance.now();
    const n = handle(f());
    return [performance.now() - t, n];
  };
  const stats = (xs: number[]) => {
    if (!xs.length) return "-";
    const s = [...xs].sort((a, b) => a - b);
    const sum = s.reduce((a, b) => a + b, 0);
    const p = (q: number) => s[Math.min(s.length - 1, Math.floor(q * s.length))].toFixed(1);
    return `n ${String(s.length).padStart(4)}  mean ${(sum / s.length).toFixed(1).padStart(6)}  p50 ${p(0.5).padStart(6)}  p99 ${p(0.99).padStart(6)}  max ${s[s.length - 1].toFixed(1).padStart(6)}  total ${sum.toFixed(0).padStart(6)}`;
  };
  handle(B.start("bench", "{}"));
  handle(B.hubs([key]));
  handle(B.online(key, true, true));
  let tj = performance.now();
  // a burst, as the apps hand it over: quiet while more wait, the last draws
  join_.forEach((f, i) => handle(B.recv(key, f.b, i < join_.length - 1, false)));
  console.log(`join: ${join_.length} frames in ${(performance.now() - tj).toFixed(0)} ms`);
  const listT: number[] = [];
  let size = 0;
  for (let i = 0; i < 8; i++) { const [ms, n] = time(() => B.screen()); listT.push(ms); size = n; }
  console.log(`screen, the list:           ${stats(listT)}  (${(size / 1e3).toFixed(0)} KB)`);
  check("the list");
  const kind = (b: string) => {
    const o = cborDec(new Uint8Array(Buffer.from(b, "base64")));
    const t = String(o?.t ?? "?");
    if (t === "delta") return [t, String(o.thread ?? "")];
    return [t, ""];
  };
  const kinds = live.map((f) => kind(f.b));
  const pass = (label: string, open: string) => {
    const by = new Map<string, number[]>();
    let busy = 0;
    const s0 = screens;
    for (let i = 0; i < live.length; i++) {
      const [t, th] = kinds[i];
      const name = t === "delta" ? (th === open ? "delta, the open thread" : "delta, another thread") : t;
      const before = screens;
      const [ms] = time(() => B.recv(key, live[i].b, false, false));
      if (screens > before) check(`${label} frame ${i} (${name})`);
      const k = name + (screens > before ? " (screen)" : "");
      if (!by.has(k)) by.set(k, []);
      by.get(k)!.push(ms);
      busy += ms;
    }
    console.log(`\n${label}: ${live.length} live frames over ${(head.wall / 1000).toFixed(0)} s, ${screens - s0} screens; bridge busy ${busy.toFixed(0)} ms (${((busy / head.wall) * 100).toFixed(0)}% of the thread)`);
    for (const [k, xs] of [...by.entries()].sort((a, b) => b[1].reduce((x, y) => x + y, 0) - a[1].reduce((x, y) => x + y, 0)))
      console.log(`  ${k.padEnd(32)} ${stats(xs)}`);
  };
  // As the iOS app runs it (Engine.swift): frames in the order and at the
  // times they came, one at a time; a frame within 250 ms of the last screen
  // builds none (quiet) and a flush 250 ms after that screen builds the one
  // it owes. Prints how busy the bridge's thread was and how long a frame
  // (or a tap) waited behind the ones before it.
  const appPass = (label: string) => {
    let clock = 0, built = -1e9, busy = 0, flushAt = -1, n = 0;
    const calls: number[] = [], waits: number[] = [], shown: number[] = [];
    const run = (at: number, f: () => string) => {
      clock = Math.max(clock, at);
      waits.push(clock - at);
      const start = clock, s0 = screens;
      const [ms] = time(f);
      clock += ms; busy += ms; calls.push(ms);
      if (screens > s0) { built = start; shown.push(ms); }
      return screens > s0;
    };
    for (const f of live) {
      while (flushAt >= 0 && flushAt <= f.ms) { const at = flushAt; flushAt = -1; run(at, () => B.flush(false)); }
      const withheld = Math.max(clock, f.ms) - built < 250;
      const drew = run(f.ms, () => B.recv(key, f.b, withheld, false));
      if (withheld && !drew && flushAt < 0) flushAt = built + 250;
      n++;
    }
    if (flushAt >= 0) run(flushAt, () => B.flush(false));
    const p = (xs: number[], q: number) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.floor(q * s.length))] : 0; };
    console.log(`\n${label}, as the app runs it: bridge busy ${((busy / head.wall) * 100).toFixed(0)}% of the thread; ${shown.length} screens (p50 ${p(shown, 0.5).toFixed(1)}, p99 ${p(shown, 0.99).toFixed(1)} ms); a call p50 ${p(calls, 0.5).toFixed(1)}, p99 ${p(calls, 0.99).toFixed(1)} ms; waited p50 ${p(waits, 0.5).toFixed(1)}, p99 ${p(waits, 0.99).toFixed(1)}, max ${p(waits, 1).toFixed(1)} ms`);
  };

  // pass 1: the list on the screen; pass 2 starts again from the same state
  const snap = B.save();
  pass("list open", "");
  handle(B.load(snap));
  handle(B.online(key, true, true));
  appPass("list open");
  handle(B.load(snap));
  handle(B.online(key, true, true));
  // pass 2: a streaming thread open (the live frames again: deltas append, changes are seen again)
  const open = head.threads[0];
  const [openMs] = time(() => B.act("select", `${key}|${open}`));
  const focus: number[] = [];
  for (let i = 0; i < 8; i++) { const [ms, n] = time(() => B.screen()); focus.push(ms); size = n; }
  console.log(`\nopen a streaming thread: ${openMs.toFixed(1)} ms`);
  console.log(`screen, the open thread:    ${stats(focus)}  (${(size / 1e3).toFixed(0)} KB)`);
  check("the open thread");
  const snap2 = B.save();
  pass("thread open", open);
  handle(B.load(snap2));
  handle(B.online(key, true, true));
  appPass("thread open");
  if (verify) {
    console.log(`\nverify: ${checked} screens checked, ${wrong} differ`);
    if (wrong) process.exit(1);
  }
}
