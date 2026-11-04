const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');
function harness() {
  const source = fs.readFileSync(require.resolve('../../src/web/plot2d.js'), 'utf8');
  const start = source.indexOf('class View {'), end = source.indexOf('\n}\n', start) + 2;
  const park = source.slice(source.indexOf('function park(v) {'), source.indexOf('\nfunction mountOne('));
  const handlers = {}, probes = [], navigations = [], hits = [], timers = [], places = [], frames = [];let rect = { left: 0, top: 0 };let reads = 0, resize, wait = Promise.resolve();
  const canvas = { get clientWidth() { reads++;return 900; }, get clientHeight() { reads++;return 600; }, isConnected: true,
    addEventListener: (name, fn) => { handlers[name] = fn; }, setPointerCapture() {}, focus() {},
    closest: () => ({ querySelector: selector => ({ click: () => navigations.push(selector) }) }),
    getBoundingClientRect: () => rect, classList: { add() {}, remove() {} } };
  const context = { AbortController, ResizeObserver: class { constructor(fn) { resize = fn; } observe() {} }, requestAnimationFrame: (fn) => { frames.push(fn);return frames.length; }, cancelAnimationFrame: (id) => { if (id) frames[id - 1] = null; },
    views: new Map(), left: new Map(), componentSources: new Map(), sheeted: null, need: () => wait,
    hitComponent: () => ({ id: 'U1' }), selectComponent() {},
    probed: value => probes.push(value), under: (_, x, y) => { hits.push([x,y]);return []; }, partAt: () => null,
    groupsOf: () => [], P: { pickBoard: () => null, pickSheet: () => null, clickProbe: () => 'r:new' }, wheelKind: () => 'wheel',
    window: { devicePixelRatio: 1, matchMedia: () => ({ addEventListener() {}, removeEventListener() {} }) }, coarse: null, viewDpr: () => 1, inspectCard() {}, card: null, phone: () => false,
    setTimeout: (fn) => { timers.push(fn);return timers.length; }, clearTimeout: (id) => { if (id) timers[id - 1] = null; }, performance, navigator: {} };
  context.placed = place => places.push(place);
  vm.runInNewContext(source.slice(start, end) + '\n' + park + '\nglobalThis.View = View;', context);
  const view = new context.View(canvas, '');context.views.set(canvas, view);
  view.plot = { chunks: [], box: [0, 0, 100, 100] };view.key = 'board';view.probe = 'r:old';
  const fire = (name, fields = {}) => handlers[name]({ type: name, pointerType: 'mouse', pointerId: 1, button: 0,
    detail: 1, clientX: 20, clientY: 30, deltaY: 10, deltaX: 0, deltaMode: 0, preventDefault() {}, ...fields });
  const click = detail => { fire('pointerdown');fire('pointerup');fire('click', { detail }); };
  return { view, probes, timers, places, frames, navigations, hits, fire, click, inspect(fn) { context.componentSources.set(view.key, { data: {}, onSelect: fn }); }, setRect(value) { rect = value; }, park: () => context.park(view), saved: () => context.left.get(view.key), reads: () => reads, resize: (w, h) => resize([{ contentRect: { width: w, height: h } }]),
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

test('Fit replaces an older parked pan before returning to the board', () => {
  const h = harness();
  h.view.fit();h.view.ox += 120;h.view.oy += 110;h.view.touched = true;
  h.park();assert(h.saved(), 'leaving a panned board keeps its view');
  h.view.fit();h.park();
  assert.equal(h.saved(), undefined, 'leaving after Fit cannot restore the older cropped view');
});

test('resizing a moved plot keeps its board centre and zoom', () => {
  const h = harness();h.view.fit();h.view.ox += 120;h.view.oy += 110;h.view.touched = true;
  const { scale, ox, oy } = h.view;
  const centre = [(900 / 2 - ox) / scale, (600 / 2 - oy) / scale];
  h.resize(420, 260);
  assert.equal(h.view.scale, scale);
  assert.equal((420 / 2 - h.view.ox) / scale, centre[0]);
  assert.equal((260 / 2 - h.view.oy) / scale, centre[1]);
  h.resize(900, 600);
  assert.equal(h.view.ox, ox);assert.equal(h.view.oy, oy);
});

test('the PCB pane restores the board centre from a larger canvas', () => {
  const h = harness();h.view.fit();h.view.ox += 120;h.view.oy += 110;h.view.touched = true;
  h.park();const saved = h.saved(), scale = h.view.scale;
  h.view.plot = null;h.view.back = saved;h.view.viewport = { width: 420, height: 260 };
  h.view.show({ chunks: [], box: [0, 0, 100, 100] });
  assert.equal(h.view.scale, scale);
  assert.equal((420 / 2 - h.view.ox) / scale, saved.cx);
  assert.equal((260 / 2 - h.view.oy) / scale, saved.cy);
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

test('wheel zoom uses cached viewport geometry and refreshes it on resize', () => {
  const h = harness(), initial = h.reads();
  for (let i = 0; i < 100; i++) h.fire('wheel');
  assert.equal(h.reads(), initial);
  h.resize(1200, 800);
  assert.equal(h.view.viewport.width, 1200);
  assert.equal(h.view.zoomFactor(0), 0.9 * 8 / 4 / h.view.scale);
});


test('opening the component inspector keeps the net hit at the clicked board point', async () => {
  const h = harness();h.view.scale = 1;h.view.ox = 0;h.view.oy = 0;
  h.inspect(() => { h.setRect({ left: 100, top: 200 });h.view.ox = 400;h.view.oy = 500; });
  h.click(1);await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.hits, [[20,30],[20,30]], 'inspection can resize and move the PCB before its net information arrives');
});

test('a touch double tap gives back the first tap\'s probe, zooms in, then fits again', async () => {
  const h = harness();
  h.view.fit();
  const fitted = h.view.scale;
  const tap = (t) => { h.fire('pointerdown', { pointerType: 'touch', timeStamp: t });h.fire('pointerup', { pointerType: 'touch', timeStamp: t + 40 }); };
  tap(1000);await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.view.probe, 'r:new');
  tap(1200);
  assert.equal(h.view.probe, 'r:old');
  assert(h.view.scale > fitted * 2, 'the second tap zooms in about the finger');
  h.fire('dblclick', { timeStamp: 1300 });
  assert(h.view.scale > fitted * 2, "the browser's own dblclick after it does not fit again");
  tap(3000);tap(3200);
  assert.equal(h.view.scale, fitted, 'zoomed in, a double tap fits the plot');
});

test('a finger held still asks to comment there once, and is no tap', async () => {
  const h = harness();
  h.view.fit();
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 1000 });
  const held = h.timers.filter(Boolean).at(-1);
  assert(held, 'a touch press starts the long-press timer');
  held();
  h.timers.filter(Boolean).at(-1)();
  assert.equal(h.places.length, 0, 'armed, it waits for the lift');
  h.fire('contextmenu');
  assert.equal(h.places.length, 0, "the browser's own contextmenu is ignored");
  h.fire('pointerup', { pointerType: 'touch', timeStamp: 1600 });
  assert.equal(h.places.length, 1, 'lifted where it came down, the place goes to the page (rv-note)');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.probes.length, 0, 'the press does not probe');
  // a slow drag whose moves never came: armed, but lifted far away
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 2000, clientX: 20, clientY: 30 });
  h.timers.filter(Boolean).at(-1)(); h.timers.filter(Boolean).at(-1)();
  h.fire('pointerup', { pointerType: 'touch', timeStamp: 3000, clientX: 170, clientY: 30 });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.places.length, 1, 'a drag lifted far away does not comment');
  assert.equal(h.probes.length, 0, 'nor probe');
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 3000 });
  const moved = h.timers.filter(Boolean).at(-1);
  h.fire('pointermove', { pointerType: 'touch', clientX: 60 });
  assert.equal(h.timers.includes(moved), false, 'moving the finger cancels it');
});

