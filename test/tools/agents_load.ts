// Load: many agents streaming at once into one headless hub. For each
// count in --agents it starts a hub on a temporary home and a free port,
// with a stand-in `claude` first on PATH (no real model runs), makes that
// many threads, starts a turn on each, and while they all stream measures:
//   rtt     a cheap request's round trip through the hub (ms, p50/p99/max)
//   lag     a delta's age when it reaches a connected client (the stand-in
//           stamps each tick's deltas with the time it wrote them)
//   rate    deltas emitted vs received per second, and all messages/bytes
//   cpu     the hub process's CPU% over the run
// then prints the hub's heaviest /debug/perf rows.
//
//   bun test/tools/agents_load.ts [BINARY] [WIREDIR] [--agents 1,10,50]
//     [--secs 20] [--hz 20] [--cycle 20] [--perf 12] [--progress] [--keep]
//
// The stand-in (Claude's stream-json with --include-partial-messages): on
// each user line it writes the init line, then every 100 ms hz/10 text
// deltas (stream_event content_block_delta), and every `cycle` ticks the
// finished text (an assistant line), a tool call (assistant tool_use) and
// its result (a user line); after `secs` the result line.
// --progress: each agent also starts a background shell and sends a
// task_progress line every tick (a new description each time); the report
// counts the WorkSet writes that reached the client, which the hub holds to
// one per thread per 5 s (Prog.line), the newest landing on its beat.
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). --keep leaves the homes in /tmp.
// LOAD_DISPLAY=:77 opens the window too (Xvfb), so /debug/perf has ui.* rows.
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : d;
};
const pos = argv.filter((a, i) => !a.startsWith("--") && !(i > 0 && argv[i - 1].startsWith("--") && argv[i - 1] !== "--keep"));
const [bin = "build/backplane", wire = "build/wire"] = pos;
const counts = opt("agents", "1,10,50").split(",").map(Number);
const secs = Number(opt("secs", "20"));
const hz = Number(opt("hz", "20"));
const cycle = Number(opt("cycle", "20"));
const perfRows = Number(opt("perf", "12"));
const keep = argv.includes("--keep");
const progress = argv.includes("--progress");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-load-"));
const fake = join(root, "bin");
mkdirSync(fake);
// one tick: hz/10 deltas stamped with the ms they were written
const claude = `#!/bin/sh
secs=\${FAKE_SECS:-20}; per=\${FAKE_PER:-2}; cycle=\${FAKE_CYCLE:-20}
sid="fake-$$"; n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      n=$((n + 1))
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash","Read"],"model":"fake","permissionMode":"default"}'
      ticks=$((secs * 10)); t=0; k=0
      if [ -n "$FAKE_PROG" ]; then
        printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"b'"$$"'","tool_use_id":"tb'"$$-$n"'","description":"watch","task_type":"local_bash","is_backgrounded":true,"session_id":"'"$sid"'"}'
      fi
      while [ $t -lt $ticks ]; do
        t=$((t + 1)); k=$((k + 1))
        if [ $k -eq 1 ]; then
          printf '%s\\n' '{"type":"stream_event","event":{"type":"message_start","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[],"usage":{"input_tokens":10}}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"'"$sid"'"}'
        fi
        if [ -n "$FAKE_PROG" ]; then
          printf '%s\\n' '{"type":"system","subtype":"task_progress","task_id":"b'"$$"'","description":"step '"$t"'","session_id":"'"$sid"'"}'
        fi
        now=$(date +%s%3N); i=0
        while [ $i -lt $per ]; do
          i=$((i + 1))
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"@'"$now"'@ some words of an answer "}},"session_id":"'"$sid"'"}'
        done
        if [ $k -ge $cycle ]; then
          k=0
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[{"type":"text","text":"Some words of an answer, the whole paragraph the deltas built, as Claude sends it once the block ends."}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$t"'}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'b","role":"assistant","content":[{"type":"tool_use","id":"tu'"$$-$n-$t"'","name":"Bash","input":{"command":"ls -la src/core | head -'"$t"'","description":"List files"}}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$((t + 1))"'}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"user","message":{"role":"user","content":[{"tool_use_id":"tu'"$$-$n-$t"'","type":"tool_result","content":"total 42\\\\ndrwxr-xr-x  2 h h 4096 model.bend\\\\n-rw-r--r--  1 h h 1234 hub.bend","is_error":false}]},"session_id":"'"$sid"'"}'
        fi
        sleep 0.1
      done
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
`;
writeFileSync(join(fake, "claude"), claude);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

