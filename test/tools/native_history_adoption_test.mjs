// Adapter controls inject planner answers; the actual planner has Bend tests.
// These exercise real request fences, materialization, viewport and continuation.
import assert from 'node:assert/strict';
export function registerNativeHistoryChecks(check, harness) {
  const picture = (x, ids, edit = {}) => {
    const shown = x.screen();
    shown.thread.entries = ids.map(id => ({ id, kind: 'assistant', text: id, blocks: [], ...(edit[id] || {}) }));
    return { shown, overlay: { key: 'h', name: 'h', j: structuredClone(shown) },
      window: { ids: ids.map((_, i) => String(i + 1)), older: 'older-next', hasMore: true, anchor: { id: '', offset: 0 }, partial: [] } };
  };
  const opened = ids => {
    const x = harness(); x.api.historyCapacity = ({ rows, bytes }) => rows <= 260 && bytes <= 1048576;
    x.h.hubs(['h']); x.h.connection('h', '1', 'view-v1');
    const request = x.greeting().cmds[0].request; x.answer(request.id, picture(x, ids)); return x;
  };
  const keys = x => x.h.states.get('h').shown.thread.entries.map(e => e.id);
  const plan = (ids, aliases = []) => ({ ids, aliases });
  check('older page retains outgoing visible entries in both presentation halves', () => {
    const x = opened(['a', 'b', 'c', 'd']), s = x.h.states.get('h');
    s.session.drafts.a = 'retained draft';
    assert(x.h.viewport('h', 'a', { id: 'c', offset: -37.5, first: 'b', last: 'd' }));
    x.api.history = input => { assert.deepEqual(input.keep, ['b', 'c', 'd', 'a']); return plan(['p', 'a', 'b', 'c', 'd']); };
    x.h.request('h', 'earlier', ''); const req = x.h.drain().cmds[0].request;
    assert.deepEqual(s.viewport, { id: 'c', offset: -37.5, first: 'b', last: 'd' });
    assert.deepEqual(req.p.anchor, { id: 'c', offset: 38 });
    x.answer(req.id, picture(x, ['p', 'a', 'b']));
    assert.deepEqual(keys(x), ['p', 'a', 'b', 'c', 'd']);
    assert.deepEqual(s.overlay.j.thread.entries, s.shown.thread.entries);
    assert.equal(s.shown.thread.historyAnchor.offset, -37.5);
    assert.equal(s.session.drafts.a, 'retained draft');
  });
  check('a reversed gesture reports its current visible range while a page travels', () => {
    const x = opened(['a', 'b', 'c', 'd']);
    x.api.history = () => plan(['p', 'a', 'b', 'c', 'd']);
    x.h.viewport('h', 'a', { id: 'b', offset: -1, first: 'b', last: 'c' });
    x.h.request('h', 'earlier', ''); const req = x.h.drain().cmds[0].request;
    assert(x.h.viewport('h', 'a', { id: 'c', offset: -15, first: 'c', last: 'd' }));
    x.answer(req.id, picture(x, ['p', 'a', 'b']));
    assert.deepEqual(keys(x), ['p', 'a', 'b', 'c', 'd']);
    assert.equal(x.h.states.get('h').shown.thread.historyAnchor.id, 'c');
  });
  check('refresh retains the visible outgoing bridge after prepend', () => {
    const x = opened(['a', 'b', 'c']); x.api.history = () => plan(['p', 'a', 'b', 'c']);
    x.h.viewport('h', 'a', { id: 'c', offset: -3, first: 'b', last: 'c' });
    x.h.request('h', 'earlier', ''); let req = x.h.drain().cmds[0].request; x.answer(req.id, picture(x, ['p', 'a']));
    x.h.update(x.h.states.get('h')); req = x.h.drain().cmds[0].request;
    x.answer(req.id, picture(x, ['p', 'a'])); assert.deepEqual(keys(x), ['p', 'a', 'b', 'c']);
  });
  check('newer page carries direction and retains its visible older prefix', () => {
    const x = opened(['p', 'a', 'b']), s = x.h.states.get('h'); s.back = ['newer'];
    x.api.history = input => { assert.equal(input.older, false); return plan(['p', 'a', 'b', 'c']); };
    x.h.viewport('h', 'a', { id: 'p', offset: -4, first: 'p', last: 'b' });
    x.api.intent = () => ({ kind: 'later' }); x.h.request('h', 'later', '');
    const req = x.h.drain().cmds[0].request; x.answer(req.id, picture(x, ['b', 'c']));
    assert.deepEqual(keys(x), ['p', 'a', 'b', 'c']);
  });
  check('foreign, absent, reversed and nonfinite viewport reports are inert', () => {
    const x = opened(['a', 'b']);
    for (const [thread, value] of [['other', { id: 'a', offset: 0, first: 'a', last: 'b' }],
      ['a', { id: 'absent', offset: 0, first: 'a', last: 'b' }], ['a', { id: 'a', offset: 0, first: 'b', last: 'a' }],
      ['a', { id: 'a', offset: NaN, first: 'a', last: 'b' }]]) assert.equal(x.h.viewport('h', thread, value), false);
    assert.equal(x.h.states.get('h').viewport, null); assert.deepEqual(x.h.drain().cmds, []);
  });
  check('reheaded visible fold keeps one key with the current canonical action', () => {
    const x = opened(['fold:old', 'm']); let s = x.h.states.get('h');
    for (const screen of [s.shown, s.overlay.j]) screen.thread.entries[0] = { id: 'fold:old', kind: 'fold', text: '2 calls', open: true, value: '0old' };
    s.historyPage = structuredClone(s.shown.thread.entries);
    x.h.viewport('h', 'a', { id: 'fold:old', offset: -9, first: 'fold:old', last: 'm' });
    x.api.history = () => plan(['p', 'fold:old', 'm'], [{ from: 'fold:new', to: 'fold:old' }]);
    x.h.request('h', 'earlier', ''); const req = x.h.drain().cmds[0].request;
    x.answer(req.id, picture(x, ['p', 'fold:new', 'm'], { 'fold:new': { kind: 'fold', text: '5 calls', open: false, value: '1new' } }));
    assert.deepEqual(keys(x), ['p', 'fold:old', 'm']); assert.equal(s.shown.thread.entries[1].text, '2 calls');
    assert.equal(s.shown.thread.entries[1].value, '0new'); assert.equal(s.shown.thread.entries[1].open, true);
  });
  check('local retention bound parks the new picture instead of clipping visible rows', () => {
    const x = opened(['a']), s = x.h.states.get('h'), ids = Array.from({ length: 260 }, (_, i) => 'old' + i);
    for (const screen of [s.shown, s.overlay.j]) screen.thread.entries = ids.map(id => ({ id, kind: 'assistant', text: id, blocks: [] }));
    x.h.viewport('h', 'a', { id: ids[0], offset: -7, first: ids[0], last: ids.at(-1) });
    x.api.history = () => plan(['p', ...ids]); x.h.request('h', 'earlier', ''); const req = x.h.drain().cmds[0].request;
    x.answer(req.id, picture(x, ['p'])); assert.deepEqual(keys(x), ids); assert.equal(s.historyParked, true);
    x.h.request('h', 'earlier', ''); assert.deepEqual(x.h.drain().cmds, []);
    x.api.history = input => { assert.equal(input.page, false); return plan(['p', ids[0]]); };
    assert(x.h.viewport('h', 'a', { id: ids[0], offset: -8, first: ids[0], last: ids[0] }));
    const refresh = x.h.drain().cmds[0].request;
    x.answer(refresh.id, picture(x, ['p']));
    assert.equal(s.historyParked, false); assert.deepEqual(keys(x), ['p', ids[0]]);
    x.h.request('h', 'earlier', ''); assert.equal(x.h.drain().cmds[0].request.m, 'view.page');
  });
}
