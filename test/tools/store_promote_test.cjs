// Real SQLite WALs in disposable homes. PERSIST_WAL reproduces macOS's
// close behaviour on Linux too; no hub or live profile is opened.
// Optional: bun test/tools/store_promote_test.cjs /path/to/store-up-probe
// also exercises Sto.boot twice on fresh and stranded-sidecar homes, using
// test/native/store_up_probe.bend built for this platform.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const vm = require('node:vm'), assert = require('node:assert/strict'), crypto = require('node:crypto');
const { Database } = require('bun:sqlite');
const root = path.resolve(__dirname, '../..');
const source = fs.readFileSync(process.env.STOREIO_SOURCE || path.join(root, 'src/server/effects/storeio.js'), 'utf8');
const dbSource = fs.readFileSync(path.join(root, 'src/server/effects/db.js'), 'utf8');
const storedb = fs.readFileSync(path.join(root, 'src/core/storedb.bend'), 'utf8');
const codec = /def Sdb\.codec\(\) -> Nat:\s*(\d+)n/.exec(storedb)[1];
const schemaSource = storedb.split('def Sdb.schema() -> String:')[1].split('# a table')[0];
const schema = [...schemaSource.matchAll(/"(?:[^"\\]|\\.)*"/g)].map(m => JSON.parse(m[0])).filter(s => s.startsWith('CREATE ')).join(';');
const suffixes = ['', '-wal', '-shm'];
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function effects(io = fs) {
  const c = { require: n => n === 'fs' ? io : require(n), Buffer, process, console: { warn() {} },
    io_eff() {}, CID: x => x, Nil: 'Nil', Con: 'Con', Disk: {}, Db: { run: {}, batch: {}, script: {}, query: {} } };
  vm.createContext(c);
  vm.runInContext(source + '\n' + dbSource, c);
  return c;
}
function open(c, file) {
  const opened = c.db_open(file);
  assert.match(opened, /^ok /);
  const h = opened.slice(3);
  const d = c.bp_db_get(h);
  try {
    // Batch disposable fixture setup; exercise the real db_exec and retain its PRAGMAs.
    d.transaction(() => {
      assert.equal(c.db_exec(h, schema), 'ok');
      for (const [k, v] of Object.entries({ schema: '1', codec, applied_seq: '0', applied_off: '0', ord: '1' }))
        d.query('INSERT INTO meta VALUES (?,?)').run(k, v);
    })();
  } catch (e) {
    assert.equal(c.db_close(h), 'ok');
    throw e;
  }
  return h;
}
function read(file) {
  const d = new Database(file, { readonly: true });
  try { return d.query("SELECT v FROM setting WHERE k='test.marker'").get()?.v; }
  finally { d.close(); }
}
function build(file, marker, mode) {
  const c = effects();
  // For WAL-only, copy a quiescent committed SQLite pair BEFORE closing its
  // writer (closing would checkpoint it). Promotion itself has no open handle.
  const seed = mode === 'wal-only' ? file + '.seed' : file;
  const h = open(c, seed), d = c.bp_db_get(h);
  try {
    // Bun on macOS requires a pointer-sized buffer; SQLite reads an int.
    assert.equal(d.fileControl(10, new Int32Array([1, 0])), 0); // SQLITE_FCNTL_PERSIST_WAL
    if (mode === 'wal-only') d.exec('PRAGMA wal_checkpoint(TRUNCATE)');
    d.query('INSERT INTO setting VALUES (?, ?, 0)').run('test.marker', marker);
    if (mode === 'no-sidecars') d.exec('PRAGMA journal_mode=DELETE');
    if (mode === 'empty-wal') d.exec('PRAGMA wal_checkpoint(TRUNCATE)');
    if (mode === 'wal-only') {
      for (const s of suffixes) fs.copyFileSync(seed + s, file + s);
      const bare = file + '.bare';
      fs.copyFileSync(file, bare);
      // A writable open can recreate missing sidecars on macOS; it still
      // cannot recover the row without the original committed WAL.
      const isolated = new Database(bare, { readwrite: true });
      try { assert.equal(isolated.query("SELECT v FROM setting WHERE k='test.marker'").get(), null, 'the committed marker exists only in the WAL'); }
      finally { isolated.close(); }
      for (const s of suffixes) fs.rmSync(bare + s, { force: true });
    }
  } finally { assert.equal(c.db_close(h), 'ok'); }
  if (mode === 'wal-only') for (const s of suffixes) fs.unlinkSync(seed + s);
  if (mode === 'no-sidecars') for (const s of suffixes.slice(1)) fs.rmSync(file + s, { force: true });
  else {
    assert(fs.existsSync(file + '-shm'));
    if (mode === 'empty-wal') assert.equal(fs.statSync(file + '-wal').size, 0);
    if (mode === 'wal-only') assert(fs.statSync(file + '-wal').size > 0);
  }
}
let count = 0, failures = 0;
function test(name, run) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-promote-日本語 space-'));
  try {
    fs.writeFileSync(path.join(home, 'events.jsonl'), '');
    run(home);
    count++;
    console.log('ok ' + name);
  } catch (e) { failures++; console.error('FAIL ' + name + ': ' + e.stack); }
  finally { fs.rmSync(home, { recursive: true, force: true }); }
}
for (const replacing of [false, true]) for (const mode of ['persistent', 'empty-wal', 'wal-only']) {
  test((replacing ? 'replace' : 'fresh') + ' store with ' + mode, home => {
    const c = effects(), target = path.join(home, 'store.db'), log = path.join(home, 'events.jsonl');
    if (replacing) build(target, 'old', 'wal-only');
    const old = replacing ? suffixes.map(s => hash(target + s)) : [];
    const temp = c.store_temp(home), next = path.join(temp, 'store.db');
    build(next, 'new', mode);
    const staged = suffixes.map(s => hash(next + s));
    const trace = [], opened = new Map(), io = Object.create(fs);
    io.openSync = (file, ...args) => { const fd = fs.openSync(file, ...args); opened.set(fd, file); return fd; };
    io.closeSync = fd => { opened.delete(fd); return fs.closeSync(fd); };
    io.fsyncSync = fd => { trace.push(['sync', opened.get(fd)]); return fs.fsyncSync(fd); };
    io.renameSync = (a, b) => { trace.push(['rename', a, b]); return fs.renameSync(a, b); };
    assert.equal(effects(io).store_promote(home + path.sep, temp), 'ok');
    const firstMove = trace.findIndex(t => t[0] === 'rename');
    for (const s of ['', '-wal']) assert(trace.slice(0, firstMove).some(t => t[0] === 'sync' && t[1] === next + s), 'sync the database and WAL before moving any current file');
    assert(!fs.existsSync(temp), 'no ENOTEMPTY residue');
    for (const [i, s] of suffixes.entries()) assert.equal(hash(target + s), staged[i], 'move the complete built file set');
    if (replacing) {
      const backups = fs.readdirSync(home).filter(n => /^store\.db\.previous-\d+$/.test(n));
      assert.equal(backups.length, 1);
      const backup = path.join(home, backups[0]);
      for (const [i, s] of suffixes.entries()) assert.equal(hash(backup + s), old[i]);
      assert.equal(read(backup), 'old', 'old WAL-backed snapshot is recoverable');
    }
    assert.equal(c.store_validate(target, codec).codec, codec);
    assert.equal(read(target), 'new', 'read-only boot sees committed WAL rows');
    for (let i = 0; i < 2; i++) {
      assert.equal(c.store_prepare(home, codec, ''), 'ok');
      assert.equal(read(target), 'new');
      assert(!fs.readdirSync(home).some(n => n.startsWith('store.db.invalid-')), 'next boot keeps the store');
    }
    assert.equal(fs.readFileSync(log, 'utf8'), '');
  });
}
test('single-file store still promotes', home => {
  const c = effects(), temp = c.store_temp(home);
  build(path.join(temp, 'store.db'), 'single', 'no-sidecars');
  assert.equal(c.store_promote(home, temp), 'ok');
  assert.equal(read(path.join(home, 'store.db')), 'single');
  assert(!fs.existsSync(temp));
});
test('WAL is durable before any current store is moved', home => {
  const c = effects(), target = path.join(home, 'store.db');
  build(target, 'old', 'wal-only');
  const old = suffixes.map(s => hash(target + s)), temp = c.store_temp(home), next = path.join(temp, 'store.db');
  build(next, 'new', 'wal-only');
  const trace = [], opened = new Map(), io = Object.create(fs);
  io.openSync = (file, ...args) => { const fd = fs.openSync(file, ...args); opened.set(fd, file); return fd; };
  io.closeSync = fd => { opened.delete(fd); return fs.closeSync(fd); };
  io.fsyncSync = fd => {
    const file = opened.get(fd); trace.push(['sync', file]);
    if (file === next + '-wal') throw Error('injected WAL sync failure');
    return fs.fsyncSync(fd);
  };
  io.renameSync = (a, b) => { trace.push(['rename', a, b]); return fs.renameSync(a, b); };
  assert.match(effects(io).store_promote(home, temp), /^fail injected WAL sync failure/);
  assert(!trace.some(t => t[0] === 'rename'));
  for (const [i, s] of suffixes.entries()) assert.equal(hash(target + s), old[i]);
  assert.equal(read(next), 'new');
  // Retrying after the flush error promotes the same complete file set.
  assert.equal(c.store_promote(home, temp), 'ok');
  assert.equal(read(target), 'new');
});
for (const kind of ['unexpected-file', 'sidecar-directory', 'sidecar-symlink']) test('reject ' + kind + ' before moving files', home => {
  const c = effects(), temp = c.store_temp(home), next = path.join(temp, 'store.db');
  build(next, 'new', 'persistent');
  if (kind === 'unexpected-file') fs.writeFileSync(path.join(temp, 'keep.me'), 'evidence');
  else {
    fs.unlinkSync(next + '-shm');
    if (kind === 'sidecar-directory') fs.mkdirSync(next + '-shm');
    else fs.symlinkSync(path.join(home, 'events.jsonl'), next + '-shm');
  }
  const before = hash(next + '-wal');
  assert.match(c.store_promote(home, temp), /^fail /);
  assert(!fs.existsSync(path.join(home, 'store.db')));
  assert.equal(hash(next + '-wal'), before);
});
const probe = process.argv[2] && path.resolve(process.argv[2]);
if (probe) for (const stranded of [false, true]) test('native store boot twice: ' + (stranded ? 'stranded sidecars' : 'fresh home'), home => {
  const c = effects(), target = path.join(home, 'store.db');
  const log = path.join(home, 'events.jsonl');
  const event = JSON.stringify({ $: 'SettingSet', key: 'test.marker', value: 'from-log', at: 1 }) + '\n';
  fs.writeFileSync(log, event);
  let residue, evidence;
  if (stranded) {
    residue = c.store_temp(home);
    const built = path.join(residue, 'store.db');
    build(built, 'old-cache', 'persistent');
    evidence = suffixes.slice(1).map(s => hash(built + s));
    // The original failing promotion moved only the main file.
    fs.renameSync(built, target);
  }
  let inode;
  for (let i = 0; i < 2; i++) {
    const result = require('node:child_process').spawnSync(probe, [home], {
      encoding: 'utf8', timeout: 30000,
      env: { ...process.env, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: 'off' }
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    assert.match(result.stdout, /probe: ok/);
    assert.equal(c.store_validate(target, codec).codec, codec);
    assert.equal(read(target), 'from-log');
    const now = fs.statSync(target).ino;
    if (i) assert.equal(now, inode, 'second boot keeps the promoted database');
    inode = now;
    assert.equal(fs.readFileSync(log, 'utf8'), event);
    assert.deepEqual(fs.readdirSync(home).filter(n => n.startsWith('.store-rebuild-')), residue ? [path.basename(residue)] : []);
  }
  if (stranded) for (const [i, s] of suffixes.slice(1).entries()) assert.equal(hash(path.join(residue, 'store.db') + s), evidence[i], 'retain old stranded WAL evidence');
});
console.log(JSON.stringify({ platform: process.platform, arch: process.arch, bun: Bun.version, passed: count, failed: failures }));
if (failures) process.exitCode = 1;
