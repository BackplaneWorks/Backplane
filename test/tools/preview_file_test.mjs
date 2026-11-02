// Production preview sender against real files, with controlled socket writes.
// node --test test/tools/preview_file_test.mjs
// bun test ./test/tools/preview_file_test.mjs
// preview_e2e.ts exercises the compiled hub and browser.
import assert from 'node:assert/strict';
import {test, after} from 'node:test';
import * as fs from 'node:fs';
import {join, resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {runInNewContext} from 'node:vm';
import {execFileSync} from 'node:child_process';

const source = fs.readFileSync(resolve('src/server/effects/host.js'), 'utf8');
function definition(name, next) {
  const start = source.indexOf(`function ${name}(`);
  const end = source.indexOf(`\nfunction ${next}(`, start);
  assert.ok(start >= 0 && end > start, name);
  return source.slice(start, end);
}
const effect = definition('sock_send_preview', 'sock_send_file');
const park = definition('host_park_out', 'host_send');
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-preview-file-'));
after(() => fs.rmSync(dir, {recursive: true, force: true}));
const path = join(dir, 'render.png');
const bytes = Buffer.alloc(12 * 1024 * 1024 + 101);
for (let i = 0; i < bytes.length; i++) bytes[i] = i % 251;
fs.writeFileSync(path, bytes);

function sender({backend = 'bun', platform = process.platform, maxWrite = Infinity,
    write, onPark, override = {}} = {}) {
  const opened = new Set(), reads = [], sent = [], queue = [];
  let now = 0, calls = 0, parks = 0, closed = 0, errno;
  const shim = {...fs, ...override,
    openSync(...args) {
      const fd = (override.openSync || fs.openSync)(...args);
      opened.add(fd); return fd;
    },
    closeSync(fd) {
      assert.ok(opened.delete(fd), 'descriptor closed exactly once'); closed++;
      return (override.closeSync || fs.closeSync)(fd);
    },
    readFileSync() { assert.fail('a preview must not be read into one file-sized buffer'); },
    readSync(...args) {
      reads.push({length: args[3], position: args[4]});
      assert.ok(args[3] <= 65536, 'bounded read');
      return (override.readSync || fs.readSync)(...args);
    },
  };
  const put = part => {
    calls++;
    const n = write ? write({calls, part, now}) : Math.min(part.length, maxWrite);
    if (n > 0) sent.push(Buffer.from(part.subarray(0, n)));
    return n;
  };
  const sandbox = {Buffer, process: {platform}, performance: {now: () => now},
    require: name => { assert.equal(name, 'fs'); return shim; },
    io_tup: (socket, result) => ({socket, result}),
    io_done: () => ({ok: true}), io_fail: errno => ({ok: false, errno}),
    host_errno: e => Math.abs(e?.errno ?? 5),
    io_sys: () => ({mac: platform === 'darwin', ptr: (b, i = 0) => b.subarray(i), errno: () => errno}),
    host_write: (_socket, part) => { const n = put(part); errno = -n; return n < 0 ? -1 : n; },
    io_park_on: (socket, out, k, go, deadline) => {
      assert.equal(socket, 73); assert.equal(out, true); assert.equal(k, continuation);
      assert.ok(deadline > now && deadline <= now + 1000);
      parks++; queue.push(go); onPark?.({parks, reads, opened});
    },
  };
  if (backend === 'bun') sandbox.BP_H = {EAGAIN: 11, is: socket => socket === 73, get: () => ({write: put})};
  const send = runInNewContext(`${park}\n${effect}\nsock_send_preview`, sandbox);
  const continuation = () => assert.fail('the effect does not own its continuation');
  return {
    opened, reads, sent, queue,
    get parks() { return parks; }, get closed() { return closed; },
    advance(ms) { now += ms; },
    start(file = path, range = '', ctype = 'image/png') { return send(73, file, ctype, range, continuation); },
    drain(result) {
      let turns = 0;
      while (result === undefined) {
        assert.ok(queue.length, 'a pending send must be parked');
        assert.ok(++turns < 100000, 'send eventually completes');
        result = queue.shift()();
      }
      assert.equal(queue.length, 0); assert.equal(opened.size, 0);
      return result;
    },
    response() {
      const all = Buffer.concat(sent), at = all.indexOf('\r\n\r\n');
      assert.ok(at > 0);
      const lines = all.subarray(0, at).toString().split('\r\n');
      return {status: Number(lines.shift().split(' ')[1]),
        headers: Object.fromEntries(lines.map(l => { const i = l.indexOf(':'); return [l.slice(0, i).toLowerCase(), l.slice(i + 1).trim()]; })),
        body: all.subarray(at + 4)};
    },
  };
}

for (const backend of ['bun', 'posix']) {
  test(`${backend}: >12 MiB exact bytes, bounded reads and a yield per chunk`, () => {
    const c = sender({backend});
    assert.equal(c.drain(c.start()).result.ok, true);
    const r = c.response();
    assert.equal(r.status, 200); assert.deepEqual(r.body, bytes);
    assert.equal(r.headers['content-length'], String(bytes.length));
    assert.equal(r.headers['content-type'], 'image/png');
    assert.equal(r.headers['accept-ranges'], 'bytes');
    assert.equal(r.headers['cache-control'], 'no-cache');
    assert.equal(r.headers['x-content-type-options'], 'nosniff');
    assert.equal(c.parks, Math.ceil(bytes.length / 65536)); assert.equal(c.closed, 1);
  });
  test(`${backend}: partial writes and repeated EAGAIN retain the unsent bytes`, () => {
    let attempts = 0;
    const c = sender({backend, write: ({part}) => ++attempts % 3 === 0
      ? -(backend === 'bun' ? 11 : (process.platform === 'darwin' ? 35 : 11)) : Math.min(part.length, 4093)});
    c.drain(c.start(path, 'bytes=65530-196701'));
    assert.deepEqual(c.response().body, bytes.subarray(65530, 196702));
    assert.ok(c.parks > 2); assert.equal(c.closed, 1);
  });
}

for (const [range, status, start, end] of [
  ['bytes=0-0', 206, 0, 1], ['bytes=65530-65540', 206, 65530, 65541],
  [`bytes=${bytes.length - 2}-`, 206, bytes.length - 2, bytes.length],
  ['bytes=-3', 206, bytes.length - 3, bytes.length],
  ['bytes=-99999999999999999999999999', 206, 0, bytes.length],
  ['bytes=1-99999999999999999999999999', 206, 1, bytes.length],
  ['bytes=10-9', 416], [`bytes=${bytes.length}-`, 416], ['bytes=-0', 416],
  ['bytes=99999999999999999999999999-', 416],
  ['bytes=0-1,4-5', 200, 0, bytes.length], ['bits=0-1', 200, 0, bytes.length],
  ['bytes=-', 200, 0, bytes.length], ['bytes=x-1', 200, 0, bytes.length],
]) test(`HTTP range ${range}`, () => {
  const c = sender(); c.drain(c.start(path, range)); const r = c.response();
  assert.equal(r.status, status);
  assert.equal(r.headers['content-length'], String(r.body.length));
  if (status === 416) assert.equal(r.headers['content-range'], `bytes */${bytes.length}`);
  else {
    assert.deepEqual(r.body, bytes.subarray(start, end));
    assert.equal(r.headers['content-range'], status === 206 ? `bytes ${start}-${end - 1}/${bytes.length}` : undefined);
  }
});

test('a range past 4 GiB uses full-width file offsets', () => {
  const file = join(dir, 'sparse.pdf'), fd = fs.openSync(file, 'w');
  const position = 2 ** 32 + 123;
  fs.writeSync(fd, Buffer.from('PDF tail'), 0, 8, position); fs.closeSync(fd);
  const c = sender(); c.drain(c.start(file, `bytes=${position}-`, 'application/pdf'));
  const r = c.response();
  assert.equal(r.status, 206); assert.equal(r.body.toString(), 'PDF tail');
  assert.equal(c.reads[0].position, position);
  assert.equal(r.headers['content-range'], `bytes ${position}-${position + 7}/${position + 8}`);
});

test('empty files have a complete response; their byte ranges are unsatisfiable', () => {
  const file = join(dir, 'empty.txt'); fs.writeFileSync(file, '');
  for (const range of ['', 'bytes=0-', 'bytes=-1']) {
    const c = sender(); c.drain(c.start(file, range, 'text/plain; charset=utf-8'));
    const r = c.response(); assert.equal(r.status, range ? 416 : 200);
    if (!range) assert.equal(r.body.length, 0);
    assert.equal(c.reads.length, 0); assert.equal(c.closed, 1);
  }
});

test('current no-follow policy is retained; directories and missing paths fail', () => {
  const file = join(dir, 'link.png'); fs.symlinkSync(path, file);
  const good = sender(); good.drain(good.start(file, 'bytes=0-7'));
  if (fs.constants.O_NOFOLLOW) {
    assert.equal(good.response().status, 500);
    assert.equal(good.reads.length, 0);
  } else {
    assert.deepEqual(good.response().body, bytes.subarray(0, 8));
  }
  for (const invalid of [dir, join(dir, 'missing.png'), join(dir, 'missing', 'child.png')]) {
    const c = sender(); c.drain(c.start(invalid)); assert.equal(c.response().status, 404);
  }
  const nul = sender(); nul.drain(nul.start(path + '\0suffix')); assert.equal(nul.response().status, 400);
  const injection = sender(); injection.drain(injection.start(path, '', 'image/png\r\nInjected: yes'));
  assert.equal(injection.response().status, 400);
});

for (const [code, status] of [['EACCES', 403], ['EPERM', 403], ['EIO', 500]]) {
  test(`open ${code} returns a useful HTTP status without leaking OS details`, () => {
    const c = sender({override: {openSync() { throw {code, errno: -5, message: 'private path'}; }}});
    c.drain(c.start()); const r = c.response(); assert.equal(r.status, status);
    assert.ok(!r.body.includes(Buffer.from('private path'))); assert.equal(c.reads.length, 0);
  });
}

test('a FIFO cannot block open or masquerade as a regular preview', {skip: process.platform === 'win32'}, () => {
  const file = join(dir, 'pipe.png'); execFileSync('mkfifo', [file]);
  const c = sender({override: {openSync(path, flags) {
    // Fail before opening if a regression would otherwise block this test.
    assert.ok(flags & fs.constants.O_NONBLOCK);
    return fs.openSync(path, flags);
  }}});
  c.drain(c.start(file)); assert.equal(c.response().status, 404);
  assert.equal(c.reads.length, 0); assert.equal(c.closed, 1);
});

test('metadata failures close the descriptor before returning an error', () => {
  for (const fstatSync of [() => { throw {errno: -5}; }, () => ({isFile: () => true, size: 2 ** 54})]) {
    const c = sender({override: {fstatSync}}); c.drain(c.start());
    assert.equal(c.response().status, 500); assert.equal(c.closed, 1);
  }
});

test('atomic replacement does not switch the open descriptor; appends do not extend the response', () => {
  for (const replace of [true, false]) {
    const file = join(dir, `changing-${replace}.png`); fs.writeFileSync(file, bytes);
    const c = sender({onPark({parks}) {
      if (parks !== 1) return;
      if (replace) { fs.renameSync(file, file + '.old'); fs.writeFileSync(file, 'new file'); }
      else fs.appendFileSync(file, 'new tail');
    }});
    c.drain(c.start(file)); assert.deepEqual(c.response().body, bytes);
  }
});

test('truncation and read errors close the transfer without injecting an error body', () => {
  for (const readSync of [() => 0, () => { throw {errno: -5}; }]) {
    const c = sender({override: {readSync}}); assert.equal(c.drain(c.start()).result.errno, 5);
    assert.equal(c.response().body.length, 0); assert.equal(c.closed, 1);
  }
});

for (const backend of ['bun', 'posix']) test(`${backend}: disconnect closes the file without reading it all`, () => {
  const c = sender({backend, write: ({calls, part}) => calls === 1 ? part.length : -32});
  assert.equal(c.drain(c.start()).result.errno, 32);
  assert.equal(c.reads.length, 1); assert.equal(c.closed, 1);
});

test('stalled and zero-byte writes expire; progress renews the idle deadline', () => {
  for (const code of [0, -11]) {
    const c = sender({write: () => code, onPark: () => c.advance(1000)});
    assert.equal(c.drain(c.start()).result.errno, process.platform === 'darwin' ? 60 : 110);
    assert.equal(c.closed, 1); assert.equal(c.reads.length, 0);
  }
  const c = sender({maxWrite: 64, onPark: () => c.advance(29000)});
  c.drain(c.start(path, 'bytes=0-131071')); assert.equal(c.response().body.length, 131072);
});

test('Bun handles retain their errno namespace on Darwin', () => {
  const c = sender({platform: 'darwin', write: ({calls, part}) => calls % 3 === 1 ? -11 : Math.min(part.length, 4093)});
  c.drain(c.start(path, 'bytes=0-131071'));
  assert.equal(c.response().body.length, 131072);
  assert.equal(c.closed, 1);
});

test('Windows handle path needs no POSIX syscall interface', () => {
  const c = sender({platform: 'win32'}); c.drain(c.start(path, 'bytes=0-7'));
  assert.deepEqual(c.response().body, bytes.subarray(0, 8));
});
