// Pure acceptance policy. No IO, clocks, processes, fixture text or paths.
const LIMITS = Object.freeze({ threads: 20, hz: 40, bytes: 96, minHz: 32,
  maxGapMs: 2000, cpuPercent: 80, rssBytes: 1000000000, sampleGapMs: 15000,
  loadedMs: 1800000, maxLoadedMs: 3600000, windowMs: 60000,
  growthBytes: 65536 * 1024, slopeBytesPerHour: 65536 * 1024 });
const requireThat = (v, code) => { if (!v) throw Error(code); };
const finite = n => typeof n === 'number' && Number.isFinite(n);
const digest = s => typeof s === 'string' && /^[a-f0-9]{64}$/.test(s);
const gitId = s => typeof s === 'string' && /^[a-f0-9]{40}$/.test(s);
function failureCode(e) {
  const known = new Set(['child-exited', 'deadline', 'loaded-window-failed', 'view-protocol', 'view-request', 'view-closed',
    'view-transport', 'view-tail-deadline', 'observer-cursor-epoch', 'hub-swap', 'hub-resource-stop', 'child-replaced',
    'child-log-bound', 'child-log-stream', 'phase-boundary-scheduling', 'stale-emission-epoch', 'producer-replaced',
    'producer-receipt', 'producer-identity', 'service-sha', 'artifact-receipt', 'fence-prerequisite', 'fence-source-sha',
    'fence-sha', 'preflight-exit', 'preflight-fence', 'fence-receipt-missing', 'fence-not-installed', 'fixture-counts',
    'fixture-receipt', 'fixture-receipt-sha', 'fixture-log-sha', 'archive-sha', 'archive-checksum', 'archive-padding',
    'archive-incomplete', 'archive-pax-override', 'archive-member', 'account-metadata', 'ownership-metadata-bound']);
  return known.has(e?.message) ? e.message : 'fixture-or-contract';
}
const FENCE = Object.freeze({ source: '74de3b99ba21abff2650de474fd6459d47f32778ef120d8940da8be3ed13f2d5',
  test: 'cf433ab8152d463816c8a086a49f26b109ffac7e79f394659b7c1b6f98cd0bfe',
  manifest: '794c59a5d820f588d7dd794cf20ba8bf6006740ec9783a93c050161257271bf7', job: 'j20261104003249-591142' });
