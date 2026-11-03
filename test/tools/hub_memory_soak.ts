// Source-only qualification driver. Never builds, attaches to a live hub,
// imports a private event log, runs a real agent, or forces garbage collection.
// bun test/tools/hub_memory_soak.ts --bin /absolute/hub --out /tmp/soak.json
// Default: 300s bounded warmup + 7200s measured, memory samples every 600s.
// Short CI: --duration-secs 60 (recorded as short-ci, not a two-hour pass).
// Sample/checkpoint history goes to OUT.samples.jsonl; controller RAM is O(20).
import { chmodSync, createReadStream, existsSync, mkdirSync, mkdtempSync, openSync, closeSync, readFileSync, readlinkSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createHash, randomBytes } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { cpus, tmpdir } from "node:os";
import assert from "node:assert/strict";
import { utf8 } from "../../src/web/wire.js";
import { ThinViews } from "../../src/web/thin.js";

type Row = Record<string, any>;
const HELP = `Usage: bun test/tools/hub_memory_soak.ts --bin BINARY --out REPORT.json
  --duration-secs 7200     measured time, excluding warmup (30..86400)
  --warmup-secs 300        bounded warmup (10..600; short CI auto-scales)
  --sample-secs 600        RSS+heap sample interval (5..600; short CI auto-scales)
  --max-rss-mib 1024       absolute owned hub RSS/high-water ceiling (64..65536)
  --owned-inspector       optional read-only owned inspector heap diagnostics
  --keep                  retain the generated isolated home
  --self-test             pure negative controls and codec checks, no hub/agent
No URL, home, token, port, auth or arbitrary inspector target is accepted.
Fixed load: 20 fake agents, 4 deltas/s each, message+tool commit every 2s;
1000 settled/completed historical metadata threads across 40 projects.
Limits: RSS regression slope AND endpoint growth <=10% of warm RSS/hour,
no swap, continuous delivery >=80%, lag p99<=2s, no stream gap>5s.
Heap comes from proc.heap_used_bytes/proc.heap_total_bytes perf gauges;
Missing heap gauges invalidate sampling unless the owned inspector supplies them.
Inspector diagnostics are never used to collect garbage or qualify RSS slope.`;
const options = new Map<string, string>(), args = process.argv.slice(2);
const flags = new Set(["--help", "--self-test", "--keep", "--owned-inspector"]);
const values = new Set(["--bin", "--out", "--duration-secs", "--warmup-secs", "--sample-secs", "--max-rss-mib"]);
for (let i = 0; i < args.length; i++) {
  const key = args[i];
  if ((!flags.has(key) && !values.has(key)) || options.has(key)) throw Error(`Unknown/duplicate option ${key}\n${HELP}`);
  if (flags.has(key)) options.set(key, "1");
  else { const value = args[++i]; if (!value || value.startsWith("--")) throw Error(`Missing value for ${key}`); options.set(key, value); }
}
if (options.has("--help")) { console.log(HELP); process.exit(0); }
function number(name: string, fallback: number, min: number, max: number): number {
  const value = Number(options.get(name) ?? fallback);
  if (!Number.isFinite(value) || value < min || value > max) throw Error(`${name} must be ${min}..${max}`);
  return value;
}
const duration = number("--duration-secs", 7200, 30, 86400);
const params = { durationSecs: duration, warmupSecs: number("--warmup-secs", Math.min(300, Math.max(10, duration / 10)), 10, 600),
  sampleSecs: number("--sample-secs", Math.min(600, duration / 4), 5, 600), maxRssMiB: number("--max-rss-mib", 1024, 64, 65536),
  maxGrowthFractionPerHour: 0.10, threads: 20, metadata: 1000, hz: 4, cycleMs: 2000,
  healthSecs: 10, changeViewSecs: 15, reconnectSecs: 60, requestTimeoutMs: 5000, startupTimeoutMs: 60000, maxClockSkewMs: 500,
  minLoadFraction: 0.8, maxLagMs: 2000, maxGapMs: 5000, ownedInspector: options.has("--owned-inspector") };
if (params.durationSecs / params.sampleSecs < 3) throw Error("Need >=3 sample intervals after warmup; decrease --sample-secs");
const CAPS = Object.freeze({ clients: 4, pendingPerClient: 2, viewLinks: 1, viewCalls: 2, deltaBases: 8,
  threadStates: 20, receiptFiles: 20, histogramBins: 34, sampleHistory: 0, errorHistory: 0,
  frameBytes: 8 * 1024 * 1024, traversalNodes: 200000, nestingDepth: 96, snapshotBytes: 4096 });
function bound(name: keyof typeof CAPS, value: number) {
  if (!Number.isSafeInteger(value) || value < 0 || value > CAPS[name]) throw Error(`Controller bound ${name}: ${value} > ${CAPS[name]}`);
}
const mono = () => Number(process.hrtime.bigint()) / 1e6;
const sleep = (ms: number) => new Promise<void>(r => setTimeout(r, ms));
const ids = Array.from({ length: params.threads }, (_, i) => `active-${String(i).padStart(2, "0")}`);
const idIndex = new Map(ids.map((id, i) => [id, i]));
const dictionarySource = readFileSync(resolve(import.meta.dir, "../../src/core/cbor.bend"), "utf8");
function dictionary(name: string): string[] {
  const at = dictionarySource.indexOf(`def Cbor.${name}()`);
  if (at < 0) throw Error(`Missing CBOR dictionary ${name}`);
  return JSON.parse(dictionarySource.slice(dictionarySource.indexOf("[", at), dictionarySource.indexOf("]", at) + 1));
}
const KEYS = dictionary("keys"), WORDS = dictionary("words");

