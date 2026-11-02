import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Issue #404: the plot diff compares board-sized strings. Keep the emitted
// runtime below macOS's 64 MiB stack even when test.sh raises the checker's
// allowance. A recursive call beneath Bool.and must fail at this size.
if (!process.argv[3]) {
  const result = spawnSync('/bin/sh', ['-c', 'ulimit -s 65536 && exec "$0" "$@"',
    process.execPath, import.meta.filename, resolve(process.argv[2]), 'child'], {
    env: { ...process.env, BUN_JSC_maxPerThreadStackUsage: '33554432' },
    encoding: 'utf8',
    timeout: 120000,
  });
  process.stdout.write(result.stdout ?? '');
  process.stderr.write((result.stderr ?? '').slice(0, 4000));
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `string equality child failed (${result.signal ?? 'no signal'})`);
} else {
  const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
  const check = (name, a, b, want) => {
    assert.equal(Number(lib.equal(a, b)), want, name);
    console.log(`ok Eq.str ${name}`);
  };
  const long = 'a'.repeat(2000000);
  check('two million equal characters', long, 'a'.repeat(2000000), 1);
  check('last character differs', long, 'a'.repeat(1999999) + 'b', 0);
  check('right is longer', long, long + 'a', 0);
  check('left is longer', long + 'a', long, 0);
  check('first character differs', 'b' + long, 'a' + long, 0);
  check('empty strings', '', '', 1);
  check('empty left', '', 'a', 0);
  check('empty right', 'a', '', 0);
  check('short equal', 'ab', 'ab', 1);
  check('short unequal', 'ab', 'ac', 0);
  check('Unicode equal', '板🦋é', '板🦋é', 1);
  check('Unicode unequal', '板🦋é', '板🦋e', 0);
}
