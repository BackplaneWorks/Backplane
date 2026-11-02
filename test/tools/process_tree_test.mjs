// Production Windows effects against a deterministic kernel model. No live
// agents, process-name matching, shell commands or Windows installation needed.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const source = readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function fixture() {
  const rows = new Map(), handles = new Set(), killed = [], parked = [];
  let error = 18, deny = 0, snapshotError = false, onTerminate = () => {}, onOpen = () => {}, now = Date.now();
  const add = (pid, parent, born = BigInt(pid)) => {
    const row = { pid, parent, born, dead: false, terminated: false };
    rows.set(pid, row);return row;
  };
  const k = {
    ptr: x => x,
    OpenProcess: (rights, inherit, pid) => {
      assert.equal(rights, 0x101001);assert.equal(inherit, 0);
      if (pid === deny) { error = 5;return null; }
      onOpen(pid);
      const row = rows.get(pid);if (!row) { error = 87;return null; }
      const handle = { row, closed: false };handles.add(handle);return handle;
    },
    CloseHandle: handle => { assert.equal(handle.closed, false);handle.closed = true;handles.delete(handle);return 1; },
    GetProcessTimes: (handle, born) => { born[0] = handle.row.born;return 1; },
    WaitForSingleObject: (handle, timeout) => { assert.equal(timeout, 0);assert.equal(handle.closed, false);return handle.row.dead ? 0 : 258; },
    TerminateProcess: (handle, code) => {
      assert.equal(handle.closed, false);assert.equal(code, 143);
      if (!handle.row.terminated) { handle.row.terminated = true;killed.push(handle.row.pid);onTerminate(handle.row); }
      return 1; // deliberately asynchronous, as the real API is
    },
    CreateToolhelp32Snapshot: () => {
      if (snapshotError) return -1;
      const snapshot = { rows: [...rows.values()], at: 0, closed: false };handles.add(snapshot);return snapshot;
    },
    Process32FirstW: (snapshot, entry) => { snapshot.at = 0;return write(snapshot, entry); },
    Process32NextW: (snapshot, entry) => { snapshot.at++;return write(snapshot, entry); },
    GetLastError: () => error,
  };
  function write(snapshot, entry) {
    const row = snapshot.rows[snapshot.at];
    if (!row) { error = 18;return 0; }
    const view = new DataView(entry.buffer);
    assert.equal(view.getUint32(0, true), 568);
    view.setUint32(8, row.pid, true);view.setUint32(32, row.parent, true);return 1;
  }
  const globals = { BP_WIN_PROCESS: k, BP_PROCS: new Map() };
  const processShim = { platform: 'win32', pid: 5000, env: {}, kill: () => { throw Error('unowned raw PID kill'); } };
  const effects = new Map();
  const api = new Function('require', 'process', 'globalThis', 'io_eff', 'io_park_promise', 'io_done', 'io_fail', 'Date', source + `
    return { open: bp_win_process_open, stop: bp_win_tree_stop, kill: bp_proc_kill,
      retire: proc_retire, barrier: thread => proc_await_retired(thread, "", null), wait: bp_proc_wait,
      begin: proc_turn_begin, cancel: proc_turn_cancel, spawned: proc_turn_spawned, failed: proc_turn_failed,
      enter: (thread, gen) => proc_await_retired(thread, gen, null) };
  `)(require, processShim, globals, (id, fn) => effects.set(id, fn),
    (promise, continuation, done) => {
      const result = Promise.resolve(promise).then(value => done(value), error => done(undefined, error));
      parked.push(result);return result;
    }, value => ({ ok: true, value }), (errno, message) => ({ ok: false, errno, message }), { now: () => now });
  function child(pid) {
    let resolve;
    const c = { pid, exitCode: null, signalCode: null, exited: new Promise(r => { resolve = r; }), kill: () => { throw Error('leader-only kill'); } };
    c.bpTree = { members: new Map([[pid, api.open(pid)]]), closed: false, stopping: null };
    c.exit = () => { rows.get(pid).dead = true;c.exitCode = 143;resolve(143); };
    globals.BP_PROCS.set(pid, c);return c;
  }
  return { add, child, api, rows, handles, killed, parked, globals,
    deny: pid => { deny = pid; }, snapshotError: v => { snapshotError = v; },
    onTerminate: fn => { onTerminate = fn; }, onOpen: fn => { onOpen = fn; }, advance: ms => { now += ms; } };
}

// A root-only Windows signal is the original failure: the writer stays alive.
{
  const before = process.env.BP_TREE_BASELINE ? readFileSync(process.env.BP_TREE_BASELINE, 'utf8') : '';
  if (before) {
    const oldKill = before.slice(before.indexOf('function bp_proc_kill('), before.indexOf('// Where an agent\'s stderr goes'));
    let leader = true, writer = true;
    const f = new Function('process', 'bp_procs', oldKill + '; return bp_proc_kill;')(
      { platform: 'win32' }, () => new Map([[42, { kill() { leader = false; } }]]));
    f(42, 15);
    assert.equal(leader, false);assert.equal(writer, true);
    console.log('confirmed origin/main: wrapper killed, writer survives');
  }
}

