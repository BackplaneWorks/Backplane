const { test } = require('node:test'), assert = require('node:assert/strict');
const input = fields => ({ ctrlKey:false, altKey:false, metaKey:false, shiftKey:false, ...fields });
function element(attrs = {}) {
  const values = new Map(Object.entries(attrs));
  return { getAttribute: name => values.get(name) ?? null,
    setAttribute: (name, value) => values.set(name, value), removeAttribute: name => values.delete(name) };
}
function emitter(object = {}) {
  const listeners = new Map();
  object.addEventListener = (type, handler) => { if (!listeners.has(type)) listeners.set(type, new Set());listeners.get(type).add(handler); };
  object.removeEventListener = (type, handler) => listeners.get(type)?.delete(handler);
  object.emit = (type, value = {}) => { for (const handler of listeners.get(type) || []) handler(value); };
  return object;
}
function harness(installWorkspaceHints) {
  const label = () => element(), p2 = label(), p1 = label(), parent = label(), child = label(), excluded = label(), activeProject = label();
  const head = (id, name) => ({ ...element({'data-value':id}), querySelector: () => name });
  const row = (id, title, pid) => ({ ...element({'data-value':id, ...(pid ? {'data-shortcut-project':pid} : {})}),
    querySelector: selector => selector === '.title' ? title : pid ? activeProject : null });
  const sidebar = element({'data-shortcut-projects':'["absent","p2","p1"]', 'data-shortcut-threads':'["parent","child"]'});
  sidebar.querySelectorAll = selector => selector.startsWith('.project-head') ? [head('p1', p1), head('p2', p2)] :
    [row('child', child, 'p2'), row('excluded', excluded), row('parent', parent)];
  const body = { closest: () => null };
  // dialogs in the page: shown ones have boxes, hidden ones (display:none) none
  const dialog = shown => ({ getClientRects: () => shown ? [{}] : [] });
  const doc = emitter({ activeElement:body, visibilityState:'visible', focused:true, blocked:false, hiddenDialog:false,
    documentElement:{dataset:{backplaneRemoteReady:'true'}}, hasFocus: () => doc.focused,
    querySelector: selector => selector === '.sidebar' ? sidebar : null,
    querySelectorAll: () => [...(doc.hiddenDialog ? [dialog(false)] : []), ...(doc.blocked ? [dialog(true)] : [])] });
  let observer;
  const win = emitter({ navigator:{platform:'Linux'}, MutationObserver:class {
    constructor(callback) { observer = callback; } observe() {} disconnect() {}
  } });
  const dispose = installWorkspaceHints({document:doc, window:win});
  return { doc, win, body, sidebar, p1, p2, parent, child, excluded, activeProject, dispose,
    refresh:() => observer(), mode:() => sidebar.getAttribute('data-shortcut-hints'),
    key:(type, fields) => win.emit(type, input(fields)) };
}

test('shortcut hints keep authoritative digits despite DOM order, missing IDs and target refresh', async () => {
  const { installWorkspaceHints, shortcutTargets } = await import('../../src/web/workspace-hints.js');
  const h = harness(installWorkspaceHints);
  assert.equal(h.p1.getAttribute('data-shortcut-digit'), '3');
  assert.equal(h.p2.getAttribute('data-shortcut-digit'), '2');
  assert.equal(h.activeProject.getAttribute('data-shortcut-digit'), '2');
  assert.equal(h.child.getAttribute('data-shortcut-digit'), '2');
  assert.equal(h.parent.getAttribute('data-shortcut-digit'), '1');
  assert.equal(h.excluded.getAttribute('data-shortcut-digit'), null);
  assert.equal(h.mode(), null);
  h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), 'threads');
  // An authoritative collapse removes the child even while Ctrl stays held.
  h.sidebar.setAttribute('data-shortcut-threads', '["parent"]');h.refresh();
  assert.equal(h.child.getAttribute('data-shortcut-digit'), null);
  assert.equal(h.mode(), 'threads');
  // An authoritative search/expanded/shelf projection is used as-is.
  h.sidebar.setAttribute('data-shortcut-threads', '["excluded","child","parent"]');h.refresh();
  assert.equal(h.excluded.getAttribute('data-shortcut-digit'), '1');
  assert.equal(h.child.getAttribute('data-shortcut-digit'), '2');
  assert.equal(h.parent.getAttribute('data-shortcut-digit'), '3');
  assert.equal(shortcutTargets(JSON.stringify(Array.from({length:12}, (_, i) => String(i)))).length, 9);
  assert.deepEqual(shortcutTargets('["valid",null]'), []);assert.deepEqual(shortcutTargets('bad'), []);
  h.dispose();assert.equal(h.mode(), null);assert.equal(h.parent.getAttribute('data-shortcut-digit'), null);
});

