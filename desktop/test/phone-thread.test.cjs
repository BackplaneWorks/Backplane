const { test } = require('node:test'), assert = require('node:assert/strict');

// A stand-in page: html carries the phone shell's class or not; listeners
// and one ResizeObserver are captured so a test can drive them.
const page = (phone, mic = true) => {
  const on = {}, observers = [], classes = new Set(phone ? ['phone'] : []);
  const timeline = { id: 'timeline', scrollTop: 0, scrollHeight: 2000, clientHeight: 500 };
  const doc = {
    documentElement: { classList: { contains: c => classes.has(c), add: c => classes.add(c) } },
    activeElement: null,
    getElementById: id => id === 'timeline' ? timeline : null,
    addEventListener: (type, f) => { (on[type] ||= []).push(f); },
  };
  class RO { constructor(f) { this.f = f; observers.push(this); } observe(t) { this.t = t; } disconnect() { this.t = null; } }
  const fire = (type, target = timeline) => (on[type] || []).forEach(f => f({ target }));
  const resize = h => { timeline.clientHeight = h; for (const o of observers) if (o.t) o.f([{ target: o.t }]); };
  return { doc, win: { ResizeObserver: RO, navigator: { mediaDevices: mic ? { getUserMedia() {} } : undefined } }, timeline, fire, resize, classes };
};

test('Return in the draft is a new line on a phone only, Send still sends', async () => {
  const { phoneKeepsReturn } = await import('../../src/web/phone-thread.js');
  const composer = { id: 'composer' }, key = { ctrlKey: false, metaKey: false };
  assert.equal(phoneKeepsReturn(page(true).doc, composer, 'send', key), true);
  assert.equal(phoneKeepsReturn(page(false).doc, composer, 'send', key), false);
  assert.equal(phoneKeepsReturn(page(true).doc, composer, 'skill', key), false);
  assert.equal(phoneKeepsReturn(page(true).doc, composer, 'send', { ctrlKey: true, metaKey: false }), false);
  assert.equal(phoneKeepsReturn(page(true).doc, { id: 'needs:q1' }, 'send', key), false);
});

test('the keyboard keeps the end in view on a phone, and the bottom edge elsewhere', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  // at the end: the keyboard shrinks it, it stays at the end; a later scroll
  // no finger made (a view reply) is put back at the end
  const p = page(true);
  installPhoneThread({ document: p.doc, window: p.win });
  p.timeline.scrollTop = 1500; p.fire('touchstart'); p.doc.activeElement = { id: 'composer' };
  p.resize(160); assert.equal(p.timeline.scrollTop, 1840);
  p.fire('scroll'); p.timeline.scrollTop = 1500; p.fire('scroll'); assert.equal(p.timeline.scrollTop, 1840);
  // a finger's drag reads back, and that is kept
  p.fire('touchmove'); p.timeline.scrollTop = 1200; p.fire('scroll'); assert.equal(p.timeline.scrollTop, 1200);
  // reading back, the keyboard going moves the top by the height
  p.resize(500); assert.equal(p.timeline.scrollTop, 860);
  // not a phone: nothing moves
  const d = page(false);
  installPhoneThread({ document: d.doc, window: d.win });
  d.timeline.scrollTop = 1500; d.fire('touchstart'); d.resize(160); assert.equal(d.timeline.scrollTop, 1500);
});

test('a page that cannot record says so (the composer hides its mic on a phone)', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const plain = page(true, false), secure = page(true, true);
  installPhoneThread({ document: plain.doc, window: plain.win });
  installPhoneThread({ document: secure.doc, window: secure.win });
  assert.equal(plain.classes.has('ph-nomic'), true);
  assert.equal(secure.classes.has('ph-nomic'), false);
});
