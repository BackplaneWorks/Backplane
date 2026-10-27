import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, readdirSync, statSync, mkdtempSync, rmSync, existsSync, chmodSync, mkdirSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
const require = createRequire(import.meta.url);
globalThis.require = require;

// The secret box (src/core/secret.bend, docs/secrets.md): its masks in the
// emitted runtime and with the hub's native Sec.fine (scripts/js-native.py),
// the source gate that keeps stored values in their own types, and the
// vault effect against temporary homes: fail-closed opening, staged writes
// under injected faults, bounded runs and run-scoped masking. Fixtures are
// obviously not credentials (SECRET-FIXTURE-n).
const root = resolve(dirname(import.meta.filename), '..', '..');
const raw = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const BUL = '•';
const F1 = 'SECRET-FIXTURE-1', F2 = 'SECRET-FIXTURE-22';
const ms = (f) => { const a = performance.now(); const r = f(); return [performance.now() - a, r]; };

// 0. the hub's build of the same module: minified as build-js.sh does, then
// js-native.py (Sec.fine native)
const work = mkdtempSync(join(tmpdir(), 'bp-secret-native-'));
const nat = join(work, 'secret.native.mjs');
let r0 = spawnSync('bun', ['build', '--minify-whitespace', '--minify-syntax', '--keep-names', '--target=bun', resolve(process.argv[2]), '--outfile', nat], { encoding: 'utf8' });
assert.equal(r0.status, 0, r0.stderr);
r0 = spawnSync('python3', [join(root, 'scripts/js-native.py'), nat], { encoding: 'utf8' });
assert.equal(r0.status, 0, r0.stderr + r0.stdout);
assert.match(r0.stdout, /Sec\.fine/, 'Sec.fine is made native');
assert.ok(readFileSync(nat, 'utf8').includes('){return __bpSecFine('));
const native = (await import(pathToFileURL(nat).href)).default;

// 1. masks, pure and native: no stored value survives, idempotent, length
// kept, clean text untouched, native == pure
const vals = [F1, F2, 'abcdabcd', 'cdabcdab', 'xyzxyzxyzxyz', 'FIXTURE-🦋-VALUE'];
const vs = vals.join('\n');
const texts = ['', 'nothing here', `see ${F1} ok`, F1 + F1, `x ${F2} y ${F2}`, 'abcdabcdabcdab', 'zzabcdabcdabzz', 'xyzxyzxyzxyzxyzxyz',
  `{"text":"${F1}"}`, F1.slice(0, -1), `ünïcödé ${F1} 🦋`, 'a FIXTURE-🦋-VALUE b', '\ud83d lone ' + F1 + ' \udc00'];