test('modifier hints inherit navigation editor/PTY/IME/platform guards without consuming keys', async () => {
  const { installWorkspaceHints, shortcutHintMode } = await import('../../src/web/workspace-hints.js');
  const h = harness(installWorkspaceHints);
  const forbid = () => assert.fail('Hints never prevent input or dispatch navigation');
  h.key('keydown', {altKey:true, preventDefault:forbid, stopImmediatePropagation:forbid});assert.equal(h.mode(), 'projects');
  h.key('keyup', {});assert.equal(h.mode(), null);
  for (const fields of [{ctrlKey:true,shiftKey:true}, {ctrlKey:true,altKey:true},
    {ctrlKey:true,isComposing:true}, {ctrlKey:true,keyCode:229}, {altKey:true,getModifierState:() => true}]) {
    h.key('keydown', fields);assert.equal(h.mode(), null);
  }
  // Ctrl+1..9 works from the composer and other text fields, so Ctrl lights
  // the thread digits there; Alt+digit stays with the field, and a PTY keeps both
  const field = (id, pty) => ({id, closest:selector => !pty && selector === '[data-keys],[data-keyto]' ? null : {}});
  for (const id of ['composer','search']) {
    h.doc.activeElement = field(id, false);
    h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), 'threads');
    h.key('keydown', {altKey:true});assert.equal(h.mode(), null);
  }
  h.doc.activeElement = field('term', true);
  h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), null);
  h.key('keydown', {altKey:true});assert.equal(h.mode(), null);
  h.doc.activeElement = h.body;
  h.doc.emit('compositionstart');h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), null);
  h.doc.emit('compositionend');assert.equal(h.mode(), null);
  h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), 'threads');
  assert.equal(shortcutHintMode(input({metaKey:true}), h.body, 'MacIntel'), 'threads');
  assert.equal(shortcutHintMode(input({ctrlKey:true}), h.body, 'MacIntel'), '');
  h.dispose();
});

test('modifier hints clear on keyup, focus, blur, visibility, dialogs and disconnected readiness', async () => {
  const { installWorkspaceHints } = await import('../../src/web/workspace-hints.js');
  const h = harness(installWorkspaceHints);
  for (const [object, type] of [[h.win,'blur'],[h.doc,'focusout'],[h.doc,'focusin'],[h.doc,'visibilitychange']]) {
    h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), 'threads');
    object.emit(type);assert.equal(h.mode(), null);h.refresh();assert.equal(h.mode(), null);
  }
  h.doc.blocked = true;h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), null);
  h.doc.blocked = false;h.doc.focused = false;h.refresh();assert.equal(h.mode(), null);
  h.doc.focused = true;h.doc.visibilityState = 'hidden';h.refresh();assert.equal(h.mode(), null);
  h.doc.visibilityState = 'visible';h.doc.documentElement.dataset.backplaneRemoteReady = 'false';h.refresh();assert.equal(h.mode(), null);
  h.doc.documentElement.dataset.backplaneRemoteReady = 'true';h.key('keydown', {altKey:true});assert.equal(h.mode(), 'projects');
  h.key('keyup', {});assert.equal(h.mode(), null);h.dispose();
});

test('a dialog kept hidden in the page (the model picker) leaves the hints on; a shown one turns them off', async () => {
  const { installWorkspaceHints } = await import('../../src/web/workspace-hints.js');
  const h = harness(installWorkspaceHints);
  h.doc.hiddenDialog = true;
  h.key('keydown', {ctrlKey:true});assert.equal(h.mode(), 'threads');
  h.key('keyup', {});assert.equal(h.mode(), null);
  h.key('keydown', {altKey:true});assert.equal(h.mode(), 'projects');
  h.doc.blocked = true;h.refresh();assert.equal(h.mode(), null);
  h.doc.blocked = false;h.refresh();assert.equal(h.mode(), 'projects');
  h.dispose();
});
