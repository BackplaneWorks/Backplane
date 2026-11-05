// The Files tab's zip writer (src/server/effects/zip.js, loaded in a scope
// of its own as Bend loads it) against core/fm.bend, from the emitted
// test/files_zip_test.bend: the same decisions as Fm.zip.skip, within,
// store and entry over many generated paths, and real zips made with the
// plan the hub makes (server.bend's FilesHttp.zip), through a socket that
// takes partial writes. Each zip is checked by unzip -t, Python's zipfile
// and, on macOS, ditto (Archive Utility's unpacker), unpacked and compared
// with the folder less what the rules leave out. Prints "ok ..." / "FAIL ...".
//
//   bend test/files_zip_test.bend -o build/files_zip.mjs && bun test/tools/files_zip.mjs build/files_zip.mjs
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as zlib from 'node:zlib';
import * as pathm from 'node:path';
import * as os from 'node:os';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { createContext, runInContext } from 'node:vm';
import { execFileSync } from 'node:child_process';

const F = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const source = fs.readFileSync(resolve('src/server/effects/zip.js'), 'utf8');
const list = xs => { const out = []; for (; xs && xs.$ === 'Con'; xs = xs.tail) out.push(xs.head); return out; };
const bendList = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
const RULES = list(F.rules()), STORED = list(F.stored()), WHY = list(F.why());

let failed = 0;
async function t(name, opts, f) {
  if (typeof opts === 'function') { f = opts; opts = {}; }
  if (opts.skip) { console.log('ok ' + name + ' (skipped: ' + opts.why + ')'); return; }
  try { await f(); console.log('ok ' + name); }
  catch (e) { failed++; console.log('FAIL ' + name + ': ' + String(e?.stack ?? e).slice(0, 1500)); }
}

// zip.js in a context of its own, with a Bun-loop socket handle that takes
// partial writes and is full now and then; limit/count lower zip64's start
function load({ limit, count, step, cancelAfter = Infinity, disconnectBeforeHeaders = false } = {}) {
  let text = source;
  if (limit) text = text.replace('const ZIP_LIMIT = 0xFFFFFFFF;', `const ZIP_LIMIT = ${limit};`);
  if (count) text = text.replace('const ZIP_COUNT = 0xFFFF;', `const ZIP_COUNT = ${count};`);
  if (step) text = text.replace('const ZIP_STEP = 256;', `const ZIP_STEP = ${step};`);
  const sent = [], queue = [], effects = new Map();
  let calls = 0, now = 0;
  const write = b => {
    calls++;
    if (calls > cancelAfter) return -32;
    if (calls % 7 === 0) return -11;
    const n = Math.min(b.length, 1 + (calls * 7919) % 40000);
    sent.push(Buffer.from(b.subarray(0, n)));
    return n;
  };
  const ctx = createContext({ Buffer, BigInt, process, performance: { now: () => now }, Date, Error, Object, JSON,
    require: name => ({ fs, zlib, path: pathm, os })[name],
    io_tup: (socket, result) => ({ socket, result }), io_done: () => ({ ok: true }), io_fail: errno => ({ ok: false, errno }),
    io_park_on: (_s, _o, _k, go) => queue.push(go),
    io_sys: () => assert.fail('a Bun-loop handle needs no descriptor'),
    io_eff: (name, fn) => effects.set(name, fn),
    BP_H: { EAGAIN: 11, is: s => s === 73, get: () => ({ write, gone: disconnectBeforeHeaders && now > 0 }) } });
  runInContext('(function(){' + text.replace(/CID\(([\w.]+)\)/g, (_, n) => JSON.stringify(n)) + '})();', ctx);
  assert.ok(effects.has('Zip.send') && effects.has('Zip.inside'), 'both effects registered');
  const drain = r => {
    let turns = 0, parks = 0;
    while (r === undefined) {
      assert.ok(queue.length, 'a pending send is parked');
      assert.ok(++turns < 1e7);
      now += 1; parks++;
      r = queue.shift()();
    }
    return { r, parks };
  };
  return { Z: ctx.BP_ZIP, effects, sent, drain };
}

