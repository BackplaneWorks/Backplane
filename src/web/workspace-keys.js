// Event normalization only; Bend owns navigation order and action decisions.
import { closeWorkspaceMenu } from './workspace-menus.js';
const EDITOR = 'input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[data-keys],[data-keyto]';
// A PTY (the terminal, an embedded one) or a bot's browser frame: every key
// it gets is its own (Ctrl+K kills a line, Ctrl+D ends the shell).
const PTY = '[data-keys],[data-keyto]';

// A bare letter or digit (no modifier, outside every editor and the
// composer's own buttons, where it is a word meant for the draft) may be
// one of the rail's keycaps (view.bend writes data-keycap on the button
// that shows it): the host presses that button, so the key does what a
// click does. '' for any other key.
export function railKey(event, doc = null) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat || event.getModifierState?.('AltGraph')) return '';
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return '';
  const key = typeof event.key === 'string' ? event.key.toLowerCase() : '';
  if (!/^[a-z0-9]$/.test(key) || event.target?.closest?.(EDITOR + ',.composer,#viewer')) return '';
  // Reserve the viewer's keys while the pointer is over it too, before
  // its canvas has taken focus.
  if (doc?.querySelector?.('#viewer:hover')) return '';
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
  if (menu) return () => closeWorkspaceMenu(menu);
  // The inline editor already supplies its cancel action, like a dialog.
  if ([...(doc.querySelectorAll?.('.draw-back,[role="dialog"],[aria-modal="true"]') || [])].some(shown)) return null;
  const account = doc.querySelector?.('.account-picker');
  if (account) return () => doc.querySelector?.('.account-chip')?.click();
  const cancel = doc.querySelector?.('.title-box [data-on-click="rename-no"]');
  if (cancel) return () => cancel.click();
  const bot = doc.querySelector?.('.bform [data-on-click="bot-new"]');
  if (bot) return () => bot.click();
  return null;
}

// What Tab reaches in a dialog, in order: its controls, and a part that
// scrolls with no control in it (the browser lets the keyboard in to
// scroll it).
const TABBABLE = 'a[href],button:not(:disabled),input:not(:disabled):not([type="hidden"]),select:not(:disabled),textarea:not(:disabled),summary,[tabindex]:not([tabindex="-1"])';
export function tabStops(dialog, win = window) {
  const scrolls = el => {
    const s = win.getComputedStyle(el);
    return (/auto|scroll/.test(s.overflowY) && el.scrollHeight > el.clientHeight || /auto|scroll/.test(s.overflowX) && el.scrollWidth > el.clientWidth) && !el.querySelector(TABBABLE);
  };
  return [...dialog.querySelectorAll('*')].filter(el => shown(el) && (el.matches(TABBABLE) || scrolls(el)));
}

