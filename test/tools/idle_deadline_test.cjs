// The scheduler's idle boundary: ready deadlines yield once, future ones sleep,
// descriptor-only waits still poll, and an outstanding Bun operation can wake it.
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('src/server/effects/loop.js', 'utf8');
const start = source.indexOf('async function bp_idle(io) {');
const end = source.indexOf('\n// A slow synchronous', start);
assert(start >= 0 && end > start);
function scheduler() {
  const immediates = [], timers = [], polls = [], due = [], cleared = [];
  const run = vm.runInNewContext(`(${source.slice(start, end)})`, {
    performance: { now: () => 100 },
    setImmediate: f => { immediates.push(f); return f; },
    setTimeout: (f, ms) => { const t = { f, ms }; timers.push(t); return t; },
    clearTimeout: t => cleared.push(t),
    bp_poll: (io, ms) => polls.push(ms), bp_due: io => due.push(io),
  });
  return { run, immediates, timers, polls, due, cleared };
}
const idle = (waits, pend = 1) => ({ waits, pend, tick: 1, wake: null });
for (const at of [99, 100]) test(`deadline ${at}: Bun gets a turn before due work, with no timer floor`, async () => {
  const s = scheduler(), io = idle([{ at }]);
  const done = s.run(io);
  assert.equal(s.immediates.length, 1); assert.equal(s.timers.length, 0);
  assert.equal(s.due.length, 0);
  s.immediates.shift()(); await done;
  assert.equal(s.due[0], io); assert.equal(io.wake, null);
});
test('future deadlines retain their delay and are cleared after firing', async () => {
  const s = scheduler(), io = idle([{ at: 112.5 }]);
  const done = s.run(io);
  assert.equal(s.immediates.length, 0); assert.equal(s.timers[0].ms, 13);
  s.timers[0].f(); await done;
  assert.equal(s.cleared[0], s.timers[0]); assert.equal(io.wake, null);
});
test('a Bun operation can wake a future deadline early, clearing the timer', async () => {
  const s = scheduler(), io = idle([{ at: 300 }]);
  const done = s.run(io); io.wake(); await done;
  assert.equal(s.cleared[0], s.timers[0]); assert.equal(s.due.length, 1);
});
test('with no deadline, Bun wake stays timer-free', async () => {
  const s = scheduler(), io = idle([]);
  const done = s.run(io);
  assert.equal(s.immediates.length + s.timers.length, 0);
  io.wake(); await done; assert.equal(io.wake, null);
});
test('descriptor-only waits still use blocking poll rather than spinning', async () => {
  for (const [at, expected] of [[undefined, -1], [99, 0], [125.1, 26]]) {
    const s = scheduler(); await s.run(idle([{ fd: 8, at }], 0));
    assert.equal(s.polls[0], expected);
    assert.equal(s.immediates.length + s.timers.length + s.due.length, 0);
  }
});
test('mixed descriptor/Bun waits keep adaptive polling for future deadlines', async () => {
  const s = scheduler(), io = idle([{ fd: 8, at: 200 }]);
  const done = s.run(io);
  assert.equal(s.timers[0].ms, 1); assert.equal(io.tick, 2);
  s.timers[0].f(); await done; assert.equal(s.due.length, 1);
});
test('mixed descriptor/Bun waits with due work yield and then poll due work', async () => {
  const s = scheduler(), io = idle([{ fd: 8, at: 99 }]);
  const done = s.run(io);
  assert.equal(s.immediates.length, 1); assert.equal(s.timers.length, 0);
  s.immediates[0](); await done; assert.equal(s.due.length, 1);
});
