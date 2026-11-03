const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');

// Run the production layout listeners with measured panes and the existing
// Bend clamp boundary. The desktop check exercises the real Bend export.
function harness({project='',hub='hub-A',kept=new Map(),home=false} = {}) {
  const listeners = {}, values = {}, removed = []; let width = 600, written = false, afterWrite = 0, attributes = 0;
  const rect = value => { if (written) afterWrite++;return value; };
  const edge = { parentElement: { getBoundingClientRect: () => rect({ width }) },
    getAttribute: key => values[key],
    setAttribute: (key, value) => { attributes++;values[key] = value; },
    closest: selector => selector === '.viewer-split' ? edge : null };
  const root = { dataset: {}, style: {
    setProperty: (_key, value) => { width = Number.parseFloat(value);written = true; }, removeProperty(name) {removed.push(name);} } };
  const doc = { documentElement: root,
    querySelector: selector => selector === '.viewer-split' ? edge : selector === '.app' ? { getBoundingClientRect: () => rect({ width: 1400 }) }
      : selector === '.app.panel' ? {} : selector === '#viewer[data-split-project]' && project ? {dataset:{splitProject:project}}
      : selector === '.app > main.project-home' && home ? {} : selector === '.workspace-thread > .workspace-bar' ? { getBoundingClientRect: () => rect({ width: 76 }) } : null,
    addEventListener: (key, fn) => { (listeners[key] ??= []).push(fn); } };
  const context = { document: doc, localStorage: { getItem: key => kept.get(key), setItem: (key, value) => kept.set(key, value), removeItem: key => kept.delete(key) },
    window: { matchMedia: () => ({ matches: true }), addEventListener() {} },
    App: { panes_viewer: (_w, _side, _strip, _rail, drag) => BigInt(Math.max(320, Math.min(700, Number(drag) || 595))), panes_fold: () => false, panes_drawer: () => false },
    thin: { hub, session: { sel: 'test' } }, ui: {}, composerGeometry() {} };
  const source = fs.readFileSync(require.resolve('../../src/web/host.js'), 'utf8');
  vm.runInNewContext(source.slice(source.indexOf('const VIEWER_W ='), source.indexOf('// Socket\n')), context);
  return { values, kept, removed, context, afterWrite: () => afterWrite, attributes: () => attributes, layout() { written = false;context.panes(); }, width: () => width, press(key, fields = {}) {
    written = false;
    const event = { key, target: edge, preventDefault() { this.defaultPrevented = true; }, ...fields };
    for (const fn of listeners.keydown) fn(event);
    return event;
  } };
}

test('the divider moves its left edge, keeps the width and exposes its bounds', () => {
  const h = harness();
  assert.equal(h.press('ArrowLeft').defaultPrevented, true); assert.equal(h.width(), 616);
  h.press('ArrowRight', { shiftKey: true }); assert.equal(h.width(), 552);
  assert.equal(h.kept.get('backplane-viewer-w'), '552');
  assert.deepEqual(h.values, { 'aria-valuemin': '320', 'aria-valuemax': '700', 'aria-valuenow': '552' });
  for (let i = 0; i < 20; i++) h.press('ArrowLeft', { shiftKey: true });
  assert.equal(h.width(), 700);
  for (let i = 0; i < 20; i++) h.press('ArrowRight', { shiftKey: true });
  assert.equal(h.width(), 320);
  h.press('Home'); assert.equal(h.width(), 595); assert.equal(h.kept.size, 0);
});

test('the divider leaves modified, composing and unrelated keys alone', () => {
  const h = harness();
  for (const fields of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { isComposing: true }, { keyCode: 229 }]) {
    assert.equal(h.press('ArrowLeft', fields).defaultPrevented, undefined);
  }
  assert.equal(h.press('ArrowDown').defaultPrevented, undefined);
  assert.equal(h.width(), 600); assert.equal(h.kept.size, 0);
});

test('layout and divider updates reuse geometry and leave unchanged ARIA alone', () => {
  const h = harness();
  h.layout();
  assert.equal(h.afterWrite(), 0);
  const writes = h.attributes();
  h.layout();
  assert.equal(h.attributes(), writes);
  h.press('ArrowLeft');
  assert.equal(h.afterWrite(), 0);
});

test('outer viewer widths are remembered per project and hub',()=>{
  const a=harness({project:'A'});a.press('ArrowLeft');
  const b=harness({project:'B',kept:a.kept});b.layout();assert.equal(b.width(),595);
  const restored=harness({project:'A',kept:a.kept});restored.layout();assert.equal(restored.width(),616);
  const other=harness({project:'A',hub:'hub-B',kept:a.kept});other.layout();assert.equal(other.width(),595);
});

test('a project home uses its available main area without reserving an external viewer or folding its sidebar',()=>{
  const h=harness({home:true});
  h.context.App.panes_fold=h.context.App.panes_drawer=h.context.App.panes_viewer=()=>{throw Error('external viewer geometry used for embedded home');};
  h.context.document.documentElement.dataset.fold='auto';h.layout();
  assert.equal(h.context.document.documentElement.dataset.fold,undefined);assert.deepEqual(h.removed,['--viewer-w']);
});
