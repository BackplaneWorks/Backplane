// Synchronous pure checks: requiring this file actually executes every check.
// No node:test deferred scheduling, no process/clock/network/fixture IO.
const assert = require('node:assert/strict'), P = require('./real_store_active_policy.cjs');
let cases = 0;
function test(name, fn) { try { fn(); cases++; } catch (e) { throw Error('policy control failed: ' + name, { cause: e }); } }
const hash = 'a'.repeat(64), git = 'b'.repeat(40);
const latency = (n = 100, tails = 0, ms = 10) => { const h = new P.Latency(); for (let i = 0; i < n; i++) h.add(i < tails ? 2001 : ms); return h.summary(); };
test('nearest rank one sample', () => { assert.equal(latency(1, 0, 1700).p99Ms, 1700); assert.equal(latency(1, 1).ok, false); });
test('nearest rank small n does not drop its last item', () => { assert.equal(latency(2, 1).ok, false); assert.equal(latency(99, 1).ok, false); });
test('exact one percent tail', () => { assert.equal(latency(100, 1).ok, true); assert.equal(latency(100, 2).ok, false); assert.equal(latency(101, 2).ok, false); });
test('fractional 2000ms boundary is conservative', () => { assert(latency(1, 0, 2000).ok); assert(!latency(1, 0, 2000.001).ok); });
test('empty and invalid latency are unavailable', () => { assert(!new P.Latency().summary().ok); for (const v of [NaN, Infinity, -1, null]) assert.throws(() => new P.Latency().add(v)); });
function activity(start = 0, end = 60000, step = 25) { const a = new P.Activity(start, end); for (let t = start; t < end; t += step) a.add(t); return a; }
test('actual 40Hz counts and all five-second buckets', () => { const s = activity().summary(); assert(s.ok); assert.equal(s.count, 2400); assert.equal(s.hz, 40); });
test('underload cannot masquerade as low CPU', () => assert(!activity(0, 60000, 50).summary().ok));
test('initial and terminal stale streams', () => { const a = new P.Activity(0, 10000); for (let t = 3000; t < 10000; t += 10) a.add(t); assert(!a.summary().ok); const b = new P.Activity(0, 10000); for (let t = 0; t < 6000; t += 10) b.add(t); assert(!b.summary().ok); });
test('a count burst cannot hide the remaining silence', () => { const a = new P.Activity(0, 10000); for (let i = 0; i < 400; i++) a.add(10); assert(!a.summary().ok); });
test('internal gap and replayed ordering', () => { const a = new P.Activity(0, 10000); a.add(10); a.add(3010); assert(!a.summary().ok); assert.throws(() => a.add(3000)); assert.throws(() => a.add(10000)); });
test('receipt counts must equal bounded buckets', () => { const a = activity(), r = { count: a.count + 1, first: a.first, last: a.last, maxInteriorGapMs: 0, buckets: [...a.buckets] }; assert(!P.activitySummary(r, 0, 60000).ok); });
const cpuA = { birth: 9, ticks: 0, elapsedMs: 0 }, cpuB = { birth: 9, ticks: 4800, elapsedMs: 60000 };
test('actual one-core CPU inclusive 80 percent', () => { assert.equal(P.cpu(cpuA, cpuB, 100).percent, 80); assert(!P.cpu(cpuA, { ...cpuB, ticks: 4801 }, 100).ok); });
test('missing zero/regressed CPU and PID replacement fail', () => {
  for (const b of [{ ...cpuB, ticks: undefined }, { ...cpuB, ticks: 0 }, { ...cpuB, ticks: -1 }, { ...cpuB, birth: 10 }, { ...cpuB, elapsedMs: 0 }]) assert.throws(() => P.cpu(cpuA, b, 100));
  assert.throws(() => P.cpu({ ...cpuA, birth: undefined }, { ...cpuB, birth: undefined }, 100)); assert.throws(() => P.cpu(cpuA, cpuB, 0));
});
function samples(rss = () => 700000000) { return Array.from({ length: 361 }, (_, i) => ({ elapsedMs: i * 5000, birth: 9, ticks: i * 250,
  VmRSSBytes: rss(i), VmHWMBytes: Math.max(800000000, rss(i)), VmSwapBytes: 0 })); }
