// Event normalization only; Bend owns navigation order and action decisions.
const EDITOR = 'input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[data-keys],[data-keyto]';
// A PTY (the terminal, an embedded one) or a bot's browser frame: every key
// it gets is its own (Ctrl+K kills a line, Ctrl+D ends the shell).
const PTY = '[data-keys],[data-keyto]';

// A bare letter or digit (no modifier, outside every editor) may be one of
// the rail's keycaps (view.bend writes data-keycap on the button that shows
// it): the host presses that button, so the key does what a click does.
// '' for any other key.
export function railKey(event) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat || event.getModifierState?.('AltGraph')) return '';
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return '';
  const key = typeof event.key === 'string' ? event.key.toLowerCase() : '';
  if (!/^[a-z0-9]$/.test(key) || event.target?.closest?.(EDITOR)) return '';
  return key;
}

export function workspaceShortcut(event, platform = '') {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.getModifierState?.('AltGraph')) return null;
  const key = event.key?.toLowerCase(), mac = /mac/i.test(platform);
  const command = mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  // Explicit command entry/toggle chords also work from an editor or PTY.
  if (command && event.shiftKey && !event.altKey && key === 'p') return ['ws-palette-open', ''];
  if (!mac && event.metaKey && !event.ctrlKey && !event.altKey &&
      (event.code === 'Backquote' || key === '`' || key === '~')) return ['ws-key', 'term'];
  if (command && !event.shiftKey && !event.altKey && key === 'j') return ['ws-key', 'term'];
  if (command && !event.shiftKey && !event.altKey && key === 'b') return ['side-toggle', ''];
  // Ctrl+M (Ctrl on every platform: Cmd+M minimizes on macOS) toggles the
  // model picker from the workspace or the composer, never from a PTY
  // (Ctrl+M is Enter there) or another field.
  if (event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey && key === 'm') {
    const field = event.target?.closest?.('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[data-keys],[data-keyto]');
    if (!field || (field.tagName === 'TEXTAREA' && field.id === 'composer') || field.id === 'model-q') return ['flag', 'models'];
  }
  const editor = event.target?.closest?.(EDITOR);
  // The requested bare backtick works from the composer/workspace. Keep
  // literal backticks available in shells and other text fields.
  if (key === '`' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey &&
      (!editor || (editor.tagName === 'TEXTAREA' && editor.id === 'composer' &&
        !editor.hasAttribute?.('data-keys') && !editor.hasAttribute?.('data-keyto')))) return ['ws-key', 'term'];
  // The Ctrl/Cmd chords below work from the composer and every other text
  // field too: none of them edits text there (Ctrl+A/C/V/X/Z/Y, the arrows,
  // Home/End, Backspace/Delete and Ctrl+Enter stay the field's). A PTY keeps
  // all of them; bare keys and Alt+digit (Option+1 types ¡ on a Mac) stay
  // off in every field.
  if (editor && event.target?.closest?.(PTY)) return null;
  if (command && !event.altKey && event.shiftKey) {
    if (key === 'u') return ['ws-key', 'uber'];
    if (event.code === 'BracketLeft' || key === '[' || key === '{') return ['ws-key', 'prev'];
    if (event.code === 'BracketRight' || key === ']' || key === '}') return ['ws-key', 'next'];
    if (key === 's') return ['ws-key', 'settle'];
    if (key === 'f') return ['thin-search-open', ''];
  }
  if (command && !event.altKey && !event.shiftKey) {
    const action = { k: 'palette', n: 'new', d: 'diff', p: 'files' }[key];
    if (action) return ['ws-key', action];
    if (/^[1-9]$/.test(key)) return ['ws-key', 'ctrl+' + key];
  }
  if (!editor && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && /^[1-9]$/.test(key)) return ['ws-key', 'alt+' + key];
  return null;
}

// The open model picker owns these keys wherever the focus is (the
// composer, its search, the chip): Bend moves its cursor (mp-key, core's
// modelpick.bend). Escape shuts it like a click on the chip; in its search
// field the side arrows stay with the text.
export function modelPickerKey(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  if (event.key === 'Escape' && !event.shiftKey) return ['flag', 'models'];
  if (event.key === 'Tab') return ['mp-key', event.shiftKey ? 'stab' : 'tab'];
  if (event.shiftKey) return null;
  const inSearch = event.target?.id === 'model-q';
  const key = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: inSearch ? '' : 'left', ArrowRight: inSearch ? '' : 'right', Enter: 'enter' }[event.key];
  return key ? ['mp-key', key] : null;
}

