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
    const args = [data, p, 'unlisted-current', checks % 3 ? 'yes' : 'no', q];
    assert.equal(lib.fresh(...args), lib.old(...args), `${p}: ${q}: ${data.length} bytes`);
    checks++;
  }
}
console.log('ok selector rows, labels, search and current-model retention remain exact (' + checks + ' cases)');
// Timings cover catalog preparation only, not DOM, network, scheduling or paint.
// Alternate order and compare all visible catalog data on every run.
const large = catalog(300), times = {old:[], fresh:[]};
for (let pass = 0; pass < 8; pass++) {
  const args = [large, 'codex', 'model-' + pass, 'yes', ''];
  const output = {};
  for (const name of pass % 2 ? ['fresh','old'] : ['old','fresh']) {
    const start = performance.now();output[name] = lib[name](...args);times[name].push(performance.now() - start);
  }
  assert.equal(output.fresh, output.old);
}
const median = xs => [...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
console.log(JSON.stringify({checks, catalogBytes:Buffer.byteLength(large), catalogPreparationMedianMs:{old:median(times.old),fresh:median(times.fresh)}, scope:'synthetic catalog preparation; excludes DOM, network and end-to-end interaction latency'}));
