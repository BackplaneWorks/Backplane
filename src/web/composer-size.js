// field-sizing is unavailable on older iOS Safari. Size real drafts by their
// scroll height there too; never replace the value, selection or focus.
export function sizeComposer(field) {
  if (!field || globalThis.CSS?.supports?.('field-sizing', 'content')) return;
  field.style.height = 'auto';
  field.style.height = `${field.value ? field.scrollHeight + 2 : 44}px`;
}

// Native details hides its descendants on WebKit even when display:contents
// flattens the desktop toolbar. Keep it open while CSS presents inline tools;
// close it once on entry into the compact layout, preserving mobile toggles.
let controlsObserver, controlsSeen;
export function sizeComposerControls(doc = document) {
  const composer = doc.querySelector('.composer');
  if (composer !== controlsSeen && typeof ResizeObserver === 'function') {
    controlsObserver?.disconnect();
    controlsObserver = new ResizeObserver(() => sizeComposerControls(doc));
    controlsSeen = composer;
    if (composer) controlsObserver.observe(composer);
  }
  for (const menu of doc.querySelectorAll('.composer-more')) {
    const summary = menu.querySelector(':scope > summary');
    const inline = getComputedStyle(summary).display === 'none';
    // (each written only when it changes: this runs after every render, and a
    // write of the same value is still a mutation and a style invalidation)
    if (inline) { if (!menu.open) menu.open = true; }
    else if (menu.dataset.inline === 'true') menu.open = false;
    if (menu.dataset.inline !== String(inline)) menu.dataset.inline = String(inline);
    const field = menu.closest('.composer')?.querySelector('textarea');
    const hint = inline ? field?.dataset.placeholderFull : field?.dataset.placeholderShort;
    if (hint && field.placeholder !== hint) field.placeholder = hint;
  }
}
