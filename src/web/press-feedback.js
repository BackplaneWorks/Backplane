// What a press opens, shown at the press: the hub's views are drawn by the
// hub, so a press that opens a menu, a popover or a viewer pane waited a
// whole round trip for anything to change. A press the hub has answered
// once by opening, closing or changing one region of the page (the command
// palette, the viewer pane, the account and token pickers, a row's menu) is
// remembered with the region it gave: the same press on the same
// destination, with the same regions open, puts that region into the page
// at once (or takes it away), until the hub's own answer settles the press.
// Presentation only: the press still goes once through the ordered path;
// the hub's view then draws the same region in place (the same keys), so
// nothing moves, and what it holds that is newer replaces the remembered.
import { is } from './wire.js';

const items = xs => { const out = [];for (; xs && is(xs, 'Con'); xs = xs.tail) out.push(xs.head);return out; };
const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
const attr = (n, name) => items(n.attrs).find(a => is(a, 'At') && a.name === name)?.value;
const classes = n => (attr(n, 'class') || '').split(/\s+/);
const withAttr = (n, name, value) => {
  const as = items(n.attrs), at = as.findIndex(a => is(a, 'At') && a.name === name);
  if (at < 0 || as[at].value === value) return n;
  return { ...n, attrs: list(as.map((a, i) => i === at ? { ...a, value } : a)) };
};

// the regions a press may open (a kind each, the first of it in the page)
export const REGIONS = [
  ['palette', n => n.tag === 'div' && n.key === 'workspace-palette'],
  ['viewer', n => n.tag === 'aside' && classes(n).includes('viewer')],
  ['account', n => classes(n).includes('account-picker')],
  ['tokens', n => classes(n).includes('tok-pop')],
  ['rowmenu', n => classes(n).includes('rowmenu')],
  // Settings takes the place of the thread's main (its pages too)
  ['settings', n => n.tag === 'main' && classes(n).includes('settings-view')],
];
// what a region puts away where it is drawn (its sibling of this kind);
// such a region is never taken away at a press (what comes back is the hub's)
const REPLACES = { settings: n => is(n, 'El') && n.tag === 'main' };
// what a region brings along, as the hub drew it with it (Settings' top bar
// names Settings, not the thread): found, learned and drawn with it, never
// on its own (the top bar changes with every turn's state)
// (not the viewer's rail: drawn while loading, it is another height than the
// hub's next one, and its buttons moved when that came)
const ALONG = { settings: [['topbar', n => n.tag === 'header' && classes(n).includes('topbar')]] };
// presses that move to another destination or change content: never
// remembered (their answer is a view of something else, or the hub's alone)
const NEVER = new Set(['select', 'bot', 'bot-tab', 'proj-home', 'send', 'send-alt', 'draft', 'bfield', 'detach',
  'uber', 'uber-bot', 'remote', 'ref-go', 'ref-back', 'ref-fwd', 'earlier', 'view-page', 'fork', 'new-thread',
  'thread-child', 'delete', 'row-archive', 'row-settle', 'row-pin', 'row-rename', 'rename-save', 'secret-save', 'sealed']);
// presses that open the viewer pane (the rail's tabs, the header's Viewer)
const VIEWER = new Set(['view-kind', 'fm', 'diff', 'mech', 'rv-page', 'panel']);
const DEPTH = 7;
const sig = n => n.tag + '\u0000' + n.key + '\u0000' + (n.key ? '' : classes(n)[0] || '');

// every region of the page with its parent (the timeline and made pieces are
// never walked: no region is in them)
export function regions(page) {
  // (regions sit a few levels under .app: the sidebar's rows and the like
  // below that are never walked)
  const found = new Map(), stack = [[page, null, 0, 0]];
  while (stack.length) {
    const [n, parent, index, depth] = stack.pop();
    if (!is(n, 'El') || n.tag === 'bp-memo' || depth > DEPTH || attr(n, 'id') === 'timeline') continue;
    const kind = REGIONS.find(([k, test]) => !found.has(k) && test(n));
    if (kind && parent) { found.set(kind[0], { node: n, parent: sig(parent), parentClass: attr(parent, 'class'), index, prev: index ? sig(items(parent.kids)[index - 1]) : '' });continue; }
    for (const [k, test] of Object.values(ALONG).flat()) if (!found.has('+' + k) && parent && test(n)) found.set('+' + k, { node: n, parent: sig(parent), index });
    const ks = items(n.kids);
    for (let i = ks.length - 1; i >= 0; i--) stack.push([ks[i], n, i, depth + 1]);
  }
  return found;
}
const shape = found => REGIONS.map(([k]) => found.has(k) ? '1' : '0').join('');

