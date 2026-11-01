import { captureApp } from './improvements-capture.js';
import { ImprovementsStore } from './improvements-store.js';
import { ImprovementsFlow, improvementThread } from './improvements-flow.js';

export function improvementsShortcut(event, platform = '') {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat || event.getModifierState?.('AltGraph') || event.shiftKey) return false;
  const mac = /mac/i.test(platform);
  return (event.code === 'KeyI' || event.key?.toLowerCase() === 'i') && event.ctrlKey && (mac ? event.metaKey && !event.altKey : event.altKey && !event.metaKey);
}
export const improvementsChord = platform => /mac/i.test(platform) ? 'Ctrl+Cmd+I' : 'Ctrl+Alt+I';
export function clampPopup(x, y, width, height, viewport) {
  return { x: Math.max(viewport.x + 8, Math.min(x, viewport.x + viewport.width - width - 8)),
    y: Math.max(viewport.y + 8, Math.min(y, viewport.y + viewport.height - height - 8)) };
}
const timed = (promise, ms, message) => new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(Error(message)), ms);
  Promise.resolve(promise).then(value => { clearTimeout(timer);resolve(value); }, error => { clearTimeout(timer);reject(error); });
});
export function installImprovements({ rpc, context, identity, ready, select, beforeCapture = async () => {}, overlayChanged = () => {},
  capture = captureApp, store = new ImprovementsStore(), doc = document, win = window } = {}) {
  let draft, scope, opening = false, busy = false, capturing = false, previousFocus, previousRange, url, operation = 0, pos = null;
  let saving = Promise.resolve();
  let captureController;
  const chord = improvementsChord(win.navigator.platform);
  const el = (tag, text, attrs = {}) => { const n = doc.createElement(tag);if (text) n.textContent = text;for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);return n; };
  const popup = el('section', '', { class: 'improvements', role: 'dialog', 'aria-labelledby': 'improvements-title', hidden: '' });
  const header = el('header', '', { tabindex: '0', 'aria-label': 'Move Improvements with arrow keys' });
  header.append(el('strong', 'Improvements', { id: 'improvements-title' }), el('kbd', chord));
  const close = el('button', '×', { type: 'button', 'aria-label': 'Close Improvements' });header.append(close);
  const label = el('label', 'What would you like to improve?', { for: 'improvements-text' });
  const input = el('textarea', '', { id: 'improvements-text', rows: '5', placeholder: 'Describe the change…' });
  const projectLabel = el('label', 'Project', { for: 'improvements-project' });
  const project = el('select', '', { id: 'improvements-project' });
  const preview = el('img', '', { alt: 'App screenshot to send with this request', hidden: '' });
  const note = el('p', '', { role: 'status', 'aria-live': 'polite' });
  const buttons = el('footer');
  const again = el('button', 'Capture again', { type: 'button' }), choose = el('button', 'Choose image', { type: 'button' });
  const file = el('input', '', { type: 'file', accept: 'image/png,image/jpeg,image/webp', hidden: '' });
  const send = el('button', 'Send', { type: 'button', class: 'primary' });buttons.append(again, choose, send);
  popup.append(header, label, input, projectLabel, project, preview, note, buttons, file);doc.body.append(popup);
  function announce(text) { note.textContent = text; }
  function viewport() { const v = win.visualViewport;return { x: v?.offsetLeft || 0, y: v?.offsetTop || 0, width: v?.width || win.innerWidth, height: v?.height || win.innerHeight }; }
  function place() {
    if (popup.hidden) return;
    const v = viewport();
    popup.style.maxHeight = Math.max(100, v.height - 16) + 'px';
    if (v.width <= 600) { popup.style.left = v.x + 8 + 'px';popup.style.top = v.y + 8 + 'px';popup.style.width = v.width - 16 + 'px';return; }
    popup.style.width = '';
    const r = popup.getBoundingClientRect();pos = clampPopup(pos?.x ?? v.x + (v.width - r.width) / 2, pos?.y ?? v.y + 48, r.width, r.height, v);
    popup.style.left = pos.x + 'px';popup.style.top = pos.y + 'px';
  }
  const save = value => {
    const record = { ...value, updated: Date.now() }, savedScope = scope;
    saving = saving.catch(() => {}).then(() => timed(store.write(savedScope, record), 5000, 'Could not keep this draft on this device. Retry Send.'));
    return saving;
  };
  function controls() {
    const locked = busy || !!draft?.message;
    input.disabled = !draft;input.readOnly = locked;project.disabled = !draft || locked || !!draft?.key;
    again.disabled = choose.disabled = locked || capturing;
    send.disabled = busy || capturing || !draft?.image || !draft?.project || !draft?.text?.trim();
    send.textContent = busy ? 'Sending…' : draft?.accepted ? 'Open thread' : 'Send';
  }
  function showPreview() {
    if (url) URL.revokeObjectURL(url);url = draft?.image ? URL.createObjectURL(draft.image) : null;
    preview.hidden = !url;if (url) preview.src = url;else preview.removeAttribute('src');controls();place();
  }
  function show() { popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();showPreview();input.focus({ preventScroll: true }); }
  function hide() {
    operation++;popup.hidden = true;popup.removeAttribute('aria-modal');overlayChanged();
    if (previousFocus?.isConnected) { previousFocus.focus({ preventScroll: true });if (previousRange && previousFocus.setSelectionRange) previousFocus.setSelectionRange(...previousRange); }
  }
  async function recapture(initial = false) {
    if (capturing || busy || draft?.message) return;
    const stamp = ++operation, focus = doc.activeElement, range = focus === input ? [input.selectionStart, input.selectionEnd, input.selectionDirection] : null;
    capturing = true;controls();
    const captureAbort = new AbortController();captureController = captureAbort;
    popup.hidden = true;popup.removeAttribute('aria-modal');
    try {
      await beforeCapture();
      const image = await timed(capture({ signal: captureAbort.signal }), 20000, 'Capture timed out. Capture again or choose an image.');
      if (stamp !== operation) return;
      if (!image?.size || !/^image\/(png|jpeg|webp)$/.test(image.type)) throw Error('Choose a PNG, JPEG or WebP image.');
      await save({ ...draft, image, path: '' });
      if (stamp !== operation) { await save(draft);return; }
      draft.image = image;draft.path = '';announce('Screenshot refreshed.');
    } catch { captureAbort.abort();if (stamp === operation) announce('Capture was not completed. Your draft and previous image are kept. Capture again or choose an image.'); }
    finally {
      if (stamp === operation) {
        capturing = false;
        popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();showPreview();
        (initial ? input : focus)?.focus?.({ preventScroll: true });if (range) input.setSelectionRange(...range);
      }
    }
  }
  async function open() {
    if (opening || capturing || !popup.hidden) return;
    opening = true;previousFocus = doc.activeElement;
    previousRange = typeof previousFocus?.selectionStart === 'number' ? [previousFocus.selectionStart, previousFocus.selectionEnd, previousFocus.selectionDirection] : null;
    try {
      if (!ready()) throw Error('Connect to the hub before opening Improvements.');
      const auth = identity();if (!auth) throw Error('Wait for the hub connection, then open Improvements again.');
      const current = await context();scope = win.location.origin + ':' + auth;
      draft = await timed(store.read(scope), 5000, 'Could not restore the draft. Try Improvements again.');
      if (draft?.accepted) { await select(draft.thread);await store.write(scope, null);draft = null; }
      const { projects: choices, project: sourceProject, accountBinding: sourceAccountBinding, ...viewContext } = current;
      viewContext.sourceProject = sourceProject;
      draft ||= { text: '', image: null, project: current.project || '', context: viewContext, key: '' };
      input.value = draft.text;project.replaceChildren(el('option', 'Choose a project', { value: '' }));
      for (const p of current.projects || []) project.append(el('option', p.title, { value: p.id }));
      if (draft.project && ![...project.options].some(o => o.value === draft.project)) project.append(el('option', 'Saved project', { value: draft.project }));
      project.value = draft.project;
      announce(draft.message ? 'This request is ready to retry. Send uses the same conversation.' : 'Send starts a new thread in this project.');
      if (draft.image) show();else await recapture(true);
    } catch (error) { announce(error.message);popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();place();close.focus(); }
    finally { opening = false;controls(); }
  }
  input.addEventListener('input', () => { if (!draft || draft.message) return;draft.text = input.value;controls();void save(draft).catch(e => announce(e.message)); });
  project.addEventListener('change', () => { if (!draft || draft.thread) return;draft.project = project.value;void save(draft).catch(e => announce(e.message));controls(); });
  close.addEventListener('click', hide);again.addEventListener('click', () => { void recapture(); });choose.addEventListener('click', () => file.click());
  file.addEventListener('change', async () => {
    const image = file.files?.[0];file.value = '';if (!image || busy || draft?.message) return;
    if (!image.size || !/^image\/(png|jpeg|webp)$/.test(image.type)) { announce('Choose a PNG, JPEG or WebP image.');return; }
    draft.image = image;draft.path = '';showPreview();
    try { await save(draft);announce('Image refreshed.'); } catch (e) { announce(e.message); }
  });
  const flow = new ImprovementsFlow({ rpc, save, select, upload: async (thread, key, image) => {
    if (!ready() || identity() !== scope.slice(win.location.origin.length + 1)) throw Error('Reconnect to the same hub before sending.');
    const query = new URLSearchParams({ thread, key, name: 'backplane.' + ({ 'image/jpeg': 'jpg', 'image/webp': 'webp' }[image.type] || 'png') });
    const response = await fetch('/attach?' + query, { method: 'POST', body: image, credentials: 'same-origin', signal: AbortSignal.timeout(60000) });
    const result = await response.json();
    if (!response.ok || result.thread !== thread || typeof result.attached !== 'string') throw Error(result.text || 'The image did not arrive. Retry Send.');
    return result.attached;
  } });
  send.addEventListener('click', async () => {
    if (busy || capturing || !draft) return;
    busy = true;controls();announce('Sending…');
    try {
      if (!ready() || win.location.origin + ':' + identity() !== scope) throw Error('Reconnect to the same hub, then retry Send.');
      if (draft.accepted) await select(draft.thread);
      else if (!await flow.send(draft)) return;
      await store.write(scope, null);hide();draft = null;
    } catch (error) { announce(error.message || 'Could not send. Your draft and image are kept. Retry Send.'); }
    finally { busy = false;controls(); }
  });
  let drag;
  header.addEventListener('pointerdown', e => {
    if (e.target.closest('button') || viewport().width <= 600 || e.button !== 0) return;
    const r = popup.getBoundingClientRect();drag = { x: e.clientX - r.left, y: e.clientY - r.top };header.setPointerCapture(e.pointerId);e.preventDefault();header.focus();
  });
  header.addEventListener('pointermove', e => { if (drag) { pos = { x: e.clientX - drag.x, y: e.clientY - drag.y };place(); } });
  for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) header.addEventListener(name, () => { drag = null; });
  function moveKey(e) {
    if (e.target !== header || e.isComposing || e.ctrlKey || e.altKey || e.metaKey || !['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(e.key) || viewport().width <= 600) return;
    e.preventDefault();e.stopImmediatePropagation();const r = popup.getBoundingClientRect(), step = e.shiftKey ? 40 : 10;
    pos = { x: r.left + (e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0), y: r.top + (e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0) };place();
  }
  win.addEventListener('keydown', e => {
    if (e.isComposing || e.keyCode === 229 || e.defaultPrevented) return;
    if (capturing && e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();e.stopImmediatePropagation();operation++;captureController?.abort();capturing=false;
      popup.hidden=false;popup.setAttribute('aria-modal','true');overlayChanged();showPreview();input.focus();
      announce('Capture canceled. Your draft and previous image are kept.');return;
    }
    if (!popup.hidden) {
      moveKey(e);
      if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey) { e.preventDefault();e.stopImmediatePropagation();hide();return; }
      if (e.key === 'Tab') {
        const stops = [...popup.querySelectorAll('textarea,select,button,[tabindex="0"]')].filter(n => !n.disabled), at = stops.indexOf(doc.activeElement);
        if (at < 0 || !e.shiftKey && at === stops.length - 1 || e.shiftKey && at === 0) { e.preventDefault();(e.shiftKey ? stops.at(-1) : stops[0]).focus(); }
      }
      // All other keys belong to this popup's native controls, including IME.
      e.stopImmediatePropagation();return;
    }
    if (improvementsShortcut(e, win.navigator.platform) && !doc.querySelector('[aria-modal="true"],.lightbox,.draw-back,.tour')) {
      e.preventDefault();e.stopImmediatePropagation();void open();
    }
  }, true);
  doc.addEventListener('click', e => { if (e.target.closest?.('[data-improvements]')) { e.preventDefault();void open(); } }, true);
  const decorate = () => { for (const button of doc.querySelectorAll('[data-improvements]')) { button.title = 'Improvements · ' + chord;button.setAttribute('aria-keyshortcuts', /mac/i.test(win.navigator.platform) ? 'Control+Meta+I' : 'Control+Alt+I'); } };
  new win.MutationObserver(decorate).observe(doc.getElementById('root') || doc.body, { childList: true, subtree: true });decorate();
  for (const target of [win, win.visualViewport].filter(Boolean)) target.addEventListener('resize', place);
  win.visualViewport?.addEventListener('scroll', place);
  win.backplaneDesktop?.onImprovements?.(() => { if (!doc.querySelector('[aria-modal="true"],.lightbox,.draw-back,.tour')) void open(); });
  return { open, close: hide };
}