// Tab in a dialog over the page (a project's Intent) stays in it: from
// its last stop to its first, Shift+Tab from its first to its last, and
// from the dialog itself to its first (or last). null where the browser's
// own Tab stays inside.
export function dialogTab(active, back, stops) {
  if (!stops.length) return null;
  const at = stops.indexOf(active);
  if (at < 0) return back ? stops.at(-1) : stops[0];
  if (!back && at === stops.length - 1) return stops[0];
  if (back && at === 0) return stops.at(-1);
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
// reconnect, on Esc, and once a press on the turn's own buttons (Send,
// Stop, Steer, the foot's others, the top bar's Stop, a tool drawer) is let go, so the
// next words go to the draft rather than press that button again. Never
// from a field the person types in, during a press (a selection being
// made) or over a selection outside the composer, nor on a phone-sized or
// touch page (its keyboard would cover the thread). The caret goes back
// where it was in that thread's draft.
const COVERS = '.tour,.lightbox,.workspace-palette,#settings-content,.draw-back,#thin-result,[role="dialog"],[aria-modal="true"],.shade,.modal-back,.local-menu[open],.floating-terminal,.tok-pop,.account-picker,.title-box:has([data-on-click="rename-no"]),.bform:has([data-on-click="bot-new"])';
const TURN_BUTTONS = '.composer-foot button,.composer-foot .attach,.topbar .kv.state button,button.fold';
// What an Esc puts away (the palette, a picker, a menu, a row's shade, the
// token stats, a rename): it goes with the hub's next view, and the draft
// has the keyboard back at once, before the next key.
const LEAVING = '.workspace-palette,.shade,.modal-back,.local-menu,.tok-pop,.account-picker,.title-box,.bform';
// The keyboard is on its way to the composer for this long after a press on
// the turn's buttons or another thread's row, an Esc that put something
// away, or the page's load (until its first view shows the draft); the
// person typing a moment ago (TYPED) and the keyboard then fallen out of
// the field (its view redrawn) is the same.
const COMING = 3000, LOADING = 10000, TYPED = 1000;
export function installComposerFocus({ document: doc = document, window: win = window,
  phone = () => win.matchMedia?.('(max-width: 760px), (hover: none) and (pointer: coarse)').matches } = {}) {
  let want = true, thread = '', covered = false, viewer = false, pressed = false, restore = false;
  // (on its way: until when; the thread whose row was pressed; whether the
  // page's first view is still to come; the last key typed in a field; and
  // the letters typed meanwhile, for that thread's draft)
  let coming = Date.now() + LOADING, picked = '', loading = true, typed = 0, held = '', heldFor = '';
  const carets = new Map();
  const field = () => doc.getElementById?.('composer');
  const here = () => doc.querySelector?.('main.thread-view[data-thread]')?.getAttribute('data-thread') || '';
  const keep = () => {
    const el = field();
    if (el && doc.activeElement === el && here()) carets.set(here(), [el.selectionStart, el.selectionEnd]);
  };
  // the letters typed while the composer was on its way, into its draft at
  // the caret (the input event saves the draft as typing does)
  const flush = el => {
    if (!held || doc.activeElement !== el || here() !== heldFor) return;
    const text = held;held = '';
    try { el.setRangeText(text, el.selectionStart, el.selectionEnd, 'end'); } catch { return; }
    el.dispatchEvent(new win.InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }));
  };
  const put = (el, at) => {
    el.focus({ preventScroll: true });
    const n = el.value.length, [start, end] = at || [n, n];
    try { el.setSelectionRange(Math.min(start, n), Math.min(end, n)); } catch {}
    flush(el);
  };
  // (a file field, the paperclip's, takes no typing: a click on it leaves
  // the keyboard to the draft)
  const busy = el => !!el && el !== doc.body && el.type !== 'file' && !!el.closest?.(EDITOR);
  // (the page's first view keeps its draft hidden until the workspace is whole)
  const visible = el => shown(el) && el.checkVisibility?.({ visibilityProperty: true }) !== false;
  // the row pressed opens its thread: until it shows, the composer on show is another's
  const picking = () => !!picked && Date.now() < coming;
  const check = () => {
    const el = field(), now = here();
    const covers = [...(doc.querySelectorAll?.(COVERS) || [])].filter(shown), over = covers.length > 0;
    const open = !!doc.querySelector?.(VIEWER_CLOSE);
    if (now && now !== thread) {
      want = true;
      // the composer kept the keyboard while its draft changed thread
      if (el && doc.activeElement === el && carets.has(now)) put(el, carets.get(now));
    }
    if (covered && !over || viewer && !open) want = true;
    thread = now || thread;covered = over;viewer = open;
    const active = doc.activeElement, leaving = c => restore && !!c.matches?.(LEAVING);
    if (!want || !covers.every(leaving) || pressed || !el || el.disabled || !shown(el) || picking() && now !== picked) return;
    want = false;
    const done = () => { if (loading) { loading = false;coming = 0; } };
    if (active === el || busy(active) && !covers.some(c => leaving(c) && c.contains?.(active)) || phone() && !restore) { restore = false;done();return; }
    const selection = win.getSelection?.();
    if (selection && !selection.isCollapsed && !el.contains(selection.anchorNode)) { restore = false;done();return; }
    put(el, carets.get(now));
    // (hidden still, the page's first view) again once it shows
    if (doc.activeElement !== el) { want = true;return; }
    restore = false;picked = '';done();
  };
  doc.addEventListener('pointerdown', event => {
    pressed = true;restore = false;loading = false;coming = 0;picked = '';typed = 0;held = '';
    const target = event.target, row = target?.closest?.('[data-on-click="select"]'), id = row?.getAttribute?.('data-value');
    if (target?.closest?.(TURN_BUTTONS)) { want = true;coming = Date.now() + COMING; }
    if (id) { picked = id;coming = Date.now() + COMING; }
  }, true);
  // A key while the keyboard is on its way (above) is the draft's, not the
  // rail's: the composer takes the keyboard before the key's own action, so
  // the letter lands in it, or, its view not on show yet, the letter waits
  // for it. Esc, Tab, a chord or a press elsewhere is the person moving on.
  doc.addEventListener('keydown', event => {
    if (event.isComposing || event.keyCode === 229) return;
    const target = event.target, key = typeof event.key === 'string' ? event.key : '';
    const chord = event.ctrlKey || event.metaKey || event.altKey, moving = key === 'Escape' || key === 'Tab';
    if (busy(target)) {
      if (moving || target.closest?.(PTY)) { typed = 0;coming = 0;loading = false; } else if (!chord) typed = Date.now();
      return;
    }
    const t = Date.now(), on = t < coming, fell = t - typed < TYPED && (!target || target === doc.body);
    if (event.defaultPrevented || !on && !fell) return;
    if (chord || moving) { typed = 0;coming = 0;loading = false;held = '';return; }
    const letter = key.length === 1;
    if (!letter && key !== 'Enter' && key !== 'Backspace') return;
    const el = field(), now = here();
    if (el && !el.disabled && visible(el) && (!picking() || now === picked)) {
      if (doc.activeElement !== el) put(el, carets.get(now));
      want = false;picked = '';
      // (Enter would press the button the keyboard was on, or start a line)
      if (key === 'Enter') event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    const owner = picking() ? picked : now;
    if (!owner) return;
    event.preventDefault();event.stopImmediatePropagation();
    if (heldFor !== owner) { held = '';heldFor = owner; }
    if (key === 'Backspace') held = held.slice(0, -1); else if (letter) held += key;
    want = true;restore = true;
  }, true);
  // the composer taken by a click or the hub's own focus: what waited goes in
  doc.addEventListener('focusin', event => { if (held && event.target === field()) win.setTimeout(() => flush(field()), 0); }, true);
  // a menu (a <details>) opened or shut by Esc, a click or its own item:
  // no render follows, so the cover is looked at once it has toggled
  doc.addEventListener('toggle', () => win.setTimeout(check, 0), true);
  const up = event => {
    if (pressed) {
      pressed = false;
      // Drawer clicks can be followed by typing before a timer gets a turn.
      if (event.target?.closest?.('button.fold')) {
        // A mouse in a narrow desktop window still belongs to the draft.
        // Touch inspection keeps the phone keyboard out of the thread.
        restore = event.pointerType === 'mouse';
        check();
      }
      else win.setTimeout(check, 0);
    }
  };
  doc.addEventListener('pointerup', up, true);doc.addEventListener('pointercancel', up, true);
  for (const name of ['keyup', 'input', 'pointerup', 'focusout', 'select']) doc.addEventListener(name, event => { if (event.target === field()) keep(); }, true);
  // a dialog, the lightbox or the drawing pad come and go on the body
  if (win.MutationObserver && doc.body) new win.MutationObserver(check).observe(doc.body, { childList: true });
  return {
    check,
    // Esc, a reconnect: the keyboard goes there now if it can
    // An explicit keyboard dismissal may restore the caret at narrow widths
    // too, and does not wait for what it put away to leave the page;
    // automatic focus still leaves the phone's keyboard alone.
    claim(returning = false) { want = true;restore = returning; if (returning) coming = Date.now() + COMING;check(); },
    // "/": to the end of the draft
    end() { const el = field();if (el && !phone()) { want = false;put(el); return true; } return false; },
    // Esc from the draft: the keyboard leaves it on purpose (a bare key may
    // press a rail button)
    leave() { typed = 0;coming = 0;loading = false;held = ''; },
  };
}

