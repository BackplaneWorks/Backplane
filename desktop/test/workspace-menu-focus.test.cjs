const { test } = require('node:test');
const assert = require('node:assert/strict');

test('Escape from a disclosure returns the keyboard before the next key', async () => {
  const { installWorkspaceMenus } = await import('../../src/web/workspace-menus.js');
  let keydown, focused = 'menu';
  const menu = { open: true, querySelector: () => ({ focus() { focused = 'summary'; } }) };
  const doc = { addEventListener(type, handler) { if (type === 'keydown') keydown = handler; } };
  installWorkspaceMenus(doc, () => { assert.equal(menu.open, false);focused = 'composer'; });
  keydown({ key: 'Escape', target: { closest: () => menu }, preventDefault() {}, stopImmediatePropagation() {} });
  assert.equal(focused, 'composer');
});
