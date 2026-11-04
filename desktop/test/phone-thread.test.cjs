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
  const inComposer = { closest: s => s === '.composer' ? {} : null };
  const fire = (type, target = timeline) => (on[type] || []).forEach(f => f({ target }));
  const resize = h => { timeline.clientHeight = h; for (const o of observers) if (o.t) o.f([{ target: o.t }]); };
  const won = {};
  return { on, won, inComposer, doc, win: { addEventListener: (type, f) => { (won[type] ||= []).push(f); }, ResizeObserver: RO, navigator: { mediaDevices: mic ? { getUserMedia() {} } : undefined }, setTimeout: f => { f(); return 1; } }, timeline, fire, resize, classes };
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
  p.timeline.scrollTop = 1470; p.fire('touchstart', p.inComposer); p.doc.activeElement = { id: 'composer' };
  p.resize(160); assert.equal(p.timeline.scrollTop, 1840);
  p.fire('scroll'); p.timeline.scrollTop = 1500; p.fire('scroll'); assert.equal(p.timeline.scrollTop, 1840);
  // a tap elsewhere (a link going to an earlier message) lets it go
  const q = page(true);
  installPhoneThread({ document: q.doc, window: q.win });
  q.timeline.scrollTop = 1500; q.fire('touchstart', q.inComposer); q.doc.activeElement = { id: 'composer' }; q.resize(160);
  q.fire('touchstart', { closest: () => null }); q.timeline.scrollTop = 900; q.fire('scroll'); assert.equal(q.timeline.scrollTop, 900);
  // a finger's drag reads back, and that is kept
  p.fire('touchmove'); p.timeline.scrollTop = 1200; p.fire('scroll'); assert.equal(p.timeline.scrollTop, 1200);
  // reading back, the keyboard going moves the top by the height
  p.resize(500); assert.equal(p.timeline.scrollTop, 860);
  // not a phone: nothing moves
  const d = page(false);
  installPhoneThread({ document: d.doc, window: d.win });
  d.timeline.scrollTop = 1500; d.fire('touchstart', d.inComposer); d.resize(160); assert.equal(d.timeline.scrollTop, 1500);
});

test('a page that cannot record says so (the composer hides its mic on a phone)', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const plain = page(true, false), secure = page(true, true);
  installPhoneThread({ document: plain.doc, window: plain.win });
  installPhoneThread({ document: secure.doc, window: secure.win });
  assert.equal(plain.classes.has('ph-nomic'), true);
  assert.equal(secure.classes.has('ph-nomic'), false);
});

test('a tap on a reply\'s file link opens its menu on a phone, its menu\'s Open still opens', async () => {
  const { phoneClickAction } = await import('../../src/web/phone-thread.js');
  const link = { matches: s => s === 'a.link.file' }, item = { matches: () => false };
  assert.equal(phoneClickAction(page(true).doc, link, 'file-open'), 'file-menu');
  assert.equal(phoneClickAction(page(true).doc, item, 'file-open'), 'file-open');
  assert.equal(phoneClickAction(page(false).doc, link, 'file-open'), 'file-open');
  assert.equal(phoneClickAction(page(true).doc, link, 'select'), 'select');
});

test('back from the full-screen viewer, the thread is where the person left it', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const p = page(true), calls = [];
  const planner = { position: () => ({ top: 700, atEnd: false, anchor: 'm42', offset: 12 }), restore: b => calls.push(b) };
  installPhoneThread({ document: p.doc, window: p.win, list: () => planner });
  p.timeline.scrollTop = 700; p.fire('touchstart', { closest: () => null });
  p.fire('scroll');                 // read here (kept)
  p.resize(0);                      // the viewer covers the thread (display: none)
  p.timeline.scrollTop = 0;
  p.resize(500);                    // back to the thread
  assert.deepEqual(calls, [{ top: 700, atEnd: false, anchor: 'm42', offset: 12 }]);
  assert.equal(p.timeline.scrollTop, 0); // (the planner places it, not a shift)
});


test('the composer\'s ⋯ sheet shuts when the viewer covers the thread', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const p = page(true), more = { open: true };
  p.doc.querySelectorAll = s => s === '.composer-more[open]' && more.open ? [more] : [];
  installPhoneThread({ document: p.doc, window: p.win });
  p.fire('touchstart', { closest: () => null });
  p.resize(0);
  assert.equal(more.open, false);
});

test('Send pressed while typing on a phone gives the draft its focus back after the tap', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const p = page(true); let focused = 0;
  const composer = { id: 'composer', focus: () => focused++ };
  p.doc.getElementById = id => id === 'composer' ? composer : id === 'timeline' ? p.timeline : null;
  installPhoneThread({ document: p.doc, window: p.win });
  p.doc.activeElement = composer;
  const send = { closest: s => s === '.composer .foot-end > button' ? {} : null };
  for (const f of p.on.mousedown || []) f({ target: send });
  p.doc.activeElement = { id: 'send' };            // the press took the focus (the composition ended)
  for (const f of p.won.click || []) f({});
  assert.equal(focused, 1, 'focus back once');
  for (const f of p.won.click || []) f({});
  assert.equal(focused, 1, 'only for that tap');
  // a press elsewhere, or with the keyboard down, gives nothing back
  p.doc.activeElement = { id: 'x' };
  for (const f of p.on.mousedown || []) f({ target: send });
  for (const f of p.won.click || []) f({});
  assert.equal(focused, 1);
});

test('far from the end a ↓ goes back to the latest, on a phone only, never beside the pill', async () => {
  const { installPhoneThread } = await import('../../src/web/phone-thread.js');
  const made = [], setup = (phone, newest = () => true) => {
    const p = page(phone), calls = [];
    Object.assign(p.timeline, { isConnected: true, getBoundingClientRect: () => ({ bottom: 600 }) });
    p.doc.querySelector = s => s === '.hold-pill' ? p.pill : null;
    p.doc.body = { appendChild: e => made.push(e) };
    p.doc.createElement = () => { const e = { style: {}, attrs: {}, on: {}, setAttribute(k, v) { this.attrs[k] = v; }, addEventListener(t, f) { this.on[t] = f; } }; return e; };
    installPhoneThread({ document: p.doc, window: p.win, latest: () => calls.push('latest'), newest });
    p.timeline.scrollTop = 1500; p.fire('touchstart', { closest: () => null });
    return { p, calls };
  };
  const { p, calls } = setup(true);
  p.timeline.scrollTop = 1400; p.fire('scroll');          // 100 px from the end: none
  assert.equal(made.length, 0);
  p.timeline.scrollTop = 600; p.fire('scroll');           // 900 px (1.8 screens) up
  const down = made[0];
  assert.equal(down.hidden, false); assert.equal(down.style.top, '544px'); assert.equal(down.attrs['aria-label'], 'Go to the latest');
  down.on.click(); assert.deepEqual(calls, ['latest']); assert.equal(down.hidden, true);
  p.pill = {}; p.timeline.scrollTop = 500; p.fire('scroll'); // the hub's "N new ↓" says it
  assert.equal(down.hidden, true);
  // on an earlier page of the history, near its foot: still away from the latest
  const e = setup(true, () => false);
  e.p.timeline.scrollTop = 1400; e.p.fire('scroll');
  assert.equal(made[1].hidden, false);
  // not a phone: nothing
  const d = setup(false);
  d.p.timeline.scrollTop = 0; d.p.fire('scroll');
  assert.equal(made.length, 2);
});
