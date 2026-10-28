import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { createHmac } from 'node:crypto';
const require = createRequire(import.meta.url);
const root = path.resolve(import.meta.dirname, '../..');
const source = fs.readFileSync(path.join(root, 'src/server/effects/host.js'), 'utf8');
const start = source.indexOf('function fs_mkdtemp(prefix) {');
assert.ok(start >= 0);
const body = source.slice(start, source.indexOf('\nfunction ', start + 1));
assert.match(source, /io_eff\(CID\(FS\.mkdtemp\), fs_mkdtemp\)/);
assert.match(fs.readFileSync(path.join(root, 'src/server/push.bend'), 'utf8'), /H\.FS\.mkdtemp\(/);
const load = req => vm.runInNewContext(body + '\nfs_mkdtemp', {
  require: req, io_done: value => ({ok: true, value}),
  io_fail: code => ({ok: false, code}), host_errno: e => Math.abs(e.errno ?? 5),
});
const make = load(require);
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-hook-files-'));
try {
  const prefix = path.join(scratch, 'same-event-');
  const a = make(prefix), b = make(prefix);
  assert.equal(a.ok, true); assert.equal(b.ok, true);
  assert.notEqual(a.value, b.value);
  if (process.platform !== 'win32') {
    assert.equal(fs.statSync(a.value).mode & 0o777, 0o700);
    assert.equal(fs.statSync(b.value).mode & 0o777, 0o700);
  }
  const entries = [[a.value, 'first'], [b.value, 'second']];
  for (const [dir, text] of entries) {
    const payload = JSON.stringify({event: 'note', text});
    const sig = createHmac('sha256', 'synthetic-review-key').update('10.' + payload).digest('hex');
    fs.writeFileSync(path.join(dir, 'body.json'), payload, {mode: 0o600});
    fs.writeFileSync(path.join(dir, 'curl'), sig, {mode: 0o600});
  }
  // Interleave reading both jobs, then clean only the first job.
  for (const [dir] of entries) assert.equal(
    fs.readFileSync(path.join(dir, 'curl'), 'utf8'),
    createHmac('sha256', 'synthetic-review-key').update('10.' + fs.readFileSync(path.join(dir, 'body.json'), 'utf8')).digest('hex'));
  fs.rmSync(a.value, {recursive: true});
  assert.equal(JSON.parse(fs.readFileSync(path.join(b.value, 'body.json'), 'utf8')).text, 'second');
  const broken = load(() => ({mkdtempSync() { const e = new Error('synthetic full disk'); e.errno = -28; throw e; }}));
  assert.deepEqual(broken(prefix), {ok: false, code: 28});
  assert.deepEqual(fs.readdirSync(scratch), [path.basename(b.value)]);
  console.log('ok exclusive private directories, independent bodies/HMACs, isolated cleanup and allocation failure');
} finally { fs.rmSync(scratch, {recursive: true, force: true}); }
