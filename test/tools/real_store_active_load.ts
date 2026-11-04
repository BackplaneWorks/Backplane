// Preparation only until the copied-store evaluator releases execution.
// No build, install, arbitrary home/target, inspector, GC or inherited auth.
import { appendFileSync, chmodSync, closeSync, createReadStream, existsSync, fstatSync, lstatSync,
  mkdirSync, mkdtempSync, openSync, readFileSync, readlinkSync, realpathSync, renameSync,
  rmSync, writeFileSync, writeSync } from 'node:fs';
import { createHash, randomBytes } from 'node:crypto';
import { createGunzip } from 'node:zlib';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { cbor, plain } from '../../src/web/wire.js';
import { ThinViews } from '../../src/web/thin.js';
const P = require('./real_store_active_policy.cjs');
type Row = Record<string, any>;
const ensure = (v: any, code: string) => { if (!v) throw Error(code); };
const mono = () => Number(process.hrtime.bigint()) / 1e6;
const sleep = (ms: number) => new Promise<void>(r => setTimeout(r, ms));
const sha = (b: string | Uint8Array) => createHash('sha256').update(b).digest('hex');
async function hashFile(path: string, end?: number) {
  const h = createHash('sha256');
  for await (const b of createReadStream(path, end === undefined ? {} : { start: 0, end: end - 1 })) h.update(b);
  return h.digest('hex');
}
function smallJson(path: string, bound = 65536) {
  const s = lstatSync(path); ensure(s.isFile() && !s.isSymbolicLink() && s.nlink === 1 && s.size <= bound, 'small-input');
  return JSON.parse(readFileSync(path, 'utf8'));
}
// Do not import either existing executable harness (their top levels run).
const dictSource = readFileSync(resolve(import.meta.dir, '../../src/core/cbor.bend'), 'utf8');
function dictionary(name: string) {
  const at = dictSource.indexOf(`def Cbor.${name}()`); ensure(at >= 0, 'dictionary');
  return JSON.parse(dictSource.slice(dictSource.indexOf('[', at), dictSource.indexOf(']', at) + 1));
}
const keys = dictionary('keys'), words = dictionary('words');
function encode(value: any): Uint8Array {
  const out: number[] = [];
  const head = (m: number, n: number) => {
    if (n < 24) out.push(m << 5 | n);
    else if (n < 256) out.push(m << 5 | 24, n);
    else if (n < 65536) out.push(m << 5 | 25, n >> 8, n & 255);
    else out.push(m << 5 | 26, n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255);
  };
  function put(v: any) {
    if (v === null) out.push(246);
    else if (typeof v === 'boolean') out.push(v ? 245 : 244);
    else if (typeof v === 'number') { out.push(199); put(String(v)); }
    else if (typeof v === 'string') { const b = new TextEncoder().encode(v); head(3, b.length); for (const x of b) out.push(x); }
    else if (Array.isArray(v)) { head(4, v.length); for (const x of v) put(x); }
    else { const es = Object.entries(v); head(5, es.length); for (const [k, x] of es) { put(k); put(x); } }
  }
  put(value); return new Uint8Array(out);
}
// Constant-size overwrite receipts, not a growing per-delta controller log.
// The two retained epochs allow the controller to finish the previous window
// while all producers continue emitting in the next one. hrtime is one host's
// monotonic clock, shared by the hub's owned children and this controller.
const producer = String.raw`
const fs = require('node:fs'), readline = require('node:readline');
const P = require(process.env.REAL_ACTIVE_POLICY), now = () => Number(process.hrtime.bigint()) / 1e6;
if (process.argv.includes('--version')) { console.log('2.1.286 (owned stand-in)'); process.exit(0); }
const root = process.env.REAL_ACTIVE_ROOT, run = process.env.REAL_ACTIVE_RUN;
const ids = JSON.parse(process.env.REAL_ACTIVE_IDS), sid = 'owned-' + process.pid;
const emit = v => fs.writeSync(1, JSON.stringify({...v, session_id:sid}) + '\n');
let running = false;
async function go(input) {
  const id = /real-active-thread=([a-z0-9-]+)/.exec(JSON.stringify(input))?.[1], index = ids.indexOf(id);
  if (running || index < 0) throw Error('owned-producer-input'); running = true;
  const states = [], file = root + '/producer-' + index + '.json';
  let seq = 0, block = 0, text = '', cycle = now(), lastWrite = 0, next = now();
  const receipt = () => {
    const row = {schema:1,run,index,pid:process.pid,at:now(),states:states.map(s => ({epoch:s.epoch,
      count:s.a.count,first:s.a.first,last:s.a.last,maxInteriorGapMs:s.a.maxInteriorGapMs,buckets:[...s.a.buckets],
      bytes:s.bytes,firstSeq:s.firstSeq,lastSeq:s.lastSeq}))};
    fs.writeFileSync(file + '.new', JSON.stringify(row), {mode:0o600});fs.renameSync(file + '.new', file);
  };
  emit({type:'system',subtype:'init',cwd:process.cwd(),tools:['Bash'],model:'fixture',permissionMode:'default'});
  const start = () => { block++;text='';
    emit({type:'stream_event',event:{type:'message_start',message:{id:id+'-'+block,role:'assistant',content:[],usage:{input_tokens:10}}}});
    emit({type:'stream_event',event:{type:'content_block_start',index:0,content_block:{type:'text',text:''}}});
  }; start();
  for (;;) {
    const c = JSON.parse(fs.readFileSync(root + '/control.json','utf8'));
    if (c.mode === 'stop') break;
    if (!['mixed','pure'].includes(c.mode)) throw Error('owned-producer-phase');
    const at = now(), marker = '~ra:' + run + ':' + index + ':' + (++seq) + ':' + Math.floor(at) + ':' + c.epoch + ':' + c.mode + '~';
    if (marker.length >= 96) throw Error('owned-marker-bound');
    const delta = marker.padEnd(95,'x') + ' ';
    emit({type:'stream_event',event:{type:'content_block_delta',index:0,delta:{type:'text_delta',text:delta}}});
    if (at >= c.start && at < c.end) {
      let s = states.find(s => s.epoch === c.epoch);
      if (!s) {s={epoch:c.epoch,a:new P.Activity(c.start,c.end),bytes:0,firstSeq:seq,lastSeq:seq};states.push(s);if(states.length>2)states.shift();}
      s.a.add(at);s.bytes+=Buffer.byteLength(delta);s.lastSeq=seq;
    }
    text += delta;
    if (c.mode === 'mixed' && at - cycle >= 500) {
      emit({type:'stream_event',event:{type:'content_block_stop',index:0}});
      emit({type:'assistant',message:{id:id+'-'+block,role:'assistant',content:[{type:'text',text}],usage:{input_tokens:10,output_tokens:seq}}});
      const tool=id+'-tool-'+block;
      emit({type:'assistant',message:{id:tool,role:'assistant',content:[{type:'tool_use',id:tool,name:'Bash',input:{command:'echo owned-fixture',description:'Owned synthetic cycle'}}]}});
      emit({type:'user',message:{role:'user',content:[{type:'tool_result',tool_use_id:tool,content:'owned synthetic result',is_error:false}]}});
      cycle=at;start();
    }
    if (text.length > 1024*1024) throw Error('owned-producer-text-bound');
    if (at-lastWrite>=100) {receipt();lastWrite=at;}
    // No catch-up bursts after either process stalls.
    next=Math.max(next+25,now());await new Promise(r=>setTimeout(r,Math.max(1,next-now())));
  }
  receipt();process.exit(0);
}
readline.createInterface({input:process.stdin}).on('line',line=>{
  const v=JSON.parse(line);if(v.type==='user')go(v).catch(()=>{console.error('owned-producer-failed');process.exit(1)});
});
`;

