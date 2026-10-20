import {test} from 'node:test';
import assert from 'node:assert/strict';
import {installBrowserInput} from '../../src/web/browser-input.js';

// A small DOM event harness: exercises the transport and lifecycle with browser
// event sequences. This does not claim real OS IME/clipboard/Electron acceptance.
class Events {
  listeners = new Map();
  addEventListener(type, fn, options) {
    this.listeners.set(type, [...(this.listeners.get(type) || []), {fn, options}]);
  }
  removeEventListener(type, fn) {
    this.listeners.set(type, (this.listeners.get(type) || []).filter(v => v.fn !== fn));
  }
  emit(type, fields = {}) {
    const e = {type, target: this, button: 0, detail: 1, clientX: 150, clientY: 100,
      key: '', deltaX: 0, deltaY: 0, deltaMode: 0, getModifierState: () => false,
      preventDefault() { this.prevented = true; },
      stopImmediatePropagation() { this.stopped = true; }, ...fields};
    for (const {fn} of this.listeners.get(type) || []) { fn(e); if (e.stopped) break; }
    return e;
  }
}
class Element extends Events {
  attrs = new Map(); style = {}; isConnected = true; value = ''; children = [];
  constructor(doc, frame = false) { super(); this.ownerDocument = doc; this.frame = frame; }
  setAttribute(k, v) { this.attrs.set(k, v); }
  getAttribute(k) { return this.attrs.get(k) ?? null; }
  appendChild(el) { this.children.push(el); el.parent = this; }
  contains(el) { return el.isConnected && this.children.some(c => c === el || c.contains(el)); }
  closest(selector) { return selector === 'img[data-browser-input]' && this.frame ? this : null; }
  getBoundingClientRect() { return {left: 50, top: 50, width: 200, height: 100}; }
  focus() { this.ownerDocument.activeElement = this; this.ownerDocument.emit('focusin', {target: this}); }
  blur() { if (this.ownerDocument.activeElement === this) this.ownerDocument.activeElement = null; }
  remove() { this.isConnected = false; this.parent.children = this.parent.children.filter(x => x !== this); }
}
class Document extends Events {
  defaultView = new Events(); hidden = false; activeElement = null;
  constructor() { super(); this.body = new Element(this); this.frames = []; }
  createElement() { return new Element(this); }
  contains(el) { return el.isConnected && (this.frames.includes(el) || this.body.children.includes(el)); }
}
function fixture(extra = {}) {
  const doc = new Document(), calls = [], unsupported = [];
  const frame = new Element(doc, true); doc.frames.push(frame);
  frame.binding = {page: 'thread-a', tabId: 'tab-a', generation: 1};
  let current = {...frame.binding}, driving = true;
  const adapter = installBrowserInput({root: doc,
    getBinding: el => el.binding,
    isCurrent: b => driving && b.page === current.page && b.tabId === current.tabId && b.generation === current.generation,
    dispatch: (action, value, binding) => calls.push({action, value, binding}),
    onUnsupported: kind => unsupported.push(kind), ...extra});
  const event = (type, fields = {}) => doc.emit(type, {target: adapter.element, ...fields});
  function focus() { frame.focus(); }
  function input(value, fields = {}) { adapter.element.value = value; return event('input', fields); }
  function switchTo(binding = {page: 'thread-b', tabId: 'tab-b', generation: 1}) {
    current = {...binding}; frame.binding = {...binding};
  }
  const values = action => calls.filter(v => v.action === action).map(v => v.value);
  return {doc, frame, adapter, calls, unsupported, event, focus, input, switchTo, values,
    setDriving: value => { driving = value; },
    mouse: (type, fields = {}) => doc.emit(type, {target: frame, ...fields})};
}

