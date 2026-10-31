// Apply an accepted draft before the next key can read the old textarea.
// Conversation rendering may wait for a frame; text input cannot.
export function applyComposerDraft(doc, thread, text) {
  const field = doc.getElementById('composer');
  const owner = doc.querySelector('main.thread-view[data-thread]')?.getAttribute('data-thread');
  if (field && owner === thread && field.value !== text) field.value = text;
}
