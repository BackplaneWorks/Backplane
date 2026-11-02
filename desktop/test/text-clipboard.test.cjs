'use strict';
const {test} = require('node:test'), assert = require('node:assert/strict');
const {installTextClipboard} = require('../text-clipboard.cjs');

test('native text copy is write-only, trusted, bounded and acknowledged asynchronously', async () => {
  let handler, release, stored;
  installTextClipboard({handle(channel, fn) { assert.equal(channel, 'desktop:clipboard-write-text');handler = fn; }},
    {writeText: text => new Promise(resolve => { stored = text;release = resolve; })}, event => event.main === true);
  await assert.rejects(handler({}, 'secret'), /sender/);
  for (const value of [null, {}, 3, '日'.repeat(2 * 1024 * 1024)]) await assert.rejects(handler({main:true}, value), /text/);
  assert.equal(stored, undefined);
  let acknowledged = false;
  const text = '# shell comment\nprintf "日本語 & < >"\n';
  const result = handler({main:true}, text).then(value => { acknowledged = true;return value; });
  assert.equal(stored, text);await Promise.resolve();assert.equal(acknowledged, false);
  release();assert.deepEqual(await result, {copied:true});
});

test('native clipboard denial rejects without acknowledging success', async () => {
  let handler;
  installTextClipboard({handle(_channel, fn) { handler = fn; }}, {async writeText() { throw Error('write refused'); }}, () => true);
  await assert.rejects(handler({}, 'text'), /write refused/);
});
