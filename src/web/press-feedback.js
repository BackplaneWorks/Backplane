// What a press opens, shown at the press: the hub's views are drawn by the
// hub, so a press that opens a menu, a popover or a viewer pane waited a
// whole round trip for anything to change. A press the hub has answered
// once by opening, closing or changing one region of the page (the command
// palette, the viewer pane, the account and token pickers, a row's menu, a
// project's colour picker) is remembered with the region it gave (and the
// shade drawn before it): the same press on the same
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
  // a project's colour and icon picker, in its sidebar section
  ['look', n => n.tag === 'div' && classes(n).includes('look')],
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
// The rail beside a viewer press: the pressed tab marked as the one on
// show and "Close the viewer" at its foot, as the hub draws them, so neither
// pops in (and the rail's toggle drops) when the hub's view lands. Drawn
// from the tabs and the button as the hub drew them in some earlier view
// (both states of each tab); with any of them unknown the rail is left as
// it is.
const VIEW_TABS = new Set(['board', 'schematic', '3d', 'split', 'mech', 'files', 'diff', 'review', 'browser']);
const TAB_OF = { fm: () => 'files', diff: () => 'diff', mech: () => 'mech', 'rv-page': () => 'review', 'view-kind': v => v };
// (a tab never seen on show, as view.bend's View.workspace.button draws the
// one on show: marked, and its click shuts the viewer, client.bend's Rail.act)
const ON_ATTRS = { class: 'tab on', 'aria-current': 'page', 'aria-pressed': 'true', 'data-value': '' };
const onTab = n => ({ ...n, attrs: list(items(n.attrs).map(a => is(a, 'At') ? (a.name in ON_ATTRS ? { ...a, value: ON_ATTRS[a.name] } : a) : a.event === 'click' ? { ...a, action: 'panel' } : a)) });
const isRail = n => n.tag === 'nav' && n.key === 'workspace' && classes(n).includes('workspace-bar');
// A sidebar shelf (a project's Settled, Older projects, Settled in the
// active view, Conversation history) opens or shuts by its press: what the
// hub drew for the same press, with its section the same shape before it
// (so an open and a shut one never stand in for each other), is drawn at
// the press: the one section of the sidebar that press changed, its rows as
// the hub drew them then; the hub's view then draws it in place.
const SHELVES = new Set(['toggle-settled', 'side-older', 'side-settled', 'thread-history']);
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
    if (kind && parent) {
      // (a popover's shade, drawn just before it: comes and goes with it, so
      // the hub's next view finds both where it puts them)
      const ks = items(parent.kids), shade = index && isShade(ks[index - 1]) ? ks[index - 1] : null, at = shade ? index - 1 : index;
      found.set(kind[0], { node: n, parent: sig(parent), parentClass: attr(parent, 'class'), index: at, prev: at ? sig(ks[at - 1]) : '', ...shade ? { pre: [shade] } : {} });continue;
    }
    for (const [k, test] of Object.values(ALONG).flat()) if (!found.has('+' + k) && parent && test(n)) found.set('+' + k, { node: n, parent: sig(parent), index });
    const ks = items(n.kids);
    for (let i = ks.length - 1; i >= 0; i--) stack.push([ks[i], n, i, depth + 1]);
  }
  return found;
}
const isShade = n => is(n, 'El') && n.tag === 'div' && !n.key && classes(n).includes('shade') && !items(n.kids).length;
// the sidebar's sections, by their place's signature (null: two alike)
function sections(page) {
  let side = null;
  const find = (n, d) => { if (side || !is(n, 'El') || d > DEPTH || n.tag === 'bp-memo') return;if (n.tag === 'aside' && n.key === 'sidebar') { side = n;return; }for (const k of items(n.kids)) find(k, d + 1); };
  find(page, 0);
  if (!side) return null;
  const out = new Map(), walk = (n, d) => {
    if (!is(n, 'El') || d > 3 || n.tag === 'bp-memo') return;
    if (n.tag === 'section') { const k = sig(n);out.set(k, out.has(k) ? null : n);return; }
    for (const k of items(n.kids)) walk(k, d + 1);
  };
  for (const k of items(side.kids)) walk(k, 0);
  return out;
}
// a section's shape: its elements, keys and classes (not its text), hashed
function shapeOf(n) {
  let h = 2166136261;
  const add = str => { for (let i = 0; i < str.length; i++) h = Math.imul(h ^ str.charCodeAt(i), 16777619); };
  const walk = (x, d) => {
    if (!is(x, 'El') || d > 9) return;
    add('<' + x.tag + '\u0000' + x.key + '\u0000' + (x.tag === 'bp-memo' ? '' : attr(x, 'class') || ''));
    if (x.tag !== 'bp-memo') for (const k of items(x.kids)) walk(k, d + 1);
    add('>');
  };
  walk(n, 0);
  return h >>> 0;
}
const shape = found => REGIONS.map(([k]) => found.has(k) ? '1' : '0').join('');

