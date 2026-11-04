// Pure acceptance policy. No IO, clocks, processes, fixture text or paths.
const LIMITS = Object.freeze({ threads: 20, hz: 40, bytes: 96, minHz: 32,
  maxGapMs: 2000, cpuPercent: 80, rssBytes: 1000000000, sampleGapMs: 15000,
  loadedMs: 1800000, maxLoadedMs: 3600000, windowMs: 60000,
  growthBytes: 65536 * 1024, slopeBytesPerHour: 65536 * 1024 });
const requireThat = (v, code) => { if (!v) throw Error(code); };
const finite = n => typeof n === 'number' && Number.isFinite(n);
const digest = s => typeof s === 'string' && /^[a-f0-9]{64}$/.test(s);
const gitId = s => typeof s === 'string' && /^[a-f0-9]{40}$/.test(s);
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
    (name !== 'RECEIPT.json' || size <= 1024 * 1024), 'archive-member');
  seen.add(name);
  return name !== 'providers/accounts-v1/profiles.json'; // Never install copied account profiles.
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
    requireThat(finite(start) && finite(end) && end > start && end - start <= 120000, 'activity-window');
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
      t.produced.ok && t.delivered.ok && t.latency.ok && t.bytes === t.produced.count * LIMITS.bytes &&
      t.latency.n === t.delivered.count &&
      t.badPhase === 0 && t.receiptFresh === true && t.deliveryWithinProduced === true &&
      t.delivered.count >= t.produced.count * 0.99),
    foreignText: w.foreignText === 0, foreignNotices: w.foreignNotices === 0,
    viewReads: ['own', 'quiet', 'sidebar'].every(k => w.clients?.[k]?.replies >= (w.end - w.start) / 2000 &&
      w.clients[k].requestLatency.ok && w.clients[k].requestLatency.n === w.clients[k].replies &&
      w.clients[k].requests === w.clients[k].replies),
    mixedNotices: w.mode !== 'mixed' || w.ownNotices >= (w.end - w.start) / 1000 && w.sidebarNotices >= (w.end - w.start) / 2000,
    visible: w.mode === 'mixed' ? w.visible?.ok === true : w.pendingAtStart === true && w.pendingReplies > 0 && w.invalidPending === 0 && w.partialVisible === 0,
    sidebarPacing: w.sidebarPeak <= 2, ownPacing: w.ownPeak <= 20 };
  return { checks, ok: Object.values(checks).every(Boolean) };
}
function loadedWindows(windows, start, end, expectedMs) {
  return finite(start) && finite(end) && end - start >= LIMITS.loadedMs && expectedMs >= LIMITS.loadedMs &&
    windows.length === expectedMs / LIMITS.windowMs && windows.length > 0 &&
    windows[0].start >= start && windows[0].start - start <= 250 && end >= windows.at(-1).end && end - windows.at(-1).end <= 2000 &&
    windows.reduce((n, w) => n + w.end - w.start, 0) >= expectedMs - 250 * windows.length &&
    windows.every((w, i) => w.index === i && w.mode === (i % 2 ? 'pure' : 'mixed') &&
      w.acceptance?.ok === true && w.end > w.start &&
      (!i || w.start >= windows[i - 1].end && w.start - windows[i - 1].end <= 2000));
}
module.exports = { LIMITS, Latency, Activity, activitySummary, processSample, cpu, resources, pendingRow,
  usablePicture, artifactReceipt, options, archiveMember, fixtureReceipt, ownedEndpoint, phaseVerdict, loadedWindows, FENCE, fencePrerequisite };
