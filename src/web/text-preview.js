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
