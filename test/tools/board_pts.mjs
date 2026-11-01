import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// A routed board's zone fills faulted the board viewer on macOS: Board.pts
// kept a frame per polygon point in the --plot child. test.sh raises the
// runtime's stack allowance, so each case runs in a child with a small one,
// as test/tools/glb_stack.mjs does: Bun's own bound on an 8 MiB stack, about
// 40k frames. A walk that is not a tail call fails here at a fill's size.
const cases = ['fill', 'last', 'order'];
if (!process.argv[3]) {
  let failed = false;
  for (const name of cases) {
    const env = { ...process.env };
    delete env.BUN_JSC_maxPerThreadStackUsage;
    const result = spawnSync('/bin/sh', ['-c', 'ulimit -s 8192 && exec "$0" "$@"',
      process.execPath, import.meta.filename, resolve(process.argv[2]), name], {
      env,
      encoding: 'utf8',
      timeout: 120000,
    });
    process.stdout.write(result.stdout ?? '');
    process.stderr.write((result.stderr ?? '').slice(0, 2000));
    if (result.error) throw result.error;
    if (result.status !== 0) {
      console.log(`FAIL board pts ${name} child exited ${result.status} (${result.signal ?? 'no signal'})`);
      failed = true;
    }
  }
  assert.equal(failed, false, 'every polygon must be read on a small stack');
} else {
  const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
  // a zone fill's points, more than any board in hand
  const fill = 200000;
  switch (process.argv[3]) {
    case 'fill':
      assert.equal(Number(lib.count(fill)), fill);
      break;
    case 'last':
      // the walk keeps the file's order to the end
      assert.equal(lib.last(fill), String(fill - 1));
      break;
    case 'order':
      // points only, in the file's order
      assert.equal(lib.order(), '1:2,3:4,5:6,');
      break;
    default:
      throw new Error(`unknown board pts case: ${process.argv[3]}`);
  }
  console.log(`ok board pts ${process.argv[3]}`);
}