export function installWorkspaceKeys({ dispatch, document: doc = document, platform = navigator.platform, escNext = null, clearProbe = null, composer = null }) {
  let palettePending = false;
  doc.addEventListener('keydown', event => {
    if (event.isComposing || event.keyCode === 229 || event.defaultPrevented) return;
    if (doc.querySelector('.tour,.lightbox') || doc.querySelector('.draw-back')) return; // The active overlay owns every key.
    const palette = doc.querySelector('.workspace-palette');
    if (palette) palettePending = false;
    // Queue the explicit close even while the hub is still drawing the open.
    if (palettePending && event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
      palettePending = false;
      event.preventDefault(); event.stopImmediatePropagation();
      dispatch('ws-palette-close', ''); composer?.claim(true); return;
    }
    if (palette) {
      const action = { Escape: 'ws-palette-close', Enter: 'ws-palette-enter', ArrowUp: 'ws-palette-move', ArrowDown: 'ws-palette-move' }[event.key];
      if (action && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
        event.preventDefault(); event.stopImmediatePropagation();
        dispatch(action, event.key === 'ArrowUp' ? 'up' : event.key === 'ArrowDown' ? 'down' : '');
        // closed, the draft has the keyboard before the next key
        if (action === 'ws-palette-close') composer?.claim(true);
        return;
      }
      if (event.key === 'Tab') {
        const fields = [...palette.querySelectorAll('input,button')];
        const at = fields.indexOf(doc.activeElement), next = (at + (event.shiftKey ? -1 : 1) + fields.length) % fields.length;
        if (fields.length) { event.preventDefault(); event.stopImmediatePropagation();fields[next].focus(); }
        return;
      }
    }
    const models = doc.querySelector('[data-local-models="1"]');
    if (models && models.style.display !== 'none') {
      const action = modelPickerKey(event);
      if (action) { event.preventDefault(); event.stopImmediatePropagation(); dispatch(...action); return; }
    }
    // The token stats panel (view.bend's View.tok.pop) shuts on Escape like
    // the picker, after any dialog opened over it has had the key.
    if (event.key === 'Escape' && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && doc.querySelector('.tok-pop')) {
      event.preventDefault(); event.stopImmediatePropagation(); dispatch('flag', 'tokstats'); composer?.claim(true); return;
    }
    // Tab stays in a project's Intent until it closes, never reaching the
    // page it covers
    const dialog = event.key === 'Tab' && !event.ctrlKey && !event.metaKey && !event.altKey && doc.querySelector('.intent[aria-modal="true"]');
    if (dialog) {
      const next = dialogTab(doc.activeElement, event.shiftKey, tabStops(dialog, doc.defaultView || window));
      if (next) { event.preventDefault(); event.stopImmediatePropagation(); next.focus(); }
      return;
    }
    if (event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
      const close = overlayEscape(doc);
      if (close) { event.preventDefault(); event.stopImmediatePropagation(); close(); composer?.claim(true); return; }
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
      else if (esc.composer) { composer?.leave?.();event.target.blur(); return; }
      composer?.claim();
      return;
    }
    if (slashKey(event) && composer?.end()) { event.preventDefault(); event.stopImmediatePropagation(); return; }
    const action = workspaceShortcut(event, platform);
    if (!action) {
      const key = railKey(event, doc), button = key && doc.querySelector(`.workspace-bar [data-keycap="${key}"]`);
      if (button) { event.preventDefault(); event.stopImmediatePropagation(); button.click(); }
      return;
    }
    event.preventDefault(); event.stopImmediatePropagation();
    if (event.repeat && (action[0] === 'ws-palette-open' || action[1] === 'palette' || action[1] === 'term')) return;
    if (action[0] === 'ws-palette-open' || action[0] === 'ws-key' && action[1] === 'palette') palettePending = true;
    dispatch(...action);
  }, true);
}