// What was learned is kept across reloads (an app start drew each menu's
// first open from the hub again): in storage under the hub, the pairing
// identity, the bundle and the language (scope()), at most STORE_BYTES,
// fields' values left out (what the hub's view fills in), written a while
// after a press taught something.
const STORE = 'bp-press-v1:', STORE_BYTES = 1024 * 1024;
const blank = n => {
  if (!is(n, 'El')) return n;
  const field = ['input', 'textarea', 'select', 'option'].includes(n.tag);
  const attrs = field ? list(items(n.attrs).filter(a => !is(a, 'At') || !['value', 'checked', 'selected'].includes(a.name))) : n.attrs;
  return { ...n, attrs, kids: n.tag === 'textarea' ? list([]) : list(items(n.kids).map(blank)) };
};
const AHEADS = 32;
const shaped = n => n === null || is(n, 'El') && typeof n.tag === 'string' && n.attrs && n.kids;
export class PressFeedback {
  // store: {storage (localStorage), scope: () => a key for this hub and page, or ''}
  constructor(changed, limit = 64, store = null) { this.changed = changed;this.limit = limit;this.store = store;this.learned = new Map();this.aheads = new Map();this.tabs = new Map();this.closer = null;this.shelves = new Map();this.clear(); }
  restore() {
    const scope = this.store?.scope?.() || '';
    if (!scope || scope === this.storedFor) return;
    this.storedFor = scope;
    try {
      const kept = JSON.parse(this.store.storage.getItem(STORE + scope) || '[]'), rows = Array.isArray(kept) ? kept : kept?.rows;
      for (const [k, n] of Array.isArray(kept?.rail?.tabs) ? kept.rail.tabs : []) if (typeof k === 'string' && shaped(n) && n && !this.tabs.has(k)) this.tabs.set(k, n);
      if (!this.closer && shaped(kept?.rail?.closer) && kept.rail.closer) this.closer = kept.rail.closer;
      for (const [k, e] of Array.isArray(kept?.shelves) ? kept.shelves : []) if (typeof k === 'string' && e?.kind === 'section' && typeof e.sec === 'string' && shaped(e.node) && e.node && !this.shelves.has(k)) this.shelves.set(k, e);
      for (const [key, e] of Array.isArray(rows) ? rows : []) {
        if (typeof key !== 'string' || this.learned.has(key) || !e || !REGIONS.some(([k]) => k === e.kind) || !shaped(e.node) || typeof e.parent !== 'string') continue;
        if (!(e.along || []).every(x => x && shaped(x.node) && x.node && typeof x.parent === 'string')) continue;
        if (e.pre !== undefined && !(Array.isArray(e.pre) && e.pre.every(isShade))) continue;
        this.learned.set(key, e);
      }
      while (this.learned.size > this.limit) this.learned.delete(this.learned.keys().next().value);
    } catch {}
  }
  persist() {
    if (!this.store || this.saving) return;
    this.saving = setTimeout(() => {
      this.saving = 0;
      const scope = this.storedFor;
      if (!scope) return;
      try {
        const rail = { tabs: [...this.tabs], closer: this.closer };
        let shelves = [...this.shelves].map(([k, e]) => [k, { ...e, node: blank(e.node) }]);
        let rows = [...this.learned].map(([key, e]) => [key, { ...e, node: e.node && blank(e.node), along: (e.along || []).map(x => ({ ...x, node: blank(x.node) })) }]), text = JSON.stringify({ rows, rail, shelves });
        while (text.length > STORE_BYTES && (rows.length || shelves.length)) { if (rows.length) rows = rows.slice(1);else shelves = shelves.slice(1);text = JSON.stringify({ rows, rail, shelves }); }
        const st = this.store.storage;
        for (let i = st.length - 1; i >= 0; i--) { const k = st.key(i);if (k && k.startsWith(STORE) && k !== STORE + scope) st.removeItem(k); }
        st.setItem(STORE + scope, text);
      } catch {}
    }, 2000);
  }
  // a page the hub drew for `target`
  seen(page, target) {
    if (page === this.seenNode) return;
    this.restore();
    this.seenNode = page;this.now = regions(page);this.nowFor = target;
    this.learnRail(page);
    // (a pane as the hub drew it, any destination: the shape of a first open)
    if (this.now.has('viewer')) this.shell = this.now.get('viewer');
    const p = this.press;
    if (!p?.settled || p.target !== target) return;
    this.press = null;
    if (p.shelf) this.learnShelf(p, page);
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
    this.learned.set(p.key, { kind, node: after?.node || null, parent: parent.parent, parentClass, index: parent.index, prev: parent.prev, along, ...parent.pre ? { pre: parent.pre } : {} });
    while (this.learned.size > this.limit) this.learned.delete(this.learned.keys().next().value);
    this.persist();
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
    if (SHELVES.has(action)) { press.shelf = JSON.stringify([target, action, String(value ?? '')]);press.sections = sections(this.seenNode); }
    const known = (press.shelf ? this.shelfFor(press) : this.learned.get(key) || this.aheads.get(key)) || this.opening(action, action + '\u0000' + String(value ?? '')) || this.closing(back);
    this.active = known ? { ...known, revision, rail: known.kind === 'viewer' ? this.railKind(action, value, known) : undefined } : null;
    if (known) this.changed();
    return () => {
      press.settled = true;
      if (this.active?.revision === revision) { this.active = null;this.changed(); }
    };
  }
  // A viewer as the hub draws it open, read ahead for the thread on show
  // (view-prefetch.js's enqueueViewers): what its rail tab's press gives, so
  // the first open of each viewer draws at the press too. Kept apart from
  // what presses taught (never stored, never pushes a menu out), at most
  // AHEADS, the oldest out; a press's own answer is learned over it.
  ahead(page, target, action, value, kind) {
    if (!this.now || this.nowFor !== target || !VIEWER.has(action)) return;
    const after = regions(page).get('viewer');
    // (only the viewer it was asked for: class "viewer <kind>")
    if (!after || !classes(after.node).includes(kind)) return;
    const entry = { kind: 'viewer', node: after.node, parent: after.parent, parentClass: after.parentClass, index: after.index, prev: after.prev, along: [] };
    // (pressed from the thread alone or with another viewer open)
    const base = shape(this.now), at = REGIONS.findIndex(([k]) => k === 'viewer');
    for (const open of ['0', '1']) {
      const key = JSON.stringify([target, action, String(value ?? ''), base.slice(0, at) + open + base.slice(at + 1)]);
      this.aheads.delete(key);this.aheads.set(key, entry);
    }
    while (this.aheads.size > AHEADS) this.aheads.delete(this.aheads.keys().next().value);
  }
  // (the one section a shelf's press changed, as the hub drew it after)
  learnShelf(p, page) {
    const before = p.sections, after = sections(page);
    if (!before || !after || before.size !== after.size) return;
    const moved = [];
    for (const [k, n] of before) {
      const m = after.get(k);
      if (!n || !m || m === undefined) return;
      if (m !== n && JSON.stringify(m) !== JSON.stringify(n)) moved.push(k);
    }
    if (moved.length !== 1) return;
    // (by the section and its shape before: an open and a shut shelf are two)
    const key = p.shelf + '\u0000' + moved[0] + '\u0000' + shapeOf(before.get(moved[0]));
    this.shelves.delete(key);this.shelves.set(key, { kind: 'section', sec: moved[0], node: after.get(moved[0]) });
    while (this.shelves.size > this.limit) this.shelves.delete(this.shelves.keys().next().value);
    this.persist();
  }
  shelfFor(press) {
    for (const [k, n] of press.sections || []) {
      const s = n && this.shelves.get(press.shelf + '\u0000' + k + '\u0000' + shapeOf(n));
      if (s) return s;
    }
    return null;
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
  // (each viewer tab as the hub drew it on and off, and its Close button)
  learnRail(page) {
    const rail = this.find(page, isRail);
    if (!rail) return;
    const walk = n => {
      if (!is(n, 'El') || n.tag === 'details') return;
      if (n.tag === 'button' && VIEW_TABS.has(n.key) && classes(n).includes('tab')) { const k = n.key + '\u0000' + (classes(n).includes('on') ? 1 : 0);if (this.tabs.get(k) !== n) { this.tabs.delete(k);this.tabs.set(k, n); } }
      else if (n.tag === 'button' && classes(n).includes('viewer-close')) this.closer = n;
      else for (const k of items(n.kids)) walk(k);
    };
    walk(rail);
    while (this.tabs.size > 2 * VIEW_TABS.size) this.tabs.delete(this.tabs.keys().next().value);
  }
  find(n, test, d = 0) {
    if (!is(n, 'El') || d > DEPTH || n.tag === 'bp-memo' || attr(n, 'id') === 'timeline') return null;
    if (test(n)) return n;
    for (const k of items(n.kids)) { const f = this.find(k, test, d + 1);if (f) return f; }
    return null;
  }
  // the tab on show after a viewer press: the pressed one when it opens or
  // switches the pane, none when it shuts it; undefined: unknown (the
  // header's Viewer, which opens the last one)
  railKind(action, value, known) {
    if (!known.node) return '';
    const tab = TAB_OF[action]?.(String(value ?? ''));
    return VIEW_TABS.has(tab) ? tab : undefined;
  }
  railTo(page, on) {
    let whole = true;
    const swap = n => {
      if (!is(n, 'El') || n.tag === 'details') return n;
      if (n.tag === 'button' && VIEW_TABS.has(n.key) && classes(n).includes('tab')) {
        const want = n.key === on;
        if (want === classes(n).includes('on')) return n;
        const v = this.tabs.get(n.key + '\u0000' + (want ? 1 : 0)) || (want ? onTab(n) : null);
        if (!v) whole = false;
        return v || n;
      }
      if (n.tag === 'div' && classes(n).includes('workspace-close')) {
        if (on && !this.closer) { whole = false;return n; }
        const ks = items(n.kids), want = on ? [this.closer] : [];
        return ks.length === want.length && ks.every((k, i) => k === want[i]) ? n : { ...n, kids: list(want) };
      }
      const ks = items(n.kids), mapped = ks.map(swap);
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    let done = false;
    const walk = (n, d) => {
      if (done || !is(n, 'El') || d > DEPTH || n.tag === 'bp-memo' || attr(n, 'id') === 'timeline') return n;
      if (isRail(n)) { done = true;const out = swap(n);return whole ? out : n; }
      const ks = items(n.kids), mapped = ks.map(k => walk(k, d + 1));
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    const out = walk(page, 0);
    return whole ? out : page;
  }
  get showing() { return !!this.active; }
  project(page) {
    const a = this.active;
    if (!a) return page;
    if (a.kind === 'section') return this.swapSection(page, a);
    const test = REGIONS.find(([k]) => k === a.kind)[1];
    let done = false;
    const walk = n => {
      if (done || !is(n, 'El') || n.tag === 'bp-memo' || attr(n, 'id') === 'timeline') return n;
      if (sig(n) === a.parent) {
        done = true;
        let ks = items(n.kids), at = ks.findIndex(test);
        const away = REPLACES[a.kind];
        if (away && at < 0) { const i = ks.findIndex(away);if (i >= 0) { ks = ks.map((k, j) => j === i ? a.node : k);at = i; } }
        // (its shade before it, as the hub draws them: put in with it, taken away with it)
        const pre = a.pre || [], shaded = at > 0 && isShade(ks[at - 1]);
        const kids = at >= 0 ? (a.node ? [...ks.slice(0, at), ...shaded ? [] : pre, a.node, ...ks.slice(at + 1)]
          : ks.filter((_, i) => i !== at && !(shaded && i === at - 1)))
          : a.node ? (at => [...ks.slice(0, at), ...pre, a.node, ...ks.slice(at)])(this.place(ks, a)) : ks;
        const out = { ...n, kids: list(kids) };
        if (a.addClass) { const c = attr(out, 'class') || '';return c.split(/\s+/).includes(a.addClass) ? out : withAttr(out, 'class', (c + ' ' + a.addClass).trim()); }
        return a.parentClass === undefined ? out : withAttr(out, 'class', a.parentClass);
      }
      const ks = items(n.kids), mapped = ks.map(walk);
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    const out = walk(page);
    const railed = a.rail === undefined ? out : this.railTo(out, a.rail);
    return (a.along || []).reduce((pg, x) => this.swap(pg, x), railed);
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
  swapSection(page, a) {
    let done = false;
    const walk = (n, d, inSide) => {
      if (done || !is(n, 'El') || n.tag === 'bp-memo' || d > DEPTH + 3) return n;
      if (inSide && n.tag === 'section') { if (sig(n) !== a.sec) return n;done = true;return a.node; }
      const side = inSide || n.tag === 'aside' && n.key === 'sidebar';
      const ks = items(n.kids), mapped = ks.map(k => walk(k, d + 1, side));
      return mapped.every((k, i) => k === ks[i]) ? n : { ...n, kids: list(mapped) };
    };
    return walk(page, 0, false);
  }
  // (another destination picked: a press on its way is the hub's to answer)
  drop() { this.revision++;this.active = null;this.press = null; }
  clear() { this.revision = (this.revision || 0) + 1;this.active = null;this.press = null;this.now = null;this.shell = null;this.nowFor = '';this.seenNode = null;this.learned?.clear();this.aheads?.clear();this.tabs?.clear();this.closer = null;this.shelves?.clear(); }
}