function fencePrerequisite(r) {
  requireThat(r?.qualified_job === FENCE.job && r.manifest_sha256 === FENCE.manifest &&
    r.focused_tests?.passed === 2 && r.focused_tests?.failed === 0 && r.preflight?.cliVersionExit === 0 &&
    r.preflight.beforeAnyPrivateFixtureRun === true && r.preflight.fence?.installed === true &&
    r.preflight.fence.externalFetchBlocked === true && r.preflight.fence.externalConnectBlocked === true &&
    r.overall_job_exit === 1 && r.overall_job_pass === false && r.memory_startup_acceptance_pass === false, 'fence-prerequisite');
  const files = Object.entries(r.source_files ?? {}).map(([path, sha256]) => ({ path, name: path.split('/').at(-1), sha256 }));
  requireThat(files.length === 2 && files.every(f => f.path.startsWith('/') && !f.path.split('/').includes('..')) &&
    files.some(f => f.name === 'real_store_fence.cjs' && f.sha256 === FENCE.source) &&
    files.some(f => f.name === 'real_store_fence_test.cjs' && f.sha256 === FENCE.test), 'fence-prerequisite');
  return { files, qualifiedJob: FENCE.job, manifestSha256: FENCE.manifest, originalOverallPass: false };
}
function artifactReceipt(r, sha) {
  requireThat(r?.schema === 1 && r.qualification === 'qualified' && digest(sha) &&
    r.serviceSha256 === sha && gitId(r.sourceHead) && gitId(r.sourceTree), 'artifact-receipt');
  return { serviceSha256: sha, sourceHead: r.sourceHead, sourceTree: r.sourceTree };
}
function options(args) {
  const required = ['--bin', '--artifact-receipt', '--sha256', '--fixture', '--fixture-sha256',
    '--fixture-receipt-sha256', '--fence-sha256', '--fence-prerequisite', '--out'];
  const out = Object.create(null);
  for (let i = 0; i < args.length; i += 2) {
    const k = args[i], v = args[i + 1];
    requireThat([...required, '--seconds'].includes(k) && typeof v === 'string' && v.length > 0 &&
      !v.startsWith('--') && out[k] === undefined, 'options');
    out[k] = v;
  }
  requireThat(required.every(k => out[k]), 'options');
  for (const k of ['--sha256', '--fixture-sha256', '--fixture-receipt-sha256', '--fence-sha256'])
    requireThat(digest(out[k]), 'digest');
  const seconds = Number(out['--seconds'] ?? 1800);
  requireThat(Number.isInteger(seconds) && seconds >= 1800 && seconds <= 3600 && seconds % 120 === 0, 'duration');
  // Only local immutable input copies; no live target, home, port or pairing options.
  requireThat(out['--fence-sha256'] === FENCE.source, 'fence-digest');
  for (const k of ['--bin', '--fixture', '--artifact-receipt', '--fence-prerequisite']) {
    const p = out[k];
    requireThat(p.startsWith('/') && !p.split('/').includes('..') && !p.includes('://') &&
      !/^\/(proc|sys|dev)(\/|$)/.test(p) && !/\/\.local\/(bin|share\/backplane)(\/|$)/.test(p) &&
      !/\/\.backplane-bend(\/|$)/.test(p), 'input-path');
  }
  requireThat(out['--fixture'].endsWith('/private-RC3-real-store.tar.gz'), 'fixture-name');
  const output = out['--out'];
  requireThat(output.startsWith('/') && !output.split('/').includes('..') &&
    (output.startsWith('/tmp/') || output.includes('/build/')) &&
    !/\/\.local\/(bin|share\/backplane)(\/|$)/.test(output) && !/\/\.backplane-bend(\/|$)/.test(output), 'output-path');
  return { ...out, seconds };
}
const MEMBERS = Object.freeze(['RECEIPT.json', 'events.jsonl', 'store.db', 'providers/accounts-v1/profiles.json']);
function archiveMember(name, type, size, seen) {
  requireThat(MEMBERS.includes(name) && !seen.has(name) && (type === '0' || type === '\0') &&
    Number.isSafeInteger(size) && size > 0 && size <= 4 * 1024 ** 3 &&
    (name !== 'RECEIPT.json' || size <= 1024 * 1024) &&
    (name !== 'providers/accounts-v1/profiles.json' || size <= 131072), 'archive-member');
  seen.add(name);
  return true;
}
function accountMetadata(r) {
  requireThat(r && Object.keys(r).every(k => ['version', 'profiles'].includes(k)) && r.version === 1 &&
    Array.isArray(r.profiles) && r.profiles.length <= 40, 'account-metadata');
  const ids = new Set();
  for (const p of r.profiles) {
    requireThat(p && Object.keys(p).every(k => ['id', 'provider', 'label', 'epoch', 'fingerprint', 'store'].includes(k)) &&
      typeof p.id === 'string' && /^[a-f0-9-]{36}$/.test(p.id) && !ids.has(p.id) &&
      ['claude', 'codex'].includes(p.provider) && Number.isSafeInteger(p.epoch) && p.epoch >= 1 &&
      typeof p.label === 'string' && p.label.length <= 1024 &&
      (p.fingerprint === undefined || p.fingerprint === '' || digest(p.fingerprint)) &&
      (p.store === undefined || p.store === '' || /^[a-f0-9-]{36}$/.test(p.store)), 'account-metadata');
    ids.add(p.id);
  }
  return { profiles: r.profiles.length, credentialFields: false };
}
function fixtureReceipt(r) {
  requireThat(r?.private === true && Number(r.meta?.applied_seq) === 1448789 && digest(r.logSha256), 'fixture-receipt');
  return { appliedSeq: 1448789, logSha256: r.logSha256 };
}
function ownedEndpoint(url, port) {
  try {
    const u = new URL(url);
    return Number.isInteger(port) && port > 0 && port !== 3787 && u.hostname === '127.0.0.1' &&
      u.port === String(port) && ['http:', 'ws:'].includes(u.protocol) && !u.username && !u.password;
  } catch { return false; }
}
function observerLog(f, cursor) {
  requireThat(f?.t === 'log' && f.origin === cursor?.epoch && Number.isSafeInteger(cursor.seq) && cursor.seq >= 1448789 &&
    Number(f.since) === cursor.seq && Array.isArray(f.items) && f.items.length <= 1024, 'observer-cursor-epoch');
  return cursor.seq + f.items.length;
}
// Integer-millisecond buckets through the gate, plus one overflow bucket.
// ceil(value) is conservative at the exact 2000ms boundary. Rank is ceil(.99*n).
class Latency {
  constructor() { this.bins = new Uint32Array(2002); this.n = 0; this.maxMs = 0; }
  add(ms) {
    requireThat(finite(ms) && ms >= 0 && this.n < 10000000, 'latency-sample');
    this.bins[Math.min(2001, Math.ceil(ms))]++; this.n++; this.maxMs = Math.max(this.maxMs, ms);
  }
  summary() {
    const rank = Math.ceil(this.n * 99 / 100); let count = 0, p99Ms = null;
    if (rank) for (let i = 0; i < this.bins.length; i++) if ((count += this.bins[i]) >= rank) { p99Ms = i; break; }
    return { n: this.n, rank, p99Ms, overflow: this.bins[2001], maxMs: this.maxMs,
      ok: this.n > 0 && p99Ms !== null && p99Ms <= LIMITS.maxGapMs };
  }
}
class Activity {
  constructor(start, end) {
    requireThat(finite(start) && finite(end) && end > start && end - start <= LIMITS.maxLoadedMs + 3000, 'activity-window');
    this.start = start; this.end = end; this.count = 0; this.first = null; this.last = null;
    this.maxInteriorGapMs = 0; this.buckets = new Uint32Array(Math.ceil((end - start) / 5000));
  }
  add(at) {
    requireThat(finite(at) && at >= this.start && at < this.end && (this.last === null || at >= this.last), 'activity-sample');
    if (this.last !== null) this.maxInteriorGapMs = Math.max(this.maxInteriorGapMs, at - this.last);
    if (this.first === null) this.first = at;
    this.last = at; this.count++; this.buckets[Math.floor((at - this.start) / 5000)]++;
  }
  summary() {
    return activitySummary({ count: this.count, first: this.first, last: this.last,
      maxInteriorGapMs: this.maxInteriorGapMs, buckets: [...this.buckets] }, this.start, this.end);
  }
}
// Emission cohort ownership survives the receive-time phase boundary. At most
// two cohorts are live; absent deliveries count as overflow latency, not zero.
class EmissionCohort {
  constructor(epoch, mode, start, end) {
    this.epoch = epoch; this.mode = mode; this.start = start; this.end = end; this.deadline = end + 2000;
    this.seen = new Set(); this.lag = new Latency(); this.firstSeq = Infinity; this.lastSeq = 0;
    this.firstAt = null; this.late = 0; this.bad = 0; this.finished = false;
  }
  add(m, at) {
    requireThat(!this.finished && finite(at), 'cohort-closed');
    if (m.epoch !== this.epoch || m.emitted + 1 < this.start || m.emitted >= this.end) return false;
    if (m.mode !== this.mode || !Number.isSafeInteger(m.seq) || m.seq < 1 || !finite(m.emitted) || at < m.emitted) { this.bad++; return false; }
    if (this.seen.has(m.seq)) return false;
    if (at > this.deadline) { this.late++; return false; }
    requireThat(this.seen.size < 8192, 'cohort-bound'); this.seen.add(m.seq); this.lag.add(at - m.emitted);
    this.firstSeq = Math.min(this.firstSeq, m.seq); this.lastSeq = Math.max(this.lastSeq, m.seq);
    this.firstAt ??= at; return true;
  }
  finish(p) {
    requireThat(!this.finished, 'cohort-closed'); this.finished = true;
    const valid = Number.isSafeInteger(p?.count) && p.count > 0 && p.count <= 8192 &&
      Number.isSafeInteger(p.firstSeq) && Number.isSafeInteger(p.lastSeq) && p.lastSeq - p.firstSeq + 1 === p.count && this.seen.size <= p.count;
    const missing = valid ? p.count - this.seen.size : null;
    const observedLatency = this.lag.summary();
    if (valid) for (let i = 0; i < missing; i++) this.lag.add(2001);
    const latency = this.lag.summary();
    return { count: this.seen.size, missing, late: this.late, badPhase: this.bad, latency, observedLatency,
      missingAsTimeoutOverflow: true,
      firstDeliveryGapMs: this.firstAt === null ? null : this.firstAt - this.start,
      ok: valid && this.bad === 0 && this.late === 0 && this.seen.size >= p.count * 0.99 &&
        this.firstSeq === p.firstSeq && this.lastSeq === p.lastSeq && this.firstAt - this.start <= 2000 && latency.ok };
  }
}
// Iterative single-flight pump. Repeated invalidations retain one boolean,
// not a recursive promise frame or a queue of pictures/calls.
class RefreshPump {
  constructor(one, refresh, fail) { this.one = one; this.refresh = refresh; this.fail = fail;
    this.active = false; this.again = false; this.accepted = 0; this.maxInFlight = 0; this.maxQueued = 0; }
  offer(call) {
    if (!call?.id) return Promise.resolve();
    if (this.active) { this.fail('pump-overlap'); return Promise.resolve(); }
    this.active = true; this.maxInFlight = 1;
    const first = new Promise(resolve => { this.first = resolve; });
    this.done = this.drain(call); return first;
  }
  request() {
    if (this.active) { this.again = true; this.maxQueued = 1; return; }
    void this.offer(this.refresh());
  }
  async drain(call) {
    try {
      while (call?.id) {
        const more = await this.one(call); this.accepted++; if (this.first) { this.first(); this.first = null; }
        const again = more || this.again; this.again = false;
        call = again ? this.refresh() : null;
      }
    } catch { this.fail('view-request'); }
    finally { if (this.first) this.first(); this.first = null; this.active = false; this.again = false; }
  }
}
function commitFamily(c, original, syntheticIds) {
  if (c?.$ === 'SettingSet') {
    if (syntheticIds.some(id => c.key === 'liveness.' + id)) return 'syntheticLiveness';
    if (typeof c.key === 'string' && (original.settings.has(c.key) ||
      [...original.threadPrefixes].some(p => c.key.startsWith(p) && original.threads.has(c.key.slice(p.length))))) return 'originalSetting';
    return 'unattributed';
  }
  if (original.kinds.has(c?.$)) {
    if (original.threads.has(c.thread) || original.threads.has(c.id)) return 'originalThread';
    if (original.bots.has(c.bot) || original.bots.has(c.id)) return 'originalBot';
    if (original.routines.has(c.routine) || original.routines.has(c.id)) return 'originalRoutine';
    if (original.projects.has(c.project) || original.projects.has(c.id)) return 'originalProject';
    if (original.tasks.has(c.task) || original.asks.has(c.id) || original.rooms.has(c.room) || original.convs.has(c.conv)) return 'originalLifecycle';
  }
  return 'unattributed';
}
function infoFamily(info, originalKeys) {
  if (!info || typeof info !== 'object' || Array.isArray(info)) return 'unattributed';
  const keys = Object.keys(info);
  return keys.length > 0 && keys.length <= 300000 && keys.every(k =>
    originalKeys.has(k) || ['accounts.live', 'boat.usage', 'agy.job'].includes(k)) ? 'originalInfo' : 'unattributed';
}
function activitySummary(r, start, end) {
  const seconds = (end - start) / 1000, expected = Math.ceil((end - start) / 5000);
  const valid = Number.isSafeInteger(r?.count) && r.count > 0 && finite(r.first) && finite(r.last) &&
    start <= r.first && r.first <= r.last && r.last < end && finite(r.maxInteriorGapMs) && r.maxInteriorGapMs >= 0 &&
    Array.isArray(r.buckets) && r.buckets.length === expected && r.buckets.every(n => Number.isSafeInteger(n) && n >= 0) &&
    r.buckets.reduce((a, b) => a + b, 0) === r.count;
  const maxGapMs = valid ? Math.max(r.first - start, end - r.last, r.maxInteriorGapMs) : null;
  const bucketsOk = valid && r.buckets.every((n, i) => {
    const span = Math.min(5000, end - start - i * 5000) / 1000;
    return n >= Math.floor(span * LIMITS.minHz) && n <= Math.ceil(span * LIMITS.hz * 1.2);
  });
  return { count: valid ? r.count : null, hz: valid ? r.count / seconds : null, maxGapMs,
    firstGapMs: valid ? r.first - start : null, lastGapMs: valid ? end - r.last : null,
    ok: valid && bucketsOk && maxGapMs <= LIMITS.maxGapMs && r.count >= seconds * LIMITS.minHz && r.count <= seconds * LIMITS.hz * 1.2 };
}
function processSample(stat, status, pid, executableOk, alive, elapsedMs) {
  requireThat(alive && executableOk && Number.isInteger(pid) && pid > 0 && finite(elapsedMs), 'child-identity');
  const close = stat.lastIndexOf(') '), f = stat.slice(close + 2).trim().split(/\s+/);
  const ticks = Number(f[11]) + Number(f[12]), birth = Number(f[19]);
  requireThat(close > 0 && Number(stat.slice(0, stat.indexOf(' '))) === pid &&
    Number.isSafeInteger(ticks) && ticks >= 0 && Number.isSafeInteger(birth) && birth > 0, 'cpu-sample');
  const memory = Object.create(null);
  for (const name of ['VmRSS', 'VmHWM', 'VmSwap']) {
    const m = new RegExp('^' + name + ':\\s+(\\d+) kB$', 'm').exec(status);
    requireThat(m, 'memory-sample'); memory[name + 'Bytes'] = Number(m[1]) * 1024;
  }
  requireThat(memory.VmRSSBytes > 0 && memory.VmHWMBytes >= memory.VmRSSBytes && memory.VmSwapBytes >= 0, 'memory-sample');
  return { elapsedMs, ticks, birth, ...memory };
}
function cpu(before, after, hz) {
  requireThat(before && after && finite(hz) && hz > 0 && before.birth === after.birth &&
    Number.isSafeInteger(before.birth) && before.birth > 0 &&
    finite(before.ticks) && finite(after.ticks) && after.ticks > before.ticks && after.elapsedMs > before.elapsedMs, 'cpu-window');
  const wallMs = after.elapsedMs - before.elapsedMs, cpuMs = (after.ticks - before.ticks) / hz * 1000;
  return { wallMs, cpuMs, percent: cpuMs / wallMs * 100, ok: cpuMs / wallMs * 100 <= LIMITS.cpuPercent };
}
function resources(samples, start, end, tickHz) {
  const s = samples.filter(s => s.elapsedMs >= start && s.elapsedMs <= end);
  const complete = s.length >= 3 && s[0].elapsedMs - start <= 5000 && end - s.at(-1).elapsedMs <= 5000 &&
    s.every((v, i) => finite(v.VmRSSBytes) && v.VmRSSBytes > 0 && finite(v.VmHWMBytes) && v.VmHWMBytes >= v.VmRSSBytes &&
      v.VmSwapBytes === 0 && Number.isSafeInteger(v.ticks) && Number.isSafeInteger(v.birth) && v.birth > 0 &&
      (!i || (v.elapsedMs > s[i - 1].elapsedMs && v.elapsedMs - s[i - 1].elapsedMs <= LIMITS.sampleGapMs &&
        v.birth === s[0].birth && v.ticks >= s[i - 1].ticks)));
  const steady = s.filter(v => v.elapsedMs >= start + 300000);
  let slopeBytesPerHour = null, growthBytes = null;
  if (complete && steady.length >= 60 && steady.at(-1).elapsedMs - steady[0].elapsedMs >= 900000) {
    const x0 = steady[0].elapsedMs, xs = steady.map(v => (v.elapsedMs - x0) / 3600000);
    const mx = xs.reduce((a, b) => a + b, 0) / xs.length, my = steady.reduce((a, b) => a + b.VmRSSBytes, 0) / steady.length;
    slopeBytesPerHour = steady.reduce((a, v, i) => a + (xs[i] - mx) * (v.VmRSSBytes - my), 0) / xs.reduce((a, x) => a + (x - mx) ** 2, 0);
    growthBytes = steady.at(-1).VmRSSBytes - steady[0].VmRSSBytes;
  }
  let measuredCpu = null;
  try { if (complete) measuredCpu = cpu(s[0], s.at(-1), tickHz); } catch {}
  const checks = { completeSamples: complete, loaded30Minutes: end - start >= LIMITS.loadedMs,
    decimalGB: complete && s.every(v => v.VmRSSBytes < LIMITS.rssBytes && v.VmHWMBytes < LIMITS.rssBytes),
    boundedGrowth: finite(slopeBytesPerHour) && slopeBytesPerHour <= LIMITS.slopeBytesPerHour && growthBytes <= LIMITS.growthBytes,
    actualCpu: measuredCpu?.ok === true };
  return { checks, ok: Object.values(checks).every(Boolean), samples: s.length, steadySamples: steady.length,
    slopeBytesPerHour, growthBytes, cpu: measuredCpu,
    peakRssBytes: s.length ? Math.max(...s.map(v => v.VmRSSBytes)) : null,
    peakHwmBytes: s.length ? Math.max(...s.map(v => v.VmHWMBytes)) : null };
}
function pendingRow(shown, thread) {
  let count = 0, valid = true, visited = 0;
  function text(n, depth = 0) { requireThat(depth <= 64, 'picture-bound'); return n?.type === 'text' ? n.text ?? '' : (n?.kids ?? []).map(c => text(c, depth + 1)).join(''); }
  function visit(n, depth = 0) {
    requireThat(++visited <= 100000 && depth <= 64, 'picture-bound');
    if (n?.type !== 'element') return;
    const a = Object.fromEntries((n.attrs ?? []).map(a => [a.name, a.value]));
    if ((n.key === '@live:' + thread || a['data-id'] === '@live:' + thread) &&
      (n.tag === 'bp-memo' && a.fn === 'live' || String(a.class ?? '').split(/\s+/).includes('pending') && a.role === 'status')) {
      count++; valid &&= /^Working(?:…|\.{3})?$/.test(text(n).trim());
    }
    for (const c of n.kids ?? []) visit(c, depth + 1);
  }
  visit(shown); return { count, valid: count === 1 && valid };
}
function usablePicture(shown, target) {
  let sidebar = false, thread = false, timeline = false, nodes = 0;
  const walk = (n, depth = 0) => {
    requireThat(++nodes <= 100000 && depth <= 64, 'picture-bound');
    if (n?.type !== 'element') return;
    const a = Object.fromEntries((n.attrs ?? []).map(a => [a.name, a.value]));
    sidebar ||= n.tag === 'aside'; thread ||= a['data-thread'] === target.id;
    timeline ||= a.id === 'timeline';
    for (const c of n.kids ?? []) walk(c, depth + 1);
  };
  walk(shown); return target.kind === 'thread' ? thread && timeline : target.kind === 'recent' && sidebar;
}
function phaseVerdict(w) {
  const checks = { duration: w.end - w.start >= LIMITS.windowMs - 250 && ['mixed', 'pure'].includes(w.mode),
    cpu: w.cpu?.ok === true, streams: w.threads?.length === LIMITS.threads && w.threads.every(t =>
      t.produced.ok && t.received.ok && t.delivered.ok && t.latency.ok && t.bytes === t.produced.count * LIMITS.bytes &&
      t.latency.n === t.produced.count &&
      t.badPhase === 0 && t.receiptFresh === true &&
      t.delivered.count >= t.produced.count * 0.99),
    foreignText: w.foreignText === 0, foreignNotices: w.foreignNotices === 0,
    viewReads: ['own', 'quiet', 'sidebar'].every(k => w.clients?.[k]?.replies >= (w.end - w.start) / 2000 &&
      w.clients[k].requestLatency.ok && w.clients[k].requestLatency.n === w.clients[k].replies &&
      w.clients[k].requests === w.clients[k].replies),
    mixedNotices: w.mode !== 'mixed' || w.ownNotices >= (w.end - w.start) / 1000 && w.sidebarNotices >= (w.end - w.start) / 2000,
    visible: w.mode === 'mixed' ? w.visible?.ok === true : w.pendingAtStart === true && w.pendingReplies > 0 && w.invalidPending === 0 && w.partialVisible === 0,
    sidebarPacing: w.sidebarPeak <= 2, quietPacing: w.quietPeak <= 2, ownPacing: w.ownPeak <= 20,
    metadataAttribution: w.mode !== 'pure' || w.unattributedCommits === 0 && w.unattributedInfo === 0 && w.cleanMetadataMs >= (w.end - w.start) * 0.25 };
  return { checks, ok: Object.values(checks).every(Boolean) };
}
function loadedWindows(windows, start, end, expectedMs) {
  return finite(start) && finite(end) && end - start >= LIMITS.loadedMs && expectedMs >= LIMITS.loadedMs &&
    windows.length === expectedMs / LIMITS.windowMs && windows.length > 0 &&
    windows[0].start >= start && windows[0].start - start <= 250 && end >= windows.at(-1).end && end - windows.at(-1).end <= 2250 &&
    windows.reduce((n, w) => n + w.end - w.start, 0) >= expectedMs - 250 * windows.length &&
    windows.every((w, i) => w.index === i && w.mode === (i % 2 ? 'pure' : 'mixed') &&
      w.acceptance?.ok === true && w.end > w.start &&
      (!i || w.start === windows[i - 1].end));
}
module.exports = { LIMITS, Latency, Activity, activitySummary, processSample, cpu, resources, pendingRow,
  usablePicture, artifactReceipt, options, archiveMember, fixtureReceipt, ownedEndpoint, phaseVerdict, loadedWindows, FENCE, fencePrerequisite,
  accountMetadata, EmissionCohort, RefreshPump, commitFamily, infoFamily, observerLog, failureCode };
