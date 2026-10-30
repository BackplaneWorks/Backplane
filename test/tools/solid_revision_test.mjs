// bun test/tools/solid_revision_test.mjs BUILD/solid_revision_probe
// All writes, export cache directories and promotions stay in a temp folder.
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync, readFileSync, statSync, existsSync, renameSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
const probe = resolve(process.argv[2] || 'build/solid_revision_probe');
assert.ok(existsSync(probe), 'build test/native/solid_revision_probe.bend first');
const dir = mkdtempSync(join(tmpdir(), 'bp-solid-revision-'));
const env = {...process.env, XDG_CACHE_HOME: join(dir, 'cache'), BACKPLANE_HOME: join(dir, 'home'), BACKPLANE_TELEMETRY: 'off'};
function run(...args) {
  const r = spawnSync(probe, args, {encoding: 'utf8', env, timeout: 10000});
  assert.equal(r.status, 0, r.stderr);return r.stdout.trim().split('\n');
}
try {
  const results = [];
  for (const extension of ['kicad_pcb', 'step', 'glb']) {
    const path = join(dir, `source.${extension}`);
    let before, after, firstTime, secondTime;
    // Two probe processes should fit inside one second. Retry at most three
    // times if the system was scheduled across a whole-second boundary.
    for (let attempt = 0; attempt < 3; attempt++) {
      writeFileSync(path, 'first!');firstTime = statSync(path, {bigint: true}).mtimeNs;
      before = run(path);
      writeFileSync(path, 'second');secondTime = statSync(path, {bigint: true}).mtimeNs;
      after = run(path);
      if (firstTime / 1000000000n === secondTime / 1000000000n) break;
    }
    assert.equal(firstTime / 1000000000n, secondTime / 1000000000n, 'same-second condition not exercised');
    assert.equal(before.length, 4);assert.equal(after.length, 4);
    for (let i = 0; i < 4; i++) assert.notEqual(before[i], after[i], `stale ${extension} source/cache revision ${i}`);
    assert.deepEqual(run(path), after, 'unchanged source must reuse exact cache identity');
    assert.notEqual(after[2], after[3], 'with-parts and bare exports must not share a cache');
    writeFileSync(path + '.tmp', 'second');renameSync(path + '.tmp', path);
    const atomic = run(path);
    for (let i = 0; i < 4; i++) assert.notEqual(atomic[i], after[i], 'atomic replacement has a new file identity');
    results.push({extension, same_second: true, size: 6, exact_revision: true, atomic_replacement: true});
  }
  const output = join(dir, 'export.glb');
  writeFileSync(output, 'last good');writeFileSync(output + '.part', 'partial failed');
  assert.deepEqual(run('promote', '1', output), ['1']);
  assert.equal(readFileSync(output, 'utf8'), 'last good', 'failed export replaced cached success');
  writeFileSync(output + '.part', 'new complete');
  assert.deepEqual(run('promote', '0', output), ['0']);
  assert.equal(readFileSync(output, 'utf8'), 'new complete');assert.equal(existsSync(output + '.part'), false);
  assert.deepEqual(run('promote', '0', join(dir, 'missing.glb')), ['3'], 'missing staging file must fail promotion');
  console.log(JSON.stringify({sources: results, promotion: {failed_keeps_previous: true, success_atomic: true, rename_error: true}}));
} finally { rmSync(dir, {recursive: true, force: true}); }
