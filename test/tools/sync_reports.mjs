import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
const F = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const reports = [[7, 1n, 100n], [7, 10n, 5n], [7, 4n, 600n], [7, 10n, 600n], [8, 999n, 9999n]];
function* permutations(xs) {
  if (!xs.length) { yield []; return; }
  for (let i = 0; i < xs.length; i++) for (const tail of permutations(xs.filter((_, j) => j !== i))) yield [xs[i], ...tail];
}
const fold = xs => xs.reduce((c, r) => F.report(c, ...r), F.initial());
const expected = F.report(F.initial(), 7, 10n, 600n);
let checks = 0;
for (const order of permutations(reports)) {
  assert.deepEqual(fold(order), expected);
  assert.deepEqual(fold(order.flatMap(r => [r, r])), expected);
  assert.deepEqual(fold([...order, ...order.toReversed()]), expected);
  checks += 3;
}
assert.deepEqual(F.report(F.initial(), 8, 1000n, 10000n), F.initial());
console.log(`ok cumulative report convergence: ${checks} delivery orders/replays, stale generations ignored, complete reducer state equal`);
