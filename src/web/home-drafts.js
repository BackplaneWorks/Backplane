// Desktop drafts belong to the home, even when its service uses another port.
// Empty entries remember a clear, so an older port cannot revive sent text.
export function homeDraftStorage(local, home) {
  if (!home) return local;
  const prefix = 'backplane-draft:';
  const drafts = new Map(home.read());
  for (let i = 0; i < local.length; i++) {
    const key = local.key(i);
    if (key?.startsWith(prefix) && !drafts.has(key)) {
      const text = local.getItem(key);
      home.write(key, text);
      drafts.set(key, text);
    }
  }
  const keys = () => [...new Set([
    ...Array.from({length:local.length}, (_, i) => local.key(i)).filter(key => key && !key.startsWith(prefix)),
    ...[...drafts].filter(([, text]) => text !== null).map(([key]) => key),
  ])];
  return {
    get length() { return keys().length; },
    key(i) { return keys()[i] ?? null; },
    getItem(key) { return key.startsWith(prefix) ? drafts.get(key) ?? null : local.getItem(key); },
    setItem(key, text) {
      if (key.startsWith(prefix)) { home.write(key, text);drafts.set(key, text); }
      local.setItem(key, text);
    },
    removeItem(key) {
      if (key.startsWith(prefix)) { home.write(key, null);drafts.set(key, null); }
      local.removeItem(key);
    },
  };
}