// Esc puts away what is open over the page wherever the keyboard is: what
// the hub drew with a shade or a backdrop that a click beside it presses (a
// row's or a project's menu, the badge editor, a project's Intent), the
// topmost (the last) first; else an open ▾ menu. The model picker keeps its
// own Escape. The answer closes it, or is null when nothing is open.
export function overlayEscape(doc) {
  const shades = [...(doc.querySelectorAll?.('.shade[data-on-click], .modal-back[data-on-click]') || [])]
    .filter(el => el.getAttribute('data-value') !== 'models');
  const shade = shades.at(-1);
  if (shade) return () => shade.click();
  const menu = doc.querySelector?.('.local-menu[open]');
  if (menu) return () => { menu.open = false; };
  return null;
}

// A bare "/" (Shift allowed: some layouts need it) outside every text
// field puts the keyboard in the composer, at the end of its draft; in the
// composer or any field it types.
export function slashKey(event) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat) return false;
  if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey) return false;
  return !event.target?.closest?.(EDITOR);
}

// What owns Esc itself, on screen: the tour, the lightbox, Settings, the
// drawing pad, a dialog (the search results, the model picker); and the
// subagent inspector open under the keyboard (subagent-inspector.js).
const ESC_OWNERS = '.tour,.lightbox,#settings-content,.draw-back,[role="dialog"],[aria-modal="true"]';
const shown = element => !element.getClientRects || element.getClientRects().length > 0;
// The viewer's close: the rail's (or the narrow bar's) and the viewer's ×
const VIEWER_CLOSE = '.workspace-close [data-on-click="panel"],.workspace-bar .viewer-close,.viewer-tabs [data-on-click="panel"]';

// Esc (or Shift+Esc) once no menu, picker or palette took it: what the
// page has open that Esc may put away (workspace.bend's WS.esc.next picks
// which), and where the keyboard is. null when the key is not this one's:
// another modifier, a dialog or the tour on screen, Esc inside a PTY (the
// terminal's program has it: Shift+Esc hides the floating terminal
// instead), another text field, the composer's completions.
export function escapeState(event, doc) {
  if (event.key !== 'Escape' || event.ctrlKey || event.metaKey || event.altKey || event.isComposing || event.keyCode === 229) return null;
  const target = event.target, pty = target?.closest?.(PTY);
  if (event.shiftKey) return pty && target.closest('.floating-terminal') ? { shift: true } : null;
  if (pty) return null;
  const editor = target?.closest?.(EDITOR);
  const composer = !!editor && editor.tagName === 'TEXTAREA' && editor.id === 'composer';
  if (editor && !composer || composer && editor.getAttribute?.('data-esc')) return null;
  if (target?.closest?.('.subs') || [...(doc.querySelectorAll?.(ESC_OWNERS) || [])].some(shown)) return null;
  return { shift: false, composer, term: !!doc.querySelector?.('.floating-terminal'),
    probe: !!doc.querySelector?.('.probe-chip'), viewer: !!doc.querySelector?.(VIEWER_CLOSE) };
}

// Whenever a thread is on screen and nothing else has the keyboard, it is
// in the composer: on load, when another thread comes on show, when what
// covered the page goes (Settings, a dialog, the palette, a menu, the
// picker, the lightbox, the terminal) or the viewer shuts, after a
// reconnect and on Esc. Never from a field the person types in, during a
// press (a selection being made) or over a selection outside the composer,
// nor on a phone-sized or touch page (its keyboard would cover the
// thread). The caret goes back where it was in that thread's draft.
const COVERS = '.tour,.lightbox,.workspace-palette,#settings-content,.draw-back,#thin-result,[role="dialog"],[aria-modal="true"],.shade,.modal-back,.local-menu[open],.floating-terminal,.tok-pop';
export function installComposerFocus({ document: doc = document, window: win = window,
  phone = () => win.matchMedia?.('(max-width: 760px), (hover: none) and (pointer: coarse)').matches } = {}) {
  let want = true, thread = '', covered = false, viewer = false, pressed = false;
  const carets = new Map();
  const field = () => doc.getElementById?.('composer');
  const here = () => doc.querySelector?.('main.thread-view[data-thread]')?.getAttribute('data-thread') || '';
  const keep = () => {
    const el = field();
    if (el && doc.activeElement === el && here()) carets.set(here(), [el.selectionStart, el.selectionEnd]);
  };
  const put = (el, at) => {
    el.focus({ preventScroll: true });
    const n = el.value.length, [start, end] = at || [n, n];
    try { el.setSelectionRange(Math.min(start, n), Math.min(end, n)); } catch {}
  };
  const busy = el => !!el && el !== doc.body && !!el.closest?.(EDITOR);
  const check = () => {
    const el = field(), now = here();
    const over = [...(doc.querySelectorAll?.(COVERS) || [])].some(shown);
    const open = !!doc.querySelector?.(VIEWER_CLOSE);
    if (now && now !== thread) {
      want = true;
      // the composer kept the keyboard while its draft changed thread
      if (el && doc.activeElement === el && carets.has(now)) put(el, carets.get(now));
    }
    if (covered && !over || viewer && !open) want = true;
    thread = now || thread;covered = over;viewer = open;
    if (!want || over || pressed || !el || el.disabled || !shown(el)) return;
    want = false;
    const active = doc.activeElement;
    if (active === el || busy(active) || phone()) return;
    const selection = win.getSelection?.();
    if (selection && !selection.isCollapsed && !el.contains(selection.anchorNode)) return;
    put(el, carets.get(now));
  };
  doc.addEventListener('pointerdown', () => { pressed = true; }, true);
  // a menu (a <details>) opened or shut by Esc, a click or its own item:
  // no render follows, so the cover is looked at once it has toggled
  doc.addEventListener('toggle', () => win.setTimeout(check, 0), true);
  const up = () => { if (pressed) { pressed = false;win.setTimeout(check, 0); } };
  doc.addEventListener('pointerup', up, true);doc.addEventListener('pointercancel', up, true);
  for (const name of ['keyup', 'input', 'pointerup', 'focusout', 'select']) doc.addEventListener(name, event => { if (event.target === field()) keep(); }, true);
  // a dialog, the lightbox or the drawing pad come and go on the body
  if (win.MutationObserver && doc.body) new win.MutationObserver(check).observe(doc.body, { childList: true });
  return {
    check,
    // Esc, a reconnect: the keyboard goes there now if it can
    claim() { want = true;check(); },
    // "/": to the end of the draft
    end() { const el = field();if (el && !phone()) { want = false;put(el); return true; } return false; },
  };
}

