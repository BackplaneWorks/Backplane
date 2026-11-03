const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../../src/server/effects/diagnostics.js'), 'utf8');
function fixture() {
  const context = vm.createContext({ require, Buffer, process: { env: {} }, performance: { now: () => 0 }, Date,
    Diagnostics: new Proxy({}, { get: (_, name) => name }), CID: x => x, io_eff() {} });
  vm.runInContext(source, context);
  context.state = { ready: true, enabled: true, error: '', pending: new Map() };
  return { context, state: context.state, sample(minute, rss, heap, external = 0) {
    context.now = minute * 60000; context.usage = { rss: rss * 1024, heapUsed: heap * 1024, external: external * 1024 };
    vm.runInContext('diagnostics_memory(state, now, usage)', context);
  } };
}
test('stable retained floor with transient GC peaks stays quiet for hours and metadata stays bounded', () => {
  const f = fixture();
  for (let i = 0; i < 360; i++) f.sample(i, 600000 + (i % 5 ? 400000 : 0), 300000 + (i % 5 ? 250000 : 0));
  assert.equal(f.state.pending.has('error:heap_growth'), false);
  assert.ok(f.state.memory.points.length <= 61);
});
test('persistent JS or native-buffer growth produces a paced closed diagnostics event', () => {
  for (const native of [false, true]) {
    const f = fixture();
    for (let i = 0; i < 45; i++) f.sample(i, 500000 + i * 15000, native ? 200000 : 200000 + i * 10000, native ? i * 10000 : 0);
    const r = f.state.pending.get('error:heap_growth');
    assert.equal(r.code, 'heap_growth'); assert.equal(r.count, 1);
    assert.deepEqual(Object.keys(r).sort(), ['at', 'code', 'count', 'event']);
  }
});
test('disabled diagnostics and invalid samples retain no heap-monitor state', () => {
  const f = fixture(); f.state.enabled = false;
  for (let i = 0; i < 90; i++) f.sample(i, i * 100000, i * 100000);
  assert.equal(f.state.memory, undefined); f.state.enabled = true;
  f.sample(100, -1, 0); assert.equal(f.state.memory, undefined);
});
