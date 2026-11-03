const {test} = require('node:test');
const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const loaded = import('data:text/javascript;base64,' + readFileSync('src/web/star-attention.js').toString('base64'));

test('one gentle cue per offer; patches and a remounted strip do not replay it', async () => {
  const {starAttention} = await loaded, sync = starAttention(), calls = [];
  global.matchMedia = () => ({matches: false});
  const root = token => ({querySelector: () => token ? {
    getAttribute: () => token,
    querySelector: () => ({animate: (...args) => calls.push(args)})
  } : null});
  sync(root('offer')); sync(root('offer')); sync(root(null)); sync(root('offer'));
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1].iterations, 1);
  assert.equal(calls[0][1].duration, 900);
  sync(root('another-offer'));
  assert.equal(calls.length, 2);
});
test('reduced motion suppresses the cue and cannot replay it on a later render', async () => {
  const {starAttention} = await loaded, sync = starAttention();
  let calls = 0;
  const root = {querySelector: () => ({getAttribute: () => 'reduced', querySelector: () => ({animate: () => calls++})})};
  global.matchMedia = query => { assert.equal(query, '(prefers-reduced-motion: reduce)'); return {matches: true}; };
  sync(root);
  global.matchMedia = () => ({matches: false});
  sync(root);
  assert.equal(calls, 0);
});
