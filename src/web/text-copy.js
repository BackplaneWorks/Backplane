// Invoke clipboard writes in the click's task, before a hub round trip can
// spend the browser's user activation. This also works on retained/paged DOM.
function legacyCopy(text, doc) {
  const active = doc.activeElement, selection = doc.getSelection();
  const ranges = selection ? Array.from({length: selection.rangeCount}, (_, i) => selection.getRangeAt(i).cloneRange()) : [];
  const start = active?.selectionStart, end = active?.selectionEnd, direction = active?.selectionDirection;
  const field = doc.createElement('textarea');
  field.value = text;
  field.setAttribute('readonly', '');
  field.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;font-size:16px';
  doc.body.append(field);
  try {
    field.focus({preventScroll:true});field.select();field.setSelectionRange(0, text.length);
    if (!doc.execCommand('copy')) throw Error('Clipboard unavailable. Select the text and copy it.');
  } finally {
    field.remove();
    active?.focus?.({preventScroll:true});
    if (typeof start === 'number') active.setSelectionRange(start, end, direction);
    if (selection && ranges.length) { selection.removeAllRanges();for (const range of ranges) selection.addRange(range); }
  }
}

export async function copyText(text, win = window) {
  if (typeof text !== 'string') throw TypeError('Expected clipboard text');
  if (win.backplaneDesktop?.clipboard?.writeText) {
    const result = await win.backplaneDesktop.clipboard.writeText(text);
    if (!result?.copied) throw Error('Clipboard did not accept the text.');
    return;
  }
  if (win.isSecureContext && win.navigator.clipboard?.writeText) {
    try { await win.navigator.clipboard.writeText(text);return; }
    catch { /* Permission can be denied even when the API exists. */ }
  }
  legacyCopy(text, win.document);
}

const feedback = new WeakMap();
export async function copyButton(button, text) {
  const old = feedback.get(button);
  if (old) clearTimeout(old.timer);
  const state = {label:old?.label ?? button.textContent, title:old?.title ?? button.getAttribute('title')};
  feedback.set(button, state);
  const current = () => feedback.get(button) === state && button.isConnected && button.getAttribute('data-value') === text;
  try {
    await copyText(text, button.ownerDocument.defaultView);
    if (!current()) return;
    button.textContent = 'Copied';button.setAttribute('title', 'Copied');
  } catch (error) {
    if (!current()) return;
    button.textContent = 'Copy failed';button.setAttribute('title', error.message);
  }
  state.timer = setTimeout(() => {
    if (!current()) return;
    feedback.delete(button);
    button.textContent = state.label;
    if (state.title === null) button.removeAttribute('title');else button.setAttribute('title', state.title);
  }, 2000);
}
