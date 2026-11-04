// Run the production effect with real files and deterministic writable/timer
// wakeups. Private runtime helpers are deliberately absent from its scope.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {resolve, join} from 'node:path';
import {tmpdir} from 'node:os';
import {runInNewContext} from 'node:vm';
const root = resolve(process.env.VIEWER_REVIEW_SOURCE_ROOT || '.');
const source = fs.readFileSync(join(root, 'src/server/effects/host.js'), 'utf8');
const start = source.indexOf('function sock_send_file('), end = source.indexOf('\n// Sock.send_path:', start);
assert.ok(start >= 0 && end > start, 'production effect not found');
const effect = source.slice(start, end);
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-asset-file-'));
const path = join(dir, 'model.glb'), other = join(dir, 'different.glb');
const bytes = Buffer.from(Array.from({length: 3 * 65536 + 37}, (_, i) => i % 256));
fs.writeFileSync(path, bytes);fs.writeFileSync(other, Buffer.from('other source'));
function context({bun = false, absentHandles = false, writes = [], maxWrite = Infinity, override = {}, throwWrite = false, platform = process.platform} = {}) {
  const sent = [], opened = new Set(), closed = [], reads = [], parks = [];
  let clock = 0, available = true, errno = 0;
  const shim = {...fs, ...override,
    openSync(...args) { const fd = (override.openSync || fs.openSync)(...args);opened.add(fd);return fd; },
    closeSync(fd) { opened.delete(fd);closed.push(fd);return (override.closeSync || fs.closeSync)(fd); },
    readSync(fd, buf, offset, length, position) {
      reads.push({length, position});
      assert.ok(length <= 65536, 'retained read buffer exceeds 64 KiB');
      return (override.readSync || fs.readSync)(fd, buf, offset, length, position);
    },
    readFileSync() { assert.fail('whole-file read is forbidden'); },
  };
  const write = b => {
    if (throwWrite) throw {errno:-32};
    const n = writes.length ? writes.shift() : Math.min(b.length, maxWrite);
    if (n > 0) sent.push(Buffer.from(b.subarray(0, n)));
    return n;
  };
  const sandbox = {Buffer, process: {platform}, performance: {now: () => clock},
    require: name => { assert.equal(name, 'fs');return shim; },
    io_tup: (socket, result) => ({socket, result}), io_done: value => ({ok: true, value}), io_fail: errno => ({ok: false, errno}),
    host_errno: e => Math.abs(e.errno || 5),
    io_sys: () => {assert.notEqual(platform, 'win32', 'Windows handles need no POSIX interface');return {mac: platform === 'darwin', ptr: (b, at) => b.subarray(at), errno: () => errno};},
    host_write: (socket, ptr, length) => { assert.equal(socket, 73);const n = write(ptr.subarray(0, length));errno = n === -11 && platform === 'darwin' ? 35 : -n;return n < 0 ? -1 : n; },
    io_park_on: (fd, writable, continuation, more, deadline) => parks.push({fd, writable, continuation, more, deadline}),
  };
  if (!absentHandles) sandbox.BP_H = {EAGAIN: 11, is: socket => bun && socket === 73, get: () => available ? {write} : undefined};
  const continuation = () => {};
  return {send: (p = path, cap = bytes.length) => runInNewContext(`(${effect})`, sandbox)(73, p, cap, continuation),
    sent, opened, closed, reads, parks,
    disconnect() { available = false; },
    resume(time = clock) {
      const p = parks.shift();assert.ok(p, 'expected a parked continuation');
      assert.equal(p.continuation, continuation);clock = time;return p.more();
    },
    drain(result) {
      let fuel = 10000;
      while (result === undefined && fuel-- > 0) result = this.resume();
      assert.ok(fuel > 0, 'sender failed to make bounded progress');return result;
    },
  };
}
function done(c, result, expected = bytes) {
  assert.equal(result.socket, 73);assert.equal(result.result.ok, true);
  assert.deepEqual(Buffer.concat(c.sent), expected);assert.equal(c.opened.size, 0);assert.equal(c.closed.length, 1);
}
try {
  for (const options of [{bun:false}, {bun:true}, {absentHandles:true}, {bun:true,maxWrite:257}, {maxWrite:113}, {bun:true,platform:'win32'}, {bun:true,platform:'darwin'}, {platform:'darwin'}]) {
    const c = context(options), result = c.send();
    assert.equal(result, undefined, 'multi-piece transfer must yield');
    assert.equal(c.reads.length, 1);assert.equal(c.opened.size, 1);
    assert.equal(c.parks[0].fd, undefined, 'yield between pieces is independent of socket readiness');
    done(c, c.drain(result));
  }
  for (const bun of [false, true]) {
    for (const blocked of [0, -11]) {
      const c = context({bun, writes: [13, blocked, blocked]});
      assert.equal(c.send(), undefined);assert.equal(c.parks[0].writable, true);
      assert.equal(c.parks[0].fd, 73);assert.equal(c.reads.length, 1);
      assert.equal(c.resume(999), undefined);assert.equal(c.reads.length, 1, 'do not read ahead of a blocked piece');
      done(c, c.drain(c.resume(1000)));
    }
    const c = context({bun,writes:[-11,-11]});
    assert.equal(c.send(), undefined);assert.equal(c.resume(29999), undefined);
    const result = c.resume(30000);
    assert.equal(result.result.errno, process.platform === 'darwin' ? 60 : 110);
    assert.equal(c.opened.size, 0);assert.equal(c.closed.length, 1);
    const failed = context({bun,writes:[-32]});
    assert.equal(failed.send().result.errno, 32);assert.equal(failed.opened.size, 0);
    const thrown = context({bun,throwWrite:true});
    assert.equal(thrown.send().result.errno, 32);assert.equal(thrown.opened.size, 0);
  }
  const disconnect = context({bun:true,writes:[-11]});disconnect.send();disconnect.disconnect();
  assert.equal(disconnect.resume().result.errno, 9);assert.equal(disconnect.opened.size, 0);
  const renewed = context({bun:true,writes:[1,-11,1,-11]});renewed.send();
  assert.equal(renewed.resume(29999), undefined);done(renewed, renewed.drain(renewed.resume(30001)));
  const mac = context({bun:true,platform:'darwin',writes:[-11]});mac.send();done(mac,mac.drain(mac.resume()));
  for (const cap of [0, -1, NaN, Infinity, 1.5, 67108865]) {
    const c = context(), result = c.send(path, cap);
    assert.equal(result.result.errno, 22);assert.equal(c.sent.length, 0);assert.equal(c.opened.size, 0);assert.equal(c.reads.length, 0);
  }
  const tooBig = context();assert.equal(tooBig.send(path, bytes.length - 1).result.errno, 27);
  assert.equal(tooBig.reads.length, 0);assert.equal(tooBig.opened.size, 0);
  fs.symlinkSync(path, join(dir, 'link.glb'));
  for (const invalid of [join(dir, 'link.glb'), dir, path + '\0suffix', join(dir, 'missing.glb')]) {
    const c = context();assert.equal(c.send(invalid).result.ok, false);assert.equal(c.sent.length, 0);assert.equal(c.opened.size, 0);
  }
  const raced = context({override:{openSync: () => fs.openSync(other, fs.constants.O_RDONLY)}});
  assert.equal(raced.send().result.errno, 27);assert.equal(raced.reads.length, 0);assert.equal(raced.opened.size, 0);
  const short = context({override:{readSync: (fd, buf, off, n, pos) => fs.readSync(fd, buf, off, Math.min(n, 127), pos)}});
  done(short, short.drain(short.send()));
  const truncated = context({override:{readSync: () => 0}});
  assert.equal(truncated.send().result.errno, 5);assert.equal(truncated.opened.size, 0);
  const midTransfer = context();midTransfer.send();fs.truncateSync(path, 65536);
  assert.equal(midTransfer.resume().result.errno, 5);assert.equal(midTransfer.opened.size, 0);
  fs.writeFileSync(path, bytes);
  const ioFailure = context({override:{readSync: () => {throw {errno:-5};}}});
  assert.equal(ioFailure.send().result.errno, 5);assert.equal(ioFailure.opened.size, 0);
  const closeFailure = context({override:{closeSync(fd) {fs.closeSync(fd);throw {errno:-5};}}});
  assert.equal(closeFailure.drain(closeFailure.send()).result.errno, 5);assert.equal(closeFailure.opened.size, 0);
  const empty = join(dir, 'empty.glb');fs.writeFileSync(empty, '');
  const c = context();done(c, c.send(empty, 1), Buffer.alloc(0));assert.equal(c.reads.length, 0);
  console.log('ok binary asset streaming: byte parity, 64KiB bound, partial writes, yields, backpressure, timeout/disconnect, identity/cap checks and descriptor cleanup');
} finally {fs.rmSync(dir, {recursive:true, force:true});}
