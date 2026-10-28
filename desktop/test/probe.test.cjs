'use strict';
// Cross-probing's decisions (src/web/probe.js): what a click picks and
// probes, what a part or net is made of on the board and on a schematic
// sheet, and what lights up. node --test desktop/test/probe.test.cjs
const { test } = require('node:test'), assert = require('node:assert/strict'), path = require('node:path'), url = require('node:url');
const load = () => import(url.pathToFileURL(path.join(__dirname, '../../src/web/probe.js')).href);

// a board: two resistors on GND and VBUS, a track and a zone
const board = new Map([
  [11, ['pad 1;net=GND;type=smd;shape=rect;ref=R1;value=10k;footprint=R_0402', 'pad 2;net=/Power/VBUS;type=smd;shape=rect;ref=R1;value=10k;footprint=R_0402']],
  [12, ['pad 1;net=GND;type=smd;shape=rect;ref=C3;value=1u;footprint=C_0402', 'pad 2;net=Net-(C3-Pad2);type=smd;shape=rect;ref=C3;value=1u;footprint=C_0402']],
  [13, ['track;net=GND;layer=F.Cu;width=0.25', 'zone;net=GND;layer=B.Cu']],
  [14, ['line;layer=F.SilkS;width=0.12;ref=R1;value=10k;footprint=R_0402', 'text;text=R1;ref=R1;value=10k;footprint=R_0402', 'gr_line;layer=Edge.Cuts;width=0.1']],
]);

// a sheet at /Power/: R1's pins in groups 5 (with the GND power symbol) and
// 6 (the local label VBUS), U1A's pin 3 alone in group 7
const sheet = new Map([
  [21, ['wire;g=5', 'wire;g=6', 'junction;g=5']],
  [22, ['pin 1;name=~;type=passive;symbol=R1;g=5', 'pin 2;name=~;type=passive;symbol=R1;g=6', 'pin 3;name=IN;type=input;symbol=U1A;g=7']],
  [23, ['symbol;ref=#PWR01;value=GND;lib=power:GND;footprint=;g=5', 'symbol;ref=R1;value=10k;lib=Device:R;footprint=R_0402', 'symbol;ref=U1A;value=LM358;lib=Amp:LM358;footprint=SOIC-8']],
  [24, ['label;text=VBUS;g=6', 'global label;text=SPI_CLK;shape=input;g=8', 'symbol;ref=#FLG01;value=PWR_FLAG;lib=power:PWR_FLAG;footprint=;g=5']],
]);

test('infos read: kind, its argument, keys', async () => {
  const P = await load();
  const i = P.parseInfo('pad 2;net=/A=B;ref=R1');
  assert.equal(i.kind, 'pad'); assert.equal(i.arg, '2'); assert.equal(i.kv.get('net'), '/A=B'); assert.equal(i.kv.get('ref'), 'R1');
  assert.equal(P.parseInfo('global label;text=X').kind, 'global');
  assert.equal(P.baseRef('U1A'), 'U1'); assert.equal(P.baseRef('R12'), 'R12'); assert.equal(P.baseRef('#PWR01'), '#PWR01');
});

test('what an item is: pads, copper, footprint drawing, pins, symbols, power, labels', async () => {
  const P = await load();
  assert.deepEqual([P.what('pad 2;net=GND;ref=R1').net, P.what('pad 2;net=GND;ref=R1').pin, P.what('pad 2;net=GND;ref=R1').ref], ['GND', 'R1.2', 'R1']);
  assert.equal(P.what('track;net=GND;layer=F.Cu').net, 'GND');
  assert.equal(P.what('line;layer=F.SilkS;ref=R1').ref, 'R1');
  assert.equal(P.what('gr_line;layer=Edge.Cuts').ref, '');
  assert.equal(P.what('pin 3;symbol=U1A;g=7').pin, 'U1.3');
  assert.equal(P.what('pin 1;name=GND;symbol=#PWR01;g=5').pin, '');
  assert.equal(P.what('symbol;ref=U1B;value=LM358').ref, 'U1');
  assert.deepEqual([P.what('symbol;ref=#PWR01;value=GND;g=5').ref, P.what('symbol;ref=#PWR01;value=GND;g=5').name], ['', 'GND']);
  assert.equal(P.what('symbol;ref=#FLG01;value=PWR_FLAG').name, '');
  assert.equal(P.what('hier label;text=SDA;g=2').name, 'SDA');
  assert.equal(P.what('sheet pin;name=SDA;sheet=Power;g=2').name, 'SDA');
});

