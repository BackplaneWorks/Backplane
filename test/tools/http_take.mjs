import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// An image attached through POST /attach faulted the hub on macOS: the bytes
// that came with the request's head went through List.take, a frame per
// byte. test.sh raises the runtime's stack allowance, so each case runs in a
// child with a small one, as test/tools/glb_stack.mjs does: Bun's own bound
// on an 8 MiB stack, about 40k frames. A walk that is not a tail call fails
// here at an image's size.
const cases = ['image', 'short', 'rest', 'none', 'order'];
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
      console.log(`FAIL http take ${name} child exited ${result.status} (${result.signal ?? 'no signal'})`);
      failed = true;
    }
  }
  assert.equal(failed, false, 'every upload head must be taken on a small stack');
} else {
  const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
  const n = (x) => Number(x);
  // a screenshot's bytes, all of them with the head
  const image = 2000000;
  switch (process.argv[3]) {
    case 'image':
      assert.equal(n(lib.head_len(image, image)), image);
      break;
    case 'short':
      // a body cut short: what came, never more
      assert.equal(n(lib.head_len(image, image + 1000)), image);
      break;
    case 'rest':
      // only the first part came with the head; the socket copies the rest
      assert.equal(n(lib.head_len(image, image / 2)), image / 2);
      break;
    case 'none':
      assert.equal(n(lib.head_len(image, 0)), 0);
      break;
    case 'order':
      assert.equal(lib.order(), '1,2,');
      break;
    default:
      throw new Error(`unknown http take case: ${process.argv[3]}`);
  }
  console.log(`ok http take ${process.argv[3]}`);
}
