const { test } = require('node:test');
const assert = require('node:assert/strict');

test('Escape from a disclosure returns the keyboard before the next key', async () => {
  const { installWorkspaceMenus } = await import('../../src/web/workspace-menus.js');
  let keydown, focused = 'menu';
  const menu = { open: true, querySelector: selector => selector.includes('summary') ? { focus() { focused = 'summary'; } } : null };
  const doc = { addEventListener(type, handler) { if (type === 'keydown') keydown = handler; } };
  installWorkspaceMenus(doc, () => { assert.equal(menu.open, false);focused = 'composer'; });
  keydown({ key: 'Escape', target: { closest: () => menu }, preventDefault() {}, stopImmediatePropagation() {} });
  assert.equal(focused, 'composer');
});


test('dismissal cancels Delete before its menu can be reopened', async () => {
  const { closeWorkspaceMenu } = await import('../../src/web/workspace-menus.js');
  let armed = true;
  const menu = { open: true, querySelector: selector => selector.includes('delete-no') ?
    { click() { assert.equal(menu.open, false);armed = false; } } : null };
  closeWorkspaceMenu(menu);
  menu.open = true;
  assert.equal(armed, false);
});