test('a one-finger pan flicked off coasts to a stop, and a finger down stops it', () => {
  const h = harness();
  h.view.fit();
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 1000, clientX: 100, clientY: 100 });
  for (let i = 1; i <= 5; i++) h.fire('pointermove', { pointerType: 'touch', timeStamp: 1000 + i * 16, clientX: 100 + i * 20, clientY: 100 });
  const ox = h.view.ox;
  h.fire('pointerup', { pointerType: 'touch', timeStamp: 1090, clientX: 200, clientY: 100 });
  const step = h.frames.filter(Boolean).at(-1);
  assert(step, 'a flick starts the coast');
  h.frames.fill(null);
  step(performance.now() + 16);
  assert(h.view.ox > ox, 'the view keeps moving the way the finger went');
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 1200, clientX: 150, clientY: 100 });
  const held = h.view.ox;
  for (const f of h.frames) if (f === step) f(performance.now() + 32);
  assert.equal(h.view.ox, held, 'a finger down stops it');
  const probes = h.probes.length;
  h.fire('pointerup', { pointerType: 'touch', timeStamp: 1240, clientX: 150, clientY: 100 });
  assert.equal(h.view.tap, null, 'and that press is no tap');
  assert.equal(h.probes.length, probes);
  h.fire('pointerdown', { pointerType: 'mouse', timeStamp: 2000, clientX: 100, clientY: 100 });
  for (let i = 1; i <= 5; i++) h.fire('pointermove', { pointerType: 'mouse', timeStamp: 2000 + i * 16, clientX: 100 + i * 20, clientY: 100 });
  const n = h.frames.filter(Boolean).length;
  h.fire('pointerup', { pointerType: 'mouse', timeStamp: 2090, clientX: 200, clientY: 100 });
  assert.equal(h.frames.filter(Boolean).length, n, 'a mouse drag does not coast');
});

test("a touch's system long press (Android's, also in a slow drag) never comments; a mouse right click does", () => {
  const h = harness();
  h.view.fit();
  h.fire('pointerdown', { pointerType: 'touch', timeStamp: 1000, clientX: 100, clientY: 100 });
  h.fire('pointermove', { pointerType: 'touch', timeStamp: 1100, clientX: 140, clientY: 100 });
  h.fire('contextmenu', { clientX: 140, clientY: 100 });
  assert.equal(h.places.length, 0, 'the system long press of a dragging finger is ignored');
  h.fire('pointerup', { pointerType: 'touch', timeStamp: 1600, clientX: 140, clientY: 100 });
  h.fire('pointerdown', { pointerType: 'mouse', button: 2, timeStamp: 3000 });
  h.fire('contextmenu');
  assert.equal(h.places.length, 1, 'a right click asks to comment there');
});