// Same CBOR subset/dictionaries as active_threads_load.ts and wire.js, with
// explicit traversal stacks: no recursion/spread of growing event arrays.
function encode(value: any): Uint8Array {
  const out: number[] = [], todo: any[] = [value]; let nodes = 0;
  function head(major: number, n: number) {
    if (n < 24) out.push(major << 5 | n);
    else if (n < 256) out.push(major << 5 | 24, n);
    else if (n < 65536) out.push(major << 5 | 25, n >>> 8, n & 255);
    else out.push(major << 5 | 26, n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255);
  }
  while (todo.length) {
    bound("traversalNodes", ++nodes); const v = todo.pop();
    if (v === null) out.push(246);
    else if (typeof v === "boolean") out.push(v ? 245 : 244);
    else if (typeof v === "number") { if (!Number.isFinite(v)) throw Error("Nonfinite request number"); out.push(199); todo.push(String(v)); }
    else if (typeof v === "string") { const b = new TextEncoder().encode(v); head(3, b.length); for (const byte of b) out.push(byte); }
    else if (Array.isArray(v)) { head(4, v.length); for (let i = v.length - 1; i >= 0; i--) todo.push(v[i]); }
    else if (v && typeof v === "object") { const entries = Object.entries(v); head(5, entries.length); for (let i = entries.length - 1; i >= 0; i--) { todo.push(entries[i][1]); todo.push(entries[i][0]); } }
    else throw Error("Unsupported request value");
    bound("frameBytes", out.length);
  }
  return Uint8Array.from(out);
}
function decode(bytes: Uint8Array): Row {
  bound("frameBytes", bytes.length);
  const stack: { value: any; left: number; map: boolean; key?: string }[] = [];
  let at = 0, nodes = 0, root: any, rooted = false;
  function argument(ai: number): number {
    let n = 0, width = ai === 24 ? 1 : ai === 25 ? 2 : ai === 26 ? 4 : 0;
    if (ai < 24) return ai;
    if (!width || at + width > bytes.length) throw Error("Malformed CBOR argument");
    while (width--) n = n * 256 + bytes[at++]; return n;
  }
  while (at < bytes.length) {
    bound("traversalNodes", ++nodes);
    let h = bytes[at++], major = h >>> 5, ai = h & 31, tag = 0, value: any, child: typeof stack[number] | undefined;
    if (major === 6) { if (ai !== 6 && ai !== 7 || at >= bytes.length) throw Error("Malformed CBOR tag"); tag = ai; h = bytes[at++]; major = h >>> 5; ai = h & 31; }
    if (major === 7) { if (tag || ![20, 21, 22].includes(ai)) throw Error("Malformed CBOR scalar"); value = ai === 22 ? null : ai === 21; }
    else {
      const n = argument(ai);
      if (major === 0) { value = tag === 6 ? WORDS[n] : n; if (tag === 7 || value === undefined) throw Error("Malformed CBOR word"); }
      else if (major === 1 && !tag) value = -1 - n;
      else if (major === 3) { if (at + n > bytes.length || tag === 6) throw Error("Malformed CBOR text"); value = utf8(bytes, at, at + n); at += n; if (tag === 7) { value = Number(value); if (!Number.isFinite(value)) throw Error("Malformed CBOR number"); } }
      else if ((major === 4 || major === 5) && !tag) { value = major === 4 ? [] : {}; child = { value, left: n * (major === 5 ? 2 : 1), map: major === 5 }; bound("traversalNodes", child.left); }
      else throw Error("Unsupported CBOR item");
    }
    const parent = stack.at(-1);
    if (!parent) { if (rooted) throw Error("Trailing CBOR"); rooted = true; root = value; }
    else {
      if (!parent.left) throw Error("CBOR container overflow");
      if (!parent.map) parent.value.push(value);
      else if (parent.key === undefined) {
        const key = typeof value === "number" ? KEYS[value] : value;
        if (typeof key !== "string" || Object.hasOwn(parent.value, key)) throw Error("Invalid/duplicate CBOR key"); parent.key = key;
      } else { Object.defineProperty(parent.value, parent.key, { value, enumerable: true, writable: true, configurable: true }); parent.key = undefined; }
      parent.left--;
    }
    if (child?.left) { stack.push(child); bound("nestingDepth", stack.length); }
    while (stack.length && stack.at(-1)!.left === 0) stack.pop();
  }
  if (stack.length || !rooted || !root || typeof root !== "object") throw Error("Truncated/invalid CBOR envelope");
  return root;
}
function walk(root: any, visit: (node: Row) => void) {
  const todo = [root]; let nodes = 0;
  while (todo.length) {
    bound("traversalNodes", ++nodes); const n = todo.pop();
    if (!n || typeof n !== "object") continue;
    if (Array.isArray(n)) { for (let i = n.length - 1; i >= 0; i--) todo.push(n[i]); }
    else { visit(n); if (Array.isArray(n.kids)) todo.push(n.kids); }
    bound("traversalNodes", todo.length);
  }
}
// Split the old (1023, 2047] bucket at the actual acceptance limit. This
// preserves bounded storage and an upper-bound metric while making the
// nearest-rank p99 <= 2000ms decision exact, including fractional latencies.
const LATENCY_UPPER_MS = Object.freeze([...Array.from({ length: 33 }, (_, i) => 2 ** i - 1), params.maxLagMs].sort((a, b) => a - b));
class Histogram {
  bins = new Array<number>(LATENCY_UPPER_MS.length).fill(0); n = 0; max = 0;
  add(value: number) {
    if (!Number.isFinite(value) || value < 0) throw Error(`Invalid monotonic latency ${value}`);
    this.n++; this.max = Math.max(this.max, value);
    let i = 0; while (i < this.bins.length - 1 && value > LATENCY_UPPER_MS[i]) i++;
    this.bins[i]++;
  }
  p99(): number | null {
    if (!this.n) return null;
    // ceil(99*n/100), without floating multiplication at integer boundaries.
    const rank = this.n - Math.floor(this.n / 100); let n = 0;
    for (let i = 0; i < this.bins.length; i++) {
      n += this.bins[i];
      if (n >= rank) return i === this.bins.length - 1 ? Math.max(LATENCY_UPPER_MS[i], this.max) : LATENCY_UPPER_MS[i];
    }
    return null;
  }
}
class Regression {
  n = 0; meanX = 0; meanY = 0; xx = 0; xy = 0; first?: { at: number; rss: number }; last?: { at: number; rss: number };
  add(at: number, rss: number) {
    if (!Number.isFinite(at) || !Number.isFinite(rss) || rss <= 0 || this.last && at <= this.last.at) throw Error("Nonmonotonic/invalid memory sample");
    this.first ??= { at, rss }; this.last = { at, rss }; const x = (at - this.first.at) / 3600000;
    const dx = x - this.meanX; this.n++; this.meanX += dx / this.n; this.meanY += (rss - this.meanY) / this.n;
    this.xx += dx * (x - this.meanX); this.xy += dx * (rss - this.meanY);
  }
  result() {
    const hours = this.first && this.last ? (this.last.at - this.first.at) / 3600000 : 0;
    const slope = this.xx > 0 ? this.xy / this.xx : NaN, base = this.first?.rss ?? NaN;
    const endpoint = hours > 0 ? ((this.last!.rss - base) / base) / hours : NaN;
    return { samples: this.n, hours, baselineKiB: base, slopeKiBPerHour: slope, slopeFractionPerHour: slope / base, endpointFractionPerHour: endpoint,
      ok: this.n >= 4 && hours > 0 && Number.isFinite(slope) && slope / base <= params.maxGrowthFractionPerHour && endpoint <= params.maxGrowthFractionPerHour };
  }
}
function memoryStatus(text: string, enforceLimits = true) {
  const result: Row = {};
  for (const line of text.split("\n")) { const m = /^(VmRSS|VmHWM|VmSwap):\s+(\d+) kB$/.exec(line); if (m) result[m[1]] = Number(m[2]); }
  if (!["VmRSS", "VmHWM", "VmSwap"].every(k => Number.isFinite(result[k])) || result.VmRSS <= 0 || result.VmHWM < result.VmRSS) throw Error("Missing/invalid owned PID memory fields");
  if (enforceLimits && (result.VmRSS > params.maxRssMiB * 1024 || result.VmHWM > params.maxRssMiB * 1024)) throw Error("Absolute RSS/high-water cap exceeded");
  if (enforceLimits && result.VmSwap !== 0) throw Error("Owned hub swapped; RSS cannot qualify retained memory");
  return result;
}
// A failure endpoint is evidence only: do not add it to the acceptance
// regression/sample count, require heap availability, or suppress the failure.
function finalProcDiagnostic(readProc: () => Row, at: number, baseline?: { at: number; rss: number }) {
  try {
    const procKiB = readProc(), hours = baseline ? (at - baseline.at) / 3600000 : 0;
    const endpoint = baseline && hours > 0 ? { hours, baselineMonotonicMs: baseline.at, baselineKiB: baseline.rss,
      growthFraction: (procKiB.VmRSS - baseline.rss) / baseline.rss,
      growthFractionPerHour: ((procKiB.VmRSS - baseline.rss) / baseline.rss) / hours } : null;
    return { available: true, diagnosticOnly: true, sampledAtMonotonicMs: at, procKiB, endpoint };
  } catch (error) {
    return { available: false, diagnosticOnly: true, sampledAtMonotonicMs: at, error: String(error) };
  }
}
function heapValid(heap: Row) {
  return heap.available === true && Number.isFinite(heap.heapUsed) && Number.isFinite(heap.heapTotal) && heap.heapUsed >= 0 && heap.heapTotal >= heap.heapUsed;
}
function completedViewHealthy(lastShownAt: number, newestEmittedEpoch: number, now: number, epoch: number) {
  return lastShownAt > 0 && now - lastShownAt <= params.maxGapMs && epoch - newestEmittedEpoch <= params.maxGapMs && epoch >= newestEmittedEpoch;
}
class Stream {
  delivered = 0; messages = 0; tools = 0; lastSeq = 0; lastAt = 0; gap = 0; lag = new Histogram();
  mark(seq: number, emitted: number, now: number, epochMs = Date.now()) {
    if (!Number.isSafeInteger(seq) || seq <= this.lastSeq) throw Error("Duplicate/regressed delivered sequence");
    this.lastSeq = seq; this.delivered++; if (this.lastAt) this.gap = Math.max(this.gap, now - this.lastAt); this.lastAt = now; this.lag.add(epochMs - emitted);
  }
}
function workloadVerdict(s: Stream, prior: Row, produced: number, seconds: number, now: number) {
  const delivered = s.delivered - prior.delivered, messages = s.messages - prior.messages, tools = s.tools - prior.tools;
  const p99 = s.lag.p99();
  return { delivered, produced, messages, tools, seconds, gapMs: Math.max(s.gap, now - s.lastAt), lagP99UpperMs: p99,
    ok: produced >= Math.max(1, Math.floor(params.hz * seconds * params.minLoadFraction) - 1) &&
      delivered >= Math.max(1, Math.floor(params.hz * seconds * params.minLoadFraction) - 1) &&
      messages >= Math.max(1, Math.floor(seconds * 1000 / params.cycleMs * params.minLoadFraction)) &&
      tools >= Math.max(1, Math.floor(seconds * 1000 / params.cycleMs * params.minLoadFraction)) &&
      Math.max(s.gap, now - s.lastAt) <= params.maxGapMs && p99 !== null && p99 <= params.maxLagMs };
}