function response(bufs) {
  const b = Buffer.concat(bufs);
  const at = b.indexOf('\r\n\r\n');
  const head = b.subarray(0, at).toString();
  let rest = b.subarray(at + 4);
  if (!/Transfer-Encoding: chunked/.test(head)) return { head, body: rest, ended: true };
  const parts = [];
  let ended = false;
  for (;;) {
    const nl = rest.indexOf('\r\n');
    if (nl < 0) break;
    const n = parseInt(rest.subarray(0, nl).toString(), 16);
    if (n === 0) { ended = true; break; }
    parts.push(rest.subarray(nl + 2, nl + 2 + n));
    rest = rest.subarray(nl + 2 + n + 2);
  }
  return { head, body: Buffer.concat(parts), ended };
}

const dir = fs.mkdtempSync(join(tmpdir(), 'bp-files-zip-'));

function project() {
  const root = fs.mkdtempSync(join(dir, 'proj-'));
  const put = (p, data) => { fs.mkdirSync(pathm.dirname(join(root, p)), { recursive: true }); fs.writeFileSync(join(root, p), data); };
  put('boards/ecc83-pp.kicad_pcb', '(kicad_pcb (version 20240108))\n'.repeat(2000));
  put('boards/ecc83-pp.kicad_sch', '(kicad_sch)\n');
  put('boards/ecc83-backups/old.zip', 'old');
  put('boards/~ecc83-pp.kicad_sch.lck', 'lock');
  put('boards/_autosave-ecc83-pp.kicad_pcb', 'auto');
  put('docs/schéma notes – v2.md', '# Notes\n');
  put('docs/README.md', 'readme\n');
  const random = Buffer.alloc(1536 * 1024 + 77);
  let x = 12345;
  for (let i = 0; i < random.length; i++) { x = (x * 1103515245 + 12345) >>> 0; random[i] = x >>> 24; }
  put('mfg/big.bin', random);
  put('mfg/photo.png', random.subarray(0, 70000));
  put('mfg/gerbers.gbr', 'G04 gerber*\n'.repeat(30000));
  put('mfg/empty.txt', '');
  fs.mkdirSync(join(root, 'empty/inner'), { recursive: true });
  put('.git/HEAD', 'ref: refs/heads/main\n');
  put('.backplane/history.git/HEAD', 'x');
  put('.backplane/checklist.md', '- [ ] ERC\n');
  put('.backplane/worktrees/backplane-1/board.kicad_pcb', 'copy');
  put('tools/node_modules/a/index.js', 'x');
  put('.DS_Store', 'x');
  put('.env', 'TOKEN=secret');
  put('.env.local', 'TOKEN=secret');
  put('.backplane.json', '{}');
  put('.gitignore', 'build/\n');
  put('.github/workflows/ci.yml', 'on: push\n');
  fs.symlinkSync('/etc', join(root, 'docs/etc-link'));
  fs.symlinkSync(join(root, 'docs/README.md'), join(root, 'docs/readme-link.md'));
  fs.symlinkSync(join(root, '.backplane'), join(root, 'bp-link'));
  fs.symlinkSync(join(root, '.backplane/history.git'), join(root, 'history-link'));
  return root;
}

// the folder as its zip holds it, by Fm.zip.skip (the Bend def itself)
function expected(root, rel) {
  const out = new Map();
  const walk = (abs, proj, inside) => {
    for (const n of fs.readdirSync(abs)) {
      const p = proj ? proj + '/' + n : n;
      if (F.skip(p)) continue;
      const st = fs.lstatSync(join(abs, n));
      const q = inside ? inside + '/' + F.entry(n) : F.entry(n);
      if (st.isSymbolicLink()) continue;
      if (st.isDirectory()) { out.set(q + '/', null); walk(join(abs, n), p, q); }
      else out.set(q, fs.readFileSync(join(abs, n)));
    }
  };
  walk(rel ? join(root, rel) : root, rel, '');
  return out;
}

