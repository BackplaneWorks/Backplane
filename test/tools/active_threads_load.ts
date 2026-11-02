// Isolated Linux live-hub regression; never attaches to an existing hub.
// bun test/tools/active_threads_load.ts --bin build/backplane-serve --out /tmp/active.json
// Run the SAME checkout's fixture against baseline/after binaries under the
// parent's bend lock or on its remote runner. This program never builds.
import { chmodSync, createReadStream, existsSync, mkdirSync, mkdtempSync, readFileSync, readlinkSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { createHash, randomBytes } from "node:crypto";
import { cpus, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import assert from "node:assert/strict";
import { cbor, plain } from "../../src/web/wire.js";
import { ThinViews } from "../../src/web/thin.js";

const HELP = `Usage: bun test/tools/active_threads_load.ts --bin BINARY --out REPORT.json
  --secs 20           mixed streaming + tool commits, measured seconds (>=10)
  --quiet-secs 10     pure streaming, measured seconds (>=5)
  --metadata 1000     settled thread metadata, spread over 25-thread projects
  --hz 40            deltas/second PER THREAD (40..100)
  --delta-bytes 96    ASCII bytes/delta (64..256)
  --cycle-ms 1000     mixed phase tool/message cycle (500..1000)
  --warmup-secs 3     excluded settling interval before each phase (>=3)
  --max-cpu 80        hub CPU%, 100 = one full core (0 < limit <=100)
  --keep             retain isolated home, receipts and stderr
  --self-test        cheap codec/measurement guard tests; starts no hub
Twenty active threads plus one idle foreground thread; no real agents.
Nonzero exit on invalid workload, any failed assertion, or unavailable CPU.
--out is written even on failure. Baseline failures still produce measurements.`;
type Row = Record<string, any>;
const args = process.argv.slice(2), options = new Map<string, string>();
const flags = new Set(["--keep", "--help", "--self-test"]);
const valued = new Set(["--bin", "--out", "--secs", "--quiet-secs", "--metadata", "--hz", "--delta-bytes", "--cycle-ms", "--warmup-secs", "--max-cpu"]);
for (let i = 0; i < args.length; i++) {
  const key = args[i];
  if ((!flags.has(key) && !valued.has(key)) || options.has(key)) throw Error(`Unknown/duplicate option ${key}\n${HELP}`);
  if (flags.has(key)) options.set(key, "1");
  else { const value = args[++i]; if (!value || value.startsWith("--")) throw Error(`Missing value for ${key}`); options.set(key, value); }
}
if (options.has("--help")) { console.log(HELP); process.exit(0); }
const number = (key: string, fallback: number, min: number, max: number) => {
  const n = Number(options.get(key) ?? fallback);
  if (!Number.isFinite(n) || n < min || n > max) throw Error(`${key}: expected ${min}..${max}`);
  return n;
};
const params = { threads: 20, secs: number("--secs", 20, 10, 120), quietSecs: number("--quiet-secs", 10, 5, 30),
  metadata: number("--metadata", 1000, 0, 10000),
  hz: number("--hz", 40, 40, 100), deltaBytes: number("--delta-bytes", 96, 64, 256),
  cycleMs: number("--cycle-ms", 1000, 500, 1000), warmupSecs: number("--warmup-secs", 3, 3, 30),
  maxCpuPercent: number("--max-cpu", 80, 0.01, 100), minLoadFraction: 0.8, maxGapMs: 2000,
  ownNoticesPerSecond: 20, sidebarNoticesPerSecond: 2, requestTimeoutMs: 5000, transitionTimeoutMs: 10000 };
if (!Number.isInteger(params.deltaBytes)) throw Error("--delta-bytes must be an integer");
if (!Number.isInteger(params.metadata)) throw Error("--metadata must be an integer");
const ids = Array.from({ length: params.threads }, (_, i) => `active-${String(i).padStart(2, "0")}`);
// Metadata only: no historical conversations, pending work or agent sessions.
// Forty projects at the default scale exercise the complete catalog and its
// settled shelves while the same twenty producers supply measured live load.
function metadataFixture(projectRoot: string, count: number): Row[] {
  const rows: Row[] = [];
  for (let i = 0; i < count; i++) {
    const group = Math.floor(i / 25), project = `catalog-${group}`, id = `settled-${String(i).padStart(5, "0")}`;
    if (i % 25 === 0) rows.push({ $: "ProjectCreated", id: project, title: `Hardware project ${group}: controller and enclosure`, root: join(projectRoot, project), at: 1 });
    rows.push({ $: "ThreadCreated", id, project, title: `Review ${i}: connector clearance, board layout and enclosure fit`, env: "local", branch: "", worktree: "", provider: "claude", model: "fixture", at: 2 + i });
    rows.push({ $: "TurnChanged", thread: id, state: "completed", at: 10002 + i });
    rows.push({ $: "ThreadSettled", id, manual: true, at: 20002 + i });
  }
  return rows;
}
const sleep = (ms: number) => new Promise<void>(r => setTimeout(r, ms));
const percentile = (xs: number[], p: number) => xs.length ? [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * p))] : null;
const maxGap = (times: number[], start: number, end: number) => {
  let last = start, max = 0;
  for (const t of times) { max = Math.max(max, t - last); last = t; }
  return Math.max(max, end - last);
};
// Sliding, half-open windows: a 50ms cadence fits 20 notices in [t,t+1s).
// Allow 25ms arrival jitter when counting, not an extra notice per window.
const peakNotices = (times: number[]) => {
  let left = 0, peak = 0;
  for (let right = 0; right < times.length; right++) {
    while (times[right] - times[left] >= 975) left++;
    peak = Math.max(peak, right - left + 1);
  }
  return peak;
};
const streamHealthy = (times: number[], start: number, end: number, minHz: number) =>
  times.length >= (end - start) / 1000 * minHz && maxGap(times, start, end) <= params.maxGapMs;
