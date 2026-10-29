'use strict';
const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
function* each(xs) { for (; xs?.$ === 'Con'; xs = xs.tail) yield xs.head; }
const row = id => ({ $: 'El', tag: 'div', key: id, attrs: list([{ $: 'At', name: 'data-id', value: id }, { $: 'At', name: 'class', value: 'msg assistant' }]), kids: list([]) });
const fold = id => ({ $: 'El', tag: 'button', key: '', attrs: list([{ $: 'At', name: 'data-value', value: '1' + id }, { $: 'On', event: 'click', action: 'fold' }]), kids: list([]) });
const timeline = kids => ({ $: 'El', tag: 'div', key: 'timeline', attrs: list([{ $: 'At', name: 'id', value: 'timeline' }]), kids: list(kids) });
function fixture() {
  const source = fs.readFileSync(path.resolve(__dirname, '../../src/web/host.js'), 'utf8');
  const context = vm.createContext({ each, is: (x, kind) => x.$ === kind, held: null, TextEncoder,
    location:{origin:'https://fixture'},document:{cookie:'identity'},cacheIdentity:x=>x,App:{lang:()=> 'en'},ui:null,
    thin:{epoch:'epoch',target:{kind:'thread',id:'A'},conversation:'A'} });
  vm.runInContext(source.slice(source.indexOf('let historyBase ='), source.indexOf('// the entry at the top of the timeline')), context);
  return { context, show(kids, direction) { context.node = timeline(kids);context.direction = direction;
    return vm.runInContext('historyPage = direction ? {direction} : null;historyProject(node)', context); }, ids(n) { return [...each(n.kids)].map(context.historyRow).filter(Boolean); } };
}
test('rolling history keeps the visible outgoing suffix, deduplicates overlap and bounds 1000 pages', () => {
  const f = fixture();let end = 17000;
  for (let i = 0; i < 1000; i++, end -= 16) {
    const incoming = Array.from({ length: 32 }, (_, n) => row(String(end - 31 + n).padStart(6, '0')));
    const before = JSON.stringify(incoming), shown = f.show(incoming, i ? 'older' : null), ids = f.ids(shown);
    assert.equal(ids.length, i ? 48 : 32);assert.equal(new Set(ids).size, ids.length);
    assert.deepEqual(ids, ids.toSorted());assert.equal(JSON.stringify(incoming), before, 'validated input mutated');
    // A normal live/draft refresh must not accumulate another spare page.
    assert.deepEqual(f.ids(f.show(incoming)), ids);
  }
});
test('scrolling downward preserves the older prefix without duplicating overlap', () => {
  const f = fixture();f.show(Array.from({ length: 32 }, (_, n) => row(String(n).padStart(3, '0'))));
  const ids = f.ids(f.show(Array.from({ length: 32 }, (_, n) => row(String(n + 16).padStart(3, '0'))), 'newer'));
  assert.equal(ids.length, 48);assert.deepEqual(ids, ids.toSorted());
});
test('a fold that gains an earlier head does not leave a duplicate out-of-order run', () => {
  const f = fixture();f.show([fold('B'), row('C'), row('D'), row('E')]);
  f.context.held = { id: 'fold:B', off: 30 };
  const shown = f.show([row('0'), fold('A'), row('C'), row('D')], 'older');
  assert.deepEqual(f.ids(shown), ['0', 'A', 'C', 'D', 'E']);assert.equal(f.context.held.id, 'fold:A');assert.equal(f.context.held.off, 30);
});
test('a visible reheaded fold keeps its element key and never resurrects the obsolete head', () => {
  const f = fixture();f.show([fold('B'), row('C'), row('D'), row('E')]);
  f.context.held = { id: 'fold:B', off: 30 };vm.runInContext('historyKeep=["B","C"]', f.context);
  const shown = f.show([row('0'), fold('A'), row('C'), row('D')], 'older');
  assert.deepEqual(f.ids(shown), ['0', 'A', 'C', 'D', 'E']);
  assert.equal([...each(shown.kids)].find(n => f.context.historyRow(n) === 'A').key, 'fold:B');
  const refreshed = f.show([row('0'), fold('A'), row('C'), row('D')]);
  assert.equal([...each(refreshed.kids)].find(n => f.context.historyRow(n) === 'A').key, 'fold:B');
});
test('an anchor in the visual bridge cannot be remapped to a different canonical fold', () => {
  const f = fixture();f.show([fold('Z'), row('z-msg')]);f.show([fold('B'), row('C')], 'older');
  f.context.held = { id: 'fold:Z', off: 30 };vm.runInContext('historyKeep=["Z","z-msg"]', f.context);
  const shown = f.show([fold('A'), row('C')], 'older');
  assert.equal(f.context.held.id, 'fold:Z');assert.deepEqual(f.ids(shown), ['A', 'C', 'Z', 'z-msg']);
});
test('a short page without a shared rendered row still keeps the outgoing message', () => {
  const f = fixture();f.show([row('z')]);assert.deepEqual(f.ids(f.show([fold('a')], 'older')), ['a', 'z']);
  assert.deepEqual(f.ids(f.show([fold('0')], 'older')), ['0', 'a'], 'only the outgoing canonical page survives');
});
test('conversation restore retains the visual tail needed to keep an offset near the canonical page bottom', () => {
  const f = fixture(), rows = start => Array.from({length:32},(_,i)=>row(String(start+i).padStart(3,'0')));
  f.show(rows(16));const shown=f.show(rows(0),'older');f.context.historyRememberBridge();
  vm.runInContext('historyBase=null;historySpare=[];historyPage=null',f.context);
  f.context.historyRestoreBridge({target:{kind:'thread',id:'A'},position:{anchor:'030',offset:-700}});
  assert.deepEqual(f.ids(f.show(rows(0))),f.ids(shown));
});
test('restoring the adjacent issued page reconstructs the older visual prefix', () => {
  const f = fixture(), rows = start => Array.from({length:32},(_,i)=>row(String(start+i).padStart(3,'0')));
  f.show(rows(16));const shown=f.show(rows(0),'older');f.context.historyRememberBridge();
  vm.runInContext('historyBase=null;historySpare=[];historyPage=null',f.context);
  f.context.historyRestoreBridge({target:{kind:'thread',id:'A'},position:{anchor:'032',offset:0}});
  assert.deepEqual(f.ids(f.show(rows(16))),f.ids(shown));
});
test('visual bridge snapshots are bounded and never cross epoch or conversation identity', () => {
  const f=fixture();f.show([row('z')]);f.show([row('a')],'older');f.context.historyRememberBridge();
  for(let i=0;i<20;i++){f.context.thin.target.id='t'+i;f.context.historyRememberBridge();}
  assert.equal(vm.runInContext('historyBridges.size',f.context),8);assert(vm.runInContext('historyBridgeBytes<=512*1024',f.context));
  vm.runInContext('historyBase=null;historySpare=[];historyPage=null',f.context);f.context.thin.epoch='new-epoch';
  f.context.historyRestoreBridge({target:f.context.thin.target,position:{anchor:'a'}});assert.deepEqual(f.ids(f.show([row('a')])),['a']);
});
test('visual bridge snapshots exclude work and approval controls; current canonical rows win', () => {
  const f=fixture(), control={...row('work'),attrs:list([{$:'At',name:'data-id',value:'work'},{$:'At',name:'class',value:'emb'}])};
  f.show([row('b'),row('c'),control]);f.show([row('a'),row('b')],'older');f.context.historyRememberBridge();
  vm.runInContext('historyBase=null;historySpare=[];historyPage=null',f.context);
  f.context.historyRestoreBridge({target:{kind:'thread',id:'A'},position:{anchor:'b'}});
  const fresh={...row('b'),key:'fresh-b'},shown=f.show([row('a'),fresh]);
  assert.deepEqual(f.ids(shown),['a','b','c']);assert.equal([...each(shown.kids)][1].key,'fresh-b');
});
for (const transient of ['live','sending']) test('visual bridge snapshots never resurrect a '+transient+' message', () => {
  const f=fixture(), streaming={...row('transient'),attrs:list([{$:'At',name:'data-id',value:'transient'},{$:'At',name:'class',value:'msg assistant '+transient}])};
  f.show([row('b'),row('c'),streaming]);f.show([row('a'),row('b')],'older');f.context.historyRememberBridge();
  vm.runInContext('historyBase=null;historySpare=[];historyPage=null',f.context);
  f.context.historyRestoreBridge({target:{kind:'thread',id:'A'},position:{anchor:'b'}});
  assert.deepEqual(f.ids(f.show([row('a'),row('b')])),['a','b','c']);
});
test('visual bridge snapshots require a stable message id or fold head', () => {
  const f=fixture(), unstable={...row(''),attrs:list([{$:'At',name:'class',value:'msg assistant'}])};
  assert.equal(f.context.historySnapshotRow(unstable),false);assert.equal(f.context.historySnapshotRow(row('stable')),true);assert.equal(f.context.historySnapshotRow(fold('head')),true);
});
// S07 on the desktop app: two pages asked for before the first was drawn,
// the second dropped the rows on screen and the text jumped 1865 px
const rows = (from, to) => Array.from({ length: to - from + 1 }, (_, i) => row(String(from + i).padStart(3, '0')));
const ids3 = (from, to) => Array.from({ length: to - from + 1 }, (_, i) => String(from + i).padStart(3, '0'));
test('scrolling down, a second newer page keeps the rows on screen in the spare above', () => {
  const f = fixture();f.show(rows(0, 31));f.show(rows(16, 47), 'newer');
  f.context.held = { id: '010', off: 40 };vm.runInContext('historyKeep = ["010", "013"]', f.context);
  const ids = f.ids(f.show(rows(32, 63), 'newer'));
  assert.deepEqual(ids, ids3(10, 63), 'the rows on screen and those down to the new page stay, in order');
  vm.runInContext('historyKeep = ["040", "043"]', f.context);
  assert.deepEqual(f.ids(f.show(rows(48, 79), 'newer')), ids3(32, 79), 'read into the page, the usual half page is spare');
});
test('scrolling up, a second older page keeps the rows on screen in the spare below', () => {
  const f = fixture();f.show(rows(32, 63));f.show(rows(16, 47), 'older');
  f.context.held = { id: '050', off: 40 };vm.runInContext('historyKeep = ["050", "055"]', f.context);
  assert.deepEqual(f.ids(f.show(rows(0, 31), 'older')), ids3(0, 55), 'everything down to the last row on screen stays below');
});
test('a refresh that drew a page an action overtook keeps the rows on screen', () => {
  const f = fixture();f.show(rows(0, 31));
  f.context.held = { id: '005', off: 0 };vm.runInContext('historyKeep = ["005", "008"]', f.context);
  assert.deepEqual(f.ids(f.show(rows(16, 47))), ids3(5, 47));
  vm.runInContext('historyKeep = null', f.context);f.context.held = null;
  assert.deepEqual(f.ids(f.show(rows(16, 47))), ids3(5, 47), 'a later refresh keeps the spare');
});
test('nothing held, or all on screen still shown: the spare is the usual one', () => {
  const f = fixture();f.show(rows(0, 31));
  vm.runInContext('historyKeep = ["020", "024"]', f.context);f.context.held = { id: '020', off: 0 };
  assert.deepEqual(f.ids(f.show(rows(16, 47), 'newer')), ids3(0, 47));
  vm.runInContext('historyKeep = null', f.context);f.context.held = null;
  assert.deepEqual(f.ids(f.show(rows(32, 63), 'newer')), ids3(16, 63));
});
