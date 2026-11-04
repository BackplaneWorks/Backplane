// Component adapter: real production effects and the exact runtime park
// helper, with a minimal continuation scheduler. No Bend program or hub.
const fs = require('node:fs'), vm = require('node:vm'), crypto = require('node:crypto');
const EFFECT = 'src/server/effects/storeio.js', LOOP = 'src/server/effects/loop.js';
const sha = s => crypto.createHash('sha256').update(s).digest('hex');
function load({ io = fs, asyncMode = true, mode = '', platform = process.platform, bridge = true } = {}) {
  const source = fs.readFileSync(EFFECT, 'utf8'), loop = fs.readFileSync(LOOP, 'utf8');
  const begin = loop.indexOf('function io_park_promise(p, k, then) {');
  const end = loop.indexOf('\nglobalThis.io_park_promise', begin);
  if (begin < 0 || end < 0) throw Error('park helper shape changed');
  const park = loop.slice(begin, end), effects = {}, stats = { parks: 0, resumes: 0, completions: 0, duplicates: 0 };
  const context = vm.createContext({ require: n => n === 'fs' ? io : require(n), Buffer,
    process: { platform, env: { BACKPLANE_JS_LOOP: mode } }, console,
    BEND_ASYNC: asyncMode, BEND_IO: { pend: 0 }, Promise, componentStats: stats,
    CID: x => x, Nil: 'Nil', Con: 'Con', WireScan: 'WireScan',
    Disk: new Proxy({}, { get: (_, key) => 'Disk.' + key }),
    io_eff: (key, fn) => { effects[key] = fn; },
    bp_resume: (k, then) => queueMicrotask(() => { stats.resumes++; k(then()); }) });
  if (bridge) {
    // Define the wrapper in the same lexical environment as the effect.
    // Host-side context property rebinding can leave Bun's original lexical
    // function binding intact, bypassing an instrumentation-only wrapper.
    vm.runInContext('const componentPark = (' + park + ');\n'
      + 'function io_park_promise(p, k, then) { componentStats.parks++; return componentPark(p, k, then); }', context);
  }
  vm.runInContext(source, context);
  return { context, effects, stats, identity: { effect_sha256: sha(source), loop_sha256: sha(loop),
    park_sha256: sha(park), before_function_sha256: sha(context.store_sync.toString()) } };
}
function invoke(host, before, file) {
  let settled = false;
  return new Promise((resolve, reject) => {
    const fn = before ? host.context.store_sync : host.effects['Disk.sync'];
    const finish = value => { if (settled) { host.stats.duplicates++; return; } settled = true; host.stats.completions++; resolve(value); };
    try { const value = fn(file, finish); if (value !== undefined) finish(value); }
    catch (e) { reject(e); }
  });
}
function assertDrained(host, parks, completions) {
  const assert = require('node:assert/strict');
  assert.equal(host.stats.duplicates, 0, 'duplicate completion');
  assert.equal(host.stats.parks, parks); assert.equal(host.stats.resumes, parks);
  assert.equal(host.stats.completions, completions); assert.equal(host.context.BEND_IO.pend, 0);
}
module.exports = { load, invoke, assertDrained, sha };