// Small no-hub checks only. The evaluator must separately authorize their
// normal capped Boat execution after reviewing the frozen preparation.
if (process.argv.slice(2).includes('--self-test')) {
  ensure(process.argv.length === 3, 'self-test-options');
  require('./real_store_active_policy_test.cjs');
  new Bun.Transpiler({ loader: 'js' }).transformSync(producer);
  const v = { id: 2 ** 47 + 17, m: 'view.open', p: { a: [1, '✓', false, null] } };
  ensure(JSON.stringify(plain(cbor(encode(v), keys, words))) === JSON.stringify(v), 'codec-roundtrip');
  await require('./real_store_active_producer_test.cjs').run(producer);
  console.log('ok combined controller codec, producer phase switch and boundary controls; no hub started');
  process.exit(0);
}

const o = P.options(process.argv.slice(2));
ensure(process.platform === 'linux', 'linux-required');
const bin = realpathSync(o['--bin']), fixture = realpathSync(o['--fixture']), out = resolve(o['--out']);
ensure(bin === o['--bin'] && fixture === o['--fixture'], 'input-symlink');
ensure(![bin, fixture, o['--artifact-receipt']].includes(out) && !existsSync(out) && !existsSync(out + '.samples.jsonl'), 'output-collision');
const root = mkdtempSync(join(tmpdir(), 'bp-real-active-')); chmodSync(root, 0o700);
let fence = '';
const fenceInputs: { path: string; sha256: string }[] = [];
const home = join(root, 'home'), user = join(root, 'user'), agents = join(root, 'agents'), project = join(root, 'project');
for (const p of [home, user, agents, project, join(root, 'locks')]) mkdirSync(p, { mode: 0o700 });
mkdirSync(dirname(out), { recursive: true });
const samplesFd = openSync(out + '.samples.jsonl', 'wx', 0o600);
const run = randomBytes(6).toString('hex'), prefix = 'ra-' + run + '-', ids = Array.from({ length: 20 }, (_, i) => prefix + i);
const report: Row = { schema: 1, scope: 'private-RC3-copy-plus-20-active-producers', preparation: false,
  ok: false, forcedGC: false, inspector: false, copiedAccountProfilesInstalled: false,
  limits: P.LIMITS, requestedLoadedMs: o.seconds * 1000, checks: {}, phases: [], samplesFileSuffix: '.samples.jsonl',
  cpuScope: 'captured live hub PID utime+stime; 100% = one core; no normalization or subtraction',
  startedAt: new Date().toISOString() };