test('full unforced 30m resource series passes', () => assert(P.resources(samples(), 0, 1800000, 100).ok));
test('decimal GB is stricter than GiB and equality fails', () => { assert(!P.resources(samples(() => 1000000000), 0, 1800000, 100).ok); assert(P.resources(samples(() => 999999999), 0, 1800000, 100).ok); });
test('sustained memory slope is not hidden by warmup', () => assert(!P.resources(samples(i => 600000000 + i * 150000), 0, 1800000, 100).ok));
test('missing memory samples and zero memory fail', () => { const s = samples(); delete s[40].VmRSSBytes; assert(!P.resources(s, 0, 1800000, 100).ok); const z = samples(); z[50].VmRSSBytes = 0; assert(!P.resources(z, 0, 1800000, 100).ok); });
test('swap and high water are mandatory', () => { const s = samples(); s[20].VmSwapBytes = 1024; assert(!P.resources(s, 0, 1800000, 100).ok); const t = samples(); delete t[90].VmHWMBytes; assert(!P.resources(t, 0, 1800000, 100).ok); });
test('sample holes and dead-child truncation fail', () => { const s = samples(); s.splice(50, 4); assert(!P.resources(s, 0, 1800000, 100).ok); assert(!P.resources(samples().slice(0, -3), 0, 1800000, 100).ok); assert(!P.resources([], 0, 1800000, 100).ok); });
test('long idle CPU samples cannot be fake zero', () => { const s = samples(); for (const v of s) v.ticks = 0; assert(!P.resources(s, 0, 1800000, 100).ok); });
test('resource PID birth must stay identical', () => { const s = samples(); s[200].birth++; assert(!P.resources(s, 0, 1800000, 100).ok); });
const stat = (pid = 123) => { const f = Array(30).fill('0'); f[11] = '10'; f[12] = '20'; f[19] = '99'; return `${pid} (owned name) ${f.join(' ')}`; };
const status = 'VmRSS: 100 kB\nVmHWM: 200 kB\nVmSwap: 0 kB\n';
test('actual process fields and KiB conversion', () => { const s = P.processSample(stat(), status, 123, true, true, 5); assert.equal(s.ticks, 30); assert.equal(s.VmRSSBytes, 102400); });
test('wrong process missing memory and dead child rejected', () => { for (const args of [[stat(124), status, 123, true, true, 0], [stat(), status, 123, false, true, 0], [stat(), status, 123, true, false, 0], [stat(), '', 123, true, true, 0]]) assert.throws(() => P.processSample(...args)); });
const pending = { type: 'element', tag: 'bp-memo', key: '@live:own', attrs: [{ name: 'fn', value: 'live' }], kids: [{ type: 'text', text: 'Working' }] };
test('intentional static pending row is valid', () => assert(P.pendingRow(pending, 'own').valid));
test('completed foreign partial and duplicate pending rows fail', () => { assert(!P.pendingRow(pending, 'foreign').valid); assert(!P.pendingRow({ ...pending, key: 'completed' }, 'own').valid); assert(!P.pendingRow({ ...pending, kids: [{ type: 'text', text: 'partial reply' }] }, 'own').valid); assert(!P.pendingRow({ type: 'element', kids: [pending, pending] }, 'own').valid); });
test('usable thread requires actual target and timeline', () => { const n = { type: 'element', attrs: [{ name: 'data-thread', value: 'own' }], kids: [{ type: 'element', attrs: [{ name: 'id', value: 'timeline' }] }] }; assert(P.usablePicture(n, { kind: 'thread', id: 'own' })); assert(!P.usablePicture(n, { kind: 'thread', id: 'wrong' })); assert(!P.usablePicture(pending, { kind: 'thread', id: 'own' })); });
function window(mode = 'mixed') { const s = activity().summary(); return { start: 0, end: 60000, mode, cpu: { ok: true },
  threads: Array.from({ length: 20 }, () => ({ produced: s, delivered: s, latency: latency(2400), bytes: 2400 * 96, badPhase: 0, receiptFresh: true, deliveryWithinProduced: true })),
  foreignText: 0, foreignNotices: 0, visible: { ok: true }, pendingAtStart: true, pendingReplies: 60, invalidPending: 0, partialVisible: 0,
  ownPeak: 20, sidebarPeak: 2, ownNotices: 120, sidebarNotices: 60,
  clients: Object.fromEntries(['own', 'quiet', 'sidebar'].map(k => [k, { requests: 60, replies: 60, requestLatency: latency(60) }])) }; }
