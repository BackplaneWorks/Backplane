'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const { serviceDiagnostics, KEEP } = require('../service-diagnostics.cjs');

async function fixture(t) {
  const home = await fs.mkdtemp(path.join(os.tmpdir(), 'bp-diagnostics-'));
  t.after(() => fs.rm(home, { recursive: true, force: true }));
  const binary = path.join(home, 'service');
  await fs.writeFile(binary, 'synthetic executable');
  const journal = await serviceDiagnostics({ home, binary, version: '1.2.3' });
  assert(journal);
  const exception = journal.env.BACKPLANE_DIAGNOSTICS_FILE;
  return { home, binary, journal, exception, file: exception.replace('.exception.json', '.json') };
}

const read = async file => JSON.parse(await fs.readFile(file, 'utf8'));

function runtime({ file, argv = [], processPatch = {}, requirePatch } = {}) {
  const source = require('node:fs').readFileSync(path.join(__dirname, '../../src/server/effects/loop.js'), 'utf8');
  const first = source.indexOf('function bp_failure('), end = source.indexOf('\nif (BP_ASYNC)', first);
  assert(first >= 0 && end > first);
  const context = vm.createContext({ Error, RangeError, TypeError, SyntaxError, ReferenceError, URIError, EvalError,
    Date, Buffer, process: { ...process, argv: ['bun', '/$bunfs/root/backplane-serve', ...argv],
      env: file ? { BACKPLANE_DIAGNOSTICS_FILE: file } : {}, ...processPatch },
    require: requirePatch || require,
  });
  vm.runInContext(source.slice(first, end), context);
  return context;
}

test('launch, authenticated version and exit metadata survive a new reader; files are private', async t => {
  const f = await fixture(t);
  await f.journal.started(123);
  await f.journal.ready('1.2.4');
  await f.journal.exited(23, null);
  const record = await read(f.file);
  assert.equal(record.pid, 123);
  assert.equal(record.version, '1.2.4');
  assert.equal(record.binarySha256, crypto.createHash('sha256').update('synthetic executable').digest('hex'));
  assert.equal(record.state, 'exited');
  assert.equal(record.exitCode, 23);
  assert.equal(record.signal, null);
  assert(Date.parse(record.startedAt) <= Date.parse(record.endedAt));
  if (process.platform !== 'win32') {
    assert.equal((await fs.stat(path.dirname(f.file))).mode & 0o777, 0o700);
    assert.equal((await fs.stat(f.file)).mode & 0o777, 0o600);
  }
  assert.equal((await fs.readdir(path.dirname(f.file))).length, 1);
});

test('late readiness cannot erase a signal exit or invent an exit code', async t => {
  const f = await fixture(t);
  await Promise.all([f.journal.started(123), f.journal.exited(null, 'SIGTERM'), f.journal.ready('1.2.3')]);
  const r = await read(f.file);
  assert.equal(r.state, 'exited');assert.equal(r.signal, 'SIGTERM');assert.equal(r.exitCode, null);
  if (os.constants.signals.SIGXCPU) {
    await f.journal.exited(null, 'SIGXCPU');
    assert.equal((await read(f.file)).signal, 'SIGXCPU', 'retain resource-limit signals too');
  }
});

test('launch errors retain the OS code without arbitrary text', async t => {
  const f = await fixture(t);
  await f.journal.failed('ENOENT');
  assert.equal((await read(f.file)).errorCode, 'ENOENT');
  await f.journal.failed('PRIVATE_PROMPT');
  await f.journal.ready('PRIVATE credential?token=SECRET');
  await f.journal.exited('PRIVATE', 'PRIVATE');
  const text = await fs.readFile(f.file, 'utf8');
  assert(!text.includes('PRIVATE'));assert(!text.includes('SECRET'));
});

test('retention bounds both files of each launch and leaves unrelated files alone', async t => {
  const f = await fixture(t), dir = path.dirname(f.file);
  await fs.writeFile(path.join(dir, 'notes.txt'), 'keep me');
  for (let i = 0; i < KEEP + 5; i++) {
    const j = await serviceDiagnostics({ home: f.home, binary: f.binary, version: '1.2.3' });
    await j.started(123);
    runtime({ file: j.env.BACKPLANE_DIAGNOSTICS_FILE }).bp_failure(new RangeError('PRIVATE'));
  }
  const names = await fs.readdir(dir);
  assert.equal(names.filter(n => n.endsWith('.json')).length, 2 * KEEP);
  assert.equal(await fs.readFile(path.join(dir, 'notes.txt'), 'utf8'), 'keep me');
});

