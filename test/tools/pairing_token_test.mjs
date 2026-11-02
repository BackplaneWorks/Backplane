// Actual host effect; synthetic credentials in a private temporary directory.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const require = createRequire(import.meta.url);
const source = fs.readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-pairing-token-'));
const fresh = '00000000000000000000000000000001';
const other = '00000000000000000000000000000002';
let assertions = 0;
const check = (value, message) => { assert.ok(value, message); assertions++; };
function effect(overrides = {}, platform = process.platform, random = size => { assert.equal(size, 16); return Buffer.from(fresh, 'hex'); }) {
  const effects = new Map();
  const shim = { ...fs, linkSync() { throw new Error('hard links must not be used'); }, ...overrides };
  new Function('require', 'process', 'io_eff', 'io_done', 'io_fail', 'io_tup', source)(
    name => name === 'fs' ? shim : name === 'crypto' ? { randomBytes: random } : require(name), { platform },
    (id, fn) => effects.set(id, fn), value => ({ ok: true, value }), error => ({ ok: false, error }), (a, b) => [a, b]);
  return effects.get('FS.pairing_token');
}
const fail = code => Object.assign(new Error('synthetic filesystem failure'), { code });
try {
  const token = join(dir, 'token');
  const ensure = effect({ writeFileSync(fd, value, ...rest) {
    if (value === fresh && process.platform !== 'win32') check((fs.fstatSync(fd).mode & 0o777) === 0o600, 'private before first credential write');
    return fs.writeFileSync(fd, value, ...rest);
  } });
  check(ensure(token).value === fresh, 'first initialization succeeds without hard links');
  check(ensure(token).value === fresh, 'second initialization preserves the credential');
  check(fs.readFileSync(token, 'utf8') === fresh, 'file bytes unchanged');
  check(effect({}, process.platform, () => { throw new Error('RNG unavailable'); })(token).value === fresh, 'existing credential startup needs no RNG');
  check(!fs.existsSync(token + '.backplane-creating'), 'successful creation cleans its marker');

  const legacy = join(dir, 'legacy');
  fs.writeFileSync(legacy, ` ${fresh}\n`, { mode: 0o644 });
  check(ensure(legacy).value === ` ${fresh}\n`, 'effect preserves legacy whitespace');
  check(fs.readFileSync(legacy, 'utf8') === ` ${fresh}\n`, 'no file rewrite on legacy startup');
  if (process.platform !== 'win32') check((fs.statSync(legacy).mode & 0o777) === 0o600, 'existing file permissions restricted');
  for (const [name, contents] of [['empty', ''], ['whitespace', ' \n']]) {
    const p = join(dir, name); fs.writeFileSync(p, contents);
    check(!ensure(p).ok, 'empty existing credential fails');
    check(fs.readFileSync(p, 'utf8') === contents, 'empty credential never replaced');
  }
  check(!ensure(dir).ok, 'directory credential fails without blocking');
  if (process.platform !== 'win32') {
    const linked = join(dir, 'linked'), dangling = join(dir, 'dangling');
    fs.symlinkSync(token, linked); fs.symlinkSync(join(dir, 'not-present'), dangling);
    check(ensure(linked).value === fresh && fs.lstatSync(linked).isSymbolicLink(), 'existing symlink supported');
    check(!ensure(dangling).ok && fs.lstatSync(dangling).isSymbolicLink(), 'dangling symlink preserved and refused');
  }
  const unreadable = effect({ openSync(p, ...args) { if (p === token) throw fail('EACCES'); return fs.openSync(p, ...args); } });
  check(!unreadable(token).ok, 'read failure is not treated as missing');
  check(fs.readFileSync(token, 'utf8') === fresh, 'read failure preserves credential');
  if (process.platform !== 'win32') check(!effect({ fchmodSync() { throw fail('EPERM'); } })(token).ok, 'chmod failure is explicit');

  const partial = join(dir, 'partial');
  const interrupted = effect({ writeFileSync(fd, value, ...rest) {
    if (value === fresh) { fs.writeSync(fd, fresh.slice(0, 8)); throw fail('EIO'); }
    return fs.writeFileSync(fd, value, ...rest);
  } });
  check(!interrupted(partial).ok, 'partial first write fails');
  check(fs.existsSync(partial + '.backplane-creating'), 'partial first write retains recovery marker');
  check(!ensure(partial).ok, 'restart refuses incomplete creation');
  check(fs.readFileSync(partial, 'utf8') === fresh.slice(0, 8), 'partial bytes preserved for recovery');

  const race = join(dir, 'race');
  const competing = effect({ openSync(p, flags, ...rest) {
    if (p === race && flags === 'wx') { fs.writeFileSync(race, other, { flag: 'wx', mode: 0o600 }); throw fail('EEXIST'); }
    return fs.openSync(p, flags, ...rest);
  } });
  check(!competing(race).ok, 'competing creator fails explicitly');
  check(!fs.existsSync(race + '.backplane-creating'), 'only own unused marker removed');
  check(fs.readFileSync(race, 'utf8') === other && ensure(race).value === other, 'competing credential never replaced');

  const win = effect({ fchmodSync() { throw new Error('Windows must not use mode bits as an ACL'); } }, 'win32');
  const winToken = join(dir, 'win-model');
  check(win(winToken).value === fresh && win(winToken).value === fresh, 'Windows-model create and reload');
  check(!win(partial).ok, 'Windows-model interrupted creation fails closed');
  console.log(`ok ${assertions} pairing-token filesystem contracts (Windows model is not native ACL qualification)`);
} finally { fs.rmSync(dir, { recursive: true, force: true }); }