test('a click picks: copper over a footprint line over a zone; nothing on the outline', async () => {
  const P = await load();
  const c = (text, area, layer = 0) => ({ text, area, layer });
  assert.equal(P.best([c('zone;net=GND', 900, 12), c('pad 1;net=GND;ref=R1', 4, 12)]).text, 'pad 1;net=GND;ref=R1');
  assert.equal(P.best([c('pad 1;net=GND;ref=R1', 4, 12), c('zone;net=GND', 900, 12)]).text, 'pad 1;net=GND;ref=R1');
  assert.equal(P.best([c('zone;net=GND', 900, 12), c('line;ref=R1', 30, 3)]).text, 'line;ref=R1');
  assert.equal(P.best([c('pad 1;net=A;ref=R1', 4, 12), c('pad 1;net=B;ref=R2', 4, 5)]).text, 'pad 1;net=A;ref=R1');
  assert.equal(P.best([c('gr_line;layer=Edge.Cuts', 10, 22)]), null);
  assert.equal(P.best([]), null);
  // a pin before the symbol body it sits on
  assert.equal(P.best([c('symbol;ref=R1', 50, 7), c('pin 1;symbol=R1;g=5', 2, 8)]).text, 'pin 1;symbol=R1;g=5');
  // a footprint's box beats a zone and nothing, never copper
  assert.equal(P.what(P.partOrBest(P.best([c('zone;net=GND', 900, 12)]), 'U3').info).ref, 'U3');
  assert.equal(P.what(P.partOrBest(null, 'U3').info).ref, 'U3');
  assert.equal(P.what(P.partOrBest(P.best([c('pad 1;net=GND;ref=R1', 4, 12)]), 'U3').info).ref, 'R1');
});

test('a click on the board: copper or a part\'s line right under it, else the part whose box holds it, else copper near, else the zone', async () => {
  const P = await load();
  const c = (text, area, layer = 0) => ({ text, area, layer });
  const zone = c('zone;net=GND', 900, 12), track = c('track;net=CLK', 30, 12), silk = c('line;ref=R1', 20, 16);
  assert.equal(P.what(P.pickBoard([zone], [zone, track], 'R1').info).ref, 'R1');
  assert.equal(P.what(P.pickBoard([zone, track], [zone, track], 'R1').info).net, 'CLK');
  assert.equal(P.what(P.pickBoard([silk], [silk], 'R1').info).ref, 'R1');
  assert.equal(P.what(P.pickBoard([zone], [zone, track], '').info).net, 'CLK');
  assert.equal(P.what(P.pickBoard([zone], [zone], '').info).net, 'GND');
  assert.equal(P.pickBoard([], [], ''), null);
  const body = c('symbol;ref=R1', 50, 7), pin = c('pin 1;symbol=R1;g=5', 2, 8);
  assert.equal(P.pickSheet([body], [body, pin]).text, 'symbol;ref=R1');
  assert.equal(P.pickSheet([body, pin], [body, pin]).text, 'pin 1;symbol=R1;g=5');
  assert.equal(P.pickSheet([], [pin]).text, 'pin 1;symbol=R1;g=5');
});

test('an info file reads into each chunk\'s table, by its sig', async () => {
  const P = await load();
  const t = '11 10 a;n=1\u0000b;é\u0000\n4294967295 0 \n7 6 x;k=😀\u0000\n';
  const m = P.parseInfos(t);
  assert.deepEqual(m.get(11), ['a;n=1', 'b;é']);
  assert.deepEqual(m.get(4294967295), []);
  assert.deepEqual(m.get(7), ['x;k=😀']);
  assert.equal(P.parseInfos('').size, 0);
});

test('the board\'s nets: each pad\'s, each net\'s pins; a probe finds its net by name or by a pin', async () => {
  const P = await load();
  const n = P.boardNets(board);
  assert.equal(n.pin.get('R1.2'), '/Power/VBUS');
  assert.deepEqual([...n.pins.get('GND')].sort(), ['C3.1', 'R1.1']);
  assert.equal(P.boardNet(P.parseProbe('n\tGND\t'), n), 'GND');
  assert.equal(P.boardNet(P.parseProbe('n\t/VBUS\tR1.2'), n), '/Power/VBUS');
  assert.equal(P.boardNet(P.parseProbe('n\t\tC3.2'), n), 'Net-(C3-Pad2)');
  assert.equal(P.boardNet(P.parseProbe('c\tR1'), n), '');
});

test('a sheet\'s groups light for a board net: by its pins, by a power or global name, by a label under the sheet\'s place', async () => {
  const P = await load();
  const n = P.boardNets(board), gs = P.sheetGroups(sheet);
  assert.deepEqual([...gs.get('5').pins], ['R1.1']); assert.deepEqual([...gs.get('5').global], ['GND']);
  assert.deepEqual([...gs.get('6').local], ['VBUS']);
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\tGND\t'), gs, n, '/Power/')], ['5']);
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\t/Power/VBUS\t'), gs, n, '/Power/')], ['6']);
  // with no board: the probe's own pins, and names
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\t\tU1.3'), gs, null, '/Power/')], ['7']);
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\tGND\t'), gs, null, '/')], ['5']);
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\t/Power/VBUS\t'), gs, null, '/Power/')], ['6']);
  assert.deepEqual([...P.groupsOf(P.parseProbe('n\tSPI_CLK\t'), gs, null, '/')], ['8']);
  assert.equal(P.groupsOf(P.parseProbe('c\tR1'), gs, n).size, 0);
});

