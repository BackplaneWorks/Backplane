const { test } = require('node:test'), assert = require('node:assert/strict');
const { installShortcuts } = require('../window.cjs');
const event = (key, fields = {}) => ({ key, code: '', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, target: { closest: () => null }, ...fields });
// A stand-in element: closest(selector) finds it when the selector names its
// tag or one of its attributes (the PTY is a textarea with data-keys).
const element = (tagName, id = '', attrs = []) => {
  const el = { tagName: tagName.toUpperCase(), id, hasAttribute: name => attrs.includes(name),
    closest: selector => selector.split(',').some(s => s === tagName || attrs.some(a => s === `[${a}]`)) ? el : null };
  return el;
};

test('restored navigation chords respect editors, IME, PTYs and platform modifiers', async () => {
  const { workspaceShortcut: key } = await import('../../src/web/workspace-keys.js');
  assert.deepEqual(key(event('2', { altKey: true }), 'Linux'), ['ws-key', 'alt+2']);
  assert.deepEqual(key(event('2', { ctrlKey: true }), 'Linux'), ['ws-key', 'ctrl+2']);
  assert.deepEqual(key(event('}', { ctrlKey: true, shiftKey: true, code: 'BracketRight' }), 'Linux'), ['ws-key', 'next']);
  assert.deepEqual(key(event('U', { ctrlKey: true, shiftKey: true }), 'Linux'), ['ws-key', 'uber']);
  const editor = element('textarea', 'term', ['data-keys']);
  for (const input of [event('2', { altKey: true, target: editor }), event('2', { ctrlKey: true, target: editor }),
    event('}', { ctrlKey: true, shiftKey: true, target: editor }), event('U', { ctrlKey: true, shiftKey: true, target: editor }),
    event('2', { altKey: true, target: element('textarea', 'composer') }),
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
  const composer = Object.assign(element('textarea', 'composer'), { value: 'unsent draft', completion: '',
    getAttribute: () => composer.completion, blur: () => { doc.activeElement = body; } });
  doc.activeElement = composer;
  installWorkspaceKeys({ dispatch: (...action) => actions.push(action), document: doc, platform: 'Linux' });
  const press = (key, fields = {}) => {
    const input = event(key, { target: doc.activeElement, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  return { doc, composer, body, actions, press };
}

test('composer Escape leaves the field and preserves the unsent draft; Ctrl+2 jumps from either side', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  assert.equal(h.press('2', { ctrlKey: true }).defaultPrevented, true);
  assert.deepEqual(h.actions, [['ws-key', 'ctrl+2']]);
  assert.equal(h.composer.value, 'unsent draft');
  assert.equal(h.press('Escape').defaultPrevented, true);
  assert.equal(h.doc.activeElement, h.body);
  assert.equal(h.composer.value, 'unsent draft');
  h.press('2', { ctrlKey: true });
  assert.deepEqual(h.actions, [['ws-key', 'ctrl+2'], ['ws-key', 'ctrl+2']]);
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

test('a bare letter or digit names a rail keycap only outside editors and without modifiers', async () => {
  const { railKey } = await import('../../src/web/workspace-keys.js');
  const editor = { closest: () => ({}) };
  assert.equal(railKey(event('b')), 'b');
  assert.equal(railKey(event('3')), '3');
  assert.equal(railKey(event('S')), 's');
  for (const input of [event('B', { shiftKey: true }), event('b', { ctrlKey: true }), event('b', { metaKey: true }), event('b', { altKey: true }),
    event('b', { target: editor }), event('b', { isComposing: true }), event('b', { keyCode: 229 }), event('b', { repeat: true }),
    event('b', { getModifierState: () => true }), event('b', { defaultPrevented: true }), event('`'), event('Enter'), event(' '), event('é')]) assert.equal(railKey(input), '');
  // on one of the composer's own buttons (Send, Stop) a letter is a word
  // meant for the draft, never a rail button
  const send = { tagName: 'BUTTON', closest: selector => selector.split(',').includes('.composer') ? send : null };
  for (const k of ['m', 'o', 'r', 'e', '3']) assert.equal(railKey(event(k, { target: send })), '');
});

test('viewer focus and pointer ownership keep bare keys away from the rail', async () => {
  const { railKey, installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const viewer = { closest: selector => selector.split(',').includes('#viewer') ? viewer : null };
  for (const key of ['f', 'b', 's', '3', '1', '2']) assert.equal(railKey(event(key, { target: viewer })), '');
  const hovered = { querySelector: selector => selector === '#viewer:hover' ? viewer : null };
  assert.equal(railKey(event('f'), hovered), '');
  assert.equal(railKey(event('f'), { querySelector: () => null }), 'f');
  const h = railHarness(installWorkspaceKeys);
  assert.equal(h.press('b', { target: viewer }).defaultPrevented, false);
  assert.deepEqual(h.clicks, []);
  // Explicit workspace chords still work from a viewer.
  h.press('d', { ctrlKey: true, target: viewer });
  assert.deepEqual(h.actions, [['ws-key', 'diff']]);
});

// The installed listener presses the rail button that shows the key, and
// leaves every other key (and every key typed into a field) alone.
function railHarness(installWorkspaceKeys, { overlay = false } = {}) {
  let listener;
  const clicks = [], actions = [], buttons = { b: { click: () => clicks.push('b') }, d: { click: () => clicks.push('d') } };
  const doc = { activeElement: null,
    querySelector: (sel) => sel === '.tour,.lightbox' ? (overlay ? {} : null) : (/data-keycap="(.)"/.exec(sel) ? buttons[/data-keycap="(.)"/.exec(sel)[1]] || null : null),
    addEventListener: (_name, callback) => { listener = callback; } };
  installWorkspaceKeys({ dispatch: (...action) => actions.push(action), document: doc, platform: 'Linux' });
  const body = { tagName: 'BODY', closest: () => null }, composer = Object.assign(element('textarea', 'composer'), { getAttribute: () => '' });
  const press = (key, fields = {}) => {
    const input = event(key, { target: body, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  return { clicks, actions, press, body, composer };
}

test('a rail keycap presses its button once; typing, other keys and overlays are untouched', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = railHarness(installWorkspaceKeys);
  assert.equal(h.press('b').defaultPrevented, true);
  assert.deepEqual(h.clicks, ['b']);
  assert.equal(h.press('b', { repeat: true }).defaultPrevented, false);
  assert.equal(h.press('x').defaultPrevented, false);
  assert.equal(h.press('d', { target: h.composer }).defaultPrevented, false);
  assert.equal(h.press('D', { shiftKey: true }).defaultPrevented, false);
  assert.deepEqual(h.clicks, ['b']);
  h.press('d', { ctrlKey: true });
  assert.deepEqual(h.actions, [['ws-key', 'diff']]);
  assert.deepEqual(h.clicks, ['b']);
  const shut = railHarness(installWorkspaceKeys, { overlay: true });
  assert.equal(shut.press('b').defaultPrevented, false);
  assert.deepEqual(shut.clicks, []);
});

test('Ctrl/Cmd workspace chords work from the composer and other text fields, not from a PTY', async () => {
  const { workspaceShortcut: key } = await import('../../src/web/workspace-keys.js');
  const composer = element('textarea', 'composer'), search = element('input', 'side-search'), title = element('input', 'title');
  const chords = [[event('k', { ctrlKey: true }), ['ws-key', 'palette']], [event('1', { ctrlKey: true }), ['ws-key', 'ctrl+1']],
    [event('9', { ctrlKey: true }), ['ws-key', 'ctrl+9']], [event('S', { ctrlKey: true, shiftKey: true }), ['ws-key', 'settle']],
    [event('n', { ctrlKey: true }), ['ws-key', 'new']], [event('d', { ctrlKey: true }), ['ws-key', 'diff']], [event('p', { ctrlKey: true }), ['ws-key', 'files']],
    [event('{', { ctrlKey: true, shiftKey: true, code: 'BracketLeft' }), ['ws-key', 'prev']], [event('U', { ctrlKey: true, shiftKey: true }), ['ws-key', 'uber']],
    [event('F', { ctrlKey: true, shiftKey: true }), ['thin-search-open', '']]];
  for (const target of [composer, search, title])
    for (const [input, action] of chords) assert.deepEqual(key({ ...input, target }, 'Linux'), action, `${input.key} in ${target.id}`);
  // Cmd on a Mac, from the composer too
  assert.deepEqual(key(event('k', { metaKey: true, target: composer }), 'MacIntel'), ['ws-key', 'palette']);
  assert.deepEqual(key(event('3', { metaKey: true, target: composer }), 'MacIntel'), ['ws-key', 'ctrl+3']);
  assert.equal(key(event('k', { ctrlKey: true, target: composer }), 'MacIntel'), null);
  // what edits text stays the field's
  for (const [k, fields] of [['a', {}], ['c', {}], ['v', {}], ['x', {}], ['z', {}], ['y', {}], ['Z', { shiftKey: true }], ['Enter', {}],
    ['Backspace', {}], ['Delete', {}], ['ArrowLeft', {}], ['ArrowRight', { shiftKey: true }], ['Home', {}], ['End', {}]])
    assert.equal(key(event(k, { ctrlKey: true, target: composer, ...fields }), 'Linux'), null, 'Ctrl+' + k);
  for (const k of ['Home', 'End', 'ArrowUp', 'Backspace', 'Enter']) assert.equal(key(event(k, { target: composer }), 'Linux'), null);
  // a PTY keeps its control keys; IME composition is left alone
  const pty = element('textarea', 'term', ['data-keys']), frame = element('div', '', ['data-keyto']);
  for (const target of [pty, frame]) for (const [input] of chords) assert.equal(key({ ...input, target }, 'Linux'), null);
  assert.equal(key(event('k', { ctrlKey: true, target: composer, isComposing: true }), 'Linux'), null);
  assert.equal(key(event('1', { ctrlKey: true, target: composer, keyCode: 229 }), 'Linux'), null);
});

test('bare keys and Alt+digit stay with a text field', async () => {
  const { workspaceShortcut: key, railKey } = await import('../../src/web/workspace-keys.js');
  const composer = element('textarea', 'composer');
  for (const k of ['b', 's', '3', 'v', 'd']) {
    assert.equal(key(event(k, { target: composer }), 'Linux'), null);
    assert.equal(railKey(event(k, { target: composer })), '');
  }
  assert.equal(key(event('1', { altKey: true, target: composer }), 'MacIntel'), null);
  assert.deepEqual(key(event('1', { altKey: true }), 'MacIntel'), ['ws-key', 'alt+1']);
});

test('Ctrl+K from the composer opens the palette once and keeps the draft', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = composerHarness(installWorkspaceKeys);
  assert.equal(h.press('k', { ctrlKey: true }).defaultPrevented, true);
  assert.equal(h.press('k', { ctrlKey: true, repeat: true }).defaultPrevented, true);
  assert.equal(h.press('S', { ctrlKey: true, shiftKey: true }).defaultPrevented, true);
  assert.equal(h.press('a', { ctrlKey: true }).defaultPrevented, false);
  assert.equal(h.press('v', { ctrlKey: true }).defaultPrevented, false);
  assert.equal(h.press('s').defaultPrevented, false);
  assert.deepEqual(h.actions, [['ws-key', 'palette'], ['ws-key', 'settle']]);
  assert.equal(h.doc.activeElement, h.composer);
  assert.equal(h.composer.value, 'unsent draft');
});

// Esc puts away the menu or dialog over the page wherever the keyboard is
// (a row's menu opened from the sidebar, a project's Intent opened while
// another project's thread had the focus), the topmost first, and never
// the model picker's own shade.
test('Escape closes the open menu or dialog wherever the focus is', async () => {
  const { installWorkspaceKeys, overlayEscape } = await import('../../src/web/workspace-keys.js');
  let listener, shades = [], menus = [];
  const clicked = [], actions = [];
  let claimed = 0;
  const shade = (value, name) => ({ getAttribute: k => k === 'data-value' ? value : null, click: () => clicked.push(name) });
  const doc = { activeElement: null, querySelector: q => q === '.local-menu[open]' ? menus[0] || null : null,
    querySelectorAll: () => shades, addEventListener: (_n, callback) => { listener = callback; } };
  installWorkspaceKeys({ dispatch: (...a) => actions.push(a), document: doc, platform: 'Linux', composer: { claim: () => { if (menus.length) { assert.equal(menus[0].open, false);claimed++; } } } });
  const body = { tagName: 'BODY', closest: () => null, getAttribute: () => null };
  const composer = { tagName: 'TEXTAREA', id: 'composer', closest: () => composer, getAttribute: () => '', blur: () => assert.fail('the menu goes first') };
  const press = (target, fields = {}) => {
    const input = event('Escape', { target, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  assert.equal(overlayEscape(doc), null);
  // nothing over the page: Esc is the page's own (it puts nothing away here)
  assert.equal(press(body).defaultPrevented, true);
  assert.deepEqual(actions, []);
  shades = [shade('', 'row menu')];
  assert.equal(press(body).defaultPrevented, true);
  assert.deepEqual(clicked, ['row menu']);
  shades = [shade('models', 'model picker'), shade('', 'badge editor'), shade('t1', 'intent')];
  assert.equal(press(composer).defaultPrevented, true);
  assert.deepEqual(clicked, ['row menu', 'intent']);
  shades = [shade('models', 'model picker')];
  assert.equal(overlayEscape(doc), null);
  const details = { open: true, querySelector: () => null };menus = [details];
  assert.equal(press(body).defaultPrevented, true);
  assert.equal(details.open, false);
  assert.equal(claimed, 1);
  menus = [];shades = [shade('', 'row menu')];
  for (const fields of [{ ctrlKey: true }, { shiftKey: true }, { isComposing: true }]) assert.equal(press(body, fields).defaultPrevented, false);
  assert.deepEqual(clicked, ['row menu', 'intent']);
  assert.deepEqual(actions, []);
});

test('Escape shuts the token stats panel, from the composer too; nothing when it is shut', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  let listener, open = true;
  const actions = [], doc = { activeElement: null, querySelector: (sel) => sel === '.tok-pop' && open ? {} : null,
    addEventListener: (_name, callback) => { listener = callback; } };
  installWorkspaceKeys({ dispatch: (...action) => actions.push(action), document: doc, platform: 'Linux' });
  const chip = { tagName: 'BUTTON', closest: () => null }, composer = { tagName: 'TEXTAREA', id: 'composer', closest: () => composer, getAttribute: () => '', blur: () => {} };
  const press = (key, fields = {}) => {
    const input = event(key, { target: chip, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  assert.equal(press('Escape').defaultPrevented, true);
  assert.equal(press('Escape', { target: composer }).defaultPrevented, true);
  assert.deepEqual(actions, [['flag', 'tokstats'], ['flag', 'tokstats']]);
  assert.equal(press('Escape', { shiftKey: true }).defaultPrevented, false);
  open = false;
  press('Escape');
  assert.deepEqual(actions, [['flag', 'tokstats'], ['flag', 'tokstats']]);
});

// Esc once no menu, picker or palette took it: the installed listener asks
// escNext (workspace.bend's WS.esc.next, mirrored here) what to put away
// from what the page shows, and the keyboard goes back to the composer.
function escHarness(installWorkspaceKeys) {
  let listener;
  const actions = [], claims = [], page = { term: false, probe: false, viewer: false, dialog: false };
  const doc = { activeElement: null, addEventListener: (_name, callback) => { listener = callback; },
    querySelector: sel => sel === '.floating-terminal' ? (page.term ? {} : null) : sel === '.probe-chip' ? (page.probe ? {} : null)
      : sel.includes('[data-on-click="panel"]') ? (page.viewer ? {} : null) : null,
    querySelectorAll: sel => sel.includes('[role="dialog"]') && page.dialog ? [{ getClientRects: () => [{}] }] : [] };
  const escNext = (term, probe, viewer) => term ? 'term' : probe ? 'probe' : viewer ? 'viewer' : '';
  const composerFocus = { claim: () => claims.push('claim'), end: () => { claims.push('end'); return true; } };
  installWorkspaceKeys({ dispatch: (...action) => actions.push(action), document: doc, platform: 'Linux', escNext, composer: composerFocus });
  const body = { tagName: 'BODY', closest: () => null };
  let blurred = 0;
  const composer = Object.assign(element('textarea', 'composer'), { completion: '', getAttribute: () => composer.completion, blur: () => { blurred++; } });
  const press = (key, fields = {}) => {
    const input = event(key, { target: body, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() {}, ...fields });
    listener(input); return input;
  };
  return { actions, claims, page, press, body, composer, blurred: () => blurred };
}

test('Esc puts away the terminal, then the probe, then the viewer, and the keyboard goes back to the composer', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = escHarness(installWorkspaceKeys);
  Object.assign(h.page, { term: true, probe: true, viewer: true });
  assert.equal(h.press('Escape', { target: h.composer }).defaultPrevented, true);
  h.page.term = false;h.press('Escape', { target: h.composer });
  h.page.probe = false;h.press('Escape');
  assert.deepEqual(h.actions, [['ws-term-hide', ''], ['probe', ''], ['panel', '']]);
  assert.deepEqual(h.claims, ['claim', 'claim', 'claim']);
  // nothing open: from the page the keyboard goes to the composer; in the
  // composer Esc leaves it (a bare key then presses its rail button) and
  // the draft is untouched
  h.page.viewer = false;h.press('Escape');
  assert.deepEqual(h.claims, ['claim', 'claim', 'claim', 'claim']);
  h.press('Escape', { target: h.composer });
  assert.equal(h.blurred(), 1);
  assert.equal(h.actions.length, 3);
  // a held Esc puts away one thing, not all of them
  Object.assign(h.page, { term: true, viewer: true });
  h.press('Escape', { repeat: true });
  assert.equal(h.actions.length, 3);
});

test('Esc stays with a PTY, another field, the completions and a dialog; Shift+Esc hides the terminal from inside it', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  const h = escHarness(installWorkspaceKeys);
  Object.assign(h.page, { term: true, viewer: true });
  const pty = { tagName: 'TEXTAREA', id: 'term', closest: sel => sel === '.floating-terminal' || sel.includes('[data-keys]') ? pty : null, getAttribute: () => '' };
  assert.equal(h.press('Escape', { target: pty }).defaultPrevented, false);
  const rename = { tagName: 'INPUT', id: 'thread-title', closest: sel => sel.includes('input') ? rename : null };
  assert.equal(h.press('Escape', { target: rename }).defaultPrevented, false);
  h.composer.completion = 'skill-close';
  assert.equal(h.press('Escape', { target: h.composer }).defaultPrevented, false);
  h.composer.completion = '';
  h.page.dialog = true;
  assert.equal(h.press('Escape').defaultPrevented, false);
  h.page.dialog = false;
  for (const fields of [{ ctrlKey: true }, { altKey: true }, { metaKey: true }, { isComposing: true }])
    assert.equal(h.press('Escape', fields).defaultPrevented, false);
  assert.deepEqual(h.actions, []);
  assert.equal(h.press('Escape', { target: pty, shiftKey: true }).defaultPrevented, true);
  assert.equal(h.press('Escape', { shiftKey: true }).defaultPrevented, false);
  assert.deepEqual(h.actions, [['ws-term-hide', '']]);
});

test('a bare / outside every field puts the keyboard in the composer; in a field it types', async () => {
  const { installWorkspaceKeys, slashKey } = await import('../../src/web/workspace-keys.js');
  const h = escHarness(installWorkspaceKeys);
  assert.equal(h.press('/').defaultPrevented, true);
  assert.equal(h.press('/', { shiftKey: true }).defaultPrevented, true);
  assert.deepEqual(h.claims, ['end', 'end']);
  assert.equal(h.press('/', { target: h.composer }).defaultPrevented, false);
  for (const fields of [{ ctrlKey: true }, { altKey: true }, { metaKey: true }, { repeat: true }, { isComposing: true }])
    assert.equal(slashKey(event('/', fields)), false);
  assert.equal(h.claims.length, 2);
});

// The composer keeper over a stand-in page: a thread on show, what covers
// it, the viewer's close, the person's press and selection.
function keeperHarness(installComposerFocus, { phone = false } = {}) {
  const listeners = new Map();
  const page = { thread: 't1', over: false, viewer: false, selection: true, hidden: false, cover: null };
  const body = { tagName: 'BODY', closest: () => null };
  const doc = { body, activeElement: body,
    addEventListener: (type, handler) => { if (!listeners.has(type)) listeners.set(type, []);listeners.get(type).push(handler); },
    getElementById: id => id === 'composer' ? composer : null,
    querySelector: sel => sel.startsWith('main.thread-view') ? (page.thread ? { getAttribute: () => page.thread } : null)
      : sel.includes('[data-on-click="panel"]') ? (page.viewer ? {} : null) : null,
    querySelectorAll: sel => page.cover ? [page.cover] : sel.includes('.workspace-palette') && page.over ? [{ getClientRects: () => [{}] }] : [] };
  const inputs = [];
  const composer = { tagName: 'TEXTAREA', id: 'composer', value: 'a draft', selectionStart: 0, selectionEnd: 0, disabled: false,
    getClientRects: () => [{}], checkVisibility: () => !page.hidden, contains: () => false, closest: sel => sel.split(',').includes('textarea') ? composer : null,
    focus() { if (!page.hidden) doc.activeElement = composer; }, setSelectionRange(s, e) { composer.selectionStart = s;composer.selectionEnd = e; },
    setRangeText(text, s, e) { composer.value = composer.value.slice(0, s) + text + composer.value.slice(e);composer.selectionStart = composer.selectionEnd = s + text.length; },
    dispatchEvent(e) { inputs.push(e); } };
  const win = { matchMedia: () => ({ matches: phone }), setTimeout: f => f(), getSelection: () => ({ isCollapsed: page.selection, anchorNode: {} }),
    InputEvent: class { constructor(type, init) { Object.assign(this, { type }, init); } } };
  const keeper = installComposerFocus({ document: doc, window: win });
  const fire = (type, target = doc.activeElement) => { for (const handler of listeners.get(type) || []) handler({ target }); };
  // a key as the page's capture listener sees it; `rail` says whether a
  // later listener (the rail's keycaps) would still get it
  const key = (k, target = doc.activeElement, fields = {}) => {
    const e = { key: k, target, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, defaultPrevented: false, rail: true, ...fields,
      preventDefault() { e.defaultPrevented = true; }, stopImmediatePropagation() { e.rail = false; } };
    for (const handler of listeners.get('keydown') || []) handler(e);
    return e;
  };
  return { doc, body, composer, page, keeper, fire, key, inputs };
}

test('the composer takes the keyboard on load, when another thread shows, when a cover goes or the viewer shuts, caret kept', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  assert.deepEqual([h.composer.selectionStart, h.composer.selectionEnd], [7, 7]);
  // the caret where the person left it in t1's draft
  h.composer.setSelectionRange(2, 2);h.fire('keyup');
  // a click on the timeline leaves the page with the keyboard: kept there
  h.fire('pointerdown', h.body);h.doc.activeElement = h.body;h.fire('pointerup', h.body);h.keeper.check();
  assert.equal(h.doc.activeElement, h.body);
  // the palette opens and shuts: back to the composer, caret where it was
  h.page.over = true;h.keeper.check();h.page.over = false;h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  assert.deepEqual([h.composer.selectionStart, h.composer.selectionEnd], [2, 2]);
  // the viewer shuts (its rail button kept the keyboard)
  const rail = { tagName: 'BUTTON', closest: () => null };
  h.page.viewer = true;h.keeper.check();h.doc.activeElement = rail;h.page.viewer = false;h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  // another thread on show
  h.doc.activeElement = h.body;h.page.thread = 't2';h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  // Esc (claim) from the page
  h.doc.activeElement = h.body;h.keeper.claim();
  assert.equal(h.doc.activeElement, h.composer);
  // a ▾ menu opened and shut by Esc (no render, only its toggle): the
  // keyboard left on its button goes back to the composer
  const summary = { tagName: 'SUMMARY', closest: () => null };
  h.page.over = true;h.fire('toggle');h.doc.activeElement = summary;
  h.page.over = false;h.fire('toggle');
  assert.equal(h.doc.activeElement, h.composer);
});

test('a click on Send, Stop or the top bar\'s Stop gives the keyboard back to the composer once let go', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  h.keeper.check();
  // the button takes the keyboard on the press; let go, the draft has it
  // back (the caret where it was), so the next words are not lost
  h.composer.setSelectionRange(3, 3);h.fire('keyup');
  for (const where of ['.composer-foot button', '.topbar .kv.state button']) {
    const button = { tagName: 'BUTTON', closest: selector => selector.split(',').includes(where) ? button : null };
    h.fire('pointerdown', button);h.doc.activeElement = button;h.fire('pointerup', button);
    assert.equal(h.doc.activeElement, h.composer);
    assert.deepEqual([h.composer.selectionStart, h.composer.selectionEnd], [3, 3]);
  }
  // the paperclip: a click puts the keyboard in its file field, which
  // takes no typing; let go, the draft has it
  const file = { tagName: 'INPUT', type: 'file', closest: selector => selector.split(',').includes('.composer-foot .attach') || selector.split(',').includes('input') ? file : null };
  h.fire('pointerdown', file);h.doc.activeElement = file;h.fire('pointerup', file);
  assert.equal(h.doc.activeElement, h.composer);
  // any other button (a reply's Copy) keeps the keyboard it took
  const copy = { tagName: 'BUTTON', closest: () => null };
  h.fire('pointerdown', copy);h.doc.activeElement = copy;h.fire('pointerup', copy);
  assert.equal(h.doc.activeElement, copy);
});

test('the composer never takes the keyboard from a field, during a press, over a selection or on a phone', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  const rename = { tagName: 'INPUT', closest: () => rename };
  h.doc.activeElement = rename;h.keeper.check();
  assert.equal(h.doc.activeElement, rename);
  h.doc.activeElement = h.body;h.page.thread = 't2';h.fire('pointerdown', h.body);h.keeper.check();
  assert.equal(h.doc.activeElement, h.body);
  h.page.selection = false;h.fire('pointerup', h.body);
  assert.equal(h.doc.activeElement, h.body);
  const phone = keeperHarness(installComposerFocus, { phone: true });
  phone.keeper.check();phone.keeper.claim();
  assert.equal(phone.doc.activeElement, phone.body);
  assert.equal(phone.keeper.end(), false);
});

test('explicit dismissal restores a narrow composer after the popover redraw, keeping its caret', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus, { phone: true });
  h.doc.activeElement = h.composer;h.composer.setSelectionRange(2, 2);h.fire('select');
  h.page.over = true;h.doc.activeElement = h.body;h.keeper.check();
  h.keeper.claim(true);
  assert.equal(h.doc.activeElement, h.body);
  h.page.over = false;h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  assert.deepEqual([h.composer.selectionStart, h.composer.selectionEnd], [2, 2]);
});

test('Escape closes a palette or drawing dialog before token stats underneath', async () => {
  const { installWorkspaceKeys } = await import('../../src/web/workspace-keys.js');
  let listener, draw = false, palette = true;
  const actions = [];
  const doc = { addEventListener: (_type, fn) => { listener = fn; }, querySelector: selector =>
    selector === '.draw-back' ? (draw ? {} : null) : selector === '.workspace-palette' ? (palette ? {} : null) :
    selector === '.tok-pop' ? {} : null };
  installWorkspaceKeys({ dispatch: (...a) => actions.push(a), document: doc, platform: 'Linux' });
  const press = () => listener(event('Escape', { preventDefault() {}, stopImmediatePropagation() {} }));
  press();assert.deepEqual(actions, [['ws-palette-close', '']]);
  draw = true;palette = false;press();assert.equal(actions.length, 1);
  draw = false;press();assert.deepEqual(actions[1], ['flag', 'tokstats']);
});

test('Escape cancels inline renaming after the innermost overlay is dismissed', async () => {
  const { overlayEscape } = await import('../../src/web/workspace-keys.js');
  let modal = false, cancelled = 0;
  const doc = { querySelectorAll: selector => selector.startsWith('.draw-back') ? [{ getClientRects: () => modal ? [{}] : [] }] : [],
    querySelector: selector => selector.includes('rename-no') ? { click() { cancelled++; } } : null };
  modal = true;assert.equal(overlayEscape(doc), null);
  modal = false;overlayEscape(doc)();assert.equal(cancelled, 1);
});

test('Escape dismisses the account picker before the inline editor', async () => {
  const { overlayEscape } = await import('../../src/web/workspace-keys.js');
  let closed = 0;
  const doc = { querySelectorAll: () => [], querySelector: selector =>
    selector === '.account-picker' ? {} : selector === '.account-chip' ? { click() { closed++; } } :
    selector.includes('rename-no') ? { click() { assert.fail('account picker goes first'); } } : null };
  overlayEscape(doc)();assert.equal(closed, 1);
});

test('Tab goes round a dialog: last to first, Shift+Tab first to last, the dialog itself to an end', async () => {
  const { dialogTab } = await import('../../src/web/workspace-keys.js');
  const edit = { id: 'edit' }, close = { id: 'close' }, body = { id: 'body' }, stops = [edit, close, body];
  assert.equal(dialogTab(body, false, stops), edit);
  assert.equal(dialogTab(edit, true, stops), body);
  assert.equal(dialogTab(edit, false, stops), null);
  assert.equal(dialogTab(close, true, stops), null);
  const dialog = { id: 'dialog' };
  assert.equal(dialogTab(dialog, false, stops), edit);
  assert.equal(dialogTab(dialog, true, stops), body);
  assert.equal(dialogTab(dialog, false, []), null);
});

test('an Esc that puts a popover away gives the composer the keyboard at once, from the popover\'s own field too', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  for (const where of ['.account-picker', '.title-box', '.workspace-palette', '.tok-pop']) {
    const h = keeperHarness(installComposerFocus, { phone: true });
    h.doc.activeElement = h.composer;h.composer.setSelectionRange(2, 2);h.fire('select');
    // the popover is still drawn (the hub's next view takes it away), the
    // keyboard in it (the rename's or the palette's field)
    const inner = { tagName: 'INPUT', closest: () => inner };
    h.page.cover = { getClientRects: () => [{}], matches: sel => sel.split(',').includes(where), contains: el => el === inner };
    h.doc.activeElement = inner;h.keeper.check();
    assert.equal(h.doc.activeElement, inner);
    h.keeper.claim(true);
    assert.equal(h.doc.activeElement, h.composer, where);
    assert.deepEqual([h.composer.selectionStart, h.composer.selectionEnd], [2, 2]);
  }
  // Settings (or a dialog) is not put away by that Esc: it keeps the keyboard
  const h = keeperHarness(installComposerFocus);
  h.page.cover = { getClientRects: () => [{}], matches: () => false, contains: () => false };
  h.keeper.claim(true);
  assert.equal(h.doc.activeElement, h.body);
});

test('the first view after a reload gives its draft the keyboard once it shows', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  // the page's frame holds the draft hidden until the workspace is whole
  h.page.hidden = true;h.keeper.check();
  assert.equal(h.doc.activeElement, h.body);
  // letters typed meanwhile wait for it, not for the rail
  assert.equal(h.key('x', h.body).rail, false);assert.equal(h.key('y', h.body).defaultPrevented, true);
  h.page.hidden = false;h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  assert.equal(h.composer.value, 'a draftxy');
  assert.equal(h.inputs.at(-1).data, 'xy');
  // once there, the rail's keys are the person's again after Esc from it
  h.keeper.leave();h.doc.activeElement = h.body;
  assert.equal(h.key('d', h.body).rail, true);
});

test('keys typed right after a press on Send or Stop go to the draft, not the rail or the button', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  h.keeper.check();h.keeper.leave();
  const send = { tagName: 'BUTTON', closest: selector => selector.split(',').includes('.composer-foot button') ? send : null };
  // the press is still down: no focus from the keeper yet; the first key
  // moves it into the draft before the key's own action (the letter lands there)
  h.fire('pointerdown', send);h.doc.activeElement = send;
  const m = h.key('m', send);
  assert.equal(h.doc.activeElement, h.composer);assert.equal(m.rail, false);assert.equal(m.defaultPrevented, false);
  // Enter right after a press does not press the button again
  h.fire('pointerdown', send);h.doc.activeElement = send;
  assert.equal(h.key('Enter', send).defaultPrevented, true);
  // a press elsewhere (the timeline) and a bare key: the rail's
  h.fire('pointerdown', h.body);h.fire('pointerup', h.body);h.doc.activeElement = h.body;
  assert.equal(h.key('d', h.body).rail, true);
});

test('keys typed right after a press on another thread wait for its composer, never the old one', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  h.keeper.check();h.keeper.leave();
  const row = { tagName: 'DIV', closest: selector => selector === '[data-on-click="select"]' ? row : null, getAttribute: () => 't2' };
  h.fire('pointerdown', row);h.doc.activeElement = h.body;h.fire('pointerup', row);
  for (const k of 'other') { const e = h.key(k, h.body);assert.equal(e.rail, false);assert.equal(e.defaultPrevented, true); }
  h.key('x', h.body);h.key('Backspace', h.body);
  // still t1's composer on show: it does not take them
  h.keeper.check();
  assert.equal(h.composer.value, 'a draft');assert.equal(h.doc.activeElement, h.body);
  // t2 shows (its own draft): there they go, at its caret
  h.page.thread = 't2';h.composer.value = '';h.keeper.check();
  assert.equal(h.doc.activeElement, h.composer);
  assert.equal(h.composer.value, 'other');
});

test('typing a moment ago, the keyboard fallen out of its field with a redraw keeps typing in the draft', async () => {
  const { installComposerFocus } = await import('../../src/web/workspace-keys.js');
  const h = keeperHarness(installComposerFocus);
  h.keeper.check();h.keeper.leave();
  h.key('a', h.composer);
  h.doc.activeElement = h.body;
  const e = h.key('b', h.body);
  assert.equal(e.rail, false);assert.equal(h.doc.activeElement, h.composer);
  // Esc out of the draft is on purpose: the rail's keys work
  h.key('Escape', h.composer);h.doc.activeElement = h.body;
  assert.equal(h.key('d', h.body).rail, true);
});