// what the hub does for /files/zip: Zip.inside, Fm.zip.within, the plan
function plan(Z, root, rel, title = 'ecc83') {
  const w = Z.zip_where(fs, pathm, root, rel);
  if (!w) return { refused: 'not found' };
  if (F.within(w[1])) return { refused: 'left out of downloads' };
  if (w[2] !== 'd') return { refused: 'not a folder' };
  const top = F.top(title, w[1]);
  return { root: w[0], rel: w[1], home: '', maxBytes: 1073741824, maxEntries: 10000, top, disposition: F.disposition(top + '.zip'), rules: F.rules(), stored: F.stored(),
    swap: F.swap(), note: F.note(), head: F.note_head(), why: F.why() };
}

function zipOf(root, rel, opts, title) {
  const { Z, effects, sent, drain } = load(opts);
  const p = plan(Z, root, rel, title);
  if (p.refused) return p;
  const { r, parks } = drain(effects.get('Zip.send')(73, p, () => {}));
  assert.deepEqual(r.result, { ok: true });
  const res = response(sent);
  assert.match(res.head, /^HTTP\/1\.1 200 OK/);
  assert.match(res.head, /Content-Type: application\/zip/);
  assert.ok(res.ended, 'the chunked body ends');
  const file = join(dir, `${p.top}-${Math.random().toString(36).slice(2)}.zip`);
  fs.writeFileSync(file, res.body);
  return { file, top: p.top, rel: p.rel, parks };
}

const py = (code, file) => execFileSync('python3', ['-I', '-c', code, file]).toString();
function check(file, root, rel, top) {
  execFileSync('unzip', ['-tq', file]);
  py('import sys, zipfile; z = zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None', file);
  const infos = JSON.parse(py('import sys, zipfile, json; print(json.dumps([[i.filename, i.compress_type] for i in zipfile.ZipFile(sys.argv[1]).infolist()]))', file));
  const listed = infos.map(i => i[0]);
  assert.ok(listed.every(n => n === top + '/' || n.startsWith(top + '/')), 'every entry is under the top folder');
  const want = expected(root, rel);
  const got = new Set(listed.map(n => n.slice(top.length + 1)).filter(n => n !== ''));
  assert.deepEqual([...got].sort(), [...want.keys()].sort());
  const unpack = (cmd, args) => {
    const out = fs.mkdtempSync(join(dir, 'out-'));
    execFileSync(cmd, [...args, out]);
    for (const [p, data] of want) {
      const at = join(out, top, p);
      if (data === null) assert.ok(fs.statSync(at).isDirectory(), p);
      else assert.ok(fs.readFileSync(at).equals(data), cmd + ': ' + p);
    }
  };
  unpack('unzip', ['-q', file, '-d']);
  return { listed, infos };
}
const ditto = (file, root, rel, top) => {
  if (process.platform !== 'darwin') return;
  const out = fs.mkdtempSync(join(dir, 'ditto-'));
  execFileSync('ditto', ['-x', '-k', file, out]);
  for (const [p, data] of expected(root, rel)) {
    const at = join(out, top, p);
    if (data === null) assert.ok(fs.statSync(at).isDirectory(), p);
    else assert.ok(fs.readFileSync(at).equals(data), 'ditto: ' + p);
  }
};

// paths built from the names and kinds the rules are about, and others
const PARTS = ['.ENV', '.AWS', 'CREDENTIALS', 'TOKEN', 'AUTH.JSON', '.git', 'node_modules', '.DS_Store', '__pycache__', '.env', '.env.local', '.envrc', 'env', 'a-backups', 'backups', 'x.lck', 'lck',
  '_autosave-x', 'autosave', '.backplane', 'history.git', 'worktrees', 'docs', 'boards', 'Schéma ✓', 'a\\b', '.github', '.backplane.json', 'mfg', '~x.kicad_sch.lck', 'x.ZIP', 'p.PnG', 'a.tar.gz', 'b.step'];
function* paths() {
  for (const a of PARTS) { yield a; for (const b of PARTS) { yield a + '/' + b; for (const c of ['history.git', 'worktrees', 'x', '.env']) yield a + '/' + b + '/' + c; } }
  yield '.backplane/history.git'; yield '.backplane/worktrees'; yield '.backplane/history.git/objects/ab'; yield '';
}

