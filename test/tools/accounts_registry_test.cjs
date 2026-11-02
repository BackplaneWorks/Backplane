// Platform-simulated registry reloads. No provider CLI or live home is used.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const {randomUUID} = require('node:crypto');
const {test} = require('node:test');

const source = fs.readFileSync(path.join(__dirname, '../../src/server/effects/accounts.js'), 'utf8')
  .replace(/CID\(([^)]+)\)/g, (_, value) => JSON.stringify(value));

function actor(platform, report = {}) {
  const simulated = Object.create(process);
  Object.defineProperty(simulated, 'platform', {value: platform});
  if (platform === 'win32') Object.defineProperty(simulated, 'getuid', {value: undefined});
  const filesystem = Object.create(fs);
  filesystem.lstatSync = filename => {
    const actual = fs.lstatSync(filename);
    if (path.basename(filename) !== 'profiles.json') return actual;
    return new Proxy(actual, {get(target, key) {
      if (key === 'mode' && report.mode !== undefined) return report.mode;
      if (key === 'isFile' && report.regular === false) return () => false;
      if (key === 'isSymbolicLink' && report.link) return () => true;
      const value = Reflect.get(target, key);
      return typeof value === 'function' ? value.bind(target) : value;
    }});
  };
  const context = vm.createContext({
    require(name) {
      if (name === 'node:fs') return filesystem;
      if (name === 'node:child_process') throw Error('This test must never launch a provider');
      return require(name);
    },
    process: simulated, Buffer, URL, setTimeout, clearTimeout, io_eff() {},
  });
  return vm.runInContext(source + '\nBPAccountsIO', context);
}

function fixture(t, extra = {}) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-accounts-registry-'));
  t.after(() => fs.rmSync(home, {recursive: true, force: true}));
  const folder = path.join(home, 'providers', 'accounts-v1');
  fs.mkdirSync(folder, {recursive: true, mode: 0o700});
  const profile = {id: randomUUID(), provider: 'claude', label: 'Saved Windows account',
    epoch: 2, fingerprint: 'fixture-identity-digest', store: randomUUID()};
  const index = path.join(folder, 'profiles.json');
  fs.writeFileSync(index, JSON.stringify({version: 1, profiles: [profile], ...extra}), {mode: 0o600});
  const store = path.join(folder, profile.id, profile.store);
  fs.mkdirSync(store, {recursive: true, mode: 0o700});
  fs.writeFileSync(path.join(store, 'preserve.fixture'), 'fixture data stays unchanged');
  return {home, folder, profile, index, store};
}

test('Windows reload accepts Bun synthetic 0666 without changing saved account or store', t => {
  const f = fixture(t), before = fs.readFileSync(f.index, 'utf8');
  const Accounts = actor('win32', {mode: 0o100666});
  const first = new Accounts(f.home), second = new Accounts(f.home);
  assert.equal(first.loadError, undefined);
  assert.equal(second.loadError, undefined);
  const saved = second.profile(f.profile.id);
  assert.equal(saved.state, 'unchecked');
  assert.equal(saved.epoch, f.profile.epoch);
  assert.equal(saved.store, f.profile.store);
  assert.equal(saved.fingerprint, f.profile.fingerprint);
  assert.equal(fs.readFileSync(f.index, 'utf8'), before);
  assert.equal(fs.readFileSync(path.join(f.store, 'preserve.fixture'), 'utf8'), 'fixture data stays unchanged');
});

for (const platform of ['linux', 'darwin']) {
  test(platform + ' still rejects a group/world-readable registry', async t => {
    const f = fixture(t), Accounts = actor(platform, {mode: 0o100666});
    const manager = new Accounts(f.home);
    assert.equal(manager.loadError, 'The private account registry could not be verified.');
    await assert.rejects(manager.request({op: 'list'}), /could not be verified/);
  });
  test(platform + ' still reloads an owner-only registry', t => {
    const f = fixture(t), Accounts = actor(platform, {mode: 0o100600});
    assert.equal(new Accounts(f.home).profile(f.profile.id).epoch, 2);
  });
}

for (const [name, report] of [['symbolic link', {link: true}], ['non-file', {regular: false}]]) {
  test('Windows still rejects a registry reported as a ' + name, t => {
    const f = fixture(t), Accounts = actor('win32', {mode: 0o100666, ...report});
    assert.equal(new Accounts(f.home).loadError, 'The private account registry could not be verified.');
  });
}

test('Windows still rejects oversized valid JSON metadata', t => {
  const f = fixture(t, {padding: 'x'.repeat(131072)});
  const Accounts = actor('win32', {mode: 0o100666});
  assert.equal(new Accounts(f.home).loadError, 'The private account registry could not be verified.');
});

test('Windows still rejects an invalid saved identity generation', t => {
  const f = fixture(t);
  fs.writeFileSync(f.index, JSON.stringify({version: 1, profiles: [{...f.profile, epoch: 0}]}));
  const Accounts = actor('win32', {mode: 0o100666});
  assert.equal(new Accounts(f.home).loadError, 'The private account registry could not be verified.');
});

test('Windows can remove an existing profile and reload the empty registry', async t => {
  const f = fixture(t), Accounts = actor('win32', {mode: 0o100666});
  const manager = new Accounts(f.home);
  await manager.request({op: 'remove', profile: f.profile.id});
  assert.equal(manager.snapshot().profiles.length, 0);
  assert.equal(fs.existsSync(path.join(f.folder, f.profile.id)), false);
  const restarted = new Accounts(f.home);
  assert.equal(restarted.loadError, undefined);
  assert.equal(restarted.snapshot().profiles.length, 0);
});
