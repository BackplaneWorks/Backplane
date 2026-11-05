'use strict';
// The composer's own completion popup (src/web/composer-popup.js): the words
// it opens for, the hub's slug and completion rules, the rows it keeps and
// narrows, and the time it takes at 1,000 threads (a popup must be up in the
// keystroke's frame: 50 ms is the budget on a phone, a quarter of it here).
const {test} = require('node:test'), assert = require('node:assert/strict');
const load = () => import('../../src/web/composer-popup.js');

test('a popup opens for the last word only, and a pick replaces just that word', async () => {
  const {popupWord, completeWord, lastWord} = await load();
  assert.equal(popupWord('look at >sid'), '>sid');
  assert.equal(popupWord('look at >sid '), '');
  assert.equal(popupWord('a%b'), '');
  assert.equal(popupWord('line one\n@cla'), '@cla');
  assert.equal(popupWord('/'), '/');
  assert.equal(lastWord('tab\tsep'), 'sep');
  assert.equal(completeWord('ask >sid', '>sidebar-lag'), 'ask >sidebar-lag ');
  assert.equal(completeWord('%', '%proj-20'), '%proj-20 ');
});

test('slugs follow refs.bend Slug.of', async () => {
  const {slugOf} = await load();
  assert.equal(slugOf('Fix the perf lag!'), 'fix-the-perf-lag');
  assert.equal(slugOf('  --Hello__World-- '), 'hello-world');
  assert.equal(slugOf('measure bootloader 20-21'), 'measure-bootloader-20-21');
  assert.equal(slugOf('日本語'), '');
  assert.equal(slugOf('a'.repeat(39) + ' b'), 'a'.repeat(39));
});

test('the hub rows are kept per word and narrowed for a longer one', async () => {
  const {PopupRows} = await load();
  const rows = new PopupRows(3);
  rows.put('t1', '>', [{value: '>measure-bootloader', desc: 'p: measure bootloader'}, {value: '>fix-sidebar', desc: 'fix sidebar'}, {value: '>port-usb-hub', desc: ''}]);
  assert.equal(rows.get('t2', '>').length, 3, 'thread words are the same in every thread');
  assert.deepEqual(rows.get('t1', '>sid').map(r => r.value), ['>fix-sidebar']);
  assert.deepEqual(rows.get('t1', '>zzz'), []);
  assert.equal(rows.get('t1', '%'), null, 'nothing known: the index or the hub answers');
  rows.put('t1', '$', [{value: '$review', desc: ''}]);
  assert.equal(rows.get('t2', '$'), null, 'skills are a thread\'s own');
  rows.put('t1', '>a', []);rows.put('t1', '>b', []);rows.put('t1', '>c', []);
  assert.equal(rows.get('t1', '>'), null, 'the oldest word goes past the limit');
});

function store(projects, perProject) {
  const threads = [], ps = [];
  const verbs = ['fix', 'add', 'speed up', 'review', 'route', 'measure'], things = ['sidebar', 'composer', 'usb hub', 'bootloader', 'latency', 'pairing'];
  for (let p = 0; p < projects; p++) {
    ps.push({id: 'p' + p, title: 'proj-' + p});
    for (let t = 0; t < perProject; t++) threads.push({id: `t1793-${p * perProject + t}`, title: `${verbs[(p + t) % 6]} ${things[(p * 3 + t) % 6]}`, project: 'p' + p, pinned: t === 0});
  }
  return {threads, projects: ps};
}

test('sidebar words are the hub\'s: a shared slug takes the thread\'s seq', async () => {
  const {sidebarRefs, indexRows} = await load();
  const idx = sidebarRefs({threads: [
    {id: 't1-11', title: 'Fix sidebar', project: 'p1'}, {id: 't1-12', title: 'fix  sidebar!', project: 'p2'},
    {id: 't1-13', title: 'Pairing', project: 'p1'}, {id: 't1-14', title: '', project: 'p1'}], projects: [{id: 'p1', title: 'Main board'}, {id: 'p2', title: 'Case'}]}, 't1-13');
  assert.deepEqual(idx.threads.map(t => t.value), ['>fix-sidebar-11', '>fix-sidebar-12', '>t1-14']);
  assert.equal(idx.threads[0].desc, 'Fix sidebar', 'same project: the title alone');
  assert.equal(idx.threads[1].desc, 'Case: fix  sidebar!');
  assert.deepEqual(indexRows(idx, '%').map(r => r.value), ['%main-board', '%case']);
  assert.deepEqual(indexRows(idx, '>sid').map(r => r.value), ['>fix-sidebar-11', '>fix-sidebar-12'], 'its own project first');
  assert.equal(indexRows(idx, '/x'), null);
});

