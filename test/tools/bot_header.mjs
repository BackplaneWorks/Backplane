import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import {pathToFileURL} from 'node:url';

const lib = (await import(pathToFileURL(process.argv[2]).href)).default;
const source = fs.readFileSync('src/web/host.js', 'utf8');
const list = xs => xs.reduceRight((tail, head) => ({$: 'Con', head, tail}), {$: 'Nil'});
function* each(xs) { for (; xs?.$ === 'Con'; xs = xs.tail) yield xs.head; }
function vnode(n) {
  return n.type === 'text' ? {$: 'Txt', text: n.text} : {$: 'El', tag: n.tag, key: n.key,
    attrs: list(n.attrs.map(a => a.event ? {$: 'On', ...a} : {$: 'At', ...a})), kids: list(n.kids.map(vnode))};
}
const attr = (n, key) => [...each(n.attrs)].find(a => a.name === key)?.value;
function all(n) { return [n, ...[...each(n.kids)].flatMap(all)]; }
const has = (n, cls) => (attr(n, 'class') || '').split(/\s+/).includes(cls);

// Minimal DOM for the production reconciler. The timeline is outside this
// test: only the actual bot chrome and changing neighbouring controls enter it.
class Element {
  constructor(tag) { this.tagName = tag; this.childNodes = []; this.attributes = new Map(); this.parentNode = null; }
  getAttribute(k) { return this.attributes.get(k) ?? null; }
  hasAttribute(k) { return this.attributes.has(k); }
  setAttribute(k, v) { this.attributes.set(k, v); }
  removeAttribute(k) { this.attributes.delete(k); }
  removeChild(n) { this.childNodes.splice(this.childNodes.indexOf(n), 1); n.parentNode = null; }
  insertBefore(n, at) {
    if (n.parentNode) n.parentNode.removeChild(n);
    this.childNodes.splice(at ? this.childNodes.indexOf(at) : this.childNodes.length, 0, n);
    n.parentNode = this;
  }
}
const doc = {activeElement: null, createElement: tag => new Element(tag),
  createElementNS: (_ns, tag) => new Element(tag), createTextNode: text => Object.assign(new Element('#text'), {nodeValue: text})};
const ctx = vm.createContext({document: doc, T: {El: 'El'}, is: (n, type) => n?.$ === type, each,
  viewMode: 'compatibility', frontendNode: null, boardCatalog: {renderOpen: () => null}, App: {cat: () => ''}});
vm.runInContext(source.slice(source.indexOf('const SVG_NS ='), source.indexOf('// Timing, for a bench only:')), ctx);
const container = ctx.create({$: 'El', tag: 'section', key: 'fixture', attrs: list([]), kids: list([])});
const marker = tag => ({$: 'El', tag, key: '', attrs: list([{$: 'At', name: 'class', value: 'other-control'}]), kids: list([])});
let checks = 0;
for (const rail of [false, true]) {
  let head, tabs;
  for (const step of [0, 1, 2, 3, 4, 5, 6, 0, 3, 4]) {
    const page = vnode(JSON.parse(lib.scenario(BigInt(step), rail))), nodes = all(page);
    const heads = nodes.filter(n => has(n, 'bot-head')), navs = nodes.filter(n => has(n, 'bot-primary-tabs'));
    assert.equal(heads.length, 1, `one bot header: rail=${rail}, step=${step}`);
    assert.equal(navs.length, 1, `one primary tab row: rail=${rail}, step=${step}`);
    const composer = nodes.findIndex(n => has(n, 'composer'));
    if (composer !== -1) assert(nodes.indexOf(heads[0]) < composer && nodes.indexOf(navs[0]) < composer, 'bot chrome precedes composer');
    const chrome = [heads[0], navs[0]];
    // Unkeyed matching previously recycled the bot's header/nav for these
    // siblings, then created another pair. Explicit keys keep the same pair.
    for (const prefix of [[], [marker('header'), marker('nav')], []]) {
      ctx.patch(container, {$: 'El', tag: 'section', key: 'fixture', attrs: list([]), kids: list([...prefix, ...chrome])});
      const actualHead = container.childNodes.find(n => n.getAttribute('class')?.includes('bot-head'));
      const actualTabs = container.childNodes.find(n => n.getAttribute('class')?.includes('bot-primary-tabs'));
      if (head) assert.equal(actualHead, head, 'header keeps its DOM identity across neighbouring controls and page changes');
      if (tabs) assert.equal(actualTabs, tabs, 'tabs keep their DOM identity across neighbouring controls and page changes');
      head = actualHead; tabs = actualTabs;
      assert.equal(container.childNodes.length, prefix.length + 2, 'no stale chrome remains after reconciliation');
      checks++;
    }
  }
  ctx.patch(container, {$: 'El', tag: 'section', key: 'fixture', attrs: list([]), kids: list([])});
}
console.log(`ok bot header: ${checks} production DOM reconciliations, legacy and rail layouts`);