// Reuse the active-load fixture's metadata shape; write lines directly so no
// growing event history is held by the controller. Only these 20 agents run.
function seed(path: string, project: string) {
  const fd = openSync(path, "wx", 0o600);
  const put = ($: string, fields: Row, at = 1) => writeFileSync(fd, JSON.stringify({ $, ...fields, at }) + "\n");
  try {
    put("SettingSet", { key: "telemetry.share", value: "off" });
    put("SettingSet", { key: "telemetry.usage.asked", value: "off" });
    for (const key of ["restart.continue", "fallback.on"]) put("SettingSet", { key, value: "off" });
    for (const key of ["onboard.done", "onboard.uber"]) put("SettingSet", { key, value: "1" });
    put("SettingSet", { key: "sidebar.view", value: "active" });
    for (let i = 0; i < params.metadata; i++) {
      const group = `catalog-${Math.floor(i / 25)}`, id = `settled-${String(i).padStart(5, "0")}`;
      if (i % 25 === 0) { mkdirSync(join(project, group)); put("ProjectCreated", { id: group, title: `Hardware project ${group}`, root: join(project, group) }); }
      put("ThreadCreated", { id, project: group, title: `Review ${i}: connector clearance and enclosure fit`, env: "local", worktree: "", branch: "", provider: "claude", model: "fixture" }, 2 + i);
      put("TurnChanged", { thread: id, state: "completed" }, 10002 + i); put("ThreadSettled", { id, manual: true }, 20002 + i);
    }
    put("ProjectCreated", { id: "load", title: "Isolated RSS soak", root: project });
    for (const id of ids) put("ThreadCreated", { id, project: "load", title: id, env: "local", worktree: project, branch: "", provider: "claude", model: "fixture" });
  } finally { closeSync(fd); }
}
const PRODUCER = String.raw`import {writeSync,writeFileSync,renameSync,existsSync} from 'node:fs';
import {createInterface} from 'node:readline';
if(process.argv.includes('--version')){console.log('2.1.286 (isolated RSS fixture)');process.exit(0)}
const root=process.env.SOAK_ROOT, sid='soak-'+process.pid, wall=()=>Date.now();
const emit=o=>writeSync(1,JSON.stringify({...o,session_id:sid})+'\n'); let running=false;
async function run(o){
 const id=/rss-soak-thread=(active-\d+)/.exec(JSON.stringify(o))?.[1];if(!id||running)throw Error('invalid fixture turn');running=true;
 let seq=0,cycle=0,text='',next=performance.now();
 const receipt=()=>{const p=root+'/'+id+'.next';writeFileSync(p,JSON.stringify({id,pid:process.pid,seq,cycle,at:wall()}));renameSync(p,root+'/'+id+'.json')};
 emit({type:'system',subtype:'init',cwd:process.cwd(),tools:['Bash'],model:'fixture',permissionMode:'default'});
 const begin=()=>{text='';emit({type:'stream_event',event:{type:'message_start',message:{id:id+'-'+cycle,role:'assistant',content:[],usage:{input_tokens:1}}}});emit({type:'stream_event',event:{type:'content_block_start',index:0,content_block:{type:'text',text:''}}})};begin();receipt();
 while(!existsSync(root+'/stop')){
  const delta=('@rss:'+id+':'+(++seq)+':'+wall()+'@').padEnd(79,'x')+' ';text+=delta;
  emit({type:'stream_event',event:{type:'content_block_delta',index:0,delta:{type:'text_delta',text:delta}}});
  if(seq%8===0){
   emit({type:'stream_event',event:{type:'content_block_stop',index:0}});
   emit({type:'assistant',message:{id:id+'-'+cycle,role:'assistant',content:[{type:'text',text}],usage:{input_tokens:1,output_tokens:8}}});
   const tool=id+'-tool-'+cycle;emit({type:'assistant',message:{id:tool,role:'assistant',content:[{type:'tool_use',id:tool,name:'Bash',input:{command:'echo synthetic-'+cycle,description:'Small fixed soak commit'}}]}});
   emit({type:'user',message:{role:'user',content:[{type:'tool_result',tool_use_id:tool,content:'synthetic result',is_error:false}]}});cycle++;begin();
  }
  if(seq%4===0)receipt();next=Math.max(next+250,performance.now());await new Promise(r=>setTimeout(r,Math.max(1,next-performance.now())));
 }
 receipt();process.exit(0);
}
createInterface({input:process.stdin}).on('line',s=>{const o=JSON.parse(s);if(o.type==='user')run(o).catch(e=>{console.error(e);process.exit(1)})});
`;

