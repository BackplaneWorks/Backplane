// DOM transport only. The integrator owns markup, selected owner/tab, and RPC
// authorization. No browser profile, filesystem, or native API is used here.
//
// getBinding(frame) -> {page, tabId, generation?}; generation should change on
// navigation/replacement. isCurrent(binding, frame) must also check Teach.input.
// dispatch(action, stringValue, binding) synchronously enqueues an owner-bound
// action. The server must independently validate owner, active tab and drive.
//
// pointerAction / wheelAction are opt-in NEW transports, never aliases for the
// old click/vertical-scroll actions. Mouse down/up carry the OS click count;
// backend mouse.down/up must preserve it. No extra click is synthesized.
// Pointer cancellation must release only this gesture's buttons; the backend
// must also cancel gestures on owner/tab changes and disconnects.
//
// getCopyText(binding) is an optional SYNCHRONOUS, current remote-selection
// cache. Returning a Promise, image text or proxy contents is not supported.
export function installBrowserInput({
  dispatch, getBinding, isCurrent, root = globalThis.document,
  frameSelector = 'img[data-browser-input]', pointerAction = null,
  wheelAction = null, getCopyText = null, onUnsupported = () => {},
  getContentRect = frame => frame.getBoundingClientRect(),
  primaryModifier = 'Control', linePixels = 16,
} = {}) {
  if (!root || !dispatch || !getBinding || !isCurrent)
    throw new TypeError('Browser input needs root, dispatch, getBinding and isCurrent');
  const doc = root.ownerDocument || root;
  const win = doc.defaultView;
  const proxy = doc.createElement('textarea');
  proxy.setAttribute('aria-label', 'Browser page text input');
  proxy.setAttribute('autocomplete', 'off');
  proxy.setAttribute('autocapitalize', 'off');
  proxy.setAttribute('spellcheck', 'false');
  proxy.tabIndex = -1;
  // Remain focusable and on-screen for the OS IME/candidate window. The click
  // location is an approximation until the backend provides caret geometry.
  Object.assign(proxy.style, {position: 'fixed', width: '1px', height: '18px',
    opacity: '0.01', pointerEvents: 'none', padding: '0', border: '0',
    left: '0px', top: '0px', resize: 'none'});
  (root === doc ? (doc.body || doc.documentElement) : root).appendChild(proxy);
  let active = null, composition = null, drag = null, press = null, sequence = 0, eventSequence = 0, disposed = false;
  const listeners = [];
  const note = kind => onUnsupported(kind); // Never pass typed/clipboard data.
  const same = (a, b) => a && b && a.page === b.page && a.tabId === b.tabId && a.generation === b.generation;
  function binding(frame) {
    const b = getBinding(frame);
    if (!b || typeof b.page !== 'string' || !b.page || typeof b.tabId !== 'string' || !b.tabId) return null;
    if (b.generation !== undefined && !['string', 'number'].includes(typeof b.generation)) return null;
    return Object.freeze({page: b.page, tabId: b.tabId, ...(b.generation === undefined ? {} : {generation: b.generation})});
  }
  function valid(s) {
    return !disposed && s && s.frame.isConnected && root.contains(s.frame) &&
      same(s.binding, binding(s.frame)) && isCurrent(s.binding, s.frame);
  }
  function state(target) {
    if (target === proxy) return valid(active) ? active : null;
    const frame = target?.closest?.(frameSelector);
    if (!frame || !root.contains(frame)) return null;
    const s = {frame, binding: binding(frame)};
    return valid(s) ? s : null;
  }
  const consume = e => { e.preventDefault(); e.stopImmediatePropagation(); };
  function send(s, action, value) {
    if (!valid(s)) return false;
    return dispatch(action, value, s.binding) !== false;
  }
  const action = (s, attr, fallback) => s.frame.getAttribute(attr) || fallback;
  function text(s, value) {
    if (value) send(s, action(s, 'data-type', 'bb-type'), value);
  }
  function key(s, value) {
    send(s, action(s, 'data-keyto', 'bb-key'), value);
  }
  function point(s, e) {
    const r = getContentRect(s.frame);
    if (!(r.width > 0 && r.height > 0) || !Number.isFinite(e.clientX) || !Number.isFinite(e.clientY)) return null;
    const scale = (v, origin, size) => Math.max(0, Math.min(1000, Math.round((v - origin) * 1000 / size)));
    return {x: scale(e.clientX, r.left, r.width), y: scale(e.clientY, r.top, r.height)};
  }
  function pointer(s, payload) {
    if (pointerAction) send(s, pointerAction, JSON.stringify({...s.binding, unit: 'per-mille', sequence: ++eventSequence, ...payload}));
  }
  function cancelDrag() {
    if (drag) {
      // A stale UI must never release a button against the newly selected page.
      // Backend owner/tab teardown is responsible for the abandoned old gesture.
      if (valid(drag)) pointer(drag, {type: 'cancel', gesture: drag.id, button: drag.button});
      else note('stale-pointer');
      drag = null;
    }
  }
  function clear() {
    composition = null;
    proxy.value = '';
  }
  const clickCount = e => Math.max(1, Math.min(3, Math.floor(e.detail || 1)));
  function activate(s, e) {
    if (!valid(s)) return;
    if (!active || active.frame !== s.frame || !same(active.binding, s.binding)) {
      cancelDrag(); clear(); active = s;
    }
    if (e && point(s, e)) {
      proxy.style.left = `${Math.max(0, e.clientX)}px`;
      proxy.style.top = `${Math.max(0, e.clientY)}px`;
    }
    proxy.focus({preventScroll: true});
  }
  function flush() {
    if (!active || !valid(active)) { clear(); return; }
    if (composition) return;
    const value = proxy.value;
    proxy.value = '';
    text(active, value);
  }
  function on(target, type, fn, options = true) {
    if (!target) return;
    target.addEventListener(type, fn, options);
    listeners.push(() => target.removeEventListener(type, fn, options));
  }
  on(root, 'focusin', e => {
    if (e.target !== proxy) { const s = state(e.target); if (s) activate(s); }
  });
  on(root, 'compositionstart', e => {
    if (e.target !== proxy) return;
    if (!valid(active)) { clear(); return; }
    composition = active;
  });
  on(root, 'compositionend', e => {
    if (e.target !== proxy) return;
    const saved = composition;
    composition = null;
    if (!saved || saved !== active || !valid(saved) || e.data === '') { clear(); return; }
    // Read the actual editing surface, not InputEvent.data. Clearing it also
    // makes the browser's trailing composition input notification a no-op.
    flush();
  });
  on(root, 'input', e => {
    if (e.target === proxy && !e.isComposing) flush();
  });
  const editingKeys = new Set(['Enter', 'Backspace', 'Tab', 'Escape', 'Delete',
    'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown']);
  on(root, 'keydown', e => {
    const s = state(e.target);
    if (!s) { if (e.target === proxy) clear(); return; }
    if (e.isComposing || e.keyCode === 229 || composition) return;
    if (e.target !== proxy) activate(s);
    const shortcut = (e.ctrlKey || e.metaKey) && !e.altKey;
    // Let standard ClipboardEvents reach the proxy. Never forward host paste
    // shortcuts to a different process's unrelated clipboard.
    if (shortcut && ['c', 'v', 'x'].includes(e.key.toLowerCase())) return;
    if (e.getModifierState?.('AltGraph')) return;
    if (editingKeys.has(e.key) || /^F\d{1,2}$/.test(e.key) || ((e.ctrlKey || e.metaKey || e.altKey) && e.key.length === 1)) {
      consume(e); flush();
      const mods = [...(e.ctrlKey || e.metaKey ? [primaryModifier] : []), ...(e.altKey ? ['Alt'] : []), ...(e.shiftKey ? ['Shift'] : [])];
      key(s, [...mods, e.key].join('+'));
    }
    // Printable characters, including emoji, go through native textarea input.
  });
  on(root, 'beforeinput', e => {
    if (e.target !== proxy || e.isComposing || composition) return;
    const k = {deleteContentBackward: 'Backspace', deleteContentForward: 'Delete',
      insertLineBreak: 'Enter', insertParagraph: 'Enter', historyUndo: `${primaryModifier}+z`,
      historyRedo: `${primaryModifier}+Shift+z`}[e.inputType];
    if (k) { consume(e); flush(); if (valid(active)) key(active, k); }
  });
  on(root, 'paste', e => {
    const s = state(e.target); if (!s) return;
    consume(e); clear();
    if (!e.clipboardData) { note('clipboard-unavailable'); return; }
    const value = e.clipboardData.getData('text/plain');
    if (value) text(s, value);
    else if (e.clipboardData.files?.length) note('file-paste');
  });
  on(root, 'copy', e => {
    const s = state(e.target); if (!s) return;
    consume(e);
    const value = getCopyText?.(s.binding);
    if (typeof value !== 'string' || !valid(s) || !e.clipboardData) { note('copy-selection'); return; }
    e.clipboardData.setData('text/plain', value);
  });
  on(root, 'cut', e => { if (state(e.target)) { consume(e); note('cut-selection'); } });
  const buttons = ['left', 'middle', 'right'];
  // Mouse compatibility events carry OS click-count detail (pointerdown often
  // has detail=0). They provide down/move/up without guessing a double-click
  // timer or adding an extra click after the second physical button release.
  on(root, 'mousedown', e => {
    const s = state(e.target); if (!s || !buttons[e.button]) return;
    const p = point(s, e); if (!p) return;
    consume(e);
    if (drag) { note('multiple-buttons'); return; }
    press = s;
    // A field-changing click cannot leave preedit queued for another field.
    clear(); activate(s, e);
    if (pointerAction) {
      drag = {...s, id: String(++sequence), button: buttons[e.button]};
      pointer(drag, {type: 'down', gesture: drag.id, button: drag.button, clickCount: clickCount(e), ...p});
    }
  });
  on(root, 'mousemove', e => {
    if (!pointerAction) return;
    const s = drag || state(e.target); if (!s) return;
    if (!valid(s)) { cancelDrag(); return; }
    const p = point(s, e); if (!p) return;
    if (drag) consume(e);
    pointer(s, {type: 'move', gesture: drag?.id || null, ...p});
  });
  on(root, 'mouseup', e => {
    if (!drag || buttons[e.button] !== drag.button) return;
    consume(e);
    const saved = drag; drag = null;
    const p = point(saved, e);
    if (p) pointer(saved, {type: 'up', gesture: saved.id, button: saved.button, clickCount: clickCount(e), ...p});
    else if (valid(saved)) pointer(saved, {type: 'cancel', gesture: saved.id, button: saved.button});
  });
  on(root, 'click', e => {
    const s = state(e.target);
    if (!s) { press = null; return; }
    consume(e);
    const down = press; press = null;
    if (down && (!valid(down) || down.frame !== s.frame || !same(down.binding, s.binding))) return;
    if (pointerAction) return;
    if (e.button !== 0 || e.detail > 1) { note('rich-pointer'); return; }
    const p = point(s, e); if (!p) return;
    activate(s, e);
    send(s, action(s, 'data-pt', 'bb-click'), `${p.x} ${p.y}`);
  });
  for (const type of ['contextmenu', 'auxclick', 'dblclick', 'dragstart'])
    on(root, type, e => { if (state(e.target)) { consume(e); if (!pointerAction) note('rich-pointer'); } });
  on(root, 'wheel', e => {
    const s = state(e.target); if (!s) return;
    const p = point(s, e); if (!p) return;
    consume(e);
    const r = getContentRect(s.frame);
    const delta = (value, size) => Math.round(Math.max(-600, Math.min(600,
      (Number.isFinite(value) ? value : 0) * (e.deltaMode === 1 ? linePixels : e.deltaMode === 2 ? size : 1))));
    const dx = delta(e.deltaX, r.width), dy = delta(e.deltaY, r.height);
    if (wheelAction) send(s, wheelAction, JSON.stringify({...s.binding, unit: 'per-mille', deltaUnit: 'pixel', ...p, dx, dy}));
    else { if (dx) note('horizontal-wheel'); if (dy) send(s, action(s, 'data-wheel', 'bb-scroll'), String(dy)); }
  }, {capture: true, passive: false});
  on(win, 'blur', () => { cancelDrag(); clear(); active = null; press = null; });
  on(doc, 'visibilitychange', () => { if (doc.hidden) { cancelDrag(); clear(); active = null; press = null; } });
  return {
    element: proxy,
    refresh() {
      if (active && !valid(active)) { cancelDrag(); clear(); active = null; proxy.blur(); }
    },
    dispose() {
      if (disposed) return;
      cancelDrag(); clear(); active = null; press = null;
      disposed = true;
      for (const off of listeners) off();
      proxy.remove();
    },
  };
}