let hub: ReturnType<typeof Bun.spawn> | undefined, fault: string | undefined, cleaning = false, bootAt = 0;
let port = 0, token = '', logBytes = 0, logTail = '', listening = false, stackError = false;
let birth = 0, originalBytes = 0, loadedStart = 0, loadedEnd = 0, current: Row | undefined;
let lastSample = 0, nextViewRead = 0, sourceHashes: Row = {}, before: Row | undefined;
const samples: Row[] = [], clients: Client[] = [], producerIdentities = new Map<number, number>();
const logReaders: ReadableStreamDefaultReader<Uint8Array>[] = [];
const watchdog: [number, number][] = [], delayedNotices: Row[] = [];
const control = (c: Row) => { writeFileSync(join(root, 'control.next'), JSON.stringify(c), { mode: 0o600 }); renameSync(join(root, 'control.next'), join(root, 'control.json')); };
const failed = (code: string) => { fault ??= code; };
function guard() { ensure(!fault, fault ?? 'fault'); ensure(hub?.exitCode === null, 'child-exited'); }
async function until(check: () => boolean, ms: number) {
  const end = mono() + ms;
  while (!check()) { guard(); ensure(mono() < end, 'deadline'); if (mono() - lastSample >= 5000) sample(); await sleep(25); }
  guard();
}
function identity(pid: number, executable: string, argvItem: string) {
  const exe = readlinkSync(`/proc/${pid}/exe`), argv = readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0');
  return exe === executable || exe === realpathSync(process.execPath) && argv.includes(argvItem);
}
function sample() {
  guard(); const at = mono() - bootAt;
  const s = P.processSample(readFileSync(`/proc/${hub!.pid}/stat`, 'utf8'), readFileSync(`/proc/${hub!.pid}/status`, 'utf8'),
    hub!.pid, identity(hub!.pid, bin, bin), hub!.exitCode === null, at);
  if (birth) ensure(s.birth === birth, 'child-replaced'); else birth = s.birth;
  ensure(samples.length < 900, 'sample-bound'); samples.push(s); lastSample = mono();
  writeSync(samplesFd, JSON.stringify(s) + '\n');
  ensure(s.VmSwapBytes === 0, 'hub-swap');
  // Abort runaway baselines while still retaining a failing aggregate report.
  ensure(s.VmRSSBytes < 3000000000, 'hub-resource-stop');
  return s;
}
async function drain(stream: ReadableStream<Uint8Array>) {
  const reader = stream.getReader(), decoder = new TextDecoder();
  logReaders.push(reader);
  for (;;) {
    const { value, done } = await reader.read(); if (done) break;
    logBytes += value.length; const text = decoder.decode(value, { stream: true });
    logTail = (logTail + text).slice(-16384);
    listening ||= /Backplane listening on http:\/\/|web server on:/.test(logTail);
    stackError ||= /RangeError.*stack|Maximum call stack/i.test(logTail);
    if (logBytes > 64 * 1024 * 1024) failed('child-log-bound');
  }
}
// Extract regular members ourselves: reject traversal, symlinks, hard links,
// duplicate headers, PAX overrides and oversized entries BEFORE writing any
// member. Stream gzip/tar; never materialize the private archive or log.
async function extractArchive() {
  const seen = new Set<string>(); let header = Buffer.alloc(0), left = 0, padding = 0, fd: number | undefined;
  let total = 0, zeros = 0, expanded = 0;
  try {
    for await (const raw of createReadStream(fixture).pipe(createGunzip())) {
      expanded += raw.length; ensure(expanded <= 8 * 1024 ** 3 + 1048576, 'archive-expanded-bound');
      let b = Buffer.from(raw);
      while (b.length) {
        if (left) {
          const n = Math.min(left, b.length); if (fd !== undefined) { let w = 0; while (w < n) { const k = writeSync(fd, b, w, n - w); ensure(k > 0, 'archive-write'); w += k; } }
          left -= n; b = b.subarray(n); if (!left && fd !== undefined) { closeSync(fd); fd = undefined; } continue;
        }
        if (padding) { const n = Math.min(padding, b.length); padding -= n; b = b.subarray(n); continue; }
        const n = Math.min(512 - header.length, b.length); header = Buffer.concat([header, b.subarray(0, n)]); b = b.subarray(n);
        if (header.length < 512) continue;
        if (header.every(x => x === 0)) { zeros++; header = Buffer.alloc(0); continue; }
        ensure(zeros === 0, 'archive-trailing-data');
        const field = (a: number, z: number) => header.subarray(a, z).toString('utf8').replace(/\0.*$/s, '');
        const octal = (s: string) => { ensure(/^[ 0-7]+\0?[ ]*$/.test(s), 'archive-number'); return parseInt(s.replace(/\0/g, '').trim(), 8); };
        const sum = [...header].reduce((s, v, i) => s + (i >= 148 && i < 156 ? 32 : v), 0);
        ensure(octal(header.subarray(148, 156).toString('ascii')) === sum && field(345, 500) === '', 'archive-header');
        const name = field(0, 100), size = octal(header.subarray(124, 136).toString('ascii'));
        const retain = P.archiveMember(name, String.fromCharCode(header[156]), size, seen);
        total += size; ensure(total <= 8 * 1024 ** 3, 'archive-total');
        if (retain) fd = openSync(join(home, name), 'wx', 0o600);
        left = size; padding = (512 - size % 512) % 512; header = Buffer.alloc(0);
      }
    }
    ensure(left === 0 && padding === 0 && header.length === 0 && zeros >= 2 &&
      ['RECEIPT.json', 'events.jsonl', 'store.db'].every(n => seen.has(n)), 'archive-incomplete');
    return { members: seen.size, extractedBytes: total };
  } finally { if (fd !== undefined) closeSync(fd); }
}
async function stageFence() {
  const receipt = smallJson(o['--fence-prerequisite']), qualified = P.fencePrerequisite(receipt);
  report.fencePrerequisite = { qualifiedJob: qualified.qualifiedJob, manifestSha256: qualified.manifestSha256,
    originalOverallPass: false, osNetworkIsolationClaimed: false, receiptSha256: await hashFile(o['--fence-prerequisite']) };
  fenceInputs.push({ path: o['--fence-prerequisite'], sha256: report.fencePrerequisite.receiptSha256 });
  const dir = join(root, 'preload'); mkdirSync(dir, { mode: 0o700 });
  for (const f of qualified.files) {
    // A Boat packet may place the exact byte files beside the original
    // receipt. Otherwise use its owner-supplied paths, read-only.
    const sibling = join(dirname(o['--fence-prerequisite']), f.name), source = existsSync(sibling) ? sibling : f.path;
    const st = lstatSync(source); ensure(st.isFile() && !st.isSymbolicLink() && st.size <= 65536, 'fence-source');
    const bytes = readFileSync(source); ensure(sha(bytes) === f.sha256, 'fence-source-sha');
    fenceInputs.push({ path: source, sha256: f.sha256 }); writeFileSync(join(dir, f.name), bytes, { mode: 0o600, flag: 'wx' });
  }
  fence = join(dir, 'real_store_fence.cjs'); report.fenceSha256 = await hashFile(fence);
  ensure(report.fenceSha256 === o['--fence-sha256'], 'fence-sha');
  // Test this packaged artifact's actual preload handling before extracting
  // any private data. This is CLI --version, outside the startup/load clock.
  const path = join(root, 'preflight-fence.json');
  const probe = Bun.spawn([bin, '--version'], { cwd: root, env: { HOME: user, USERPROFILE: user,
    XDG_CONFIG_HOME: join(user, 'config'), XDG_CACHE_HOME: join(user, 'cache'), XDG_DATA_HOME: join(user, 'data'),
    PATH: '/usr/bin:/bin', BACKPLANE_COPY: '1', BACKPLANE_TELEMETRY: 'off', BEND_NO_TELEMETRY: '1',
    BACKPLANE_OWNED_FENCE_RECEIPT: path, BUN_OPTIONS: `--preload=${fence}` }, stdout: 'pipe', stderr: 'ignore' });
  const timer = setTimeout(() => { if (probe.exitCode === null) probe.kill('SIGKILL'); }, 5000);
  let bytes = 0;
  try {
    for await (const b of probe.stdout) { bytes += b.length; ensure(bytes <= 16384, 'preflight-output'); }
    ensure(await probe.exited === 0, 'preflight-exit');
    const installed = smallJson(path);
    ensure(installed.installed === true && installed.externalFetchBlocked === true && installed.externalConnectBlocked === true, 'preflight-fence');
    report.preflight = { cliVersionExit: 0, installed: true, beforeFixtureExtraction: true };
  } finally { clearTimeout(timer); if (probe.exitCode === null) { probe.kill('SIGKILL'); await probe.exited; } }
}
async function storeCounts() {
  const { Database } = await import('bun:sqlite'), db = new Database(join(home, 'store.db'), { readonly: true, create: false });
  const counts: Row = {};
  try {
    db.exec('BEGIN');
    for (const table of ['line', 'entry', 'project', 'thread', 'setting', 'bot', 'room', 'routine', 'ask', 'queue']) {
      counts[table] = Number((db.query(`SELECT count(*) AS n FROM ${table}`).get() as Row).n);
    }
    const seq = Number((db.query("SELECT v FROM meta WHERE k='applied_seq'").get() as Row).v);
    const originalThreads = Number((db.query('SELECT count(*) AS n FROM thread WHERE substr(id,1,?)<>?').get(prefix.length, prefix) as Row).n);
    const syntheticThreads = counts.thread - originalThreads;
    const h = createHash('sha256'); let oldEntryRows = 0;
    // Immutable index facts for the original prefix, one row at a time.
    for (const row of db.query('SELECT seq,ord,id,thread,src,kind,role,akind,text,at FROM entry WHERE seq<=1448789 ORDER BY seq').iterate()) {
      h.update(JSON.stringify(row) + '\n'); oldEntryRows++;
    }
    const catalog = createHash('sha256');
    for (const row of db.query('SELECT id,title,root FROM project WHERE substr(id,1,?)<>? ORDER BY id').iterate(prefix.length, prefix)) catalog.update('project:' + JSON.stringify(row) + '\n');
    for (const row of db.query('SELECT id,project,title FROM thread WHERE substr(id,1,?)<>? ORDER BY id').iterate(prefix.length, prefix)) catalog.update(JSON.stringify(row) + '\n');
    const target = db.query("SELECT id FROM thread WHERE deleted=0 AND archived=0 ORDER BY activity DESC LIMIT 1").get() as Row;
    ensure(target?.id, 'fixture-target');
    return { counts, seq, originalThreads, syntheticThreads, originalEntryRows: oldEntryRows,
      originalEntrySha256: h.digest('hex'), originalCatalogSha256: catalog.digest('hex'), target: target.id };
  } finally { db.close(); }
}
function countReport(s: Row) { const { target, ...safe } = s; return safe; }
function markers(text: string, visit: (m: Row) => void) {
  const re = /~ra:([a-f0-9]{12}):(\d{1,2}):(\d+):(\d+):(\d+):(mixed|pure)~/g;
  for (const m of text.matchAll(re)) {
    if (m[1] !== run) continue;
    const row = { index: Number(m[2]), seq: Number(m[3]), emitted: Number(m[4]), epoch: Number(m[5]), mode: m[6] };
    ensure(row.index < 20 && row.seq > 0 && Number.isSafeInteger(row.seq) && Number.isSafeInteger(row.emitted), 'marker-format'); visit(row);
  }
}
function cleanNotices(at: number) {
  while (watchdog.length && watchdog[0][1] < at - 3000) watchdog.shift();
  while (delayedNotices.length && delayedNotices[0].at < at - 1500) {
    const n = delayedNotices.shift()!;
    if (n.window.mode === 'pure' && n.at >= n.window.start + 2000 && !watchdog.some(([a, b]) => n.at >= a && n.at <= b)) n.window.foreignNotices++;
  }
}
class Client {
  ws: WebSocket; views: any; session: Row; greeting: any; joined = false; ready = false;
  pending = new Map<number, Row>(); sequence = 0; notices: number[] = []; replies = 0; refreshing = false;
  lastPending = false; lastShownSeq = 0; lastDelivered: number[] = Array(20).fill(0); controlBytes = 0;
  constructor(readonly kind: 'observer' | 'own' | 'quiet' | 'sidebar', readonly target?: Row, cursor?: Row) {
    const base = `ws://127.0.0.1:${port}`; ensure(P.ownedEndpoint(base, port), 'endpoint');
    const suffix = target ? '&view=1&frontend=1' : `&since=${cursor!.seq}&origin=${encodeURIComponent(cursor!.epoch)}`;
    this.ws = new WebSocket(`${base}/ws?token=${token}${suffix}`); this.ws.binaryType = 'arraybuffer'; clients.push(this);
    this.session = { sel: target?.kind === 'thread' ? target.id : '', drafts: {}, open: {}, info: {}, adding: false, panel: false };
    if (target) this.views = new ThinViews({ now: () => mono(), sidebarRegions: true, viewDelta: true, send: (_: string, r: Row) => r, command: (c: Row) => c });
    this.ws.onerror = () => { if (!cleaning) failed('view-transport'); };
    this.ws.onclose = () => { if (!cleaning) failed('view-closed'); };
    this.ws.onmessage = e => { try { this.receive(e); } catch { failed('view-protocol'); } };
  }
  receive(e: MessageEvent) {
    const at = mono(), bytes = typeof e.data === 'string' ? Buffer.byteLength(e.data) : e.data.byteLength;
    ensure(bytes <= (this.target ? 1048576 : 8 * 1024 * 1024), 'frame-bound');
    const f = typeof e.data === 'string' ? JSON.parse(e.data) : plain(cbor(new Uint8Array(e.data as ArrayBuffer), keys, words));
    ensure(typeof f?.t === 'string', 'frame-type');
    if (f.t === 'view' && this.target) { ensure(!this.greeting, 'duplicate-greeting'); this.greeting = f; }
    if (f.t === 'log' && !this.target) { ensure(Number(f.since) >= 1448789 && (f.items?.length ?? 0) <= 1024, 'historical-replay'); this.joined = true; }
    if (!this.target && f.t === 'delta') markers(f.text ?? '', m => {
      ensure(f.thread === ids[m.index], 'stream-thread');
      if (m.seq <= this.lastDelivered[m.index]) return; this.lastDelivered[m.index] = m.seq;
      if (!current || at >= current.end || m.epoch !== current.epoch) {
        if (current && at >= current.start + 2000 && at < current.end) current.threads[m.index].badPhase++;
        return;
      }
      const t = current.threads[m.index];
      // Marker stamps are floored to 1ms. Receipt stamps retain fractions.
      if (m.mode !== current.mode || m.emitted + 1 < current.start || m.emitted >= current.end) { t.badPhase++; return; }
      t.activity.add(at); t.lag.add(at - m.emitted); t.maxDeliveredSeq = Math.max(t.maxDeliveredSeq, m.seq);
    });
    if (!this.target && f.t === 'changes') for (const item of f.items ?? []) {
      const isWatchdog = item.$ === 'SettingSet' && ids.some(id => item.key === 'liveness.' + id);
      if (isWatchdog) { ensure(watchdog.length < 256, 'watchdog-bound'); watchdog.push([at - 100, at + 1250]); }
      if (current?.mode === 'pure' && at < current.end && at > current.start + 2000 && !isWatchdog) current.unexpectedPureCommits++;
    }
    if (this.target && ['delta', 'changes', 'log', 'live'].includes(f.t)) throw Error('legacy-view-leak');
    if (f.t === 'view.control') {
      this.controlBytes += Buffer.byteLength(f.text ?? ''); ensure(this.controlBytes <= 4 * 1024 * 1024, 'control-bound');
      ensure(this.views?.control('owned', f), 'view-control'); return;
    }
    if (f.t === 'view.changed') {
      ensure(this.target, 'observer-notice');
      ensure(f.target?.kind === this.target!.kind && f.target?.id === this.target!.id &&
        f.epoch === this.greeting.epoch && f.generation === this.greeting.generation, 'foreign-notice');
      const call = this.views.changed('owned', f, this.session, { id: '', offset: 0 });
      ensure(!call?.failed, 'foreign-notice');
      this.notices = this.notices.filter(t => t > at - 975); this.notices.push(at);
      ensure(this.notices.length < 128, 'notice-bound');
      if (current && at < current.end) {
        if (this.kind === 'own') { current.ownNotices++; current.ownPeak = Math.max(current.ownPeak, this.notices.length); }
        else if (this.kind === 'sidebar') { current.sidebarNotices++; current.sidebarPeak = Math.max(current.sidebarPeak, this.notices.length); }
        if (this.kind === 'sidebar' || this.kind === 'quiet') {
          ensure(delayedNotices.length < 256, 'notice-queue-bound'); delayedNotices.push({ at, window: current });
        }
      }
      if (this.ready) void this.issue(call); return;
    }
    if (f.t === 'reply') {
      const p = this.pending.get(Number(f.id)); ensure(p, 'unrequested-reply');
      clearTimeout(p!.timer); this.pending.delete(Number(f.id));
      if (p!.window) { p!.window.clients[this.kind].replies++; p!.window.clients[this.kind].lag.add(at - p!.at); }
      if (!f.ok) p!.reject(Error('rpc-rejected')); else p!.resolve(f);
    }
  }
  async send(r: Row) {
    ensure(this.pending.size < 1 && this.ws.readyState === WebSocket.OPEN, 'request-bound');
    this.controlBytes = 0;
    return new Promise<Row>((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(Number(r.id)); reject(Error('request-timeout')); }, 15000);
      const window = this.target && current && mono() < current.end ? current : undefined;
      if (window) window.clients[this.kind].requests++;
      this.pending.set(Number(r.id), { resolve, reject, timer, window, at: mono() }); this.ws.send(encode(r));
    });
  }
  async issue(call: Row) {
    try {
      ensure(!call?.failed, 'view-call'); if (!call?.id) return;
      const raw = await this.send(call.cmds[0]), taken = this.views.reply('owned', raw);
      ensure(taken?.current && taken.remoteReady && taken.shown, 'view-reply'); this.session = taken.session; this.replies++;
      ensure(P.usablePicture(taken.shown, this.target), 'unusable-picture');
      const at = mono(); let latest: Row | undefined;
      markers(JSON.stringify(taken.shown), m => {
        if (this.kind !== 'own' || m.index !== 0) { if (current && at < current.end) current.foreignText++; return; }
        if (current?.mode === 'pure' && at < current.end && m.mode === 'pure' && m.epoch === current.epoch) current.partialVisible++;
        if (!latest || m.seq > latest.seq) latest = m;
      });
      if (this.kind === 'own') {
        this.lastPending = P.pendingRow(taken.shown, ids[0]).valid;
        if (current && at < current.end) {
          if (current.mode === 'pure' && at >= current.start + 2000) { current.pendingReplies++; if (!this.lastPending) current.invalidPending++; }
          if (current.mode === 'mixed' && latest && latest.epoch === current.epoch && latest.seq > this.lastShownSeq && latest.emitted + 1 >= current.start) {
            current.visible.add(at); current.visibleLag.add(at - latest.emitted); this.lastShownSeq = latest.seq;
          }
        }
      }
      if (taken.refreshDue) await this.issue(this.views.request('owned', 'view.update', { session: this.session, anchor: { id: '', offset: 0 } }));
    } catch { if (!cleaning) failed('view-request'); }
  }
  async open() {
    await until(() => this.ws.readyState === WebSocket.OPEN && (this.target ? !!this.greeting : this.joined), 15000);
    if (this.target) {
      ensure(this.greeting.renderer === 'web' && this.views.connect('owned', this.greeting).connected, 'view-connect');
      await this.issue(this.views.select('owned', this.target, this.session, 'owned-copy')); guard(); this.ready = true;
    }
  }
  update() {
    if (!this.ready || this.pending.size || this.refreshing) return;
    this.refreshing = true;
    void this.issue(this.views.request('owned', 'view.update', { session: this.session, anchor: { id: '', offset: 0 } }))
      .finally(() => { this.refreshing = false; });
  }
}

