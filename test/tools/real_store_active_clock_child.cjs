// Narrow synthetic clock fixture; no hub, store, account or provider.
const assert = require('node:assert/strict'), Clock = require('./real_store_active_clock.cjs');
const P = require('./real_store_active_policy.cjs'), clock = Clock.hostClock();
const readline = require('node:readline'); let handled = false;
readline.createInterface({ input: process.stdin }).on('line', async line => {
  try {
    assert(!handled && line.length <= 4096); handled = true;
    const c = JSON.parse(line); Clock.assertDomain(clock, c.domain);
    assert(Number.isFinite(c.start) && Number.isFinite(c.end) && c.end > c.start && c.end - c.start === 10000);
    const activity = new P.Activity(c.start, c.end); let firstSeq = 0, lastSeq = 0;
    for (let i = 1; i <= 16; i++) {
      const at = clock.nowMs(); activity.add(at); firstSeq ||= i; lastSeq = i;
      console.log(JSON.stringify({ type: 'delta', seq: i, epoch: c.epoch, mode: c.mode,
        emitted: Math.floor(at), sampledAt: at, bytes: 96, domain: clock.contract.domain }));
      await new Promise(r => setTimeout(r, 10));
    }
    console.log(JSON.stringify({ type: 'receipt', domain: clock.contract.domain,
      at: clock.nowMs(), count: activity.count, first: activity.first, last: activity.last,
      buckets: [...activity.buckets], firstSeq, lastSeq, processRelativeMs: Number(process.hrtime.bigint()) / 1e6 }));
    process.exit(0);
  } catch { console.error('clock-child-contract'); process.exit(1); }
});