await t('directory listings refuse missing roots, traversal, symlinks and forged records', () => {
  const root = fs.mkdtempSync(join(dir, 'listing-'));
  const outside = fs.mkdtempSync(join(dir, 'outside-listing-'));
  fs.mkdirSync(join(root, 'etc')); fs.mkdirSync(join(root, 'empty'));
  fs.writeFileSync(join(root, 'note.txt'), 'inside');
  fs.writeFileSync(join(root, 'etc', 'note.txt'), 'subfolder');
  fs.writeFileSync(join(outside, 'note.txt'), 'outside');
  fs.writeFileSync(join(root, 'forged\nf\t1\toutside.txt'), 'bad record');
  fs.symlinkSync(outside, join(root, 'escape'));
  fs.symlinkSync(join(outside, 'note.txt'), join(root, 'escape.txt'));
  const effect = load().effects.get('Zip.list');
  const listing = (top, rel) => list(effect(top, rel, join(dir, 'hub-home')));
  for (const [top, rel] of [['', ''], ['', 'etc'], [root, '..'], [root, '../outside-listing'], [root, 'escape'], [root, 'escape/etc']])
    assert.deepEqual(listing(top, rel), [], JSON.stringify([top, rel]));
  assert.deepEqual(listing(root, 'empty'), ['']);
  assert.deepEqual(listing(root, 'etc'), ['f\t9\tnote.txt\n']);
  assert.deepEqual(listing(root, ''), ['d\t0\tempty\nd\t0\tetc\nf\t6\tnote.txt\n']);
});

await t('the zip writer decides as Fm.zip.skip, within, store and entry, over generated paths', () => {
  const { Z } = load();
  let n = 0;
  for (const p of paths()) {
    n++;
    assert.equal(Z.zip_skip(RULES, p), F.skip(p), 'skip ' + p);
    assert.equal(Z.zip_within(RULES, p), F.within(p), 'within ' + p);
    const name = p.slice(p.lastIndexOf('/') + 1);
    assert.equal(Z.zip_store(STORED, name), F.store(name), 'store ' + name);
    assert.equal(Z.zip_entry(F.swap(), name), F.entry(name), 'entry ' + name);
  }
  assert.ok(n > 3000, String(n));
});

await t('a rule of a kind the writer does not know is an error, not a rule that matches nothing', () => {
  const { Z } = load();
  assert.throws(() => Z.zip_skip(['?x'], 'a/x'), /unknown zip rule/);
  assert.throws(() => Z.zip_skip(['=y', '%x'], 'a/b'), /unknown zip rule/);
  assert.equal(Z.zip_skip(['=x'], 'a/x'), true);
});

await t('a project zips whole, less what the rules leave out; dotfiles but .env stay', () => {
  const root = project();
  const { file, top } = zipOf(root, '');
  const { listed, infos } = check(file, root, '', top);
  ditto(file, root, '', top);
  for (const want of ['ecc83/empty/inner/', 'ecc83/docs/schéma notes – v2.md', 'ecc83/mfg/big.bin', 'ecc83/.backplane/checklist.md',
    'ecc83/.backplane.json', 'ecc83/.gitignore', 'ecc83/.github/workflows/ci.yml']) assert.ok(listed.includes(want), want);
  for (const bad of ['.git/', 'node_modules', 'backups', '.lck', 'history.git', 'worktrees', '/.env', '.DS_Store', 'etc-link', 'readme-link', 'bp-link', 'history-link'])
    assert.ok(!listed.some(n => n.includes(bad)), bad);
  assert.ok(!listed.some(n => n.endsWith('SKIPPED.txt')), 'nothing missed, no note');
  const method = Object.fromEntries(infos);
  assert.equal(method['ecc83/mfg/photo.png'], 0, 'an image is stored');
  assert.equal(method['ecc83/boards/ecc83-pp.kicad_pcb'], 8, 'a board deflates');
  assert.ok(fs.statSync(file).size < 1536 * 1024 + 70000 + 300 * 1024, 'text deflates');
});

await t('a folder below zips under its own name', () => {
  const root = project();
  const { file, top } = zipOf(root, 'mfg');
  assert.equal(top, 'mfg');
  check(file, root, 'mfg', top);
  ditto(file, root, 'mfg', top);
});