async function runWindow(index: number, tickHz: number) {
  const start = mono(), end = start + 60000, mode = index % 2 ? 'pure' : 'mixed';
  const c0 = sample();
  current = { epoch: index + 1, mode, start, end, foreignText: 0, foreignNotices: 0, partialVisible: 0,
    pendingReplies: 0, invalidPending: 0, ownPeak: 0, sidebarPeak: 0, unexpectedPureCommits: 0,
    ownNotices: 0, sidebarNotices: 0,
    clients: Object.fromEntries(['own', 'quiet', 'sidebar'].map(k => [k, { requests: 0, replies: 0, lag: new P.Latency() }])),
    pendingAtStart: clients.find(c => c.kind === 'own')!.lastPending,
    visible: new P.Activity(start, end), visibleLag: new P.Latency(), threads: ids.map(() => ({
      activity: new P.Activity(start, end), lag: new P.Latency(), maxDeliveredSeq: 0, badPhase: 0 })) };
  const w = current;
  control({ epoch: w.epoch, mode, start, end });
  while (mono() < end) {
    guard(); const at = mono(); cleanNotices(at);
    if (at - lastSample >= 5000) sample();
    if (at >= nextViewRead) { for (const c of clients) c.update(); nextViewRead = at + 1000; }
    await sleep(25);
  }
  const c1 = sample();
  // Resolve notice attribution using actual later observations, not a fake
  // clock advance. Producers and serialized view clients keep running. The
  // <=1.5s flush is included in whole-load CPU/RSS, with <=2s phase seams.
  const flushEnd = mono() + 1500;
  while (mono() < flushEnd) {
    guard(); cleanNotices(mono());
    if (mono() - lastSample >= 5000) sample();
    if (mono() >= nextViewRead) { for (const c of clients) c.update(); nextViewRead = mono() + 1000; }
    await sleep(25);
  }
  cleanNotices(mono());
  const threads = ids.map((_, i) => {
    const r = smallJson(join(root, `producer-${i}.json`)), state = r.states?.find((s: Row) => s.epoch === w.epoch);
    ensure(r.schema === 1 && r.run === run && r.index === i && r.at >= end - 1000 && r.at <= mono() + 10 && state, 'producer-receipt');
    registerProducer(r);
    return { index: i, produced: P.activitySummary(state, start, end), delivered: w.threads[i].activity.summary(),
      latency: w.threads[i].lag.summary(), bytes: state.bytes, receiptFresh: mono() - r.at <= 2000,
      deliveryWithinProduced: w.threads[i].maxDeliveredSeq <= state.lastSeq && w.threads[i].activity.count <= state.count,
      badPhase: w.threads[i].badPhase };
  });
  const visibleLatency = w.visibleLag.summary();
  // Completed content is published at two cycles/s, unlike 40Hz raw deltas.
  const visibleOk = w.visible.count >= (end - start) / 1000 && w.visible.first !== null &&
    Math.max(w.visible.first - start, end - w.visible.last, w.visible.maxInteriorGapMs) <= 2000 && visibleLatency.ok;
  const result: Row = { index, mode, start: start - bootAt, end: end - bootAt, cpu: P.cpu(c0, c1, tickHz), threads,
    visible: { replies: w.visible.count, latency: visibleLatency, ok: visibleOk }, foreignText: w.foreignText,
    foreignNotices: w.foreignNotices, pendingReplies: w.pendingReplies, invalidPending: w.invalidPending,
    pendingAtStart: w.pendingAtStart,
    ownNotices: w.ownNotices, sidebarNotices: w.sidebarNotices,
    clients: Object.fromEntries(Object.entries(w.clients).map(([k, v]: [string, any]) => [k, { requests: v.requests, replies: v.replies, requestLatency: v.lag.summary() }])),
    partialVisible: w.partialVisible, ownPeak: w.ownPeak, sidebarPeak: w.sidebarPeak, unexpectedPureCommits: w.unexpectedPureCommits };
  result.acceptance = P.phaseVerdict(result);
  if (mode === 'pure' && w.unexpectedPureCommits) result.acceptance.ok = false;
  report.phases.push(result); current = undefined;
  ensure(result.acceptance.ok, 'loaded-window-failed');
}

