import { T, is } from './wire.js';

const list = items => items.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
const items = xs => { const out = [];for (; xs && is(xs, 'Con'); xs = xs.tail) out.push(xs.head);return out; };
const attr = (n, name) => items(n.attrs).find(a => is(a, 'At') && a.name === name)?.value;
const action = n => items(n.attrs).find(a => !is(a, 'At') && a.event === 'click')?.action || attr(n, 'data-on-click');
const continuation = n => n.tag === 'button' && action(n) === 'thin-message-more';
const classes = n => (attr(n, 'class') || '').split(/\s+/);
const text = value => ({ $: T.Txt, text: value });
const element = (tag, key, attrs, kids) => ({ $: T.El, tag, key,
  attrs: list(Object.entries(attrs).map(([name, value]) => ({ $: T.At, name, value }))), kids: list(kids) });
const label = (button, value, key) => ({ ...button, key,
  attrs: list(items(button.attrs).map(a => is(a, 'At') && a.name === 'title' ? { ...a, value } : a)), kids: list([text(value)]) });

// Presentation only: issued partial IDs/cursors remain the request authority.
// Memo values are expanded before traversal and are never modified in place.
export class ThinContinuations {
  constructor() { this.clear(); }
  clear() { this.scope = '';this.partial = new Map();this.readers = new Map(); }
  bind(hub, detail) {
    const scope = JSON.stringify([hub, detail.epoch, detail.target.kind, detail.target.id]);
    if (scope !== this.scope) this.clear();
    this.scope = scope;
    const partial = new Map(detail.window.partial.map(p => [p.id, p]));
    for (const [id, reader] of this.readers) if (!partial.has(id) || partial.get(id).version !== reader.version) this.readers.delete(id);
    this.partial = partial;
  }
  accept(message) {
    if (!this.scope || this.partial.get(message.id)?.version !== message.version) return false;
    if (!this.readers.has(message.id) && this.readers.size >= 4) this.readers.delete(this.readers.keys().next().value);
    this.readers.set(message.id, message);return true;
  }
  project(root, expand, interactive = true) {
    const walk = (node, visit) => {
      if (!is(node, 'El')) return node;
      const expanded = expand(node);
      if (expanded !== node) return walk(expanded, visit);
      return visit({ ...node, kids: list(items(node.kids).map(k => walk(k, visit))) });
    };
    const timeline = node => {
      const rows = new Map(), controls = new Map();
      const control = n => {
        if (!continuation(n)) return null;
        try {
          const p = JSON.parse(attr(n, 'data-value'));
          return Object.keys(p).sort().join(',') === 'cursor,entryId' && p.cursor === '' && this.partial.has(p.entryId) ? p.entryId : null;
        } catch { return null; }
      };
      const scan = n => {
        if (!is(n, 'El')) return;
        const id = attr(n, 'data-id');
        if (id && (classes(n).includes('msg') || classes(n).includes('act'))) rows.set(id, [...rows.get(id) || [], n]);
        const c = control(n);if (c) controls.set(c, [...controls.get(c) || [], n]);
        for (const k of items(n.kids)) scan(k);
      };scan(node);
      // Only activity rows fold: message text always arrives whole, so a
      // message row never gets a reader or a "read more" control.
      const readerNodes = (id, button) => {
        const reader = this.readers.get(id), out = [];
        if (reader) out.push(element('pre', 'thin-text:' + id, { class: 'thin-message-text',
          style: 'white-space:pre-wrap;overflow-wrap:anywhere;max-width:100%',
          'aria-label': 'Full activity text' }, [text(reader.text)]));
        if (!reader || reader.hasMore) {
          const wording = reader ? 'Read next part' : 'Read full activity text';
          // The existing ThinHost resolves its assembled next cursor. Keep the
          // original empty cursor for the first explicit request only.
          const attrs = reader ? items(button.attrs).map(a => is(a, 'At') && a.name === 'data-value'
            ? { ...a, value: JSON.stringify({ entryId: id, cursor: reader.cursor }) } : a) : items(button.attrs);
          out.push(label({ ...button, attrs: list(attrs) }, wording, 'thin-more:' + id));
        }
        return out;
      };
      const rewrite = n => {
        if (!is(n, 'El')) return n;
        if (continuation(n)) {
          // Folded or otherwise absent entries expose their reader only when
          // their own unique row is shown. Never add an orphan footer panel
          // or guess another row; issued partial metadata remains untouched.
          // Saved pictures can belong to another scope and have no current
          // partial metadata. Remove their raw footer controls as well.
          return null;
        }
        let kids = items(n.kids).map(rewrite).filter(Boolean);
        const row = attr(n, 'data-id'), button = controls.get(row);
        if (interactive && rows.get(row)?.length === 1 && rows.get(row)[0] === n && button?.length === 1 && classes(n).includes('act'))
          kids.push(...readerNodes(row, button[0]));
        if (n.tag === 'button' && action(n) === 'view-page' && classes(n).includes('earlier')) {
          return { ...label(n, 'Load earlier messages', 'thin-earlier'), attrs: list(items(n.attrs).filter(a => !is(a, 'At') || !['class', 'style', 'title'].includes(a.name)).concat([
            { $: T.At, name: 'class', value: 'thin-earlier' }, { $: T.At, name: 'title', value: 'Load earlier messages' }
          ])) };
        }
        return { ...n, kids: list(kids) };
      };
      return rewrite(node);
    };
    return walk(root, n => attr(n, 'id') === 'timeline' ? timeline(n) : n);
  }
}
