// Actual host.js effect under simulated platforms; no native exec or Windows job.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const test = require('node:test');
const source = fs.readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');

function load(platform, text = source) {
  const effects = new Map();
  const context = vm.createContext({
    process: { platform }, Buffer, TextEncoder, TextDecoder,
    require: () => { throw Error('native library must not load'); },
    io_eff: (name, f) => effects.set(name, f),
    io_fail: error => ({ ok: false, error }),
    io_done: value => ({ ok: true, value }),
    BP_H: { ERRNO: { ENOSYS: 38 } },
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
  assert.equal(result.ok, false); assert.equal(result.error, 38);
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