if (options.has("--self-test")) {
  for (const [key, max] of Object.entries(CAPS)) {
    bound(key as keyof typeof CAPS, max);
    assert.throws(() => bound(key as keyof typeof CAPS, max + 1), undefined, `${key} overflow must fail`);
  }
  const obj = { id: 2 ** 47 + 9, m: "view.update", p: { yes: true, no: false, nil: null, list: [1, "λ🙂"] } };
  assert.deepEqual(decode(encode(obj)), obj); assert.throws(() => decode(Uint8Array.of(0x81)), /Truncated/);
  let nested: any = "end"; for (let i = 0; i < 200; i++) nested = [nested];
  assert.throws(() => decode(encode({ nested })), /nestingDepth/);
  let deep: Row = { type: "text", text: "leaf" }; for (let i = 0; i < 20000; i++) deep = { kids: [deep] };
  let seen = 0; walk(deep, () => { seen++; }); assert.equal(seen, 20001);
  const status = (rss: number, hwm = rss, swap = 0) => `VmRSS: ${rss} kB\nVmHWM: ${hwm} kB\nVmSwap: ${swap} kB\n`;
  assert.equal(memoryStatus(status(1000)).VmRSS, 1000);
  assert.throws(() => memoryStatus(status(params.maxRssMiB * 1024 + 1)), /cap/);
  assert.throws(() => memoryStatus(status(1000, params.maxRssMiB * 1024 + 1)), /cap/);
  assert.throws(() => memoryStatus(status(1000, 1000, 1)), /swapped/); assert.throws(() => memoryStatus(""), /Missing/);
  const overCap = params.maxRssMiB * 1024 + 1;
  const final = finalProcDiagnostic(() => memoryStatus(status(overCap, overCap + 1, 7), false), 3600000, { at: 0, rss: 1000 });
  assert.equal(final.available, true); assert.equal(final.diagnosticOnly, true);
  assert.equal(final.procKiB!.VmRSS, overCap); assert.equal(final.procKiB!.VmHWM, overCap + 1); assert.equal(final.procKiB!.VmSwap, 7);
  assert.equal(final.endpoint!.growthFractionPerHour, (overCap - 1000) / 1000);
  assert.throws(() => memoryStatus(status(overCap, overCap + 1, 7)), /cap/, "acceptance still enforces limits after diagnostic read");
  const unavailable = finalProcDiagnostic(() => { throw Error("owned child exited"); }, 200);
  assert.equal(unavailable.available, false); assert.match(unavailable.error!, /owned child exited/);
  assert.equal(finalProcDiagnostic(() => memoryStatus(status(1000)), 200).endpoint, null, "no baseline means no diagnostic growth rate");
  assert.equal(finalProcDiagnostic(() => memoryStatus(status(1000)), 200, { at: 200, rss: 1000 }).endpoint, null, "zero elapsed time has no growth rate");
  assert(heapValid({ available: true, heapUsed: 100, heapTotal: 200 }));
  assert(!heapValid({ available: false })); assert(!heapValid({ available: true, heapUsed: 200, heapTotal: 100 }));
  assert(completedViewHealthy(100, 1000, 200, 1100));
  assert(!completedViewHealthy(0, 1000, 200, 1100), "absent visible content fails");
  assert(!completedViewHealthy(100, 1000, 200, 7000), "stale completed view fails");
  const good = new Regression(), slopeBad = new Regression(), endpointBad = new Regression();
  for (let i = 0; i <= 12; i++) { const t = i * 600000; good.add(t, 1000 + i); slopeBad.add(t, 1000 + i * 30); endpointBad.add(t, i === 12 ? 1250 : 1000); }
  assert(good.result().ok); assert(!slopeBad.result().ok); assert(!endpointBad.result().ok);
  assert(endpointBad.result().slopeFractionPerHour <= 0.10 && endpointBad.result().endpointFractionPerHour > 0.10, "endpoint-only negative control");
  const olsOnlyBad = new Regression(); for (let i = 0; i <= 12; i++) olsOnlyBad.add(i * 600000, i === 12 ? 1000 : 1000 + i * 50);
  assert(olsOnlyBad.result().slopeFractionPerHour > 0.10 && olsOnlyBad.result().endpointFractionPerHour <= 0.10 && !olsOnlyBad.result().ok, "OLS-only negative control");
  const insufficient = new Regression(); insufficient.add(0, 1000); assert(!insufficient.result().ok); assert.throws(() => insufficient.add(0, 1000), /Nonmonotonic/);
  assert.equal(new Histogram().p99(), null, "empty latency stream cannot qualify");
  for (const latency of [1999, 2000, 2000.001, 2001]) {
    const h = new Histogram(); h.add(latency);
    assert.equal(h.p99()! <= params.maxLagMs, latency <= 2000, `one-sample inclusive limit at ${latency}ms`);
    assert.equal(h.p99(), latency <= 2000 ? 2000 : 2047);
  }
  for (const n of [1, 2, 3, 40, 41, 98, 99, 100, 101, 199, 200, 201]) {
    const rank = Math.ceil(99 * n / 100), allowed = n - rank;
    for (const late of new Set([0, allowed, allowed + 1, n])) {
      const h = new Histogram();
      for (let i = 0; i < n; i++) h.add(i < late ? 2001 : i % 2 ? 1999 : 2000);
      assert.equal(h.p99()! <= params.maxLagMs, n - late >= rank, `nearest-rank p99 n=${n}, over-limit=${late}`);
      assert.equal(h.n, n); bound("histogramBins", h.bins.length);
    }
  }
  const overflow = new Histogram(); overflow.add(2 ** 40); assert(overflow.p99()! >= 2 ** 40, "overflow bucket remains an upper bound");
  const stream = new Stream(), prior = { delivered: 0, messages: 0, tools: 0 };
  assert(!workloadVerdict(stream, prior, 40, 10, 10000).ok, "dead delivery fails");
  for (let i = 1; i <= 40; i++) stream.mark(i, i * 250 - 20, i * 250, i * 250);
  stream.messages = stream.tools = 5; assert(workloadVerdict(stream, prior, 40, 10, 10000).ok);
  assert(!workloadVerdict(stream, prior, 0, 10, 10000).ok, "dead producer fails");
  assert(!workloadVerdict(stream, prior, 40, 10, 16000).ok, "burst then silence fails");
  for (const latency of [1999, 2000, 2001]) {
    stream.lag = new Histogram(); for (let i = 0; i < 40; i++) stream.lag.add(latency);
    assert.equal(workloadVerdict(stream, prior, 40, 10, 10000).ok, latency <= 2000, `actual workload verdict at ${latency}ms`);
  }
  stream.lag = new Histogram(); for (let i = 0; i < 40; i++) stream.lag.add(20);
  assert(workloadVerdict(stream, prior, 40, 10, 10000).ok, "restore healthy latency before independent commit guard");
  stream.tools = 0; assert(!workloadVerdict(stream, prior, 40, 10, 10000).ok, "missing commits fail");
  stream.tools = 5; stream.lag = new Histogram(); for (let i = 0; i < 40; i++) stream.lag.add(3000);
  assert(!workloadVerdict(stream, prior, 40, 10, 10000).ok, "queued stale delivery fails");
  assert.throws(() => stream.mark(40, 0, 10000), /sequence/); assert.throws(() => new Histogram().add(-1), /latency/);
  new Bun.Transpiler({ loader: "js" }).transformSync(PRODUCER);
  console.log("PASS pure self-test: every controller cache bound, stack-safe codec/traversal, exact inclusive 2000ms nearest-rank p99 (small n and 1% tails), failure proc endpoint, RSS/HWM/swap, slope/endpoint/missing samples, dead/underloaded/stale/burst/commit guards; no hub/agent started");
  process.exit(0);
}