test('1,000 threads: the index and a popup\'s rows are ready inside a frame', async () => {
  const {sidebarRefs, indexRows, PopupRows} = await load();
  const list = store(40, 25);
  const rows = new PopupRows();
  const t0 = performance.now();
  const idx = sidebarRefs(list, list.threads[500].id);
  const t1 = performance.now();
  let shown = 0;
  for (const w of ['>', '>s', '>si', '>sid', '%', '%pr', '>mea', '>boot']) shown += indexRows(idx, w).length;
  const t2 = performance.now();
  rows.put('x', '>', indexRows(idx, '>'));
  for (const w of ['>s', '>si', '>sid', '>side']) rows.get('x', w);
  const t3 = performance.now();
  assert.ok(shown > 0);
  assert.equal(idx.threads.length, 999);
  // per keystroke (8 words): well inside a phone's frame even at 4x slower
  assert.ok((t2 - t1) / 8 < 12, `index rows ${((t2 - t1) / 8).toFixed(2)} ms per word`);
  assert.ok((t3 - t2) / 4 < 2, `kept rows ${((t3 - t2) / 4).toFixed(2)} ms per word`);
  // once per view
  assert.ok(t1 - t0 < 50, `index ${(t1 - t0).toFixed(2)} ms`);
});

test('Tab or Enter waits for the hub only on guessed rows the person did not move through', async () => {
  const {pickWaits} = await load();
  assert.equal(pickWaits({guessed: true, moved: false}), true);
  assert.equal(pickWaits({guessed: true, moved: true}), false);
  assert.equal(pickWaits({guessed: false, moved: false}), false);
  assert.equal(pickWaits(null), false);
});

test('only reference words ask the sidebar', async () => {
  const {sidebarWord} = await load();
  for (const w of ['>', '>sid', '%', '%proj', '@', '#']) assert.equal(sidebarWord(w), true);
  for (const w of ['/', '/btw', '$']) assert.equal(sidebarWord(w), false);
});

test('bots and rooms come from the sidebar under the hub words', async () => {
  const {sidebarRefs, indexRows, sidebarWord} = await load();
  const index = sidebarRefs({threads: [], projects: [], bots: [{id: 'b1', name: 'enclosure-bot'}, {id: 'link~b2', name: 'far'}], rooms: [{id: 'r1', name: 'Usb Hub room'}]}, '');
  assert.deepEqual(indexRows(index, '@').map(r => r.value), ['@enclosure-bot']);
  assert.deepEqual(indexRows(index, '#').map(r => r.value), ['#usb-hub-room']);
  assert.deepEqual(indexRows(index, '#xyz'), []);
  assert.equal(sidebarWord('@'), true);
  assert.equal(sidebarWord('#'), true);
});

test('skills and commands follow client.bend Skills.rows and Slash.of', async () => {
  const {skillRows, slashRows} = await load();
  const skills = [{name: 'review', description: 'r'}, {name: 'bom', description: 'b'}, {name: 'build'}];
  assert.deepEqual(skillRows(skills, '$b').map(r => r.value), ['$bom', '$build']);
  assert.deepEqual(skillRows(skills, '$').length, 3);
  const cmds = [{value: '/btw', desc: 'x'}];
  assert.deepEqual(slashRows(cmds, skills, '/b').map(r => r.value), ['/btw', '$bom', '$build']);
  assert.equal(slashRows(cmds, skills, 'say /b'), null);
  assert.equal(slashRows(Array.from({length: 7}, (_, i) => ({value: '/c' + i})), skills, '/').length, 8);
});
