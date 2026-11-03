// A bounded in-memory picture of validated server views for smooth switching.
// It never restores sessions, cursors or authority. The host overlays the
// current draft and still refuses actions until the new view is ready.
export class ViewTransition {
  constructor({limit = 8, bytes = 512 * 1024, store = null} = {}) { this.limit = limit;this.bytes = bytes;this.store = store;this.views = new Map();this.positions = new Map();this.scopes = new Map();this.loads = new Map();this.generation = 0; }
  key(scope) {
    if (!scope?.identity || !scope.epoch || !scope.target?.kind) return '';
    return JSON.stringify([scope.origin, scope.identity, scope.language, scope.epoch, scope.target, scope.selected, scope.settings === true, scope.route || null]);
  }
  put(scope, shown, persist = true) {
    const key = this.key(scope);
    const has = (n, test) => n?.type === 'element' && (test(n) || n.kids.some(c => has(c, test)));
    const composer = n => n.attrs.some(a => a.name === 'class' && a.value.split(/\s+/).includes('composer'));
    // the selected thread's own view, its composer anywhere in it (the rail
    // layout keeps it in .thread-content)
    const conversation = n => has(n, m => m.tag === 'main' && m.attrs.some(a => a.name === 'data-thread' && a.value === scope.selected) && has(m, composer));
    // Settings, Mailroom and other forms retain their existing field ownership;
    // never restore their mutable fields as a conversation picture.
    if (!key || !scope.selected || !conversation(shown) || new TextEncoder().encode(JSON.stringify(shown)).length > this.bytes) return false;
    this.views.delete(key);this.views.set(key, shown);this.scopes.set(key, { ...scope });
    while (this.views.size > this.limit) { const old=this.views.keys().next().value;this.views.delete(old);this.positions.delete(old);this.scopes.delete(old); }
    if (persist && this.store) {
      if (this.store.schedule) this.store.schedule(scope, key, shown);
      else void this.store.save(scope, key, shown).catch(() => {});
    }
    return true;
  }
  get(scope) { return this.views.get(this.key(scope)) || null; }
  savePosition(scope, position) {
    const key=this.key(scope);
    if (this.views.has(key)) this.positions.set(key, position);
  }
  position(scope) { return this.positions.get(this.key(scope)) || null; }
  // Explicit bot navigation may precede knowing its backing conversation.
  // This lookup returns only a visual destination; it never restores a session.
  destination(scope, target) {
    for (const [key, candidate] of [...this.scopes].reverse()) {
      if (candidate.origin === scope.origin && candidate.identity === scope.identity && candidate.epoch === scope.epoch
        && candidate.language === scope.language && candidate.bundle === scope.bundle && candidate.settings !== true
        && candidate.target.kind === target.kind && candidate.target.id === target.id) return {scope:candidate, shown:this.views.get(key)};
    }
    return null;
  }
  warm(scope) {
    if (!this.store) return Promise.resolve(false);
    const partition = JSON.stringify([scope.origin, scope.identity, scope.language, scope.epoch, scope.bundle]);
    if (this.loads.has(partition)) return this.loads.get(partition);
    const generation = this.generation;
    const loading = this.store.load(scope).then(rows => {
      if (generation !== this.generation) return false;
      // Disk cannot overtake a picture already validated on this connection.
      for (const row of rows.slice().reverse()) if (!this.views.has(this.key(row.scope))) this.put(row.scope, row.shown, false);
      return true;
    }).catch(() => false);
    this.loads.set(partition, loading);return loading;
  }
  clear() { this.generation++;this.views.clear();this.positions.clear();this.scopes.clear();this.loads.clear(); }
}

// Light or dark is one fact for the whole page (the hub's ui.theme), never a
// thread's. A retained picture keeps the app root's class it was drawn with,
// so before one is shown again its root takes the theme of the newest
// authoritative frame; everything else in it is left exactly as it was.
const appRoot = (n, depth = 0) => n?.type !== 'element' || depth > 4 ? null
  : n.key === 'app' ? n : n.kids.map(c => appRoot(c, depth + 1)).find(Boolean) || null;
export function themeOf(shown) {
  const app = appRoot(shown);
  if (!app) return '';
  return (app.attrs.find(a => a.name === 'class')?.value || '').split(/\s+/).includes('light') ? 'light' : 'dark';
}
export function withTheme(shown, theme) {
  if (theme !== 'light' && theme !== 'dark') return shown;
  const app = appRoot(shown);
  if (!app || themeOf(shown) === theme) return shown;
  const recolor = n => {
    if (n?.type !== 'element') return n;
    if (n !== app) return {...n, kids: n.kids.map(recolor)};
    const words = (n.attrs.find(a => a.name === 'class')?.value || '').split(/\s+/).filter(w => w && w !== 'light');
    // the hub writes "light" before the tour's words (view.bend's View.page.b.sidebar)
    if (theme === 'light') { const at = words.findIndex(w => w.startsWith('tour')); words.splice(at < 0 ? words.length : at, 0, 'light'); }
    const attrs = n.attrs.some(a => a.name === 'class') ? n.attrs.map(a => a.name === 'class' ? {...a, value: words.join(' ')} : a)
      : [...n.attrs, {name: 'class', value: words.join(' ')}];
    return {...n, attrs};
  };
  return recolor(shown);
}
