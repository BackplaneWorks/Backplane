const { test } = require('node:test'), assert = require('node:assert/strict');
const { installShortcuts } = require('../window.cjs');
const event = (key, fields = {}) => ({ key, code: '', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, target: { closest: () => null }, ...fields });

test('restored navigation chords respect editors, IME, PTYs and platform modifiers', async () => {
  const { workspaceShortcut: key } = await import('../../src/web/workspace-keys.js');
  assert.deepEqual(key(event('2', { altKey: true }), 'Linux'), ['ws-key', 'alt+2']);
  assert.deepEqual(key(event('2', { ctrlKey: true }), 'Linux'), ['ws-key', 'ctrl+2']);
  assert.deepEqual(key(event('}', { ctrlKey: true, shiftKey: true, code: 'BracketRight' }), 'Linux'), ['ws-key', 'next']);
  assert.deepEqual(key(event('U', { ctrlKey: true, shiftKey: true }), 'Linux'), ['ws-key', 'uber']);
  const editor = { closest: () => ({}) };
  for (const input of [event('2', { altKey: true, target: editor }), event('2', { ctrlKey: true, target: editor }),
    event('}', { ctrlKey: true, shiftKey: true, target: editor }), event('U', { ctrlKey: true, shiftKey: true, target: editor }),
    event('P', { ctrlKey: true, shiftKey: true, isComposing: true }), event('2', { altKey: true, getModifierState: () => true })]) assert.equal(key(input, 'Linux'), null);
  assert.deepEqual(key(event('P', { ctrlKey: true, shiftKey: true, target: editor }), 'Linux'), ['ws-palette-open', '']);
  assert.deepEqual(key(event('P', { metaKey: true, shiftKey: true }), 'MacIntel'), ['ws-palette-open', '']);
  assert.deepEqual(key(event('~', { metaKey: true, shiftKey: true, code: 'Backquote', target: editor }), 'Linux'), ['ws-key', 'term']);
  assert.equal(key(event('~', { metaKey: true, shiftKey: true, code: 'Backquote' }), 'MacIntel'), null);
});

test('Electron passes shell control keys to a focused PTY and keeps terminal clipboard chords', () => {
  let handler, prevented = 0; const calls = [];
  const contents = { on: (_name, value) => { handler = value; }, copy: () => calls.push('copy'), paste: () => calls.push('paste') };
  installShortcuts({ webContents: contents }, { app: {}, webContents: { getFocusedWebContents: () => contents } }, undefined, 'linux', () => true);
  const press = (key, shift = false) => handler({ preventDefault: () => prevented++ }, { type: 'keyDown', key, control: true, shift });
  for (const key of ['c', 'a', 'z', 'r', 'd', 'p']) press(key);
  assert.equal(prevented, 0);assert.deepEqual(calls, []);
  press('c', true);press('v', true);assert.equal(prevented, 2);assert.deepEqual(calls, ['copy', 'paste']);
});

// Exercise the installed capture listener and the focus transition, not only
// the chord normalizer: Escape must release navigation without losing a draft.
function composerHarness(installWorkspaceKeys) {
  let listener;
  const actions = [], doc = { querySelector: () => null, activeElement: null,
    addEventListener: (name, callback) => { assert.equal(name, 'keydown'); listener = callback; } };
  const body = { tagName: 'BODY', closest: () => null };
  const composer = { tagName: 'TEXTAREA', id: 'composer', value: 'unsent draft', completion: '',
    closest: () => composer, getAttribute: () => composer.completion,
    blur: () => { doc.activeElement = body; } };
  doc.activeElement = composer;
  installWorkspaceKeys({ dispatch: (...action) => actions.push(action), document: doc, platform: 'Linux' });
  const press = (key, fields = {}) => {
    const input = event(key, { target: doc.activeElement, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  return { doc, composer, body, actions, press };
}

test('composer Escape restores focus-free navigation and preserves the unsent draft', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  h.press('2', { ctrlKey: true });
  assert.deepEqual(h.actions, []);
  assert.equal(h.press('Escape').defaultPrevented, true);
  assert.equal(h.doc.activeElement, h.body);
  assert.equal(h.composer.value, 'unsent draft');
  h.press('2', { ctrlKey: true });
  assert.deepEqual(h.actions, [['ws-key', 'ctrl+2']]);
});

test('composer Escape leaves completion, IME and PTY input ownership intact', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  h.composer.completion = 'skill-close';
  assert.equal(h.press('Escape').defaultPrevented, false);
  assert.equal(h.doc.activeElement, h.composer);
  h.composer.completion = '';
  for (const fields of [{ isComposing: true }, { keyCode: 229 }, { ctrlKey: true }]) {
    assert.equal(h.press('Escape', fields).defaultPrevented, false);
    assert.equal(h.doc.activeElement, h.composer);
  }
  const pty = { tagName: 'TEXTAREA', id: 'term', closest: () => pty, getAttribute: () => '',
    blur: () => assert.fail('PTY Escape must reach its shell') };
  h.doc.activeElement = pty;
  assert.equal(h.press('Escape').defaultPrevented, false);
  assert.equal(h.doc.activeElement, pty);
  assert.deepEqual(h.actions, []);
});


test('bare backtick toggles once per press from composer/workspace without inserting text', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  assert.equal(h.press('`', { code: 'Backquote' }).defaultPrevented, true);
  assert.equal(h.composer.value, 'unsent draft');
  assert.equal(h.doc.activeElement, h.composer);
  assert.deepEqual(h.actions, [['ws-key', 'term']]);
  assert.equal(h.press('`', { code: 'Backquote', repeat: true }).defaultPrevented, true);
  assert.deepEqual(h.actions, [['ws-key', 'term']]);
  h.doc.activeElement = h.body;
  assert.equal(h.press('`', { code: 'Backquote' }).defaultPrevented, true);
  assert.deepEqual(h.actions, [['ws-key', 'term'], ['ws-key', 'term']]);
});