test('creates a focusable editing surface; ordinary text and emoji use DOM input', () => {
  const f = fixture(); f.focus();
  assert.equal(f.doc.activeElement, f.adapter.element);
  assert.equal(f.adapter.element.tabIndex, -1);
  assert.notEqual(f.adapter.element.style.display, 'none');
  f.input('a'); f.input('λ🙂');
  assert.deepEqual(f.values('bb-type'), ['a', 'λ🙂']);
  assert.equal(f.adapter.element.value, '');
  assert(Object.isFrozen(f.calls[0].binding));
});
test('container-scoped installation mounts the proxy within the event root', () => {
  const doc = new Document(), container = new Element(doc), frame = new Element(doc, true), calls = [];
  container.appendChild(frame);
  const adapter = installBrowserInput({root: container, dispatch: (...a) => calls.push(a),
    getBinding: () => ({page: 'a', tabId: 't'}), isCurrent: () => true});
  assert.equal(adapter.element.parent, container);
  container.emit('focusin', {target: frame});
  adapter.element.value = 'container text'; container.emit('input', {target: adapter.element});
  assert.equal(calls[0][1], 'container text');
  adapter.dispose(); assert(!container.children.includes(adapter.element));
});
test('IME preedit is local, commits once, and trailing input does not duplicate', () => {
  const f = fixture(); f.focus(); f.event('compositionstart');
  f.input('にほん', {isComposing: true}); f.input('日本語', {isComposing: true});
  assert.equal(f.calls.length, 0);
  f.event('compositionend', {data: '日本語'});
  f.event('input', {data: '日本語', inputType: 'insertFromComposition', isComposing: false});
  assert.deepEqual(f.values('bb-type'), ['日本語']);
});
test('IME final input before compositionend and cancellation are handled', () => {
  const f = fixture(); f.focus(); f.event('compositionstart');
  f.input('日本', {isComposing: false});
  assert.equal(f.calls.length, 0);
  f.event('compositionend', {data: '日本'});
  f.event('compositionstart'); f.input('未確定', {isComposing: true});
  f.event('compositionend', {data: ''});
  assert.deepEqual(f.values('bb-type'), ['日本']);
  assert.equal(f.adapter.element.value, '');
});
test('stale owner, active tab, navigation generation, and drive permission reject input', () => {
  for (const change of [f => f.switchTo(), f => f.switchTo({page: 'thread-a', tabId: 'tab-b', generation: 1}),
    f => f.switchTo({page: 'thread-a', tabId: 'tab-a', generation: 2}), f => f.setDriving(false)]) {
    const f = fixture(); f.focus(); f.event('compositionstart'); f.input('private preedit', {isComposing: true});
    change(f); f.event('compositionend', {data: 'private preedit'}); f.input('stale');
    assert.equal(f.calls.length, 0);
  }
});
test('refresh drops old editing surface instead of retargeting it to new owner', () => {
  const f = fixture(); f.focus(); f.adapter.element.value = 'old'; f.switchTo(); f.adapter.refresh();
  f.input('late'); assert.equal(f.calls.length, 0); assert.equal(f.adapter.element.value, '');
  f.focus(); f.input('new');
  assert.equal(f.calls[0].binding.page, 'thread-b');
});
test('text paste uses event clipboard data; files are explicitly unsupported', () => {
  const f = fixture(); f.focus();
  const e = f.event('paste', {clipboardData: {getData: type => type === 'text/plain' ? '日本語\n🙂' : '', files: []}});
  assert(e.prevented);
  assert.deepEqual(f.values('bb-type'), ['日本語\n🙂']);
  f.event('paste', {clipboardData: {getData: () => '', files: [{}]}});
  assert(f.unsupported.includes('file-paste'));
  assert.equal(f.calls.length, 1);
});
test('copy requires synchronous authorized selection cache; never copies proxy/preedit', () => {
  const f = fixture(); f.focus(); f.adapter.element.value = 'must not copy';
  const writes = [];
  f.event('copy', {clipboardData: {setData: (...args) => writes.push(args)}});
  assert.deepEqual(writes, []); assert(f.unsupported.includes('copy-selection'));
  const g = fixture({getCopyText: b => b.page === 'thread-a' ? 'remote selection' : null}); g.focus();
  g.event('copy', {clipboardData: {setData: (...args) => writes.push(args)}});
  assert.deepEqual(writes, [['text/plain', 'remote selection']]);
  const h = fixture({getCopyText: async () => 'not synchronous'}); h.focus();
  h.event('copy', {clipboardData: {setData: (...args) => writes.push(args)}});
  assert.equal(writes.length, 1);
  assert(g.event('cut').prevented); assert(g.unsupported.includes('cut-selection'));
});
test('copy revalidates after reading selection and rejects a selection-triggered switch', () => {
  let f; f = fixture({getCopyText: () => { f.switchTo(); return 'old selection'; }}); f.focus();
  let copied = false; f.event('copy', {clipboardData: {setData: () => { copied = true; }}});
  assert.equal(copied, false);
});
test('local clipboard shortcuts are not forwarded; edit and virtual keys are', () => {
  const f = fixture(); f.focus();
  assert(!f.event('keydown', {key: 'v', metaKey: true}).prevented);
  assert(!f.event('keydown', {key: 'c', ctrlKey: true}).prevented);
  assert(f.event('keydown', {key: 'Tab', shiftKey: true}).prevented);
  f.event('keydown', {key: 'a', metaKey: true});
  f.event('beforeinput', {inputType: 'deleteContentBackward'});
  f.event('keydown', {key: 'Enter', isComposing: true});
  assert.deepEqual(f.values('bb-key'), ['Shift+Tab', 'Control+a', 'Backspace']);
});
test('legacy click and vertical wheel stay compatible without rich transports', () => {
  const f = fixture(); f.mouse('mousedown'); f.mouse('mouseup'); f.mouse('click');
  f.mouse('wheel', {deltaY: 3, deltaMode: 1});
  assert.deepEqual(f.calls.map(v => [v.action, v.value]), [['bb-click', '500 500'], ['bb-scroll', '48']]);
  f.mouse('auxclick', {button: 1}); f.mouse('wheel', {deltaX: 10});
  assert(f.unsupported.includes('rich-pointer')); assert(f.unsupported.includes('horizontal-wheel'));
  assert.equal(f.calls.length, 2);
});
test('same DOM image reused for a new owner between down/up cannot receive old click', () => {
  const f = fixture(); f.mouse('mousedown'); f.switchTo(); f.mouse('mouseup'); f.mouse('click');
  assert.equal(f.calls.length, 0);
});
test('rich mouse forwards ordered drags including up outside image; buttons remain distinct', () => {
  for (const button of [0, 1, 2]) {
    const f = fixture({pointerAction: 'bb-pointer'});
    f.mouse('mousedown', {button});
    f.doc.emit('mousemove', {target: f.doc.body, clientX: 350, clientY: 200});
    f.doc.emit('mouseup', {target: f.doc.body, button, clientX: 350, clientY: 200});
    const ps = f.values('bb-pointer').map(JSON.parse);
    assert.deepEqual(ps.map(v => v.type), ['down', 'move', 'up']);
    assert.equal(ps[0].button, ['left', 'middle', 'right'][button]);
    assert.equal(ps[0].gesture, ps[2].gesture);
    assert.deepEqual(ps.map(v => v.sequence), [1, 2, 3]);
    assert.equal(ps[2].x, 1000); assert.equal(ps[2].y, 1000);
    assert(ps.every(v => v.page === 'thread-a' && v.tabId === 'tab-a' && v.unit === 'per-mille'));
  }
});
test('double click uses OS down/up counts without an extra click or duplicate transport', () => {
  const f = fixture({pointerAction: 'bb-pointer'});
  for (const detail of [1, 2]) {
    f.mouse('mousedown', {detail}); f.mouse('mouseup', {detail}); f.mouse('click', {detail});
  }
  f.mouse('dblclick', {detail: 2});
  const ps = f.values('bb-pointer').map(JSON.parse);
  assert.deepEqual(ps.map(p => [p.type, p.clickCount]), [['down', 1], ['up', 1], ['down', 2], ['up', 2]]);
  assert.deepEqual(f.values('bb-click'), []);
});
test('stale drag cannot send an up or move to the switched tab; blur cancels current gesture', () => {
  const f = fixture({pointerAction: 'bb-pointer'}); f.mouse('mousedown'); f.switchTo();
  f.mouse('mousemove'); f.mouse('mouseup');
  assert.deepEqual(f.values('bb-pointer').map(x => JSON.parse(x).type), ['down']);
  assert(f.unsupported.includes('stale-pointer'));
  const g = fixture({pointerAction: 'bb-pointer'}); g.mouse('mousedown'); g.doc.defaultView.emit('blur');
  assert.deepEqual(g.values('bb-pointer').map(x => JSON.parse(x).type), ['down', 'cancel']);
});
test('rich wheel carries both axes, normalized pointer and bounded line/page deltas', () => {
  const f = fixture({wheelAction: 'bb-wheel'});
  const e = f.mouse('wheel', {deltaX: 2, deltaY: -3, deltaMode: 1});
  assert(e.prevented);
  const w = JSON.parse(f.values('bb-wheel')[0]);
  assert.equal(w.deltaUnit, 'pixel');
  assert.deepEqual([w.dx, w.dy, w.x, w.y], [32, -48, 500, 500]);
  f.mouse('wheel', {deltaX: 50, deltaY: -50, deltaMode: 2});
  assert.deepEqual(f.values('bb-wheel').map(x => [JSON.parse(x).dx, JSON.parse(x).dy]), [[32, -48], [600, -600]]);
  assert.equal(f.doc.listeners.get('wheel')[0].options.passive, false);
});
test('unrelated UI events are untouched; detached image and zero geometry are rejected', () => {
  const f = fixture();
  assert(!f.doc.emit('keydown', {target: f.doc.body, key: 'Enter'}).prevented);
  f.frame.getBoundingClientRect = () => ({left: 0, top: 0, width: 0, height: 0});
  f.mouse('click'); assert.equal(f.calls.length, 0);
  f.frame.isConnected = false; f.mouse('click'); assert.equal(f.calls.length, 0);
});
test('dispose cancels owned current gesture and removes listeners/proxy exactly once', () => {
  const f = fixture({pointerAction: 'bb-pointer'}); f.mouse('mousedown');
  f.adapter.dispose(); f.adapter.dispose();
  assert.deepEqual(f.values('bb-pointer').map(x => JSON.parse(x).type), ['down', 'cancel']);
  assert.equal(f.adapter.element.isConnected, false);
  assert([...f.doc.listeners.values()].every(v => v.length === 0));
  f.mouse('click'); f.input('late'); assert.equal(f.calls.length, 2);
});