await t('the rules go by the real path: ./.backplane, .backplane/ and a link to it keep history.git out', () => {
  const root = project();
  for (const rel of ['./.backplane', '.backplane/', 'bp-link', 'docs/../.backplane']) {
    const z = zipOf(root, rel);
    assert.equal(z.rel, '.backplane', rel);
    const { listed } = check(z.file, root, '.backplane', z.top);
    assert.ok(listed.some(n => n.endsWith('/checklist.md')), rel);
    assert.ok(!listed.some(n => n.includes('history.git') || n.includes('worktrees')), rel);
  }
  for (const rel of ['.backplane/history.git', './.backplane/history.git', 'history-link', '.backplane/history.git/', 'tools/node_modules', '.git'])
    assert.equal(zipOf(root, rel).refused, 'left out of downloads', rel);
  for (const rel of ['..', '../', 'docs/etc-link', 'nothing']) assert.equal(zipOf(root, rel).refused, 'not found', rel);
});

await t('a name with a backslash never becomes a folder, nor a path outside', () => {
  const root = project();
  // made by another program (Bun's own fs would read the backslashes as /)
  execFileSync('python3', ['-I', '-c', 'import sys; open(sys.argv[1] + "/..\\\\..\\\\evil.bat", "w").write("x"); open(sys.argv[1] + "/a\\\\b.txt", "w").write("y")', join(root, 'docs')]);
  fs.writeFileSync(join(dir, 'evil.bat'), 'outside');
  const { file, top } = zipOf(root, 'docs');
  execFileSync('unzip', ['-tq', file]);
  const names = execFileSync('unzip', ['-Z1', file]).toString().split('\n').filter(Boolean);
  assert.ok(!names.some(n => n.includes('\\') || n.includes('evil') && !n.startsWith('docs/')), JSON.stringify(names));
  const kept = ['docs/.._.._evil.bat', 'docs/a_b.txt'].filter(n => names.includes(n));
  const note = names.includes('docs/SKIPPED.txt') ? execFileSync('unzip', ['-p', file, 'docs/SKIPPED.txt']).toString() : '';
  for (const n of ['..\\..\\evil.bat', 'a\\b.txt']) assert.ok(kept.includes('docs/' + F.entry(n)) || note.includes('docs/' + n + ' (its name holds a backslash'), n + ': ' + note);
  for (const n of names) if (n.endsWith('evil.bat') && !n.includes('SKIPPED')) assert.notEqual(execFileSync('unzip', ['-p', file, n]).toString(), 'outside');
});

await t('a folder swapped for a link while the zip is made is not descended', () => {
  const root = project();
  const { Z } = load();
  const p = plan(Z, root, 'boards');
  const w = Z.zip_writer(fs, zlib, pathm, p);
  const outside = fs.mkdtempSync(join(dir, 'outside-'));
  // the same names as the folder's own files, so a walk that followed the
  // link would read them
  for (const n of fs.readdirSync(join(root, 'boards'))) fs.writeFileSync(join(outside, n), 'outside');
  fs.writeFileSync(join(outside, 'stolen.txt'), 'outside');
  const parts = [];
  let swapped = false;
  for (let b; (b = w.next()) !== null;) {
    parts.push(b);
    // after the folder's own entry, before its files are looked at
    if (!swapped) { swapped = true; fs.renameSync(join(root, 'boards'), join(root, 'boards-old')); fs.symlinkSync(outside, join(root, 'boards')); }
  }
  const file = join(dir, 'swap.zip');
  fs.writeFileSync(file, Buffer.concat(parts));
  execFileSync('unzip', ['-tq', file]);
  const names = execFileSync('unzip', ['-Z1', file]).toString().split('\n').filter(n => n && !n.endsWith('/'));
  assert.ok(!names.some(n => n.includes('stolen')), names.join(' '));
  for (const n of names) if (!n.endsWith('SKIPPED.txt')) assert.notEqual(execFileSync('unzip', ['-p', file, n]).toString(), 'outside', n);
  assert.ok(names.every(n => n.startsWith('boards/')), 'the changed tree never leaves its root');
});

