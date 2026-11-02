// Run the same contract cases through Bend's JavaScript export.
// Usage: node test/allowance_test.mjs [precompiled-test-module.mjs]
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

const stage = process.argv[2] ? null : mkdtempSync(join(tmpdir(), 'backplane-allowance-'));
try {
  const output = process.argv[2] ? resolve(process.argv[2]) : join(stage, 'allowance-test.mjs');
  if (stage) {
    const result = spawnSync('bend', [fileURLToPath(new URL('../allowance_test.bend', import.meta.url)), '-o', output], {encoding: 'utf8'});
    assert.equal(result.status, 0, result.error?.message || result.stdout + result.stderr);
  }
  const lib = (await import(pathToFileURL(output).href)).default;
  let count = 0;
  for (let cases = lib.cases(); cases.$ === 'Con'; cases = cases.tail) {
    const {label, got, want} = cases.head;
    assert.equal(got, want, label);
    console.log('ok ' + label);
    count++;
  }
  assert(count > 0, 'contract cases were exported');
  console.log(`${count} allowance contract cases passed (JavaScript export)`);
} finally {
  if (stage) rmSync(stage, {recursive: true, force: true});
}