type Commit = { at: number; thread: string; kind: string; key: string };
const watchdogCommit = (e: Commit) => e.kind === "SettingSet" && e.key.startsWith("liveness.") && ids.includes(e.key.slice(9));
// Watchdog.line writes liveness.<thread> every five seconds even when an
// agent only streams. Those real metadata commits legitimately wake sidebar
// views. Test live-only isolation between these observed commits, while
// retaining all notices/requests and CPU in the full-phase measurements.
const metadataWindow = { beforeMs: 100, afterMs: 1250, minimumCleanFraction: 0.25 };
function quietIsolation(start: number, end: number, commits: Commit[]) {
  const windows: [number, number][] = [];
  for (const e of commits.filter(watchdogCommit).sort((a, b) => a.at - b.at)) {
    const left = Math.max(start, e.at - metadataWindow.beforeMs), right = Math.min(end, e.at + metadataWindow.afterMs);
    if (right <= left) continue;
    const last = windows.at(-1);
    if (last && left <= last[1]) last[1] = Math.max(last[1], right);
    else windows.push([left, right]);
  }
  return { windows, cleanMs: end - start - windows.reduce((n, [a, b]) => n + b - a, 0),
    clean: (at: number) => at >= start && at < end && !windows.some(([a, b]) => at >= a && at < b) };
}
function perfDiff(before: Row, after: Row) {
  if (!before.data || !after.data) return { error: "Performance snapshot unavailable", spans: [], counts: [], gauges: [], regressions: [] };
  const previous = new Map<string, Row>(before.data.rows.map((r: Row) => [r.name, r]));
  const spans: Row[] = [], counts: Row[] = [], gauges: Row[] = [], regressions: string[] = [];
  for (const row of after.data.rows) {
    const old = previous.get(row.name);
    if (row.kind === "gauge") {
      gauges.push({ name: row.name, before: old?.last ?? null, after: row.last, change: old ? row.last - old.last : null });
      continue;
    }
    const n = row.n - (old?.n ?? 0), sum = row.sum - (old?.sum ?? 0);
    if (n < 0 || sum < 0 || old && old.kind !== row.kind) regressions.push(row.name);
    if (n === 0 && sum === 0) continue;
    if (row.kind === "span") spans.push({ name: row.name, n, totalUs: sum, totalMs: sum / 1000, meanMs: n > 0 ? sum / n / 1000 : null });
    else counts.push({ name: row.name, kind: row.kind, n, sum, mean: n > 0 ? sum / n : null });
  }
  return { spans: spans.sort((a, b) => b.totalUs - a.totalUs), counts: counts.sort((a, b) => b.n - a.n), gauges, regressions,
    bracketStart: before.startedAt, bracketEnd: after.finishedAt, bracketMs: after.finishedAt - before.startedAt };
}
const parseCpu = (stat: string) => {
  const f = stat.slice(stat.lastIndexOf(") ") + 2).trim().split(/\s+/);
  const ticks = Number(f[11]) + Number(f[12]), birth = Number(f[19]);
  if (!Number.isFinite(ticks) || !Number.isFinite(birth) || birth <= 0) throw Error("Invalid /proc PID stat CPU sample");
  return { ticks, birth };
};
// Hub CBOR subset: tag 7 text for numbers beyond uint32 (thin RPC ids are
// 48-bit); dictionaries are optional on requests. No generated wire build.
function encode(value: any): Uint8Array {
  const out: number[] = [];
  const head = (major: number, n: number) => {
    if (n < 24) out.push(major << 5 | n);
    else if (n < 256) out.push(major << 5 | 24, n);
    else if (n < 65536) out.push(major << 5 | 25, n >> 8, n & 255);
    else out.push(major << 5 | 26, n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255);
  };
  const put = (v: any) => {
    if (v === null) out.push(246);
    else if (typeof v === "boolean") out.push(v ? 245 : 244);
    else if (typeof v === "number") { out.push(199); put(String(v)); }
    else if (typeof v === "string") { const b = new TextEncoder().encode(v); head(3, b.length); for (const byte of b) out.push(byte); }
    else if (Array.isArray(v)) { head(4, v.length); for (const x of v) put(x); }
    else { const entries = Object.entries(v); head(5, entries.length); for (const [k, x] of entries) { put(k); put(x); } }
  };
  put(value); return new Uint8Array(out);
}
const dictionarySource = readFileSync(resolve(import.meta.dir, "../../src/core/cbor.bend"), "utf8");
const dictionary = (name: string): string[] => {
  const at = dictionarySource.indexOf(`def Cbor.${name}()`);
  if (at < 0) throw Error(`Missing CBOR ${name}`);
  return JSON.parse(dictionarySource.slice(dictionarySource.indexOf("[", at), dictionarySource.indexOf("]", at) + 1));
};
const keys = dictionary("keys"), words = dictionary("words");