await t('zip64 fields and records when sizes, offsets and the count pass the limits', () => {
  const root = project();
  const { file, top } = zipOf(root, '', { limit: 100000, count: 5 });
  check(file, root, '', top);
  const b = fs.readFileSync(file);
  assert.ok(b.indexOf(Buffer.from([0x50, 0x4b, 0x06, 0x06])) > 0, 'a zip64 end record');
  assert.ok(b.indexOf(Buffer.from([0x50, 0x4b, 0x06, 0x07])) > 0, 'a zip64 locator');
  assert.equal(b.readUInt16LE(b.length - 22 + 10), 0xFFFF, 'the end record points at zip64');
});

await t('a big folder yields: every step is bounded, and the directory goes out in pieces', () => {
  const root = fs.mkdtempSync(join(dir, 'many-'));
  for (let i = 0; i < 1500; i++) fs.writeFileSync(join(root, `node_modules-${i}.lck`), '');
  for (let i = 0; i < 700; i++) fs.writeFileSync(join(root, `f${i}.txt`), String(i));
  const { Z } = load({ step: 64 });
  const w = Z.zip_writer(fs, zlib, pathm, plan(Z, root, '', 'many'));
  let empty = 0, steps = 0, biggest = 0;
  const parts = [];
  for (let b; (b = w.next()) !== null; steps++) { if (b.length === 0) empty++; biggest = Math.max(biggest, b.length); parts.push(b); }
  // Directory iteration order varies; accepted entries may be interleaved
  // with skipped names. A separate all-skipped tree checks the yield bound.
  const skippedRoot = fs.mkdtempSync(join(dir, 'skipped-'));
  for (let i = 0; i < 1500; i++) fs.writeFileSync(join(skippedRoot, `n${i}.lck`), '');
  const skipped = Z.zip_writer(fs, zlib, pathm, plan(Z, skippedRoot, '', 'skipped'));
  let yields = 0;
  for (let b; (b = skipped.next()) !== null;) if (b.length === 0) yields++;
  skipped.close();
  assert.ok(yields >= 1500 / 64 - 1, 'skipped entries yield: ' + yields);
  assert.ok(biggest < 64 * 200, 'no step writes the whole directory: ' + biggest);
  const file = join(dir, 'many.zip');
  fs.writeFileSync(file, Buffer.concat(parts));
  execFileSync('unzip', ['-tq', file]);
  assert.equal(execFileSync('unzip', ['-Z1', file]).toString().trim().split('\n').length, 701);
});

await t('a download is found only inside the project', () => {
  const root = project();
  const { Z } = load();
  const real = fs.realpathSync(root);
  const where = rel => JSON.parse(JSON.stringify(Z.zip_where(fs, pathm, root, rel)));
  assert.deepEqual(where('docs/README.md'), [real, 'docs/README.md', 'f']);
  assert.deepEqual(where('docs/readme-link.md'), [real, 'docs/README.md', 'f']);
  assert.deepEqual(where(''), [real, '', 'd']);
  for (const rel of ['docs/etc-link/hosts', '../../etc/hosts', 'nothing.txt']) assert.equal(Z.zip_where(fs, pathm, root, rel), null, rel);
  assert.equal(F.within('history-link'), false, 'the link\'s own name is not the rule');
  assert.equal(F.within(Z.zip_where(fs, pathm, root, 'history-link')[1]), true, 'its real path is');
});

await t('download paths Windows would read as another place are refused (Fm.files.safe)', () => {
  for (const rel of ['..\\..\\x', 'a\\b', 'C:/x', 'c:', 'x:y', '//host/share', '\\\\host\\share', '/etc', '../x']) assert.equal(F.files_safe(rel), false, rel);
  for (const rel of ['', 'docs/a b.pdf', 'mfg', 'docs/a:b']) assert.equal(F.files_safe(rel), true, rel);
});


await t('an actual file response streams bytes, ranges, and unicode disposition', () => {
  const root = project();
  const { effects, sent, drain } = load();
  const name = 'schéma – ✓.md';
  fs.writeFileSync(join(root, name), 'hello unicode');
  const { r } = drain(effects.get('Zip.file')(73, fs.realpathSync(root), name, '', F.disposition(name), 'bytes=1-4', F.rules(), () => {}));
  assert.equal(r.result.ok, true);
  const res = response(sent);
  assert.match(res.head, /206 Partial Content/);
  assert.match(res.head, /filename\*=UTF-8''sch%C3%A9ma/);
  assert.equal(res.body.toString(), 'ello');
});

