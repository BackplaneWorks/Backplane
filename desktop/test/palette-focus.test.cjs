const { test } = require('node:test'), assert = require('node:assert/strict');

async function paletteHarness() {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  let listener;
  const actions = [], doc = { activeElement: null,
    querySelector: selector => selector === '.workspace-palette' ? palette : null,
    querySelectorAll: () => [], addEventListener: (_, fn) => { listener = fn; } };
  const control = (tag, extra = {}) => ({ tagName: tag.toUpperCase(), disabled: false,
    closest(selector) { return selector.split(',').includes(tag) ? this : null; },
    checkVisibility: () => true, focus() { doc.activeElement = this; }, ...extra });
  const close = control('button'), query = control('input'), result = control('button');
  const help = control('summary', { closest: selector => selector === 'footer' ? {} : selector.includes('summary') ? help : null });
  const hidden = control('button', { checkVisibility: () => false }), disabled = control('button', { disabled: true });
  const palette = { querySelectorAll: selector => [close, query, result, help, hidden, disabled].filter(el => selector.includes(el.tagName.toLowerCase())) };
  installWorkspaceKeys({ dispatch: (...args) => actions.push(args), document: doc, platform: 'Linux' });
  query.focus();
  const press = (key, extra = {}) => {
    const event = { key, target: doc.activeElement, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...extra };
    listener(event);return event;
  };
  return { doc, close, query, result, help, actions, press };
}

test('Tab reaches shortcut help in both directions and skips hidden or disabled controls', async () => {
  const h = await paletteHarness();
  h.press('Tab');assert.equal(h.doc.activeElement, h.result);
  h.press('Tab');assert.equal(h.doc.activeElement, h.help);
  h.press('Tab', { shiftKey: true });assert.equal(h.doc.activeElement, h.result);
  h.press('Tab');h.press('Tab');assert.equal(h.doc.activeElement, h.close);
  h.press('Tab', { shiftKey: true });assert.equal(h.doc.activeElement, h.help);
});

test('help and buttons retain native activation while query Enter runs the command', async () => {
  const h = await paletteHarness();
  h.help.focus();
  for (const key of ['Enter', ' ', 'ArrowUp', 'ArrowDown']) assert.equal(h.press(key).defaultPrevented, false);
  h.close.focus();assert.equal(h.press('Enter').defaultPrevented, false);
  h.result.focus();assert.equal(h.press('Enter').defaultPrevented, false);
  assert.deepEqual(h.actions, []);
  h.query.focus();assert.equal(h.press('Enter').defaultPrevented, true);
  h.help.focus();assert.equal(h.press('Escape').defaultPrevented, true);
  assert.deepEqual(h.actions, [['ws-palette-enter', ''], ['ws-palette-close', '']]);
});
