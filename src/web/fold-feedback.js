// A run of tool calls opened or shut at the click, before the hub's view
// comes back. The hub draws a run's rows only while it is open (fold.bend;
// an open run's rows carry data-run, its first entry's id), so: shutting a
// run seen shut before drops its rows and puts its shut head back at once;
// opening one seen open before puts back the head and rows the hub drew
// then, until the click settles and the hub's own rows (the same keys)
// replace them in place. A run never seen in the other state waits for the
// hub as before. Presentation only: the click still goes once through the
// ordered path. (Subagent rows are subs-feedback.js's.)
import { is } from './wire.js';

const items = xs => { const out = [];for (; xs && is(xs, 'Con'); xs = xs.tail) out.push(xs.head);return out; };
const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
const attr = (n, name) => items(n.attrs).find(a => is(a, 'At') && a.name === name)?.value;
const isTimeline = n => is(n, 'El') && attr(n, 'id') === 'timeline';
// a run's head: its id and whether it is open
const runHead = n => {
  if (!is(n, 'El') || n.tag !== 'button' || !items(n.attrs).some(a => !is(a, 'At') && a.event === 'click' && a.action === 'fold')) return null;
  const v = attr(n, 'data-value') || '';
  return /^[01]./.test(v) && !/^[01](sub|subs|todos|todos\.all):/.test(v) ? { id: v.slice(1), open: v[0] === '0' } : null;
};
const runOf = n => is(n, 'El') ? attr(n, 'data-run') : undefined;
function timeline(page, depth = 14) {
  const stack = [[page, 0]];
  while (stack.length) {
    const [n, d] = stack.pop();
    if (!is(n, 'El') || d > depth || n.tag === 'bp-memo') continue;
    if (isTimeline(n)) return n;
    for (const k of items(n.kids)) stack.push([k, d + 1]);
  }
  return null;
}

export class FoldFeedback {
  constructor(changed, limit = 64) { this.changed = changed;this.limit = limit;this.heads = new Map();this.rows = new Map();this.clear(); }
  keep(map, key, value) { map.delete(key);map.set(key, value);while (map.size > this.limit) map.delete(map.keys().next().value); }
  // a page the hub drew: each run's head as drawn, an open run's rows
  seen(page) {
    if (page === this.seenNode) return;
    this.seenNode = page;
    const tl = timeline(page);
    if (!tl) return;
    const ks = items(tl.kids);
    for (let i = 0; i < ks.length; i++) {
      const h = runHead(ks[i]);
      if (!h) continue;
      this.keep(this.heads, h.id + (h.open ? '\u00001' : '\u00000'), ks[i]);
      if (!h.open) continue;
      const rows = [];
      for (let j = i + 1; j < ks.length && runOf(ks[j]) === h.id; j++) rows.push(ks[j]);
      if (rows.length) this.keep(this.rows, h.id, rows);
    }
  }
  // the shut runs of a page whose rows were never seen (their first ids,
  // the newest last; at most `n`)
  unseen(page, n = 32) {
    const tl = timeline(page), out = [];
    for (const k of tl ? items(tl.kids) : []) { const h = runHead(k);if (h && !h.open && !this.rows.has(h.id)) out.push(h.id); }
    return out.slice(-n);
  }
  // a click: a function that settles it (the hub's answer), or undefined
  choice(action, value) {
    if (action !== 'fold') return undefined;
    const v = String(value);
    if (!/^[01]./.test(v) || /^[01](sub|subs|todos|todos\.all):/.test(v)) return undefined;
    const id = v.slice(1), open = v[0] === '1', drawn = this.heads.get(id + (open ? '\u00001' : '\u00000'));
    if (!drawn || open && !this.rows.has(id)) return undefined;
    const revision = ++this.revision;
    this.active = { id, open, head: drawn, rows: open ? this.rows.get(id) : [], revision };
    this.changed();
    return () => { if (this.active?.revision === revision) { this.active = null;this.changed(); } };
  }
  get showing() { return !!this.active; }
  // the rows a run opened at the click puts back (their ids)
  restored() { return this.active?.open ? new Set(this.active.rows.map(r => attr(r, 'data-id') || r.key).filter(Boolean)) : null; }
  project(page) {
    const a = this.active;
    if (!a) return page;
    let done = false;
    const walk = n => {
      if (done || !is(n, 'El') || n.tag === 'bp-memo') return n;
      if (isTimeline(n)) {
        done = true;
        const ks = items(n.kids), out = [];
        for (let i = 0; i < ks.length; i++) {
          const h = runHead(ks[i]);
          if (h?.id !== a.id) { if (runOf(ks[i]) !== a.id) out.push(ks[i]);continue; }
          out.push(a.head, ...a.rows);
        }
        return { ...n, kids: list(out) };
      }
      const ks = items(n.kids), mapped = ks.map(walk);
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    return walk(page);
  }
  drop() { this.revision++;this.active = null; }
  clear() { this.revision = (this.revision || 0) + 1;this.active = null;this.seenNode = null;this.heads?.clear();this.rows?.clear(); }
}