// The fixture records every successfully written delta, rather than assuming
// timer rate == offered load. All twenty producers run until the controller
// says stop, so a slow startup cannot silently shorten the measured load.
const standin = String.raw`import { appendFileSync, readFileSync, writeFileSync, writeSync } from 'node:fs';
import { createInterface } from 'node:readline';
if (process.argv.includes('--version')) { console.log('2.1.286 (fixture)'); process.exit(0); }
const root = process.env.ACTIVE_ROOT, hz = Number(process.env.ACTIVE_HZ), size = Number(process.env.ACTIVE_BYTES);
const cycle = Number(process.env.ACTIVE_CYCLE), sid = 'fixture-' + process.pid;
const emit = o => writeSync(1, JSON.stringify({ ...o, session_id: sid }) + '\n');
const phase = () => JSON.parse(readFileSync(root + '/control.json', 'utf8')).phase;
let running = false;
async function run(input) {
  const id = /active-load-thread=(active-\d+)/.exec(JSON.stringify(input))?.[1];
  if (!id || running) throw Error('missing/duplicate fixture turn');
  running = true;
  writeFileSync(root + '/' + id + '.pid', String(process.pid));
  emit({ type: 'system', subtype: 'init', cwd: process.cwd(), tools: ['Bash'], model: 'fixture', permissionMode: 'default' });
  let seq = 0, block = 0, text = '', lastCycle = Date.now(), next = performance.now();
  const start = () => {
    block++; text = '';
    emit({ type: 'stream_event', event: { type: 'message_start', message: { id: id + '-' + block, role: 'assistant', content: [], usage: { input_tokens: 10 } } } });
    emit({ type: 'stream_event', event: { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } } });
  };
  start();
  for (;;) {
    const mode = phase();
    if (mode === 'stop') break;
    if (mode !== 'mixed' && mode !== 'live') throw Error('unknown producer phase');
    const at = Date.now();
    const delta = ('~bp-load:' + id + ':' + (++seq) + ':' + at + ':' + mode + '~').padEnd(size - 1, 'x') + ' ';
    emit({ type: 'stream_event', event: { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: delta } } });
    appendFileSync(root + '/' + id + '.jsonl', JSON.stringify({ at, seq, phase: mode, bytes: delta.length }) + '\n');
    text += delta;
    if (mode === 'mixed' && at - lastCycle >= cycle) {
      emit({ type: 'stream_event', event: { type: 'content_block_stop', index: 0 } });
      emit({ type: 'assistant', message: { id: id + '-' + block, role: 'assistant', content: [{ type: 'text', text }], usage: { input_tokens: 10, output_tokens: seq } } });
      const tool = id + '-tool-' + block;
      emit({ type: 'assistant', message: { id: tool, role: 'assistant', content: [{ type: 'tool_use', id: tool, name: 'Bash', input: { command: 'echo synthetic-' + block, description: 'Synthetic tool cycle ' + block } }] } });
      emit({ type: 'user', message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: tool, content: 'synthetic result ' + block, is_error: false }] } });
      lastCycle = at; start();
    }
    // Never catch up with a burst after a stall: count actual production.
    next = Math.max(next + 1000 / hz, performance.now());
    await new Promise(r => setTimeout(r, Math.max(1, next - performance.now())));
  }
  process.exit(0);
}
createInterface({ input: process.stdin }).on('line', line => {
  const o = JSON.parse(line);
  if (o.type === 'user') run(o).catch(e => { console.error(e); process.exit(1); });
});
`;

if (options.has("--self-test")) {
  const catalog = metadataFixture("/fixture/project", 1000);
  assert.equal(catalog.filter(r => r.$ === "ProjectCreated").length, 40);
  assert.equal(catalog.filter(r => r.$ === "ThreadCreated").length, 1000);
  assert.equal(catalog.filter(r => r.$ === "ThreadSettled" && r.manual).length, 1000);
  assert.equal(catalog.filter(r => r.$ === "TurnChanged" && r.state === "completed").length, 1000);
  assert.equal(new Set(catalog.filter(r => r.$ === "ThreadCreated").map(r => r.id)).size, 1000);
  assert(catalog.filter(r => r.$ === "ThreadCreated").every(r => !ids.includes(r.id) && r.id !== "quiet"));
  assert.deepEqual(metadataFixture("/fixture/project", 0), []);
  assert.equal(metadataFixture("/fixture/project", 26).filter(r => r.$ === "ProjectCreated").length, 2);
  assert.equal(metadataFixture("/fixture/project", params.metadata).filter(r => r.$ === "ThreadCreated").length, params.metadata);
  const isolation = quietIsolation(0, 10000, [5000, 5100].map(at => ({ at, thread: ids[0], kind: "SettingSet", key: "liveness." + ids[0] })));
  assert.equal(isolation.cleanMs, 8550);
  assert(!isolation.clean(5000) && isolation.clean(7000));
  assert(!watchdogCommit({ at: 5000, thread: ids[0], kind: "MessagePosted", key: "liveness." + ids[0] }));
  assert(!watchdogCommit({ at: 5000, thread: "", kind: "SettingSet", key: "liveness.unknown" }));
  const diff = perfDiff({ startedAt: 10, data: { rows: [{ name: "hub.loop", kind: "span", n: 10, sum: 10000 }, { name: "events", kind: "value", n: 2, sum: 4 }] } },
    { finishedAt: 1010, data: { rows: [{ name: "hub.loop", kind: "span", n: 15, sum: 30000 }, { name: "events", kind: "value", n: 5, sum: 13 }, { name: "new", kind: "span", n: 1, sum: 1000 }] } });
  assert.deepEqual(diff.spans[0], { name: "hub.loop", n: 5, totalUs: 20000, totalMs: 20, meanMs: 4 });
  assert.equal(diff.counts[0].sum, 9); assert.equal(diff.counts[0].n, 3); assert.equal(diff.spans[1].n, 1);
  assert.equal(diff.bracketMs, 1000); assert(perfDiff({}, {}).error);
  const value = { id: 2 ** 47 + 17, m: "view.open", p: { b: true, n: null, a: [1, "utf8 ✓", false] } };
  assert.deepEqual(plain(cbor(encode(value), keys, words)), value);
  assert.equal(peakNotices(Array.from({ length: 81 }, (_, i) => i * 50)), 20);
  assert.equal(peakNotices(Array.from({ length: 10 }, (_, i) => i * 500)), 2);
  assert.equal(peakNotices(Array.from({ length: 30 }, (_, i) => i * 25)), 30);
  assert(!streamHealthy([], 0, 10000, 1));
  assert(!streamHealthy(Array.from({ length: 100 }, (_, i) => i * 10), 0, 10000, 1));
  assert(!streamHealthy(Array.from({ length: 10 }, (_, i) => 3000 + i * 700), 0, 10000, 1));
  assert(streamHealthy(Array.from({ length: 400 }, (_, i) => i * 25), 0, 10000, 32));
  assert.throws(() => parseCpu("bad"));
  assert(parseCpu(readFileSync(`/proc/${process.pid}/stat`, "utf8")).ticks >= 0);
  // Exercise one generated producer briefly, without a hub or real agent.
  new Bun.Transpiler({ loader: "js" }).transformSync(standin);
  const temp = mkdtempSync(join(tmpdir(), "bp-active-selftest-"));
  let child: ReturnType<typeof Bun.spawn> | undefined;
  const phase = (value: string) => {
    writeFileSync(join(temp, "next"), JSON.stringify({ phase: value }));
    renameSync(join(temp, "next"), join(temp, "control.json"));
  };
  try {
    writeFileSync(join(temp, "producer.js"), standin); phase("mixed");
    child = Bun.spawn([process.execPath, join(temp, "producer.js")], {
      env: { ACTIVE_ROOT: temp, ACTIVE_HZ: "40", ACTIVE_BYTES: "96", ACTIVE_CYCLE: "100" },
      stdin: "pipe", stdout: "pipe", stderr: "pipe" });
    const output = new Response(child.stdout).text(), errors = new Response(child.stderr).text();
    child.stdin!.write(JSON.stringify({ type: "user", message: { content: "active-load-thread=active-00" } }) + "\n");
    const deadline = performance.now() + 2000, receipt = join(temp, "active-00.jsonl");
    while (!existsSync(receipt) && performance.now() < deadline) await sleep(10);
    assert(existsSync(receipt), "synthetic producer starts");
    await sleep(150); phase("live"); await sleep(150); phase("stop");
    const timer = setTimeout(() => child!.kill("SIGKILL"), 2000);
    try { assert.equal(await child.exited, 0, await errors); } finally { clearTimeout(timer); }
    const emitted = (await output).trim().split("\n").map(line => JSON.parse(line));
    const produced = readFileSync(receipt, "utf8").trim().split("\n").map(line => JSON.parse(line));
    const deltas = emitted.filter(o => o.event?.type === "content_block_delta");
    assert.equal(deltas.length, produced.length);
    assert(produced.some(r => r.phase === "mixed") && produced.some(r => r.phase === "live"));
    assert(produced.every((r, i) => r.seq === i + 1 && r.bytes === 96));
    assert(deltas.every((o, i) => o.event.delta.text.length === 96 &&
      new RegExp('~bp-load:active-00:' + produced[i].seq + ':' + produced[i].at + ':' + produced[i].phase + '~').test(o.event.delta.text)));
    assert(emitted.some(o => o.message?.content?.some((c: Row) => c.type === "tool_use")));
    const firstLive = emitted.findIndex(o => o.event?.delta?.text?.includes(":live~"));
    assert(firstLive >= 0 && emitted.slice(firstLive).every(o => o.type === "stream_event" && o.event.type === "content_block_delta"),
      "producer emits no committed messages/tool calls/results after the live phase begins");
  } finally {
    if (child && child.exitCode === null) { child.kill("SIGKILL"); await child.exited; }
    rmSync(temp, { recursive: true, force: true });
  }
  console.log("ok 1000 settled metadata/40 projects, codec, CPU samples, perf diffs, notice/liveness guards, producer phase switch/receipts (no hub started)");
  process.exit(0);
}

