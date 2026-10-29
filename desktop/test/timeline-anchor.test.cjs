'use strict';
const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const source = fs.readFileSync(path.resolve(__dirname, '../../src/web/host.js'), 'utf8');
const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
function* each(xs) { for (; xs?.$ === 'Con'; xs = xs.tail) yield xs.head; }
function fixture() {
  const tl = { id: 'timeline', scrollTop: 250, scrollHeight: 2000, clientHeight: 200, clientWidth: 700, rows: [],
    getBoundingClientRect: () => ({ top: 0, bottom: 200 }),
    querySelectorAll() { return this.rows; },
    querySelector(selector) { return this.rows.find(r => selector === `[data-id="${r.id}"]`) || null; },
  };
  const ctx = { each, is: (x, t) => x.$ === t, App: { hold_at_end: n => n <= 4n, sel: () => 'thread' }, ui: null,
    focusedNode: null, frontendNode: {}, thin: { atNewest: () => true }, performance: { now: () => 1000 },
    document: { addEventListener() {}, getElementById: () => tl }, CSS: { escape: x => x }, queueMicrotask() {}, ResizeObserver: undefined };
  vm.createContext(ctx);
  vm.runInContext(source.slice(source.indexOf('function patchKey('), source.indexOf('// make el')), ctx);
  vm.runInContext(source.slice(source.indexOf('function anchorOf('), source.indexOf('const thinMailDraft')), ctx);
  function row(id, y, height = 100) {
    const r = { id, y, height, getBoundingClientRect: () => ({ top: r.y - tl.scrollTop, bottom: r.y - tl.scrollTop + r.height, height: r.height }),
      getAttribute: name => name === 'data-id' ? id : null };
    return r;
  }
  return { ctx, tl, row, set: (name, value) => { ctx.value = value;vm.runInContext(`${name}=value`, ctx); }, get: name => vm.runInContext(name, ctx) };
}
test('fold keys retain run identity across toggles and distinguish preceding runs', () => {
  const { ctx } = fixture();
  const fold = (id, open) => ({ $: 'El', tag: 'button', key: '', attrs: list([
    { $: 'At', name: 'class', value: 'fold' }, { $: 'At', name: 'data-value', value: (open ? '0' : '1') + id }, { $: 'On', event: 'click', action: 'fold' }]) });
  assert.equal(ctx.patchKey(fold('first', false)), ctx.patchKey(fold('first', true)));
  assert.notEqual(ctx.patchKey(fold('first', false)), ctx.patchKey(fold('older', false)));
  assert.equal(ctx.patchKey({ ...fold('first', false), key: 'server-key' }), 'server-key');
});
test('prepend restores the visible entry including its within-entry offset', () => {
  const f = fixture(), a = f.row('a', 200), b = f.row('b', 300);f.tl.rows = [a, b];
  f.set('held', f.ctx.anchorOf(f.tl));a.y += 325.5;b.y += 325.5;f.tl.scrollHeight += 325.5;
  f.ctx.holdFix(f.tl);assert.equal(a.getBoundingClientRect().top, -50);assert.equal(b.getBoundingClientRect().top, 50);
});
test('device-pixel scroll rounding retains a fractional anchor with a bounded track inset', () => {
  const f = fixture(), a = f.row('a', 200), b = f.row('b', 300);f.tl.rows = [a, b];
  let top = 250, inset = 0;
  Object.defineProperty(f.tl, 'scrollTop', { get: () => top, set: v => { top = Math.round(v); } });
  f.tl.style = { setProperty(name, value) {
    assert.equal(name, '--anchor-inset');const next = parseFloat(value);
    for (const row of f.tl.rows) row.y += next - inset;
    inset = next;assert(inset >= 0 && inset < 1);
  } };
  f.set('held', f.ctx.anchorOf(f.tl));
  for (let i = 0; i < 12; i++) {
    a.y += 325.5;b.y += 325.5;f.tl.scrollHeight += 325.5;
    f.ctx.holdFix(f.tl);assert.equal(a.getBoundingClientRect().top, -50);
  }
});
test('a removed fold falls back to the next surviving visible entry at its own offset', () => {
  const f = fixture(), a = f.row('fold-a', 200), b = f.row('b', 300);f.tl.rows = [a, b];
  f.set('held', f.ctx.anchorOf(f.tl));f.tl.rows = [b];b.y += 200;
  f.ctx.holdFix(f.tl);assert.equal(b.getBoundingClientRect().top, 50);assert.equal(f.get('held').id, 'b');
});
test('an initial or unchanged resize notification never undoes an ongoing user scroll', () => {
  const f = fixture(), a = f.row('a', 200, 500);f.tl.rows = [a];
  f.set('held', f.ctx.anchorOf(f.tl));f.ctx.holdFix(f.tl);
  f.tl.scrollTop = 350;f.ctx.holdResize([]);
  assert.equal(f.tl.scrollTop, 350, 'unchanged geometry leaves the user movement alone');
  a.y += 100;f.tl.scrollHeight += 100;f.ctx.holdRead(f.tl);
  assert.equal(a.getBoundingClientRect().top, -50, 'a real layout change still repairs the stored anchor');
});
test('streaming within a third of the bottom does not opt a reader into following', () => {
  const f = fixture();f.tl.scrollTop = 1780;f.tl.rows = [f.row('tail', 1700, 300)];f.set('towardEndUntil', 2000);
  f.ctx.App.hold_catch_up = () => { throw Error('Near-bottom catch-up must not run'); };
  f.ctx.holdRead(f.tl);assert.equal(f.get('held').id, 'tail');
  f.tl.scrollTop = 1800;f.ctx.holdRead(f.tl);assert.equal(f.get('held'), null);
});
test('a page window bottom is not the conversation bottom', () => {
  const f = fixture();f.ctx.thin.atNewest = () => false;f.tl.scrollTop = 1800;f.tl.rows = [f.row('old-tail', 1700, 300)];
  f.ctx.holdRead(f.tl);assert.equal(f.get('held').id, 'old-tail');
});