for (let turn = 0; turn < 3; turn++) {
  const f = fixture(), root = 100 + turn * 10;
  f.add(root, 5000, 100n);f.add(root + 1, root, 101n);f.add(root + 2, root + 1, 102n);
  f.add(900, 5000, 80n); // another thread
  f.add(901, root, 90n); // stale PPID predating this parent
  f.add(5000, root, 1n); // never the hub itself
  const c = f.child(root);
  let exited = false;
  const reader = f.api.wait(root).then(() => { exited = true; });
  f.api.retire('same-codex-session', root);
  const barrier = f.api.barrier('same-codex-session');
  let resumed = false;barrier.then(result => { assert.equal(result.ok, true);resumed = true; });
  await delay(5);
  assert.deepEqual(f.killed, [root, root + 1, root + 2]);
  c.exit();await delay(5);
  assert.equal(resumed, false, 'wrapper exit alone must not resume');
  assert.equal(exited, false, 'reader/reaper shares the tree completion barrier');
  f.rows.get(root + 1).dead = true;
  await delay(25);assert.equal(resumed, false, 'grandchild still owns the writer');
  f.rows.get(root + 2).dead = true;
  await Promise.all([barrier, reader]);await delay(0);
  assert.equal(resumed, true);assert.equal(f.handles.size, 0, 'all process and snapshot handles closed');
  assert.equal(f.rows.get(900).terminated, false);assert.equal(f.rows.get(901).terminated, false);
  assert.equal(f.globals.BP_RETIREMENTS.size, 0);
}
console.log('ok repeated wrapper/writer/grandchild handoffs wait for every owned process, isolate other threads and close handles');

{
  const f = fixture();f.add(42, 5000, 1n);f.add(43, 42, 2n);const c = f.child(42);
  let late = false;
  f.onTerminate(row => {
    if (row.pid === 43 && !late) { late = true;f.add(44, 43, 3n); }
    row.dead = true;
  });
  f.api.retire('late-child', 42);
  assert.equal((await f.api.barrier('late-child')).ok, true);
  assert.deepEqual(f.killed, [42, 43, 44], 'a child born during termination is captured in the final walk');
  assert.equal(f.handles.size, 0);c.exit();
}
console.log('ok descendants born during termination cannot escape the final snapshot');

for (const failure of ['access', 'snapshot']) {
  const f = fixture();f.add(42, 5000, 1n);f.add(43, 42, 2n);const c = f.child(42);
  if (failure === 'access') f.deny(43);else f.snapshotError(true);
  f.api.retire('blocked', 42);
  const result = await f.api.barrier('blocked');
  assert.equal(result.ok, false);assert.match(result.message, /refusing to resume/);
  assert.equal(f.globals.BP_RETIREMENTS.get('blocked').size, 1, 'failed cleanup survives registry removal');
  f.globals.BP_PROCS.delete(42);f.deny(0);f.snapshotError(false);f.onTerminate(row => { row.dead = true; });
  assert.equal((await f.api.barrier('blocked')).ok, true, 'Retry reuses captured identities after the reader reaps the root');
  assert.equal(f.handles.size, 0);c.exit();
}
console.log('ok access/enumeration failures refuse resume; retry retains ownership through registry removal');

{
  const f = fixture();
  for (const pid of [0, 1, -1, 12345]) f.api.kill(pid, 15);
  f.api.retire('placeholder', 0);
  assert.equal(f.killed.length, 0);assert.equal((await f.api.barrier('placeholder')).ok, true);
  f.api.retire('unknown', 12345);
  assert.equal((await f.api.barrier('unknown')).ok, false, 'an unowned PID cannot be treated as confirmed cleanup');
}
console.log('ok placeholders, protected and unowned PIDs are never signalled');

{
  const f = fixture();f.add(42, 5000, 1n);f.add(43, 42, 2n);const c = f.child(42);
  f.onOpen(pid => { if (pid === 43) f.add(43, 999, 10n); });
  f.onTerminate(row => { row.dead = true; });
  f.api.retire('reused-child', 42);
  assert.equal((await f.api.barrier('reused-child')).ok, true);
  assert.deepEqual(f.killed, [42], 'PID reused during OpenProcess is revalidated before signalling');
  assert.equal(f.handles.size, 0);c.exit();
}
console.log('ok a descendant PID reused between snapshot and OpenProcess never kills the replacement process');

{
  const f = fixture();f.add(42, 5000, 1n);const c = f.child(42);
  f.onTerminate(() => { f.advance(10001); });
  f.api.retire('timeout', 42);
  assert.equal((await f.api.barrier('timeout')).ok, false, 'a timeout never permits resume');
  f.rows.get(42).dead = true;
  assert.equal((await f.api.barrier('timeout')).ok, true);
  assert.equal(f.handles.size, 0);c.exit();
}
console.log('ok termination timeout refuses resume; a later retry confirms exit');

{
  const f = fixture();f.add(42, 5000, 1n);const c = f.child(42);
  f.api.retire('rapid', 42);
  f.api.begin('rapid', 'old-steer');
  const old = f.api.enter('rapid', 'old-steer');
  f.api.cancel('rapid');f.api.begin('rapid', 'new-steer');
  const latest = f.api.enter('rapid', 'new-steer');
  c.exit();
  assert.equal((await old).ok, false, 'the superseded waiter cannot resume');
  assert.equal((await latest).ok, true);
  f.api.failed('rapid', 'new-steer');
}
console.log('ok rapid steers sharing one retirement admit only the latest generation');

{
  const f = fixture();f.api.begin('preparing', 'old');
  assert.equal((await f.api.enter('preparing', 'old')).ok, true);
  f.api.cancel('preparing');f.api.begin('preparing', 'new');
  let admitted = false;
  const next = f.api.enter('preparing', 'new').then(result => { admitted = result.ok;return result; });
  await delay(0);assert.equal(admitted, false, 'credential preparation still holds the launch lease');
  f.add(42, 5000, 1n);const c = f.child(42);
  assert.equal(f.api.spawned('preparing', 'old', 42).$, 'False');
  await delay(0);assert.equal(admitted, false, 'a late PID is retired before the new launch can enter');
  c.exit();assert.equal((await next).ok, true);
  f.api.failed('preparing', 'new');assert.equal(f.handles.size, 0);
}
console.log('ok a superseded launch still preparing credentials is serialized, then its late PID is retired before resume');
