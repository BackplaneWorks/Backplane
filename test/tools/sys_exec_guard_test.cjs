// Actual host.js effect under simulated platforms; no native exec or Windows job.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const test = require('node:test');
const loopSource = fs.readFileSync('src/server/effects/loop.js', 'utf8');
const source = fs.readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');

// Execute the real runtime table declarations and its publication statement.
// The rest of loop.js owns event-loop resources and is outside this unit test.
function errnoFor(platform, runtime = loopSource) {
  const start = runtime.indexOf('const BP_LINUX_ERRNO =');
  const end = runtime.indexOf('\nconst BP_TEXT =', start);
  assert(start >= 0 && end > start, 'real errno declarations exist');
  const publication = runtime.match(/^BP_H\.ERRNO = BP_ERRNO;$/m);
  assert(publication, 'real runtime publishes BP_H.ERRNO');
  const table = vm.runInNewContext(runtime.slice(start, end) + '\n' + publication[0] + '\nBP_H.ERRNO;', {
    BP_WIN: platform === 'win32', BP_H: {},
    require: name => { assert.equal(name, 'os'); return require(name); },
  });
  assert(Object.hasOwn(table, 'ENOSYS') && Number.isInteger(table.ENOSYS) && table.ENOSYS > 0,
    'real runtime table must define a numeric ENOSYS');
  return table;
}

test('the real runtime publishes a defined numeric ENOSYS', () => {
  assert.equal(errnoFor('win32').ENOSYS, 38);
});

test('removing ENOSYS from the real runtime table is detected', () => {
  const missing = loopSource.replace(/\bENOSYS: 38, /, '');
  assert.notEqual(missing, loopSource);
  assert.throws(() => errnoFor('win32', missing), /must define a numeric ENOSYS/);
});

function load(platform, text = source) {
  const effects = new Map();
  const context = vm.createContext({
    process: { platform }, Buffer, TextEncoder, TextDecoder,
    require: () => { throw Error('native library must not load'); },
    io_eff: (name, f) => effects.set(name, f),
    io_fail: error => ({ ok: false, error }),
    io_done: value => ({ ok: true, value }),
    BP_H: { ERRNO: errnoFor(platform) },
  });
  vm.runInContext(text, context);
  return { context, run: effects.get('Sys.exec') };
}

test('Windows reexec refuses before libc, descriptor changes or handoff hooks', () => {
  const { context, run } = load('win32');
  const touched = [];
  context.host_libc = () => { touched.push('libc'); throw Error('libc loaded'); };
  context.host_fds = () => { touched.push('fds'); return []; };
  context.BACKPLANE_ADOPT_EXEC = () => touched.push('adopt');
  context.BACKPLANE_HUBLOCK_EXEC = () => touched.push('hublock');
  const result = run('C:\\Backplane\\service.exe', { $: 'Nil' });
  assert.equal(result.ok, false); assert.equal(result.error, errnoFor('win32').ENOSYS);
  assert.deepEqual(touched, []);
});

test('removing the Windows guard is detected by the native-library sentinel', () => {
  const unguarded = source.replace(/  if \(process\.platform === "win32"\) \{\n    return io_fail\(globalThis\.BP_H\.ERRNO\.ENOSYS\);\n  \}\n/, '');
  assert.notEqual(unguarded, source);
  const { context, run } = load('win32', unguarded);
  context.host_libc = () => { throw Error('libc loaded'); };
  assert.throws(() => run('C:\\service.exe', { $: 'Nil' }), /libc loaded/);
});

for (const platform of ['linux', 'darwin']) {
  test(platform + ' keeps the POSIX exec attempt and restores hooks on failure', () => {
    const { context, run } = load(platform);
    const calls = [];
    context.host_libc = () => ({ ptr: x => x, execve: (...args) => calls.push(['execve', ...args]) });
    context.host_self = xs => xs;
    context.host_list_js = () => ['--foreground'];
    context.host_fds = () => [3];
    context.io_sys = () => ({ fcntl: (...args) => calls.push(['fcntl', ...args]), errno: () => 13 });
    context.host_vec = xs => ({ bufs: [xs[0]], vec: xs });
    context.host_envp = () => ['TEST=1'];
    context.host_eilseq = () => 22;
    context.BACKPLANE_ADOPT_EXEC = mode => calls.push(['adopt', mode]);
    context.BACKPLANE_HUBLOCK_EXEC = mode => calls.push(['hublock', mode]);
    const result = run('/fixture/backplane', { $: 'Nil' });
    assert.equal(result.error, 13);
    assert.deepEqual(calls.map(x => x[0]), ['fcntl', 'adopt', 'hublock', 'execve', 'hublock', 'adopt']);
    assert.deepEqual(calls.filter(x => ['adopt', 'hublock'].includes(x[0])), [['adopt', 1], ['hublock', 1], ['hublock', 0], ['adopt', 0]]);
  });
}