if (process.platform !== "linux") throw Error("Linux /proc required");
if (!options.has("--bin") || !options.has("--out")) throw Error(HELP);
const bin = realpathSync(resolve(options.get("--bin")!)), out = resolve(options.get("--out")!);
if (bin === out || existsSync(out) || existsSync(out + ".samples.jsonl")) throw Error("Refusing overwrite of binary/report/samples");
const root = mkdtempSync(join(tmpdir(), "bp-rss-soak-"));
const home = join(root, "home"), user = join(root, "user"), agents = join(root, "agents"), project = join(root, "project");
for (const dir of [home, user, agents, project, join(root, "locks")]) mkdirSync(dir);
mkdirSync(dirname(out), { recursive: true });
const sampleFd = openSync(out + ".samples.jsonl", "wx", 0o600), token = randomBytes(24).toString("hex");
writeFileSync(join(home, "token"), token, { mode: 0o600 }); seed(join(home, "events.jsonl"), project);
writeFileSync(join(agents, "producer.js"), PRODUCER);
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
writeFileSync(join(agents, "claude"), `#!/bin/sh\nunset BUN_OPTIONS\nexec ${quote(process.execPath)} ${quote(join(agents, "producer.js"))} "$@"\n`); chmodSync(join(agents, "claude"), 0o755);
for (const name of ["codex", "grok", "muse", "opencode", "antigravity", "agy", "pi", "omp", "gh", "tailscale", "notify-send"])
  { writeFileSync(join(agents, name), "#!/bin/sh\nexit 127\n"); chmodSync(join(agents, name), 0o755); }
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true); if (port === 3787) throw Error("Refusing live port");
const url = `http://127.0.0.1:${port}`, streams = ids.map(() => new Stream()), clients = new Set<Client>();
let hub: ReturnType<typeof Bun.spawn> | undefined, failure: Error | undefined, stopping = false, birth = "", measuredAt = 0;
let deliveredFrames = 0, receivedBytes = 0, commitEvents = 0, switches = 0, reconnects = 0, healthChecks = 0;
const regression = new Regression(), requestLag = new Histogram();
const clockOrigin = { mono: mono(), epoch: Date.now() };
const report: Row = { schema: 1, fixture: "hub-memory-soak-v1", mode: duration >= 7200 ? "two-hour-or-longer" : "short-ci", params, controllerCaps: CAPS, root, bin,
  sampleFile: out + ".samples.jsonl", startedAt: new Date().toISOString(), controllerPid: process.pid, pidScope: "Owned hub PID only; excludes fake agents and measuring controller; no forced GC",
  clockPolicy: "Local monotonic clocks for durations/gaps; emission epoch milliseconds for cross-process latency. Bun hrtime origins differ per process. Wall/monotonic skew >500ms invalidates the run.",
  latencyPolicy: { percentile: "nearest rank ceil(99*n/100); no samples fails", upperBoundsMs: LATENCY_UPPER_MS, inclusiveLimitMs: params.maxLagMs },
  host: { platform: process.platform, arch: process.arch, bun: Bun.version, cpus: cpus().length, cpuModel: cpus()[0]?.model } };
function fail(error: any) { if (!stopping) failure ??= error instanceof Error ? error : Error(String(error)); }
function guard() {
  if (failure) throw failure; if (hub && hub.exitCode !== null) throw Error(`Owned hub exited ${hub.exitCode}`);
  if (Math.abs((Date.now() - clockOrigin.epoch) - (mono() - clockOrigin.mono)) > params.maxClockSkewMs) throw Error("Clock adjustment invalidated cross-process latency samples");
}
async function until(label: string, predicate: () => boolean, ms = params.requestTimeoutMs) {
  const end = mono() + ms; while (!predicate()) { guard(); if (mono() >= end) throw Error(`Timeout: ${label}`); await sleep(20); }
}
function emit(kind: string, data: Row) { writeFileSync(sampleFd, JSON.stringify({ kind, monotonicMs: mono(), elapsedMeasuredMs: measuredAt ? mono() - measuredAt : null, ...data }) + "\n"); }
function receipt(id: string): Row {
  const path = join(root, `${id}.json`); if (!existsSync(path)) return { seq: 0, cycle: 0, at: 0 };
  bound("snapshotBytes", statSync(path).size);
  const text = readFileSync(path, "utf8"); bound("snapshotBytes", Buffer.byteLength(text));
  const value = JSON.parse(text);
  if (value.id !== id || !Number.isSafeInteger(value.seq) || !Number.isSafeInteger(value.cycle) || !Number.isFinite(value.at)) throw Error(`Invalid producer receipt ${id}`);
  return value;
}
function processMemory(enforceLimits = true) {
  const stat = readFileSync(`/proc/${hub!.pid}/stat`, "utf8");
  const observed = stat.slice(stat.lastIndexOf(") ") + 2).split(" ")[19];
  if (!birth) birth = observed; if (!observed || birth !== observed) throw Error("Owned hub PID identity changed");
  return memoryStatus(readFileSync(`/proc/${hub!.pid}/status`, "utf8"), enforceLimits);
}

