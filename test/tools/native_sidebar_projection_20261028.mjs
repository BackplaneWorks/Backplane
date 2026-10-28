// Actual Bend Actor output against the production native adapter validators.
// No emitted bridge, service execution, app, simulator or delivery claim.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { validateNativeResult307265, validateNativeSidebar307265, validateNativeSidebarAction307265 } from '../../src/mobile/thin-native-host-307265.js';

const [directory] = process.argv.slice(2);
assert(directory, 'usage: node test/tools/native_sidebar_projection_20261028.mjs FIXTURE_DIRECTORY');
const receiptBytes = fs.readFileSync(path.join(directory, 'RECEIPT.json'));
const receipt = JSON.parse(receiptBytes), results = [];
let pictures = 0, paging = 0, redirects = 0;
for (const item of receipt.fixtures) {
  const bytes = fs.readFileSync(path.join(directory, item.name + '.json'));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), item.sha256, 'Fixture bytes changed');
  const { request, result } = JSON.parse(bytes);
  if (!result.shown) {
    assert(result.nextTarget && result.remoteReady === false);
    redirects++;
    results.push({ name: item.name, status: 'redirect-excluded' });
    continue;
  }
  assert(validateNativeResult307265(result, request), 'General native contract: ' + item.name);
  assert(validateNativeSidebar307265(result, request, request.nativeSidebar === 1), 'Sidebar contract: ' + item.name);
  if (request.action === 'native-sidebar-page') {
    assert(validateNativeSidebarAction307265(result, request), 'Read-only paging contract: ' + item.name);
    paging++;
  }
  pictures++;
  results.push({ name: item.name, status: 'accepted', bytes: Buffer.byteLength(JSON.stringify(result)) });
}
assert(pictures > 0 && paging > 0, 'Actual ready and executed paging fixtures required');
console.log(JSON.stringify({ pass: true, scope: 'Actual Actor fixtures + native source adapter validators only',
  receiptSHA256: createHash('sha256').update(receiptBytes).digest('hex'), pictures, paging, redirectsExcluded: redirects, results }));