test('mixed and intentional static pure phase contracts', () => { assert(P.phaseVerdict(window()).ok); assert(P.phaseVerdict(window('pure')).ok); });
test('stale phase or wrong bytes cannot pass', () => { const w = window(); w.threads[19].badPhase++; assert(!P.phaseVerdict(w).ok); const b = window(); b.threads[0].bytes--; assert(!P.phaseVerdict(b).ok); });
test('missing producers stale receipts and filtered delivery cannot pass', () => { const w = window(); w.threads.pop(); assert(!P.phaseVerdict(w).ok); const t = window(); t.threads[3].receiptFresh = false; assert(!P.phaseVerdict(t).ok); const d = window(); d.threads[1].delivered = { ...d.threads[1].delivered, count: 2300 }; assert(!P.phaseVerdict(d).ok); });
test('missing latency samples cannot substitute for counted traffic', () => { const w = window(); w.threads[0].latency.n--; assert(!P.phaseVerdict(w).ok); const r = window(); r.clients.own.requestLatency.n--; assert(!P.phaseVerdict(r).ok); });
test('foreign text notices and uncapped pacing fail', () => { for (const key of ['foreignText', 'foreignNotices', 'invalidPending', 'partialVisible']) { const w = window('pure'); w[key]++; assert(!P.phaseVerdict(w).ok); } const w = window(); w.sidebarPeak = 3; assert(!P.phaseVerdict(w).ok); });
test('static pending needs actual accepted replies and a valid phase-start picture', () => { const a = window('pure'); a.pendingReplies = 0; assert(!P.phaseVerdict(a).ok); const b = window('pure'); b.pendingAtStart = false; assert(!P.phaseVerdict(b).ok); const c = window(); c.clients.sidebar.replies = 0; assert(!P.phaseVerdict(c).ok); });
const windows = () => Array.from({ length: 30 }, (_, i) => ({ index: i, mode: i % 2 ? 'pure' : 'mixed', start: i * 60000, end: (i + 1) * 60000, acceptance: { ok: true } }));
test('actual loaded 30m and alternating phases', () => assert(P.loadedWindows(windows(), 0, 1800000, 1800000)));
test('idle padding short load missing window or no phase switch fail', () => { assert(!P.loadedWindows(windows(), 0, 1799999, 1800000)); const a = windows(); a[2].mode = 'pure'; assert(!P.loadedWindows(a, 0, 1800000, 1800000)); const b = windows(); b[5].start += 5000; assert(!P.loadedWindows(b, 0, 1800000, 1800000)); const c = windows(); c.pop(); assert(!P.loadedWindows(c, 0, 1800000, 1800000)); });
test('artifact receipt requires exact qualified SHA source and tree', () => { assert(P.artifactReceipt({ schema: 1, qualification: 'qualified', serviceSha256: hash, sourceHead: git, sourceTree: git }, hash)); for (const r of [{}, { schema: 1, qualification: 'prepared', serviceSha256: hash, sourceHead: git, sourceTree: git }]) assert.throws(() => P.artifactReceipt(r, hash)); });
test('fixture receipt pins existing real fixture sequence', () => { assert(P.fixtureReceipt({ private: true, meta: { applied_seq: '1448789' }, logSha256: hash })); assert.throws(() => P.fixtureReceipt({ private: true, meta: { applied_seq: '1' }, logSha256: hash })); });
test('archive traversal symlink hardlink duplicates and PAX fail before extraction', () => { for (const [name, type, size] of [['../events.jsonl', '0', 10], ['/events.jsonl', '0', 10], ['events.jsonl', '2', 10], ['store.db', '1', 10], ['events.jsonl', 'x', 10], ['RECEIPT.json', '0', 2000000]]) assert.throws(() => P.archiveMember(name, type, size, new Set())); const seen = new Set(); assert(P.archiveMember('events.jsonl', '0', 10, seen)); assert.throws(() => P.archiveMember('events.jsonl', '0', 10, seen)); assert.equal(P.archiveMember('providers/accounts-v1/profiles.json', '0', 10, new Set()), false); });
test('owned endpoints exclude live ports credentials and arbitrary hosts', () => { assert(P.ownedEndpoint('http://127.0.0.1:43210/hello', 43210)); for (const url of ['http://127.0.0.1:3787', 'http://localhost:43210', 'http://127.0.0.1.evil:43210', 'https://127.0.0.1:43210', 'http://u:p@127.0.0.1:43210', 'file:///tmp/test']) assert(!P.ownedEndpoint(url, 43210)); });
const args = ['--bin', '/tmp/artifact/service', '--artifact-receipt', '/tmp/artifact/receipt.json', '--sha256', hash,
  '--fixture', '/tmp/private-RC3-real-store.tar.gz', '--fixture-sha256', hash, '--fixture-receipt-sha256', hash,
  '--fence-sha256', P.FENCE.source, '--fence-prerequisite', '/tmp/FENCE-PREREQUISITE.json', '--out', '/tmp/result.json'];