if (process.platform !== "linux") throw Error("Linux /proc is required for actual hub process CPU measurement");
if (!options.has("--bin") || !options.has("--out")) throw Error(HELP);
const bin = resolve(options.get("--bin")!), out = resolve(options.get("--out")!);
if (!existsSync(bin)) throw Error(`Missing --bin ${bin}; build separately`);
if (out === bin) throw Error("--out must not overwrite --bin");
const root = mkdtempSync(join(tmpdir(), "bp-active-threads-"));
const home = join(root, "home"), agents = join(root, "agents"), user = join(root, "user"), project = join(root, "project");
for (const p of [home, agents, user, project, join(root, "locks")]) mkdirSync(p);
const token = randomBytes(24).toString("hex");
writeFileSync(join(home, "token"), token, { mode: 0o600 });
const control = (phase: string) => { writeFileSync(join(root, "control.next"), JSON.stringify({ phase })); renameSync(join(root, "control.next"), join(root, "control.json")); };
control("mixed");
writeFileSync(join(agents, "producer.js"), standin);
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
writeFileSync(join(agents, "claude"), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(join(agents, "producer.js"))} "$@"\n`);
chmodSync(join(agents, "claude"), 0o755);
for (const name of ["codex", "grok", "muse", "opencode", "antigravity", "agy", "pi", "omp", "gh", "tailscale", "notify-send"]) {
  writeFileSync(join(agents, name), "#!/bin/sh\nexit 127\n"); chmodSync(join(agents, name), 0o755);
}
const event = ($: string, fields: Row) => ({ $, ...fields, at: 1 });
const metadata = metadataFixture(project, params.metadata);
for (const row of metadata) if (row.$ === "ProjectCreated") mkdirSync(row.root, { recursive: true });
const fixture = [...metadata, event("ProjectCreated", { id: "load", title: "Synthetic active threads", root: project }),
  ...[...ids, "quiet"].map(id => event("ThreadCreated", { id, project: "load", title: id, env: "local", branch: "", worktree: project, provider: "claude", model: "fixture" })),
  ...["restart.continue", "fallback.on", "telemetry.share"].map(key => event("SettingSet", { key, value: "off" })),
  ...["onboard.done", "onboard.uber"].map(key => event("SettingSet", { key, value: "1" })),
  event("SettingSet", { key: "sidebar.view", value: "active" })];
writeFileSync(join(home, "events.jsonl"), fixture.map(x => JSON.stringify(x)).join("\n") + "\n");
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true);
if (port === 3787) throw Error("Refusing live port");
const url = `http://127.0.0.1:${port}`;
type Frame = { at: number; kind: string; bytes: number; events: number };
type Mark = { at: number; emitted: number; seq: number; thread: string; phase: string };
type Request = { at: number; method: string; bytes: number; done?: number };
const clients: Client[] = [], marks: Mark[] = [], events: Commit[] = [];
const latestDelivered = new Map<string, Mark>();
const checks: { name: string; ok: boolean; actual: any }[] = [], phases: Row[] = [];
let hub: ReturnType<typeof Bun.spawn> | undefined, fault: Error | undefined, finishing = false;
const check = (name: string, ok: boolean, actual: any) => { checks.push({ name, ok, actual }); console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${JSON.stringify(actual)}`); };
const guard = () => {
  if (fault) throw fault;
  if (hub && hub.exitCode !== null) throw Error(`Owned hub exited ${hub.exitCode}`);
};
async function until<T>(label: string, fn: () => T | undefined | false, ms = 30000): Promise<T> {
  const end = performance.now() + ms;
  do { guard(); const v = fn(); if (v !== undefined && v !== false) return v as T; await sleep(25); } while (performance.now() < end);
  throw Error(`Timed out: ${label}`);
}
const parseMarks = (text: string, at: number): Mark[] => Array.from(text.matchAll(/~bp-load:(active-\d+):(\d+):(\d+):(mixed|live)~/g),
  m => ({ at, thread: m[1], seq: Number(m[2]), emitted: Number(m[3]), phase: m[4] }));
async function takePerf(): Promise<Row> {
  const startedAt = Date.now();
  try {
    const response = await fetch(url + "/debug/perf.json", { signal: AbortSignal.timeout(params.requestTimeoutMs) });
    if (!response.ok) throw Error(`HTTP ${response.status}`);
    const data = await response.json() as Row;
    if (data.unit !== "us" || !Array.isArray(data.rows) || !data.rows.length || !data.rows.every((r: Row) =>
      typeof r.name === "string" && ["span", "value", "gauge"].includes(r.kind) && Number.isFinite(r.n) && Number.isFinite(r.sum))) throw Error("Invalid performance rows");
    return { startedAt, finishedAt: Date.now(), data };
  } catch (error) { return { startedAt, finishedAt: Date.now(), error: String(error) }; }
}

class Client {
  ws: WebSocket;
  frames: Frame[] = [];
  notices: number[] = [];
  requests: Request[] = [];
  pending = new Map<number, { request: Request; resolve: (v: Row) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  greeting?: Row;
  joined = false;
  views?: ThinViews;
  session: Row;
  shownMarks: Mark[] = [];
  n = 0;
  ready = false;
  constructor(readonly name: string, readonly target?: Row) {
    clients.push(this);
    this.session = { sel: target?.kind === "thread" ? target.id : "", rid: 1, drafts: {}, open: {}, info: {}, adding: false, panel: false };
    this.ws = new WebSocket(`${url.replace("http:", "ws:")}/ws?token=${token}${target ? "&view=1&frontend=1" : ""}`);
    this.ws.binaryType = "arraybuffer";
    if (target) this.views = new ThinViews({ now: () => performance.now(), sidebarRegions: true, viewDelta: true, send: (_hub: string, r: Row) => r, command: (c: Row) => c });
    this.ws.onerror = () => { if (!finishing) fault = Error(`${name}: websocket error`); };
    this.ws.onclose = () => { if (!finishing) fault = Error(`${name}: unexpected websocket close`); };
    this.ws.onmessage = e => { try { this.receive(e); } catch (error) { fault = error as Error; } };
  }
  receive(e: MessageEvent) {
    const at = Date.now(), raw = typeof e.data === "string" ? new TextEncoder().encode(e.data) : new Uint8Array(e.data as ArrayBuffer);
    const f = typeof e.data === "string" ? JSON.parse(e.data) : plain(cbor(raw, keys, words));
    if (!f || typeof f.t !== "string") throw Error(`${this.name}: undecodable frame`);
    this.frames.push({ at, bytes: raw.length, kind: f.t, events: f.items?.length ?? 0 });
    if (f.t === "view" && this.target) { if (this.greeting) throw Error("Duplicate greeting"); this.greeting = f; }
    if (!this.target && f.t === "log") this.joined = true;
    if (!this.target && f.t === "delta") {
      const parsed = parseMarks(f.text ?? "", at);
      if (parsed.some(m => m.thread !== f.thread)) throw Error("Delta thread/marker mismatch");
      marks.push(...parsed);
      for (const mark of parsed) latestDelivered.set(mark.thread, mark);
    }
    if (!this.target && f.t === "changes") for (const item of f.items ?? []) events.push({ at,
      thread: item.thread ?? (item.$ === "SettingSet" && String(item.key).startsWith("liveness.") ? String(item.key).slice(9) : ""),
      kind: item.$, key: item.key ?? "" });
    if (this.target && ["delta", "changes", "log", "live"].includes(f.t)) throw Error(`${this.name}: leaked legacy ${f.t} frame`);
    if (f.t === "view.changed") {
      this.notices.push(at);
      if (this.ready) this.issue(this.views!.changed("fixture", f, this.session, { id: "", offset: 0 }));
    }
    if (f.t === "reply") {
      const pending = this.pending.get(Number(f.id));
      if (!pending) throw Error(`${this.name}: unrequested reply ${f.id}`);
      clearTimeout(pending.timer); this.pending.delete(Number(f.id)); pending.request.done = at;
      if (!f.ok) { pending.reject(Error(`${this.name}: rejected request ${JSON.stringify(f)}`)); return; }
      pending.resolve(f);
    }
  }
  send(r: Row): Promise<Row> {
    const raw = encode(r), request = { at: Date.now(), method: r.m, bytes: raw.length };
    this.requests.push(request);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(r.id); reject(Error(`${this.name}: ${r.m} timed out`)); }, params.requestTimeoutMs);
      this.pending.set(r.id, { request, resolve, reject, timer }); this.ws.send(raw);
    });
  }
  issue(call: Row): Promise<void> {
    if (call.failed) { fault = Error(`${this.name}: ${call.failed}`); return Promise.resolve(); }
    if (!call.id) return Promise.resolve(); // production ThinViews coalesces
    return this.send(call.cmds[0]).then(raw => {
      const taken = this.views!.reply("fixture", raw);
      if (!taken?.current || !taken.remoteReady || !taken.shown) throw Error(`${this.name}: invalid thin reply ${JSON.stringify(taken).slice(0, 500)}`);
      this.session = taken.session;
      this.shownMarks.push(...parseMarks(JSON.stringify(taken.shown), Date.now()));
      if (taken.refreshDue) return this.issue(this.views!.request("fixture", "view.update", { session: this.session, anchor: { id: "", offset: 0 } }));
    }).catch(error => { if (!finishing) fault = error; });
  }
  async open() {
    await until(`${this.name} socket open`, () => this.ws.readyState === WebSocket.OPEN);
    if (!this.target) { await until("observer initial log", () => this.joined); return; }
    const greeting = await until(`${this.name} greeting`, () => this.greeting);
    if (greeting.renderer !== "web" || !this.views!.connect("fixture", greeting).connected) throw Error(`${this.name}: expected view-web`);
    await this.issue(this.views!.select("fixture", this.target, this.session, this.name)); guard(); this.ready = true;
  }
}

const receipts = (id: string): Row[] => {
  const path = join(root, `${id}.jsonl`);
  if (!existsSync(path)) return [];
  const text = readFileSync(path, "utf8");
  return text.slice(0, text.lastIndexOf("\n") + 1).split("\n").filter(Boolean).map(line => JSON.parse(line));
};
const hashFile = async (path: string) => { const h = createHash("sha256"); for await (const b of createReadStream(path)) h.update(b); return h.digest("hex"); };
const sourceHashes = Object.fromEntries(["test/tools/active_threads_load.ts", "src/core/cbor.bend", "src/web/wire.js", "src/web/thin.js",
  "src/web/sidebar-region.js", "src/web/view-delta.js", "src/web/terminal-input.js", "src/web/model-menu-input.js", "src/web/startup.js"]
  .map(path => [path, createHash("sha256").update(readFileSync(resolve(import.meta.dir, "../..", path))).digest("hex")]));
const report: Row = { schema: 1, fixture: "active-threads-v2", params, bin, out, root, port, sourceHashes,
  quietIsolationPolicy: metadataWindow,
  catalog: { settledThreads: params.metadata, activeThreads: ids.length, quietThreads: 1,
    totalThreads: params.metadata + ids.length + 1, projects: Math.ceil(params.metadata / 25) + 1, initialEvents: fixture.length },
  fixtureSha256: createHash("sha256").update(standin + JSON.stringify(params) + JSON.stringify(sourceHashes)).digest("hex"),
  host: { platform: process.platform, arch: process.arch, cpus: cpus().length, cpuModel: cpus()[0]?.model, bun: Bun.version },
  cpuScope: "Owned hub PID only, utime+stime across all its threads; excludes synthetic agents/client. 100% = one full core; no host-core normalization.",
  checks, phases, startedAt: new Date().toISOString() };
try {
  report.binarySha256 = await hashFile(bin);
  const clk = Bun.spawnSync(["getconf", "CLK_TCK"], { stdout: "pipe", stderr: "pipe" });
  const tickHz = Number(new TextDecoder().decode(clk.stdout).trim());
  if (clk.exitCode !== 0 || !Number.isFinite(tickHz) || tickHz <= 0) throw Error("Cannot read CLK_TCK");
  report.cpuTickHz = tickHz;
  // Deliberately do not inherit auth, peer, profile or runtime selectors.
  hub = Bun.spawn([bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    cwd: project, env: { HOME: user, USERPROFILE: user, XDG_CONFIG_HOME: join(user, "config"), XDG_CACHE_HOME: join(user, "cache"), XDG_DATA_HOME: join(user, "data"),
      CODEX_HOME: join(user, "codex"), CLAUDE_CONFIG_DIR: join(user, "claude"), GEMINI_HOME: join(user, "gemini"),
      PATH: `${agents}:${dirname(process.execPath)}:/usr/bin:/bin`, TMPDIR: root, DISPLAY: "", WAYLAND_DISPLAY: "", CI: "1", DO_NOT_TRACK: "1",
      BACKPLANE_AGENTS: agents, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_TELEMETRY: "off", BEND_NO_TELEMETRY: "1", BACKPLANE_GOOGLE_BAKE: "0",
      BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_WAKE_TICK_MS: "60000", BACKPLANE_LOCK_DIR: join(root, "locks"),
      ACTIVE_ROOT: root, ACTIVE_HZ: String(params.hz), ACTIVE_BYTES: String(params.deltaBytes), ACTIVE_CYCLE: String(params.cycleMs) },
    stdout: "ignore", stderr: Bun.file(join(root, "hub.stderr")) });
  report.hubPid = hub.pid;
  const readyEnd = performance.now() + 30000;
  for (;;) {
    guard();
    try { if ((await fetch(url + "/hello", { signal: AbortSignal.timeout(500) })).ok) break; } catch {}
    if (performance.now() > readyEnd) throw Error("Owned hub readiness timeout");
    await sleep(50);
  }
  report.cpuProcessExecutable = readlinkSync(`/proc/${hub.pid}/exe`);
  report.cpuProcessArgv = readFileSync(`/proc/${hub.pid}/cmdline`, "utf8").split("\0").filter(Boolean);
  // A non-exec shell launcher would leave us measuring the waiting shell.
  // Accept the hub executable itself or a Bun-interpreted hub script only.
  if (report.cpuProcessExecutable !== realpathSync(bin) &&
      !(report.cpuProcessExecutable === realpathSync(process.execPath) && report.cpuProcessArgv.includes(bin)))
    throw Error("--bin must execute the hub in the captured PID; non-exec launchers cannot provide actual hub CPU");
  const observer = new Client("observer"); await observer.open();
  const own = new Client("own", { kind: "thread", id: ids[0] }); await own.open();
  const quiet = new Client("quiet", { kind: "thread", id: "quiet" }); await quiet.open();
  const sidebar = new Client("sidebar", { kind: "recent", id: "" }); await sidebar.open();
  await Promise.all(ids.map(id => observer.send({ id: ++observer.n, m: "turn.start", p: { thread: id, text: `active-load-thread=${id}`, accountProfiles: 1, accountBinding: "" } })));
  await until("all 20 producers AND delivered streams", () => ids.every(id => receipts(id).length >= 10 && marks.filter(m => m.thread === id).length >= 10));

  const cpu = () => parseCpu(readFileSync(`/proc/${hub!.pid}/stat`, "utf8"));
  for (const [name, duration] of [["mixed", params.secs], ["live-only", params.quietSecs]] as const) {
    const mode = name === "mixed" ? "mixed" : "live", transitionAt = Date.now();
    control(mode);
    const warmEnd = performance.now() + params.warmupSecs * 1000;
    while (performance.now() < warmEnd) { guard(); await sleep(50); }
    const boundaryReady = () => ids.every(id => {
      const mark = latestDelivered.get(id);
      return mark?.phase === mode && mark.emitted >= transitionAt && Date.now() - mark.emitted <= params.maxGapMs;
    });
    const boundaryDeadline = performance.now() + params.transitionTimeoutMs;
    while (!boundaryReady() && performance.now() < boundaryDeadline) { guard(); await sleep(50); }
    const transition = { requestedAt: transitionAt, readyAt: Date.now(), elapsedMs: Date.now() - transitionAt, ready: boundaryReady(),
      lastDelivered: Object.fromEntries(ids.map(id => [id, latestDelivered.get(id)])) };
    check(`${name}: all 20 delivered streams crossed the phase boundary with <=2s lag`, transition.ready, transition);
    const perfBefore = await takePerf();
    const c0 = cpu(), mono0 = performance.now(), start = Date.now();
    console.log(`measuring ${name}: ${params.metadata} settled + 20 active, 20 x ${params.hz} deltas/s, ${duration}s, hub PID ${hub!.pid}`);
    while (performance.now() - mono0 < duration * 1000) { guard(); await sleep(50); }
    const c1 = cpu(), wallMs = performance.now() - mono0, end = Date.now(), seconds = wallMs / 1000;
    if (c0.birth !== c1.birth || c1.ticks < c0.ticks) throw Error("Hub PID changed or CPU counter regressed");
    const inWindow = (at: number) => at >= start && at < end;
    const phase: Row = { name, start, end, wallMs, transition, perfBefore, cpuTicks: c1.ticks - c0.ticks, cpuMs: (c1.ticks - c0.ticks) / tickHz * 1000,
      cpuPercent: (c1.ticks - c0.ticks) / tickHz / seconds * 100, threads: {}, clients: {} };
    phases.push(phase);
    phase.perfAfter = await takePerf(); phase.perfDiff = perfDiff(perfBefore, phase.perfAfter);
    check(`${name}: performance snapshots and monotonic span/count differences available`,
      !phase.perfDiff.error && phase.perfDiff.spans.length > 0 && phase.perfDiff.regressions.length === 0, phase.perfDiff.error ?? { spans: phase.perfDiff.spans.length, counts: phase.perfDiff.counts.length, regressions: phase.perfDiff.regressions });
    console.log(`${name}: heaviest phase spans (ms, count): ${phase.perfDiff.spans.slice(0, 8).map((s: Row) => `${s.name}=${s.totalMs.toFixed(1)}/${s.n}`).join(", ")}`);
    check(`${name}: nonzero hub CPU <= ${params.maxCpuPercent}% of one core`, phase.cpuPercent > 0 && phase.cpuPercent <= params.maxCpuPercent, phase.cpuPercent);
    for (const id of ids) {
      const produced = receipts(id).filter(r => inWindow(r.at));
      const delivered = marks.filter(m => m.thread === id && inWindow(m.at));
      const unique = [...new Map(delivered.map(m => [m.seq, m])).values()];
      const stat = { produced: produced.length, producedHz: produced.length / seconds, producedBytes: produced.reduce((n, r) => n + r.bytes, 0),
        delivered: unique.length, deliveredHz: unique.length / seconds, producedMaxGapMs: maxGap(produced.map(r => r.at), start, end),
        deliveredMaxGapMs: maxGap(unique.map(m => m.at), start, end), lagP99Ms: percentile(unique.map(m => m.at - m.emitted), 0.99),
        events: events.filter(e => e.thread === id && inWindow(e.at)).length,
        producedBytesPerSecond: produced.reduce((n, r) => n + r.bytes, 0) / seconds,
        eventsPerSecond: events.filter(e => e.thread === id && inWindow(e.at)).length / seconds,
        producedWrongPhase: produced.filter(r => r.phase !== mode).length, deliveredWrongPhase: unique.filter(m => m.phase !== mode).length };
      phase.threads[id] = stat;
      check(`${name}: ${id} adequate production and continuous delivery`,
        streamHealthy(produced.map(r => r.at), start, end, params.hz * params.minLoadFraction) &&
        streamHealthy(unique.map(m => m.at), start, end, params.hz * params.minLoadFraction) &&
        stat.lagP99Ms !== null && stat.lagP99Ms <= params.maxGapMs && stat.producedWrongPhase === 0 && stat.deliveredWrongPhase === 0 &&
        (name !== "mixed" || stat.events >= seconds * 0.5), stat);
    }
    for (const client of clients) {
      const frames = client.frames.filter(f => inWindow(f.at)), notices = client.notices.filter(inWindow), requests = client.requests.filter(r => inWindow(r.at));
      const byKind: Row = {}, requestsByMethod: Row = {};
      for (const f of frames) { const k = byKind[f.kind] ??= { frames: 0, bytes: 0, events: 0 }; k.frames++; k.bytes += f.bytes; k.events += f.events; }
      for (const r of requests) requestsByMethod[r.method] = (requestsByMethod[r.method] ?? 0) + 1;
      for (const k of Object.values(byKind) as Row[]) { k.framesPerSecond = k.frames / seconds; k.bytesPerSecond = k.bytes / seconds; k.eventsPerSecond = k.events / seconds; }
      const stat = { frames: frames.length, framesPerSecond: frames.length / seconds, bytes: frames.reduce((n, f) => n + f.bytes, 0),
        bytesPerSecond: frames.reduce((n, f) => n + f.bytes, 0) / seconds, eventsPerSecond: frames.reduce((n, f) => n + f.events, 0) / seconds,
        notices: notices.length, noticesPerSecond: notices.length / seconds, peakNotices975ms: peakNotices(notices),
        requests: requests.length, requestsPerSecond: requests.length / seconds, requestBytesPerSecond: requests.reduce((n, r) => n + r.bytes, 0) / seconds,
        replies: frames.filter(f => f.kind === "reply").length, requestRttP99Ms: percentile(requests.filter(r => r.done !== undefined).map(r => r.done! - r.at), 0.99), byKind, requestsByMethod };
      phase.clients[client.name] = stat;
      if (client.target) {
        const limit = client === own ? params.ownNoticesPerSecond : params.sidebarNoticesPerSecond;
        check(`${name}: ${client.name} notices bounded at ${limit}/s`, notices.length <= Math.ceil(seconds * limit) && peakNotices(notices) <= limit, stat);
        if (name === "mixed") check(`${name}: ${client.name} notices AND successful view refreshes are live`,
          streamHealthy(notices, start, end, client === own ? 1 : 0.5) && stat.replies >= seconds * 0.5 && stat.requests >= seconds * 0.5, stat);
      }
    }
    // The active view must SHOW advancing, freshly emitted text; replies and
    // heartbeats alone cannot stand in for presentation liveness.
    let lastSeq = 0;
    const advancing = own.shownMarks.filter(m => m.thread === ids[0] && inWindow(m.at) && m.emitted >= start && m.phase === mode)
      .filter(m => { if (m.seq <= lastSeq) return false; lastSeq = m.seq; return true; });
    phase.visible = { advancing: advancing.length, maxGapMs: maxGap(advancing.map(m => m.at), start, end), lagP99Ms: percentile(advancing.map(m => m.at - m.emitted), 0.99) };
    check(`${name}: visible view shows fresh advancing own-thread text`, streamHealthy(advancing.map(m => m.at), start, end, 1) &&
      phase.visible.lagP99Ms !== null && phase.visible.lagP99Ms <= params.maxGapMs, phase.visible);
    for (const client of [own, quiet, sidebar]) {
      const foreign = client.shownMarks.filter(m => inWindow(m.at) && m.thread !== client.target!.id);
      check(`${name}: ${client.name} has no foreign live text`, foreign.length === 0, foreign.length);
    }
    if (name === "live-only") {
      phase.committedEvents = events.filter(e => inWindow(e.at));
      const unexpected = phase.committedEvents.filter((e: Commit) => !watchdogCommit(e));
      check("live-only: commits are only recognized watchdog liveness metadata", unexpected.length === 0, unexpected);
      const isolation = quietIsolation(start, end, events);
      phase.isolation = { cleanMs: isolation.cleanMs, excludedWindows: isolation.windows, watchdogCommits: phase.committedEvents.length - unexpected.length };
      check("live-only: sufficient uncontaminated streaming time", isolation.cleanMs >= duration * 1000 * metadataWindow.minimumCleanFraction, phase.isolation);
      for (const id of ids) check(`live-only: ${id} streams during uncontaminated time`,
        marks.filter(m => m.thread === id && isolation.clean(m.at)).length >= isolation.cleanMs / 1000 * params.hz * params.minLoadFraction,
        { delivered: marks.filter(m => m.thread === id && isolation.clean(m.at)).length, cleanMs: isolation.cleanMs });
      for (const client of [quiet, sidebar]) {
        const unexplained = client.notices.filter(isolation.clean);
        phase.isolation[client.name] = { unexplainedNotices: unexplained, cleanRequests: client.requests.filter(r => isolation.clean(r.at)).length,
          totalNotices: phase.clients[client.name].notices, totalRequests: phase.clients[client.name].requests };
        check(`live-only: ${client.name} receives no live-only invalidations outside watchdog metadata windows`, unexplained.length === 0, phase.isolation[client.name]);
      }
    }
  }
  report.totalCpuPercent = phases.reduce((n, p) => n + p.cpuMs, 0) / phases.reduce((n, p) => n + p.wallMs, 0) * 100;
  check(`total actual hub process CPU <= ${params.maxCpuPercent}%`, report.totalCpuPercent <= params.maxCpuPercent, report.totalCpuPercent);
  guard();
} catch (error) {
  report.error = String(error instanceof Error ? error.stack : error);
  check("fixture completed without transport/process/workload error", false, report.error);
} finally {
  finishing = true; control("stop");
  for (const c of clients) {
    for (const p of c.pending.values()) { clearTimeout(p.timer); p.reject(Error("fixture stopped")); }
    c.ws.close();
  }
  if (hub && hub.exitCode === null) {
    hub.kill("SIGTERM");
    const timer = setTimeout(() => { if (hub!.exitCode === null) hub!.kill("SIGKILL"); }, 3000);
    try { report.hubExitCode = await hub.exited; } finally { clearTimeout(timer); }
  }
  report.stderr = existsSync(join(root, "hub.stderr")) ? readFileSync(join(root, "hub.stderr"), "utf8").slice(-16000) : "";
  // Keep startup/dead-stream/watchdog evidence even if a baseline times out
  // before completing a measurement phase and its temporary home is removed.
  report.recentCommittedEvents = events.slice(-200);
  report.streamDiagnostics = Object.fromEntries(ids.map(id => {
    const produced = receipts(id), delivered = marks.filter(m => m.thread === id);
    return [id, { produced: produced.length, delivered: delivered.length,
      firstProduced: produced[0]?.at, lastProduced: produced.at(-1)?.at,
      firstDelivered: delivered[0]?.at, lastDelivered: delivered.at(-1)?.at }];
  }));
  report.transportError = fault?.message;
  report.ok = !report.error && !fault && checks.length > 0 && checks.every(c => c.ok);
  report.finishedAt = new Date().toISOString();
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, JSON.stringify(report, null, 2) + "\n");
  if (!options.has("--keep")) rmSync(root, { recursive: true, force: true });
  console.log(`${report.ok ? "PASS" : "FAIL"} report: ${out}${options.has("--keep") ? `; kept ${root}` : ""}`);
}
process.exitCode = report.ok ? 0 : 1;