export function installWorkspaceKeys({ dispatch, document: doc = document, platform = navigator.platform, escNext = null, clearProbe = null, composer = null }) {
  doc.addEventListener('keydown', event => {
    if (event.isComposing || event.keyCode === 229 || event.defaultPrevented) return;
    if (doc.querySelector('.tour,.lightbox')) return; // The active overlay owns every key.
    const models = doc.querySelector('[data-local-models="1"]');
    if (models && models.style.display !== 'none') {
      const action = modelPickerKey(event);
      if (action) { event.preventDefault(); event.stopImmediatePropagation(); dispatch(...action); return; }
    }
    // The token stats panel (view.bend's View.tok.pop) shuts on Escape like
    // the picker, as a click on its chip does
    if (event.key === 'Escape' && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && doc.querySelector('.tok-pop')) {
      event.preventDefault(); event.stopImmediatePropagation(); dispatch('flag', 'tokstats'); return;
    }
    const palette = doc.querySelector('.workspace-palette');
    if (palette) {
      const action = { Escape: 'ws-palette-close', Enter: 'ws-palette-enter', ArrowUp: 'ws-palette-move', ArrowDown: 'ws-palette-move' }[event.key];
      if (action && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
        event.preventDefault(); event.stopImmediatePropagation();
        dispatch(action, event.key === 'ArrowUp' ? 'up' : event.key === 'ArrowDown' ? 'down' : ''); return;
      }
      if (event.key === 'Tab') {
        const fields = [...palette.querySelectorAll('input,button')];
        const at = fields.indexOf(doc.activeElement), next = (at + (event.shiftKey ? -1 : 1) + fields.length) % fields.length;
        if (fields.length) { event.preventDefault(); event.stopImmediatePropagation();fields[next].focus(); }
        return;
      }
    }
    if (event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
      const close = overlayEscape(doc);
      if (close) { event.preventDefault(); event.stopImmediatePropagation(); close(); return; }
    }
    // Esc with no menu or dialog open puts away the floating terminal, else
    // the probe, else the viewer (escNext: workspace.bend's WS.esc.next),
    // and the keyboard goes back to the composer. With nothing to put away
    // it leaves the composer (as the native one does), so a bare key can
    // press its rail button, and from anywhere else it goes back to it.
    // Shift+Esc in the terminal hides it.
    const esc = escapeState(event, doc);
    if (esc) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.repeat) return;
      if (esc.shift) { dispatch('ws-term-hide', ''); return; }
      const what = escNext ? escNext(esc.term, esc.probe, esc.viewer) : '';
      if (what === 'term') dispatch('ws-term-hide', '');
      else if (what === 'probe') { if (clearProbe) clearProbe(); else dispatch('probe', ''); }
      else if (what === 'viewer') dispatch('panel', '');
      else if (esc.composer) { event.target.blur(); return; }
      composer?.claim();
      return;
    }
    if (slashKey(event) && composer?.end()) { event.preventDefault(); event.stopImmediatePropagation(); return; }
    const action = workspaceShortcut(event, platform);
    if (!action) {
      const key = railKey(event), button = key && doc.querySelector(`.workspace-bar [data-keycap="${key}"]`);
      if (button) { event.preventDefault(); event.stopImmediatePropagation(); button.click(); }
      return;
    }
    event.preventDefault(); event.stopImmediatePropagation();
    if (event.repeat && (action[0] === 'ws-palette-open' || action[1] === 'palette' || action[1] === 'term')) return;
    dispatch(...action);
  }, true);
}