const pct = (xs: number[], p: number) => {
  if (!xs.length) return NaN;
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
};
const f1 = (x: number) => (Number.isFinite(x) ? x.toFixed(1) : "-");
const cpuTicks = (pid: number) => {
  try {
    const f = readFileSync(`/proc/${pid}/stat`, "utf8").split(") ")[1].split(" ");
    return Number(f[11]) + Number(f[12]);
  } catch {
    return NaN;
  }
};
const rssKb = (pid: number) => {
  try {
    return Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${pid}/status`, "utf8"))![1]);
  } catch {
    return NaN;
  }
};

type Result = { n: number; rtt: number[]; lag: number[]; emitted: number; got: number; msgs: number; bytes: number; cpu: number; rss: number; drain: number; perf: string; ws: number };

async function run(n: number): Promise<Result> {
  const home = join(root, `home-${n}`);
  const proj = join(root, `proj-${n}`);
  mkdirSync(proj, { recursive: true });
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: {
      ...process.env, DISPLAY: process.env.LOAD_DISPLAY ?? "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "",
      FAKE_SECS: String(secs), ...(progress ? { FAKE_PROG: "1" } : {}), FAKE_PER: String(Math.max(1, Math.round(hz / 10))), FAKE_CYCLE: String(cycle),
      BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`,
    },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const replies = new Map<number, number>();
  let msgs = 0, bytes = 0, got = 0;
  const lag: number[] = [];
  let lastMsg = Date.now();
  let counting = false;
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const now = Date.now();
    const text = typeof e.data === "string";
    const raw = text ? new TextEncoder().encode(e.data as string) : new Uint8Array(e.data as ArrayBuffer);
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(raw)); } catch {}
    if (!o || typeof o !== "object") { if (process.env.LOAD_DEBUG) console.log("undecoded", raw.length, raw.slice(0, 16)); return; }
    if (process.env.LOAD_DEBUG) console.log("msg", JSON.stringify(o).slice(0, 200));
    if (counting) {
      msgs++;
      bytes += raw.length;
      lastMsg = now;
    }
    if (o.t === "delta") {
      if (counting) got++;
      const m = /@(\d{13})\d*@/.exec(o.text ?? "");
      if (m && counting) lag.push(now - Number(m[1]));
    }
    for (const c of o.items ?? []) seen.push(c);
    if (o.t === "reply") replies.set(Number(o.id), now);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => {
    const i = ++id;
    ws.send(W.encode(JSON.stringify({ id: i, m, p })));
    return i;
  };
  const until = async <T>(ms: number, f: () => T | undefined) => {
    const end = Date.now() + ms;
    for (;;) {
      const v = f();
      if (v) return v;
      if (Date.now() > end) return undefined;
      await sleep(20);
    }
  };
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  for (let i = 0; i < n; i++) send("thread.create", { project: pc.id, title: `load ${i}`, env: "local" });
  const ths = await until(20000, () => {
    const t = seen.filter((c) => c.$ === "ThreadCreated");
    return t.length >= n ? t : undefined;
  });
  if (!ths) throw new Error("threads not created");
  const ping = async () => {
    const t0 = performance.now();
    const i = send("load.ping", {});
    await until(30000, () => replies.get(i));
    return performance.now() - t0;
  };
  for (let i = 0; i < 5; i++) await ping();
  counting = true;
  const c0 = cpuTicks(proc.pid);
  const t0 = Date.now();
  for (const t of ths) send("turn.start", { thread: t.id, text: "stream for the load test" });
  const rtt: number[] = [];
  let rss = 0;
  while (Date.now() - t0 < secs * 1000) {
    rtt.push(await ping());
    rss = Math.max(rss, rssKb(proc.pid));
    await sleep(200);
  }
  const busyMs = Date.now() - t0;
  const c1 = cpuTicks(proc.pid);
  // what was still queued: wait until no message came for 1.5 s
  const tEnd = Date.now();
  await until(120000, () => (Date.now() - lastMsg > 1500 ? true : undefined));
  const drain = Math.max(0, lastMsg - tEnd);
  const perf = await (await fetch(`http://127.0.0.1:${port}/debug/perf`)).text();
  ws.close();
  proc.kill();
  await proc.exited;
  const emitted = n * secs * 10 * Math.max(1, Math.round(hz / 10));
  const ws = seen.filter((c) => c.$ === "WorkSet").length;
  return { n, rtt, lag, emitted, got, msgs, bytes, cpu: ((c1 - c0) * 10) / busyMs * 100, rss, drain, perf, ws };
}

const results: Result[] = [];
try {
  for (const n of counts) {
    const r = await run(n);
    results.push(r);
    console.log(`\n== ${n} agents, ${secs} s ==`);
    console.log(`rtt ms      p50 ${f1(pct(r.rtt, 50))}  p90 ${f1(pct(r.rtt, 90))}  p99 ${f1(pct(r.rtt, 99))}  max ${f1(Math.max(...r.rtt))}  (${r.rtt.length} pings)`);
    console.log(`delta lag   p50 ${f1(pct(r.lag, 50))}  p90 ${f1(pct(r.lag, 90))}  p99 ${f1(pct(r.lag, 99))}  max ${f1(Math.max(...r.lag))}`);
    console.log(`deltas      emitted ${r.emitted} (${f1(r.emitted / secs)}/s)  received ${r.got}  drain after end ${r.drain} ms`);
    console.log(`client      ${r.msgs} msgs (${f1(r.msgs / secs)}/s), ${f1(r.bytes / 1024 / secs)} KB/s`);
    if (progress) console.log(`progress    ${r.ws} WorkSet writes for ${r.n * secs * 10} lines sent (${f1(r.ws / r.n / secs)} per thread per s; at most 0.2 plus the starts and ends)`);
    console.log(`hub         cpu ${f1(r.cpu)}%  rss ${Math.round(r.rss / 1024)} MB`);
    const lines = r.perf.split("\n");
    console.log(lines.slice(0, perfRows + 2).join("\n"));
  }
  console.log("\nagents  rtt p50/p99/max ms   lag p50/p99 ms   deltas/s in->out   hub cpu%");
  for (const r of results)
    console.log(`${String(r.n).padStart(6)}  ${f1(pct(r.rtt, 50))}/${f1(pct(r.rtt, 99))}/${f1(Math.max(...r.rtt))}`.padEnd(34) +
      `${f1(pct(r.lag, 50))}/${f1(pct(r.lag, 99))}`.padEnd(17) + `${f1(r.emitted / secs)}->${f1(r.got / secs)}`.padEnd(19) + f1(r.cpu));
} finally {
  if (!keep) rmSync(root, { recursive: true, force: true });
  else console.log(`kept ${root}`);
}