export class PressFeedback {
  constructor(changed, limit = 64) { this.changed = changed;this.limit = limit;this.learned = new Map();this.clear(); }
  // a page the hub drew for `target`
  seen(page, target) {
    if (page === this.seenNode) return;
    this.seenNode = page;this.now = regions(page);this.nowFor = target;
    // (a pane as the hub drew it, any destination: the shape of a first open)
    if (this.now.has('viewer')) this.shell = this.now.get('viewer');
    const p = this.press;
    if (!p?.settled || p.target !== target) return;
    this.press = null;
    // exactly one region opened, shut or changed: that is what the press gives
    const moved = REGIONS.map(([k]) => k).filter(k => p.before.get(k)?.node !== this.now.get(k)?.node);
    if (moved.length !== 1) return;
    const kind = moved[0], after = this.now.get(kind) || null, before = p.before.get(kind) || null;
    if (!after && REPLACES[kind]) return;
    // (which press put the pane on show: the same press again shuts it)
    if (kind === 'viewer') this.viewerBy = after ? p.by : '';
    const parent = after || before, parentClass = after ? after.parentClass : before && this.parentClass(before.parent);
    this.learned.delete(p.key);
    const along = (ALONG[kind] || []).map(([k]) => this.now.get('+' + k)).filter(Boolean);
    this.learned.set(p.key, { kind, node: after?.node || null, parent: parent.parent, parentClass, index: parent.index, prev: parent.prev, along });
    while (this.learned.size > this.limit) this.learned.delete(this.learned.keys().next().value);
  }
  parentClass(parentSig) {
    // a shut region's parent, as the hub draws it now
    let out;
    const walk = n => { if (out !== undefined || !is(n, 'El') || n.tag === 'bp-memo') return;if (sig(n) === parentSig) { out = attr(n, 'class');return; }for (const k of items(n.kids)) walk(k); };
    walk(this.seenNode);return out;
  }
  // a press: a function that settles it (the hub's answer), or undefined.
  // back: a retained picture of what a REPLACES region (Settings) puts back
  // when it shuts, as the host has it (the thread's own picture): drawn in
  // its place at the press
  choice(action, value, target, back = null) {
    if (NEVER.has(action) || !this.now || this.nowFor !== target) return undefined;
    const key = JSON.stringify([target, action, String(value ?? ''), shape(this.now)]);
    const revision = ++this.revision, press = { key, target, before: this.now, settled: false, revision, by: action + '\u0000' + String(value ?? '') };
    this.press = press;
    const known = this.learned.get(key) || this.opening(action, action + '\u0000' + String(value ?? '')) || this.closing(back);
    this.active = known ? { ...known, revision } : null;
    if (known) this.changed();
    return () => {
      press.settled = true;
      if (this.active?.revision === revision) { this.active = null;this.changed(); }
    };
  }
  // A viewer opened for the first time here: its pane, empty, where the hub
  // puts it (the same column and size), so the click shows at once and the
  // hub's pane fills it; nothing else moves again. With a pane open, another
  // tab's press empties it the same way; the press that put it on show (it
  // shuts it) and a pane whose press is unknown are left to the hub.
  opening(action, by) {
    if (!VIEWER.has(action)) return null;
    if (this.now.has('viewer') && (!this.viewerBy || this.viewerBy === by)) return null;
    const sh = this.now.get('viewer') || this.shell || this.bare();
    if (!sh) return null;
    const ks = items(sh.node.kids), edge = ks.find(k => is(k, 'El') && classes(k).includes('viewer-split'));
    const id = attr(sh.node, 'id');
    const pane = { ...sh.node, key: sh.node.key, attrs: list([{ ...items(sh.node.attrs).find(a => is(a, 'At')), name: 'class', value: 'viewer pending' },
      ...(id ? [{ ...items(sh.node.attrs).find(a => is(a, 'At')), name: 'id', value: id }] : [])]),
      kids: list([...(edge ? [edge] : []), { ...sh.node, tag: 'div', key: 'viewer-pending', attrs: list([{ ...items(sh.node.attrs).find(a => is(a, 'At')), name: 'class', value: 'viewer-stage' }]), kids: list([]) }]) };
    return { kind: 'viewer', node: pane, parent: sh.parent, addClass: 'panel', index: sh.index };
  }
  // Settings shut: the thread's main and top bar from its retained picture
  // take Settings' place (and the top bar's)
  closing(back) {
    const open = this.now?.get('settings');
    if (!back || !open) return null;
    const find = (n, test, d = 0) => { if (!is(n, 'El') || d > DEPTH || n.tag === 'bp-memo') return null;if (test(n)) return n;for (const k of items(n.kids)) { const f = find(k, test, d + 1);if (f) return f; }return null; };
    const main = find(back, n => n.tag === 'main' && classes(n).includes('thread-view'));
    if (!main) return null;
    const top = find(back, n => n.tag === 'header' && classes(n).includes('topbar')), here = this.now.get('+topbar');
    return { kind: 'settings', node: main, parent: open.parent, index: open.index, prev: open.prev, along: top && here ? [{ node: top, parent: here.parent }] : [] };
  }
  // (no pane seen yet: one made from the page's own pieces, last in .app)
  bare() {
    let app = null, at = null;
    const walk = (n, d) => { if (app || !is(n, 'El') || d > 4) return;if (classes(n).includes('app')) { app = n;return; }for (const k of items(n.kids)) walk(k, d + 1); };
    walk(this.seenNode, 0);
    if (app) at = items(app.attrs).find(a => is(a, 'At'));
    if (!app || !at) return null;
    return { node: { ...app, tag: 'aside', key: 'viewer', attrs: list([{ ...at, name: 'class', value: 'viewer' }, { ...at, name: 'id', value: 'viewer' }]), kids: list([]) },
      parent: sig(app), index: items(app.kids).length };
  }
  get showing() { return !!this.active; }
  project(page) {
    const a = this.active;
    if (!a) return page;
    const test = REGIONS.find(([k]) => k === a.kind)[1];
    let done = false;
    const walk = n => {
      if (done || !is(n, 'El') || n.tag === 'bp-memo' || attr(n, 'id') === 'timeline') return n;
      if (sig(n) === a.parent) {
        done = true;
        let ks = items(n.kids), at = ks.findIndex(test);
        const away = REPLACES[a.kind];
        if (away && at < 0) { const i = ks.findIndex(away);if (i >= 0) { ks = ks.map((k, j) => j === i ? a.node : k);at = i; } }
        const kids = at >= 0 ? (a.node ? ks.map((k, i) => i === at ? a.node : k) : ks.filter((_, i) => i !== at))
          : a.node ? (at => [...ks.slice(0, at), a.node, ...ks.slice(at)])(this.place(ks, a)) : ks;
        const out = { ...n, kids: list(kids) };
        if (a.addClass) { const c = attr(out, 'class') || '';return c.split(/\s+/).includes(a.addClass) ? out : withAttr(out, 'class', (c + ' ' + a.addClass).trim()); }
        return a.parentClass === undefined ? out : withAttr(out, 'class', a.parentClass);
      }
      const ks = items(n.kids), mapped = ks.map(walk);
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    const out = walk(page);
    return (a.along || []).reduce((pg, x) => this.swap(pg, x), out);
  }
  // where a region goes among its parent's kids: after the sibling it came
  // after when the hub drew it (the kids may be more or fewer now), else at
  // the same index
  place(ks, a) {
    if (a.prev !== undefined) { const i = a.prev ? ks.findIndex(k => is(k, 'El') && sig(k) === a.prev) : -1;if (i >= 0 || !a.prev) return i + 1; }
    return Math.min(a.index, ks.length);
  }
  // a region drawn along: put in place of its kind in its parent
  swap(page, x) {
    const test = Object.values(ALONG).flat().find(([, t]) => t(x.node))?.[1];
    if (!test) return page;
    let done = false;
    const walk = n => {
      if (done || !is(n, 'El') || n.tag === 'bp-memo' || attr(n, 'id') === 'timeline') return n;
      if (sig(n) === x.parent) { done = true;const ks = items(n.kids), at = ks.findIndex(test);return at < 0 ? n : { ...n, kids: list(ks.map((k, i) => i === at ? x.node : k)) }; }
      const ks = items(n.kids), mapped = ks.map(walk);
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    return walk(page);
  }
  // (another destination picked: a press on its way is the hub's to answer)
  drop() { this.revision++;this.active = null;this.press = null; }
  clear() { this.revision = (this.revision || 0) + 1;this.active = null;this.press = null;this.now = null;this.shell = null;this.nowFor = '';this.seenNode = null;this.learned?.clear(); }
}