test('failure report locates the exception without retaining its message, fake frames or directories', async t => {
  const f = await fixture(t), ctx = runtime({ file: f.exception });
  function diagnosticsOrigin() { return new RangeError('PRIVATE_PROMPT\n    at forged (/credential/SECRET:1:2)'); }
  const old = Object.getOwnPropertyDescriptor(Error, 'prepareStackTrace');
  ctx.bp_failure(diagnosticsOrigin());
  assert.deepEqual(Object.getOwnPropertyDescriptor(Error, 'prepareStackTrace'), old);
  const r = await read(f.exception), text = await fs.readFile(f.exception, 'utf8');
  assert.equal(r.exception, 'RangeError');assert.equal(r.pid, process.pid);assert(r.stackAvailable);
  // JSC can inline the helper; source coordinates must still identify it.
  const originLines = require('node:fs').readFileSync(__filename, 'utf8').split('\n')
    .flatMap((line, i) => line.includes('function diagnosticsOrigin()') || line.trim() === 'ctx.bp_failure(diagnosticsOrigin());' ? [i + 1] : []);
  assert(r.frames.some(x => x.source === 'service-diagnostics.test.cjs' && originLines.includes(x.line)), JSON.stringify(r.frames));
  for (const word of ['PRIVATE_PROMPT', 'forged', 'credential', 'SECRET', os.homedir()]) assert(!text.includes(word), word);
  assert((await fs.stat(f.exception)).size < 32768);
  if (process.platform !== 'win32') assert.equal((await fs.stat(f.exception)).mode & 0o777, 0o600);
});

test('unknown thrown values are not inspected or converted to strings', async t => {
  const f = await fixture(t);
  const error = { toString() { throw Error('must not run'); }, get stack() { throw Error('must not read'); } };
  runtime({ file: f.exception }).bp_failure(error);
  const r = await read(f.exception);assert.equal(r.exception, 'Unknown');assert.deepEqual(r.frames, []);assert(!r.stackAvailable);
});

test('a real stack overflow leaves usable frames after unwinding', async t => {
  const f = await fixture(t);
  function diagnosticsOverflow() { return 1 + diagnosticsOverflow(); }
  let error;
  try { diagnosticsOverflow(); } catch (caught) { error = caught; }
  assert(error instanceof RangeError);
  runtime({ file: f.exception }).bp_failure(error);
  const report = await read(f.exception);
  assert.equal(report.exception, 'RangeError');assert(report.stackAvailable);
  assert(report.frames.some(frame => frame.function === 'diagnosticsOverflow'));
  assert((await fs.stat(f.exception)).size < 32768);
});

test('previous stack rendering never copies arbitrary text into the report', async t => {
  const f = await fixture(t), error = new TypeError('PRIVATE');
  void error.stack;
  runtime({ file: f.exception }).bp_failure(error);
  const r = await read(f.exception);assert.equal(r.exception, 'TypeError');
  // Bun can regenerate structured sites; Node may retain only the rendered text.
  if (!r.stackAvailable) assert.deepEqual(r.frames, []);
  else assert(r.frames.length <= 32);
  assert(!(await fs.readFile(f.exception, 'utf8')).includes('PRIVATE'));
});

