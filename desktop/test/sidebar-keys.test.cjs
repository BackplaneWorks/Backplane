const { test } = require('node:test'), assert = require('node:assert/strict');

async function harness() {
  const { installSidebarKeys } = await import('../../src/web/sidebar-keys.js');
  let listener;
  const clicks = [], doc = { activeElement: null, addEventListener: (_, fn) => { listener = fn; }, querySelectorAll: () => rows };
  const row = (id, visible = true) => ({ id, checkVisibility: () => visible, closest() { return this; },
    focus() { doc.activeElement = this; }, click() { clicks.push(id); } });
  const rows = [row('first'), row('hidden', false), row('last')];rows[0].focus();
  const press = (key, extra = {}) => {
    const event = { key, target: doc.activeElement, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...extra };
    listener(event);return event;
  };
  installSidebarKeys(doc);return { doc, rows, clicks, press };
}

test('conversation keys focus visible rows without selecting until activation', async () => {
  const h = await harness();
  h.press('ArrowDown');assert.equal(h.doc.activeElement, h.rows[2]);assert.deepEqual(h.clicks, []);
  h.press('Enter');h.press('Enter', { repeat: true });assert.deepEqual(h.clicks, ['last']);
  h.press('ArrowUp');assert.equal(h.doc.activeElement, h.rows[0]);
  h.press(' ');assert.deepEqual(h.clicks, ['last', 'first']);
  h.press('End');assert.equal(h.doc.activeElement, h.rows[2]);
  h.press('Home');assert.equal(h.doc.activeElement, h.rows[0]);
  h.press('ArrowUp');assert.equal(h.doc.activeElement, h.rows[0]);
});

test('nested controls, modified keys and composition keep their keyboard events', async () => {
  const h = await harness();
  for (const extra of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { shiftKey: true }, { isComposing: true }, { keyCode: 229 },
    { target: { closest: () => h.rows[0] } }]) assert.equal(h.press('Enter', extra).defaultPrevented, false);
  assert.deepEqual(h.clicks, []);
});