test('inputs require immutable artifact and exact existing archive identity', () => assert.equal(P.options(args).seconds, 1800));
test('arbitrary homes live targets pairing short runs and live binaries rejected', () => {
  for (const extra of [['--home', '/tmp/home'], ['--target', 'http://127.0.0.1:3787'], ['--pairing', 'anything'], ['--seconds', '60'], ['--seconds', '1860'], ['--sha256', hash]]) assert.throws(() => P.options([...args, ...extra]));
  const live = [...args]; live[1] = '/home/test/.local/share/backplane/backplane-serve'; assert.throws(() => P.options(live));
});
const fenceReceipt = () => ({ qualified_job: P.FENCE.job, manifest_sha256: P.FENCE.manifest,
  source_files: { '/owner/real_store_fence.cjs': P.FENCE.source, '/owner/real_store_fence_test.cjs': P.FENCE.test },
  focused_tests: { passed: 2, failed: 0 }, preflight: { cliVersionExit: 0, beforeAnyPrivateFixtureRun: true,
    fence: { installed: true, externalFetchBlocked: true, externalConnectBlocked: true } },
  overall_job_exit: 1, overall_job_pass: false, memory_startup_acceptance_pass: false });
test('fence prerequisite preserves exact focused PASS and overall FAIL', () => { assert.equal(P.fencePrerequisite(fenceReceipt()).originalOverallPass, false); });
test('fence mismatch missing receipt or relabelled overall pass fail', () => {
  assert.throws(() => P.fencePrerequisite({})); const r = fenceReceipt(); r.overall_job_pass = true; assert.throws(() => P.fencePrerequisite(r));
  const m = fenceReceipt(); m.source_files['/owner/real_store_fence.cjs'] = hash; assert.throws(() => P.fencePrerequisite(m));
  const p = fenceReceipt(); p.preflight.beforeAnyPrivateFixtureRun = false; assert.throws(() => P.fencePrerequisite(p));
});
console.log(JSON.stringify({ scope: 'combined-real-store-pure-policy', cases, ok: true, hubStarted: false }));