await t('a large folder returns 413 before any ZIP header or file content', () => {
  const root = fs.mkdtempSync(join(dir, 'huge-'));
  fs.writeFileSync(join(root, 'huge.bin'), '');
  fs.truncateSync(join(root, 'huge.bin'), 1073741825);
  const { Z, effects, sent, drain } = load();
  const p = plan(Z, root, '');
  drain(effects.get('Zip.send')(73, p, () => {}));
  const res = response(sent);
  assert.match(res.head, /413 Content Too Large/);
  assert.match(res.body.toString(), /1 GiB or 10,000/);
  assert.ok(!res.head.includes('application/zip'));
});

await t('the entry limit is enforced before streaming (also counts skipped names)', () => {
  const root = project();
  const { Z, effects, sent, drain } = load();
  const p = { ...plan(Z, root, ''), maxEntries: 3 };
  drain(effects.get('Zip.send')(73, p, () => {}));
  assert.match(response(sent).head, /413/);
});

await t('cancel during preflight, file streaming and archive streaming closes every descriptor', () => {
  const root = project();
  const fds = () => fs.readdirSync('/proc/self/fd').length;
  for (const kind of ['preflight', 'zip', 'file']) {
    const { Z, effects, sent, drain } = load({ cancelAfter: 15, disconnectBeforeHeaders: kind === 'preflight' });
    const before = fds();
    const call = kind === 'file'
      ? effects.get('Zip.file')(73, fs.realpathSync(root), 'mfg/big.bin', '', F.disposition('big.bin'), '', F.rules(), () => {})
      : effects.get('Zip.send')(73, plan(Z, root, ''), () => {});
    const { r } = drain(call);
    assert.equal(r.result.ok, false, kind);
    assert.equal(fds(), before, kind + ': open descriptors');
    if (kind === 'preflight') assert.equal(sent.length, 0, 'preflight cancel sends no bytes');
    else assert.ok(!response(sent).ended || kind === 'file');
  }
});

await t('private hub storage, credentials, hard links and symlink escapes cannot be downloaded', () => {
  const root = project();
  const home = join(root, 'private-hub');
  fs.mkdirSync(home); fs.writeFileSync(join(home, 'token'), 'private');
  const { Z, effects, sent, drain } = load();
  assert.equal(Z.zip_where(fs, pathm, root, 'private-hub/token', home), null);
  assert.equal(Z.zip_where(fs, pathm, home, '', home), null);
  assert.equal(Z.zip_where(fs, pathm, os.homedir(), '', home), null);
  fs.linkSync(join(home, 'token'), join(root, 'hardlink.txt'));
  fs.symlinkSync(home, join(root, 'symlink'));
  for (const rel of ['symlink/token', 'extra/etc-link/hosts', 'docs/etc-link/hosts', 'hardlink.txt']) {
    const a = load();
    a.drain(a.effects.get('Zip.file')(73, fs.realpathSync(root), rel, home, F.disposition('x'), '', F.rules(), () => {}));
    assert.match(response(a.sent).head, /404/, rel);
  }
  for (const p of ['.ENV', '.AWS/credentials', 'AUTH.JSON', 'secrets/key', 'vault/master', 'credentials', '.ssh/id_ed25519', '.aws/credentials', '.config/auth.json', 'auth.json', 'token']) assert.equal(F.within(p), true, p);
  const p = { ...plan(Z, root, ''), home };
  drain(effects.get('Zip.send')(73, p, () => {}));
  assert.ok(!response(sent).body.includes(Buffer.from('private')));
});

await t('authenticated requests only: the production credential and Fetch Metadata gate', () => {
  assert.equal(F.allowed('paired', '', ''), false);
  assert.equal(F.allowed('paired', 'wrong', ''), false);
  assert.equal(F.allowed('paired', 'paired', ''), true);
  assert.equal(F.allowed('paired', 'paired', 'cross-site'), false);
  assert.equal(F.allowed('', '', ''), false);
});

fs.rmSync(dir, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
