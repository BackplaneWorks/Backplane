// Controlled production methods: scheduling/invalidation, not Canvas pixels.
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.resolve(__dirname, '../../src/web/plot2d.js'), 'utf8');
const body = source.slice(source.indexOf('class View {'), source.indexOf('// the views on the page, by canvas'));

function fixture() {
  let paints = 0, images = 0, timer = 0;
  const timers = new Map(), components = new Map();
  const context = { setTransform() {}, fillRect() {}, drawImage() { images++; } };
  const canvas = { clientWidth: 480, clientHeight: 320, width: 480, height: 320, getContext: () => context };
  const plot = { chunks: [{}] };
  const View = vm.runInNewContext(body + '\nView;', {
    window: { devicePixelRatio: 1 }, viewDpr: () => 1, coarse: null, document: { createElement: () => ({ width: 0, height: 0, getContext: () => context }) },
    performance: { now: () => 0 }, stats: new Map(), plots: new Map([['fixture', plot]]), componentSources: components,
    setTimeout(fn) { timers.set(++timer, fn); return timer; }, clearTimeout(id) { timers.delete(id); },
    clearInterval() {}, cancelAnimationFrame() {}, cancelIdleCallback() {},
  });
  const view = Object.assign(Object.create(View.prototype), {
    canvas, key: 'fixture', plot, off: 0, dim: 0, lift: 255, fade: 0, bg: '#000', scale: 1, ox: 0, oy: 0,
    pins() {}, treeWaiting: () => false, later() {}, paint() { paints++; return false; },
  });
  view.draw();
  view.bake(plot, view.look, canvas.width, canvas.height, 1);
  return { view, components, timers, counts: () => ({ paints, images }) };
}

test('motion uses the bitmap and a rested frame returns to vector drawing', () => {
  const f = fixture(), before = f.counts();
  f.view.ox += 0.375;
  f.view.draw();
  assert.equal(f.counts().paints, before.paints);
  assert.equal(f.counts().images, before.images + 1);
  f.view.rest = true;
  f.view.draw();
  assert.equal(f.counts().paints, before.paints + 1);
  assert.equal(f.view.moving, false);
});

test('probe and selected-component metadata bypass the base cache', () => {
  const f = fixture(), before = f.counts();
  f.view.probe = 'p:R1';
  f.view.ox += 1;
  f.view.draw();
  assert.equal(f.counts().paints, before.paints + 1);
  assert.equal(f.counts().images, before.images);
  f.view.probe = '';
  f.components.set('fixture', { selected: 'R1' });
  f.view.ox += 1;
  f.view.draw();
  assert.equal(f.counts().paints, before.paints + 2);
  assert.equal(f.counts().images, before.images);
});

test('layer and fade changes invalidate motion caches', () => {
  for (const [field, value] of [['off', 1], ['fade', 120], ['bg', '#123']]) {
    const f = fixture(), before = f.counts();
    f.view[field] = value;
    f.view.ox += 1;
    f.view.draw();
    assert.equal(f.counts().paints, before.paints + 1, field);
    assert.equal(f.counts().images, before.images, field);
  }
});
