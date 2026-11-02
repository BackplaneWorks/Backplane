// Real body-sender effect plus a Bend wiring contract, without a compiler/hub.
import assert from 'node:assert/strict';
import {after, test} from 'node:test';
import * as fs from 'node:fs';
import {join, resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {runInNewContext} from 'node:vm';

const host = fs.readFileSync(resolve('src/server/effects/host.js'), 'utf8');
const effect = host.slice(host.indexOf('const SEND_PATH_PIECE ='), host.indexOf('// Sock.send_until:'));
assert.ok(effect.includes('function sock_send_path('));
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-farview-body-'));
after(() => fs.rmSync(dir, {recursive: true, force: true}));
const file = join(dir, 'synthetic.bin');
const bytes = Buffer.alloc(2 * 262144 + 37);
for (let i = 0; i < bytes.length; i++) bytes[i] = i % 251;
fs.writeFileSync(file, bytes);

function sender({backend = 'handle', write, read} = {}) {
  const queue = [], chunks = [], opened = new Set(), reads = [];
  let closed = 0, attempts = 0, errno = 0;
  const shim = {...fs,
    openSync(...args) { const fd = fs.openSync(...args); opened.add(fd); return fd; },
    closeSync(fd) { assert.ok(opened.delete(fd)); closed++; fs.closeSync(fd); },
    readFileSync() { assert.fail('body transport must not read the whole file'); },
    readSync(...args) {
      assert.ok(args[3] <= 262144, 'bounded read'); reads.push(args[3]);
      return read ? read(...args) : fs.readSync(...args);
    },
  };
  const put = b => {
    const n = write ? write(++attempts, b) : b.length;
    if (n > 0) chunks.push(Buffer.from(b.subarray(0, n)));
    return n;
  };
  const sandbox = {
    require: name => { assert.equal(name, 'fs'); return shim; },
    performance: {now: () => 0},
    io_sys: () => ({mac: false, ptr: b => b, errno: () => errno}),
    host_write: (_socket, b) => { const n = put(b); errno = -n; return n < 0 ? -1 : n; },
    host_errno: e => Math.abs(e.errno || 5),
    io_tup: (socket, result) => ({socket, result}),
    io_done: () => ({ok: true}), io_fail: code => ({ok: false, code}),
    io_park_on: (_socket, _out, _k, go) => { queue.push(go); },
  };
  if (backend === 'handle') sandbox.BP_H = {EAGAIN: 11, is: () => true, get: () => ({write: put})};
  const send = runInNewContext(effect + '\nsock_send_path;', sandbox);
  return {
    reads, chunks, get closed() { return closed; },
    run(path = file, length = bytes.length) {
      let result = send(73, path, 0, length, () => assert.fail('unexpected continuation'));
      let turns = 0;
      while (result === undefined) {
        assert.ok(++turns < 10000 && queue.length, 'parked send terminates');
        result = queue.shift()();
      }
      assert.equal(opened.size, 0); assert.equal(queue.length, 0);
      return result.result;
    },
  };
}

test('far-view wiring emits one head, bounds conversion, and closes the body socket', () => {
  const source = fs.readFileSync(resolve('src/server/server.bend'), 'utf8');
  const block = name => source.split('def ' + name + '(', 2)[1].split('\ndef ', 1)[0];
  const body = block('Conn.bot.stream.k'), wrapper = block('Conn.bot.stream');
  assert.equal((body.match(/Conn\.send\(/g) || []).length, 1);
  assert.match(body, /BRANGE\.Rng\.head\([^\n]+"application\/octet-stream", "no-store"/);
  assert.match(body, /H\.Sock\.send_path\(s2, path, 0, U32\.from_nat\(size\)\)/);
  assert.match(body, /Conn\.sent\(sent\)[\s\S]*Socket\.close\(s3\)/);
  assert.match(body, /case False\{\}:[\s\S]*413 Payload Too Large/);
  assert.match(wrapper, /Nat\.is_le\(size, FV\.Fv\.cap\(\)\)/);
  assert.match(wrapper, /Nat\.is_le\(size, 4294967295n\)/);
  assert.doesNotMatch(body + wrapper, /Img\.send|send_preview/);
  assert.ok(source.indexOf('def Conn.bot.stream.k(') < source.indexOf('def Conn.bot.stream('));
});

for (const backend of ['handle', 'posix']) {
  test(`${backend}: body bytes survive partial writes and backpressure without a second header`, () => {
    const c = sender({backend, write: (n, b) => n % 3 === 0 ? -11 : Math.min(b.length, 4093)});
    assert.equal(c.run().ok, true);
    const body = Buffer.concat(c.chunks);
    assert.deepEqual(body, bytes);
    // Model the already-sent head; the production effect contributes only bytes.
    const head = Buffer.from(`HTTP/1.1 200 OK\r\nContent-Length: ${bytes.length}\r\n\r\n`);
    assert.equal(Buffer.concat([head, body]).indexOf('HTTP/1.1', head.length), -1);
    assert.equal(c.reads.length, 3); assert.equal(c.closed, 1);
  });
}

test('disconnect closes the file and never appends an HTTP error body', () => {
  const c = sender({write: (n, b) => n === 1 ? Math.min(100, b.length) : -32});
  assert.equal(c.run().code, 32);
  assert.deepEqual(Buffer.concat(c.chunks), bytes.subarray(0, 100));
  assert.equal(c.closed, 1);
});

test('short reads and read failures close the file without injected bytes', () => {
  for (const mode of ['short', 'error']) {
    const c = sender({read: (...args) => {
      if (args[4] === 0) return fs.readSync(...args);
      if (mode === 'short') return 0;
      throw {errno: 5};
    }});
    assert.equal(c.run().code, 5);
    assert.deepEqual(Buffer.concat(c.chunks), bytes.subarray(0, 262144));
    assert.equal(c.closed, 1);
  }
});

test('zero-length bodies close without file reads', () => {
  const c = sender(); assert.equal(c.run(file, 0).ok, true);
  assert.equal(c.chunks.length, 0); assert.equal(c.reads.length, 0); assert.equal(c.closed, 1);
});

test('the existing body sender retains its no-follow refusal', {skip: !fs.constants.O_NOFOLLOW}, () => {
  const link = join(dir, 'link.bin'); fs.symlinkSync(file, link);
  const c = sender(); assert.equal(c.run(link).ok, false);
  assert.equal(c.chunks.length, 0); assert.equal(c.closed, 0);
});
