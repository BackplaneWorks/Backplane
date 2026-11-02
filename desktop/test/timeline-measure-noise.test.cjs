const { test } = require('node:test');
const assert = require('node:assert/strict');

async function measurement(previous) {
  const { TimelineScroller } = await import('../../src/web/timeline-scroller.js');
  const s = Object.create(TimelineScroller.prototype);
  let height = 100;
  Object.assign(s, { _measureAll: true, _observer: null, _dirty: new Set(),
    mounted: new Map([['row', { wrapper: { getBoundingClientRect: () => ({ height }) } }]]),
    _byKey: new Map([['row', {}]]), sizes: previous === undefined ? {} : { row: previous } });
  return { s, set: value => { height = value; } };
}

test('alternating DOMRect rounding noise does not keep requesting plans', async () => {
  const { s, set } = await measurement(100);
  let changed = 0;
  for (let i = 0; i < 100; i++) { set(100 + (i % 2 ? 1 / 65536 : 0)); changed += Number(s._readSizes()); }
  assert.equal(changed, 0);
  assert.equal(s.sizes.row, 100);
});
test('genuine fractional-pixel changes and first measurements are retained', async () => {
  const { s, set } = await measurement();
  assert.equal(s._readSizes(), true);
  set(100.25); assert.equal(s._readSizes(), true); assert.equal(s.sizes.row, 100.25);
  assert.equal(s._readSizes(), false);
});
test('a retired row never restores a measured size', async () => {
  const { s } = await measurement(); s._byKey.clear();
  assert.equal(s._readSizes(), false); assert.equal(s.sizes.row, undefined);
});