const alpha = ['a', 'b', 'c', 'd', 'x', 'y', 'z', '-', 'S', 'E', '🦋', 'é', '1', '2'];
const rnd = (n) => { let s = ''; for (let i = 0; i < n; i++) s += alpha[Math.floor(Math.random() * alpha.length)]; return s; };
for (let i = 0; i < 300; i++) texts.push(rnd(5) + vals[i % vals.length] + rnd(7) + vals[(i * 3) % vals.length].slice(2) + rnd(3) + vals[(i * 5) % vals.length].slice(0, 6));
for (const t of texts) {
  const m = raw.mask(vs, t);
  assert.equal(native.mask(vs, t), m, `native == pure on ${JSON.stringify(t)}`);
  for (const v of vals) assert.ok(!m.includes(v), `a value survived in ${JSON.stringify(m)}`);
  assert.equal(raw.clean(vs, m), true);
  assert.equal(raw.mask(vs, m), m, 'masking twice is masking once');
  assert.equal([...m].length, [...t].length, 'the length is kept');
  if (!vals.some((v) => t.includes(v))) assert.equal(m, t, 'clean text is left alone');
  const a = [...t], b = [...m];
  for (let k = 0; k < a.length; k++) if (a[k] !== b[k]) assert.equal(b[k], BUL);
}
assert.equal(raw.mask('', F1), F1, 'an empty vault masks nothing');
assert.equal(raw.mask(vs, `see ${F1}.`), 'see ' + BUL.repeat(F1.length) + '.');
const j = `{"a":"${F1}","${F1}":1,"l":["ok","x abcdabcd"],"n":12}`;
for (const lib of [raw, native]) {
  const jm = lib.json(vs, j);
  assert.ok(!vals.some((v) => jm.includes(v)), jm);
  assert.equal(lib.json_clean(vs, j), true);
  assert.equal(JSON.parse(jm).n, 12);
  assert.equal(lib.json(vs, '{"a":"fine"}'), '{"a":"fine"}');
}
// the cost on long lines (the hub's native build; the pure walk only small)
const two = [F1, F2].join('\n');
const timing = {};
for (const n of [1e5, 1e6, 4e6]) {
  const clean = 'x'.repeat(n), dirty = 'y'.repeat(n / 2) + F1 + 'z'.repeat(n / 2);
  const [tc, mc] = ms(() => native.mask(two, clean));
  const [td, md] = ms(() => native.mask(two, dirty));
  assert.equal(mc, clean);
  assert.ok(!md.includes(F1) && md.length === dirty.length);
  timing[n] = { clean: Math.round(tc), dirty: Math.round(td) };
}
const [tp] = ms(() => raw.mask(two, 'y'.repeat(5000) + F1 + 'z'.repeat(5000)));
timing.pure10k = Math.round(tp);
console.log('secret: native mask ms', JSON.stringify(timing));
assert.ok(timing[4e6].dirty < 5000 && timing[4e6].clean < 5000, 'a 4 MB line masks in seconds, not minutes');
rmSync(work, { recursive: true, force: true });

// names, values, handles, the request the host sends, the prompt
assert.equal(raw.name('Tailscale Bub'), 'tailscale-bub');
assert.equal(raw.name('  GH_token.v2  '), 'gh_token.v2');
assert.equal(raw.bad('x', 'short'), 'A secret is at least 8 characters');
assert.equal(raw.bad('', F1), 'Give the secret a name');
assert.match(raw.bad('x', 'has space inside'), /no spaces/);
assert.match(raw.bad('x', 'has"quote1'), /quotes/);
assert.equal(raw.bad('x', F1), '');
assert.equal(raw.handle('tailscale-bub'), '{"secret":"tailscale-bub","set":true}');
const ok = JSON.parse(raw.req(`Tailscale Bub\u001fjoin the tailnet\u001ft1\u001f ${F1} `));
assert.equal(ok.name, 'tailscale-bub');
const rpc = JSON.parse(ok.rpc);
assert.equal(rpc.m, 'secret.set');
assert.deepEqual(rpc.p, { name: 'tailscale-bub', purpose: 'join the tailnet', thread: 't1', value: F1 });
assert.match(JSON.parse(raw.req('n\u001fp\u001ft\u001fshort')).error, /at least 8/);
assert.ok(JSON.parse(raw.req('garbage')).error);
// Settings' API key Save on the web: one api-add request with the value,
// the name as an environment variable, or why nothing goes (Keys.bad)
const key = JSON.parse(raw.key_req(`openai key\u001f${F1}`));
assert.equal(key.name, 'OPENAI_KEY');
const keyRpc = JSON.parse(key.rpc);
assert.equal(keyRpc.m, 'bots.voice');
assert.deepEqual(keyRpc.p, { op: 'api-add', name: 'openai key', value: F1 });
assert.equal(JSON.parse(raw.key_req(`  \u001f${F1}`)).error, 'Give the key a name');
assert.equal(JSON.parse(raw.key_req('NAME\u001f  ')).error, 'Paste the key itself');
assert.equal(JSON.parse(raw.key_req('garbage')).error, 'Give the key a name');
assert.match(raw.prompt('[{"name":"tailscale-bub","purpose":"p"}]'), /`tailscale-bub`/);
assert.match(raw.prompt('[]'), /secret_request/);
assert.ok(!raw.prompt('[]').includes('Set now'));

