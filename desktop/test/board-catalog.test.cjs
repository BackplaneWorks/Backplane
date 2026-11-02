const { test } = require('node:test');
const fs = require('node:fs'), vm = require('node:vm');
const assert = require('node:assert/strict');

async function fixture() {
  const { installBoardCatalog } = await import('../../src/web/board-catalog.js');
  const handlers = {}, sent = [];
  let selected = 'thread';
  const menu = { open: false, dataset: { boardCatalog: 'source-hash', scope: 'thread', renderOpen: '0' },
    isConnected: true, matches: s => s === '[data-board-catalog]', querySelector: () => input, querySelectorAll: () => rows };
  const input = { value: '', dataset: { renderQuery: '' }, matches: s => s === '[data-board-query]', closest: () => menu, focus() {} };
  let chosen = '';
  const rows = ['other/selected.kicad_pcb', 'matching/board.kicad_pcb'].map(path => ({ dataset: { boardPath: path }, click() { chosen = path; }, focus() {} }));
  const controller = installBoardCatalog({ addEventListener(type, fn) { handlers[type] = fn; } }, () => selected, (action, value) => sent.push([action, JSON.parse(value)]));
  const key = () => handlers.keydown({ target: input, key: 'Enter', preventDefault() {}, stopImmediatePropagation() {} });
  return { handlers, sent, menu, input, rows, key, controller, chosen: () => chosen, select: s => selected = s };
}

test('initial/rendered closed disclosure sends nothing; explicit open/close is scoped', async () => {
  const f = await fixture();
  f.handlers.toggle({ target: f.menu });assert.equal(f.sent.length, 0);
  f.menu.open = true;f.handlers.toggle({ target: f.menu });
  f.menu.dataset.renderOpen = '1';f.handlers.toggle({ target: f.menu });
  assert.equal(f.sent.length, 1);
  f.menu.open = false;f.handlers.toggle({ target: f.menu });
  assert.deepEqual(f.sent.map(x => x[1].open), [true, false]);
  f.select('other');f.menu.open = true;f.handlers.toggle({ target: f.menu });
  assert.equal(f.sent.length, 2, 'detached old target cannot reopen another catalog');
});

test('rapid open then outside close records close even before open reply', async () => {
  const f = await fixture();
  f.menu.open = true;f.handlers.toggle({ target: f.menu });
  f.menu.open = false;f.handlers.toggle({ target: f.menu });
  assert.deepEqual(f.sent.map(x => x[1].open), [true, false]);
});

test('metadata search resets page and IME sends only committed query', async () => {
  const f = await fixture();f.menu.open = true;
  f.handlers.compositionstart({ target: f.input });f.input.value = '基板';
  f.handlers.input({ target: f.input, isComposing: true });assert.equal(f.sent.length, 0);
  f.handlers.compositionend({ target: f.input });
  assert.deepEqual(f.sent[0][1], { source: 'source-hash', open: true, query: '基板', offset: 0 });
  f.select('other');f.input.value = 'stale';f.handlers.input({ target: f.input });
  assert.equal(f.sent.length, 1);
});

test('Enter waits for current server search and cannot select pinned nonmatch', async () => {
  const f = await fixture();f.menu.open = true;
  f.input.value = 'matching';f.key();assert.equal(f.chosen(), '');
  f.input.dataset.renderQuery = 'matching';f.key();assert.equal(f.chosen(), 'matching/board.kicad_pcb');
});


test('delayed renderer open ACK cannot reverse an explicit outside close or emit another open', async () => {
  const f = await fixture();
  f.menu.open = true;f.handlers.toggle({ target: f.menu });
  f.menu.open = false;f.handlers.toggle({ target: f.menu });
  const src = fs.readFileSync(require('node:path').join(__dirname, '../../src/web/host.js'), 'utf8');
  const start = src.indexOf('function setAttrs('), end = src.indexOf('\n}', start) + 2;
  const attrs = new Map();
  f.menu.getAttribute = key => attrs.get(key) ?? null;
  f.menu.setAttribute = (key,value) => { attrs.set(key,value);if(key === 'open')f.menu.open = true; };
  f.menu.removeAttribute = key => { attrs.delete(key);if(key === 'open')f.menu.open = false; };
  const patch = vm.runInNewContext('(' + src.slice(start,end) + ')', { each: a => a, is: (a,k) => a.$ === k, boardCatalog: f.controller });
  const at = (name,value) => ({ $:'At', name, value });
  const closed = [at('data-board-catalog','source-hash'),at('data-scope','thread'),at('data-render-open','0')];
  const opened = [at('data-board-catalog','source-hash'),at('data-scope','thread'),at('data-render-open','1'),at('open','')];
  patch(f.menu,closed,opened);f.handlers.toggle({ target:f.menu });
  assert.equal(f.menu.open,false,'production keyed attribute patch respects the newest local close');
  assert.deepEqual(f.sent.map(x => x[1].open),[true,false],'render-induced toggle issues no new user command');
  patch(f.menu,opened,closed);f.handlers.toggle({ target:f.menu });
  f.menu.open = true;f.handlers.toggle({ target:f.menu });
  assert.deepEqual(f.sent.map(x => x[1].open),[true,false,true],'a later explicit opener still works');
});

test('stale search pager cannot restore query A after query B is typed', async () => {
  const f = await fixture();f.menu.open = true;f.input.value = 'B';f.input.dataset.renderQuery = 'A';
  const button = { closest: () => f.menu };
  let blocked = 0, stopped = 0;
  const click = () => f.handlers.click({ target:{closest:()=>button}, preventDefault(){blocked++;}, stopImmediatePropagation(){stopped++;} });
  click();assert.equal(blocked,1);assert.equal(stopped,1);
  f.input.dataset.renderQuery='B';click();assert.equal(blocked,1,'acknowledged query makes its current pager usable');
});