class Client {
  ws: WebSocket; views?: ThinViews; greeting?: Row; joined = false; closed = false; selecting = false; pumping = false; refresh = false;
  pending = new Map<number, { resolve: (v: Row) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout>; at: number }>();
  target?: Row; session: Row; n = 0; replyCount = 0; newestShown = 0; newestShownAt = 0;
  constructor(readonly name: string, target?: Row) {
    this.target = target; this.session = { sel: target?.id ?? "", rid: 1, drafts: {}, open: {}, info: {}, adding: false, panel: false };
    this.ws = new WebSocket(`${url.replace("http:", "ws:")}/ws?token=${token}${target ? "&view=1&frontend=1" : ""}`); this.ws.binaryType = "arraybuffer";
    clients.add(this); bound("clients", clients.size);
    if (target) this.views = new ThinViews({ now: mono, sidebarRegions: true, viewDelta: true, send: (_hub: string, r: Row) => r, command: (c: Row) => c });
    this.ws.onclose = () => { clients.delete(this); if (!this.closed) fail(Error(`${name}: unexpected socket close`)); };
    this.ws.onerror = () => { if (!this.closed) fail(Error(`${name}: socket error`)); };
    this.ws.onmessage = e => { try { this.receive(e); } catch (error) { fail(error); } };
  }
  receive(event: MessageEvent) {
    if (this.closed) return;
    const raw = typeof event.data === "string" ? new TextEncoder().encode(event.data) : new Uint8Array(event.data as ArrayBuffer);
    bound("frameBytes", raw.length); receivedBytes += raw.length; deliveredFrames++;
    const f = typeof event.data === "string" ? JSON.parse(event.data) : decode(raw), now = mono();
    if (!f || typeof f.t !== "string") throw Error("Undecodable envelope");
    if (f.t === "view" && this.views) { if (this.greeting) throw Error("Repeated thin greeting"); this.greeting = f; }
    if (f.t === "log" && !this.views) this.joined = true; // discard immediately
    if (this.views && ["log", "changes", "delta", "live"].includes(f.t)) throw Error("Thin view received legacy broadcast");
    if (!this.views && f.t === "delta") {
      const index = idIndex.get(f.thread); if (index === undefined) throw Error("Unexpected live thread");
      const re = /@rss:(active-\d+):(\d+):(\d+)@/g; let match: RegExpExecArray | null, found = false;
      while ((match = re.exec(f.text ?? ""))) { if (match[1] !== f.thread) throw Error("Cross-thread delta"); streams[index].mark(Number(match[2]), Number(match[3]), now); found = true; }
      if (!found) throw Error("Unrecognized synthetic delta");
    }
    if (!this.views && f.t === "changes") for (const e of f.items ?? []) {
      commitEvents++; const index = idIndex.get(e.thread); if (index === undefined) continue;
      if (e.$ === "MessagePosted" && e.role === "assistant") streams[index].messages++;
      if (e.$ === "ActivityLogged") streams[index].tools++;
    }
    if (f.t === "view.changed" && this.views && this.greeting) {
      const result = this.views.changed("soak", f, this.session, { id: "", offset: 0 }, true);
      if (result.failed) throw Error(`Invalid notice ${result.failed}`);
      if (!result.ignored) { this.refresh = true; void this.pump(); }
    }
    if (f.t === "reply") {
      const p = this.pending.get(Number(f.id)); if (!p) throw Error("Unexpected reply");
      clearTimeout(p.timer); this.pending.delete(Number(f.id)); requestLag.add(now - p.at);
      if (f.ok !== true) p.reject(Error(`Rejected ${this.name} request: ${JSON.stringify(f).slice(0, 400)}`)); else p.resolve(f);
    }
  }
  send(r: Row): Promise<Row> {
    if (this.closed) return Promise.reject(Error("Closed client")); bound("pendingPerClient", this.pending.size + 1);
    const raw = encode(r);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(r.id); reject(Error(`${this.name}: request deadline`)); }, params.requestTimeoutMs);
      this.pending.set(r.id, { resolve, reject, timer, at: mono() }); this.ws.send(raw);
    });
  }
  async picture(call: Row) {
    if (call.failed || !call.id || call.cmds.length !== 1) throw Error(`Invalid thin call ${call.failed}`);
    const raw = await this.send(call.cmds[0]); const result = this.views!.reply("soak", raw);
    if (!result?.current || !result.remoteReady || !result.shown) throw Error(`${this.name}: invalid view response`);
    this.replyCount++; this.session = result.session;
    if (this.target?.kind === "thread") {
      let newest = 0;
      walk(result.shown, n => {
        if (n.type !== "text" || typeof n.text !== "string") return;
        const re = /@rss:(active-\d+):(\d+):(\d+)@/g; let match: RegExpExecArray | null;
        while ((match = re.exec(n.text))) { if (match[1] !== this.target!.id) throw Error("Foreign synthetic text in visible view"); newest = Math.max(newest, Number(match[3])); }
      });
      if (newest > this.newestShown) { this.newestShown = newest; this.newestShownAt = mono(); }
    }
    this.refresh ||= result.refreshDue === true; this.cacheBounds();
  }
  // One pump, one in-flight update and one owed-refresh bit. No recursive
  // promise chain, accumulated notices, frame history or unbounded callback queue.
  async pump() {
    if (this.pumping || this.selecting || !this.greeting || !this.views?.links.get("soak")?.viewId) return;
    this.pumping = true;
    try { while (this.refresh && !this.closed && !this.selecting) { this.refresh = false; await this.picture(this.views.request("soak", "view.update", { session: this.session, anchor: { id: "", offset: 0 } })); } }
    catch (error) { if (!this.closed) fail(error); } finally { this.pumping = false; }
  }
  async select(target: Row) {
    this.selecting = true; await until("view pump idle", () => !this.pumping);
    try {
      this.refresh = false; this.target = target; this.session.sel = target.kind === "thread" ? target.id : ""; this.newestShown = this.newestShownAt = 0;
      await this.picture(this.views!.select("soak", target, this.session, this.name));
    } finally { this.selecting = false; }
    void this.pump();
  }
  async open() {
    await until("socket open", () => this.ws.readyState === WebSocket.OPEN);
    if (!this.views) { await until("initial observer log", () => this.joined); return; }
    await until("thin greeting", () => !!this.greeting);
    if (this.greeting!.renderer !== "web" || !this.views.connect("soak", this.greeting).connected) throw Error("Expected view-web greeting");
    await this.select(this.target!);
  }
  cacheBounds() {
    bound("clients", clients.size); bound("pendingPerClient", this.pending.size);
    if (!this.views) return;
    bound("viewLinks", this.views.links.size); bound("viewCalls", this.views.calls.size);
    for (const link of this.views.links.values()) bound("deltaBases", link.bases.map.size);
  }
  close() {
    this.closed = true; this.refresh = false; this.ws.close();
    for (const p of this.pending.values()) { clearTimeout(p.timer); p.reject(Error("Disposable client closed")); }
    this.pending.clear(); this.views?.disconnect("soak");
  }
}