// 2. the source gate: SecVal{ and Vault{ are built or taken apart only in
// secret.bend, vault.bend and the proofs, and no other type names them in
// a field (no Change, mail, prompt, render or tool-result constructor can
// hold a stored value); no provider-shaped key in the fixtures
const allowed = new Set(['src/core/secret.bend', 'src/server/vault.bend', 'test/secret_proof.bend', 'test/secret_test.bend', 'LAWS.bend', 'PROOF.bend']);
const walk = (d, out = []) => {
  for (const n of readdirSync(join(root, d))) {
    if (n.startsWith('.') || n === 'node_modules' || n === 'build' || n === 'dist') continue;
    const p = join(d, n);
    if (statSync(join(root, p)).isDirectory()) walk(p, out);
    else if (n.endsWith('.bend')) out.push(p);
  }
  return out;
};
for (const f of ['src', 'test'].flatMap((d) => walk(d)).concat(['LAWS.bend', 'PROOF.bend'])) {
  if (allowed.has(f)) continue;
  const text = readFileSync(join(root, f), 'utf8');
  assert.ok(!/\bSecVal\s*\{/.test(text), `${f} builds or matches a SecVal`);
  assert.ok(!/\.Vault\s*\{/.test(text) && !/(^|[^.\w])Vault\s*\{/m.test(text), `${f} builds or matches a Vault`);
  for (const m of text.matchAll(/^type [^\n]*\n((?:  [^\n]*\n)+)/gm)) assert.ok(!/:\s*[\w.]*\b(SecVal|Vault)\b/.test(m[1]), `${f}: a type holds a stored value: ${m[0].split('\n')[0]}`);
}
for (const f of ['src/core/secret.bend', 'test/secret_test.bend', 'test/tools/secret.mjs', 'src/server/effects/vault.js', 'docs/secrets.md']) {
  const text = readFileSync(join(root, f), 'utf8');
  const shaped = new RegExp('(' + ['ts' + 'key-', 'gh' + 'p_', '\\bs' + 'k-[A-Za-z0-9]'].join('|') + ')');
  assert.ok(!shaped.test(text), `${f} has a provider-shaped key fixture`);
}

// 3. the vault effect
process.env.BACKPLANE_VAULT_KEYRING = 'file';
process.env.BACKPLANE_VAULT_TEST = '1';
const fx = {};
globalThis.io_eff = (id, fn) => { fx[id] = fn; };
globalThis.CID = (x) => x;
globalThis.io_park_promise = (p, k, then) => p.then((v) => then(v), (e) => then(undefined, e));
globalThis.VaultFx = new Proxy({}, { get: (_, n) => n });
new Function(readFileSync(join(root, 'src/server/effects/vault.js'), 'utf8'))();
const homes = [];
const home = () => { const h = mkdtempSync(join(tmpdir(), 'bp-vault-')); homes.push(h); return h; };
const vfile = (h) => join(h, 'secrets', 'vault.json');
const bytes = (f) => existsSync(f) ? readFileSync(f).toString('hex') : '';
const run = async (spec) => JSON.parse(await fx.run(JSON.stringify(spec), null));
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };
try {
  // 3a. a new vault, sealed, 0600/0700, reopened
  let h = home();
  assert.equal(JSON.parse(fx.open(h)).n, 0);
  assert.equal(fx.values(), '');
  assert.equal(fx.put('fixture-one', F1, 'join', 'person'), 'ok');
  assert.equal(fx.put('fixture-two', F2, '', 'person'), 'ok');
  const disk = readFileSync(vfile(h), 'utf8');
  assert.ok(!disk.includes(F1) && !disk.includes(F2), 'sealed at rest');
  assert.equal(statSync(vfile(h)).mode & 0o777, 0o600);
  assert.equal(statSync(join(h, 'secrets', 'vault.key')).mode & 0o777, 0o600);
  assert.equal(statSync(join(h, 'secrets')).mode & 0o777, 0o700);
  assert.ok(!JSON.stringify(JSON.parse(fx.list())).includes(F1), 'the list carries no value');
  assert.equal(JSON.parse(fx.open(h)).n, 2);
  assert.deepEqual(fx.values().split('\n').sort(), [F1, F2]);

  // 3b. staged writes: a fault at write, fsync or rename changes nothing
  for (const step of ['write', 'fsync', 'rename']) {
    const before = { disk: bytes(vfile(h)), list: fx.list(), values: fx.values() };
    process.env.BACKPLANE_VAULT_FAULT = step;
    assert.match(fx.put('fixture-three', 'SECRET-FIXTURE-333', '', ''), new RegExp('^fail injected ' + step));
    assert.match(fx.put('fixture-one', 'SECRET-FIXTURE-444', '', ''), /^fail injected/);
    assert.match(fx.del('fixture-two'), /^fail injected/);
    delete process.env.BACKPLANE_VAULT_FAULT;
    assert.equal(bytes(vfile(h)), before.disk, `${step}: disk unchanged`);
    assert.equal(fx.list(), before.list, `${step}: memory unchanged`);
    assert.equal(fx.values(), before.values, `${step}: masking unchanged (the undeleted value is still masked)`);
    assert.deepEqual(readdirSync(join(h, 'secrets')).filter((n) => n.includes('.tmp')), [], `${step}: no temporary file left`);
    assert.equal(JSON.parse(fx.open(h)).n, 2, `${step}: reopens as before`);
    assert.deepEqual(fx.values().split('\n').sort(), [F1, F2]);
  }

  // 3b'. after the rename is committed: a real folder-flush failure (EIO)
  // still publishes (memory matches disk) but answers "committed", not ok,
  // and status says so; an unsupported flush (EINVAL) is ok, recorded
  process.env.BACKPLANE_VAULT_FAULT = 'dirsync';
  assert.match(fx.put('fixture-three', 'SECRET-FIXTURE-333', '', ''), /^committed the folder flush failed \(EIO\)/);
  delete process.env.BACKPLANE_VAULT_FAULT;
  assert.equal(JSON.parse(fx.status()).flush, 'unflushed EIO', 'status records the failed flush');
  assert.ok(fx.values().split('\n').includes('SECRET-FIXTURE-333'), 'committed: in use (masked)');
  assert.ok(readFileSync(vfile(h), 'utf8').includes('fixture-three'), 'committed: on disk');
  assert.equal(JSON.parse(fx.open(h)).n, 3, 'committed: reopens with it');
  assert.ok(fx.values().split('\n').includes('SECRET-FIXTURE-333'));
  process.env.BACKPLANE_VAULT_FAULT = 'dirsync-unsupported';
  assert.equal(fx.del('fixture-three'), 'ok', 'an unsupported folder flush is accepted');
  delete process.env.BACKPLANE_VAULT_FAULT;
  assert.equal(JSON.parse(fx.status()).flush, 'unsupported EINVAL', 'and recorded');
  assert.equal(JSON.parse(fx.open(h)).n, 2);
  assert.equal(fx.put('fixture-two', F2, '', ''), 'ok');
  assert.equal(JSON.parse(fx.status()).flush, 'durable');

  // 3c. fail closed: corrupt, foreign or unreadable vaults lock; nothing is
  // written over them and nothing re-keyed
  const good = readFileSync(vfile(h), 'utf8');
  const key = readFileSync(join(h, 'secrets', 'vault.key'), 'utf8');
  const doc = JSON.parse(good);
  const corrupt = {
    'not JSON': '{"v":1,"key":"file","items":[',
    'version 2': JSON.stringify({ ...doc, v: 2 }),
    'no version': JSON.stringify({ key: doc.key, items: doc.items }),
    'unknown key store': JSON.stringify({ ...doc, key: 'cloud' }),
    'no item list': JSON.stringify({ v: 1, key: 'file', items: {} }),
    'twice': JSON.stringify({ ...doc, items: [doc.items[0], doc.items[0]] }),
    'bad name': JSON.stringify({ ...doc, items: [{ ...doc.items[0], name: '../x' }] }),
    'no sealed value': JSON.stringify({ ...doc, items: [{ name: 'fixture-one' }] }),
    'tampered': JSON.stringify({ ...doc, items: [doc.items[0], { ...doc.items[1], ct: Buffer.from('SECRET-FIXTURE-NOT-SEALED-AT-ALL').toString('base64') }] }),
    'swapped': JSON.stringify({ ...doc, items: [{ ...doc.items[0], iv: doc.items[1].iv, ct: doc.items[1].ct }, { ...doc.items[1], iv: doc.items[0].iv, ct: doc.items[0].ct }] }),
  };
  for (const [what, text] of Object.entries(corrupt)) {
    const c = home();
    mkdirSync(join(c, 'secrets'), { mode: 0o700 });
    writeFileSync(vfile(c), text, { mode: 0o600 });
    writeFileSync(join(c, 'secrets', 'vault.key'), key, { mode: 0o600 });
    const st = JSON.parse(fx.open(c));
    assert.equal(st.locked, true, `${what}: locked`);
    assert.ok(st.why, `${what}: says why`);
    const kb = bytes(join(c, 'secrets', 'vault.key'));
    assert.match(fx.put('fixture-new', 'SECRET-FIXTURE-555', '', ''), /^fail the vault is locked/, `${what}: no save`);
    assert.match(fx.del('fixture-one'), /^fail the vault is locked/, `${what}: no delete`);
    assert.match((await run({ command: 'true' })).error, /locked/, `${what}: no run`);
    assert.equal(readFileSync(vfile(c), 'utf8'), text, `${what}: the evidence is untouched`);
    assert.equal(bytes(join(c, 'secrets', 'vault.key')), kb, `${what}: not re-keyed`);
    if (what === 'tampered') {
      assert.deepEqual(JSON.parse(fx.list()).map((m) => m.name), ['fixture-one', 'fixture-two'], 'tampered: every name still listed');
      assert.equal(fx.values(), F1, 'tampered: what did open is still masked');
    }
  }
  // unreadable (not as root) and a missing key
  if (process.getuid && process.getuid() !== 0) {
    const c = home();
    mkdirSync(join(c, 'secrets'), { mode: 0o700 });
    writeFileSync(vfile(c), good, { mode: 0o600 });
    chmodSync(vfile(c), 0o000);
    const st = JSON.parse(fx.open(c));
    assert.equal(st.locked, true);
    assert.match(st.why, /could not be read/);
    assert.match(fx.put('x-name', 'SECRET-FIXTURE-666', '', ''), /locked/);
    chmodSync(vfile(c), 0o600);
    assert.equal(readFileSync(vfile(c), 'utf8'), good);
  }
  {
    const c = home();
    mkdirSync(join(c, 'secrets'), { mode: 0o700 });
    writeFileSync(vfile(c), good, { mode: 0o600 });
    const st = JSON.parse(fx.open(c));
    assert.equal(st.locked, true);
    assert.match(fx.put('x-name', 'SECRET-FIXTURE-777', '', ''), /locked/);
    assert.ok(!existsSync(join(c, 'secrets', 'vault.key')), 'a missing key is not made again');
    writeFileSync(join(c, 'secrets', 'vault.key'), require('node:crypto').randomBytes(32).toString('hex') + '\n', { mode: 0o600 });
    assert.equal(JSON.parse(fx.open(c)).locked, true, 'the wrong key: locked');
    assert.equal(fx.values(), '');
  }

  // 3d. runs: env and a 0600 file, removed after
  assert.equal(JSON.parse(fx.open(h)).locked, false);
  let r = await run({ command: `test "$A" = ${F1} && stat -c %a "$B" && cat "$B"`, env: { A: 'fixture-one' }, files: { B: 'fixture-two' }, cwd: h, thread: 't1' });
  assert.equal(r.code, 0, JSON.stringify(r));
  assert.ok(r.stdout.startsWith('600\n') && !r.stdout.includes(F2) && r.stdout.includes(BUL.repeat(F2.length)), 'the run masks what it injected');
  assert.equal(readdirSync(join(h, 'secrets', 'run')).length, 0, 'temp files removed');
  assert.equal(JSON.parse(fx.list()).find((m) => m.name === 'fixture-two').usedBy, 't1');
  assert.match((await run({ command: 'true', env: { X: 'nope' } })).error, /no secret named nope/);
  r = await run({ command: 'exit 7' });
  assert.equal(r.code, 7, 'the real exit code');
  // escaped forms of an injected value are masked too
  assert.equal(fx.put('fixture-odd', 'SECRET/FIXTURE+8=', '', ''), 'ok');
  r = await run({ command: 'printf "%s %s" "$O" "SECRET%2FFIXTURE%2B8%3D"', env: { O: 'fixture-odd' } });
  assert.ok(!r.stdout.includes('SECRET/FIXTURE+8=') && !r.stdout.includes('SECRET%2FFIXTURE%2B8%3D'), r.stdout);
  // bounded output: 50 MB of output keeps only the last 64K characters
  let t0 = Date.now();
  r = await run({ command: 'yes a | head -c 50000000; echo end' });
  assert.equal(r.code, 0);
  assert.ok(r.stdout.length < 66000 && /characters cut/.test(r.stdout) && r.stdout.endsWith('end\n'), `bounded: ${r.stdout.length}`);
  // a descendant that keeps the pipes: settled ~2 s after the exit, the group killed
  t0 = Date.now();
  r = await run({ command: 'sleep 30 & echo $!' });
  const kid = Number(r.stdout.trim());
  assert.equal(r.code, 0);
  assert.ok(Date.now() - t0 < 6000, `settled after the exit: ${Date.now() - t0} ms`);
  await new Promise((ok) => setTimeout(ok, 200));
  assert.equal(alive(kid), false, 'the descendant was ended with the group');
  // a timeout: the group gets SIGTERM, SIGKILL; settled, code 124, files removed
  t0 = Date.now();
  r = await run({ command: 'sleep 30 & echo $!; sleep 30', timeout: 1, files: { B: 'fixture-two' } });
  assert.equal(r.code, 124);
  assert.equal(r.timedOut, true);
  assert.ok(Date.now() - t0 < 9000, `timeout settled: ${Date.now() - t0} ms`);
  await new Promise((ok) => setTimeout(ok, 200));
  assert.equal(alive(Number(r.stdout.trim())), false, 'the timed-out group is gone');
  assert.equal(readdirSync(join(h, 'secrets', 'run')).length, 0, 'temp files removed after a timeout');
  // the kill guard: no empty, 0 or 1 pid is ever signalled
  const kills = [];
  const real = process.kill;
  process.kill = (...a) => { kills.push(a); };
  try { for (const p of [undefined, null, '', 0, 1, -1, NaN, '1234', 1.5]) assert.equal(globalThis.BP_VAULT_KILL(p, 'SIGKILL'), false); } finally { process.kill = real; }
  assert.deepEqual(kills, [], 'nothing signalled');
  // the redaction race: a value deleted or replaced while its run works
  assert.equal(fx.put('fixture-race', 'SECRET-FIXTURE-RACE-1', '', ''), 'ok');
  let pr = run({ command: 'sleep 1; echo "$R"', env: { R: 'fixture-race' } });
  assert.equal(fx.del('fixture-race'), 'ok');
  r = await pr;
  assert.ok(!r.stdout.includes('SECRET-FIXTURE-RACE-1') && r.stdout.includes(BUL), `deleted mid-run: ${r.stdout}`);
  assert.equal(fx.put('fixture-race', 'SECRET-FIXTURE-RACE-2', '', ''), 'ok');
  pr = run({ command: 'sleep 1; echo "$R" >&2', env: { R: 'fixture-race' } });
  assert.equal(fx.put('fixture-race', 'SECRET-FIXTURE-RACE-3', '', ''), 'ok');
  r = await pr;
  assert.ok(!r.stderr.includes('SECRET-FIXTURE-RACE-2'), `replaced mid-run: ${r.stderr}`);
} finally {
  for (const x of homes) { try { chmodSync(join(x, 'secrets', 'vault.json'), 0o600); } catch {} rmSync(x, { recursive: true, force: true }); }
}
console.log('secret: ok');
