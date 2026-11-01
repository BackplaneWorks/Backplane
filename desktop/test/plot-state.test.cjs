const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');

// Run the production mount, cache and camera code with browser scheduling
// under our control. No plot transport or rendering algorithm is replaced.
function harness() {
  const source = fs.readFileSync(require.resolve('../../src/web/plot2d.js'), 'utf8')
    .replace(/^import .*;\n/gm, '').replace(/^export /gm, '');
  const frames = new Map(), timers = new Map(), observers = [];
  let serial = 0, draws = 0;
  const context = {
    AbortController,
    performance: { now: () => 100 }, setTimeout() {},
    requestAnimationFrame(fn) { const id = ++serial; frames.set(id, fn); return id; },
    setInterval(fn) { const id = ++serial; timers.set(id, fn); return id; },
    clearInterval(id) { timers.delete(id); },
    ResizeObserver: class {
      constructor(fn) { this.fn = fn; observers.push(this); }
      observe() { this.active = true; }
      disconnect() { this.active = false; }
    },
    window: { devicePixelRatio: 1 },
    document: { documentElement: { lang: 'en' }, addEventListener() {}, querySelector: () => null, querySelectorAll: () => [] },
    getComputedStyle: () => ({ fontFamily: 'monospace' }),
    App: { plot_stats: () => '', plot_loading: () => '' }, P: {}, wheelKind: () => 'wheel',
  };
  vm.createContext(context);
  vm.runInContext(source + '\nglobalThis.plot = { mount, got, views };', context);
  function canvas(scope = 'A', key = '0|project|board', width = 900, height = 600) {
    const handlers = {}, g = { setTransform() {}, fillRect() {}, fillText() { draws++; }, measureText: () => ({ width: 0 }) };
    return { dataset: { scope, key }, clientWidth: width, clientHeight: height, width, height, isConnected: true,
      parentElement: { querySelectorAll: () => [] }, closest: () => null,
      classList: { add() {}, remove() {} }, focus() {}, setPointerCapture() {},
      getBoundingClientRect: () => ({ left: 0, top: 0 }), getContext: () => g,
      addEventListener: (type, fn, options) => {
        handlers[type] = fn;
        options?.signal?.addEventListener('abort', () => { if (handlers[type] === fn) delete handlers[type]; }, { once: true });
      }, handlers };
  }
  return { ...context.plot, canvas, frames, timers, observers, draws: () => draws,
    flush() { const pending = [...frames.values()]; frames.clear(); for (const fn of pending) fn(); },
    receive(c) { context.plot.got({ key: c.dataset.key, cs: [], box: [0, 0, 100, 100] }); },
    move(c) { c.handlers.wheel({ preventDefault() {}, deltaX: 0, deltaY: -300, deltaMode: 0, timeStamp: 1, clientX: 300, clientY: 200 }); },
  };
}

const pose = v => [v.scale, (v.viewport.width / 2 - v.ox) / v.scale, (v.viewport.height / 2 - v.oy) / v.scale];

test('threads showing the same board keep independent cameras on a retained canvas', () => {
  const h = harness(), c = h.canvas(); h.mount([c]); h.receive(c);
  const v = h.views.get(c), fitted = pose(v); h.move(c); const moved = pose(v);
  assert.notDeepEqual(moved, fitted);
  c.dataset.scope = 'B'; h.mount([c]);
  assert.deepEqual(pose(v), fitted, 'another thread starts fitted');
  v.fit(); c.dataset.scope = 'A'; h.mount([c]);
  assert.deepEqual(pose(v), moved, 'fitting B cannot erase A');
  v.fit(); c.dataset.scope = 'B'; h.mount([c]); c.dataset.scope = 'A'; h.mount([c]);
  assert.deepEqual(pose(v), fitted, 'Fit replaces only this thread’s saved pan');
});

test('a thread restores its board centre through Split, Files and another thread', () => {
  const h = harness(), c = h.canvas(); h.mount([c]); h.receive(c); h.move(c);
  const moved = pose(h.views.get(c)); h.mount([]);
  const split = h.canvas('A', c.dataset.key, 420, 260); h.mount([split]);
  assert.deepEqual(pose(h.views.get(split)), moved, 'smaller pane keeps board centre and scale');
  h.mount([]); const other = h.canvas('B', c.dataset.key); h.mount([other]);
  assert.notDeepEqual(pose(h.views.get(other)), moved);
  h.mount([]); const returned = h.canvas('A', c.dataset.key); h.mount([returned]);
  assert.deepEqual(pose(h.views.get(returned)), moved);
});

test('leaving a loading board stops its timer, observer and queued draws', () => {
  const h = harness(), c = h.canvas(); h.mount([c]); h.flush();
  assert.equal(h.timers.size, 1, 'loading view counts seconds');
  const tick = [...h.timers.values()][0], resize = h.observers[0];
  tick(); h.mount([]); c.isConnected = false;
  const before = h.draws(); h.flush(); tick(); resize.fn([{ contentRect: { width: 0, height: 0 } }]); h.flush();
  assert.equal(h.timers.size, 0);
  assert.equal(resize.active, false);
  assert.equal(h.draws(), before, 'already queued work cannot restart removed loading views');
});

test('leaving before the first frame cannot create a detached loading timer', () => {
  const h = harness(), c = h.canvas(); h.mount([c]); h.mount([]); h.flush();
  assert.equal(h.draws(), 0);
  assert.equal(h.timers.size, 0);
});

test('a remounted canvas has only its current view’s gesture handlers', () => {
  const h = harness(), c = h.canvas(); h.mount([c]); h.receive(c);
  const old = h.views.get(c), before = old.scale; h.mount([]);
  assert.equal(Object.keys(c.handlers).length, 0, 'removed view releases canvas listeners');
  h.mount([c]); h.move(c);
  assert.equal(old.scale, before);
  assert.notEqual(h.views.get(c).scale, before);
});
