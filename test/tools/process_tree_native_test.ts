// Native Win32 regression (Bun, no Bend build and no real provider).
// bun test/tools/process_tree_native_test.ts
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { appendFileSync, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const [mode, fixture, portText] = process.argv.slice(2);
if (mode === 'wrapper' || mode === 'writer' || mode === 'grandchild') {
  appendFileSync(join(fixture, 'pids'), process.pid + '\n');
  if (mode === 'wrapper') spawn(process.execPath, [import.meta.filename, 'writer', fixture, portText], { stdio: 'inherit', windowsHide: true });
  if (mode === 'writer') {
    // A listening socket stands in for Codex's OS-owned writer lock.
    const listener = Bun.listen({ hostname: '127.0.0.1', port: Number(portText), socket: { data() {} } });
    spawn(process.execPath, [import.meta.filename, 'grandchild', fixture, portText], { stdio: 'inherit', windowsHide: true });
    appendFileSync(join(fixture, 'ready'), String(listener.port));
  }
  setInterval(() => {}, 1000);
} else if (process.platform !== 'win32') {
  console.log('skip native Win32 tree test: needs Windows; process_tree_test.mjs tests the kernel model everywhere');
} else {
  const source = readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');
  const globals: any = { BP_H: {
    Stream: class { feed() {} finish() {} wake() {} }, add: () => 1, ERRNO: { ENOTDIR: 20, ENOENT: 2 },
  } };
  const api = new Function('require', 'process', 'globalThis', 'io_eff', 'io_park_promise', 'io_done', 'io_fail', 'io_tup', source + `
    return { spawn: bp_proc_spawn, retire: proc_retire, barrier: thread => proc_await_retired(thread, "", null),
      open: bp_win_process_open, dead: bp_win_tree_exited, close: member => bp_win_process_api().CloseHandle(member.handle) };
  `)(require, process, globals, () => {},
    (promise: Promise<any>, k: any, done: Function) => promise.then(value => done(value), error => done(undefined, error)),
    (value: any) => ({ ok: true, value }), (errno: number, message: string) => ({ ok: false, errno, message }), (a: any, b: any) => [a, b]);
  const root = mkdtempSync(join(tmpdir(), 'bp-native-tree-'));
  const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
  const captured: any[] = [];
  try {
    for (let turn = 0; turn < 3; turn++) {
      const path = join(root, String(turn));require('fs').mkdirSync(path);
      const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } });const port = listener.port;listener.stop(true);
      const started = api.spawn(process.execPath, [resolve(import.meta.filename), 'wrapper', path, String(port)], path, 2);
      assert.equal(started.ok, true, started.message);
      const pid = started.value[0];
      const end = Date.now() + 10000;
      while ((!existsSync(join(path, 'ready')) || readFileSync(join(path, 'pids'), 'utf8').trim().split('\n').length < 3) && Date.now() < end) await sleep(20);
      assert.ok(existsSync(join(path, 'ready')), 'writer acquired the session lock');
      const pids = readFileSync(join(path, 'pids'), 'utf8').trim().split('\n').map(Number);
      assert.equal(pids.length, 3);assert.equal(pids[0], pid);
      const members = pids.map((pid: number) => api.open(pid));captured.push(...members);
      api.retire('same-session', pid);
      const barrier = await api.barrier('same-session');assert.equal(barrier.ok, true, barrier.message);
      assert.ok(members.every((member: any) => api.dead(member)), 'wrapper, writer and grandchild all exited before resume');
      const resumed = Bun.listen({ hostname: '127.0.0.1', port, socket: { data() {} } });resumed.stop(true);
      // Node's streams finish only after the descendants holding them die.
      await globals.BP_PROCS.get(pid).exited;
      for (const member of members) { api.close(member);captured.splice(captured.indexOf(member), 1); }
      globals.BP_PROCS.delete(pid);
    }
    console.log('ok native Win32: three wrapper/writer/grandchild handoffs release the OS writer lock before resume');
  } finally {
    for (const pid of globals.BP_PROCS?.keys() ?? []) {
      api.retire('fixture-cleanup-' + pid, pid);
      await api.barrier('fixture-cleanup-' + pid);
    }
    for (const member of captured) {
      // Cleanup only exact process handles captured from our fixture.
      try { globals.BP_WIN_PROCESS.TerminateProcess(member.handle, 143);api.close(member); } catch {}
    }
    rmSync(root, { recursive: true, force: true });
  }
}
