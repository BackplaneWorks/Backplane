// Apply an accepted draft before the next key can read the old textarea.
// Conversation rendering may wait for a frame; text input cannot. Only the
// part that differs is replaced (setRangeText), so a caret or a selection
// in the text the person kept stays where they left it; a sent text taken
// off the front moves them back with the text. A caret in or at the end of
// the replaced part goes to the end of what replaced it, as a whole new
// value puts it. During an IME composition nothing is written: the
// composition's own input events carry the field's text, and the host
// settles what is due when it ends.
export function applyComposerDraft(doc, thread, text, composing = false) {
  const field = doc.getElementById('composer');
  const owner = doc.querySelector('main.thread-view[data-thread]')?.getAttribute('data-thread');
  if (!field || owner !== thread || field.value === text || composing) return;
  const old = field.value;
  // the common end first: a text taken off the front is cut there
  let end = 0;
  while (end < old.length && end < text.length && old[old.length - 1 - end] === text[text.length - 1 - end]) end++;
  let start = 0;
  while (start < old.length - end && start < text.length - end && old[start] === text[start]) start++;
  if (typeof field.setRangeText === 'function') {
    const to = old.length - end, outside = field.selectionEnd <= start || field.selectionStart > to;
    try { field.setRangeText(text.slice(start, text.length - end), start, to, outside ? 'preserve' : 'end');return; } catch {}
  }
  field.value = text;
}
