// Async logical retention/queue control; no process, picture, fixture or IO.
const assert = require('node:assert/strict'), P = require('./real_store_active_policy.cjs');
module.exports.run = async function run() {
  let calls = 0, inFlight = 0, maxInFlight = 0, fault = false, id = 0;
  const pump = new P.RefreshPump(async () => {
    inFlight++; maxInFlight = Math.max(maxInFlight, inFlight); await Promise.resolve(); inFlight--; calls++;
    return calls < 20000;
  }, () => ({ id: ++id }), () => { fault = true; });
  const first = pump.offer({ id: ++id });
  for (let i = 0; i < 20000; i++) pump.request();
  await first; assert(calls >= 1 && calls < 20000, 'first accepted picture does not await an infinite refresh stream');
  await pump.done;
  assert.equal(calls, 20000); assert.equal(fault, false); assert.equal(maxInFlight, 1);
  assert.equal(pump.maxInFlight, 1); assert.equal(pump.maxQueued, 1); assert.equal(pump.active, false); assert.equal(pump.again, false);
  assert.equal(pump.first, null);
  console.log(JSON.stringify({ scope: 'combined-real-store-iterative-refresh-control', ok: true,
    accepted: calls, maxInFlight, maxCoalescedFlags: pump.maxQueued, forcedGC: false, hubStarted: false }));
};
if (require.main === module) module.exports.run().catch(() => { console.error('FAIL iterative refresh retention control'); process.exitCode = 1; });
