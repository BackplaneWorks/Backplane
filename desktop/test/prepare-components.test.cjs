'use strict';
// prepare-components.cjs: the shell's files are what the app references, and
// what its receipt claims holds in the sources it is packed from.
const {test} = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const c = require('../prepare-components.cjs');
const desktop = path.join(__dirname, '..');

test('the shell is every file main.cjs and its preloads reach', () => {
  const files = c.appFiles(desktop);
  for (const f of ['package.json', 'main.cjs', 'preload.cjs', 'service.cjs', 'startup.html', 'startup.css', 'startup.js',
    'browser-guest.cjs', 'browser-recording-preload.cjs', 'browser-shield.html'])
    assert.ok(files.includes(f), f);
  assert.ok(!files.some(f => f.includes('test') || f.startsWith('produce-') || f === 'stage-linux.cjs'), 'no release tooling in the shell');
});

test('the receipt\'s features are checked against main.cjs and service.cjs', () => {
  const f = c.features(desktop);
  assert.equal(f.contextIsolation, true); assert.equal(f.sandbox, true); assert.equal(f.nodeIntegration, false);
  assert.equal(f.managedStop, false); assert.equal(f.windowsManagedControl, 'UNSUPPORTED');
});

test('a reference to a missing file stops it', () => {
  const d = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-prep-'));
  fs.writeFileSync(path.join(d, 'package.json'), JSON.stringify({name: 'backplane-desktop', main: 'main.cjs'}));
  fs.writeFileSync(path.join(d, 'main.cjs'), "require('./gone.cjs')");
  assert.throws(() => c.appFiles(d), /missing file/);
  fs.writeFileSync(path.join(d, 'main.cjs'), "require('./../x.cjs')");
  assert.throws(() => c.appFiles(d), /leaves desktop/);
});
