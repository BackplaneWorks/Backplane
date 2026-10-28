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

export function installWorkspaceKeys({ dispatch, document: doc = document, platform = navigator.platform }) {
  doc.addEventListener('keydown', event => {
    if (event.isComposing || event.keyCode === 229 || event.defaultPrevented) return;
    if (doc.querySelector('.tour,.lightbox')) return; // The active overlay owns every key.
    const models = doc.querySelector('[data-local-models="1"]');
    if (models && models.style.display !== 'none') {
      const action = modelPickerKey(event);
      if (action) { event.preventDefault(); event.stopImmediatePropagation(); dispatch(...action); return; }
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
    // Match the native composer: Escape leaves it once completions are closed.
    // Keep editor navigation guarded until focus has actually left the field.
    if (event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey &&
        event.target?.tagName === 'TEXTAREA' && event.target.id === 'composer' &&
        !event.target.getAttribute?.('data-esc')) {
      event.preventDefault(); event.stopImmediatePropagation(); event.target.blur(); return;
    }
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
