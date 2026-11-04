'use strict';
// Run on Boat or a disposable native Windows runner, never a user's machine.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const source = fs.readFileSync(path.join(__dirname, '../../src/server/effects/host.js'), 'utf8')
  .replace(/CID\(([^)]+)\)/g, '"$1"');
// POSIX argv would hide a caller-only reversion to the original shell writer.
// Keep this assertion in the small gate as well as exercising the native effect.
const saveDefinition = fs.readFileSync(path.join(__dirname, '../../src/server/kicad.bend'), 'utf8')
  .split(/^def /m).find(definition => definition.startsWith('Rt.save('));
assert(saveDefinition, 'Production Rt.save definition is missing');
assert.match(saveDefinition, /\bH\.FS\.write_atomic\(/, 'Rt.save must use the native atomic writer');
assert.doesNotMatch(saveDefinition, /\b(?:P\.)?Proc\.run\(/, 'Rt.save must never transport JSON through child argv');
function writer(io = fs) {
  const effects = new Map();
  new Function('require', 'io_eff', 'io_done', 'io_fail', source)(
    name => name === 'fs' ? io : require(name), (id, fn) => effects.set(id, fn),
    value => ({ ok: true, value }), errno => ({ ok: false, errno }));
  return effects.get('FS.write_atomic');
}
const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'bp atomic 日本 '));
try {
  const target = path.join(folder, 'kicad-health.json');
  const values = ['C:\\backplane_kicad\\', 'C:\\bp927\\new tools\\test\\',
    'D:\\Program Files\\KiCad\\bin\\kicad-cli.exe', '\\\\server\\share\\new\\test\\',
    'quotes " and \\ and actual controls \b\n\t and 日本'];
  const write = writer();
  for (const setting of values) {
    const text = JSON.stringify({ setting, stamp: values.join('\n') });
    assert.equal(write(target, text).ok, true);
    assert.equal(fs.readFileSync(target, 'utf8'), text);
    assert.equal(JSON.parse(fs.readFileSync(target, 'utf8')).setting, setting);
    assert.deepEqual(fs.readdirSync(folder), ['kicad-health.json']);
    if (process.platform !== 'win32') assert.equal(fs.statSync(target).mode & 0o777, 0o600);
  }
  const previous = fs.readFileSync(target, 'utf8');
  for (const failure of ['writeFileSync', 'renameSync']) {
    const broken = Object.create(fs);
    broken[failure] = (...args) => {
      if (failure === 'writeFileSync') fs.writeSync(args[0], 'partial');
      throw Object.assign(Error('injected disk failure'), { errno: 5 });
    };
    assert.equal(writer(broken)(target, 'incomplete').ok, false);
    assert.equal(fs.readFileSync(target, 'utf8'), previous);
    assert.deepEqual(fs.readdirSync(folder), ['kicad-health.json']);
  }
  assert.equal(write(path.join(folder, 'absent', 'cache.json'), 'x').ok, false);
  assert.equal(fs.readFileSync(target, 'utf8'), previous);
  console.log('ok native atomic UTF-8 text: caller has no shell transport; Windows escapes, replacement, private mode, failure preservation and cleanup');
} finally { fs.rmSync(folder, { recursive: true, force: true }); }
