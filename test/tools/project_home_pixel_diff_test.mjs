import assert from 'node:assert/strict';
import { pixelDiff } from './project_home_viewer_parity.mjs';

// Validate the acceptance metric, including the requested no-mirroring case.
const frame = Uint8Array.from({length: 16 * 8 * 4}, (_, i) => i % 4 === 3 ? 255 : ((i * 73) ^ (i >> 4)) % 256);
assert.equal(pixelDiff(frame, frame).fraction, 0);
const noise = frame.map((v, i) => i % 4 === 3 ? v : Math.min(255, v + 2));
assert.equal(pixelDiff(frame, noise).fraction, 0, 'small raster rounding is tolerated');
const mirrored = Uint8Array.from(frame);
for (let y = 0; y < 8; y++) for (let x = 0; x < 16; x++) {
  mirrored.set(frame.subarray((y * 16 + x) * 4, (y * 16 + x + 1) * 4), (y * 16 + 15 - x) * 4);
}
assert.ok(pixelDiff(frame, mirrored).fraction > 0.5, 'mirroring must fail the pixel gate');
assert.ok(pixelDiff(frame, new Uint8Array(frame.length)).fraction > 0.5, 'missing model must fail');
assert.throws(() => pixelDiff(frame, frame.subarray(4)), /dimensions differ/);
assert.throws(() => pixelDiff([], []), /empty/);
console.log('ok actual-frame diff metric detects mirrored, missing and differently sized renders');
