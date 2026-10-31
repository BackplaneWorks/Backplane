const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');
function harness() {
  const source = fs.readFileSync(require.resolve('../../src/web/plot2d.js'), 'utf8');
  const start = source.indexOf('class View {'), end = source.indexOf('\n}\n', start) + 2;
  const handlers = {}, probes = [], navigations = [];let reads = 0, resize, wait = Promise.resolve();
  const canvas = { get clientWidth() { reads++;return 900; }, get clientHeight() { reads++;return 600; }, isConnected: true,
    addEventListener: (name, fn) => { handlers[name] = fn; }, setPointerCapture() {}, focus() {},
    closest: () => ({ querySelector: selector => ({ click: () => navigations.push(selector) }) }),
    getBoundingClientRect: () => ({ left: 0, top: 0 }), classList: { add() {}, remove() {} } };
  const context = { ResizeObserver: class { constructor(fn) { resize = fn; } observe() {} }, requestAnimationFrame() {},
    views: new Map(), componentSources: new Map(), sheeted: null, need: () => wait,
    probed: value => probes.push(value), under: () => [], partAt: () => null,
    groupsOf: () => [], P: { pickBoard: () => null, pickSheet: () => null, clickProbe: () => 'r:new' }, wheelKind: () => 'wheel' };
  vm.runInNewContext(source.slice(start, end) + '\nglobalThis.View = View;', context);
  const view = new context.View(canvas, '');context.views.set(canvas, view);
  view.plot = { chunks: [], box: [0, 0, 100, 100] };view.key = 'board';view.probe = 'r:old';
  const fire = (name, fields = {}) => handlers[name]({ type: name, pointerType: 'mouse', pointerId: 1, button: 0,
    detail: 1, clientX: 20, clientY: 30, deltaY: 10, deltaX: 0, deltaMode: 0, preventDefault() {}, ...fields });
  const click = detail => { fire('pointerdown');fire('pointerup');fire('click', { detail }); };
  return { view, probes, navigations, fire, click, reads: () => reads, resize: (w, h) => resize([{ contentRect: { width: w, height: h } }]),
    defer() { let release;wait = new Promise(resolve => { release = resolve; });return release; } };
}

test('native double-click restores the preceding probe without a timer threshold', async () => {
  const h = harness();
  h.click(1);await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.view.mouseTap, true);
  assert.equal(h.view.probe, 'r:new');
  h.click(2);h.fire('dblclick');
  assert.equal(h.view.probe, 'r:old');
  assert.equal(h.probes.at(-1), 'r:old');
});

test('a new gesture invalidates a probe still waiting for hit information', async () => {
  const h = harness(), release = h.defer();
  h.click(1);h.fire('wheel');release();await Promise.resolve();await Promise.resolve();
  assert.deepEqual(h.probes, []);
});

test('a touch double-click cannot restore a preceding mouse selection', async () => {
  const h = harness();h.click(1);await new Promise(resolve => setImmediate(resolve));
  h.fire('pointerdown', { pointerType: 'touch' });h.fire('pointerup', { pointerType: 'touch' });
  await new Promise(resolve => setImmediate(resolve));h.fire('dblclick');
  assert.equal(h.view.probe, 'r:new');
});

test('schematic back invalidates an asynchronous hit before navigating', async () => {
  const h = harness(), release = h.defer();h.view.side = 'schematic';
  h.click(1);h.fire('pointerdown', { button: 3 });release();
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.probes, []);
  assert.equal(h.navigations.length, 1);assert(h.navigations[0].includes('sch-back'));
});
