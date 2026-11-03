// Tiny emitted-runtime regressions for checked Unicode escape transitions.
// No network, hub, real data or budget-sized fixtures.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
const F = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const bad = ["\"\\uZZZZ\"", "\"\\u123\"", "\"\\u123", "\"\\u/000\"", "\"\\u:000\"", "\"\\uG000\"", "\"\\ug000\"", "\"\\u0G01\" null", "[\"\\u0G01\",1]", "\"\\ud83d\\u0G01\""];
for (const text of bad) assert.equal(F.parse_show(text), '<bad>');
for (const text of ["\"\\u0000\"", "\"\\u00aF\"", "\"\\uFFFF\"", "\"\\uD83D\\udc69\""]) {
  assert.equal(F.parse_show(text), JSON.stringify(JSON.parse(text)));
}
assert.equal(F.parse_show("\"\\uD800\""), JSON.stringify('�'));
console.log('ok 15 tiny emitted Unicode-escape contracts');
