'use strict';
// Called by the native owner with the actual emitted host and planner.
// This is the two-page shape from the retained Android observation, with
// anonymous non-tool ids. Action payloads are source-reconstructed: the
// original trace captured Entry ids, not the complete wire fold objects.
const assert = require('node:assert/strict');
const plain = value => JSON.parse(JSON.stringify(value));
const fold = (head, open = false, text = '6 calls') => ({ id: 'fold:' + head, kind: 'fold', value: (open ? '0' : '1') + head, open, text });
const row = id => ({ id, kind: 'assistant', text: id, blocks: [] });
const picture = entries => ({ shown: { thread: { id: 'h|t', entries } }, overlay: { j: { thread: { id: 'h|t', entries: plain(entries) } } } });
const state = entries => ({ shown: picture(entries).shown, viewport: { id: 'user', first: 'user', last: 'ride', offset: -37 }, historyPage: entries });

function runNativeFoldBoundaryCases(q, check) {
  assert.equal(typeof q.history, 'function', 'actual emitted planner must be supplied');
  assert.equal(typeof q.host?.projectHistory, 'function', 'actual emitted host must be supplied');
  check('emitted two-page fold lineage survives a fresh page ending at the fold', () => {
    const s = state([row('user'), fold('item_9', true, '3 calls'), row('ride')]);
    const preceding = q.host.projectHistory(s, picture([row('older'), row('user'), fold('item_6'), row('ride')]), { direction: 'older' });
    assert.deepEqual(plain(preceding.shown.thread.entries.map(e => e.id)), ['older', 'user', 'fold:item_9', 'ride']);
    const held = preceding.shown.thread.entries[2];
    assert.equal(held.value, '0item_6');
    assert.equal(held.text, '3 calls');
    s.shown = preceding.shown; s.historyPage = preceding.historyPage;
    const clipped = q.host.projectHistory(s, picture([row('earlier'), row('older'), row('user'), fold('item_6')]), { direction: 'older' });
    assert.deepEqual(plain(clipped.shown.thread.entries.map(e => e.id)), ['earlier', 'older', 'user', 'fold:item_9', 'ride']);
    assert.deepEqual(plain(clipped.shown.thread.entries[3]), plain(held));
    assert.deepEqual(plain(clipped.overlay.j.thread.entries), plain(clipped.shown.thread.entries));
    assert.equal(clipped.shown.thread.historyAnchor.offset, -37);
  });
  check('emitted remembered fold identity never merges an unrelated page-end fold', () => {
    const s = state([row('user'), { ...fold('item_9'), value: '1item_6' }, row('ride')]);
    const unrelated = q.host.projectHistory(s, picture([row('older'), row('user'), fold('other')]), { direction: 'older' });
    assert.deepEqual(plain(unrelated.shown.thread.entries.map(e => e.id)), ['older', 'user', 'fold:other', 'fold:item_9', 'ride']);
    assert.equal(unrelated.shown.thread.entries[2].value, '1other');
  });
  check('emitted planner rejects stale, ambiguous and malformed lineage', () => {
    const input = { old: ['user', 'fold:item_9', 'ride'], fresh: ['older', 'user', 'fold:item_6'],
      keep: ['user', 'fold:item_9', 'ride'], visible: ['user', 'fold:item_9', 'ride'], page: true, older: true,
      oldRows: [{ ...fold('item_9'), value: '1item_6' }], freshRows: [fold('item_6')] };
    assert.deepEqual(plain(q.history(input).aliases), [{ from: 'fold:item_6', to: 'fold:item_9' }]);
    const malformed = plain(input); malformed.oldRows[0].open = true;
    assert.deepEqual(plain(q.history(malformed).aliases), []);
    const wrong = plain(input); wrong.freshRows[0].value = '1other';
    assert.deepEqual(plain(q.history(wrong).aliases), []);
    const absent = plain(input); absent.old = ['user', 'ride']; absent.keep = absent.visible = absent.old;
    assert.deepEqual(plain(q.history(absent).aliases), []);
    const duplicate = plain(input); duplicate.old.push('fold:another'); duplicate.oldRows.push({ ...fold('another'), value: '1item_6' });
    assert.deepEqual(plain(q.history(duplicate).aliases), []);
  });
  check('emitted planner applies exactly returned aliases without a cascading second pass', () => {
    const input = { old: ['fold:a', 'fold:d', 'x'], fresh: ['fold:b', 'fold:c', 'x'],
      keep: ['fold:a', 'fold:d', 'x'], visible: ['fold:a', 'fold:d', 'x'], page: true, older: true,
      oldRows: [fold('a'), fold('d')], freshRows: [fold('b'), fold('c')] };
    const plan = plain(q.history(input));
    assert.deepEqual(plan.aliases, [{ from: 'fold:c', to: 'fold:d' }]);
    assert.deepEqual(plan.ids, ['fold:b', 'fold:a', 'fold:d', 'x']);
  });
  check('emitted host omits large non-fold values from lineage descriptors', () => {
    const original = q.host.api.history;
    let observed = false;
    q.host.api.history = input => {
      observed = true;
      for (const descriptor of [...input.oldRows, ...input.freshRows]) {
        if (descriptor.kind !== 'fold') assert.deepEqual(Object.keys(descriptor).sort(), ['id', 'kind']);
      }
      assert(JSON.stringify(input).length < 4096);
      return original(input);
    };
    try {
      const s = state([{ ...row('user'), value: 'large non-fold value '.repeat(16000) }, row('ride')]);
      q.host.projectHistory(s, picture([row('older'), { ...row('user'), value: 'another large value '.repeat(16000) }]), { direction: 'older' });
      assert(observed);
    } finally { q.host.api.history = original; }
  });
}
module.exports = { runNativeFoldBoundaryCases };
