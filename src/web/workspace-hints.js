import { workspaceShortcut } from './workspace-keys.js';

// The server exports the exact Side.projects/Side.here digit destinations.
// Missing DOM targets leave gaps; the host never sorts or renumbers them.
export function shortcutTargets(text) {
  try {
    const ids = JSON.parse(text);
    return Array.isArray(ids) && ids.every(id => typeof id === 'string') ? ids.slice(0, 9) : [];
  } catch { return []; }
}

export function shortcutHintMode(event, target, platform) {
  if (!event) return '';
  const action = workspaceShortcut({
    key: '1', code: 'Digit1', target, ctrlKey: event.ctrlKey, metaKey: event.metaKey,
    altKey: event.altKey, shiftKey: event.shiftKey, isComposing: event.isComposing,
    keyCode: event.keyCode, defaultPrevented: event.defaultPrevented,
    getModifierState: name => event.getModifierState?.(name),
  }, platform);
  return action?.[0] === 'ws-key' ? ({ 'alt+1': 'projects', 'ctrl+1': 'threads' }[action[1]] || '') : '';
}

export function installWorkspaceHints({ document: doc = document, window: win = window, platform = win.navigator.platform } = {}) {
  // (modPress: a press with Ctrl/Cmd still held, as on the legend's "all
  // shortcuts": the focus it moves keeps the hints up, so the click lands)
  let held, sidebar, composing = false, modPress = false;
  const annotated = new Set(), listeners = [];
  const listen = (object, type, handler) => {
    object.addEventListener(type, handler, true);
    listeners.push(() => object.removeEventListener(type, handler, true));
  };
  const set = (element, name, value) => {
    if (element.getAttribute(name) !== value) {
      if (value === null) element.removeAttribute(name); else element.setAttribute(name, value);
    }
  };
  // Only a dialog on screen covers the sidebar: the model picker stays in
  // the page, display:none, while a thread is open.
  const covered = () => [...doc.querySelectorAll('.tour,.workspace-palette,[role="dialog"],[aria-modal="true"]')]
    .some(element => element.getClientRects().length > 0);
  const mode = () => {
    if (composing || !doc.hasFocus() || doc.visibilityState !== 'visible' || doc.documentElement.dataset.backplaneRemoteReady === 'false') return '';
    const hint = shortcutHintMode(held, doc.activeElement, platform);
    return hint && !covered() ? hint : '';
  };
  const sync = () => {
    const next = doc.querySelector('.sidebar');
    if (sidebar !== next) { if (sidebar) set(sidebar, 'data-shortcut-hints', null); sidebar = next; }
    const desired = new Map();
    if (sidebar) {
      const projects = new Map(shortcutTargets(sidebar.getAttribute('data-shortcut-projects')).map((id, i) => [id, String(i + 1)]));
      const threads = new Map(shortcutTargets(sidebar.getAttribute('data-shortcut-threads')).map((id, i) => [id, String(i + 1)]));
      const bind = (element, kind, digit) => { if (element && digit) desired.set(element, { kind, digit }); };
      for (const head of sidebar.querySelectorAll('.project-head[data-value]')) {
        bind(head.querySelector('.name'), 'projects', projects.get(head.getAttribute('data-value')));
      }
      for (const row of sidebar.querySelectorAll('.thread[data-on-click="select"][data-value]')) {
        bind(row.querySelector('.title'), 'threads', threads.get(row.getAttribute('data-value')));
        // Active view names its project in the note, except when an agent
        // count replaces that name. Never label an agent count as a project.
        bind(row.querySelector('.ago:not(.agents)'), 'projects', projects.get(row.getAttribute('data-shortcut-project')));
      }
    }
    for (const element of annotated) if (!desired.has(element)) {
      set(element, 'data-shortcut-kind', null); set(element, 'data-shortcut-digit', null);
    }
    annotated.clear();
    for (const [element, { kind, digit }] of desired) {
      set(element, 'data-shortcut-kind', kind); set(element, 'data-shortcut-digit', digit); annotated.add(element);
    }
    show();
  };
  // Every keycap on the page (the rail's, the top bar's Ctrl K, the
  // sidebar's legend) shows only while Ctrl (Cmd on a Mac) is held where
  // its shortcuts work: <html data-keys-held> (style.css)
  const show = () => {
    const hint = mode();
    if (sidebar) set(sidebar, 'data-shortcut-hints', hint || null);
    set(doc.documentElement, 'data-keys-held', hint === 'threads' ? '1' : null);
  };
  const reset = () => { held = null; if (sidebar) set(sidebar, 'data-shortcut-hints', null); set(doc.documentElement, 'data-keys-held', null); };
  const keys = event => { modPress = false; held = event; sync(); };
  const press = event => {
    modPress = !!(event.ctrlKey || event.metaKey);
    if (modPress) { held = event; sync(); } else reset();
  };
  const moved = () => { if (modPress) sync(); else reset(); };
  listen(win, 'keydown', keys); listen(win, 'keyup', keys); listen(win, 'pointerdown', press);
  // (a capturing blur also hears each element's blur: only the window's own
  // ends a press)
  listen(win, 'blur', event => { if (event.target === win || event.target == null) { modPress = false; reset(); } else moved(); });
  listen(doc, 'visibilitychange', () => { modPress = false; reset(); });
  listen(doc, 'focusin', moved); listen(doc, 'focusout', moved);
  listen(doc, 'compositionstart', () => { composing = true; reset(); });
  listen(doc, 'compositionend', () => { composing = false; reset(); });
  const observer = new win.MutationObserver((records = []) => {
    // Streaming timeline updates should not rescan every sidebar label.
    if (!records.length || !sidebar?.isConnected || records.some(record => record.target === sidebar || sidebar.contains(record.target))) sync();
    else show();
  });
  observer.observe(doc.documentElement, { childList: true, subtree: true, attributes: true,
    attributeFilter: ['data-shortcut-projects', 'data-shortcut-threads', 'data-backplane-remote-ready', 'data-shortcut-project', 'class'] });
  sync();
  return () => {
    reset(); observer.disconnect(); for (const remove of listeners) remove();
    for (const element of annotated) { set(element, 'data-shortcut-kind', null); set(element, 'data-shortcut-digit', null); }
    annotated.clear();
  };
}
