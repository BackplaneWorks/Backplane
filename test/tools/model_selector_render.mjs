import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const providers = ['claude', 'codex', 'grok', 'muse', 'antigravity', 'opencode', 'pi', 'omp', 'unknown'];
function catalog(n) {
  const rows = Array.from({length:n}, (_, i) => 'model-' + i);
  return JSON.stringify(Object.assign({ about: Object.fromEntries(providers.map(p => [p, {
    labels: Object.fromEntries([['default','Default 日本語'], ...rows.map(m => [m, 'Label 日本語 & <name> ' + m])]),
    efforts: {default:['low','high'], 'model-0':['low'], 'model-1':[]}
  }])) }, Object.fromEntries(providers.map(p => [p, ['default', '', ...rows]]))));
}
let checks = 0;
for (const data of [catalog(0), catalog(4), catalog(64), 'not json', '{"codex":["a",17,"a"],"about":{"codex":{"labels":{"a":9},"efforts":null}}}']) {
  for (const p of providers) for (const q of ['', '日本語 model-1', 'unmatched']) {
    const args = [data, catalog(2), p, 'unlisted-current', 'model-1', 'low', q, checks % 2 ? 'ja' : 'en', checks % 3 ? p : 'claude'];
    assert.equal(lib.fresh(...args), lib.old(...args), `${p}: ${q}: ${data.length} bytes`);
    checks++;
  }
}
// Saved account catalogs, stale generations and malformed bindings preserve
// their canonical overlay behavior. Legacy catalogs need no JSON round trip.
const inventory = JSON.stringify({profiles:[{id:'A',provider:'codex',epoch:2,state:'connected',catalog:[{id:'account-model',label:'Work',efforts:['low']}]}]});
for (const binding of ['', '{"id":"","epoch":0}', '{"id":"A","epoch":2}', '{"id":"A","epoch":9}', 'broken']) {
  for (const data of [catalog(3), 'broken', 'null']) {
    const args = [data, binding, inventory];
    const parse = text => { try { return JSON.parse(text); } catch { return null; } };
    assert.deepEqual(parse(lib['fresh.overlay'](...args)), parse(lib['old.overlay'](...args)));
    checks++;
  }
}
console.log('ok selector DOM and account catalogs remain exact (' + checks + ' cases)');
const raw = ' { "codex": ["one"] }\n';
assert.equal(lib['fresh.overlay'](raw, '', ''), raw);
// Timings cover selector rendering only, not network, hub scheduling or paint.
// Alternate order and compare the complete DOM on every run.
const large = catalog(300), times = {old:[], fresh:[]};
for (let pass = 0; pass < 8; pass++) {
  const args = [large, catalog(2), 'codex', 'model-0', 'model-' + pass, 'low', '', 'en', 'codex'];
  const output = {};
  for (const name of pass % 2 ? ['fresh','old'] : ['old','fresh']) {
    const start = performance.now();output[name] = lib[name](...args);times[name].push(performance.now() - start);
  }
  assert.equal(output.fresh, output.old);
}
const median = xs => [...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
console.log(JSON.stringify({checks, catalogBytes:Buffer.byteLength(large), selectorRenderMedianMs:{old:median(times.old),fresh:median(times.fresh)}, scope:'synthetic selector render; excludes end-to-end interaction latency'}));
