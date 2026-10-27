'use strict';
const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
function* each(xs) { for (; xs?.$ === 'Con'; xs = xs.tail) yield xs.head; }
const row = id => ({ $: 'El', tag: 'div', key: id, attrs: list([{ $: 'At', name: 'data-id', value: id }, { $: 'At', name: 'class', value: 'msg assistant' }]), kids: list([]) });
const fold = (id, ids) => ({ $: 'El', tag: 'button', key: '', attrs: list([{ $: 'At', name: 'data-value', value: '1' + id }, { $: 'On', event: 'click', action: 'fold' }, ...(ids ? [{$:'At',name:'data-fold-ids',value:JSON.stringify(ids)}] : [])]), kids: list([]) });
const timeline = kids => ({ $: 'El', tag: 'div', key: 'timeline', attrs: list([{ $: 'At', name: 'id', value: 'timeline' }]), kids: list(kids) });
function fixture() {
  const source = fs.readFileSync(process.env.HISTORY_HOST_SOURCE || path.resolve(__dirname, '../../src/web/host.js'), 'utf8');
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

const ids = (start, count = 32) => Array.from({length:count}, (_,i) => 'entry-'+String(start+i).padStart(3,'0'));
const coverage = (f, n) => [...each(n.kids)].flatMap(f.context.historyCoverage);
test('history coverage coalesces a changed tool head without a shared following message and holds its offset', () => {
  const f = fixture();f.show([fold('entry-016',ids(16))]);f.context.held = { id: 'fold:entry-016', off: 30 };
  const shown = f.show([fold('entry-000',ids(0))], 'older');
  assert.deepEqual(f.ids(shown), ['entry-000']);assert.deepEqual(coverage(f,shown),ids(0));
  assert.equal(f.context.held.id, 'fold:entry-000');assert.equal(f.context.held.off, 30);
  // The adjacent issued page still exposes its unique calls when reversed.
  const newer=f.show([fold('entry-016',ids(16))],'newer');assert.deepEqual(coverage(f,newer),ids(16));
  assert.deepEqual([...new Set([...coverage(f,shown),...coverage(f,newer)])].sort(),ids(0,48));
});
test('history coverage removes the exact duplicated subset and preserves unique messages and disjoint calls', () => {
  const f=fixture(),rows=ids(720,6).map(row);
  f.show([fold('entry-704',ids(704,16)),...rows,fold('entry-726',ids(726,10))]);
  const shown=f.show([fold('entry-688',ids(688,8)),...ids(696,6).map(row),fold('entry-702',ids(702,18))],'older');
  assert.deepEqual(f.ids(shown),['entry-688',...ids(696,6),'entry-702',...ids(720,6),'entry-726']);
  const seen=coverage(f,shown);assert.equal(new Set(seen).size,seen.length);assert.deepEqual(seen,ids(688,48));
});
test('retained fold returns to its issued adjacent page before toggling, then opens and closes the correct unique calls', () => {
  const f=fixture(),calls=[];f.context.thin.retainedFold=(...args)=>calls.push(args);
  f.show([row('entry-032'),fold('entry-033',ids(33,15))]);
  f.show([fold('entry-000',ids(0,16)),...ids(16,16).map(row)],'older');
  assert.equal(f.context.historyRetainedFold('1entry-033'),true);assert.deepEqual(calls,[['1entry-033','newer']]);
  f.show([row('entry-032'),fold('entry-033',ids(33,15)),...ids(33,15).map(row)],'newer');
  assert.equal(f.context.historyRetainedFold('0entry-033'),false,'canonical fold toggles directly');
  const opened=f.show([row('entry-032'),fold('entry-033',ids(33,15)),...ids(33,15).map(row)]);
  assert.deepEqual(f.ids(opened).filter(id=>id>='entry-033'),['entry-033',...ids(33,15)]);
  const closed=f.show([row('entry-032'),fold('entry-033',ids(33,15))]);assert.deepEqual(f.ids(closed).slice(-2),['entry-032','entry-033']);
});
test('history coverage respects messages and requires bounded actual-entry metadata', () => {
  const f=fixture();assert.deepEqual(Array.from(f.context.historyCoverage(fold('x',[]))),['x']);
  assert.deepEqual(Array.from(f.context.historyCoverage(fold('x',ids(0,33)))),['x']);
  assert.deepEqual(Array.from(f.context.historyCoverage(fold('x',['x','x']))),['x']);
  assert.deepEqual(Array.from(f.context.historyCoverage(fold('x',['unrelated']))),['x']);
  f.show([fold('a',['a','b']),row('c'),fold('d',['d'])]);
  const shown=f.show([fold('0',['0','1']),row('2')],'older');assert.deepEqual(f.ids(shown),['0','2','a','c','d']);
});