// Heap diagnostics never call GC. A fixed, owned inspector endpoint can be
// enabled only on the child this driver launches; no target option exists.
let inspector: WebSocket | undefined, inspectorReply: ((v: Row) => void) | undefined, inspectorError: ((e: Error) => void) | undefined, inspectId = 0, inspectPort = 0;
async function inspectorSample(): Promise<Row> {
  if (!params.ownedInspector) return { available: false, reason: "Owned inspector diagnostics not requested" };
  if (!inspector) {
    // Read just the bounded tail; never accumulate the process log in RAM.
    const text = await tailStderr(); const match = text.match(new RegExp(`ws://127\\.0\\.0\\.1:${inspectPort}/[A-Za-z0-9_-]+`));
    if (!match) throw Error("Owned inspector URL absent from child stderr");
    inspector = new WebSocket(match[0]);
    inspector.onmessage = e => { try {
      const text = String(e.data); bound("snapshotBytes", Buffer.byteLength(text)); const reply = JSON.parse(text);
      if (reply.id === inspectId) { if (reply.error) inspectorError?.(Error(JSON.stringify(reply.error))); else inspectorReply?.(reply.result); }
    } catch (error) { inspectorError?.(error as Error); } };
    inspector.onerror = () => inspectorError?.(Error("Owned inspector error"));
    await until("owned inspector open", () => inspector!.readyState === WebSocket.OPEN);
  }
  const result = await new Promise<Row>((resolve, reject) => {
    const timer = setTimeout(() => reject(Error("Owned inspector stats timeout")), params.requestTimeoutMs);
    inspectorReply = v => { clearTimeout(timer); resolve(v); }; inspectorError = e => { clearTimeout(timer); reject(e); };
    inspector!.send(JSON.stringify({ id: ++inspectId, method: "Runtime.evaluate", params: { expression: "JSON.stringify({pid:process.pid,memory:process.memoryUsage()})", returnByValue: true, objectGroup: "soak-read-only" } }));
  });
  inspectorReply = inspectorError = undefined;
  const v = JSON.parse(result.result?.value ?? "null");
  if (v?.pid !== hub!.pid) throw Error("Inspector PID mismatch");
  return { available: true, source: "owned-inspector-read-only", ...v.memory };
}
async function tailStderr(): Promise<string> {
  if (!existsSync(join(root, "hub.stderr"))) return "";
  const f = Bun.file(join(root, "hub.stderr")); return f.slice(Math.max(0, f.size - 16384)).text();
}
async function limitedText(response: Response): Promise<string> {
  if (!response.body) throw Error("Empty snapshot body");
  const reader = response.body.getReader(), bytes = new Uint8Array(CAPS.frameBytes); let n = 0;
  try {
    for (;;) {
      const chunk = await reader.read(); if (chunk.done) break;
      bound("frameBytes", n + chunk.value.length); bytes.set(chunk.value, n); n += chunk.value.length;
    }
  } catch (error) { await reader.cancel().catch(() => {}); throw error; }
  finally { reader.releaseLock(); }
  return new TextDecoder().decode(bytes.subarray(0, n));
}
async function takeMemory() {
  guard(); const at = mono(), proc = processMemory();
  const response = await fetch(url + "/debug/perf.json?token=" + token, { signal: AbortSignal.timeout(params.requestTimeoutMs) });
  if (!response.ok) throw Error(`Performance snapshot HTTP ${response.status}`);
  const text = await limitedText(response);
  const perf = JSON.parse(text), gauges: Row = {};
  if (!Array.isArray(perf.rows)) throw Error("Invalid performance snapshot");
  for (const row of perf.rows) if (row.kind === "gauge" && typeof row.name === "string") gauges[row.name] = row.last;
  const heap: Row = Number.isFinite(gauges["proc.heap_used_bytes"]) && Number.isFinite(gauges["proc.heap_total_bytes"])
    ? { available: true, source: "owned-hub-perf", heapUsed: gauges["proc.heap_used_bytes"], heapTotal: gauges["proc.heap_total_bytes"] }
    : await inspectorSample();
  if (!heapValid(heap)) { emit("missing-heap", { heap }); throw Error("Missing/invalid owned heap gauges; expose proc.heap_used_bytes + proc.heap_total_bytes or use optional --owned-inspector diagnostics"); }
  regression.add(at, proc.VmRSS);
  const sample = { hubPid: hub!.pid, sampledAtMonotonicMs: at, epochMs: Date.now(), procKiB: proc, heap,
    controller: { memory: process.memoryUsage(), liveClients: clients.size }, regression: regression.result() };
  report.lastMemory = sample; report.heapSamples = (report.heapSamples ?? 0) + Number(heap.available);
  emit("memory", sample);
}
async function hashFile(path: string) { const h = createHash("sha256"); for await (const bytes of createReadStream(path)) h.update(bytes); return h.digest("hex"); }
const sources = ["test/tools/hub_memory_soak.ts", "test/tools/active_threads_load.ts", "src/core/cbor.bend", "src/web/wire.js", "src/web/thin.js",
  "src/web/view-delta.js", "src/web/sidebar-region.js", "src/web/terminal-input.js", "src/web/model-menu-input.js", "src/web/startup.js"];
