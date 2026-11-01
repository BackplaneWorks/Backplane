// A project note is read-only text. It never becomes HTML or executes code.
const pending = new WeakSet();
const loaded = new WeakMap();
export function previewTextFiles(root = document) {
  for (const node of root.querySelectorAll('[data-text-src]')) {
    const source = node.getAttribute('data-text-src');
    if (loaded.has(node)) {
      if (node.textContent !== loaded.get(node)) node.textContent = loaded.get(node);
      continue;
    }
    if (pending.has(node)) continue;
    pending.add(node);
    node.textContent = node.dataset.loading;
    fetch(source, { credentials: 'same-origin' }).then(async response => {
      if (!response.ok) throw Error('read');
      const text = await response.text();
      loaded.set(node, text);
      if (node.isConnected && node.getAttribute('data-text-src') === source) node.textContent = text;
    }).catch(() => {
      loaded.set(node, node.dataset.error);
      if (node.isConnected) node.textContent = node.dataset.error;
    });
  }
}

// A Markdown note is read the same way; draw(text, node) makes its nodes
// with app.bend's md (the chat's Markdown, never HTML). What was drawn is kept
// with its element, so a frame only puts it back if a patch took it out.
const drawn = new WeakMap();
export function previewMarkdownFiles(draw, root = document) {
  for (const node of root.querySelectorAll('[data-md-src]')) {
    const source = node.getAttribute('data-md-src');
    const kept = drawn.get(node);
    if (kept) {
      if (kept.source === source) {
        if (node.firstChild !== kept.doc || node.childNodes.length !== 1) node.replaceChildren(kept.doc);
        continue;
      }
      drawn.delete(node);
      pending.delete(node);
    }
    if (pending.has(node)) continue;
    pending.add(node);
    node.textContent = node.dataset.loading;
    const show = doc => {
      drawn.set(node, { source, doc });
      if (node.isConnected && node.getAttribute('data-md-src') === source) node.replaceChildren(doc);
    };
    fetch(source, { credentials: 'same-origin' }).then(async response => {
      if (!response.ok) throw Error('read');
      show(draw(await response.text(), node));
    }).catch(() => {
      const note = document.createElement('span');
      note.className = 'fm-note';
      note.textContent = node.dataset.error;
      show(note);
    });
  }
}