test('a click probes: copper its net, a footprint its part, a pin or wire its group\'s net, a symbol its part', async () => {
  const P = await load();
  const n = P.boardNets(board), gs = P.sheetGroups(sheet);
  const pick = (text) => ({ info: P.parseInfo(text), area: 1, layer: 0 });
  assert.equal(P.clickProbe(pick('pad 2;net=/Power/VBUS;ref=R1'), 'board'), 'n\t/Power/VBUS\tR1.2');
  assert.equal(P.clickProbe(pick('track;net=GND;layer=F.Cu'), 'board'), 'n\tGND\t');
  assert.equal(P.clickProbe(pick('pad 3;type=np_thru_hole;ref=H1'), 'board'), 'c\tH1');
  assert.equal(P.clickProbe(pick('text;text=R1;ref=R1'), 'board'), 'c\tR1');
  assert.equal(P.clickProbe(null, 'board'), '');
  const ctx = { groups: gs, nets: n, place: '/Power/' };
  assert.equal(P.clickProbe(pick('wire;g=6'), 'schematic', ctx), 'n\t/Power/VBUS\tR1.2');
  assert.equal(P.clickProbe(pick('pin 1;symbol=R1;g=5'), 'schematic', ctx), 'n\tGND\tR1.1');
  assert.equal(P.clickProbe(pick('symbol;ref=#PWR01;value=GND;g=5'), 'schematic', ctx), 'n\tGND\tR1.1');
  assert.equal(P.clickProbe(pick('symbol;ref=U1A;value=LM358'), 'schematic', ctx), 'c\tU1');
  // no board yet: a label's name under the sheet's place, a global label's own
  assert.equal(P.clickProbe(pick('label;text=VBUS;g=6'), 'schematic', { groups: gs, nets: null, place: '/Power/' }), 'n\t/Power/VBUS\tR1.2');
  assert.equal(P.clickProbe(pick('global label;text=SPI_CLK;g=8'), 'schematic', { groups: gs, nets: null, place: '/' }), 'n\tSPI_CLK\t');
  assert.equal(P.clickProbe(pick('pin 3;symbol=U1A;g=7'), 'schematic', { groups: gs, nets: null, place: '/' }), 'n\t\tU1.3');
});

test('what lights up: a part\'s pieces on the board and its symbol and pins on a sheet; a net\'s copper and groups', async () => {
  const P = await load();
  const n = P.boardNets(board), gs = P.sheetGroups(sheet);
  const part = P.parseProbe('c\tR1'), net = P.parseProbe('n\tGND\tR1.1');
  const on = (f, m) => [...m.values()].flat().filter(f);
  assert.deepEqual(on(P.lit(part, 'board'), board), ['pad 1;net=GND;type=smd;shape=rect;ref=R1;value=10k;footprint=R_0402', 'pad 2;net=/Power/VBUS;type=smd;shape=rect;ref=R1;value=10k;footprint=R_0402', 'line;layer=F.SilkS;width=0.12;ref=R1;value=10k;footprint=R_0402', 'text;text=R1;ref=R1;value=10k;footprint=R_0402']);
  assert.deepEqual(on(P.lit(net, 'board', { nets: n }), board), ['pad 1;net=GND;type=smd;shape=rect;ref=R1;value=10k;footprint=R_0402', 'pad 1;net=GND;type=smd;shape=rect;ref=C3;value=1u;footprint=C_0402', 'track;net=GND;layer=F.Cu;width=0.25', 'zone;net=GND;layer=B.Cu']);
  assert.deepEqual(on(P.lit(P.parseProbe('c\tU1'), 'schematic'), sheet), ['pin 3;name=IN;type=input;symbol=U1A;g=7', 'symbol;ref=U1A;value=LM358;lib=Amp:LM358;footprint=SOIC-8']);
  assert.deepEqual(on(P.lit(net, 'schematic', { groups: gs, nets: n, place: '/Power/' }), sheet), ['wire;g=5', 'junction;g=5', 'pin 1;name=~;type=passive;symbol=R1;g=5', 'symbol;ref=#PWR01;value=GND;lib=power:GND;footprint=;g=5', 'symbol;ref=#FLG01;value=PWR_FLAG;lib=power:PWR_FLAG;footprint=;g=5']);
  assert.equal(on(P.lit(P.parseProbe(''), 'board'), board).length, 0);
});

test('probes as text, and the chip\'s words', async () => {
  const P = await load();
  assert.equal(P.netProbe('GND', ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i']), 'n\tGND\ta b c d e f g h');
  assert.equal(P.netProbe('', []), '');
  assert.equal(P.partProbe(''), '');
  assert.deepEqual(P.parseProbe('x'), { kind: '', ref: '', net: '', pins: [] });
  assert.equal(P.label(P.parseProbe('c\tR1')), 'R1');
  assert.equal(P.label(P.parseProbe('n\tGND\t')), 'Net GND');
  assert.equal(P.label(P.parseProbe('n\t\tU1.3')), 'Net U1.3');
  assert.equal(P.label(P.parseProbe('n\tNet-(Q1-G)\tQ1.1')), 'Net-(Q1-G)');
});