test('children and unconfigured programs never write a hub exception report', async t => {
  const f = await fixture(t);
  const cli = await fs.readFile(path.join(__dirname, '../../src/core/cli.bend'), 'utf8');
  const child = cli.match(/def Cli\.child\([^]*?(?=\ndef )/)[0];
  const flags = [...child.matchAll(/case "(--[\w-]+)"/g)].map(match => match[1]);
  assert(flags.length >= 7, 'exercise every child accepted by the real CLI');
  for (const flag of flags) {
    runtime({ file: f.exception, argv: [flag, 'input'] }).bp_failure(new Error('PRIVATE'));
    await assert.rejects(fs.stat(f.exception), { code: 'ENOENT' });
  }
  runtime({ requirePatch() { throw Error('filesystem must not be touched'); } }).bp_failure(new Error('PRIVATE'));
});

test('diagnostics cannot follow a linked directory or overwrite a linked report target', async t => {
  const f = await fixture(t), outside = path.join(f.home, 'outside');
  await fs.mkdir(outside);
  const linkedHome = path.join(f.home, 'linked-home');await fs.mkdir(linkedHome);
  await fs.symlink(outside, path.join(linkedHome, 'diagnostics'), 'dir');
  assert.equal(await serviceDiagnostics({ home: linkedHome, binary: f.binary }), null);
  runtime({ file: path.join(linkedHome, 'diagnostics/exception.json') }).bp_failure(new Error('PRIVATE'));
  assert.deepEqual(await fs.readdir(outside), []);
  const target = path.join(outside, 'keep');await fs.writeFile(target, 'unchanged');
  await fs.symlink(target, f.exception);
  runtime({ file: f.exception }).bp_failure(new Error('PRIVATE'));
  assert.equal(await fs.readFile(target, 'utf8'), 'unchanged');
  assert(!(await fs.lstat(f.exception)).isSymbolicLink());
});

test('every preparation failure is nonfatal, and failed writes leave no temporary files', async t => {
  const f = await fixture(t);
  for (const operation of ['mkdir', 'lstat', 'chmod', 'readdir']) {
    const files = new Proxy(fs, { get(target, key) {
      return key === operation ? async () => { throw Object.assign(Error('PRIVATE'), { code: 'EACCES' }); } : target[key];
    } });
    assert.equal(await serviceDiagnostics({ home: f.home, binary: f.binary, files }), null, operation);
  }
  for (const operation of ['writeFile', 'rename']) {
    const files = new Proxy(fs, { get(target, key) {
      return key === operation ? async () => { throw Error('PRIVATE'); } : target[key];
    } });
    const j = await serviceDiagnostics({ home: f.home, binary: f.binary, files });
    await j.started(123);await j.exited(1, null);
    assert(!(await fs.readdir(path.join(f.home, 'diagnostics'))).some(x => x.endsWith('.tmp')));
  }
});

test('fatal-handler exit code and stderr are unchanged even when report writing fails', async t => {
  const f = await fixture(t), output = [], exits = [];
  const ctx = runtime({ file: f.exception, processPatch: { exit: code => exits.push(code) },
    requirePatch(name) { if (name === 'fs') throw Error('PRIVATE');return require(name); } });
  ctx.bp_run = async () => { throw new RangeError('original'); };
  ctx.io_errs = text => output.push(text);
  ctx.bp_start(() => {});
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(exits, [1]);assert.deepEqual(output, ['bend: memory fault (machine stack overflow?)']);
  await assert.rejects(fs.stat(f.exception), { code: 'ENOENT' });
});

test('exception classes, large messages and injected fields cannot expand or contaminate a report', async t => {
  const f = await fixture(t), ctx = runtime({ file: f.exception });
  for (const Kind of [Error, RangeError, TypeError, SyntaxError, ReferenceError, URIError, EvalError]) {
    const error = new Kind(('PRIVATE_PROMPT 日本語 🔑 CAD_CONTENT credential=SECRET\n').repeat(25000));
    error.name = 'PRIVATE_PROMPT';error.cause = { secret: 'SECRET' };
    error.argv = ['CAD_CONTENT'];error.headers = { Authorization: 'SECRET' };
    ctx.bp_failure(error);
    const text = await fs.readFile(f.exception, 'utf8'), r = JSON.parse(text);
    assert.equal(r.exception, Kind.name);assert(r.stackAvailable);assert(r.frames.length <= 32);
    assert(Buffer.byteLength(text) < 32768);
    for (const word of ['PRIVATE_PROMPT', 'SECRET', 'CAD_CONTENT', 'headers', 'cause', 'argv']) assert(!text.includes(word));
  }
});

test('missing binary and unwritable diagnostics do not prevent the normal spawn path', async t => {
  const f = await fixture(t);
  const j = await serviceDiagnostics({ home: f.home, binary: path.join(f.home, 'missing'), version: '1.2.3' });
  assert(j);await j.failed('ENOENT');
  const rows = await fs.readdir(path.dirname(f.file));
  const report = await read(path.join(path.dirname(f.file), rows.find(n => n !== path.basename(f.file))));
  assert.equal(report.errorCode, 'ENOENT');assert(!('binarySha256' in report));
  const damaged = path.join(f.home, 'blocked');await fs.mkdir(damaged);
  await fs.writeFile(path.join(damaged, 'diagnostics'), 'directory obstruction');
  assert.equal(await serviceDiagnostics({ home: damaged, binary: f.binary }), null);
});

test('a pre-existing stack formatter is restored without recording its output', async t => {
  const f = await fixture(t), original = Object.getOwnPropertyDescriptor(Error, 'prepareStackTrace');
  const formatter = () => 'PRIVATE_FORMATTER_TEXT';
  try {
    Error.prepareStackTrace = formatter;
    runtime({ file: f.exception }).bp_failure(new Error('PRIVATE'));
    assert.equal(Error.prepareStackTrace, formatter);
    assert(!(await fs.readFile(f.exception, 'utf8')).includes('PRIVATE'));
  } finally {
    if (original) Object.defineProperty(Error, 'prepareStackTrace', original);
    else delete Error.prepareStackTrace;
  }
});

test('desktop packaging includes the diagnostics module and refuses an incomplete shell', async t => {
  const f = await fixture(t), desktop = path.resolve(__dirname, '..');
  const { appFiles } = require('../prepare-components.cjs');
  const files = appFiles(desktop);
  assert(files.includes('service-diagnostics.cjs'));
  const shell = path.join(f.home, 'shell');await fs.mkdir(shell);
  for (const file of files) {
    await fs.mkdir(path.dirname(path.join(shell, file)), { recursive: true });
    await fs.copyFile(path.join(desktop, file), path.join(shell, file));
  }
  assert.deepEqual(appFiles(shell), files);
  await fs.unlink(path.join(shell, 'service-diagnostics.cjs'));
  assert.throws(() => appFiles(shell), /missing file: desktop\/service-diagnostics\.cjs/);
});