test('bare backtick preserves shell/text input, tilde, composition and other modifiers', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  for (const fields of [{ isComposing: true }, { keyCode: 229 }, { getModifierState: () => true },
    { shiftKey: true }, { ctrlKey: true }, { altKey: true }]) {
    assert.equal(h.press('`', { code: 'Backquote', ...fields }).defaultPrevented, false);
  }
  assert.equal(h.press('~', { code: 'Backquote', shiftKey: true }).defaultPrevented, false);
  assert.equal(h.press('Dead', { code: 'Backquote' }).defaultPrevented, false);
  for (const [tagName, id] of [['TEXTAREA', 'term'], ['INPUT', 'search'], ['INPUT', 'password'], ['DIV', 'editor']]) {
    const field = { tagName, id, closest: () => field };
    h.doc.activeElement = field;
    assert.equal(h.press('`', { code: 'Backquote' }).defaultPrevented, false);
  }
  assert.deepEqual(h.actions, []);
});

test('Ctrl+B (Cmd+B on a Mac) folds the sidebar, from the composer too; Shift or Alt does not', async () => {
  const { workspaceShortcut: key } = await import('../../src/web/workspace-keys.js');
  const editor = { closest: () => ({}) };
  assert.deepEqual(key(event('b', { ctrlKey: true }), 'Linux'), ['side-toggle', '']);
  assert.deepEqual(key(event('b', { ctrlKey: true, target: editor }), 'Linux'), ['side-toggle', '']);
  assert.deepEqual(key(event('b', { metaKey: true }), 'MacIntel'), ['side-toggle', '']);
  assert.equal(key(event('b', { ctrlKey: true }), 'MacIntel'), null);
  assert.equal(key(event('B', { ctrlKey: true, shiftKey: true }), 'Linux'), null);
  assert.equal(key(event('b', { ctrlKey: true, altKey: true }), 'Linux'), null);
});

test('Ctrl+M opens the model picker from the workspace and the composer, never from a PTY or another field', async () => {
  const { workspaceShortcut: key } = await import('../../src/web/workspace-keys.js');
  const field = (tagName, id) => { const f = { tagName, id, closest: () => f }; return f; };
  assert.deepEqual(key(event('m', { ctrlKey: true }), 'Linux'), ['flag', 'models']);
  assert.deepEqual(key(event('m', { ctrlKey: true, target: field('TEXTAREA', 'composer') }), 'Linux'), ['flag', 'models']);
  assert.deepEqual(key(event('m', { ctrlKey: true, target: field('INPUT', 'model-q') }), 'Linux'), ['flag', 'models']);
  assert.deepEqual(key(event('m', { ctrlKey: true }), 'MacIntel'), ['flag', 'models']);
  assert.equal(key(event('m', { metaKey: true }), 'MacIntel'), null); // Cmd+M minimizes the desktop window
  for (const f of [field('TEXTAREA', 'term'), field('INPUT', 'search'), field('DIV', 'editor')])
    assert.equal(key(event('m', { ctrlKey: true, target: f }), 'Linux'), null);
  for (const fields of [{ shiftKey: true }, { altKey: true }, { metaKey: true }, { isComposing: true }])
    assert.equal(key(event('m', { ctrlKey: true, ...fields }), 'Linux'), null);
});

test('the open model picker owns Tab, the arrows, Enter and Escape wherever the focus is', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  const layer = { style: { display: 'contents' } };
  h.doc.querySelector = selector => selector === '[data-local-models="1"]' ? layer : null;
  const keys = [['Tab'], ['Tab', { shiftKey: true }], ['ArrowUp'], ['ArrowDown'], ['ArrowLeft'], ['ArrowRight'], ['Enter'], ['Escape']];
  for (const [k, fields] of keys) assert.equal(h.press(k, fields).defaultPrevented, true);
  assert.deepEqual(h.actions, [['mp-key', 'tab'], ['mp-key', 'stab'], ['mp-key', 'up'], ['mp-key', 'down'], ['mp-key', 'left'], ['mp-key', 'right'], ['mp-key', 'enter'], ['flag', 'models']]);
  h.actions.length = 0;
  // the search field keeps its caret keys; typing and chords go on as before
  const search = { tagName: 'INPUT', id: 'model-q', closest: () => search };
  h.doc.activeElement = search;
  assert.equal(h.press('ArrowLeft').defaultPrevented, false);
  assert.equal(h.press('ArrowDown').defaultPrevented, true);
  h.doc.activeElement = h.composer;
  assert.equal(h.press('a').defaultPrevented, false);
  assert.equal(h.press('Enter', { shiftKey: true }).defaultPrevented, false);
  assert.equal(h.press('Enter', { isComposing: true }).defaultPrevented, false);
  assert.equal(h.press('m', { ctrlKey: true }).defaultPrevented, true);
  assert.deepEqual(h.actions, [['mp-key', 'down'], ['flag', 'models']]);
  // shut: the keys are the composer's again
  h.actions.length = 0;layer.style.display = 'none';
  assert.equal(h.press('Enter').defaultPrevented, false);
  assert.equal(h.press('Tab').defaultPrevented, false);
  assert.deepEqual(h.actions, []);
});