const producerPids = new Map<number, number>();
function registerProducer(r: Row) {
  ensure(r.run === run && r.schema === 1 && Number.isInteger(r.index) && r.index >= 0 && r.index < 20 && Number.isInteger(r.pid) && r.pid > 0, 'producer-receipt');
  const s = readFileSync(`/proc/${r.pid}/stat`, 'utf8'), f = s.slice(s.lastIndexOf(') ') + 2).trim().split(/\s+/), born = Number(f[19]);
  ensure(Number.isSafeInteger(born) && born > 0 && readFileSync(`/proc/${r.pid}/cmdline`, 'utf8').split('\0').includes(join(agents, 'producer.cjs')), 'producer-identity');
  const oldPid = producerPids.get(r.index), oldBirth = producerIdentities.get(r.pid);
  ensure((!oldPid || oldPid === r.pid) && (!oldBirth || oldBirth === born) &&
    ![...producerPids].some(([i, pid]) => i !== r.index && pid === r.pid), 'producer-replaced');
  producerPids.set(r.index, r.pid); producerIdentities.set(r.pid, born);
}

let drains: Promise<void>[] = [];
try {
  const artifact = smallJson(o['--artifact-receipt']); report.artifact = P.artifactReceipt(artifact, o['--sha256']);
  report.artifactReceiptSha256 = await hashFile(o['--artifact-receipt']);
  report.controller = { platform: process.platform, bun: Bun.version, executableSha256: await hashFile(realpathSync(process.execPath)) };
  ensure(lstatSync(bin).isFile() && lstatSync(fixture).isFile(), 'input-file');
  ensure(await hashFile(bin) === o['--sha256'], 'service-sha');
  await stageFence(); // Missing/mismatched prerequisite fails before extraction.
  report.fixtureArchiveSha256 = await hashFile(fixture); ensure(report.fixtureArchiveSha256 === o['--fixture-sha256'], 'archive-sha');
  const sourcePaths = ['test/tools/real_store_active_load.ts', 'test/tools/real_store_active_policy.cjs',
    'src/core/cbor.bend', 'src/web/wire.js', 'src/web/thin.js', 'src/web/sidebar-region.js', 'src/web/view-delta.js',
    'src/web/conversation-frame.js', 'src/web/terminal-input.js', 'src/web/model-menu-input.js', 'src/web/startup.js'];
  sourceHashes = Object.fromEntries(sourcePaths.map(p => [p, sha(readFileSync(resolve(import.meta.dir, '../..', p)))])); report.sourceHashes = sourceHashes;
  report.extraction = await extractArchive();
  const receiptPath = join(home, 'RECEIPT.json'); report.fixtureReceiptSha256 = await hashFile(receiptPath);
  ensure(report.fixtureReceiptSha256 === o['--fixture-receipt-sha256'], 'fixture-receipt-sha');
  report.fixture = P.fixtureReceipt(smallJson(receiptPath, 1048576));
  originalBytes = lstatSync(join(home, 'events.jsonl')).size;
  ensure(originalBytes > 0 && await hashFile(join(home, 'events.jsonl')) === report.fixture.logSha256, 'fixture-log-sha');
  before = await storeCounts(); ensure(before.seq === 1448789 && before.originalThreads === 10841 && before.syntheticThreads === 0, 'fixture-counts');
  report.before = countReport(before);
  // Only this private copy receives new events. The original authoritative
  // prefix and SQLite checkpoint are preserved, not replaced by a catalog.
  const e = ($: string, row: Row) => ({ $, ...row, at: Date.now() });
  const added = [e('ProjectCreated', { id: prefix + 'project', title: 'Owned synthetic load', root: project }),
    ...ids.map(id => e('ThreadCreated', { id, project: prefix + 'project', title: id, env: 'local', branch: '', worktree: project, provider: 'claude', model: 'fixture' })),
    e('SettingSet', { key: 'telemetry.share', value: 'off' })];
  const logFd = openSync(join(home, 'events.jsonl'), 'r+');
  try { const last = Buffer.alloc(1); ensure(fstatSync(logFd).size === originalBytes, 'fixture-length');
    // A complete original log is required; never repair or rewrite its tail.
    const { readSync } = await import('node:fs'); readSync(logFd, last, 0, 1, originalBytes - 1); ensure(last[0] === 10, 'fixture-torn-tail');
  } finally { closeSync(logFd); }
  appendFileSync(join(home, 'events.jsonl'), added.map(v => JSON.stringify(v)).join('\n') + '\n');
  report.appendedFixtureEvents = added.length;
  token = randomBytes(32).toString('hex'); writeFileSync(join(home, 'token'), token, { mode: 0o600 });
  control({ mode: 'mixed', epoch: 0, start: 0, end: 0 });
  writeFileSync(join(agents, 'producer.cjs'), producer, { mode: 0o600 });
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(agents, 'claude'), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(join(agents, 'producer.cjs'))} "$@"\n`, { mode: 0o700 });
  for (const name of ['codex', 'grok', 'muse', 'opencode', 'antigravity', 'agy', 'pi', 'omp', 'gh', 'tailscale', 'curl', 'wget', 'ssh', 'notify-send'])
    writeFileSync(join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o700 });
  const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); port = listener.port; listener.stop(true);
  ensure(P.ownedEndpoint(`http://127.0.0.1:${port}`, port), 'owned-port');
  const clk = Bun.spawnSync(['getconf', 'CLK_TCK'], { stdout: 'pipe', stderr: 'ignore', env: { PATH: '/usr/bin:/bin' } });
  const tickHz = Number(clk.stdout.toString().trim()); ensure(clk.exitCode === 0 && tickHz > 0, 'clock-ticks'); report.tickHz = tickHz;
  bootAt = mono();
  hub = Bun.spawn([bin, '--home', home, '--host', '127.0.0.1', '--port', String(port), '--no-tailscale', '--headless', '--foreground'], {
    cwd: project, env: { HOME: user, USERPROFILE: user, XDG_CONFIG_HOME: join(user, 'config'), XDG_CACHE_HOME: join(user, 'cache'), XDG_DATA_HOME: join(user, 'data'),
      CODEX_HOME: join(user, 'codex'), CLAUDE_CONFIG_DIR: join(user, 'claude'), GEMINI_HOME: join(user, 'gemini'),
      PATH: `${agents}:${dirname(process.execPath)}:/usr/bin:/bin`, TMPDIR: root, DISPLAY: '', WAYLAND_DISPLAY: '', CI: '1', DO_NOT_TRACK: '1',
      BACKPLANE_HOME: home, BACKPLANE_COPY: '1', BACKPLANE_AGENTS: agents, BACKPLANE_PEERS: '', BACKPLANE_NO_UPDATE: '1',
      BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_TELEMETRY: 'off', BEND_NO_TELEMETRY: '1', BACKPLANE_GOOGLE_BAKE: '0', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9',
      BACKPLANE_LOCK_DIR: join(root, 'locks'), BACKPLANE_OWNED_FENCE_RECEIPT: join(root, 'fence.json'),
      BUN_OPTIONS: `--preload=${fence}`,
      REAL_ACTIVE_ROOT: root, REAL_ACTIVE_RUN: run, REAL_ACTIVE_IDS: JSON.stringify(ids), REAL_ACTIVE_POLICY: resolve(import.meta.dir, 'real_store_active_policy.cjs') },
    stdout: 'pipe', stderr: 'pipe' });
  report.hubPid = hub.pid; drains = [drain(hub.stdout), drain(hub.stderr)].map(p => p.catch(() => { failed('child-log-stream'); }));
  // Sample as soon as the actual executable is admitted; never time a shell.
  sample(); report.childIdentity = { pid: hub.pid, birth, executableMatchesPackagedService: true };
  await until(() => listening, 120000); report.listenMs = mono() - bootAt;
  ensure(existsSync(join(root, 'fence.json')), 'fence-receipt-missing');
  const installed = smallJson(join(root, 'fence.json'));
  ensure(installed.installed === true && installed.externalFetchBlocked === true && installed.externalConnectBlocked === true, 'fence-not-installed');
  report.networkFence = { installed: true, sha256: report.fenceSha256 };
  const hello = await fetch(`http://127.0.0.1:${port}/hello`, { redirect: 'error', signal: AbortSignal.timeout(15000) });
  ensure(hello.ok, 'hello'); await hello.arrayBuffer();
  const quiet = new Client('quiet', { kind: 'thread', id: before.target }); await quiet.open();
  const sidebar = new Client('sidebar', { kind: 'recent', id: '' }); await sidebar.open();
  const own = new Client('own', { kind: 'thread', id: ids[0] }); await own.open();
  report.usableMs = mono() - bootAt; report.checks.usableWithin15s = report.usableMs <= 15000;
  // Startup includes usable real-thread, synthetic own-thread and sidebar
  // views. Only now attach a cursor-only raw stream observer.
  const { Database } = await import('bun:sqlite'), checkpoint = new Database(join(home, 'store.db'), { readonly: true, create: false });
  let seq: number;
  try { seq = Number((checkpoint.query("SELECT v FROM meta WHERE k='applied_seq'").get() as Row).v); } finally { checkpoint.close(); }
  const observer = new Client('observer', undefined, { seq, epoch: own.greeting.epoch }); await observer.open();
  // Serialize requests; a pending RPC queue never provides an artificial load.
  for (const id of ids) { const r = await observer.send({ id: ++observer.sequence, m: 'turn.start', p: { thread: id, text: `real-active-thread=${id}`, accountProfiles: 1, accountBinding: '' } }); ensure(r.ok, 'turn-start'); }
  await until(() => ids.every((_, i) => {
    if (!existsSync(join(root, `producer-${i}.json`))) return false;
    registerProducer(smallJson(join(root, `producer-${i}.json`))); return observer.lastDelivered[i] >= 10;
  }), 30000);
  loadedStart = mono() - bootAt; report.loadedStartMs = loadedStart;
  for (let index = 0; index < o.seconds / 60; index++) await runWindow(index, tickHz);
  loadedEnd = mono() - bootAt; sample();
  report.loadedEndMs = loadedEnd; report.actualLoadedMs = loadedEnd - loadedStart;
  report.resources = P.resources(samples, loadedStart, loadedEnd, tickHz);
  report.checks.resources = report.resources.ok;
  report.checks.phases = report.phases.length === o.seconds / 60 && report.phases.every((p: Row) => p.acceptance.ok);
  report.checks.continuousActive = P.loadedWindows(report.phases, loadedStart, loadedEnd, o.seconds * 1000);
  guard();
} catch (e) {
  // Never serialize exception messages/stacks from SQLite, RPCs or copied data.
  const code = e instanceof Error ? e.message : '';
  report.failureCode = /^[a-z][a-z0-9-]{0,63}$/.test(code) ? code : 'fixture-or-contract';
  report.ok = false;
} finally {
  cleaning = true; current = undefined;
  try { control({ mode: 'stop' }); } catch {}
  for (const c of clients) {
    for (const p of c.pending.values()) { clearTimeout(p.timer); p.reject(Error('owned-stop')); }
    c.ws.close();
  }
  if (hub && hub.exitCode === null) {
    hub.kill('SIGTERM'); const timer = setTimeout(() => { if (hub!.exitCode === null) hub!.kill('SIGKILL'); }, 3000);
    try { await hub.exited; } finally { clearTimeout(timer); }
  }
  // Only known synthetic producers with unchanged birth/argv are eligible.
  for (const [pid, born] of producerIdentities) try {
    const s = readFileSync(`/proc/${pid}/stat`, 'utf8'), f = s.slice(s.lastIndexOf(') ') + 2).trim().split(/\s+/);
    if (Number(f[19]) === born && readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0').includes(join(agents, 'producer.cjs'))) process.kill(pid, 'SIGTERM');
  } catch {}
  let drainTimer: ReturnType<typeof setTimeout> | undefined;
  const drained = await Promise.race([Promise.allSettled(drains).then(() => true),
    new Promise<boolean>(r => { drainTimer = setTimeout(() => r(false), 3000); })]);
  clearTimeout(drainTimer); report.checks.logStreamsClosed = drained;
  if (!drained) for (const r of logReaders) void r.cancel().catch(() => {});
  try {
    if (before) {
      const after = await storeCounts(); report.after = countReport(after);
      report.checks.catalogPreserved = after.originalThreads === before.originalThreads && after.originalCatalogSha256 === before.originalCatalogSha256;
      report.checks.historyPreserved = after.originalEntryRows === before.originalEntryRows && after.originalEntrySha256 === before.originalEntrySha256;
      report.checks.syntheticThreadCount = after.syntheticThreads === 20;
      report.checks.syntheticProjectCount = after.counts.project === before.counts.project + 1;
      report.checks.originalLogPrefix = await hashFile(join(home, 'events.jsonl'), originalBytes) === report.fixture.logSha256;
    }
    report.checks.archiveUnchanged = await hashFile(fixture) === o['--fixture-sha256'];
    report.checks.serviceUnchanged = await hashFile(bin) === o['--sha256'];
    report.checks.sourcesUnchanged = Object.entries(sourceHashes).length > 0 && Object.entries(sourceHashes).every(([p, h]) => sha(readFileSync(resolve(import.meta.dir, '../..', p))) === h);
    report.checks.fenceInputsUnchanged = fenceInputs.length === 3 && fenceInputs.every(p => sha(readFileSync(p.path)) === p.sha256);
  } catch { report.checks.finalIdentityAndCounts = false; }
  report.childLogs = { bytes: logBytes, listening, stackError, retainedPrivateText: false };
  report.checks.childLogs = !stackError && logBytes <= 64 * 1024 * 1024;
  report.checks.noFault = !fault && !report.failureCode;
  report.ok = Object.keys(report.checks).length >= 10 && Object.values(report.checks).every(v => v === true);
  report.finishedAt = new Date().toISOString(); closeSync(samplesFd);
  writeFileSync(out, JSON.stringify(report, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  rmSync(root, { recursive: true, force: true });
  console.log(JSON.stringify({ scope: report.scope, ok: report.ok, failureCode: report.failureCode,
    usableMs: report.usableMs, actualLoadedMs: report.actualLoadedMs, loadedWindows: report.phases.length }));
  process.exitCode = report.ok ? 0 : 1;
}
