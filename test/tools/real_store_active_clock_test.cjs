// Source preparation only. Proposed tests are Boat-only under reviewed caps.
const assert = require('node:assert/strict'), Clock = require('./real_store_active_clock.cjs');
const P = require('./real_store_active_policy.cjs'), { resolve } = require('node:path');
const sleep = ms => new Promise(r => setTimeout(r, ms));
module.exports.run = async function run() {
  let cases = 0;
  const test = (label, fn) => { fn(); cases++; };
  const domain = 'a'.repeat(64), other = 'b'.repeat(64);
  test('exact nanosecond timespec', () => assert.equal(Clock.timespecNs(12n, 345n), 12000000345n));
  test('negative seconds refused', () => assert.throws(() => Clock.timespecNs(-1n, 0n)));
  test('invalid nanos refused', () => { for (const n of [-1n, 1000000000n]) assert.throws(() => Clock.timespecNs(1n, n)); });
  test('wrong native ABI values refused', () => assert.throws(() => Clock.timespecNs(1, 0n)));
  test('conversion range bound enforced', () => assert.throws(() => Clock.timespecNs(1n << 40n, 0n)));
  test('canonical projection preserves fractional milliseconds', () => {
    const c = Clock.checkedReader(() => 123456789012345n, { domain }); assert.equal(c.nowMs(), 123456789.012345);
  });
  test('equal monotonic samples legal', () => { const c = Clock.checkedReader(() => 100n, { domain }); assert.equal(c.nowMs(), c.nowMs()); });
  test('regression refuses and failure stays sticky', () => {
    const n = [100n, 99n, 101n], c = Clock.checkedReader(() => n.shift(), { domain }); c.nowMs();
    assert.throws(() => c.nowMs(), /clock-regressed/); assert.throws(() => c.nowMs(), /clock-failed/);
  });
  test('missing native sample refuses', () => assert.throws(() => Clock.checkedReader(() => undefined, { domain }).nowMs()));
  test('native failure refuses and stays sticky', () => {
    let error = true; const c = Clock.checkedReader(() => { if (error) throw Error('native'); return 100n; }, { domain });
    assert.throws(() => c.nowMs()); error = false; assert.throws(() => c.nowMs(), /clock-failed/);
  });
  test('matching clock domain accepted', () => Clock.assertDomain({ contract: { domain } }, domain));
  test('missing foreign malformed clock domains refused', () => {
    for (const id of [undefined, '', other, 'a']) assert.throws(() => Clock.assertDomain({ contract: { domain } }, id));
  });
  // Simulated launch origins are deliberately irrelevant to the same absolute
  // kernel samples. No guessed offset or before-window filtering is applied.
  test('different launch origins preserve cohort and delivery lag', () => {
    const start = 700000.25, end = start + 1000;
    for (const launchOrigin of [0, 1916.633334, 600000]) {
      const activity = new P.Activity(start, end), cohort = new P.EmissionCohort(2, 'pure', start, end);
      const legacy = new P.EmissionCohort(2, 'pure', start, end);
      for (let seq = 1; seq <= 16; seq++) {
        const emitted = start + seq * 25;
        const c = Clock.checkedReader(() => BigInt(Math.round(emitted * 1000000)), { domain });
        const at = c.nowMs(); activity.add(at);
        assert(cohort.add({ seq, epoch: 2, mode: 'pure', emitted: Math.floor(at) }, at + 10));
        // Counterexample only: the former process-relative timestamp loses
        // membership when launch origins differ. Production never corrects
        // clocks by subtracting/estimating any such offset.
        assert.equal(legacy.add({ seq, epoch: 2, mode: 'pure', emitted: Math.floor(at - launchOrigin) }, at + 10), launchOrigin === 0);
      }
      const r = cohort.finish({ count: activity.count, firstSeq: 1, lastSeq: 16 });
      assert.equal(activity.count, 16); assert.equal(r.missing, 0); assert.equal(r.latency.p99Ms, 11); assert(r.ok);
    }
  });
  test('phase switch retains previous cohort and rejects wrong epoch', () => {
    const a = new P.EmissionCohort(1, 'mixed', 1000, 2000), b = new P.EmissionCohort(2, 'pure', 2000, 3000);
    assert(a.add({ seq: 1, epoch: 1, mode: 'mixed', emitted: 1999 }, 2010));
    assert(!b.add({ seq: 1, epoch: 1, mode: 'mixed', emitted: 1999 }, 2010));
    assert(b.add({ seq: 2, epoch: 2, mode: 'pure', emitted: 2000 }, 2010));
    assert.equal(a.finish({ count: 1, firstSeq: 1, lastSeq: 1 }).latency.p99Ms, 11);
    assert.equal(b.finish({ count: 1, firstSeq: 2, lastSeq: 2 }).latency.p99Ms, 10);
  });
  const clock = Clock.hostClock(), summaries = [], parentProcessStart = Number(process.hrtime.bigint()) / 1e6;
  for (const delayMs of [0, 300]) {
    // Publish BEFORE a deliberately delayed launch; child samples must remain
    // in that exact window without changing the control timestamps.
    const start = clock.nowMs(), end = start + 10000; await sleep(delayMs);
    const lower = clock.nowMs(); let child, finished = false;
    try {
      child = Bun.spawn([process.execPath, resolve(__dirname, 'real_store_active_clock_child.cjs')], {
        env: { PATH: '/usr/bin:/bin' }, stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
      child.stdin.write(JSON.stringify({ domain: clock.contract.domain, epoch: 1, mode: 'mixed', start, end }) + '\n');
      const cohort = new P.EmissionCohort(1, 'mixed', start, end), activity = new P.Activity(start, end);
      let output = '', count = 0, receipt, previous = 0, firstArrival = null;
      const consume = (async () => {
        for await (const bytes of child.stdout) {
          output += new TextDecoder().decode(bytes); assert(output.length <= 16384);
          for (;;) { const at = output.indexOf('\n'); if (at < 0) break;
            const row = JSON.parse(output.slice(0, at)); output = output.slice(at + 1);
            const arrival = clock.nowMs(); Clock.assertDomain(clock, row.domain);
            if (row.type === 'delta') {
              assert.equal(row.seq, ++count); assert.equal(row.bytes, 96); assert(row.emitted >= Math.floor(lower));
              assert(row.emitted >= previous && row.emitted < end && arrival >= row.emitted); previous = row.emitted;
              activity.add(row.sampledAt); assert(cohort.add(row, arrival)); firstArrival ??= arrival;
            } else { assert.equal(row.type, 'receipt'); assert(!receipt); receipt = row; }
          }
        }
        assert.equal(output, '');
      })();
      const errorOutput = (async () => { let total = ''; for await (const b of child.stderr) { total += new TextDecoder().decode(b); assert(total.length <= 4096); } return total; })();
      // Observe early stream failures immediately; awaiting below still fails
      // the control after the bounded child has terminated.
      consume.catch(() => {}); errorOutput.catch(() => {});
      const timer = setTimeout(() => { if (child.exitCode === null) child.kill('SIGKILL'); }, 5000);
      try { assert.equal(await child.exited, 0); } finally { clearTimeout(timer); }
      finished = true; await consume; assert.equal(await errorOutput, '');
      test('delayed child domain count/bytes/sequence match', () => {
        assert(receipt); assert.equal(count, 16); assert.equal(receipt.count, count); assert.equal(activity.count, count);
        assert.equal(receipt.buckets.reduce((a, b) => a + b, 0), count); assert.equal(receipt.firstSeq, 1); assert.equal(receipt.lastSeq, 16);
      });
      test('delayed child times are bracketed without offsets', () => {
        assert(receipt.first >= lower && receipt.last >= receipt.first && receipt.at >= receipt.last && receipt.at <= clock.nowMs());
        if (delayMs) assert(receipt.first >= start + delayMs);
      });
      test('delayed child has no dropped emissions or origin-skewed lag', () => {
        const r = cohort.finish(receipt); assert.equal(r.missing, 0); assert.equal(r.count, 16); assert.equal(r.badPhase, 0);
        assert(r.ok && r.latency.p99Ms <= 2000 && r.firstDeliveryGapMs >= delayMs);
        summaries.push({ delayMs, count, missing: r.missing, p99Ms: r.latency.p99Ms,
          firstSincePublicationMs: receipt.first - start, receiveLagMs: firstArrival - receipt.first,
          controllerProcessRelativeMs: Number(process.hrtime.bigint()) / 1e6,
          childProcessRelativeMs: receipt.processRelativeMs });
      });
    } finally { if (child && !finished && child.exitCode === null) { child.kill('SIGKILL'); await child.exited; } }
  }
  assert.equal(cases, 20);
  console.log(JSON.stringify({ scope: 'combined-real-store-host-clock', ok: true, cases, deterministicCases: 14,
    syntheticClockChildren: 2, clock: clock.contract, parentProcessStart, summaries, hubStarted: false, realFixtureOpened: false }));
};
if (require.main === module) module.exports.run().catch(() => { console.error('FAIL host clock controls'); process.exitCode = 1; });