try {
  report.binarySha256 = await hashFile(bin); report.controllerExecutableSha256 = await hashFile(realpathSync(process.execPath));
  report.sourceHashes = Object.fromEntries(await Promise.all(sources.map(async path => [path, await hashFile(resolve(import.meta.dir, "../..", path))])));
  report.fixtureSha256 = createHash("sha256").update(PRODUCER + JSON.stringify(params) + JSON.stringify(report.sourceHashes)).digest("hex");
  if (params.ownedInspector) { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); inspectPort = s.port; s.stop(true); }
  if (inspectPort === 3787 || inspectPort === port) throw Error("Refusing inspector port collision");
  hub = Bun.spawn([bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    cwd: project, env: { HOME: user, USERPROFILE: user, XDG_CONFIG_HOME: join(user, "config"), XDG_CACHE_HOME: join(user, "cache"), XDG_DATA_HOME: join(user, "data"),
      CODEX_HOME: join(user, "codex"), CLAUDE_CONFIG_DIR: join(user, "claude"), GEMINI_HOME: join(user, "gemini"), TMPDIR: root,
      PATH: `${agents}:${dirname(process.execPath)}:/usr/bin:/bin`, DISPLAY: "", WAYLAND_DISPLAY: "", CI: "1", DO_NOT_TRACK: "1",
      BACKPLANE_AGENTS: agents, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_TELEMETRY: "off", BEND_NO_TELEMETRY: "1",
      BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", SOAK_ROOT: root,
      ...(params.ownedInspector ? { BUN_OPTIONS: `--inspect=127.0.0.1:${inspectPort}` } : {}) }, stdout: "ignore", stderr: Bun.file(join(root, "hub.stderr")) });
  report.hubPid = hub.pid; report.port = port;
  const deadline = mono() + params.startupTimeoutMs;
  for (;;) { guard(); try { if ((await fetch(url + "/hello", { signal: AbortSignal.timeout(500) })).ok) break; } catch {} if (mono() > deadline) throw Error("Owned hub startup timeout"); await sleep(50); }
  report.processExecutable = readlinkSync(`/proc/${hub.pid}/exe`);
  const argv = readFileSync(`/proc/${hub.pid}/cmdline`, "utf8").split("\0");
  if (report.processExecutable !== bin && !(report.processExecutable === realpathSync(process.execPath) && argv.includes(bin))) throw Error("Non-exec launcher would measure the wrong PID");
  report.executingImageSha256 = await hashFile(`/proc/${hub.pid}/exe`);
  if (report.processExecutable === bin && report.executingImageSha256 !== report.binarySha256) throw Error("Spawned binary hash changed");
  if (params.ownedInspector) report.startupHeapDiagnostic = await inspectorSample();
  processMemory(); const observer = new Client("observer"); await observer.open();
  let visible = new Client("visible", { kind: "thread", id: ids[0] }), sidebar = new Client("sidebar", { kind: "recent", id: "" });
  await visible.open(); await sidebar.open();
  for (const id of ids) await observer.send({ id: ++observer.n, m: "turn.start", p: { thread: id, text: `rss-soak-thread=${id}`, accountProfiles: 1, accountBinding: "" } });
  await until("all 20 actual streams and commits", () => streams.every(s => s.delivered >= 4 && s.messages > 0 && s.tools > 0), params.startupTimeoutMs);
  let index = 0, nextSwitch = mono() + params.changeViewSecs * 1000, nextReconnect = mono() + params.reconnectSecs * 1000;
  async function churn() {
    if (mono() >= nextReconnect) {
      visible.close(); sidebar.close();
      await until("disposable sockets/pumps retire", () => !visible.pumping && !sidebar.pumping && visible.ws.readyState === WebSocket.CLOSED && sidebar.ws.readyState === WebSocket.CLOSED);
      visible = new Client("visible", { kind: "thread", id: ids[index] }); sidebar = new Client("sidebar", { kind: "recent", id: "" });
      await visible.open(); await sidebar.open(); reconnects++; nextReconnect = mono() + params.reconnectSecs * 1000;
    }
    if (mono() >= nextSwitch) { index = (index + 1) % ids.length; await visible.select({ kind: "thread", id: ids[index] }); switches++; nextSwitch = mono() + params.changeViewSecs * 1000; }
  }
  const warmEnd = mono() + params.warmupSecs * 1000;
  while (mono() < warmEnd) { guard(); processMemory(); await churn(); await sleep(200); }
  measuredAt = mono(); report.measurementStartMonotonicMs = measuredAt;
  const warmSwitches = switches, warmReconnects = reconnects;
  let checkpointAt = measuredAt, nextSample = measuredAt, checkpoint = streams.map((s, i) => ({ delivered: s.delivered, messages: s.messages, tools: s.tools, produced: receipt(ids[i]).seq }));
  for (const s of streams) { s.gap = 0; s.lag = new Histogram(); }
  let previousVisibleReplies = visible.replyCount, previousSidebarReplies = sidebar.replyCount;
  async function health(now: number) {
    const seconds = (now - checkpointAt) / 1000, summaries: Row = {};
    for (let i = 0; i < streams.length; i++) {
      const s = streams[i], p = receipt(ids[i]);
      const verdict = workloadVerdict(s, checkpoint[i], p.seq - checkpoint[i].produced, seconds, now); summaries[ids[i]] = verdict;
      const receiptAgeMs = Date.now() - p.at;
      if (!verdict.ok || receiptAgeMs < -params.maxClockSkewMs || receiptAgeMs > params.maxGapMs) {
        emit("invalid-workload", { thread: ids[i], ...verdict, receiptAgeMs }); throw Error(`Workload failed: ${ids[i]}`);
      }
      checkpoint[i] = { delivered: s.delivered, messages: s.messages, tools: s.tools, produced: p.seq }; s.gap = 0; s.lag = new Histogram();
    }
    // The visible reply must publish actual completed content; the static
    // pending label and heartbeats alone are not proof of view liveness.
    if (!completedViewHealthy(visible.newestShownAt, visible.newestShown, now, Date.now()))
      throw Error("Visible completed content stalled");
    // Counts are per disposable connection, so a reconnect resets baselines.
    if (visible.replyCount <= previousVisibleReplies || sidebar.replyCount <= previousSidebarReplies) throw Error("Visible/sidebar refresh replies stalled");
    previousVisibleReplies = visible.replyCount; previousSidebarReplies = sidebar.replyCount;
    healthChecks++; emit("health", { seconds, threads: summaries, visibleReplies: visible.replyCount, sidebarReplies: sidebar.replyCount, switches, reconnects }); checkpointAt = now;
  }
  const end = measuredAt + params.durationSecs * 1000;
  while (mono() < end) {
    guard(); const mem = processMemory(); report.peakObservedRssKiB = Math.max(report.peakObservedRssKiB ?? 0, mem.VmRSS); report.peakHighWaterKiB = Math.max(report.peakHighWaterKiB ?? 0, mem.VmHWM);
    bound("threadStates", streams.length); bound("receiptFiles", ids.length); bound("sampleHistory", 0); bound("errorHistory", 0); bound("clients", clients.size + Number(!!inspector));
    for (const c of clients) c.cacheBounds(); for (const s of streams) { bound("histogramBins", s.lag.bins.length); if (mono() - s.lastAt > params.maxGapMs) throw Error("Dead stream during soak"); }
    if (mono() - checkpointAt >= params.healthSecs * 1000 && end - mono() >= 1000) {
      await health(mono());
    }
    if (mono() >= nextSample) { await takeMemory(); nextSample += params.sampleSecs * 1000; }
    const beforeReconnect = reconnects;
    await churn();
    if (reconnects !== beforeReconnect) { previousVisibleReplies = 0; previousSidebarReplies = 0; }
    await sleep(200);
  }
  if (mono() - checkpointAt >= 2000) await health(mono());
  await takeMemory(); guard(); const result = regression.result(); report.memoryGrowth = result;
  if (!result.ok) throw Error(`RSS slope/endpoint exceeds 10%/hour or samples missing: ${JSON.stringify(result)}`);
  report.measuredChurn = { switches: switches - warmSwitches, reconnects: reconnects - warmReconnects };
  if (healthChecks < Math.floor(params.durationSecs / params.healthSecs) - 1 || report.measuredChurn.switches < Math.max(1, Math.floor(params.durationSecs / params.changeViewSecs * 0.8)) ||
    params.durationSecs >= params.reconnectSecs && report.measuredChurn.reconnects < Math.max(1, Math.floor(params.durationSecs / params.reconnectSecs * 0.8))) throw Error("Insufficient measured workload/churn coverage");
  if (!sidebar.replyCount || !visible.replyCount) throw Error("Zero thin view replies");
  for (const path of sources) if (await hashFile(resolve(import.meta.dir, "../..", path)) !== report.sourceHashes[path]) throw Error(`Fixture/toolchain source changed during soak: ${path}`);
  report.sourceUnchanged = true;
  report.ok = true;
} catch (error) { report.ok = false; report.error = String(error instanceof Error ? error.stack : error); }
finally {
  if (report.ok !== true || failure) {
    report.ok = false;
    report.finalProcMemory = { hubPid: hub?.pid ?? null, ...finalProcDiagnostic(() => {
      if (!hub) throw Error("Owned child was not started");
      return processMemory(false);
    }, mono(), regression.first) };
    try { emit("failure-proc-memory", report.finalProcMemory); }
    catch (error) { report.finalProcMemoryWriteError = String(error); }
  }
  stopping = true; writeFileSync(join(root, "stop"), "1"); for (const c of clients) c.close(); inspector?.close();
  if (hub && hub.exitCode === null) {
    hub.kill("SIGTERM"); const timer = setTimeout(() => { if (hub!.exitCode === null) hub!.kill("SIGKILL"); }, 3000);
    try { report.hubExit = await hub.exited; } finally { clearTimeout(timer); }
  }
  // Keep stop present through at least one complete fake-producer tick.
  await sleep(300);
  report.memoryGrowth ??= regression.result(); report.transportError = failure?.message;
  if (failure) report.ok = false;
  report.totals = { deliveredFrames, receivedBytes, commitEvents, switches, reconnects, healthChecks, successfulRequests: requestLag.n, requestP99UpperMs: requestLag.p99() };
  report.streams = Object.fromEntries(ids.map((id, i) => [id, { delivered: streams[i].delivered, messages: streams[i].messages, tools: streams[i].tools, lastSeq: streams[i].lastSeq }]));
  report.stderrTail = await tailStderr(); report.finishedAt = new Date().toISOString();
  report.measurementElapsedMs = measuredAt ? mono() - measuredAt : 0;
  emit("finished", { ok: report.ok, error: report.error, memoryGrowth: report.memoryGrowth }); closeSync(sampleFd);
  writeFileSync(out, JSON.stringify(report, null, 2) + "\n");
  if (!options.has("--keep")) rmSync(root, { recursive: true, force: true });
  console.log(`${report.ok ? "PASS" : "FAIL"} ${out}; samples ${out}.samples.jsonl`);
}
process.exitCode = report.ok ? 0 : 1;
