// Execute the production binary-file effect against real temporary files.
// Backend routing is stubbed; actual isolated-hub streaming is a separate gate.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {resolve, join} from 'node:path';
import {tmpdir} from 'node:os';
import {runInNewContext} from 'node:vm';
const root = resolve(process.env.VIEWER_REVIEW_SOURCE_ROOT || '.');
const source = fs.readFileSync(join(root, 'src/server/effects/host.js'), 'utf8');
const start = source.indexOf('function sock_send_file('), end = source.indexOf('\n// Sock.send_until:', start);
assert.ok(start >= 0 && end > start, 'production effect not found');
const effect = source.slice(start, end);
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-asset-file-'));
const path = join(dir, 'model.glb'), other = join(dir, 'different.glb');
const bytes = Buffer.from(Array.from({length: 4096}, (_, i) => i % 256));
fs.writeFileSync(path, bytes);fs.writeFileSync(other, Buffer.from('other source'));
function context({bun = false, parked = false, override = {}, absentHandles = false} = {}) {
  const sent = [], opened = new Set(), closed = [], fileReads = [];
  const shim = {...fs,
    openSync(...args) { const fd = (override.openSync || fs.openSync)(...args);opened.add(fd);return fd; },
    closeSync(fd) { opened.delete(fd);closed.push(fd);return (override.closeSync || fs.closeSync)(fd); },
    readFileSync(fd) { fileReads.push(fd);return (override.readFileSync || fs.readFileSync)(fd); },
    ...Object.fromEntries(Object.entries(override).filter(([key]) => !['openSync','closeSync','readFileSync'].includes(key))),
  };
  const sandbox = {require: name => { assert.equal(name, 'fs');return shim; },
    io_tup: (socket, result) => ({socket, result}), io_done: value => ({ok: true, value}), io_fail: errno => ({ok: false, errno}),
    host_send: (socket, data, continuation) => { sent.push({backend:'posix', socket, data:Buffer.from(data), continuation});return {backend:'posix'}; },
    bp_send_with: (socket, data, continuation, options) => { sent.push({backend:'bun', socket, data:Buffer.from(data), continuation, options});return parked ? undefined : {backend:'bun'}; },
  };
  if (!absentHandles) sandbox.BP_H = {is: socket => bun && socket === 73};
  return {send: runInNewContext(`(${effect})`, sandbox), sent, opened, closed, fileReads};
}
try {
  for (const options of [{bun:false}, {bun:true}, {bun:true, parked:true}, {absentHandles:true}]) {
    const c = context(options), continuation = () => {};
    const result = c.send(73, path, bytes.length, continuation);
    assert.equal(c.sent.length, 1);assert.deepEqual(c.sent[0].data, bytes);
    assert.equal(c.sent[0].socket, 73);assert.equal(c.sent[0].continuation, continuation);
    assert.equal(c.sent[0].backend, options.bun ? 'bun' : 'posix');
    if (options.bun) assert.equal(c.sent[0].options, null);
    if (options.parked) assert.equal(result, undefined, 'parking must return to the existing continuation owner');
    assert.equal(c.opened.size, 0);assert.equal(c.closed.length, 1);
  }
  for (const cap of [0, -1, NaN, Infinity, 1.5, 67108865]) {
    const c = context(), result = c.send(73, path, cap, () => {});
    assert.equal(result.result.ok, false);assert.equal(result.result.errno, 22);
    assert.equal(c.sent.length, 0);assert.equal(c.opened.size, 0);assert.equal(c.fileReads.length, 0);
  }
  const tooBig = context(), result = tooBig.send(73, path, bytes.length - 1, () => {});
  assert.equal(result.result.errno, 27);assert.equal(tooBig.sent.length, 0);assert.equal(tooBig.fileReads.length, 0);assert.equal(tooBig.opened.size, 0);
  fs.symlinkSync(path, join(dir, 'link.glb'));
  for (const invalid of [join(dir, 'link.glb'), dir, path + '\0suffix', join(dir, 'missing.glb')]) {
    const c = context(), result = c.send(73, invalid, 67108864, () => {});
    assert.equal(result.result.ok, false);assert.equal(c.sent.length, 0);assert.equal(c.opened.size, 0);
  }
  const raced = context({override:{openSync: () => fs.openSync(other, fs.constants.O_RDONLY)}});
  assert.equal(raced.send(73, path, 67108864, () => {}).result.errno, 27);
  assert.equal(raced.sent.length, 0);assert.equal(raced.fileReads.length, 0);assert.equal(raced.opened.size, 0);
  const changedLength = context({override:{readFileSync: () => Buffer.from('short')}});
  assert.equal(changedLength.send(73, path, bytes.length, () => {}).result.errno, 5);
  assert.equal(changedLength.sent.length, 0);assert.equal(changedLength.opened.size, 0);
  const ioFailure = context({override:{readFileSync: () => { throw {errno: -5}; }}});
  assert.equal(ioFailure.send(73, path, bytes.length, () => {}).result.errno, 5);
  assert.equal(ioFailure.sent.length, 0);assert.equal(ioFailure.opened.size, 0);
  const empty = join(dir, 'empty.glb');fs.writeFileSync(empty, '');
  const c = context();c.send(73, empty, 1, () => {});assert.equal(c.sent[0].data.length, 0);assert.equal(c.opened.size, 0);
  console.log('ok production binary asset effect: all byte values unchanged, Bun/parked/POSIX routing, caps, symlink/directory/NUL rejection, descriptor-identity race, changed length/read failure and descriptor cleanup');
} finally { fs.rmSync(dir, {recursive:true, force:true}); }
